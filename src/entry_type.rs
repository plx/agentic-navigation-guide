//! Filesystem entry-type classification.
//!
//! [`classify_observation`] is verified by Verus against the v0.2 contract
//! (`docs/v0.2-contract.md`, "Filesystem entry types"): its specification
//! `observation_class` states the exact production priority order, and
//! `theorem_fail_closed` proves that an entry is accepted only when the
//! platform reports exactly one of "regular file" or "directory" and nothing
//! that marks a link, reparse point, FIFO, socket, or device. Every other
//! observation, including none or both of the supported kinds, fails closed.
//!
//! [`classify_path`], [`classify_metadata`], and the Windows reparse-tag
//! query read host metadata, which Verus does not model; they only build the
//! observation that the verified classifier decides.

use std::fmt;
use std::fs::{self, Metadata};
use std::io;
use std::path::Path;

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SupportedEntryKind {
    RegularFile,
    Directory,
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
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

/// What the platform reported about one entry, without following it.
#[cfg_attr(verus_keep_ghost, verus_verify)]
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

#[cfg(verus_keep_ghost)]
verus! {

/// The observation carries any Windows reparse evidence: the reparse
/// attribute or a reparse tag.
pub(crate) open spec fn has_reparse_evidence(o: EntryTypeObservation) -> bool {
    o.is_windows_reparse_point || o.windows_reparse_tag is Some
}

/// The observation reports a Unix special file or a symbolic link.
pub(crate) open spec fn has_special_kind(o: EntryTypeObservation) -> bool {
    o.is_symbolic_link || o.is_fifo || o.is_unix_domain_socket || o.is_character_device
        || o.is_block_device
}

/// The classification of one observation, in production priority order:
/// reparse evidence (carrying the observed tag), then symbolic link, FIFO,
/// Unix-domain socket, character device, and block device; otherwise exactly
/// one of regular file or directory is supported and anything else is
/// unknown.
pub(crate) open spec fn observation_class(o: EntryTypeObservation) -> EntryClassification {
    if has_reparse_evidence(o) {
        Err(UnsupportedEntryKind::WindowsReparsePoint { tag: o.windows_reparse_tag })
    } else if o.is_symbolic_link {
        Err(UnsupportedEntryKind::SymbolicLink)
    } else if o.is_fifo {
        Err(UnsupportedEntryKind::Fifo)
    } else if o.is_unix_domain_socket {
        Err(UnsupportedEntryKind::UnixDomainSocket)
    } else if o.is_character_device {
        Err(UnsupportedEntryKind::CharacterDevice)
    } else if o.is_block_device {
        Err(UnsupportedEntryKind::BlockDevice)
    } else if o.is_regular_file && !o.is_directory {
        Ok(SupportedEntryKind::RegularFile)
    } else if o.is_directory && !o.is_regular_file {
        Ok(SupportedEntryKind::Directory)
    } else {
        Err(UnsupportedEntryKind::Unknown)
    }
}

/// Fail closed: an entry is supported exactly when there is no reparse
/// evidence, no symbolic-link, FIFO, socket, or device flag, and exactly one
/// of "regular file" and "directory" is reported; the supported kind is the
/// reported one.
pub(crate) proof fn theorem_fail_closed(o: EntryTypeObservation)
    ensures
        (observation_class(o) is Ok) == (!has_reparse_evidence(o) && !has_special_kind(o) && (
        o.is_regular_file != o.is_directory)),
        observation_class(o) == Ok::<SupportedEntryKind, UnsupportedEntryKind>(
            SupportedEntryKind::RegularFile,
        ) ==> o.is_regular_file && !o.is_directory,
        observation_class(o) == Ok::<SupportedEntryKind, UnsupportedEntryKind>(
            SupportedEntryKind::Directory,
        ) ==> o.is_directory && !o.is_regular_file,
{
}

/// Reparse evidence outranks every other flag, and the reported tag is
/// preserved exactly.
pub(crate) proof fn theorem_reparse_first(o: EntryTypeObservation)
    requires
        has_reparse_evidence(o),
    ensures
        observation_class(o) == Err::<SupportedEntryKind, UnsupportedEntryKind>(
            UnsupportedEntryKind::WindowsReparsePoint { tag: o.windows_reparse_tag },
        ),
{
}

/// An entry reported as both or neither of the supported kinds is unknown
/// unless a higher-priority flag names it.
pub(crate) proof fn theorem_ambiguous_is_unknown(o: EntryTypeObservation)
    requires
        !has_reparse_evidence(o),
        !has_special_kind(o),
        o.is_regular_file == o.is_directory,
    ensures
        observation_class(o) == Err::<SupportedEntryKind, UnsupportedEntryKind>(
            UnsupportedEntryKind::Unknown,
        ),
{
}

} // verus!

/// Classify one observation, failing closed on every entry that is not
/// exactly a regular file or exactly a directory.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == observation_class(observation),
))]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every observation: all 2^8 flag combinations with and without a tag.
    fn observations() -> Vec<EntryTypeObservation> {
        let mut all = Vec::new();
        for bits in 0u16..256 {
            for tag in [None, Some(0xA000_000C)] {
                let flag = |n: u16| bits & (1 << n) != 0;
                all.push(EntryTypeObservation {
                    is_regular_file: flag(0),
                    is_directory: flag(1),
                    is_symbolic_link: flag(2),
                    is_fifo: flag(3),
                    is_unix_domain_socket: flag(4),
                    is_character_device: flag(5),
                    is_block_device: flag(6),
                    is_windows_reparse_point: flag(7),
                    windows_reparse_tag: tag,
                });
            }
        }
        all
    }

    #[test]
    fn classification_follows_the_priority_order() {
        for observation in observations() {
            let special = [
                (
                    observation.is_symbolic_link,
                    UnsupportedEntryKind::SymbolicLink,
                ),
                (observation.is_fifo, UnsupportedEntryKind::Fifo),
                (
                    observation.is_unix_domain_socket,
                    UnsupportedEntryKind::UnixDomainSocket,
                ),
                (
                    observation.is_character_device,
                    UnsupportedEntryKind::CharacterDevice,
                ),
                (
                    observation.is_block_device,
                    UnsupportedEntryKind::BlockDevice,
                ),
            ];
            let expected = if observation.is_windows_reparse_point
                || observation.windows_reparse_tag.is_some()
            {
                Err(UnsupportedEntryKind::WindowsReparsePoint {
                    tag: observation.windows_reparse_tag,
                })
            } else if let Some((_, kind)) = special.iter().find(|(flag, _)| *flag) {
                Err(*kind)
            } else if observation.is_regular_file && !observation.is_directory {
                Ok(SupportedEntryKind::RegularFile)
            } else if observation.is_directory && !observation.is_regular_file {
                Ok(SupportedEntryKind::Directory)
            } else {
                Err(UnsupportedEntryKind::Unknown)
            };
            assert_eq!(
                classify_observation(observation),
                expected,
                "{observation:?}"
            );
        }
    }

    #[test]
    fn only_exactly_one_supported_kind_without_other_flags_is_accepted() {
        for observation in observations() {
            let clean = !observation.is_windows_reparse_point
                && observation.windows_reparse_tag.is_none()
                && !observation.is_symbolic_link
                && !observation.is_fifo
                && !observation.is_unix_domain_socket
                && !observation.is_character_device
                && !observation.is_block_device;
            let accepted = clean && observation.is_regular_file != observation.is_directory;
            assert_eq!(
                classify_observation(observation).is_ok(),
                accepted,
                "{observation:?}"
            );
        }
    }

    #[test]
    fn unsupported_kinds_have_stable_descriptions() {
        let cases = [
            (UnsupportedEntryKind::SymbolicLink, "symbolic link"),
            (UnsupportedEntryKind::Fifo, "FIFO"),
            (UnsupportedEntryKind::UnixDomainSocket, "Unix-domain socket"),
            (UnsupportedEntryKind::CharacterDevice, "character device"),
            (UnsupportedEntryKind::BlockDevice, "block device"),
            (
                UnsupportedEntryKind::WindowsReparsePoint {
                    tag: Some(0xA000_000C),
                },
                "Windows reparse point (tag 0xA000000C)",
            ),
            (
                UnsupportedEntryKind::WindowsReparsePoint { tag: None },
                "Windows reparse point",
            ),
            (
                UnsupportedEntryKind::Unknown,
                "unknown filesystem entry type",
            ),
        ];
        for (kind, text) in cases {
            assert_eq!(kind.to_string(), text);
        }
    }
}
