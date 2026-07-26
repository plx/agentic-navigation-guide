//! Syntax validation for navigation guides

use crate::errors::{Result, SyntaxError};
use crate::path_codec::{
    contains_forbidden_control, has_windows_drive_prefix, render_utf8_component,
};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use std::collections::HashSet;

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
            self.validate_item(item, true)?;
        }

        // Validate indentation consistency
        self.validate_indentation(&guide.items)?;

        // Reject duplicate decoded file/directory identities, including paths
        // reached through different hierarchy spellings. Placeholders are
        // structural wildcards rather than filesystem identities.
        let mut full_paths = HashSet::new();
        Self::validate_unique_full_paths(&guide.items, "", &mut full_paths)?;

        Ok(())
    }

    /// Validate a single navigation guide item
    fn validate_item(&self, item: &NavigationGuideLine, at_root: bool) -> Result<()> {
        match &item.item {
            FilesystemItem::Placeholder { .. } => {
                // Placeholders don't need path validation
                // They will have additional validation in validate_placeholders
            }
            _ => {
                // Validate path structure for non-placeholder items
                self.validate_path_structure(item, at_root)?;
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
                    self.validate_item(child, false)?;
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
    fn validate_path_structure(&self, item: &NavigationGuideLine, at_root: bool) -> Result<()> {
        let path = item.path();

        // Check for empty path
        if path.is_empty() {
            return Err(SyntaxError::InvalidPathFormat {
                line: item.line_number,
                path: path.to_string(),
            }
            .into());
        }

        // `/` is the sole logical separator, but a leading backslash is still
        // a rooted Windows spelling and rejects on every host.
        if path.starts_with('/') || (at_root && path.starts_with('\\')) {
            return Err(SyntaxError::InvalidPathFormat {
                line: item.line_number,
                path: path.to_string(),
            }
            .into());
        }

        // Validate raw slash-delimited components before any normalization so
        // `.` / `..` and empty components remain observable.
        for (index, component) in path.split('/').enumerate() {
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

            if contains_forbidden_control(component) {
                return Err(SyntaxError::InvalidPathFormat {
                    line: item.line_number,
                    path: render_utf8_component(path),
                }
                .into());
            }

            if at_root && index == 0 && has_windows_drive_prefix(component) {
                return Err(SyntaxError::InvalidPathFormat {
                    line: item.line_number,
                    path: path.to_string(),
                }
                .into());
            }
        }

        Ok(())
    }

    fn validate_unique_full_paths(
        items: &[NavigationGuideLine],
        parent: &str,
        full_paths: &mut HashSet<String>,
    ) -> Result<()> {
        for item in items {
            if item.is_placeholder() {
                continue;
            }

            let full_path = if parent.is_empty() {
                item.path().to_string()
            } else {
                format!("{parent}/{}", item.path())
            };
            if !full_paths.insert(full_path.clone()) {
                return Err(SyntaxError::InvalidPathFormat {
                    line: item.line_number,
                    path: full_path,
                }
                .into());
            }

            if let Some(children) = item.children() {
                Self::validate_unique_full_paths(children, &full_path, full_paths)?;
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
    fn test_validate_allows_dot_text_with_literal_backslashes() {
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
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_allows_parent_text_with_literal_backslashes() {
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
        assert!(result.is_ok());
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
}
