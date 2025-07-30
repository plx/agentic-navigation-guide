//! Syntax validation for navigation guides

use crate::errors::{Result, SyntaxError};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};

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
        match &item.item {
            FilesystemItem::Directory { path, children, .. } => {
                // Directory paths should not contain the trailing slash in our internal representation
                // (it's added during parsing)
                if path.ends_with('/') {
                    return Err(SyntaxError::DirectoryMissingSlash {
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

    /// Validate indentation consistency across items
    fn validate_indentation(&self, _items: &[NavigationGuideLine]) -> Result<()> {
        // This is a simplified version - full implementation would check
        // that indentation levels are consistent and properly nested
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
}
