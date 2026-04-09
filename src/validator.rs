//! Syntax validation for navigation guides

use crate::errors::{Result, SyntaxError};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

/// Validator for navigation guide syntax
pub struct Validator;

impl Validator {
    /// Create a new validator instance
    pub fn new() -> Self {
        Self
    }

    /// Validate the syntax of a navigation guide
    pub fn validate_syntax(&self, guide: &NavigationGuide) -> Result<()> {
        // Check for empty guide
        if guide.items.is_empty() {
            return Err(SyntaxError::EmptyGuideBlock.into());
        }

        // Validate each item
        for item in &guide.items {
            self.validate_item(item)?;
        }

        // Validate indentation consistency
        self.validate_indentation(&guide.items)?;

        // Check for duplicate entries within each scope
        self.validate_no_duplicates(&guide.items)?;

        Ok(())
    }

    /// Validate a single navigation guide item
    fn validate_item(&self, item: &NavigationGuideLine) -> Result<()> {
        match &item.item {
            FilesystemItem::Placeholder { .. } => {
                // Placeholders don't need path validation
                // They will have additional validation in validate_placeholders
            }
            _ => {
                // Validate path structure for non-placeholder items
                self.validate_path_structure(item)?;
            }
        }

        match &item.item {
            FilesystemItem::Directory { path, children, .. } => {
                // Directory paths should not contain the trailing slash in our internal representation
                // (it's stripped during parsing, but this is a double-check)
                if path.ends_with('/') {
                    return Err(SyntaxError::InvalidPathFormat {
                        line: item.line_number,
                        path: path.clone(),
                    }
                    .into());
                }

                // Validate children recursively
                for child in children {
                    self.validate_item(child)?;
                }

                // Check placeholder-specific rules for children
                self.validate_placeholder_rules(children)?;
            }
            FilesystemItem::File { path, .. } | FilesystemItem::Symlink { path, .. } => {
                // Files and symlinks should not end with slash
                if path.ends_with('/') {
                    return Err(SyntaxError::InvalidPathFormat {
                        line: item.line_number,
                        path: path.clone(),
                    }
                    .into());
                }
            }
            FilesystemItem::Placeholder { .. } => {
                // Placeholder-specific validation is done at the parent level
            }
        }

        Ok(())
    }

    /// Validate path structure without restricting character classes.
    fn validate_path_structure(&self, item: &NavigationGuideLine) -> Result<()> {
        let path = item.path();

        // Check for empty path
        if path.is_empty() {
            return Err(SyntaxError::InvalidPathFormat {
                line: item.line_number,
                path: path.to_string(),
            }
            .into());
        }

        let path_obj = Path::new(path);

        // Paths must be relative.
        if path_obj.is_absolute() {
            return Err(SyntaxError::InvalidPathFormat {
                line: item.line_number,
                path: path.to_string(),
            }
            .into());
        }

        // Validate raw separator-delimited components so we preserve `.` / `..` and empty components.
        // Treat both `/` and `\` as separators to make validation platform-agnostic.
        for component in path.split(['/', '\\']) {
            if component.is_empty() {
                return Err(SyntaxError::InvalidPathFormat {
                    line: item.line_number,
                    path: path.to_string(),
                }
                .into());
            }

            if component == "." || component == ".." {
                return Err(SyntaxError::InvalidSpecialDirectory {
                    line: item.line_number,
                    path: path.to_string(),
                }
                .into());
            }
        }

        // Reject rooted/prefixed paths and platform-native `.` / `..` components.
        for component in path_obj.components() {
            if matches!(component, Component::RootDir | Component::Prefix(_)) {
                return Err(SyntaxError::InvalidPathFormat {
                    line: item.line_number,
                    path: path.to_string(),
                }
                .into());
            }

            if matches!(component, Component::CurDir | Component::ParentDir) {
                return Err(SyntaxError::InvalidSpecialDirectory {
                    line: item.line_number,
                    path: path.to_string(),
                }
                .into());
            }
        }

        Ok(())
    }

    /// Validate indentation consistency across items
    fn validate_indentation(&self, items: &[NavigationGuideLine]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        // Collect all unique indent levels
        let mut indent_levels: HashSet<usize> = HashSet::new();
        self.collect_indent_levels(items, &mut indent_levels);

        // Check that all indentation levels are consistent
        // First, find the base indentation unit (smallest non-zero indent)
        let base_indent = indent_levels
            .iter()
            .filter(|&&level| level > 0)
            .min()
            .copied();

        if let Some(base) = base_indent {
            // All indent levels should be multiples of the base
            for &level in &indent_levels {
                if level > 0 && level % base != 0 {
                    // Find the first item with this indent level to report the error
                    if let Some(item) = self.find_item_with_indent(items, level) {
                        return Err(SyntaxError::InconsistentIndentation {
                            line: item.line_number,
                            expected: ((level / base) + 1) * base,
                            found: level,
                        }
                        .into());
                    }
                }
            }
        }

        // Validate proper nesting (no skipping levels)
        self.validate_nesting(items)?;

        // Validate placeholder rules at root level
        self.validate_placeholder_rules(items)?;

        Ok(())
    }

    /// Validate placeholder-specific rules
    fn validate_placeholder_rules(&self, items: &[NavigationGuideLine]) -> Result<()> {
        // Check that placeholders are not adjacent
        for i in 0..items.len() {
            if items[i].is_placeholder() {
                // Check if next item is also a placeholder
                if i + 1 < items.len() && items[i + 1].is_placeholder() {
                    return Err(SyntaxError::AdjacentPlaceholders {
                        line: items[i + 1].line_number,
                    }
                    .into());
                }

                // Placeholders cannot have children (this should be enforced by parser)
                if let Some(children) = items[i].children() {
                    if !children.is_empty() {
                        return Err(SyntaxError::PlaceholderWithChildren {
                            line: items[i].line_number,
                        }
                        .into());
                    }
                }
            }
        }

        Ok(())
    }

    /// Check for duplicate entries within the same scope (sibling level)
    fn validate_no_duplicates(&self, items: &[NavigationGuideLine]) -> Result<()> {
        let mut seen: HashMap<&str, usize> = HashMap::new();
        for item in items {
            if item.is_placeholder() {
                continue;
            }
            let path = item.path();
            if let Some(&first_line) = seen.get(path) {
                return Err(SyntaxError::DuplicateEntry {
                    line: item.line_number,
                    path: path.to_string(),
                    first_line,
                }
                .into());
            }
            seen.insert(path, item.line_number);

            // Recursively check children of directories
            if let Some(children) = item.children() {
                self.validate_no_duplicates(children)?;
            }
        }
        Ok(())
    }

    /// Collect all indent levels from items and their children
    fn collect_indent_levels(&self, items: &[NavigationGuideLine], levels: &mut HashSet<usize>) {
        for item in items {
            levels.insert(item.indent_level);
            if let Some(children) = item.children() {
                self.collect_indent_levels(children, levels);
            }
        }
    }

    /// Find the first item with the given indent level
    fn find_item_with_indent<'a>(
        &self,
        items: &'a [NavigationGuideLine],
        target_level: usize,
    ) -> Option<&'a NavigationGuideLine> {
        for item in items {
            if item.indent_level == target_level {
                return Some(item);
            }
            if let Some(children) = item.children() {
                if let Some(found) = self.find_item_with_indent(children, target_level) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Validate that indentation levels don't skip (e.g., 0 -> 2 without 1)
    fn validate_nesting(&self, items: &[NavigationGuideLine]) -> Result<()> {
        for item in items {
            if let Some(children) = item.children() {
                for child in children {
                    // Children should be exactly one level deeper than parent
                    if child.indent_level != item.indent_level + 1 {
                        return Err(SyntaxError::InvalidIndentationLevel {
                            line: child.line_number,
                        }
                        .into());
                    }
                }
                // Recursively validate this item's subtree once.
                self.validate_nesting(children)?;
            }
        }
        Ok(())
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_guide() {
        let guide = NavigationGuide::new();
        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::EmptyGuideBlock
            ))
        ));
    }

    #[test]
    fn test_validate_allows_utf8_and_unrestricted_characters() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "目录/résumé|draft(1).txt".to_string(),
                comment: None,
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_double_slashes() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "path//with//double//slashes.txt".to_string(),
                comment: None,
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidPathFormat { .. }
            ))
        ));
    }

    #[test]
    fn test_validate_rejects_dot_components() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "src/./main.rs".to_string(),
                comment: None,
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidSpecialDirectory { .. }
            ))
        ));
    }

    #[test]
    fn test_validate_rejects_parent_components() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "src/../main.rs".to_string(),
                comment: None,
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidSpecialDirectory { .. }
            ))
        ));
    }

    #[test]
    fn test_validate_rejects_dot_components_with_backslash_separator() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "src\\.\\main.rs".to_string(),
                comment: None,
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidSpecialDirectory { .. }
            ))
        ));
    }

    #[test]
    fn test_validate_rejects_parent_components_with_backslash_separator() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "src\\..\\main.rs".to_string(),
                comment: None,
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidSpecialDirectory { .. }
            ))
        ));
    }

    #[test]
    fn test_validate_adjacent_placeholders() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::Directory {
                path: "src".to_string(),
                comment: None,
                children: vec![
                    NavigationGuideLine {
                        line_number: 2,
                        indent_level: 1,
                        item: FilesystemItem::Placeholder {
                            comment: Some("first placeholder".to_string()),
                        },
                    },
                    NavigationGuideLine {
                        line_number: 3,
                        indent_level: 1,
                        item: FilesystemItem::Placeholder {
                            comment: Some("second placeholder".to_string()),
                        },
                    },
                ],
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::AdjacentPlaceholders { line: 3 }
            ))
        ));
    }

    #[test]
    fn test_validate_non_adjacent_placeholders() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::Directory {
                path: "src".to_string(),
                comment: None,
                children: vec![
                    NavigationGuideLine {
                        line_number: 2,
                        indent_level: 1,
                        item: FilesystemItem::Placeholder {
                            comment: Some("first placeholder".to_string()),
                        },
                    },
                    NavigationGuideLine {
                        line_number: 3,
                        indent_level: 1,
                        item: FilesystemItem::File {
                            path: "main.rs".to_string(),
                            comment: None,
                        },
                    },
                    NavigationGuideLine {
                        line_number: 4,
                        indent_level: 1,
                        item: FilesystemItem::Placeholder {
                            comment: Some("second placeholder".to_string()),
                        },
                    },
                ],
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(result.is_ok());
    }

    #[test]
    fn test_no_duplicates_passes() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "main.rs".to_string(),
                comment: None,
            },
        });
        guide.items.push(NavigationGuideLine {
            line_number: 2,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "lib.rs".to_string(),
                comment: None,
            },
        });

        let validator = Validator::new();
        assert!(validator.validate_syntax(&guide).is_ok());
    }

    #[test]
    fn test_duplicate_root_entries() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "main.rs".to_string(),
                comment: None,
            },
        });
        guide.items.push(NavigationGuideLine {
            line_number: 2,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "main.rs".to_string(),
                comment: Some("duplicate".to_string()),
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::DuplicateEntry {
                    line: 2,
                    first_line: 1,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn test_duplicate_nested_entries() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
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
                        item: FilesystemItem::File {
                            path: "main.rs".to_string(),
                            comment: None,
                        },
                    },
                ],
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::DuplicateEntry {
                    line: 3,
                    first_line: 2,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn test_same_name_different_scopes_is_not_duplicate() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "main.rs".to_string(),
                comment: None,
            },
        });
        guide.items.push(NavigationGuideLine {
            line_number: 2,
            indent_level: 0,
            item: FilesystemItem::Directory {
                path: "src".to_string(),
                comment: None,
                children: vec![NavigationGuideLine {
                    line_number: 3,
                    indent_level: 1,
                    item: FilesystemItem::File {
                        path: "main.rs".to_string(),
                        comment: None,
                    },
                }],
            },
        });

        let validator = Validator::new();
        assert!(validator.validate_syntax(&guide).is_ok());
    }

    #[test]
    fn test_duplicate_directory_entries() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::Directory {
                path: "src".to_string(),
                comment: None,
                children: vec![],
            },
        });
        guide.items.push(NavigationGuideLine {
            line_number: 2,
            indent_level: 0,
            item: FilesystemItem::Directory {
                path: "src".to_string(),
                comment: None,
                children: vec![],
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::DuplicateEntry {
                    line: 2,
                    first_line: 1,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn test_placeholders_are_not_considered_duplicates() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "main.rs".to_string(),
                comment: None,
            },
        });
        guide.items.push(NavigationGuideLine {
            line_number: 2,
            indent_level: 0,
            item: FilesystemItem::Placeholder {
                comment: Some("other files".to_string()),
            },
        });
        guide.items.push(NavigationGuideLine {
            line_number: 3,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "lib.rs".to_string(),
                comment: None,
            },
        });
        guide.items.push(NavigationGuideLine {
            line_number: 4,
            indent_level: 0,
            item: FilesystemItem::Placeholder {
                comment: Some("more files".to_string()),
            },
        });

        let validator = Validator::new();
        // Placeholders would fail adjacency check normally, but here they're
        // separated by a file. This test verifies placeholders are skipped
        // by duplicate detection.
        assert!(validator.validate_syntax(&guide).is_ok());
    }

    #[test]
    fn test_duplicate_file_vs_directory_same_name() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "src".to_string(),
                comment: None,
            },
        });
        guide.items.push(NavigationGuideLine {
            line_number: 2,
            indent_level: 0,
            item: FilesystemItem::Directory {
                path: "src".to_string(),
                comment: None,
                children: vec![],
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::DuplicateEntry {
                    line: 2,
                    first_line: 1,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn test_duplicate_entry_reports_correct_path() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "lib.rs".to_string(),
                comment: None,
            },
        });
        guide.items.push(NavigationGuideLine {
            line_number: 2,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "lib.rs".to_string(),
                comment: None,
            },
        });

        let validator = Validator::new();
        let result = validator.validate_syntax(&guide);
        match result {
            Err(crate::errors::AppError::Syntax(SyntaxError::DuplicateEntry {
                line,
                path,
                first_line,
            })) => {
                assert_eq!(line, 2);
                assert_eq!(first_line, 1);
                assert_eq!(path, "lib.rs");
            }
            other => panic!("expected DuplicateEntry, got: {other:?}"),
        }
    }
}
