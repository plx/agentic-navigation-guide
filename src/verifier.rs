//! Filesystem verification for navigation guides

use crate::errors::{Result, SemanticError};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use std::path::{Path, PathBuf};

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
                self.verify_placeholder_with_context(item, &self.root_path, &mentioned_names)?;
            } else {
                self.verify_item(item, &self.root_path)?;
            }
        }

        Ok(())
    }

    /// Verify a single item against the filesystem
    fn verify_item(&self, item: &NavigationGuideLine, parent_path: &Path) -> Result<()> {
        // Handle placeholders specially
        if item.is_placeholder() {
            return self.verify_placeholder(item, parent_path);
        }

        let item_path = parent_path.join(item.path());

        // Check if the item exists
        if !item_path.exists() {
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
                if !item_path.is_dir() {
                    return Err(SemanticError::TypeMismatch {
                        line: item.line_number,
                        expected: "directory".to_string(),
                        found: if item_path.is_file() {
                            "file".to_string()
                        } else {
                            "symlink".to_string()
                        },
                        path: item.path().to_string(),
                    }
                    .into());
                }

                // Verify children recursively
                let mut mentioned_names = std::collections::HashSet::new();
                for child in children {
                    if !child.is_placeholder() {
                        mentioned_names.insert(child.path().to_string());
                    }
                }

                for child in children {
                    if child.is_placeholder() {
                        self.verify_placeholder_with_context(child, &item_path, &mentioned_names)?;
                    } else {
                        self.verify_item(child, &item_path)?;
                    }
                }
            }
            FilesystemItem::File { .. } => {
                if !item_path.is_file() {
                    return Err(SemanticError::TypeMismatch {
                        line: item.line_number,
                        expected: "file".to_string(),
                        found: if item_path.is_dir() {
                            "directory".to_string()
                        } else {
                            "symlink".to_string()
                        },
                        path: item.path().to_string(),
                    }
                    .into());
                }
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
        self.verify_placeholder_with_context(item, parent_path, &mentioned_names)
    }

    /// Verify a placeholder with context of mentioned sibling items
    fn verify_placeholder_with_context(
        &self,
        item: &NavigationGuideLine,
        parent_path: &Path,
        mentioned_names: &std::collections::HashSet<String>,
    ) -> Result<()> {
        // Check that the parent directory has at least one item not in mentioned_names
        let entries = match std::fs::read_dir(parent_path) {
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
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if !mentioned_names.contains(name) {
                    unmentioned_count += 1;
                }
            }
        }

        if unmentioned_count == 0 {
            return Err(SemanticError::PlaceholderNoUnmentionedItems {
                line: item.line_number,
                parent: parent_path.to_string_lossy().to_string(),
            }
            .into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        };

        // Should succeed because lib.rs and mod.rs are unmentioned
        let result = verifier.verify(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_placeholder_without_unmentioned_items() {
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
                        comment: Some("other files".to_string()),
                    },
                },
            ],
            prologue: None,
            epilogue: None,
        };

        // Should fail because all items are mentioned
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
        };

        // Should fail because directory is empty
        let result = verifier.verify(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Semantic(
                SemanticError::PlaceholderNoUnmentionedItems { .. }
            ))
        ));
    }
}
