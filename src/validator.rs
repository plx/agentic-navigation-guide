//! Syntax validation for navigation guides

use crate::errors::{Result, SyntaxError};
use crate::path_codec::render_utf8_component;
use crate::proven::path_rules::{self, PathVerdict};
use crate::proven::tree_rules::{
    self, DuplicateCheck, HashKeys, PathEntry, PlaceholderVerdict, SiblingShape,
};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use std::collections::hash_map::RandomState;
use std::collections::HashSet;
use std::hash::BuildHasher;

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

        // One reusable buffer holds each sibling list's shapes for the
        // placeholder rules.
        let mut shapes = Vec::new();

        // Validate each item
        for item in &guide.items {
            self.validate_item(item, true, &mut shapes)?;
        }

        // Validate indentation consistency
        self.validate_indentation(&guide.items, &mut shapes)?;

        // Reject duplicate decoded file/directory identities, including paths
        // reached through different hierarchy spellings. Placeholders are
        // structural wildcards rather than filesystem identities.
        Self::validate_unique_full_paths(&guide.items)?;

        Ok(())
    }

    /// Validate a single navigation guide item
    fn validate_item(
        &self,
        item: &NavigationGuideLine,
        at_root: bool,
        shapes: &mut Vec<SiblingShape>,
    ) -> Result<()> {
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
                    self.validate_item(child, false, shapes)?;
                }

                // Check placeholder-specific rules for children
                self.validate_placeholder_rules(children, shapes)?;
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

    /// Reject the first entry, in preorder, whose full path repeats an
    /// earlier file or directory entry's full path.
    ///
    /// The decision is the Verus-verified
    /// [`crate::proven::tree_rules::first_repeated_full_path`] kernel over
    /// the preorder listing that [`Self::list_preorder`] builds; a full path
    /// is the owner's full path, `/`, and the item path, and placeholders
    /// have none. The kernel's hash keys come from [`RandomState`] and
    /// affect only running time.
    fn validate_unique_full_paths(items: &[NavigationGuideLine]) -> Result<()> {
        let mut entries = Vec::new();
        let mut lines = Vec::new();
        Self::list_preorder(items, None, &mut entries, &mut lines);

        let random = RandomState::new();
        let keys = HashKeys {
            base: random.hash_one(0_u8),
            scale: random.hash_one(1_u8),
            shift: random.hash_one(2_u8),
        };
        match tree_rules::first_repeated_full_path(&entries, keys) {
            DuplicateCheck::Unique => Ok(()),
            DuplicateCheck::Duplicate { index, full_path } => Err(SyntaxError::InvalidPathFormat {
                line: lines[index],
                path: render_utf8_component(&full_path),
            }
            .into()),
        }
    }

    /// List `items` and their descendants in preorder, each with the index
    /// of the directory entry that owns it (`None` for `items` at the guide
    /// root), and record each entry's line number at the same index.
    fn list_preorder<'a>(
        items: &'a [NavigationGuideLine],
        parent: Option<usize>,
        entries: &mut Vec<PathEntry<'a>>,
        lines: &mut Vec<usize>,
    ) {
        for item in items {
            let index = entries.len();
            entries.push(PathEntry {
                parent,
                path: item.path(),
                placeholder: item.is_placeholder(),
            });
            lines.push(item.line_number);
            if let Some(children) = item.children() {
                Self::list_preorder(children, Some(index), entries, lines);
            }
        }
    }

    /// Validate indentation consistency across items
    fn validate_indentation(
        &self,
        items: &[NavigationGuideLine],
        shapes: &mut Vec<SiblingShape>,
    ) -> Result<()> {
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
        self.validate_placeholder_rules(items, shapes)?;

        Ok(())
    }

    /// Validate placeholder-specific rules for one sibling list.
    ///
    /// The decision is the Verus-verified
    /// [`crate::proven::tree_rules::check_placeholder_siblings`] kernel: in
    /// list order, a placeholder followed by another placeholder reports the
    /// second one, and otherwise a placeholder with children reports itself.
    /// `shapes` is scratch space reused across sibling lists.
    fn validate_placeholder_rules(
        &self,
        items: &[NavigationGuideLine],
        shapes: &mut Vec<SiblingShape>,
    ) -> Result<()> {
        shapes.clear();
        shapes.extend(items.iter().map(Self::sibling_shape));
        match tree_rules::check_placeholder_siblings(shapes) {
            PlaceholderVerdict::Valid => Ok(()),
            PlaceholderVerdict::Adjacent { index } => Err(SyntaxError::AdjacentPlaceholders {
                line: items[index].line_number,
            }
            .into()),
            // Unreachable: a placeholder has no children field, so
            // `sibling_shape` never reports one that owns children.
            PlaceholderVerdict::WithChildren { index } => {
                Err(SyntaxError::PlaceholderWithChildren {
                    line: items[index].line_number,
                }
                .into())
            }
        }
    }

    /// The shape of one sibling for the placeholder rules.
    fn sibling_shape(item: &NavigationGuideLine) -> SiblingShape {
        if item.is_placeholder() {
            SiblingShape::Placeholder {
                has_children: item.children().is_some_and(|children| !children.is_empty()),
            }
        } else {
            SiblingShape::Entry
        }
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

    /// `Validator` exactly as it was before the tree rules moved into
    /// `crate::proven::tree_rules` (commit 0077a2b), for differential tests.
    mod original {
        use crate::errors::{Result, SyntaxError};
        use crate::path_codec::{
            contains_forbidden_control, has_windows_drive_prefix, render_utf8_component,
        };
        use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
        use std::collections::HashSet;

        pub(super) struct Original;

        impl Original {
            /// The original `Validator::validate_syntax`.
            pub(super) fn check(&self, guide: &NavigationGuide) -> Result<()> {
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
            fn validate_path_structure(
                &self,
                item: &NavigationGuideLine,
                at_root: bool,
            ) -> Result<()> {
                let path = item.path();
                let diagnostic_path = render_utf8_component(path);

                // Check for empty path
                if path.is_empty() {
                    return Err(SyntaxError::InvalidPathFormat {
                        line: item.line_number,
                        path: diagnostic_path,
                    }
                    .into());
                }

                // `/` is the sole logical separator, but a leading backslash is still
                // a rooted Windows spelling and rejects on every host.
                if path.starts_with('/') || (at_root && path.starts_with('\\')) {
                    return Err(SyntaxError::InvalidPathFormat {
                        line: item.line_number,
                        path: diagnostic_path,
                    }
                    .into());
                }

                // Validate raw slash-delimited components before any normalization so
                // `.` / `..` and empty components remain observable.
                for (index, component) in path.split('/').enumerate() {
                    if component.is_empty() {
                        return Err(SyntaxError::InvalidPathFormat {
                            line: item.line_number,
                            path: diagnostic_path,
                        }
                        .into());
                    }

                    if component == "." || component == ".." {
                        return Err(SyntaxError::InvalidSpecialDirectory {
                            line: item.line_number,
                            path: diagnostic_path,
                        }
                        .into());
                    }

                    if contains_forbidden_control(component) {
                        return Err(SyntaxError::InvalidPathFormat {
                            line: item.line_number,
                            path: diagnostic_path,
                        }
                        .into());
                    }

                    if at_root && index == 0 && has_windows_drive_prefix(component) {
                        return Err(SyntaxError::InvalidPathFormat {
                            line: item.line_number,
                            path: diagnostic_path,
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
            fn collect_indent_levels(
                &self,
                items: &[NavigationGuideLine],
                levels: &mut HashSet<usize>,
            ) {
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
    }

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

    fn file(line_number: usize, indent_level: usize, path: &str) -> NavigationGuideLine {
        NavigationGuideLine {
            line_number,
            indent_level,
            item: FilesystemItem::File {
                path: path.to_string(),
                comment: None,
            },
        }
    }

    fn directory(
        line_number: usize,
        indent_level: usize,
        path: &str,
        children: Vec<NavigationGuideLine>,
    ) -> NavigationGuideLine {
        NavigationGuideLine {
            line_number,
            indent_level,
            item: FilesystemItem::Directory {
                path: path.to_string(),
                comment: None,
                children,
            },
        }
    }

    fn placeholder(line_number: usize, indent_level: usize) -> NavigationGuideLine {
        NavigationGuideLine {
            line_number,
            indent_level,
            item: FilesystemItem::Placeholder { comment: None },
        }
    }

    fn guide_of(items: Vec<NavigationGuideLine>) -> NavigationGuide {
        let mut guide = NavigationGuide::new();
        guide.items = items;
        guide
    }

    #[test]
    fn test_validate_rejects_full_path_repeated_through_hierarchy_spelling() {
        let guide = guide_of(vec![
            directory(1, 0, "src", vec![file(2, 1, "main.rs")]),
            file(3, 0, "src/main.rs"),
        ]);
        let result = Validator::new().validate_syntax(&guide);
        match result {
            Err(crate::errors::AppError::Syntax(SyntaxError::InvalidPathFormat { line, path })) => {
                assert_eq!(line, 3);
                assert_eq!(path, render_utf8_component("src/main.rs"));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_validate_reports_first_repeating_entry_in_preorder() {
        let guide = guide_of(vec![
            directory(
                1,
                0,
                "x",
                vec![file(2, 1, "y"), placeholder(3, 1), file(4, 1, "y")],
            ),
            file(5, 0, "x"),
        ]);
        let result = Validator::new().validate_syntax(&guide);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidPathFormat { line: 4, .. }
            ))
        ));
    }

    #[test]
    fn test_validate_reports_adjacent_root_placeholders_after_nesting() {
        let guide = guide_of(vec![file(1, 0, "a"), placeholder(2, 0), placeholder(3, 0)]);
        assert!(matches!(
            Validator::new().validate_syntax(&guide),
            Err(crate::errors::AppError::Syntax(
                SyntaxError::AdjacentPlaceholders { line: 3 }
            ))
        ));
    }

    #[test]
    fn test_placeholder_shapes_never_own_children() {
        assert_eq!(
            Validator::sibling_shape(&placeholder(1, 0)),
            SiblingShape::Placeholder {
                has_children: false
            }
        );
        assert_eq!(
            Validator::sibling_shape(&directory(1, 0, "d", vec![file(2, 1, "f")])),
            SiblingShape::Entry
        );
        assert_eq!(
            Validator::sibling_shape(&file(1, 0, "f")),
            SiblingShape::Entry
        );
    }

    /// A deterministic xorshift generator for the differential tests.
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, bound: usize) -> usize {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % bound as u64) as usize
        }
    }

    /// Names that collide through hierarchy spellings, followed by names the
    /// path rules reject.
    const NAMES: [&str; 12] = [
        "a", "b", "a/b", "b/a", "a/b/a", "\u{e9}", "", ".", "/x", "C:x", "a//b", "x\u{1}",
    ];
    const VALID_NAMES: usize = 6;

    /// A random sibling list at `depth`, numbered in preorder. At most one
    /// entry is indented one level too deep, so indentation verdicts never
    /// depend on hash-set iteration order.
    fn random_items(
        rng: &mut Rng,
        depth: usize,
        invalid_names: bool,
        line: &mut usize,
        perturbed: &mut bool,
    ) -> Vec<NavigationGuideLine> {
        let count = rng.below(4);
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            *line += 1;
            let line_number = *line;
            let mut indent_level = depth;
            if !*perturbed && rng.below(40) == 0 {
                indent_level += 1;
                *perturbed = true;
            }
            let roll = rng.below(10);
            let name = if invalid_names && rng.below(8) == 0 {
                NAMES[rng.below(NAMES.len())]
            } else {
                NAMES[rng.below(VALID_NAMES)]
            };
            items.push(if roll < 3 {
                placeholder(line_number, indent_level)
            } else if roll < 6 && depth < 3 {
                let children = random_items(rng, depth + 1, invalid_names, line, perturbed);
                directory(line_number, indent_level, name, children)
            } else {
                file(line_number, indent_level, name)
            });
        }
        items
    }

    #[test]
    fn test_validate_syntax_matches_original_validator() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut outcomes = std::collections::BTreeMap::new();
        for round in 0..20_000 {
            let mut line = 0;
            let mut perturbed = false;
            let invalid_names = round % 4 == 0;
            let guide = guide_of(random_items(
                &mut rng,
                0,
                invalid_names,
                &mut line,
                &mut perturbed,
            ));
            let expected = original::Original.check(&guide);
            let actual = Validator::new().validate_syntax(&guide);
            let expected = format!("{expected:?}");
            assert_eq!(format!("{actual:?}"), expected, "{guide:?}");
            let kind = expected
                .split(|ch: char| !ch.is_alphanumeric())
                .filter(|word| !word.is_empty())
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
            // With only valid names, a path-format error is a repeated full
            // path.
            let kind = if invalid_names {
                kind
            } else {
                format!("valid names: {kind}")
            };
            *outcomes.entry(kind).or_insert(0_usize) += 1;
        }
        // The generator reaches every decision the tree rules make.
        for kind in [
            "Ok",
            "Err Syntax AdjacentPlaceholders",
            "Err Syntax InvalidPathFormat",
            "valid names: Err Syntax InvalidPathFormat",
            "valid names: Err Syntax AdjacentPlaceholders",
            "valid names: Err Syntax InvalidIndentationLevel",
            "Err Syntax EmptyGuideBlock",
        ] {
            assert!(
                outcomes.get(kind).copied().unwrap_or(0) >= 100,
                "{kind}: {outcomes:?}"
            );
        }
    }
}
