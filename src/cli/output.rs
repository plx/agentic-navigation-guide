//! Shared, exclusive filesystem output for `init` and `dump --output`.

use agentic_navigation_guide::errors::{AppError, Result as AppResult};
use std::env;
use std::fmt;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

/// A destination that has passed the output-specific authority and safety
/// checks. Generation happens after this plan is prepared but before the
/// destination is exclusively created.
#[derive(Debug)]
struct PreparedOutput {
    /// The exact caller-selected spelling. Only this path is shown in errors.
    requested_path: PathBuf,
    /// A path beneath a validated canonical parent. Never show this path in a
    /// diagnostic because it can disclose a resolved alias target.
    creation_path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExistingKind {
    RegularFile,
    Directory,
    SpecialEntry,
    UnknownEntry,
}

impl fmt::Display for ExistingKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RegularFile => "a regular file",
            Self::Directory => "a directory",
            Self::SpecialEntry => "a special filesystem entry",
            Self::UnknownEntry => "an existing filesystem entry",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeliveryStage {
    ValidateCreatedHandle,
    Write,
    Flush,
    SynchronizeData,
    ValidateCompletedHandle,
}

impl fmt::Display for DeliveryStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ValidateCreatedHandle => "created-handle validation",
            Self::Write => "write",
            Self::Flush => "userspace flush",
            Self::SynchronizeData => "data synchronization",
            Self::ValidateCompletedHandle => "completed-handle validation",
        })
    }
}

#[derive(Debug, Error)]
#[error("{stage}: {source}")]
struct DeliveryFailure {
    stage: DeliveryStage,
    #[source]
    source: io::Error,
}

#[derive(Debug, Error)]
enum CleanupFailure {
    #[error("the created entry's identity could not be established: {0}")]
    IdentityUnavailable(io::Error),

    #[error("the entry at the output name no longer has the identity created by this command")]
    IdentityChanged,

    #[error("safe removal failed: {0}")]
    Remove(io::Error),

    #[error("the output name still exists after removal")]
    StillPresent,

    #[error("the removal result could not be verified: {0}")]
    Verification(io::Error),

    #[cfg(test)]
    #[error("injected cleanup failure")]
    Injected,
}

/// Kept binary-private so #45 can return typed output failures without
/// expanding the approved public Rust API ledger before #54.
#[derive(Debug, Error)]
enum OutputError {
    #[error("invalid output path {path:?}: {reason}")]
    InvalidPath { path: PathBuf, reason: String },

    #[error(
        "output destination {path:?} already exists as {kind}; choose a new --output path or remove the existing entry"
    )]
    Existing { path: PathBuf, kind: ExistingKind },

    #[error("unsafe output destination {path:?}: {reason}")]
    Unsafe { path: PathBuf, reason: String },

    #[error("the parent directory for output {path:?} does not exist")]
    MissingParent { path: PathBuf },

    #[error("the parent of output {path:?} is not a directory")]
    ParentNotDirectory { path: PathBuf },

    #[error("the parent directory for output {path:?} is not writable")]
    ParentNotWritable { path: PathBuf },

    #[error("could not {operation} for output {path:?}: {source}")]
    Io {
        path: PathBuf,
        operation: &'static str,
        #[source]
        source: io::Error,
    },

    #[error("could not exclusively create output {path:?}: {source}")]
    Create {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(
        "delivery to output {path:?} failed during {failure}; the created artifact was removed"
    )]
    Delivery {
        path: PathBuf,
        #[source]
        failure: DeliveryFailure,
    },

    #[error(
        "delivery to output {path:?} failed during {failure}, and identity-safe cleanup failed ({cleanup}); a residual artifact may remain at {path:?}"
    )]
    CleanupFailed {
        path: PathBuf,
        failure: DeliveryFailure,
        cleanup: CleanupFailure,
    },
}

#[derive(Debug)]
enum GenerateOutputError<E> {
    Output(OutputError),
    Generation(E),
}

/// Prepare, generate, and deliver in the contractually required order.
///
/// The closure is intentionally invoked only after destination preflight, and
/// exclusive creation is intentionally delayed until the closure has returned
/// the complete byte buffer.
fn generate_to_file_typed<E, F>(
    generation_root: &Path,
    output_path: &Path,
    generate: F,
) -> std::result::Result<(), GenerateOutputError<E>>
where
    F: FnOnce() -> std::result::Result<Vec<u8>, E>,
{
    let destination = PreparedOutput::prepare(generation_root, output_path)
        .map_err(GenerateOutputError::Output)?;
    let bytes = generate().map_err(GenerateOutputError::Generation)?;
    destination
        .deliver(&bytes)
        .map_err(GenerateOutputError::Output)
}

pub(crate) fn generate_to_file<F>(
    generation_root: &Path,
    output_path: &Path,
    generate: F,
) -> AppResult<()>
where
    F: FnOnce() -> AppResult<Vec<u8>>,
{
    match generate_to_file_typed(generation_root, output_path, generate) {
        Ok(()) => Ok(()),
        Err(GenerateOutputError::Generation(error)) => Err(error),
        Err(GenerateOutputError::Output(error)) => Err(AppError::Other(error.to_string())),
    }
}

impl PreparedOutput {
    fn prepare(generation_root: &Path, requested_path: &Path) -> Result<Self, OutputError> {
        validate_output_spelling(requested_path)?;

        let current_dir = env::current_dir().map_err(|source| OutputError::Io {
            path: requested_path.to_path_buf(),
            operation: "determine the current directory",
            source,
        })?;
        let lexical_root = lexical_absolute(generation_root, &current_dir, requested_path)?;
        let lexical_output = lexical_absolute(requested_path, &current_dir, requested_path)?;

        let final_name = match lexical_output.components().next_back() {
            Some(Component::Normal(name)) => name.to_os_string(),
            _ => {
                return Err(OutputError::InvalidPath {
                    path: requested_path.to_path_buf(),
                    reason: "the path must end in an ordinary file name".to_string(),
                });
            }
        };

        let creation_parent =
            match lexical_output
                .strip_prefix(&lexical_root)
                .ok()
                .filter(|relative| {
                    let mut components = relative.components();
                    components.clone().next().is_some()
                        && components.all(|component| matches!(component, Component::Normal(_)))
                }) {
                Some(relative) => Self::prepare_in_root_parent(
                    &lexical_root,
                    relative,
                    requested_path,
                    &final_name,
                )?,
                None => Self::prepare_external_parent(&lexical_output, requested_path)?,
            };
        ensure_writable_parent(&creation_parent, requested_path)?;

        let creation_path = creation_parent.join(&final_name);
        match fs::symlink_metadata(&creation_path) {
            Ok(metadata) => return Err(existing_entry_error(requested_path, &metadata)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(OutputError::Io {
                    path: requested_path.to_path_buf(),
                    operation: "inspect the final destination without following it",
                    source,
                });
            }
        }

        Ok(Self {
            requested_path: requested_path.to_path_buf(),
            creation_path,
        })
    }

    fn prepare_in_root_parent(
        lexical_root: &Path,
        relative_output: &Path,
        requested_path: &Path,
        final_name: &std::ffi::OsStr,
    ) -> Result<PathBuf, OutputError> {
        let components: Vec<_> = relative_output.components().collect();
        if components.is_empty()
            || !components
                .iter()
                .all(|component| matches!(component, Component::Normal(_)))
            || !matches!(
                components.last(),
                Some(Component::Normal(name)) if *name == final_name
            )
        {
            return Err(OutputError::InvalidPath {
                path: requested_path.to_path_buf(),
                reason:
                    "a destination beneath the generation root may not escape with '..' components"
                        .to_string(),
            });
        }

        let canonical_root = fs::canonicalize(lexical_root).map_err(|source| OutputError::Io {
            path: requested_path.to_path_buf(),
            operation: "resolve the selected generation-root alias",
            source,
        })?;
        let root_metadata = fs::metadata(&canonical_root).map_err(|source| OutputError::Io {
            path: requested_path.to_path_buf(),
            operation: "inspect the selected generation root",
            source,
        })?;
        if !root_metadata.is_dir() {
            return Err(OutputError::ParentNotDirectory {
                path: requested_path.to_path_buf(),
            });
        }

        let mut cursor = canonical_root.clone();
        for component in &components[..components.len() - 1] {
            let Component::Normal(name) = component else {
                unreachable!("relative components were validated above");
            };
            cursor.push(name);
            let metadata = match fs::symlink_metadata(&cursor) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Err(OutputError::MissingParent {
                        path: requested_path.to_path_buf(),
                    });
                }
                Err(source) => {
                    return Err(OutputError::Io {
                        path: requested_path.to_path_buf(),
                        operation: "inspect an output ancestor without following it",
                        source,
                    });
                }
            };
            if is_link_like(&metadata) {
                return Err(OutputError::Unsafe {
                    path: requested_path.to_path_buf(),
                    reason:
                        "an ancestor below the selected generation root is a link or reparse point"
                            .to_string(),
                });
            }
            if !metadata.is_dir() {
                return Err(OutputError::ParentNotDirectory {
                    path: requested_path.to_path_buf(),
                });
            }
        }

        let canonical_parent = fs::canonicalize(&cursor).map_err(|source| OutputError::Io {
            path: requested_path.to_path_buf(),
            operation: "resolve the validated output parent",
            source,
        })?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(OutputError::Unsafe {
                path: requested_path.to_path_buf(),
                reason: "the canonical parent leaves the selected generation root".to_string(),
            });
        }

        Ok(canonical_parent)
    }

    fn prepare_external_parent(
        lexical_output: &Path,
        requested_path: &Path,
    ) -> Result<PathBuf, OutputError> {
        let Some(parent) = lexical_output.parent() else {
            return Err(OutputError::InvalidPath {
                path: requested_path.to_path_buf(),
                reason: "the path has no parent directory".to_string(),
            });
        };
        match fs::canonicalize(parent) {
            Ok(parent) => Ok(parent),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Err(OutputError::MissingParent {
                    path: requested_path.to_path_buf(),
                })
            }
            Err(source) => Err(OutputError::Io {
                path: requested_path.to_path_buf(),
                operation: "resolve the explicitly selected external output parent",
                source,
            }),
        }
    }

    fn deliver(&self, bytes: &[u8]) -> Result<(), OutputError> {
        let mut control = ProductionControl;
        self.deliver_controlled(bytes, &mut control)
    }

    fn deliver_controlled<C: DeliveryControl>(
        &self,
        bytes: &[u8],
        control: &mut C,
    ) -> Result<(), OutputError> {
        let mut file = self.open_exclusive()?;
        let created_identity = match handle_identity(&file) {
            Ok(identity) => identity,
            Err(source) => {
                let failure = delivery_failure(DeliveryStage::ValidateCreatedHandle, source);
                drop(file);
                return Err(OutputError::CleanupFailed {
                    path: self.requested_path.clone(),
                    failure,
                    cleanup: CleanupFailure::IdentityUnavailable(io::Error::new(
                        io::ErrorKind::Other,
                        "the created handle had no stable filesystem identity",
                    )),
                });
            }
        };

        let delivery_result = (|| {
            control
                .checkpoint(DeliveryStage::ValidateCreatedHandle, &mut file)
                .map_err(|source| delivery_failure(DeliveryStage::ValidateCreatedHandle, source))?;
            validate_regular_handle(&file)
                .map_err(|source| delivery_failure(DeliveryStage::ValidateCreatedHandle, source))?;

            control
                .checkpoint(DeliveryStage::Write, &mut file)
                .map_err(|source| delivery_failure(DeliveryStage::Write, source))?;
            file.write_all(bytes)
                .map_err(|source| delivery_failure(DeliveryStage::Write, source))?;

            control
                .checkpoint(DeliveryStage::Flush, &mut file)
                .map_err(|source| delivery_failure(DeliveryStage::Flush, source))?;
            file.flush()
                .map_err(|source| delivery_failure(DeliveryStage::Flush, source))?;

            control
                .checkpoint(DeliveryStage::SynchronizeData, &mut file)
                .map_err(|source| delivery_failure(DeliveryStage::SynchronizeData, source))?;
            file.sync_data()
                .map_err(|source| delivery_failure(DeliveryStage::SynchronizeData, source))?;

            control
                .checkpoint(DeliveryStage::ValidateCompletedHandle, &mut file)
                .map_err(|source| {
                    delivery_failure(DeliveryStage::ValidateCompletedHandle, source)
                })?;
            validate_regular_handle(&file).map_err(|source| {
                delivery_failure(DeliveryStage::ValidateCompletedHandle, source)
            })?;
            let completed_identity = handle_identity(&file).map_err(|source| {
                delivery_failure(DeliveryStage::ValidateCompletedHandle, source)
            })?;
            if completed_identity != created_identity {
                return Err(delivery_failure(
                    DeliveryStage::ValidateCompletedHandle,
                    io::Error::new(
                        io::ErrorKind::Other,
                        "the created handle's filesystem identity changed",
                    ),
                ));
            }
            let expected_length = u64::try_from(bytes.len()).map_err(|_| {
                delivery_failure(
                    DeliveryStage::ValidateCompletedHandle,
                    io::Error::new(
                        io::ErrorKind::Other,
                        "the output buffer is too large to validate",
                    ),
                )
            })?;
            let actual_length = file
                .metadata()
                .map_err(|source| delivery_failure(DeliveryStage::ValidateCompletedHandle, source))?
                .len();
            if actual_length != expected_length {
                return Err(delivery_failure(
                    DeliveryStage::ValidateCompletedHandle,
                    io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "the completed file length was {actual_length}, expected {expected_length}"
                        ),
                    ),
                ));
            }

            Ok(())
        })();

        drop(file);
        match delivery_result {
            Ok(()) => Ok(()),
            Err(failure) => self.finish_failed_delivery(failure, &created_identity, control),
        }
    }

    fn open_exclusive(&self) -> Result<File, OutputError> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        configure_exclusive_open(&mut options);

        match options.open(&self.creation_path) {
            Ok(file) => Ok(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(
                classify_racing_entry(&self.creation_path, &self.requested_path),
            ),
            Err(source) if is_no_follow_error(&source) => Err(OutputError::Unsafe {
                path: self.requested_path.clone(),
                reason: "the final destination became a link or reparse point".to_string(),
            }),
            Err(source) => Err(OutputError::Create {
                path: self.requested_path.clone(),
                source,
            }),
        }
    }

    fn finish_failed_delivery<C: DeliveryControl>(
        &self,
        failure: DeliveryFailure,
        created_identity: &FileIdentity,
        control: &mut C,
    ) -> Result<(), OutputError> {
        let cleanup = cleanup_created_entry(&self.creation_path, created_identity, control);
        match cleanup {
            Ok(()) => Err(OutputError::Delivery {
                path: self.requested_path.clone(),
                failure,
            }),
            Err(cleanup) => Err(OutputError::CleanupFailed {
                path: self.requested_path.clone(),
                failure,
                cleanup,
            }),
        }
    }
}

fn lexical_absolute(
    path: &Path,
    current_dir: &Path,
    requested_path: &Path,
) -> Result<PathBuf, OutputError> {
    #[cfg(not(windows))]
    let _ = requested_path;

    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    #[cfg(windows)]
    {
        use std::path::Prefix;

        match path.components().next() {
            Some(Component::Prefix(component)) => match component.kind() {
                Prefix::Disk(_) => {
                    return Err(OutputError::InvalidPath {
                        path: requested_path.to_path_buf(),
                        reason: "drive-relative output paths are not supported".to_string(),
                    });
                }
                _ => {
                    return Err(OutputError::InvalidPath {
                        path: requested_path.to_path_buf(),
                        reason: "unsupported Windows path prefix".to_string(),
                    });
                }
            },
            Some(Component::RootDir) => {
                return Err(OutputError::InvalidPath {
                    path: requested_path.to_path_buf(),
                    reason: "current-drive-root-relative paths are not supported".to_string(),
                });
            }
            _ => {}
        }
    }

    Ok(current_dir.join(path))
}

fn ensure_writable_parent(parent: &Path, requested_path: &Path) -> Result<(), OutputError> {
    let metadata = fs::metadata(parent).map_err(|source| OutputError::Io {
        path: requested_path.to_path_buf(),
        operation: "inspect the output parent directory",
        source,
    })?;
    if !metadata.is_dir() {
        return Err(OutputError::ParentNotDirectory {
            path: requested_path.to_path_buf(),
        });
    }

    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::PermissionsExt;

        let mode = metadata.permissions().mode();
        if mode & 0o222 == 0 || mode & 0o111 == 0 {
            return Err(OutputError::ParentNotWritable {
                path: requested_path.to_path_buf(),
            });
        }

        let encoded =
            CString::new(parent.as_os_str().as_bytes()).map_err(|_| OutputError::InvalidPath {
                path: requested_path.to_path_buf(),
                reason: "the parent path contains an interior NUL byte".to_string(),
            })?;
        // SAFETY: `encoded` is a NUL-terminated representation of the
        // validated parent path and remains alive for the duration of access.
        let accessible = unsafe { libc::access(encoded.as_ptr(), libc::W_OK | libc::X_OK) };
        if accessible != 0 {
            let source = io::Error::last_os_error();
            return match source.kind() {
                io::ErrorKind::PermissionDenied => Err(OutputError::ParentNotWritable {
                    path: requested_path.to_path_buf(),
                }),
                _ => Err(OutputError::Io {
                    path: requested_path.to_path_buf(),
                    operation: "verify write and search access to the output parent",
                    source,
                }),
            };
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ADD_FILE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };

        let access = OpenOptions::new()
            .access_mode(FILE_ADD_FILE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(parent);
        if let Err(source) = access {
            return match source.kind() {
                io::ErrorKind::PermissionDenied => Err(OutputError::ParentNotWritable {
                    path: requested_path.to_path_buf(),
                }),
                _ => Err(OutputError::Io {
                    path: requested_path.to_path_buf(),
                    operation: "verify create access to the output parent",
                    source,
                }),
            };
        }
    }

    Ok(())
}

fn existing_entry_error(requested_path: &Path, metadata: &Metadata) -> OutputError {
    if is_link_like(metadata) {
        return OutputError::Unsafe {
            path: requested_path.to_path_buf(),
            reason: "the final destination is a link or reparse point".to_string(),
        };
    }
    OutputError::Existing {
        path: requested_path.to_path_buf(),
        kind: if metadata.is_file() {
            ExistingKind::RegularFile
        } else if metadata.is_dir() {
            ExistingKind::Directory
        } else {
            ExistingKind::SpecialEntry
        },
    }
}

fn classify_racing_entry(creation_path: &Path, requested_path: &Path) -> OutputError {
    match fs::symlink_metadata(creation_path) {
        Ok(metadata) => existing_entry_error(requested_path, &metadata),
        Err(_) => OutputError::Existing {
            path: requested_path.to_path_buf(),
            kind: ExistingKind::UnknownEntry,
        },
    }
}

fn delivery_failure(stage: DeliveryStage, source: io::Error) -> DeliveryFailure {
    DeliveryFailure { stage, source }
}

trait DeliveryControl {
    fn checkpoint(&mut self, _stage: DeliveryStage, _file: &mut File) -> io::Result<()> {
        Ok(())
    }

    fn before_cleanup(&mut self, _path: &Path) -> Result<(), CleanupFailure> {
        Ok(())
    }
}

struct ProductionControl;

impl DeliveryControl for ProductionControl {}

fn cleanup_created_entry<C: DeliveryControl>(
    path: &Path,
    created_identity: &FileIdentity,
    control: &mut C,
) -> Result<(), CleanupFailure> {
    control.before_cleanup(path)?;

    let current_identity = match path_identity(path) {
        Ok(Some(identity)) => identity,
        Ok(None) => return Ok(()),
        Err(error) => return Err(CleanupFailure::IdentityUnavailable(error)),
    };
    if &current_identity != created_identity {
        return Err(CleanupFailure::IdentityChanged);
    }

    fs::remove_file(path).map_err(CleanupFailure::Remove)?;
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(CleanupFailure::StillPresent),
        Err(error) => Err(CleanupFailure::Verification(error)),
    }
}

#[cfg(unix)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
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
fn path_identity(path: &Path) -> io::Result<Option<FileIdentity>> {
    use std::os::unix::fs::MetadataExt;

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "the current output entry is not a regular file",
        ));
    }
    Ok(Some(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }))
}

#[cfg(unix)]
fn validate_regular_handle(file: &File) -> io::Result<()> {
    if file.metadata()?.file_type().is_file() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "the created handle is not a regular file",
        ))
    }
}

#[cfg(unix)]
fn configure_exclusive_open(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(unix)]
fn is_link_like(metadata: &Metadata) -> bool {
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
    // SAFETY: `id` points to an adequately sized writable structure for the
    // requested information class and `handle` remains owned by `file`.
    let id_ok = unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            id.as_mut_ptr().cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    };
    let identity = if id_ok != 0 {
        // SAFETY: the successful API call initialized the full structure.
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
            // SAFETY: `legacy` is the exact writable structure required by
            // GetFileInformationByHandle and `handle` remains valid.
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
                    "the filesystem supplied no reliable file identity",
                ));
            }
            FileIdentity::Legacy {
                volume_serial: legacy.dwVolumeSerialNumber,
                file_index,
            }
        }
    };

    let mut attributes = MaybeUninit::<FILE_ATTRIBUTE_TAG_INFO>::zeroed();
    // SAFETY: `attributes` is the buffer required by FileAttributeTagInfo.
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
    // SAFETY: the successful API call initialized the full structure.
    let attributes = unsafe { attributes.assume_init() };

    // SAFETY: GetFileType only reads the valid open handle.
    let file_type = unsafe { GetFileType(handle) };
    Ok((identity, attributes.FileAttributes, file_type))
}

#[cfg(windows)]
fn handle_identity(file: &File) -> io::Result<FileIdentity> {
    windows_handle_information(file).map(|(identity, _, _)| identity)
}

#[cfg(windows)]
fn path_identity(path: &Path) -> io::Result<Option<FileIdentity>> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if is_link_like(&metadata) || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "the current output entry is not a regular non-reparse file",
        ));
    }

    let file = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    validate_regular_handle(&file)?;
    handle_identity(&file).map(Some)
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
            "the created handle is not a regular non-reparse disk file",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn configure_exclusive_open(_options: &mut OpenOptions) {
    // Rust's `create_new(true)` maps to CREATE_NEW and includes
    // FILE_FLAG_OPEN_REPARSE_POINT. The final leaf is therefore neither
    // followed nor overwritten.
}

#[cfg(windows)]
fn is_link_like(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(windows)]
fn is_no_follow_error(_error: &io::Error) -> bool {
    false
}

#[cfg(not(any(unix, windows)))]
compile_error!("exclusive output identity validation is implemented only for Unix and Windows");

#[cfg(unix)]
fn validate_output_spelling(path: &Path) -> Result<(), OutputError> {
    use std::os::unix::ffi::OsStrExt;

    if path.as_os_str().is_empty() {
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "the path is empty".to_string(),
        });
    }
    if path.as_os_str().as_bytes().ends_with(b"/") {
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "the path ends with a directory separator".to_string(),
        });
    }
    if path
        .as_os_str()
        .as_bytes()
        .rsplit(|unit| *unit == b'/')
        .next()
        .is_some_and(|component| component == b"." || component == b"..")
    {
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "the path must end in an ordinary file name".to_string(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn validate_output_spelling(path: &Path) -> Result<(), OutputError> {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Prefix;

    let raw: Vec<u16> = path.as_os_str().encode_wide().collect();
    if raw.is_empty() {
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "the path is empty".to_string(),
        });
    }
    if raw
        .last()
        .is_some_and(|unit| *unit == u16::from(b'\\') || *unit == u16::from(b'/'))
    {
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "the path ends with a directory separator".to_string(),
        });
    }
    let final_component = raw
        .rsplit(|unit| *unit == u16::from(b'\\') || *unit == u16::from(b'/'))
        .next()
        .unwrap_or_default();
    if final_component == [u16::from(b'.')] || final_component == [u16::from(b'.'), u16::from(b'.')]
    {
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "the path must end in an ordinary file name".to_string(),
        });
    }
    if has_windows_namespace_prefix(&raw) {
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "device, named-pipe, and verbatim namespaces are not supported".to_string(),
        });
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
                        return Err(OutputError::InvalidPath {
                            path: path.to_path_buf(),
                            reason: "named-pipe and mailslot namespaces are not filesystem shares"
                                .to_string(),
                        });
                    }
                    validate_windows_component(path, server)?;
                    validate_windows_component(path, share)?;
                }
                Prefix::DeviceNS(_)
                | Prefix::Verbatim(_)
                | Prefix::VerbatimDisk(_)
                | Prefix::VerbatimUNC(_, _) => {
                    return Err(OutputError::InvalidPath {
                        path: path.to_path_buf(),
                        reason: "device and verbatim path prefixes are not supported".to_string(),
                    });
                }
            },
            Component::Normal(name) => validate_windows_component(path, name)?,
            Component::CurDir | Component::ParentDir | Component::RootDir => {}
        }
    }
    Ok(())
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
fn validate_windows_component(path: &Path, component: &std::ffi::OsStr) -> Result<(), OutputError> {
    use std::os::windows::ffi::OsStrExt;

    let units: Vec<u16> = component.encode_wide().collect();
    if units.contains(&u16::from(b':')) {
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "alternate data stream syntax is not allowed".to_string(),
        });
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
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "the path contains a character Windows does not allow in filesystem names"
                .to_string(),
        });
    }
    if units
        .last()
        .is_some_and(|unit| *unit == u16::from(b' ') || *unit == u16::from(b'.'))
    {
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "Windows path components may not end in a space or dot".to_string(),
        });
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
        return Err(OutputError::InvalidPath {
            path: path.to_path_buf(),
            reason: "a reserved DOS device alias is not an ordinary filesystem name".to_string(),
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use tempfile::TempDir;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum EvidenceOutcome {
        Conformant,
        Unavailable(&'static str),
    }

    struct EvidenceGroup {
        ids: &'static [&'static str],
        observe: fn() -> EvidenceOutcome,
    }

    const OUTPUT_TRUST_EVIDENCE: &[EvidenceGroup] = &[
        EvidenceGroup {
            ids: &[
                "trust-output-init-new-in-root",
                "trust-output-dump-new-in-root",
                "trust-output-new-external",
            ],
            observe: observe_new_basic,
        },
        EvidenceGroup {
            ids: &[
                "trust-output-explicit-external-link-ancestor",
                "trust-output-beneath-root-alias",
            ],
            observe: observe_linked_authorities,
        },
        EvidenceGroup {
            ids: &["trust-output-created-handle-regular"],
            observe: observe_created_handle,
        },
        EvidenceGroup {
            ids: &[
                "trust-output-existing-regular",
                "trust-output-existing-hard-link",
                "trust-output-existing-directory",
            ],
            observe: observe_existing_basic,
        },
        EvidenceGroup {
            ids: &[
                "trust-output-link-existing-target",
                "trust-output-dangling-link-external",
                "trust-output-link-chain-or-loop",
            ],
            observe: observe_existing_links,
        },
        EvidenceGroup {
            ids: &["trust-output-existing-special-entry"],
            observe: observe_existing_special,
        },
        EvidenceGroup {
            ids: &["trust-output-windows-reparse-entry"],
            observe: observe_windows_reparse,
        },
        EvidenceGroup {
            ids: &[
                "trust-output-windows-alternate-data-stream",
                "trust-output-windows-device-namespace",
            ],
            observe: observe_windows_spelling,
        },
        EvidenceGroup {
            ids: &["trust-output-link-ancestor-below-root"],
            observe: observe_in_root_link_ancestor,
        },
        EvidenceGroup {
            ids: &["trust-output-missing-parent"],
            observe: observe_missing_parent,
        },
        EvidenceGroup {
            ids: &["trust-output-read-only-parent"],
            observe: observe_read_only_parent,
        },
        EvidenceGroup {
            ids: &["trust-output-creator-race"],
            observe: observe_creator_race,
        },
        EvidenceGroup {
            ids: &["trust-output-write-or-flush-failure"],
            observe: observe_delivery_failures,
        },
        EvidenceGroup {
            ids: &["trust-output-cleanup-failure"],
            observe: observe_cleanup_failure,
        },
        EvidenceGroup {
            ids: &["trust-output-preexisting-inside-root"],
            observe: observe_preexisting_output,
        },
        EvidenceGroup {
            ids: &["trust-output-generation-failure"],
            observe: observe_generation_failure,
        },
        EvidenceGroup {
            ids: &["trust-output-in-progress-visibility"],
            observe: observe_visibility_documentation,
        },
    ];

    #[derive(Clone, Copy)]
    enum CleanupAction {
        Normal,
        Fail,
        Replace,
    }

    struct FaultControl {
        stage: DeliveryStage,
        cleanup: CleanupAction,
        fired: bool,
    }

    impl FaultControl {
        fn at(stage: DeliveryStage) -> Self {
            Self {
                stage,
                cleanup: CleanupAction::Normal,
                fired: false,
            }
        }
    }

    impl DeliveryControl for FaultControl {
        fn checkpoint(&mut self, stage: DeliveryStage, file: &mut File) -> io::Result<()> {
            if self.fired || stage != self.stage {
                return Ok(());
            }
            self.fired = true;
            if stage == DeliveryStage::Write {
                file.write_all(b"partial")?;
            }
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("injected {stage} failure"),
            ))
        }

        fn before_cleanup(&mut self, path: &Path) -> Result<(), CleanupFailure> {
            match self.cleanup {
                CleanupAction::Normal => Ok(()),
                CleanupAction::Fail => Err(CleanupFailure::Injected),
                CleanupAction::Replace => {
                    fs::remove_file(path).map_err(CleanupFailure::Remove)?;
                    fs::write(path, b"replacement").map_err(CleanupFailure::Verification)?;
                    Ok(())
                }
            }
        }
    }

    #[test]
    fn output_trust_evidence_is_an_exact_set_for_issue_45() {
        let expected = issue_45_trust_ids(include_str!("../../tests/fixtures/v0_2_trust.rs"));
        let mut declared = BTreeSet::new();
        let mut conformant = BTreeSet::new();
        let mut unavailable = BTreeSet::new();
        for group in OUTPUT_TRUST_EVIDENCE {
            let outcome = (group.observe)();
            for id in group.ids {
                assert!(
                    declared.insert(*id),
                    "duplicate output trust evidence ID '{id}'"
                );
                match outcome {
                    EvidenceOutcome::Conformant => {
                        conformant.insert(*id);
                    }
                    EvidenceOutcome::Unavailable(reason) => {
                        assert!(!reason.is_empty());
                        unavailable.insert(*id);
                    }
                }
            }
        }

        assert_eq!(declared, expected, "output evidence declaration drifted");
        assert!(
            conformant.is_disjoint(&unavailable),
            "an output row cannot be both conformant and unavailable"
        );
        assert_eq!(
            conformant
                .union(&unavailable)
                .copied()
                .collect::<BTreeSet<_>>(),
            declared,
            "every declared output row must have an explicit observation"
        );

        #[cfg(not(windows))]
        let expected_unavailable = BTreeSet::from([
            "trust-output-windows-reparse-entry",
            "trust-output-windows-alternate-data-stream",
            "trust-output-windows-device-namespace",
        ]);
        #[cfg(windows)]
        let expected_unavailable = BTreeSet::new();

        assert_eq!(
            unavailable, expected_unavailable,
            "unexpected unavailable output evidence on this platform"
        );
        assert_eq!(
            conformant,
            declared
                .difference(&expected_unavailable)
                .copied()
                .collect(),
            "host-applicable output evidence is not fully conformant"
        );
    }

    #[test]
    fn preexisting_destination_prevents_generation() {
        let root = TempDir::new().unwrap();
        let output = root.path().join("already-there.md");
        fs::write(&output, b"sentinel").unwrap();
        let generated = std::cell::Cell::new(false);

        let result = generate_to_file_typed(root.path(), &output, || {
            generated.set(true);
            Ok::<_, &'static str>(b"replacement".to_vec())
        });

        assert!(matches!(
            result,
            Err(GenerateOutputError::Output(OutputError::Existing { .. }))
        ));
        assert!(!generated.get());
        assert_eq!(fs::read(output).unwrap(), b"sentinel");
    }

    #[test]
    fn generation_failure_precedes_creation() {
        let root = TempDir::new().unwrap();
        let output = root.path().join("must-stay-absent.md");

        let result = generate_to_file_typed(root.path(), &output, || {
            Err::<Vec<u8>, _>("injected generation failure")
        });

        assert!(matches!(
            result,
            Err(GenerateOutputError::Generation(
                "injected generation failure"
            ))
        ));
        assert!(!output.exists());
    }

    #[test]
    fn delivery_stage_failures_are_cleaned_up() {
        for stage in [
            DeliveryStage::ValidateCreatedHandle,
            DeliveryStage::Write,
            DeliveryStage::Flush,
            DeliveryStage::SynchronizeData,
            DeliveryStage::ValidateCompletedHandle,
        ] {
            let root = TempDir::new().unwrap();
            let output = root.path().join("failed-output.md");
            let destination = PreparedOutput::prepare(root.path(), &output).unwrap();
            let mut control = FaultControl::at(stage);

            let result = destination.deliver_controlled(b"complete output", &mut control);

            assert!(
                matches!(
                    result,
                    Err(OutputError::Delivery {
                        failure: DeliveryFailure {
                            stage: failed_stage,
                            ..
                        },
                        ..
                    }) if failed_stage == stage
                ),
                "unexpected result for {stage}: {result:?}"
            );
            assert!(
                !output.exists(),
                "delivery failure at {stage} left a partial output"
            );
        }
    }

    #[test]
    fn cleanup_failure_reports_residual_and_preserves_replacement() {
        let root = TempDir::new().unwrap();

        let output = root.path().join("cleanup-failure.md");
        let destination = PreparedOutput::prepare(root.path(), &output).unwrap();
        let mut removal_failure = FaultControl {
            stage: DeliveryStage::Write,
            cleanup: CleanupAction::Fail,
            fired: false,
        };
        let result = destination.deliver_controlled(b"complete output", &mut removal_failure);
        assert!(matches!(
            &result,
            Err(OutputError::CleanupFailed {
                cleanup: CleanupFailure::Injected,
                ..
            })
        ));
        assert_eq!(fs::read(&output).unwrap(), b"partial");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("residual artifact"));

        let replacement_output = root.path().join("identity-replacement.md");
        let destination = PreparedOutput::prepare(root.path(), &replacement_output).unwrap();
        let mut identity_replacement = FaultControl {
            stage: DeliveryStage::Write,
            cleanup: CleanupAction::Replace,
            fired: false,
        };
        let result = destination.deliver_controlled(b"complete output", &mut identity_replacement);
        assert!(matches!(
            result,
            Err(OutputError::CleanupFailed {
                cleanup: CleanupFailure::IdentityChanged,
                ..
            })
        ));
        assert_eq!(fs::read(replacement_output).unwrap(), b"replacement");
    }

    #[test]
    fn exclusive_create_has_exactly_one_winner_for_100_races() {
        use std::sync::{Arc, Barrier};

        for iteration in 0..100 {
            let root = TempDir::new().unwrap();
            let output = root.path().join(format!("race-{iteration}.md"));
            let first = PreparedOutput::prepare(root.path(), &output).unwrap();
            let second = PreparedOutput::prepare(root.path(), &output).unwrap();
            let barrier = Arc::new(Barrier::new(3));

            let first_barrier = Arc::clone(&barrier);
            let first_thread = std::thread::spawn(move || {
                first_barrier.wait();
                first.deliver(b"first contender")
            });
            let second_barrier = Arc::clone(&barrier);
            let second_thread = std::thread::spawn(move || {
                second_barrier.wait();
                second.deliver(b"second contender")
            });
            barrier.wait();

            let first_result = first_thread.join().unwrap();
            let second_result = second_thread.join().unwrap();
            assert_ne!(
                first_result.is_ok(),
                second_result.is_ok(),
                "race {iteration} did not produce exactly one winner: first={first_result:?}, second={second_result:?}"
            );
            let expected = if first_result.is_ok() {
                b"first contender".as_slice()
            } else {
                b"second contender".as_slice()
            };
            let loser = if first_result.is_ok() {
                &second_result
            } else {
                &first_result
            };
            assert!(matches!(
                loser,
                Err(OutputError::Existing { .. } | OutputError::Unsafe { .. })
            ));
            assert_eq!(fs::read(output).unwrap(), expected);
        }
    }

    #[cfg(unix)]
    #[test]
    fn created_handle_validation_rejects_special_handle() {
        let null = File::open("/dev/null").unwrap();
        assert!(validate_regular_handle(&null).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn created_handle_validation_rejects_device_handle() {
        let null = File::open("NUL").unwrap();
        assert!(validate_regular_handle(&null).is_err());
    }

    #[test]
    fn readme_documents_non_atomic_content_publication() {
        let readme = include_str!("../../README.md");
        let normalized = readme.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(normalized.contains("in-progress"));
        assert!(normalized.contains("crash durability"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_output_spelling_rejects_non_filesystem_names() {
        for path in [
            r"C:\safe\output.md:stream",
            r"C:\safe\NUL.txt",
            r"C:\safe\CONIN$.txt",
            r"C:\safe\CONOUT$",
            r"C:\safe\COM¹.log",
            r"C:\safe\LPT9",
            r"C:\safe\bad?.md",
            r"C:\safe\bad|name.md",
            r"\\.\pipe\guide",
            r"\\localhost\pipe\guide",
            r"\\localhost\mailslot\guide",
            r"\\localhost\IPC$\guide",
            r"//?/GLOBALROOT/Device/HarddiskVolume1/guide",
            r"\\?/GLOBALROOT/Device/HarddiskVolume1/guide",
            r"\\?\C:\safe\output.md",
            r"\??\C:\safe\output.md",
        ] {
            assert!(
                matches!(
                    validate_output_spelling(Path::new(path)),
                    Err(OutputError::InvalidPath { .. })
                ),
                "unsafe Windows spelling was accepted: {path:?}"
            );
        }
        validate_output_spelling(Path::new(r"C:\safe\ordinary-output.md")).unwrap();
        validate_output_spelling(Path::new(r"\\server\share\ordinary-output.md")).unwrap();
    }

    fn observe_new_basic() -> EvidenceOutcome {
        use crate::cli::{dump::DumpArgs, init::InitArgs};
        use agentic_navigation_guide::types::Config;

        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input"), b"input").unwrap();
        let config = Config::default();

        let init_in_root = root.path().join("init-in-root");
        InitArgs {
            output: init_in_root.clone(),
            depth: None,
            exclude: Vec::new(),
            indent: 2,
            root: Some(root.path().to_path_buf()),
            include_vcs_directories: false,
        }
        .execute(&config)
        .unwrap();
        assert!(fs::read_to_string(init_in_root)
            .unwrap()
            .contains("<agentic-navigation-guide>"));

        let dump_in_root = root.path().join("dump-in-root");
        DumpArgs {
            output: Some(dump_in_root.clone()),
            depth: None,
            exclude: Vec::new(),
            indent: 2,
            omit_xml_wrapper: false,
            root: Some(root.path().to_path_buf()),
        }
        .execute(&config)
        .unwrap();
        assert!(fs::read_to_string(dump_in_root)
            .unwrap()
            .contains("<agentic-navigation-guide>"));

        let external = TempDir::new().unwrap();
        let init_external = external.path().join("init-external");
        InitArgs {
            output: init_external.clone(),
            depth: None,
            exclude: Vec::new(),
            indent: 2,
            root: Some(root.path().to_path_buf()),
            include_vcs_directories: false,
        }
        .execute(&config)
        .unwrap();
        assert!(init_external.is_file());

        let dump_external = external.path().join("dump-external");
        DumpArgs {
            output: Some(dump_external.clone()),
            depth: None,
            exclude: Vec::new(),
            indent: 2,
            omit_xml_wrapper: false,
            root: Some(root.path().to_path_buf()),
        }
        .execute(&config)
        .unwrap();
        assert!(dump_external.is_file());
        EvidenceOutcome::Conformant
    }

    fn observe_linked_authorities() -> EvidenceOutcome {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input"), b"input").unwrap();
        let external = TempDir::new().unwrap();

        let alias_parent = TempDir::new().unwrap();
        let root_alias = alias_parent.path().join("root-alias");
        let link_parent = TempDir::new().unwrap();
        let external_link = link_parent.path().join("external-link");

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            symlink(root.path(), &root_alias).unwrap();
            symlink(external.path(), &external_link).unwrap();
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_dir;

            if symlink_dir(root.path(), &root_alias).is_err()
                || symlink_dir(external.path(), &external_link).is_err()
            {
                return EvidenceOutcome::Unavailable(
                    "Windows directory-link privilege is unavailable",
                );
            }
        }

        let alias_output = root_alias.join("alias-output");
        PreparedOutput::prepare(&root_alias, &alias_output)
            .unwrap()
            .deliver(b"alias")
            .unwrap();
        assert_eq!(
            fs::read(root.path().join("alias-output")).unwrap(),
            b"alias"
        );

        let linked_output = external_link.join("linked-output");
        PreparedOutput::prepare(root.path(), &linked_output)
            .unwrap()
            .deliver(b"linked")
            .unwrap();
        assert_eq!(
            fs::read(external.path().join("linked-output")).unwrap(),
            b"linked"
        );
        EvidenceOutcome::Conformant
    }

    fn observe_created_handle() -> EvidenceOutcome {
        let root = TempDir::new().unwrap();
        let output = root.path().join("regular-output");
        PreparedOutput::prepare(root.path(), &output)
            .unwrap()
            .deliver(b"regular")
            .unwrap();
        assert!(fs::metadata(output).unwrap().is_file());

        #[cfg(unix)]
        created_handle_validation_rejects_special_handle();
        #[cfg(windows)]
        created_handle_validation_rejects_device_handle();

        EvidenceOutcome::Conformant
    }

    fn observe_existing_basic() -> EvidenceOutcome {
        let root = TempDir::new().unwrap();
        let entries = TempDir::new().unwrap();

        let regular = entries.path().join("regular");
        fs::write(&regular, b"sentinel").unwrap();
        assert!(PreparedOutput::prepare(root.path(), &regular).is_err());
        assert_eq!(fs::read(&regular).unwrap(), b"sentinel");

        let hard_link = entries.path().join("hard-link");
        fs::hard_link(&regular, &hard_link).unwrap();
        assert!(PreparedOutput::prepare(root.path(), &hard_link).is_err());
        assert_eq!(fs::read(&hard_link).unwrap(), b"sentinel");

        let directory = entries.path().join("directory");
        fs::create_dir(&directory).unwrap();
        assert!(PreparedOutput::prepare(root.path(), &directory).is_err());
        EvidenceOutcome::Conformant
    }

    fn observe_existing_links() -> EvidenceOutcome {
        let root = TempDir::new().unwrap();
        let entries = TempDir::new().unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let target = entries.path().join("target");
            fs::write(&target, b"target").unwrap();
            let link = entries.path().join("link");
            symlink(&target, &link).unwrap();
            assert!(PreparedOutput::prepare(root.path(), &link).is_err());
            assert_eq!(fs::read(&target).unwrap(), b"target");

            let dangling_target = entries.path().join("missing-target");
            let dangling = entries.path().join("dangling");
            symlink(&dangling_target, &dangling).unwrap();
            assert!(PreparedOutput::prepare(root.path(), &dangling).is_err());
            assert!(!dangling_target.exists());

            let second = entries.path().join("chain-second");
            let chain = entries.path().join("chain-first");
            symlink(&target, &second).unwrap();
            symlink(&second, &chain).unwrap();
            assert!(PreparedOutput::prepare(root.path(), &chain).is_err());

            let loop_a = entries.path().join("loop-a");
            let loop_b = entries.path().join("loop-b");
            symlink(&loop_b, &loop_a).unwrap();
            symlink(&loop_a, &loop_b).unwrap();
            assert!(PreparedOutput::prepare(root.path(), &loop_a).is_err());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_file;

            let target = entries.path().join("target");
            fs::write(&target, b"target").unwrap();
            let link = entries.path().join("link");
            if symlink_file(&target, &link).is_err() {
                return EvidenceOutcome::Unavailable("Windows file-link privilege is unavailable");
            }
            assert!(PreparedOutput::prepare(root.path(), &link).is_err());
            assert_eq!(fs::read(&target).unwrap(), b"target");

            let dangling_target = entries.path().join("missing-target");
            let dangling = entries.path().join("dangling");
            symlink_file(&dangling_target, &dangling).unwrap();
            assert!(PreparedOutput::prepare(root.path(), &dangling).is_err());
            assert!(!dangling_target.exists());

            let second = entries.path().join("chain-second");
            let chain = entries.path().join("chain-first");
            symlink_file(&target, &second).unwrap();
            symlink_file(&second, &chain).unwrap();
            assert!(PreparedOutput::prepare(root.path(), &chain).is_err());
        }

        EvidenceOutcome::Conformant
    }

    fn observe_existing_special() -> EvidenceOutcome {
        #[cfg(unix)]
        {
            use std::os::unix::net::UnixListener;

            let root = TempDir::new().unwrap();
            let entries = TempDir::new().unwrap();
            let socket = entries.path().join("socket");
            let listener = UnixListener::bind(&socket).unwrap();
            assert!(PreparedOutput::prepare(root.path(), &socket).is_err());
            drop(listener);
        }
        #[cfg(windows)]
        created_handle_validation_rejects_device_handle();

        EvidenceOutcome::Conformant
    }

    #[cfg(not(windows))]
    fn observe_windows_reparse() -> EvidenceOutcome {
        EvidenceOutcome::Unavailable("Windows reparse evidence requires Windows")
    }

    #[cfg(windows)]
    fn observe_windows_reparse() -> EvidenceOutcome {
        use std::os::windows::fs::symlink_file;

        let root = TempDir::new().unwrap();
        let entries = TempDir::new().unwrap();
        let target = entries.path().join("target");
        let link = entries.path().join("link");
        fs::write(&target, b"target").unwrap();
        if symlink_file(&target, &link).is_err() {
            return EvidenceOutcome::Unavailable("Windows file-link privilege is unavailable");
        }
        assert!(PreparedOutput::prepare(root.path(), &link).is_err());
        assert_eq!(fs::read(target).unwrap(), b"target");
        EvidenceOutcome::Conformant
    }

    #[cfg(not(windows))]
    fn observe_windows_spelling() -> EvidenceOutcome {
        EvidenceOutcome::Unavailable("Windows path-spelling evidence requires Windows")
    }

    #[cfg(windows)]
    fn observe_windows_spelling() -> EvidenceOutcome {
        windows_output_spelling_rejects_non_filesystem_names();
        EvidenceOutcome::Conformant
    }

    fn observe_in_root_link_ancestor() -> EvidenceOutcome {
        let root = TempDir::new().unwrap();
        let external = TempDir::new().unwrap();
        let linked = root.path().join("linked");

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(external.path(), &linked).unwrap();
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_dir;
            if symlink_dir(external.path(), &linked).is_err() {
                return EvidenceOutcome::Unavailable(
                    "Windows directory-link privilege is unavailable",
                );
            }
        }

        let output = linked.join("output");
        assert!(matches!(
            PreparedOutput::prepare(root.path(), &output),
            Err(OutputError::Unsafe { .. })
        ));
        assert!(!external.path().join("output").exists());
        EvidenceOutcome::Conformant
    }

    fn observe_missing_parent() -> EvidenceOutcome {
        let root = TempDir::new().unwrap();
        let external = TempDir::new().unwrap();
        let missing = external.path().join("missing");
        let output = missing.join("output");
        assert!(matches!(
            PreparedOutput::prepare(root.path(), &output),
            Err(OutputError::MissingParent { .. })
        ));
        assert!(!missing.exists());
        EvidenceOutcome::Conformant
    }

    #[cfg(unix)]
    fn observe_read_only_parent() -> EvidenceOutcome {
        use std::os::unix::fs::PermissionsExt;

        let root = TempDir::new().unwrap();
        let parent = TempDir::new().unwrap();
        let original = fs::metadata(parent.path()).unwrap().permissions();
        fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o666)).unwrap();
        let output = parent.path().join("output");
        let generated = std::cell::Cell::new(false);
        let result = generate_to_file_typed(root.path(), &output, || {
            generated.set(true);
            Ok::<_, &'static str>(b"output".to_vec())
        });
        fs::set_permissions(parent.path(), original).unwrap();

        assert!(matches!(
            result,
            Err(GenerateOutputError::Output(
                OutputError::ParentNotWritable { .. } | OutputError::Io { .. }
            ))
        ));
        assert!(!generated.get());
        assert!(!output.exists());
        EvidenceOutcome::Conformant
    }

    #[cfg(windows)]
    fn observe_read_only_parent() -> EvidenceOutcome {
        use std::process::Command;

        let root = TempDir::new().unwrap();
        let parent = TempDir::new().unwrap();
        let denied = Command::new("icacls")
            .arg(parent.path())
            .arg("/deny")
            .arg("*S-1-1-0:(W)")
            .output()
            .unwrap();
        if !denied.status.success() {
            return EvidenceOutcome::Unavailable(
                "Windows DACL-denial fixture could not be configured",
            );
        }

        let output = parent.path().join("output");
        let generated = std::cell::Cell::new(false);
        let result = generate_to_file_typed(root.path(), &output, || {
            generated.set(true);
            Ok::<_, &'static str>(b"output".to_vec())
        });

        let restored = Command::new("icacls")
            .arg(parent.path())
            .arg("/remove:d")
            .arg("*S-1-1-0")
            .output()
            .unwrap();
        assert!(
            restored.status.success(),
            "failed to restore Windows DACL fixture: {}",
            String::from_utf8_lossy(&restored.stderr)
        );

        assert!(matches!(
            result,
            Err(GenerateOutputError::Output(
                OutputError::ParentNotWritable { .. } | OutputError::Io { .. }
            ))
        ));
        assert!(!generated.get());
        assert!(!output.exists());
        EvidenceOutcome::Conformant
    }

    fn observe_creator_race() -> EvidenceOutcome {
        exclusive_create_has_exactly_one_winner_for_100_races();
        EvidenceOutcome::Conformant
    }

    fn observe_delivery_failures() -> EvidenceOutcome {
        delivery_stage_failures_are_cleaned_up();
        EvidenceOutcome::Conformant
    }

    fn observe_cleanup_failure() -> EvidenceOutcome {
        cleanup_failure_reports_residual_and_preserves_replacement();
        EvidenceOutcome::Conformant
    }

    fn observe_preexisting_output() -> EvidenceOutcome {
        preexisting_destination_prevents_generation();
        EvidenceOutcome::Conformant
    }

    fn observe_generation_failure() -> EvidenceOutcome {
        generation_failure_precedes_creation();
        EvidenceOutcome::Conformant
    }

    fn observe_visibility_documentation() -> EvidenceOutcome {
        readme_documents_non_atomic_content_publication();
        EvidenceOutcome::Conformant
    }

    fn issue_45_trust_ids(source: &str) -> BTreeSet<&str> {
        source
            .split("TrustCase {")
            .skip(1)
            .filter_map(|block| {
                let block = block.split_once("},").map_or(block, |(case, _)| case);
                if !block.contains("owner_issue: 45") {
                    return None;
                }
                block.lines().find_map(|line| {
                    line.trim()
                        .strip_prefix("id: \"")
                        .and_then(|value| value.strip_suffix("\","))
                })
            })
            .collect()
    }
}
