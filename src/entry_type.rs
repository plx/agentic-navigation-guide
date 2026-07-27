use std::fmt;
use std::fs::{self, Metadata};
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SupportedEntryKind {
    RegularFile,
    Directory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnsupportedEntryKind {
    SymbolicLink,
    Fifo,
    UnixDomainSocket,
    CharacterDevice,
    BlockDevice,
    WindowsReparsePoint { tag: Option<u32> },
    Unknown,
}

impl fmt::Display for UnsupportedEntryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SymbolicLink => formatter.write_str("symbolic link"),
            Self::Fifo => formatter.write_str("FIFO"),
            Self::UnixDomainSocket => formatter.write_str("Unix-domain socket"),
            Self::CharacterDevice => formatter.write_str("character device"),
            Self::BlockDevice => formatter.write_str("block device"),
            Self::WindowsReparsePoint { tag: Some(tag) } => {
                write!(formatter, "Windows reparse point (tag 0x{tag:08X})")
            }
            Self::WindowsReparsePoint { tag: None } => formatter.write_str("Windows reparse point"),
            Self::Unknown => formatter.write_str("unknown filesystem entry type"),
        }
    }
}

pub(crate) type EntryClassification = std::result::Result<SupportedEntryKind, UnsupportedEntryKind>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct EntryTypeObservation {
    pub(crate) is_regular_file: bool,
    pub(crate) is_directory: bool,
    pub(crate) is_symbolic_link: bool,
    pub(crate) is_fifo: bool,
    pub(crate) is_unix_domain_socket: bool,
    pub(crate) is_character_device: bool,
    pub(crate) is_block_device: bool,
    pub(crate) is_windows_reparse_point: bool,
    pub(crate) windows_reparse_tag: Option<u32>,
}

pub(crate) fn classify_observation(observation: EntryTypeObservation) -> EntryClassification {
    if observation.is_windows_reparse_point || observation.windows_reparse_tag.is_some() {
        return Err(UnsupportedEntryKind::WindowsReparsePoint {
            tag: observation.windows_reparse_tag,
        });
    }
    if observation.is_symbolic_link {
        return Err(UnsupportedEntryKind::SymbolicLink);
    }
    if observation.is_fifo {
        return Err(UnsupportedEntryKind::Fifo);
    }
    if observation.is_unix_domain_socket {
        return Err(UnsupportedEntryKind::UnixDomainSocket);
    }
    if observation.is_character_device {
        return Err(UnsupportedEntryKind::CharacterDevice);
    }
    if observation.is_block_device {
        return Err(UnsupportedEntryKind::BlockDevice);
    }

    match (observation.is_regular_file, observation.is_directory) {
        (true, false) => Ok(SupportedEntryKind::RegularFile),
        (false, true) => Ok(SupportedEntryKind::Directory),
        (false, false) | (true, true) => Err(UnsupportedEntryKind::Unknown),
    }
}

pub(crate) fn classify_path(path: &Path) -> io::Result<EntryClassification> {
    let metadata = fs::symlink_metadata(path)?;
    classify_metadata(path, &metadata)
}

pub(crate) fn classify_metadata(
    path: &Path,
    metadata: &Metadata,
) -> io::Result<EntryClassification> {
    #[cfg(not(windows))]
    let _ = path;

    let file_type = metadata.file_type();
    let mut observation = EntryTypeObservation {
        is_regular_file: file_type.is_file(),
        is_directory: file_type.is_dir(),
        is_symbolic_link: file_type.is_symlink(),
        ..EntryTypeObservation::default()
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;

        observation.is_fifo = file_type.is_fifo();
        observation.is_unix_domain_socket = file_type.is_socket();
        observation.is_character_device = file_type.is_char_device();
        observation.is_block_device = file_type.is_block_device();
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

        observation.is_windows_reparse_point =
            metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
        if observation.is_windows_reparse_point {
            observation.windows_reparse_tag = Some(windows_reparse_tag(path)?);
        }
    }

    Ok(classify_observation(observation))
}

#[cfg(windows)]
fn windows_reparse_tag(path: &Path) -> io::Result<u32> {
    use std::fs::OpenOptions;
    use std::mem::{size_of, MaybeUninit};
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileAttributeTagInfo, GetFileInformationByHandleEx, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let file = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let handle = file.as_raw_handle().cast();
    let mut information = MaybeUninit::<FILE_ATTRIBUTE_TAG_INFO>::zeroed();
    // SAFETY: `information` is the exact writable structure requested by
    // FileAttributeTagInfo, and `handle` remains owned by `file`.
    let succeeded = unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileAttributeTagInfo,
            information.as_mut_ptr().cast(),
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful API call initialized the complete structure.
    let information = unsafe { information.assume_init() };
    if information.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0 {
        return Err(io::Error::other(
            "filesystem entry changed while its reparse tag was inspected",
        ));
    }

    Ok(information.ReparseTag)
}
