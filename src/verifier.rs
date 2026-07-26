//! Filesystem verification for navigation guides

use crate::entry_type::{
    classify_metadata, EntryClassification, SupportedEntryKind, UnsupportedEntryKind,
};
use crate::errors::{AppError, Result, SemanticError};
use crate::path_codec::{
    contains_forbidden_control, has_windows_drive_prefix, render_os_component,
    render_utf8_component,
};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn read_directory(path: &Path) -> std::io::Result<std::fs::ReadDir> {
    #[cfg(test)]
    DIRECTORY_ENUMERATION_COUNTS.with(|counts| {
        *counts.borrow_mut().entry(path.to_path_buf()).or_insert(0) += 1;
    });

    std::fs::read_dir(path)
}

#[cfg(test)]
thread_local! {
    static DIRECTORY_ENUMERATION_COUNTS:
        std::cell::RefCell<std::collections::BTreeMap<PathBuf, usize>> =
            const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

#[cfg(test)]
fn reset_directory_enumeration_counts() {
    DIRECTORY_ENUMERATION_COUNTS.with(|counts| counts.borrow_mut().clear());
}

#[cfg(test)]
fn directory_enumeration_counts() -> std::collections::BTreeMap<PathBuf, usize> {
    DIRECTORY_ENUMERATION_COUNTS.with(|counts| counts.borrow().clone())
}

/// Verifier for navigation guides against filesystem
pub struct Verifier {
    /// Root path for verification
    root_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EntryObservation {
    classification: EntryClassification,
    identity: FileIdentity,
}

#[derive(Clone, Debug)]
struct SnapshotEntry {
    path: PathBuf,
    observation: EntryObservation,
}

struct ResolvedItem {
    entry: SnapshotEntry,
    ancestors: Vec<SnapshotEntry>,
}

#[derive(Debug)]
struct DirectorySnapshot {
    parent_path: PathBuf,
    parent_observation: EntryObservation,
    entries: BTreeMap<String, SnapshotEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerificationCheckpoint {
    AfterSnapshotEntrySelected,
    AfterDirectoryEnumeration,
    BeforeFinalRevalidation,
}

trait VerificationControl {
    fn checkpoint(&mut self, stage: VerificationCheckpoint, path: &Path) -> io::Result<()>;
}

struct NoopVerificationControl;

impl VerificationControl for NoopVerificationControl {
    fn checkpoint(&mut self, _stage: VerificationCheckpoint, _path: &Path) -> io::Result<()> {
        Ok(())
    }
}

struct VerificationRun<'a, C> {
    verifier: &'a Verifier,
    canonical_root_path: PathBuf,
    snapshots: HashMap<PathBuf, Rc<DirectorySnapshot>>,
    control: C,
}

impl Verifier {
    /// Create a new verifier with the given root path
    pub fn new(root_path: &Path) -> Self {
        Self {
            root_path: root_path.to_path_buf(),
        }
    }

    /// Verify a navigation guide against the filesystem
    pub fn verify(&self, guide: &NavigationGuide) -> Result<()> {
        self.verify_with_control(guide, NoopVerificationControl)
    }

    fn verify_with_control<C: VerificationControl>(
        &self,
        guide: &NavigationGuide,
        control: C,
    ) -> Result<()> {
        // First validate syntax (should already be done, but double-check)
        crate::validator::Validator::new().validate_syntax(guide)?;
        let canonical_root_path = self.canonicalize_root_path()?;

        // The caller-selected spelling is used only to select the anchor.
        // Every dependent access starts from its once-canonicalized directory,
        // so retargeting a root alias cannot redirect later verification I/O.
        VerificationRun::new(self, canonical_root_path.clone(), control).verify_siblings(
            &guide.items,
            &canonical_root_path,
            Path::new(""),
            true,
        )
    }

    /// Canonicalize the root once while retaining parent-component order.
    fn canonicalize_root_path(&self) -> Result<PathBuf> {
        Ok(canonicalize_root_preserving_parent_order(&self.root_path)?)
    }

    /// Get a human-readable string for the item type
    fn get_item_type_string(&self, item: &NavigationGuideLine) -> String {
        match &item.item {
            FilesystemItem::Directory { .. } => "directory".to_string(),
            FilesystemItem::File { .. } => "file".to_string(),
            FilesystemItem::Symlink { .. } => "symlink".to_string(),
            FilesystemItem::Placeholder { .. } => "placeholder".to_string(),
        }
    }

    fn placeholder_has_meaningful_comment(item: &NavigationGuideLine) -> bool {
        item.comment()
            .map(|comment| !comment.trim().is_empty())
            .unwrap_or(false)
    }
}

fn canonicalize_root_preserving_parent_order(path: &Path) -> io::Result<PathBuf> {
    use std::path::Component;

    if !path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return std::fs::canonicalize(path);
    }

    // Windows normally normalizes `..` before CreateFile follows a reparse
    // component. Resolve each spelling prefix that precedes `..` first so a
    // caller-selected `alias/..` means the parent of the alias target on every
    // supported platform, as required by the v0.2 anchor contract.
    let mut pending = match path.components().next() {
        Some(Component::Prefix(_) | Component::RootDir) => PathBuf::new(),
        _ => std::env::current_dir()?,
    };
    for component in path.components() {
        match component {
            Component::ParentDir => {
                let resolved_prefix = std::fs::canonicalize(&pending)?;
                if !std::fs::metadata(&resolved_prefix)?.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "a verification-root component before '..' is not a directory",
                    ));
                }
                pending = resolved_prefix;
                pending.pop();
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                pending.push(component.as_os_str());
            }
        }
    }

    std::fs::canonicalize(pending)
}

impl<'a, C: VerificationControl> VerificationRun<'a, C> {
    fn new(verifier: &'a Verifier, canonical_root_path: PathBuf, control: C) -> Self {
        Self {
            verifier,
            canonical_root_path,
            snapshots: HashMap::new(),
            control,
        }
    }

    fn verify_siblings(
        &mut self,
        items: &[NavigationGuideLine],
        parent_path: &Path,
        logical_parent: &Path,
        at_root: bool,
    ) -> Result<()> {
        let Some(snapshot_line) = items.iter().map(|item| item.line_number).min() else {
            return Ok(());
        };
        let snapshot = self.snapshot(parent_path, snapshot_line, at_root)?;
        let mentioned_names = items
            .iter()
            .filter(|item| !item.is_placeholder())
            .filter_map(|item| item.path().split('/').next())
            .map(str::to_string)
            .collect::<HashSet<_>>();
        let has_unmentioned_item = snapshot
            .entries
            .keys()
            .any(|name| !mentioned_names.contains(name));

        for item in items {
            if item.is_placeholder() {
                if !has_unmentioned_item && !Verifier::placeholder_has_meaningful_comment(item) {
                    return Err(SemanticError::PlaceholderNoUnmentionedItems {
                        line: item.line_number,
                        parent: self
                            .verifier
                            .root_path
                            .join(logical_parent)
                            .to_string_lossy()
                            .to_string(),
                    }
                    .into());
                }
            } else {
                self.verify_item(item, parent_path, logical_parent, at_root)?;
            }
        }

        self.revalidate_observation(
            &snapshot.parent_path,
            &snapshot.parent_observation,
            snapshot_line,
            "the containing directory",
        )?;
        Ok(())
    }

    fn verify_item(
        &mut self,
        item: &NavigationGuideLine,
        parent_path: &Path,
        logical_parent: &Path,
        at_root: bool,
    ) -> Result<()> {
        let resolved_entry =
            self.resolve_exact_item_path(item, parent_path, logical_parent, at_root)?;
        let item_path = resolved_entry.entry.path.clone();
        let classification = resolved_entry.entry.observation.classification;

        // Preserve the legacy programmatic Symlink variant until its #53
        // removal. Its existing dangling-link behavior is deliberately not
        // part of textual file/directory classification.
        if matches!(&item.item, FilesystemItem::Symlink { .. }) && !item_path.exists() {
            return Err(SemanticError::ItemNotFound {
                line: item.line_number,
                item_type: self.verifier.get_item_type_string(item),
                path: item.path().to_string(),
                full_path: item_path,
            }
            .into());
        }

        match &item.item {
            FilesystemItem::Directory { children, .. } => {
                Self::require_entry_kind(item, classification, SupportedEntryKind::Directory)?;
                self.verify_siblings(
                    children,
                    &item_path,
                    &logical_parent.join(item.path()),
                    false,
                )?;
            }
            FilesystemItem::File { .. } => {
                Self::require_entry_kind(item, classification, SupportedEntryKind::RegularFile)?;
            }
            FilesystemItem::Symlink { target, .. } => {
                if classification != Err(UnsupportedEntryKind::SymbolicLink) {
                    return Err(SemanticError::TypeMismatch {
                        line: item.line_number,
                        expected: "symlink".to_string(),
                        found: Self::classification_name(classification),
                        path: item.path().to_string(),
                    }
                    .into());
                }

                if let Some(expected_target) = target {
                    if let Ok(actual_target) = std::fs::read_link(&item_path) {
                        if actual_target.to_string_lossy() != *expected_target {
                            return Err(SemanticError::SymlinkTargetMismatch {
                                line: item.line_number,
                                path: item.path().to_string(),
                                expected: expected_target.clone(),
                                actual: actual_target.to_string_lossy().to_string(),
                            }
                            .into());
                        }
                    }
                }
            }
            FilesystemItem::Placeholder { .. } => {
                unreachable!("placeholder items are handled as sibling assertions")
            }
        }

        self.control
            .checkpoint(
                VerificationCheckpoint::BeforeFinalRevalidation,
                &resolved_entry.entry.path,
            )
            .map_err(AppError::Io)?;
        self.revalidate_observation(
            &resolved_entry.entry.path,
            &resolved_entry.entry.observation,
            item.line_number,
            item.path(),
        )?;
        for ancestor in resolved_entry.ancestors.iter().rev() {
            self.control
                .checkpoint(
                    VerificationCheckpoint::BeforeFinalRevalidation,
                    &ancestor.path,
                )
                .map_err(AppError::Io)?;
            self.revalidate_observation(
                &ancestor.path,
                &ancestor.observation,
                item.line_number,
                item.path(),
            )?;
        }
        Ok(())
    }

    fn resolve_exact_item_path(
        &mut self,
        item: &NavigationGuideLine,
        parent_path: &Path,
        logical_parent: &Path,
        at_root: bool,
    ) -> Result<ResolvedItem> {
        let components = item.path().split('/').collect::<Vec<_>>();
        let full_path = self
            .verifier
            .root_path
            .join(logical_parent)
            .join(item.path());
        let mut current_parent = parent_path.to_path_buf();
        let mut current_at_root = at_root;
        let mut ancestors = Vec::new();

        for (index, component) in components.iter().enumerate() {
            let snapshot = self.snapshot(&current_parent, item.line_number, current_at_root)?;
            let exact_entry = snapshot.entries.get(*component).cloned();
            let Some(entry) = exact_entry else {
                return self.missing_exact_component(item, &current_parent, component, &full_path);
            };

            self.control
                .checkpoint(
                    VerificationCheckpoint::AfterSnapshotEntrySelected,
                    &entry.path,
                )
                .map_err(AppError::Io)?;
            self.revalidate_observation(
                &entry.path,
                &entry.observation,
                item.line_number,
                &components[..=index].join("/"),
            )?;

            let classification = entry.observation.classification;
            if index + 1 == components.len() {
                if classification.is_ok() {
                    self.require_canonical_containment(&entry.path, item)?;
                }
                return Ok(ResolvedItem { entry, ancestors });
            }
            if classification != Ok(SupportedEntryKind::Directory) {
                return Err(SemanticError::TypeMismatch {
                    line: item.line_number,
                    expected: "directory".to_string(),
                    found: Self::classification_name(classification),
                    path: components[..=index].join("/"),
                }
                .into());
            }

            self.require_canonical_containment(&entry.path, item)?;
            ancestors.push(entry.clone());
            current_parent = entry.path;
            current_at_root = false;
        }

        unreachable!("validated filesystem item paths contain at least one component")
    }

    fn missing_exact_component(
        &self,
        item: &NavigationGuideLine,
        parent_path: &Path,
        component: &str,
        full_path: &Path,
    ) -> Result<ResolvedItem> {
        if Self::is_single_host_component(component) {
            match std::fs::symlink_metadata(parent_path.join(component)) {
                Ok(_) => {
                    return Err(AppError::Other(format!(
                        "line {}: path component {} is not an exact filesystem name \
                         (host lookup resolved a spelling absent from the directory snapshot)",
                        item.line_number,
                        render_utf8_component(component)
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(SemanticError::PermissionDenied {
                        line: item.line_number,
                        path: item.path().to_string(),
                    }
                    .into());
                }
                Err(error) => return Err(error.into()),
            }
        }

        Err(SemanticError::ItemNotFound {
            line: item.line_number,
            item_type: self.verifier.get_item_type_string(item),
            path: item.path().to_string(),
            full_path: full_path.to_path_buf(),
        }
        .into())
    }

    fn require_canonical_containment(
        &self,
        entry_path: &Path,
        item: &NavigationGuideLine,
    ) -> Result<()> {
        let resolved = match std::fs::canonicalize(entry_path) {
            Ok(resolved) => resolved,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                return Err(SemanticError::PermissionDenied {
                    line: item.line_number,
                    path: item.path().to_string(),
                }
                .into());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(Self::observed_change_error(
                    item.line_number,
                    item.path(),
                    "disappeared during containment validation",
                ));
            }
            Err(error) => return Err(error.into()),
        };
        if resolved.starts_with(&self.canonical_root_path) {
            return Ok(());
        }

        Err(Self::path_escape_error(item))
    }

    fn path_escape_error(item: &NavigationGuideLine) -> AppError {
        SemanticError::PathEscapesRoot {
            line: item.line_number,
            path: item.path().to_string(),
            // Keep the transitional public variant shape stable until #54,
            // but never store resolved alias targets in an emitted error.
            root: PathBuf::from("<redacted>"),
            resolved: PathBuf::from("<redacted>"),
        }
        .into()
    }

    fn is_single_host_component(component: &str) -> bool {
        let mut components = Path::new(component).components();
        matches!(
            (components.next(), components.next()),
            (Some(std::path::Component::Normal(_)), None)
        )
    }

    fn require_entry_kind(
        item: &NavigationGuideLine,
        classification: EntryClassification,
        expected: SupportedEntryKind,
    ) -> Result<()> {
        if classification == Ok(expected) {
            return Ok(());
        }

        Err(SemanticError::TypeMismatch {
            line: item.line_number,
            expected: match expected {
                SupportedEntryKind::RegularFile => "file",
                SupportedEntryKind::Directory => "directory",
            }
            .to_string(),
            found: Self::classification_name(classification),
            path: item.path().to_string(),
        }
        .into())
    }

    fn classification_name(classification: EntryClassification) -> String {
        match classification {
            Ok(SupportedEntryKind::RegularFile) => "file".to_string(),
            Ok(SupportedEntryKind::Directory) => "directory".to_string(),
            Err(unsupported) => unsupported.to_string(),
        }
    }

    fn snapshot(
        &mut self,
        parent_path: &Path,
        line: usize,
        at_root: bool,
    ) -> Result<Rc<DirectorySnapshot>> {
        if let Some(snapshot) = self.snapshots.get(parent_path).cloned() {
            self.revalidate_observation(
                &snapshot.parent_path,
                &snapshot.parent_observation,
                line,
                "the containing directory",
            )?;
            return Ok(snapshot);
        }

        let snapshot = Rc::new(self.build_snapshot(parent_path, line, at_root)?);
        self.snapshots
            .insert(parent_path.to_path_buf(), Rc::clone(&snapshot));
        Ok(snapshot)
    }

    fn build_snapshot(
        &mut self,
        parent_path: &Path,
        line: usize,
        at_root: bool,
    ) -> Result<DirectorySnapshot> {
        let parent_before =
            Self::observe_for_verification(parent_path, line, "the containing directory")?;
        if parent_before.classification != Ok(SupportedEntryKind::Directory) {
            return Err(Self::observed_change_error(
                line,
                "the containing directory",
                "is no longer a real directory",
            ));
        }

        let entries = match read_directory(parent_path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                return Err(SemanticError::PermissionDenied {
                    line,
                    path: "the containing directory".to_string(),
                }
                .into());
            }
            Err(error) => return Err(error.into()),
        };

        let mut observed_entries = Vec::new();
        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    return Err(SemanticError::PermissionDenied {
                        line,
                        path: "the containing directory".to_string(),
                    }
                    .into());
                }
                Err(error) => return Err(error.into()),
            };
            observed_entries.push((entry.file_name(), entry.path()));
        }

        let snapshot_entries =
            Self::build_snapshot_from_observations(line, at_root, observed_entries)?;
        self.control
            .checkpoint(
                VerificationCheckpoint::AfterDirectoryEnumeration,
                parent_path,
            )
            .map_err(AppError::Io)?;
        let parent_after =
            Self::observe_for_verification(parent_path, line, "the containing directory")?;
        if parent_before != parent_after {
            return Err(Self::observed_change_error(
                line,
                "the containing directory",
                "changed identity or type during enumeration",
            ));
        }

        Ok(DirectorySnapshot {
            parent_path: parent_path.to_path_buf(),
            parent_observation: parent_after,
            entries: snapshot_entries,
        })
    }

    fn build_snapshot_from_observations(
        line: usize,
        at_root: bool,
        mut observed_entries: Vec<(OsString, PathBuf)>,
    ) -> Result<BTreeMap<String, SnapshotEntry>> {
        observed_entries.sort_by(|left, right| left.0.cmp(&right.0));

        let mut snapshot_entries = BTreeMap::new();
        for (name, path) in observed_entries {
            let utf8_name = name.to_str().ok_or_else(|| SemanticError::NonUtf8Path {
                line,
                path: PathBuf::from(render_os_component(&name)),
            })?;
            if contains_forbidden_control(utf8_name) {
                return Err(AppError::Other(format!(
                    "line {line}: unsupported control-bearing filesystem name {}",
                    render_utf8_component(utf8_name)
                )));
            }
            if at_root && (utf8_name.starts_with('\\') || has_windows_drive_prefix(utf8_name)) {
                return Err(AppError::Other(format!(
                    "line {line}: unsupported rooted or drive-prefixed filesystem name {}",
                    render_utf8_component(utf8_name)
                )));
            }

            let observation = match observe_path(&path) {
                Ok(observation) => observation,
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    return Err(SemanticError::PermissionDenied {
                        line,
                        path: utf8_name.to_string(),
                    }
                    .into());
                }
                Err(error) => {
                    return Err(AppError::Other(format!(
                        "line {line}: could not classify filesystem name {}: {error}",
                        render_utf8_component(utf8_name)
                    )));
                }
            };
            let previous =
                snapshot_entries.insert(utf8_name.to_string(), SnapshotEntry { path, observation });
            if previous.is_some() {
                return Err(AppError::Other(format!(
                    "line {line}: ambiguous duplicate exact filesystem name {}",
                    render_utf8_component(utf8_name)
                )));
            }
        }

        Ok(snapshot_entries)
    }

    fn observe_for_verification(
        path: &Path,
        line: usize,
        logical_path: &str,
    ) -> Result<EntryObservation> {
        match observe_path(path) {
            Ok(observation) => Ok(observation),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Err(
                Self::observed_change_error(line, logical_path, "disappeared during verification"),
            ),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                Err(SemanticError::PermissionDenied {
                    line,
                    path: logical_path.to_string(),
                }
                .into())
            }
            Err(error) => Err(AppError::Other(format!(
                "line {line}: could not observe {} without following it: {error}",
                render_utf8_component(logical_path)
            ))),
        }
    }

    fn revalidate_observation(
        &self,
        path: &Path,
        expected: &EntryObservation,
        line: usize,
        logical_path: &str,
    ) -> Result<()> {
        let observed = Self::observe_for_verification(path, line, logical_path)?;
        if observed == *expected {
            return Ok(());
        }

        Err(Self::observed_change_error(
            line,
            logical_path,
            "changed identity or type during verification",
        ))
    }

    fn observed_change_error(line: usize, logical_path: &str, reason: &str) -> AppError {
        AppError::Other(format!(
            "line {line}: filesystem entry {} {reason}",
            render_utf8_component(logical_path)
        ))
    }
}

fn observe_path(path: &Path) -> io::Result<EntryObservation> {
    // Classification and identity may require separate non-following system
    // calls. Their pair is re-observed before dependent use; this is
    // defense-in-depth for a stable tree, not an atomic hostile-race primitive.
    let metadata = std::fs::symlink_metadata(path)?;
    let classification = classify_metadata(path, &metadata)?;
    let identity = path_identity(path, &metadata)?;
    Ok(EntryObservation {
        classification,
        identity,
    })
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
fn path_identity(path: &Path, _metadata: &Metadata) -> io::Result<FileIdentity> {
    use std::fs::OpenOptions;
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
    windows_handle_identity(&file)
}

#[cfg(windows)]
fn windows_handle_identity(file: &std::fs::File) -> io::Result<FileIdentity> {
    use std::mem::{size_of, MaybeUninit};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileIdInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
        BY_HANDLE_FILE_INFORMATION, FILE_ID_INFO,
    };

    let handle = file.as_raw_handle().cast();
    let mut extended = MaybeUninit::<FILE_ID_INFO>::zeroed();
    // SAFETY: `extended` is the exact writable structure requested by
    // FileIdInfo, and `handle` remains owned by `file` for the call.
    let extended_ok = unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            extended.as_mut_ptr().cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if extended_ok != 0 {
        // SAFETY: the successful API call initialized the complete structure.
        let extended = unsafe { extended.assume_init() };
        if extended.FileId.Identifier.iter().any(|byte| *byte != 0) {
            return Ok(FileIdentity::Extended {
                volume_serial: extended.VolumeSerialNumber,
                file_id: extended.FileId.Identifier,
            });
        }
    }

    let mut legacy = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: `legacy` is the exact writable structure required by
    // GetFileInformationByHandle, and `handle` remains valid for the call.
    let legacy_ok = unsafe { GetFileInformationByHandle(handle, legacy.as_mut_ptr()) };
    if legacy_ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful API call initialized the complete structure.
    let legacy = unsafe { legacy.assume_init() };
    let file_index = (u64::from(legacy.nFileIndexHigh) << 32) | u64::from(legacy.nFileIndexLow);
    if file_index == 0 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "the filesystem supplied no reliable entry identity",
        ));
    }

    Ok(FileIdentity::Legacy {
        volume_serial: legacy.dwVolumeSerialNumber,
        file_index,
    })
}

#[cfg(not(any(unix, windows)))]
compile_error!("verification identity validation is implemented only for Unix and Windows");

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    use tempfile::TempDir;

    #[test]
    fn test_verify_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::File {
                    path: "missing.txt".to_string(),
                    comment: None,
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::ItemNotFound { .. }
            ))
        ));
    }

    #[test]
    fn test_verify_rejects_path_outside_root_boundary() {
        let temp_dir = TempDir::new().unwrap();
        let root_dir = temp_dir.path().join("project");
        std::fs::create_dir(&root_dir).unwrap();
        std::fs::write(temp_dir.path().join("outside.txt"), "").unwrap();

        let verifier = Verifier::new(&root_dir);
        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::File {
                    path: "../outside.txt".to_string(),
                    comment: None,
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                crate::errors::SyntaxError::InvalidSpecialDirectory { .. }
            ))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_rejects_final_directory_symlink_without_following_outside_root() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let root_dir = temp_dir.path().join("project");
        let outside_dir = temp_dir.path().join("outside");
        std::fs::create_dir(&root_dir).unwrap();
        std::fs::create_dir(&outside_dir).unwrap();
        std::fs::write(outside_dir.join("secret.txt"), "").unwrap();
        symlink(&outside_dir, root_dir.join("linked")).unwrap();

        let verifier = Verifier::new(&root_dir);
        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::Directory {
                    path: "linked".to_string(),
                    comment: None,
                    children: vec![NavigationGuideLine {
                        line_number: 2,
                        indent_level: 1,
                        item: FilesystemItem::File {
                            path: "secret.txt".to_string(),
                            comment: None,
                        },
                    }],
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::TypeMismatch {
                    expected,
                    found,
                    path,
                    ..
                }
            )) if expected == "directory"
                && found == "symbolic link"
                && path == "linked"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_rejects_final_file_symlink_without_following_outside_root() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let root_dir = temp_dir.path().join("project");
        let outside_file = temp_dir.path().join("outside.txt");
        std::fs::create_dir(&root_dir).unwrap();
        std::fs::write(&outside_file, "secret").unwrap();
        symlink(&outside_file, root_dir.join("linked.txt")).unwrap();

        let verifier = Verifier::new(&root_dir);
        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::File {
                    path: "linked.txt".to_string(),
                    comment: None,
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::TypeMismatch {
                    expected,
                    found,
                    path,
                    ..
                }
            )) if expected == "file"
                && found == "symbolic link"
                && path == "linked.txt"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_rejects_missing_path_within_symlink_outside_root() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let root_dir = temp_dir.path().join("project");
        let outside_dir = temp_dir.path().join("outside");
        std::fs::create_dir(&root_dir).unwrap();
        std::fs::create_dir(&outside_dir).unwrap();
        symlink(&outside_dir, root_dir.join("linked")).unwrap();

        let verifier = Verifier::new(&root_dir);
        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::File {
                    path: "linked/missing.txt".to_string(),
                    comment: None,
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::TypeMismatch {
                    expected,
                    found,
                    path,
                    ..
                }
            ))
                if expected == "directory"
                    && found == "symbolic link"
                    && path == "linked"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_handles_circular_symlink_without_panicking() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let root_dir = temp_dir.path().join("project");
        std::fs::create_dir(&root_dir).unwrap();
        symlink("loop", root_dir.join("loop")).unwrap();

        let verifier = Verifier::new(&root_dir);
        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::File {
                    path: "loop/file.txt".to_string(),
                    comment: None,
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::TypeMismatch {
                    expected,
                    found,
                    path,
                    ..
                }
            ))
                if expected == "directory"
                    && found == "symbolic link"
                    && path == "loop"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_rejects_final_directory_symlink_within_root() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let root_dir = temp_dir.path().join("project");
        let real_dir = root_dir.join("real");
        std::fs::create_dir(&root_dir).unwrap();
        std::fs::create_dir(&real_dir).unwrap();
        std::fs::write(real_dir.join("inside.txt"), "").unwrap();
        symlink(&real_dir, root_dir.join("alias")).unwrap();

        let verifier = Verifier::new(&root_dir);
        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::Directory {
                    path: "alias".to_string(),
                    comment: None,
                    children: vec![NavigationGuideLine {
                        line_number: 2,
                        indent_level: 1,
                        item: FilesystemItem::File {
                            path: "inside.txt".to_string(),
                            comment: None,
                        },
                    }],
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::TypeMismatch {
                    expected,
                    found,
                    path,
                    ..
                }
            )) if expected == "directory"
                && found == "symbolic link"
                && path == "alias"
        ));
    }

    #[test]
    fn test_verify_placeholder_with_unmentioned_items() {
        let temp_dir = TempDir::new().unwrap();

        // Create files in temp directory
        std::fs::write(temp_dir.path().join("main.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("lib.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("mod.rs"), "").unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![
                NavigationGuideLine {
                    line_number: 1,
                    indent_level: 0,
                    item: FilesystemItem::File {
                        path: "main.rs".to_string(),
                        comment: None,
                    },
                },
                NavigationGuideLine {
                    line_number: 2,
                    indent_level: 0,
                    item: FilesystemItem::Placeholder {
                        comment: Some("other source files".to_string()),
                    },
                },
            ],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        // Should succeed because lib.rs and mod.rs are unmentioned
        let result = verifier.verify(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_placeholder_with_comment_no_items() {
        let temp_dir = TempDir::new().unwrap();

        // Create only one file
        std::fs::write(temp_dir.path().join("main.rs"), "").unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![
                NavigationGuideLine {
                    line_number: 1,
                    indent_level: 0,
                    item: FilesystemItem::File {
                        path: "main.rs".to_string(),
                        comment: None,
                    },
                },
                NavigationGuideLine {
                    line_number: 2,
                    indent_level: 0,
                    item: FilesystemItem::Placeholder {
                        comment: Some("future files will appear here".to_string()),
                    },
                },
            ],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        // Should succeed because placeholder has a comment (represents future items)
        let result = verifier.verify(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_placeholder_with_whitespace_comment_no_items_fails() {
        let temp_dir = TempDir::new().unwrap();

        // Create only one file
        std::fs::write(temp_dir.path().join("main.rs"), "").unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![
                NavigationGuideLine {
                    line_number: 1,
                    indent_level: 0,
                    item: FilesystemItem::File {
                        path: "main.rs".to_string(),
                        comment: None,
                    },
                },
                NavigationGuideLine {
                    line_number: 2,
                    indent_level: 0,
                    item: FilesystemItem::Placeholder {
                        comment: Some("   \t   ".to_string()),
                    },
                },
            ],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::PlaceholderNoUnmentionedItems { .. }
            ))
        ));
    }

    #[test]
    fn test_verify_placeholder_with_non_empty_comment_remains_relaxed() {
        let temp_dir = TempDir::new().unwrap();

        // Create only one file
        std::fs::write(temp_dir.path().join("main.rs"), "").unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![
                NavigationGuideLine {
                    line_number: 1,
                    indent_level: 0,
                    item: FilesystemItem::File {
                        path: "main.rs".to_string(),
                        comment: None,
                    },
                },
                NavigationGuideLine {
                    line_number: 2,
                    indent_level: 0,
                    item: FilesystemItem::Placeholder {
                        comment: Some("  future files  ".to_string()),
                    },
                },
            ],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let result = verifier.verify(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_placeholder_without_comment_no_items() {
        let temp_dir = TempDir::new().unwrap();

        // Create only one file
        std::fs::write(temp_dir.path().join("main.rs"), "").unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![
                NavigationGuideLine {
                    line_number: 1,
                    indent_level: 0,
                    item: FilesystemItem::File {
                        path: "main.rs".to_string(),
                        comment: None,
                    },
                },
                NavigationGuideLine {
                    line_number: 2,
                    indent_level: 0,
                    item: FilesystemItem::Placeholder { comment: None },
                },
            ],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        // Should fail because placeholder has no comment and all items are mentioned
        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::PlaceholderNoUnmentionedItems { .. }
            ))
        ));
    }

    #[test]
    fn test_verify_placeholder_in_directory() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();

        // Create files in src directory
        std::fs::write(src_dir.join("main.rs"), "").unwrap();
        std::fs::write(src_dir.join("lib.rs"), "").unwrap();
        std::fs::write(src_dir.join("utils.rs"), "").unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::Directory {
                    path: "src".to_string(),
                    comment: None,
                    children: vec![
                        NavigationGuideLine {
                            line_number: 2,
                            indent_level: 1,
                            item: FilesystemItem::File {
                                path: "main.rs".to_string(),
                                comment: None,
                            },
                        },
                        NavigationGuideLine {
                            line_number: 3,
                            indent_level: 1,
                            item: FilesystemItem::Placeholder {
                                comment: Some("other modules".to_string()),
                            },
                        },
                    ],
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        // Should succeed because lib.rs and utils.rs are unmentioned
        let result = verifier.verify(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_placeholder_in_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::Directory {
                    path: "src".to_string(),
                    comment: None,
                    children: vec![NavigationGuideLine {
                        line_number: 2,
                        indent_level: 1,
                        item: FilesystemItem::Placeholder {
                            comment: Some("future files".to_string()),
                        },
                    }],
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        // Should succeed because placeholder has a comment (represents future files)
        let result = verifier.verify(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_placeholder_in_empty_directory_no_comment() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::Directory {
                    path: "src".to_string(),
                    comment: None,
                    children: vec![NavigationGuideLine {
                        line_number: 2,
                        indent_level: 1,
                        item: FilesystemItem::Placeholder { comment: None },
                    }],
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        // Should fail because directory is empty and placeholder has no comment
        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::PlaceholderNoUnmentionedItems { .. }
            ))
        ));
    }

    #[test]
    fn test_multiple_placeholders_mixed_comments() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();

        // Create some files
        std::fs::write(src_dir.join("main.rs"), "").unwrap();
        std::fs::write(src_dir.join("lib.rs"), "").unwrap();
        std::fs::write(src_dir.join("utils.rs"), "").unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::Directory {
                    path: "src".to_string(),
                    comment: None,
                    children: vec![
                        NavigationGuideLine {
                            line_number: 2,
                            indent_level: 1,
                            item: FilesystemItem::File {
                                path: "main.rs".to_string(),
                                comment: None,
                            },
                        },
                        NavigationGuideLine {
                            line_number: 3,
                            indent_level: 1,
                            item: FilesystemItem::Placeholder {
                                comment: Some("other modules".to_string()),
                            },
                        },
                        NavigationGuideLine {
                            line_number: 4,
                            indent_level: 1,
                            item: FilesystemItem::File {
                                path: "lib.rs".to_string(),
                                comment: None,
                            },
                        },
                        NavigationGuideLine {
                            line_number: 5,
                            indent_level: 1,
                            item: FilesystemItem::Placeholder {
                                comment: Some("future expansion files".to_string()),
                            },
                        },
                    ],
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        // Should succeed - both placeholders have comments, and there's an unmentioned file (utils.rs)
        let result = verifier.verify(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_placeholder_with_comment_in_nested_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("src/modules/auth");
        std::fs::create_dir_all(&nested_dir).unwrap();

        // Create only one file in the nested directory
        std::fs::write(nested_dir.join("login.rs"), "").unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![NavigationGuideLine {
                line_number: 1,
                indent_level: 0,
                item: FilesystemItem::Directory {
                    path: "src".to_string(),
                    comment: None,
                    children: vec![NavigationGuideLine {
                        line_number: 2,
                        indent_level: 1,
                        item: FilesystemItem::Directory {
                            path: "modules".to_string(),
                            comment: None,
                            children: vec![NavigationGuideLine {
                                line_number: 3,
                                indent_level: 2,
                                item: FilesystemItem::Directory {
                                    path: "auth".to_string(),
                                    comment: None,
                                    children: vec![
                                        NavigationGuideLine {
                                            line_number: 4,
                                            indent_level: 3,
                                            item: FilesystemItem::File {
                                                path: "login.rs".to_string(),
                                                comment: None,
                                            },
                                        },
                                        NavigationGuideLine {
                                            line_number: 5,
                                            indent_level: 3,
                                            item: FilesystemItem::Placeholder {
                                                comment: Some(
                                                    "additional auth features coming soon"
                                                        .to_string(),
                                                ),
                                            },
                                        },
                                    ],
                                },
                            }],
                        },
                    }],
                },
            }],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        // Should succeed - placeholder has a comment even in deeply nested directory
        let result = verifier.verify(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_placeholder_without_comment_with_unmentioned() {
        let temp_dir = TempDir::new().unwrap();

        // Create multiple files
        std::fs::write(temp_dir.path().join("main.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("lib.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("utils.rs"), "").unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![
                NavigationGuideLine {
                    line_number: 1,
                    indent_level: 0,
                    item: FilesystemItem::File {
                        path: "main.rs".to_string(),
                        comment: None,
                    },
                },
                NavigationGuideLine {
                    line_number: 2,
                    indent_level: 0,
                    item: FilesystemItem::Placeholder { comment: None },
                },
            ],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        // Should succeed - placeholder without comment is ok when unmentioned items exist
        let result = verifier.verify(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_placeholder_with_utf8_unmentioned_items() {
        let temp_dir = TempDir::new().unwrap();

        std::fs::write(temp_dir.path().join("main.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("naïve-文件.rs"), "").unwrap();

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![
                NavigationGuideLine {
                    line_number: 1,
                    indent_level: 0,
                    item: FilesystemItem::File {
                        path: "main.rs".to_string(),
                        comment: None,
                    },
                },
                NavigationGuideLine {
                    line_number: 2,
                    indent_level: 0,
                    item: FilesystemItem::Placeholder { comment: None },
                },
            ],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let result = verifier.verify(&guide);
        assert!(result.is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_placeholder_rejects_non_utf8_items() {
        use std::ffi::OsStr;

        let temp_dir = TempDir::new().unwrap();

        std::fs::write(temp_dir.path().join("main.rs"), "").unwrap();
        let non_utf8_name = OsStr::from_bytes(b"bad-\xFF-file");
        if std::fs::write(temp_dir.path().join(non_utf8_name), "").is_err() {
            // Some Unix filesystems (notably on macOS) reject invalid UTF-8 names at creation time.
            return;
        }

        let verifier = Verifier::new(temp_dir.path());

        let guide = NavigationGuide {
            items: vec![
                NavigationGuideLine {
                    line_number: 1,
                    indent_level: 0,
                    item: FilesystemItem::File {
                        path: "main.rs".to_string(),
                        comment: None,
                    },
                },
                NavigationGuideLine {
                    line_number: 2,
                    indent_level: 0,
                    item: FilesystemItem::Placeholder { comment: None },
                },
            ],
            prologue: None,
            epilogue: None,
            ignore: false,
        };

        let error = verifier
            .verify(&guide)
            .expect_err("placeholder enumeration must reject a non-UTF-8 name");
        assert!(matches!(
            &error,
            crate::errors::AppError::Semantic(SemanticError::NonUtf8Path { .. })
        ));
        let diagnostic = error.to_string();
        assert!(
            diagnostic.contains("\"\\x62\\x61\\x64\\x2D\\xFF\\x2D\\x66\\x69\\x6C\\x65\""),
            "diagnostic did not preserve every raw byte: {diagnostic}"
        );
        assert!(
            !diagnostic.contains('\u{fffd}'),
            "diagnostic used a lossy replacement character: {diagnostic}"
        );
    }

    #[test]
    fn issue_50_flat_path_mentions_its_first_component() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::create_dir(temp_dir.path().join("src")).unwrap();
        std::fs::write(temp_dir.path().join("src/main.rs"), "").unwrap();

        let guide = crate::parser::Parser::new()
            .parse(
                "<agentic-navigation-guide>\n\
                 - src/main.rs\n\
                 - ...\n\
                 </agentic-navigation-guide>",
            )
            .unwrap();
        let result = Verifier::new(temp_dir.path()).verify(&guide);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::PlaceholderNoUnmentionedItems { line: 3, .. }
            ))
        ));
    }

    #[test]
    fn issue_50_repeated_placeholders_enumerate_the_parent_once() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("main.rs"), "").unwrap();

        let guide = crate::parser::Parser::new()
            .parse(
                "<agentic-navigation-guide>\n\
                 - ... # before\n\
                 - main.rs\n\
                 - ... # after\n\
                 </agentic-navigation-guide>",
            )
            .unwrap();

        reset_directory_enumeration_counts();
        Verifier::new(temp_dir.path()).verify(&guide).unwrap();
        let canonical_root = std::fs::canonicalize(temp_dir.path()).unwrap();

        assert_eq!(
            directory_enumeration_counts(),
            std::collections::BTreeMap::from([(canonical_root, 1)]),
            "listed lookup, type classification, and every placeholder must share one snapshot"
        );
    }

    #[test]
    fn issue_50_root_and_nested_parents_are_each_enumerated_once() {
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("main.rs"), "").unwrap();

        let guide = crate::parser::Parser::new()
            .parse(
                "<agentic-navigation-guide>\n- src/\n  - ... # before\n  - main.rs\n  - ... # after\n</agentic-navigation-guide>",
            )
            .unwrap();

        reset_directory_enumeration_counts();
        Verifier::new(temp_dir.path()).verify(&guide).unwrap();
        let canonical_root = std::fs::canonicalize(temp_dir.path()).unwrap();
        let canonical_src = std::fs::canonicalize(&src).unwrap();

        assert_eq!(
            directory_enumeration_counts(),
            std::collections::BTreeMap::from([(canonical_root, 1), (canonical_src, 1),]),
            "each visited parent must have exactly one per-verification snapshot"
        );
    }

    #[test]
    fn issue_50_flat_siblings_share_each_component_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("main.rs"), "").unwrap();
        std::fs::write(src.join("lib.rs"), "").unwrap();

        let guide = crate::parser::Parser::new()
            .parse(
                "<agentic-navigation-guide>\n- src/main.rs\n- src/lib.rs\n</agentic-navigation-guide>",
            )
            .unwrap();

        reset_directory_enumeration_counts();
        Verifier::new(temp_dir.path()).verify(&guide).unwrap();
        let canonical_root = std::fs::canonicalize(temp_dir.path()).unwrap();
        let canonical_src = std::fs::canonicalize(&src).unwrap();

        assert_eq!(
            directory_enumeration_counts(),
            std::collections::BTreeMap::from([(canonical_root, 1), (canonical_src, 1),]),
            "flat siblings must reuse every shared parent snapshot"
        );
    }

    #[test]
    fn issue_50_ambiguous_exact_snapshot_names_fail_closed() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("actual.txt");
        std::fs::write(&path, "").unwrap();
        let observations = vec![
            (OsString::from("duplicate.txt"), path.clone()),
            (OsString::from("duplicate.txt"), path),
        ];

        let error = VerificationRun::<NoopVerificationControl>::build_snapshot_from_observations(
            17,
            true,
            observations,
        )
        .expect_err("duplicate exact observations must be ambiguous");
        assert_eq!(
            error.to_string(),
            "line 17: ambiguous duplicate exact filesystem name \"duplicate.txt\""
        );
    }

    #[test]
    fn issue_50_snapshot_name_diagnostics_are_order_independent() {
        let temp_dir = TempDir::new().unwrap();
        let first = vec![
            (
                OsString::from("bad\nname"),
                temp_dir.path().join("bad\nname"),
            ),
            (
                OsString::from("bad\tname"),
                temp_dir.path().join("bad\tname"),
            ),
        ];
        let mut reversed = first.clone();
        reversed.reverse();

        let first_error =
            VerificationRun::<NoopVerificationControl>::build_snapshot_from_observations(
                23, true, first,
            )
            .expect_err("control-bearing snapshot must fail");
        let reversed_error =
            VerificationRun::<NoopVerificationControl>::build_snapshot_from_observations(
                23, true, reversed,
            )
            .expect_err("reversed control-bearing snapshot must fail identically");

        assert_eq!(first_error.to_string(), reversed_error.to_string());
        assert_eq!(
            first_error.to_string(),
            "line 23: unsupported control-bearing filesystem name \"bad\\tname\""
        );
    }

    type MutationHook = Box<dyn FnMut(&Path) -> io::Result<()>>;

    struct InjectedMutationControl {
        stage: VerificationCheckpoint,
        target: PathBuf,
        fired: Rc<std::cell::Cell<bool>>,
        mutation: MutationHook,
    }

    impl VerificationControl for InjectedMutationControl {
        fn checkpoint(&mut self, stage: VerificationCheckpoint, path: &Path) -> io::Result<()> {
            if self.fired.get() || stage != self.stage || path != self.target {
                return Ok(());
            }
            self.fired.set(true);
            (self.mutation)(path)
        }
    }

    #[derive(Clone, Copy)]
    enum ReplacementKind {
        RegularFile,
        Directory,
        Disappear,
    }

    fn replace_observed_entry(path: &Path, replacement: ReplacementKind) -> io::Result<()> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("entry");
        let tombstone = path.with_file_name(format!(".issue51-observed-{name}"));
        std::fs::rename(path, tombstone)?;
        match replacement {
            ReplacementKind::RegularFile => std::fs::write(path, b"replacement"),
            ReplacementKind::Directory => std::fs::create_dir(path),
            ReplacementKind::Disappear => Ok(()),
        }
    }

    fn verify_with_injected_mutation(
        root: &Path,
        source: &str,
        stage: VerificationCheckpoint,
        target: PathBuf,
        mutation: MutationHook,
    ) -> (Result<()>, Rc<std::cell::Cell<bool>>) {
        let guide = crate::parser::Parser::new()
            .parse(&format!(
                "<agentic-navigation-guide>\n{source}</agentic-navigation-guide>"
            ))
            .expect("issue #51 mutation guide must parse");
        let target = std::fs::canonicalize(target)
            .expect("the deterministic mutation target must exist before verification");
        let fired = Rc::new(std::cell::Cell::new(false));
        let result = Verifier::new(root).verify_with_control(
            &guide,
            InjectedMutationControl {
                stage,
                target,
                fired: Rc::clone(&fired),
                mutation,
            },
        );
        (result, fired)
    }

    fn assert_observed_mutation_rejected(result: Result<()>, fired: &std::cell::Cell<bool>) {
        assert!(fired.get(), "the deterministic mutation hook did not run");
        let error = result.expect_err("an observed filesystem mutation must fail closed");
        assert!(
            matches!(error, AppError::Other(_)),
            "observed changes must use the private error surface: {error:?}"
        );
        let diagnostic = error.to_string();
        assert!(
            diagnostic.contains("changed identity or type")
                || diagnostic.contains("disappeared during verification"),
            "unexpected observed-change diagnostic: {diagnostic}"
        );
    }

    #[test]
    fn issue_51_observed_item_identity_and_type_changes_fail_closed() {
        for (initial, replacement) in [
            (ReplacementKind::RegularFile, ReplacementKind::RegularFile),
            (ReplacementKind::RegularFile, ReplacementKind::Directory),
            (ReplacementKind::Directory, ReplacementKind::RegularFile),
            (ReplacementKind::Directory, ReplacementKind::Directory),
            (ReplacementKind::RegularFile, ReplacementKind::Disappear),
        ] {
            let temp = TempDir::new().expect("temporary observed-mutation root");
            let victim = temp.path().join("victim");
            let source = match initial {
                ReplacementKind::RegularFile => {
                    std::fs::write(&victim, b"original").expect("original regular file");
                    "- victim\n"
                }
                ReplacementKind::Directory => {
                    std::fs::create_dir(&victim).expect("original directory");
                    "- victim/\n"
                }
                ReplacementKind::Disappear => unreachable!("disappearance is replacement-only"),
            };
            let (result, fired) = verify_with_injected_mutation(
                temp.path(),
                source,
                VerificationCheckpoint::AfterSnapshotEntrySelected,
                victim,
                Box::new(move |path| replace_observed_entry(path, replacement)),
            );
            assert_observed_mutation_rejected(result, &fired);
        }
    }

    #[test]
    fn issue_51_observed_parent_identity_change_during_enumeration_fails_closed() {
        let temp = TempDir::new().expect("temporary parent-mutation container");
        let root = temp.path().join("root");
        std::fs::create_dir(&root).expect("verification root");
        std::fs::write(root.join("inside.txt"), "").expect("listed fixture");

        let (result, fired) = verify_with_injected_mutation(
            &root,
            "- inside.txt\n",
            VerificationCheckpoint::AfterDirectoryEnumeration,
            root.clone(),
            Box::new(|path| replace_observed_entry(path, ReplacementKind::Directory)),
        );
        assert_observed_mutation_rejected(result, &fired);
    }

    #[cfg(unix)]
    fn replace_with_external_directory_link(path: &Path, external: &Path) -> io::Result<()> {
        let tombstone = path.with_file_name(".issue51-observed-ancestor");
        std::fs::rename(path, tombstone)?;
        std::os::unix::fs::symlink(external, path)
    }

    #[cfg(windows)]
    fn replace_with_external_directory_link(path: &Path, external: &Path) -> io::Result<()> {
        let tombstone = path.with_file_name(".issue51-observed-ancestor");
        std::fs::rename(path, tombstone)?;
        std::os::windows::fs::symlink_dir(external, path)
    }

    #[test]
    fn issue_51_observed_ancestor_replacement_cannot_satisfy_an_in_root_item() {
        const TARGET_SENTINEL: &str = "ISSUE51_OBSERVED_EXTERNAL_TARGET";

        let temp = TempDir::new().expect("temporary ancestor-mutation container");
        let root = temp.path().join("root");
        let ancestor = root.join("ancestor");
        let external = temp.path().join(TARGET_SENTINEL);
        std::fs::create_dir_all(&ancestor).expect("in-root ancestor");
        std::fs::create_dir(&external).expect("external directory");
        std::fs::write(ancestor.join("inside.txt"), "").expect("in-root file");
        std::fs::write(external.join("inside.txt"), "").expect("external file");

        let external_for_mutation = external.clone();
        let (result, fired) = verify_with_injected_mutation(
            &root,
            "- ancestor/inside.txt\n",
            VerificationCheckpoint::BeforeFinalRevalidation,
            ancestor,
            Box::new(move |path| {
                replace_with_external_directory_link(path, &external_for_mutation)
            }),
        );
        assert!(
            fired.get(),
            "the deterministic ancestor mutation did not run"
        );
        let error = result.expect_err("an observed external replacement must fail closed");
        let diagnostic = error.to_string();
        assert!(
            diagnostic.contains("changed identity or type"),
            "{diagnostic}"
        );
        assert!(
            !diagnostic.contains(TARGET_SENTINEL),
            "the observed-change diagnostic disclosed the external target: {diagnostic}"
        );
    }

    #[test]
    fn issue_51_path_escape_errors_do_not_retain_resolved_targets() {
        let item = NavigationGuideLine {
            line_number: 31,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "safe/logical.txt".to_string(),
                comment: None,
            },
        };
        let error = VerificationRun::<NoopVerificationControl>::path_escape_error(&item);

        match error {
            AppError::Semantic(SemanticError::PathEscapesRoot {
                line,
                path,
                root,
                resolved,
            }) => {
                assert_eq!(line, 31);
                assert_eq!(path, "safe/logical.txt");
                assert_eq!(root, Path::new("<redacted>"));
                assert_eq!(resolved, Path::new("<redacted>"));
            }
            other => panic!("unexpected containment error: {other:?}"),
        }
    }
}
