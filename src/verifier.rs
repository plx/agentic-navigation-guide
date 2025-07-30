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

        // Then verify each item against the filesystem
        for item in &guide.items {
            self.verify_item(item, &self.root_path)?;
        }

        Ok(())
    }

    /// Verify a single item against the filesystem
    fn verify_item(&self, item: &NavigationGuideLine, parent_path: &Path) -> Result<()> {
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
                for child in children {
                    self.verify_item(child, &item_path)?;
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
        }

        Ok(())
    }

    /// Get a human-readable string for the item type
    fn get_item_type_string(&self, item: &NavigationGuideLine) -> String {
        match &item.item {
            FilesystemItem::Directory { .. } => "directory".to_string(),
            FilesystemItem::File { .. } => "file".to_string(),
            FilesystemItem::Symlink { .. } => "symlink".to_string(),
        }
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
}
