//! Filesystem verification for navigation guides

use crate::entry_type::{
    classify_path, EntryClassification, SupportedEntryKind, UnsupportedEntryKind,
};
use crate::errors::{AppError, Result, SemanticError};
use crate::path_codec::{
    contains_forbidden_control, has_windows_drive_prefix, render_os_component,
    render_utf8_component,
};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
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

#[derive(Debug)]
struct SnapshotEntry {
    path: PathBuf,
    classification: EntryClassification,
}

#[derive(Debug)]
struct DirectorySnapshot {
    entries: BTreeMap<String, SnapshotEntry>,
}

struct VerificationRun<'a> {
    verifier: &'a Verifier,
    canonical_root_path: PathBuf,
    snapshots: HashMap<PathBuf, Rc<DirectorySnapshot>>,
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
        // First validate syntax (should already be done, but double-check)
        crate::validator::Validator::new().validate_syntax(guide)?;
        let canonical_root_path = self.canonicalize_root_path()?;

        VerificationRun::new(self, canonical_root_path).verify_siblings(
            &guide.items,
            &self.root_path,
            true,
        )
    }

    /// Canonicalize root path once before verification
    fn canonicalize_root_path(&self) -> Result<PathBuf> {
        Ok(std::fs::canonicalize(&self.root_path)?)
    }

    /// Resolve candidate path for containment checks.
    ///
    /// When `follow_final_component` is false, containment checks are performed on the path
    /// itself without resolving the final component as a symlink target.
    fn resolve_for_containment(
        &self,
        candidate_path: &Path,
        follow_final_component: bool,
        line: usize,
        guide_path: &str,
    ) -> Result<PathBuf> {
        let mut missing_suffix: Vec<OsString> = Vec::new();

        let mut probe_path = if follow_final_component {
            candidate_path.to_path_buf()
        } else {
            let file_name = candidate_path
                .file_name()
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("path '{}' has no terminal component", guide_path),
                    )
                })?
                .to_os_string();
            let parent = candidate_path.parent().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("path '{}' has no parent component", guide_path),
                )
            })?;

            missing_suffix.push(file_name);
            parent.to_path_buf()
        };

        loop {
            match std::fs::symlink_metadata(&probe_path) {
                Ok(_) => break,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let missing_component = match probe_path.file_name() {
                        Some(component) => component.to_os_string(),
                        None => return Err(e.into()),
                    };
                    missing_suffix.push(missing_component);
                    probe_path = match probe_path.parent() {
                        Some(parent) if parent != probe_path => parent.to_path_buf(),
                        None => return Err(e.into()),
                        _ => return Err(e.into()),
                    };
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(SemanticError::PermissionDenied {
                        line,
                        path: guide_path.to_string(),
                    }
                    .into());
                }
                Err(e) => return Err(e.into()),
            }
        }

        let mut resolved_path = match std::fs::canonicalize(&probe_path) {
            Ok(path) => path,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(SemanticError::PermissionDenied {
                    line,
                    path: guide_path.to_string(),
                }
                .into());
            }
            Err(e) => return Err(e.into()),
        };

        for component in missing_suffix.iter().rev() {
            resolved_path.push(component);
        }

        Ok(resolved_path)
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

impl<'a> VerificationRun<'a> {
    fn new(verifier: &'a Verifier, canonical_root_path: PathBuf) -> Self {
        Self {
            verifier,
            canonical_root_path,
            snapshots: HashMap::new(),
        }
    }

    fn verify_siblings(
        &mut self,
        items: &[NavigationGuideLine],
        parent_path: &Path,
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
                        parent: parent_path.to_string_lossy().to_string(),
                    }
                    .into());
                }
            } else {
                self.verify_item(item, parent_path, at_root)?;
            }
        }

        Ok(())
    }

    fn verify_item(
        &mut self,
        item: &NavigationGuideLine,
        parent_path: &Path,
        at_root: bool,
    ) -> Result<()> {
        let candidate_path = parent_path.join(item.path());
        // Preserve the existing containment boundary check. Filesystem
        // identity and type decisions below come only from exact snapshots;
        // this resolution is solely the separate containment policy.
        let resolved_path = self.verifier.resolve_for_containment(
            &candidate_path,
            false,
            item.line_number,
            item.path(),
        )?;
        if !resolved_path.starts_with(&self.canonical_root_path) {
            return Err(SemanticError::PathEscapesRoot {
                line: item.line_number,
                path: item.path().to_string(),
                root: self.canonical_root_path.clone(),
                resolved: resolved_path,
            }
            .into());
        }

        let (item_path, classification) =
            self.resolve_exact_item_path(item, parent_path, at_root)?;

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
                self.verify_siblings(children, &item_path, false)?;
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

        Ok(())
    }

    fn resolve_exact_item_path(
        &mut self,
        item: &NavigationGuideLine,
        parent_path: &Path,
        at_root: bool,
    ) -> Result<(PathBuf, EntryClassification)> {
        let components = item.path().split('/').collect::<Vec<_>>();
        let full_path = parent_path.join(item.path());
        let mut current_parent = parent_path.to_path_buf();
        let mut current_at_root = at_root;

        for (index, component) in components.iter().enumerate() {
            let snapshot = self.snapshot(&current_parent, item.line_number, current_at_root)?;
            let exact_entry = snapshot
                .entries
                .get(*component)
                .map(|entry| (entry.path.clone(), entry.classification));
            let Some((entry_path, classification)) = exact_entry else {
                return self.missing_exact_component(item, &current_parent, component, &full_path);
            };

            if index + 1 == components.len() {
                return Ok((entry_path, classification));
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

            current_parent = entry_path;
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
    ) -> Result<(PathBuf, EntryClassification)> {
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
        if let Some(snapshot) = self.snapshots.get(parent_path) {
            return Ok(Rc::clone(snapshot));
        }

        let snapshot = Rc::new(Self::build_snapshot(parent_path, line, at_root)?);
        self.snapshots
            .insert(parent_path.to_path_buf(), Rc::clone(&snapshot));
        Ok(snapshot)
    }

    fn build_snapshot(parent_path: &Path, line: usize, at_root: bool) -> Result<DirectorySnapshot> {
        let entries = match read_directory(parent_path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(SemanticError::PermissionDenied {
                    line,
                    path: parent_path.to_string_lossy().to_string(),
                }
                .into());
            }
            Err(error) => return Err(error.into()),
        };

        let mut observed_entries = Vec::new();
        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(SemanticError::PermissionDenied {
                        line,
                        path: parent_path.to_string_lossy().to_string(),
                    }
                    .into());
                }
                Err(error) => return Err(error.into()),
            };
            observed_entries.push((entry.file_name(), entry.path()));
        }

        Self::build_snapshot_from_observations(line, at_root, observed_entries)
    }

    fn build_snapshot_from_observations(
        line: usize,
        at_root: bool,
        mut observed_entries: Vec<(OsString, PathBuf)>,
    ) -> Result<DirectorySnapshot> {
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

            let classification = match classify_path(&path) {
                Ok(classification) => classification,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
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
            let previous = snapshot_entries.insert(
                utf8_name.to_string(),
                SnapshotEntry {
                    path,
                    classification,
                },
            );
            if previous.is_some() {
                return Err(AppError::Other(format!(
                    "line {line}: ambiguous duplicate exact filesystem name {}",
                    render_utf8_component(utf8_name)
                )));
            }
        }

        Ok(DirectorySnapshot {
            entries: snapshot_entries,
        })
    }
}

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
                SemanticError::PathEscapesRoot { .. }
            ))
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
        assert!(matches!(result, Err(crate::errors::AppError::Io(_))));
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

        assert_eq!(
            directory_enumeration_counts(),
            std::collections::BTreeMap::from([(temp_dir.path().to_path_buf(), 1)]),
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

        assert_eq!(
            directory_enumeration_counts(),
            std::collections::BTreeMap::from([(temp_dir.path().to_path_buf(), 1), (src, 1),]),
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

        assert_eq!(
            directory_enumeration_counts(),
            std::collections::BTreeMap::from([(temp_dir.path().to_path_buf(), 1), (src, 1),]),
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

        let error = VerificationRun::build_snapshot_from_observations(17, true, observations)
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

        let first_error = VerificationRun::build_snapshot_from_observations(23, true, first)
            .expect_err("control-bearing snapshot must fail");
        let reversed_error = VerificationRun::build_snapshot_from_observations(23, true, reversed)
            .expect_err("reversed control-bearing snapshot must fail identically");

        assert_eq!(first_error.to_string(), reversed_error.to_string());
        assert_eq!(
            first_error.to_string(),
            "line 23: unsupported control-bearing filesystem name \"bad\\tname\""
        );
    }
}
