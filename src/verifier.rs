//! Filesystem verification for navigation guides

use crate::entry_type::{classify_path, SupportedEntryKind};
use crate::errors::{AppError, Result, SemanticError};
use crate::path_codec::{
    contains_forbidden_control, has_windows_drive_prefix, render_os_component,
    render_utf8_component,
};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
            std::cell::RefCell::new(std::collections::BTreeMap::new());
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

        // Collect mentioned root-level names for placeholder verification
        let mut mentioned_names = std::collections::HashSet::new();
        for item in &guide.items {
            if !item.is_placeholder() {
                mentioned_names.insert(item.path().to_string());
            }
        }

        // Then verify each item against the filesystem
        for item in &guide.items {
            if item.is_placeholder() {
                self.verify_placeholder_with_context(
                    item,
                    &self.root_path,
                    &mentioned_names,
                    true,
                )?;
            } else {
                self.verify_item(item, &self.root_path, &canonical_root_path)?;
            }
        }

        Ok(())
    }

    /// Verify a single item against the filesystem
    fn verify_item(
        &self,
        item: &NavigationGuideLine,
        parent_path: &Path,
        canonical_root_path: &Path,
    ) -> Result<()> {
        // Handle placeholders specially
        if item.is_placeholder() {
            return self.verify_placeholder(item, parent_path);
        }

        let item_path = parent_path.join(item.path());
        // The textual format never grants authority to follow its final
        // component. Resolve only the parent for containment, then classify
        // the final entry with non-following metadata below.
        let resolved_path =
            self.resolve_for_containment(&item_path, false, item.line_number, item.path())?;

        if !resolved_path.starts_with(canonical_root_path) {
            return Err(SemanticError::PathEscapesRoot {
                line: item.line_number,
                path: item.path().to_string(),
                root: canonical_root_path.to_path_buf(),
                resolved: resolved_path,
            }
            .into());
        }

        // Preserve the legacy programmatic Symlink variant until its #53
        // removal. Its existing dangling-link behavior is deliberately not
        // part of textual file/directory classification.
        if matches!(&item.item, FilesystemItem::Symlink { .. }) && !item_path.exists() {
            return Err(SemanticError::ItemNotFound {
                line: item.line_number,
                item_type: self.get_item_type_string(item),
                path: item.path().to_string(),
                full_path: item_path,
            }
            .into());
        }

        // Check if the item type matches
        match &item.item {
            FilesystemItem::Directory { children, .. } => {
                self.require_textual_entry_kind(item, &item_path, SupportedEntryKind::Directory)?;

                // Verify children recursively
                let mut mentioned_names = std::collections::HashSet::new();
                for child in children {
                    if !child.is_placeholder() {
                        mentioned_names.insert(child.path().to_string());
                    }
                }

                for child in children {
                    if child.is_placeholder() {
                        self.verify_placeholder_with_context(
                            child,
                            &item_path,
                            &mentioned_names,
                            false,
                        )?;
                    } else {
                        self.verify_item(child, &item_path, canonical_root_path)?;
                    }
                }
            }
            FilesystemItem::File { .. } => {
                self.require_textual_entry_kind(item, &item_path, SupportedEntryKind::RegularFile)?;
            }
            FilesystemItem::Symlink { target, .. } => {
                let metadata = match std::fs::symlink_metadata(&item_path) {
                    Ok(m) => m,
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        return Err(SemanticError::PermissionDenied {
                            line: item.line_number,
                            path: item.path().to_string(),
                        }
                        .into());
                    }
                    Err(e) => return Err(e.into()),
                };

                if !metadata.is_symlink() {
                    return Err(SemanticError::TypeMismatch {
                        line: item.line_number,
                        expected: "symlink".to_string(),
                        found: if item_path.is_dir() {
                            "directory".to_string()
                        } else {
                            "file".to_string()
                        },
                        path: item.path().to_string(),
                    }
                    .into());
                }

                // Verify symlink target if specified
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
                // This case should never be reached because we handle placeholders
                // at the beginning of the function, but we need it for exhaustiveness
                unreachable!("Placeholder should have been handled earlier");
            }
        }

        Ok(())
    }

    fn require_textual_entry_kind(
        &self,
        item: &NavigationGuideLine,
        item_path: &Path,
        expected: SupportedEntryKind,
    ) -> Result<()> {
        let classification = match classify_path(item_path) {
            Ok(classification) => classification,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(SemanticError::ItemNotFound {
                    line: item.line_number,
                    item_type: self.get_item_type_string(item),
                    path: item.path().to_string(),
                    full_path: item_path.to_path_buf(),
                }
                .into());
            }
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(SemanticError::PermissionDenied {
                    line: item.line_number,
                    path: item.path().to_string(),
                }
                .into());
            }
            Err(error) => return Err(error.into()),
        };

        let found = match classification {
            Ok(actual) if actual == expected => return Ok(()),
            Ok(SupportedEntryKind::RegularFile) => "file".to_string(),
            Ok(SupportedEntryKind::Directory) => "directory".to_string(),
            Err(unsupported) => unsupported.to_string(),
        };
        let expected = match expected {
            SupportedEntryKind::RegularFile => "file",
            SupportedEntryKind::Directory => "directory",
        };

        Err(SemanticError::TypeMismatch {
            line: item.line_number,
            expected: expected.to_string(),
            found,
            path: item.path().to_string(),
        }
        .into())
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

    /// Verify a placeholder at the root level
    fn verify_placeholder(&self, item: &NavigationGuideLine, parent_path: &Path) -> Result<()> {
        // For root-level placeholders, we need to check that there's at least one item
        // in the parent directory that isn't mentioned in the guide
        let mentioned_names = std::collections::HashSet::new();
        self.verify_placeholder_with_context(
            item,
            parent_path,
            &mentioned_names,
            parent_path == self.root_path,
        )
    }

    /// Verify a placeholder with context of mentioned sibling items
    fn verify_placeholder_with_context(
        &self,
        item: &NavigationGuideLine,
        parent_path: &Path,
        mentioned_names: &std::collections::HashSet<String>,
        at_root: bool,
    ) -> Result<()> {
        // Check that the parent directory has at least one item not in mentioned_names
        let entries = match read_directory(parent_path) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(SemanticError::PermissionDenied {
                    line: item.line_number,
                    path: parent_path.to_string_lossy().to_string(),
                }
                .into());
            }
            Err(e) => return Err(e.into()),
        };

        // Count unmentioned items
        let mut unmentioned_count = 0;
        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(SemanticError::PermissionDenied {
                        line: item.line_number,
                        path: parent_path.to_string_lossy().to_string(),
                    }
                    .into());
                }
                Err(e) => return Err(e.into()),
            };

            let name = entry.file_name();
            let name = name.to_str().ok_or_else(|| SemanticError::NonUtf8Path {
                line: item.line_number,
                path: PathBuf::from(render_os_component(&name)),
            })?;
            if contains_forbidden_control(name) {
                return Err(AppError::Other(format!(
                    "line {}: unsupported control-bearing filesystem name {}",
                    item.line_number,
                    render_utf8_component(name)
                )));
            }
            if at_root && (name.starts_with('\\') || has_windows_drive_prefix(name)) {
                return Err(AppError::Other(format!(
                    "line {}: unsupported rooted or drive-prefixed filesystem name {}",
                    item.line_number,
                    render_utf8_component(name)
                )));
            }

            if !mentioned_names.contains(name) {
                unmentioned_count += 1;
            }
        }

        if unmentioned_count == 0 {
            // Only require unmentioned items if the placeholder has no comment.
            // Placeholders with comments can represent future items that don't yet exist.
            if !Self::placeholder_has_meaningful_comment(item) {
                return Err(SemanticError::PlaceholderNoUnmentionedItems {
                    line: item.line_number,
                    parent: parent_path.to_string_lossy().to_string(),
                }
                .into());
            }
            // Placeholders with comments are allowed even without unmentioned items
        }

        Ok(())
    }

    fn placeholder_has_meaningful_comment(item: &NavigationGuideLine) -> bool {
        item.comment()
            .map(|comment| !comment.trim().is_empty())
            .unwrap_or(false)
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
}
