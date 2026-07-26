//! Shared guide-path classification and non-following opening.
//!
//! This file is compiled privately into both the legacy library target and
//! the CLI binary. Keeping one implementation source avoids adding a public
//! Rust API solely to bridge those two temporary crate targets.

use std::error::Error;
use std::fmt;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

const MAX_DIAGNOSTIC_CHARS: usize = 320;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GuideAuthority {
    Implicit,
    Explicit,
}

#[derive(Debug)]
pub(crate) enum GuideInputError {
    InvalidName {
        name: String,
        reason: &'static str,
    },
    InvalidPath {
        path: PathBuf,
        reason: &'static str,
    },
    InvalidAnchor {
        path: PathBuf,
        reason: &'static str,
    },
    UnsafePath {
        path: PathBuf,
        reason: &'static str,
    },
    Io {
        path: PathBuf,
        operation: &'static str,
        source: io::Error,
    },
}

impl fmt::Display for GuideInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName { name, reason } => write!(
                formatter,
                "invalid implicit guide name {}: {reason}",
                bounded_debug(name)
            ),
            Self::InvalidPath { path, reason } => write!(
                formatter,
                "invalid explicit guide path {}: {reason}",
                render_path(path)
            ),
            Self::InvalidAnchor { path, reason } => write!(
                formatter,
                "invalid guide trust anchor {}: {reason}",
                render_path(path)
            ),
            Self::UnsafePath { path, reason } => {
                write!(
                    formatter,
                    "unsafe guide path {}: {reason}",
                    render_path(path)
                )
            }
            Self::Io {
                path,
                operation,
                source,
            } => write!(
                formatter,
                "could not {operation} guide path {} ({:?})",
                render_path(path),
                source.kind()
            ),
        }
    }
}

impl Error for GuideInputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GuideAnchor {
    current_dir: PathBuf,
    spelling: PathBuf,
    canonical: PathBuf,
}

impl GuideAnchor {
    pub(crate) fn new(path: &Path) -> Result<Self, GuideInputError> {
        let current_dir = std::env::current_dir().map_err(|source| GuideInputError::Io {
            path: PathBuf::from("."),
            operation: "resolve the current directory for",
            source,
        })?;
        let spelling = lexical_absolute(path, &current_dir, path)?;
        let canonical = fs::canonicalize(&spelling).map_err(|source| GuideInputError::Io {
            path: path.to_path_buf(),
            operation: "resolve the trust anchor for",
            source,
        })?;
        let metadata = fs::metadata(&canonical).map_err(|source| GuideInputError::Io {
            path: path.to_path_buf(),
            operation: "inspect the trust anchor for",
            source,
        })?;
        if !metadata.is_dir() {
            return Err(GuideInputError::InvalidAnchor {
                path: path.to_path_buf(),
                reason: "the selected anchor is not a directory",
            });
        }

        Ok(Self {
            current_dir,
            spelling,
            canonical,
        })
    }

    pub(crate) fn validate_implicit(
        &self,
        path: &Path,
        logical_path: &Path,
    ) -> Result<(), GuideInputError> {
        let candidate = lexical_absolute(path, &self.current_dir, path)?;
        let tail =
            safe_tail(&self.spelling, &candidate).ok_or_else(|| GuideInputError::UnsafePath {
                path: logical_path.to_path_buf(),
                reason: "an implicit guide must remain beneath its trust anchor",
            })?;
        self.validate_beneath(&candidate, tail, logical_path)?;
        validate_exact_implicit_entry(&candidate, logical_path)
    }

    pub(crate) fn read(
        &self,
        path: &Path,
        logical_path: &Path,
        authority: GuideAuthority,
    ) -> Result<String, GuideInputError> {
        // Validate every concrete path before entry access. CLI inputs and
        // manually constructed legacy GuideLocations are also prevalidated
        // before anchor construction; this remains defense in depth.
        validate_explicit_path(path)?;

        let candidate = lexical_absolute(path, &self.current_dir, path)?;
        let metadata = match safe_tail(&self.spelling, &candidate) {
            Some(tail) => {
                let metadata = self.validate_beneath(&candidate, tail, logical_path)?;
                if authority == GuideAuthority::Implicit {
                    validate_exact_implicit_entry(&candidate, logical_path)?;
                }
                metadata
            }
            None if authority == GuideAuthority::Explicit => {
                validate_final_entry(&candidate, logical_path)?
            }
            None => {
                return Err(GuideInputError::UnsafePath {
                    path: logical_path.to_path_buf(),
                    reason: "an implicit guide must remain beneath its trust anchor",
                });
            }
        };

        open_and_read(&candidate, logical_path, &metadata)
    }

    fn validate_beneath(
        &self,
        candidate: &Path,
        tail: &Path,
        logical_path: &Path,
    ) -> Result<Metadata, GuideInputError> {
        let components: Vec<_> = tail.components().collect();
        if components.is_empty() {
            return Err(GuideInputError::UnsafePath {
                path: logical_path.to_path_buf(),
                reason: "the selected guide path names the trust anchor, not a regular file",
            });
        }

        let mut current = self.spelling.clone();
        for (index, component) in components.iter().enumerate() {
            let Component::Normal(name) = component else {
                return Err(GuideInputError::UnsafePath {
                    path: logical_path.to_path_buf(),
                    reason: "the guide path has an unresolved parent, root, or prefix below its trust anchor",
                });
            };
            current.push(name);
            let metadata =
                fs::symlink_metadata(&current).map_err(|source| GuideInputError::Io {
                    path: logical_path.to_path_buf(),
                    operation: "inspect",
                    source,
                })?;
            if is_link_like(&metadata) {
                return Err(GuideInputError::UnsafePath {
                    path: logical_path.to_path_buf(),
                    reason: if index + 1 == components.len() {
                        "the final guide entry is a link or reparse point"
                    } else {
                        "a guide-path ancestor below the trust anchor is a link or reparse point"
                    },
                });
            }
            if index + 1 < components.len() && !metadata.is_dir() {
                return Err(GuideInputError::UnsafePath {
                    path: logical_path.to_path_buf(),
                    reason: "a guide-path ancestor below the trust anchor is not a directory",
                });
            }
        }

        let metadata = validate_final_entry(candidate, logical_path)?;
        let canonical_candidate =
            fs::canonicalize(candidate).map_err(|source| GuideInputError::Io {
                path: logical_path.to_path_buf(),
                operation: "resolve",
                source,
            })?;
        if !canonical_candidate.starts_with(&self.canonical) {
            return Err(GuideInputError::UnsafePath {
                path: logical_path.to_path_buf(),
                reason: "the guide resolves outside its canonical trust anchor",
            });
        }
        Ok(metadata)
    }
}

pub(crate) fn validate_implicit_name(name: &str) -> Result<(), GuideInputError> {
    if name.is_empty() {
        return Err(GuideInputError::InvalidName {
            name: name.to_string(),
            reason: "the name must contain exactly one nonempty filename component",
        });
    }
    if name.contains(['/', '\\']) {
        return Err(GuideInputError::InvalidName {
            name: name.to_string(),
            reason: "directory separators are not allowed",
        });
    }

    let path = Path::new(name);
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(GuideInputError::InvalidName {
            name: name.to_string(),
            reason: "the name must be one ordinary relative filename component",
        });
    }

    #[cfg(windows)]
    validate_windows_component(path, path.as_os_str(), false)?;

    Ok(())
}

pub(crate) fn is_link_like(metadata: &Metadata) -> bool {
    platform_is_link_like(metadata)
}

pub(crate) fn render_path(path: &Path) -> String {
    match path.to_str() {
        Some(text)
            if text
                .chars()
                .all(|character| !character.is_control() && character != '\u{7f}') =>
        {
            bounded_text(text)
        }
        _ => bounded_debug(path),
    }
}

pub(crate) fn validate_explicit_path(path: &Path) -> Result<(), GuideInputError> {
    validate_explicit_path_spelling(path)
}

fn safe_tail<'a>(anchor: &Path, candidate: &'a Path) -> Option<&'a Path> {
    let tail = candidate.strip_prefix(anchor).ok()?;
    if tail.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        None
    } else {
        Some(tail)
    }
}

fn validate_final_entry(path: &Path, logical_path: &Path) -> Result<Metadata, GuideInputError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| GuideInputError::Io {
        path: logical_path.to_path_buf(),
        operation: "inspect",
        source,
    })?;
    if is_link_like(&metadata) {
        return Err(GuideInputError::UnsafePath {
            path: logical_path.to_path_buf(),
            reason: "the final guide entry is a link or reparse point",
        });
    }
    if !metadata.is_file() {
        return Err(GuideInputError::UnsafePath {
            path: logical_path.to_path_buf(),
            reason: if metadata.is_dir() {
                "the selected guide entry is a directory"
            } else {
                "the selected guide entry is not a regular file"
            },
        });
    }
    Ok(metadata)
}

#[cfg(windows)]
fn validate_exact_implicit_entry(path: &Path, logical_path: &Path) -> Result<(), GuideInputError> {
    let parent = path.parent().ok_or_else(|| GuideInputError::UnsafePath {
        path: logical_path.to_path_buf(),
        reason: "the implicit guide has no containing directory",
    })?;
    let expected_name = path
        .file_name()
        .ok_or_else(|| GuideInputError::UnsafePath {
            path: logical_path.to_path_buf(),
            reason: "the implicit guide has no filename component",
        })?;
    let entries = fs::read_dir(parent).map_err(|source| GuideInputError::Io {
        path: logical_path.to_path_buf(),
        operation: "enumerate the containing directory for",
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| GuideInputError::Io {
            path: logical_path.to_path_buf(),
            operation: "enumerate the containing directory for",
            source,
        })?;
        if entry.file_name() == expected_name {
            return Ok(());
        }
    }

    Err(GuideInputError::UnsafePath {
        path: logical_path.to_path_buf(),
        reason: "the implicit name did not exactly match an enumerated directory entry",
    })
}

#[cfg(not(windows))]
fn validate_exact_implicit_entry(
    _path: &Path,
    _logical_path: &Path,
) -> Result<(), GuideInputError> {
    Ok(())
}

fn open_and_read(
    path: &Path,
    logical_path: &Path,
    path_metadata: &Metadata,
) -> Result<String, GuideInputError> {
    let path_identity =
        path_identity(path, path_metadata).map_err(|source| GuideInputError::Io {
            path: logical_path.to_path_buf(),
            operation: "record the identity of",
            source,
        })?;

    let mut options = OpenOptions::new();
    options.read(true);
    configure_nonfollowing_open(&mut options);
    let mut file = options.open(path).map_err(|source| {
        if is_no_follow_error(&source) {
            GuideInputError::UnsafePath {
                path: logical_path.to_path_buf(),
                reason: "the final guide entry became a link or reparse point",
            }
        } else {
            GuideInputError::Io {
                path: logical_path.to_path_buf(),
                operation: "open",
                source,
            }
        }
    })?;

    validate_regular_handle(&file).map_err(|source| GuideInputError::Io {
        path: logical_path.to_path_buf(),
        operation: "validate the opened handle for",
        source,
    })?;
    let opened_identity = handle_identity(&file).map_err(|source| GuideInputError::Io {
        path: logical_path.to_path_buf(),
        operation: "record the opened identity of",
        source,
    })?;
    if path_identity != opened_identity {
        return Err(GuideInputError::UnsafePath {
            path: logical_path.to_path_buf(),
            reason: "the guide entry identity changed while it was being opened",
        });
    }

    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|source| GuideInputError::Io {
            path: logical_path.to_path_buf(),
            operation: "read",
            source,
        })?;
    validate_regular_handle(&file).map_err(|source| GuideInputError::Io {
        path: logical_path.to_path_buf(),
        operation: "revalidate the opened handle for",
        source,
    })?;
    let completed_identity = handle_identity(&file).map_err(|source| GuideInputError::Io {
        path: logical_path.to_path_buf(),
        operation: "revalidate the opened identity of",
        source,
    })?;
    if completed_identity != opened_identity {
        return Err(GuideInputError::UnsafePath {
            path: logical_path.to_path_buf(),
            reason: "the opened guide identity changed while it was being read",
        });
    }
    Ok(content)
}

fn lexical_absolute(
    path: &Path,
    current_dir: &Path,
    logical_path: &Path,
) -> Result<PathBuf, GuideInputError> {
    #[cfg(not(windows))]
    let _ = logical_path;

    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    #[cfg(windows)]
    {
        use std::path::Prefix;

        match path.components().next() {
            Some(Component::Prefix(component)) => match component.kind() {
                Prefix::Disk(_) => {
                    return Err(GuideInputError::InvalidPath {
                        path: logical_path.to_path_buf(),
                        reason: "drive-relative guide paths are not supported",
                    });
                }
                _ => {
                    return Err(GuideInputError::InvalidPath {
                        path: logical_path.to_path_buf(),
                        reason: "the guide path uses an unsupported Windows prefix",
                    });
                }
            },
            Some(Component::RootDir) => {
                return Err(GuideInputError::InvalidPath {
                    path: logical_path.to_path_buf(),
                    reason: "current-drive-root-relative guide paths are not supported",
                });
            }
            _ => {}
        }
    }

    Ok(current_dir.join(path))
}

fn bounded_debug(value: &(impl fmt::Debug + ?Sized)) -> String {
    let rendered = format!("{value:?}");
    bounded_text(&rendered)
}

fn bounded_text(rendered: &str) -> String {
    let mut characters = rendered.chars();
    let bounded: String = characters.by_ref().take(MAX_DIAGNOSTIC_CHARS).collect();
    if characters.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}

#[cfg(unix)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
fn path_identity(_path: &Path, metadata: &Metadata) -> io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;

    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(unix)]
fn handle_identity(file: &File) -> io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata()?;
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(unix)]
fn validate_regular_handle(file: &File) -> io::Result<()> {
    if file.metadata()?.file_type().is_file() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "the opened guide handle is not a regular file",
        ))
    }
}

#[cfg(unix)]
fn configure_nonfollowing_open(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(unix)]
fn platform_is_link_like(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(unix)]
fn is_no_follow_error(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::ELOOP)
}

#[cfg(windows)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum FileIdentity {
    Extended {
        volume_serial: u64,
        file_id: [u8; 16],
    },
    Legacy {
        volume_serial: u32,
        file_index: u64,
    },
}

#[cfg(windows)]
fn windows_handle_information(file: &File) -> io::Result<(FileIdentity, u32, u32)> {
    use std::mem::{size_of, MaybeUninit};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileAttributeTagInfo, FileIdInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
        GetFileType, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_TAG_INFO, FILE_ID_INFO,
    };

    let handle = file.as_raw_handle().cast();
    let mut id = MaybeUninit::<FILE_ID_INFO>::zeroed();
    // SAFETY: `id` is the exact writable structure requested by
    // FileIdInfo, and `handle` remains owned by `file`.
    let id_ok = unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            id.as_mut_ptr().cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    };
    let identity = if id_ok != 0 {
        // SAFETY: the successful API call initialized the structure.
        let id = unsafe { id.assume_init() };
        if id.FileId.Identifier.iter().any(|byte| *byte != 0) {
            Some(FileIdentity::Extended {
                volume_serial: id.VolumeSerialNumber,
                file_id: id.FileId.Identifier,
            })
        } else {
            None
        }
    } else {
        None
    };
    let identity = match identity {
        Some(identity) => identity,
        None => {
            let mut legacy = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
            // SAFETY: `legacy` is the exact writable structure requested by
            // GetFileInformationByHandle, and `handle` remains valid.
            let legacy_ok = unsafe { GetFileInformationByHandle(handle, legacy.as_mut_ptr()) };
            if legacy_ok == 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: the successful API call initialized the structure.
            let legacy = unsafe { legacy.assume_init() };
            let file_index =
                (u64::from(legacy.nFileIndexHigh) << 32) | u64::from(legacy.nFileIndexLow);
            if file_index == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "the filesystem supplied no reliable guide identity",
                ));
            }
            FileIdentity::Legacy {
                volume_serial: legacy.dwVolumeSerialNumber,
                file_index,
            }
        }
    };

    let mut attributes = MaybeUninit::<FILE_ATTRIBUTE_TAG_INFO>::zeroed();
    // SAFETY: `attributes` is the exact writable structure requested by
    // FileAttributeTagInfo, and `handle` remains valid.
    let attributes_ok = unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileAttributeTagInfo,
            attributes.as_mut_ptr().cast(),
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    if attributes_ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful API call initialized the structure.
    let attributes = unsafe { attributes.assume_init() };

    // SAFETY: GetFileType only reads the valid open handle.
    let file_type = unsafe { GetFileType(handle) };
    Ok((identity, attributes.FileAttributes, file_type))
}

#[cfg(windows)]
fn path_identity(path: &Path, _metadata: &Metadata) -> io::Result<FileIdentity> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let file = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    validate_regular_handle(&file)?;
    handle_identity(&file)
}

#[cfg(windows)]
fn handle_identity(file: &File) -> io::Result<FileIdentity> {
    windows_handle_information(file).map(|(identity, _, _)| identity)
}

#[cfg(windows)]
fn validate_regular_handle(file: &File) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_TYPE_DISK,
    };

    let (_, attributes, file_type) = windows_handle_information(file)?;
    if file_type != FILE_TYPE_DISK
        || attributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0
        || !file.metadata()?.is_file()
    {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "the opened guide handle is not a regular non-reparse disk file",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn configure_nonfollowing_open(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    options
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
}

#[cfg(windows)]
fn platform_is_link_like(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(windows)]
fn is_no_follow_error(_error: &io::Error) -> bool {
    false
}

#[cfg(windows)]
fn validate_explicit_path_spelling(path: &Path) -> Result<(), GuideInputError> {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Prefix;

    let raw: Vec<u16> = path.as_os_str().encode_wide().collect();
    if raw.is_empty() {
        return Err(GuideInputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "the path is empty",
        });
    }
    if has_windows_namespace_prefix(&raw) {
        return Err(GuideInputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "device, named-pipe, and verbatim namespaces are not supported",
        });
    }
    match path.components().next() {
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_)) && !path.has_root() =>
        {
            return Err(GuideInputError::InvalidPath {
                path: path.to_path_buf(),
                reason: "drive-relative guide paths are not supported",
            });
        }
        Some(Component::RootDir) => {
            return Err(GuideInputError::InvalidPath {
                path: path.to_path_buf(),
                reason: "current-drive-root-relative guide paths are not supported",
            });
        }
        _ => {}
    }

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(_) => {}
                Prefix::UNC(server, share) => {
                    if windows_name_eq_ascii(share, "pipe")
                        || windows_name_eq_ascii(share, "mailslot")
                        || windows_name_eq_ascii(share, "IPC$")
                    {
                        return Err(GuideInputError::InvalidPath {
                            path: path.to_path_buf(),
                            reason: "named-pipe and mailslot namespaces are not filesystem shares",
                        });
                    }
                    validate_windows_component(path, server, true)?;
                    validate_windows_component(path, share, true)?;
                }
                Prefix::DeviceNS(_)
                | Prefix::Verbatim(_)
                | Prefix::VerbatimDisk(_)
                | Prefix::VerbatimUNC(_, _) => {
                    return Err(GuideInputError::InvalidPath {
                        path: path.to_path_buf(),
                        reason: "device and verbatim path prefixes are not supported",
                    });
                }
            },
            Component::Normal(name) => validate_windows_component(path, name, true)?,
            Component::CurDir | Component::ParentDir | Component::RootDir => {}
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn validate_explicit_path_spelling(path: &Path) -> Result<(), GuideInputError> {
    if path.as_os_str().is_empty() {
        Err(GuideInputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "the path is empty",
        })
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn has_windows_namespace_prefix(raw: &[u16]) -> bool {
    let separator = |unit| unit == u16::from(b'\\') || unit == u16::from(b'/');
    (raw.len() >= 4
        && separator(raw[0])
        && separator(raw[1])
        && matches!(raw[2], 46 | 63)
        && separator(raw[3]))
        || (raw.len() >= 4
            && separator(raw[0])
            && raw[1] == u16::from(b'?')
            && raw[2] == u16::from(b'?')
            && separator(raw[3]))
        || (raw.len() >= 5
            && separator(raw[0])
            && separator(raw[1])
            && raw[2] == u16::from(b'?')
            && raw[3] == u16::from(b'?')
            && separator(raw[4]))
}

#[cfg(windows)]
fn validate_windows_component(
    path: &Path,
    component: &std::ffi::OsStr,
    explicit: bool,
) -> Result<(), GuideInputError> {
    use std::os::windows::ffi::OsStrExt;

    let invalid = |reason| {
        if explicit {
            GuideInputError::InvalidPath {
                path: path.to_path_buf(),
                reason,
            }
        } else {
            GuideInputError::InvalidName {
                name: component.to_string_lossy().into_owned(),
                reason,
            }
        }
    };

    let units: Vec<u16> = component.encode_wide().collect();
    if units.contains(&u16::from(b':')) {
        return Err(invalid("alternate data stream syntax is not allowed"));
    }
    if units.iter().any(|unit| {
        *unit < 32
            || matches!(
                *unit,
                value
                    if value == u16::from(b'<')
                        || value == u16::from(b'>')
                        || value == u16::from(b'"')
                        || value == u16::from(b'|')
                        || value == u16::from(b'?')
                        || value == u16::from(b'*')
            )
    }) {
        return Err(invalid(
            "the name contains a character Windows does not allow in filesystem names",
        ));
    }
    if units
        .last()
        .is_some_and(|unit| *unit == u16::from(b' ') || *unit == u16::from(b'.'))
    {
        return Err(invalid(
            "Windows path components may not end in a space or dot",
        ));
    }

    let uppercase = component.to_string_lossy().to_uppercase();
    let base = uppercase
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    let reserved = matches!(
        base,
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || reserved_numbered_alias(base, "COM")
        || reserved_numbered_alias(base, "LPT");
    if reserved {
        return Err(invalid(
            "a reserved DOS device alias is not an ordinary filesystem name",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn windows_name_eq_ascii(name: &std::ffi::OsStr, expected: &str) -> bool {
    name.to_string_lossy().eq_ignore_ascii_case(expected)
}

#[cfg(windows)]
fn reserved_numbered_alias(name: &str, prefix: &str) -> bool {
    let Some(suffix) = name.strip_prefix(prefix) else {
        return false;
    };
    matches!(
        suffix,
        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
    )
}

#[cfg(not(any(unix, windows)))]
compile_error!("safe guide opening is implemented only for Unix and Windows");
