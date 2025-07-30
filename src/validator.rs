//! Syntax validation for navigation guides

use crate::errors::{Result, SyntaxError};
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
            self.validate_item(item)?;
        }

        // Validate indentation consistency
        self.validate_indentation(&guide.items)?;

        Ok(())
    }

    /// Validate a single navigation guide item
    fn validate_item(&self, item: &NavigationGuideLine) -> Result<()> {
        // Validate path characters
        self.validate_path_characters(item)?;

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
        }

        Ok(())
    }

    /// Validate path characters
    fn validate_path_characters(&self, item: &NavigationGuideLine) -> Result<()> {
        let path = item.path();

        // Check for empty path
        if path.is_empty() {
            return Err(SyntaxError::InvalidPathFormat {
                line: item.line_number,
                path: path.to_string(),
            }
            .into());
        }

        // Check for invalid characters
        // Allow alphanumeric, dash, underscore, dot, and forward slash
        for ch in path.chars() {
            if !ch.is_alphanumeric()
                && !matches!(
                    ch,
                    '-' | '_'
                        | '.'
                        | '/'
                        | ' '
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '@'
                        | '+'
                        | '~'
                        | ','
                )
            {
                return Err(SyntaxError::InvalidPathFormat {
                    line: item.line_number,
                    path: path.to_string(),
                }
                .into());
            }
        }

        // Check for double slashes
        if path.contains("//") {
            return Err(SyntaxError::InvalidPathFormat {
                line: item.line_number,
                path: path.to_string(),
            }
            .into());
        }

        // Check for paths starting or ending with slash (should have been handled in parsing)
        if path.starts_with('/') || path.ends_with('/') {
            return Err(SyntaxError::InvalidPathFormat {
                line: item.line_number,
                path: path.to_string(),
            }
            .into());
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
                    // Recursively check children
                    self.validate_nesting(children)?;
                }
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
    fn test_validate_invalid_path_characters() {
        let mut guide = NavigationGuide::new();
        guide.items.push(NavigationGuideLine {
            line_number: 1,
            indent_level: 0,
            item: FilesystemItem::File {
                path: "file|with|pipes.txt".to_string(),
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
}
