//! Syntax validation for navigation guides

use crate::errors::{Result, SyntaxError};
use crate::path_codec::render_utf8_component;
use crate::proven::path_rules::{self, PathVerdict};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use std::collections::HashSet;

/// Validator for navigation guide syntax
pub(crate) struct Validator;

impl Validator {
    /// Create a new validator instance
    pub(crate) fn new() -> Self {
        Self
    }

    /// Validate the syntax of a navigation guide
    pub(crate) fn validate_syntax(&self, guide: &NavigationGuide) -> Result<()> {
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
                        path: render_utf8_component(path),
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
            FilesystemItem::File { path, .. } => {
                // File paths should not end with slash
                if path.ends_with('/') {
                    return Err(SyntaxError::InvalidPathFormat {
                        line: item.line_number,
                        path: render_utf8_component(path),
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
    ///
    /// The decision is the Verus-verified
    /// [`crate::proven::path_rules::check_path_structure`] kernel, which
    /// applies these rules in this precedence: an empty path, a leading `/`,
    /// or (at the guide root) a leading `\` is an invalid format; then each
    /// raw `/`-delimited component, left to right and before any
    /// normalization, is rejected when it is empty, `.` or `..` (a special
    /// directory), contains a forbidden control, or (as the first component
    /// at the guide root) begins with an ASCII letter and `:`.
    fn validate_path_structure(&self, item: &NavigationGuideLine, at_root: bool) -> Result<()> {
        let path = item.path();
        match path_rules::check_path_structure(path, at_root) {
            PathVerdict::Valid => Ok(()),
            PathVerdict::InvalidFormat => Err(SyntaxError::InvalidPathFormat {
                line: item.line_number,
                path: render_utf8_component(path),
            }
            .into()),
            PathVerdict::SpecialDirectory => Err(SyntaxError::InvalidSpecialDirectory {
                line: item.line_number,
                path: render_utf8_component(path),
            }
            .into()),
        }
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
                    path: render_utf8_component(&full_path),
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
                            // Saturating arithmetic only differs on hand-built
                            // trees with levels near `usize::MAX`; the parser
                            // caps logical depth far below that.
                            expected: (level / base).saturating_add(1).saturating_mul(base),
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
                    // Children should be exactly one level deeper than parent.
                    // A parent at `usize::MAX` (reachable only in hand-built
                    // trees) can have no valid child level.
                    if item.indent_level.checked_add(1) != Some(child.indent_level) {
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

    #[test]
    fn test_validate_rejects_empty_and_rooted_paths() {
        for path in ["", "/absolute", "\\windows-rooted"] {
            let mut guide = NavigationGuide::new();
            guide.items.push(NavigationGuideLine {
                line_number: 7,
                indent_level: 0,
                item: FilesystemItem::File {
                    path: path.to_string(),
                    comment: None,
                },
            });

            let result = Validator::new().validate_syntax(&guide);
            assert!(
                matches!(
                    result,
                    Err(crate::errors::AppError::Syntax(
                        SyntaxError::InvalidPathFormat { line: 7, .. }
                    ))
                ),
                "path {path:?} should be rejected"
            );
        }
    }

    #[test]
    fn test_validate_allows_leading_backslash_below_root() {
        let mut guide = NavigationGuide::new();
        guide.items = vec![NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::Directory {
                path: "root".to_string(),
                comment: None,
                children: vec![NavigationGuideLine {
                    line_number: 2,
                    indent_level: 1,
                    item: FilesystemItem::File {
                        path: "\\literal-name".to_string(),
                        comment: None,
                    },
                }],
            },
        }];

        assert!(Validator::new().validate_syntax(&guide).is_ok());
    }

    #[test]
    fn test_validate_rejects_inconsistent_indentation_unit() {
        let mut guide = NavigationGuide::new();
        guide.items = vec![NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::Directory {
                path: "root".to_string(),
                comment: None,
                children: vec![NavigationGuideLine {
                    line_number: 2,
                    indent_level: 2,
                    item: FilesystemItem::Directory {
                        path: "child".to_string(),
                        comment: None,
                        children: vec![NavigationGuideLine {
                            line_number: 3,
                            indent_level: 3,
                            item: FilesystemItem::File {
                                path: "leaf".to_string(),
                                comment: None,
                            },
                        }],
                    },
                }],
            },
        }];

        let result = Validator::new().validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InconsistentIndentation {
                    line: 3,
                    expected: 4,
                    found: 3
                }
            ))
        ));
    }

    #[test]
    fn test_validate_rejects_skipped_nesting_level() {
        let mut guide = NavigationGuide::new();
        guide.items = vec![NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::Directory {
                path: "root".to_string(),
                comment: None,
                children: vec![NavigationGuideLine {
                    line_number: 2,
                    indent_level: 2,
                    item: FilesystemItem::File {
                        path: "child".to_string(),
                        comment: None,
                    },
                }],
            },
        }];

        let result = Validator::new().validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidIndentationLevel { line: 2 }
            ))
        ));
    }

    fn single_file_guide(path: &str) -> NavigationGuide {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 4,
            indent_level: 0,
            item: FilesystemItem::File {
                path: path.to_string(),
                comment: None,
            },
        });
        guide
    }

    #[test]
    fn test_validate_path_structure_reports_rendered_path_per_verdict() {
        let result = Validator::new().validate_syntax(&single_file_guide("src/../main.rs"));
        match result {
            Err(crate::errors::AppError::Syntax(SyntaxError::InvalidSpecialDirectory {
                line,
                path,
            })) => {
                assert_eq!(line, 4);
                assert_eq!(path, "\"src/../main.rs\"");
            }
            other => panic!("unexpected result: {other:?}"),
        }

        let result = Validator::new().validate_syntax(&single_file_guide("a\tb/C:x"));
        match result {
            Err(crate::errors::AppError::Syntax(SyntaxError::InvalidPathFormat { line, path })) => {
                assert_eq!(line, 4);
                assert_eq!(path, "\"a\\tb/C:x\"");
            }
            other => panic!("unexpected result: {other:?}"),
        }

        let result = Validator::new().validate_syntax(&single_file_guide("C:notes"));
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidPathFormat { line: 4, .. }
            ))
        ));
        assert!(Validator::new()
            .validate_syntax(&single_file_guide("dir/C:notes"))
            .is_ok());
    }

    #[test]
    fn test_validate_nesting_rejects_child_of_maximum_level_without_overflow() {
        let mut guide = NavigationGuide::new();
        guide.items = vec![NavigationGuideLine {
            line_number: 1,
            indent_level: usize::MAX,
            item: FilesystemItem::Directory {
                path: "root".to_string(),
                comment: None,
                children: vec![NavigationGuideLine {
                    line_number: 2,
                    indent_level: 0,
                    item: FilesystemItem::File {
                        path: "child".to_string(),
                        comment: None,
                    },
                }],
            },
        }];

        let result = Validator::new().validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidIndentationLevel { line: 2 }
            ))
        ));
    }

    #[test]
    fn test_validate_indentation_expected_level_saturates_without_overflow() {
        let mut guide = NavigationGuide::new();
        guide.items = vec![
            NavigationGuideLine {
                line_number: 1,
                indent_level: 2,
                item: FilesystemItem::File {
                    path: "even".to_string(),
                    comment: None,
                },
            },
            NavigationGuideLine {
                line_number: 2,
                indent_level: usize::MAX,
                item: FilesystemItem::File {
                    path: "odd".to_string(),
                    comment: None,
                },
            },
        ];

        let result = Validator::new().validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InconsistentIndentation {
                    line: 2,
                    expected: usize::MAX,
                    found: usize::MAX
                }
            ))
        ));
    }
}
