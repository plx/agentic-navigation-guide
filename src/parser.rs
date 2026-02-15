//! Parser for navigation guide markdown files

use crate::errors::{Result, SyntaxError};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use regex::Regex;

/// Parser for navigation guide markdown content
pub struct Parser {
    /// Regular expression for detecting list items
    list_item_regex: Regex,
}

impl Parser {
    /// Create a new parser instance
    pub fn new() -> Self {
        Self {
            list_item_regex: Regex::new(r"^(\s*)-\s+(.+)$").unwrap(),
        }
    }

    /// Parse navigation guide content from a markdown string
    pub fn parse(&self, content: &str) -> Result<NavigationGuide> {
        // Find the guide block
        let (prologue, guide_content, epilogue, line_offset, ignore) =
            self.extract_guide_block(content)?;

        // Parse the guide content
        let items = self.parse_guide_content(&guide_content, line_offset)?;

        Ok(NavigationGuide {
            items,
            prologue,
            epilogue,
            ignore,
        })
    }

    /// Extract the guide block from the markdown content
    #[allow(clippy::type_complexity)]
    fn extract_guide_block(
        &self,
        content: &str,
    ) -> Result<(Option<String>, String, Option<String>, usize, bool)> {
        let lines: Vec<&str> = content.lines().collect();
        let mut start_idx = None;
        let mut end_idx = None;
        let mut ignore = false;

        // Find and validate opening/closing markers across the full document.
        // We require exactly one opening marker and exactly one closing marker.
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Check for opening tag with or without attributes
            if trimmed.starts_with("<agentic-navigation-guide") && trimmed.ends_with(">") {
                if start_idx.is_some() || end_idx.is_some() {
                    return Err(SyntaxError::MultipleGuideBlocks { line: idx + 1 }.into());
                }
                start_idx = Some(idx);

                // Parse ignore attribute if present
                ignore = self.parse_ignore_attribute(trimmed);
            } else if trimmed == "</agentic-navigation-guide>" {
                if start_idx.is_some() {
                    if end_idx.is_some() {
                        return Err(SyntaxError::MultipleGuideBlocks { line: idx + 1 }.into());
                    }
                    end_idx = Some(idx);
                } else if end_idx.is_none() {
                    // Preserve missing opening marker behavior while still tracking
                    // a stray closing marker for follow-up marker conflict detection.
                    end_idx = Some(idx);
                } else {
                    return Err(SyntaxError::MultipleGuideBlocks { line: idx + 1 }.into());
                }
            }
        }

        // Validate markers
        let start = start_idx.ok_or(SyntaxError::MissingOpeningMarker { line: 1 })?;
        let end = end_idx.ok_or(SyntaxError::MissingClosingMarker { line: lines.len() })?;

        // Extract prologue, guide content, and epilogue
        let prologue = if start > 0 {
            Some(lines[..start].join("\n"))
        } else {
            None
        };

        let guide_content = lines[start + 1..end].join("\n");

        let epilogue = if end + 1 < lines.len() {
            Some(lines[end + 1..].join("\n"))
        } else {
            None
        };

        // Calculate line offset: prologue lines + opening tag line
        let line_offset = start + 1;

        Ok((prologue, guide_content, epilogue, line_offset, ignore))
    }

    /// Parse the ignore attribute from the opening tag
    /// Supports both `ignore=true` and `ignore="true"` formats
    fn parse_ignore_attribute(&self, tag: &str) -> bool {
        // Check for ignore=true or ignore="true"
        if tag.contains("ignore=true") || tag.contains("ignore=\"true\"") {
            return true;
        }
        false
    }

    /// Parse the guide content into navigation guide lines
    fn parse_guide_content(
        &self,
        content: &str,
        line_offset: usize,
    ) -> Result<Vec<NavigationGuideLine>> {
        if content.trim().is_empty() {
            return Err(SyntaxError::EmptyGuideBlock.into());
        }

        let mut items = Vec::new();
        let mut indent_size = None;
        let lines: Vec<&str> = content.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            // Calculate the actual line number in the file
            let line_number = idx + 1 + line_offset;

            // Check for blank lines
            if line.trim().is_empty() {
                return Err(SyntaxError::BlankLineInGuide { line: line_number }.into());
            }

            // Parse the list item
            if let Some(captures) = self.list_item_regex.captures(line) {
                let indent = captures.get(1).unwrap().as_str().len();
                let content = captures.get(2).unwrap().as_str();

                // Determine indent size from first indented item
                if indent > 0 && indent_size.is_none() {
                    indent_size = Some(indent);
                }

                // Validate indentation
                let indent_level = if indent == 0 {
                    0
                } else if let Some(size) = indent_size {
                    if indent % size != 0 {
                        return Err(
                            SyntaxError::InvalidIndentationLevel { line: line_number }.into()
                        );
                    }
                    indent / size
                } else {
                    // First indented item
                    1
                };

                // Parse path and comment
                let (path, comment) = self.parse_path_comment(content, line_number)?;
                let expanded_paths = Self::expand_wildcard_path(&path, line_number)?;

                for expanded in expanded_paths {
                    // Determine item type
                    let item = if expanded == "..." {
                        FilesystemItem::Placeholder {
                            comment: comment.clone(),
                        }
                    } else if expanded.ends_with('/') {
                        FilesystemItem::Directory {
                            path: expanded.trim_end_matches('/').to_string(),
                            comment: comment.clone(),
                            children: Vec::new(),
                        }
                    } else {
                        // Could be a file or symlink - we'll treat as file for now
                        FilesystemItem::File {
                            path: expanded,
                            comment: comment.clone(),
                        }
                    };

                    items.push(NavigationGuideLine {
                        line_number,
                        indent_level,
                        item,
                    });
                }
            } else {
                return Err(SyntaxError::InvalidListFormat { line: line_number }.into());
            }
        }

        // Build the hierarchy
        let hierarchical_items = self.build_hierarchy(items)?;

        Ok(hierarchical_items)
    }

    /// Parse path and optional comment from item content
    fn parse_path_comment(
        &self,
        content: &str,
        line_number: usize,
    ) -> Result<(String, Option<String>)> {
        let (raw_path, raw_comment) = Self::split_path_comment(content);
        let path = raw_path.trim().to_string();
        let comment = raw_comment.map(|value| value.trim().to_string());

        // Validate path
        if path.is_empty() {
            return Err(SyntaxError::InvalidPathFormat {
                line: line_number,
                path: String::new(),
            }
            .into());
        }

        // Check for special directories (but allow "..." placeholder)
        if path == "..." {
            // Allowed as placeholder
        } else if path == "." || path == ".." || path == "./" || path == "../" {
            return Err(SyntaxError::InvalidSpecialDirectory {
                line: line_number,
                path,
            }
            .into());
        }

        Ok((path, comment))
    }

    /// Split a list item value into path and optional comment at the first unescaped `#`.
    fn split_path_comment(content: &str) -> (&str, Option<&str>) {
        let mut escaped = false;

        for (idx, ch) in content.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }

            if ch == '\\' {
                escaped = true;
                continue;
            }

            if ch == '#' {
                return (&content[..idx], Some(&content[idx + 1..]));
            }
        }

        (content, None)
    }

    /// Process escape sequences in a string, converting escaped characters to their literal forms.
    ///
    /// Handles the following escape sequences:
    /// - `\"` → `"`
    /// - `\,` → `,`
    /// - `\\` → `\`
    /// - `\[` → `[`
    /// - `\]` → `]`
    /// - `\#` → `#`
    ///
    /// # Arguments
    /// * `s` - The string containing escape sequences
    ///
    /// # Returns
    /// A new string with escape sequences processed
    fn process_escapes(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(&next) = chars.peek() {
                    // Consume the escaped character
                    chars.next();
                    result.push(next);
                } else {
                    // Trailing backslash - just include it
                    result.push(ch);
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Expand wildcard choices within a path, if present.
    ///
    /// This function processes paths containing choice blocks (syntax: `prefix[choice1, choice2]suffix`)
    /// and expands them into multiple paths. It supports:
    /// - Multiple choices separated by commas: `Foo[.h, .cpp]` → `["Foo.h", "Foo.cpp"]`
    /// - Quoted strings to preserve commas and special chars: `Foo["a, b", c]`
    /// - Escape sequences for literal special characters: `\,`, `\"`, `\\`, `\[`, `\]`
    /// - Prefix and suffix around the choice block: `src[/main, /lib].rs` → `["src/main.rs", "src/lib.rs"]`
    ///
    /// Escape sequences are preserved during parsing and processed at the end,
    /// ensuring consistent handling across prefix, choices, and suffix.
    ///
    /// # Arguments
    /// * `path` - The path potentially containing a choice block
    /// * `line_number` - Line number in the source file for error reporting
    ///
    /// # Returns
    /// A vector of expanded paths. Returns a single-element vector if no choice block is present.
    ///
    /// # Errors
    /// Returns `SyntaxError::InvalidWildcardSyntax` if:
    /// - The choice block is malformed (unterminated, invalid escapes, etc.)
    /// - Multiple choice blocks are present (only one is allowed per path)
    /// - The choice block is empty or contains only whitespace
    ///
    /// # Examples
    /// ```ignore
    /// // Single expansion (no choice block)
    /// expand_wildcard_path("foo.rs", 1) → Ok(vec!["foo.rs"])
    ///
    /// // Multiple choices
    /// expand_wildcard_path("File[.h, .cpp]", 1) → Ok(vec!["File.h", "File.cpp"])
    ///
    /// // With prefix and suffix
    /// expand_wildcard_path("src[/main, /lib].rs", 1) → Ok(vec!["src/main.rs", "src/lib.rs"])
    ///
    /// // Quoted strings and escapes
    /// expand_wildcard_path("file[\"a, b\", \\,c]", 1) → Ok(vec!["filea, b", "file,c"])
    /// ```
    fn expand_wildcard_path(path: &str, line_number: usize) -> Result<Vec<String>> {
        let mut prefix = String::new();
        let mut suffix = String::new();
        let mut block_content = String::new();

        let mut in_block = false;
        let mut block_found = false;
        let mut in_quotes = false;
        let mut iter = path.chars().peekable();

        while let Some(ch) = iter.next() {
            match ch {
                '\\' => {
                    let next = iter
                        .next()
                        .ok_or_else(|| SyntaxError::InvalidWildcardSyntax {
                            line: line_number,
                            path: path.to_string(),
                            message: "incomplete escape sequence".to_string(),
                        })?;

                    // Preserve escape sequences consistently across prefix, block, and suffix
                    if in_block {
                        block_content.push('\\');
                        block_content.push(next);
                    } else if block_found {
                        suffix.push('\\');
                        suffix.push(next);
                    } else {
                        prefix.push('\\');
                        prefix.push(next);
                    }
                }
                '[' if !in_block => {
                    if block_found {
                        return Err(SyntaxError::InvalidWildcardSyntax {
                            line: line_number,
                            path: path.to_string(),
                            message: "multiple wildcard choice blocks are not supported"
                                .to_string(),
                        }
                        .into());
                    }
                    block_found = true;
                    in_block = true;
                    in_quotes = false;
                }
                ']' if in_block && !in_quotes => {
                    in_block = false;
                    in_quotes = false;
                }
                ']' if in_block => {
                    block_content.push(ch);
                }
                '"' if in_block => {
                    in_quotes = !in_quotes;
                    block_content.push(ch);
                }
                _ => {
                    if in_block {
                        block_content.push(ch);
                    } else if block_found {
                        suffix.push(ch);
                    } else {
                        prefix.push(ch);
                    }
                }
            }
        }

        if in_block {
            return Err(SyntaxError::InvalidWildcardSyntax {
                line: line_number,
                path: path.to_string(),
                message: "unterminated wildcard choice block".to_string(),
            }
            .into());
        }

        if !block_found {
            // No wildcard block - just process escapes in the prefix and return
            return Ok(vec![Self::process_escapes(&prefix)]);
        }

        let choices = Self::parse_choice_block(&block_content, path, line_number)?;
        let mut results = Vec::with_capacity(choices.len());

        // Process escapes in prefix and suffix once
        let processed_prefix = Self::process_escapes(&prefix);
        let processed_suffix = Self::process_escapes(&suffix);

        for choice in choices {
            // Process escapes in each choice and combine with prefix/suffix
            let processed_choice = Self::process_escapes(&choice);
            let mut expanded = processed_prefix.clone();
            expanded.push_str(&processed_choice);
            expanded.push_str(&processed_suffix);
            results.push(expanded);
        }

        Ok(results)
    }

    /// Parse the contents of a wildcard choice block into individual options.
    ///
    /// Takes the content between `[` and `]` and splits it into individual choices.
    /// This is a helper function for `expand_wildcard_path`.
    ///
    /// # Parsing Rules
    /// - Choices are separated by commas (`,`)
    /// - Commas inside quoted strings (`"..."`) are not treated as separators
    /// - Whitespace outside quotes is ignored/trimmed
    /// - Whitespace inside quotes is preserved
    /// - Escape sequences (`\,`, `\"`, etc.) are preserved for later processing
    /// - Quote characters (`"`) toggle quote mode but are not included in output
    ///
    /// # Arguments
    /// * `content` - The string content between `[` and `]` (without the brackets)
    /// * `path` - The full original path for error messages
    /// * `line_number` - Line number in the source file for error reporting
    ///
    /// # Returns
    /// A vector of choice strings with escape sequences still intact (to be processed by caller).
    ///
    /// # Errors
    /// Returns `SyntaxError::InvalidWildcardSyntax` if:
    /// - Quote strings are unterminated
    /// - Escape sequences are incomplete (trailing backslash)
    /// - The choice block is empty or all choices are empty/whitespace
    ///
    /// # Examples
    /// ```ignore
    /// parse_choice_block("a, b, c", "path", 1) → Ok(vec!["a", "b", "c"])
    /// parse_choice_block("\"a, b\", c", "path", 1) → Ok(vec!["a, b", "c"])
    /// parse_choice_block("\\,a, b", "path", 1) → Ok(vec!["\\,a", "b"])  // Escape preserved
    /// parse_choice_block("  a  ,  b  ", "path", 1) → Ok(vec!["a", "b"])  // Trimmed
    /// ```
    fn parse_choice_block(content: &str, path: &str, line_number: usize) -> Result<Vec<String>> {
        let mut choices = Vec::new();
        let mut current = String::new();
        let mut chars = content.chars().peekable();
        let mut in_quotes = false;

        while let Some(ch) = chars.next() {
            match ch {
                '\\' => {
                    let next = chars
                        .next()
                        .ok_or_else(|| SyntaxError::InvalidWildcardSyntax {
                            line: line_number,
                            path: path.to_string(),
                            message: "incomplete escape sequence".to_string(),
                        })?;
                    // Preserve escape sequences - they'll be processed later
                    current.push('\\');
                    current.push(next);
                }
                '"' => {
                    in_quotes = !in_quotes;
                }
                ',' if !in_quotes => {
                    choices.push(current.trim().to_string());
                    current.clear();
                }
                ch if ch.is_whitespace() && !in_quotes => {
                    // Ignore whitespace outside of quotes
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        if in_quotes {
            return Err(SyntaxError::InvalidWildcardSyntax {
                line: line_number,
                path: path.to_string(),
                message: "unterminated quoted string in wildcard choices".to_string(),
            }
            .into());
        }

        choices.push(current.trim().to_string());

        // Validate that the choice block is not empty
        if choices.is_empty() || choices.iter().all(|c| c.is_empty()) {
            return Err(SyntaxError::InvalidWildcardSyntax {
                line: line_number,
                path: path.to_string(),
                message: "choice block cannot be empty".to_string(),
            }
            .into());
        }

        Ok(choices)
    }

    /// Build a hierarchical structure from flat list items
    fn build_hierarchy(&self, items: Vec<NavigationGuideLine>) -> Result<Vec<NavigationGuideLine>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        // First pass: organize items by their parent-child relationships
        let mut result: Vec<NavigationGuideLine> = Vec::new();
        let mut parent_indices: Vec<Option<usize>> = vec![None; items.len()];

        // Find parent index for each item
        for i in 0..items.len() {
            let current_level = items[i].indent_level;

            if current_level == 0 {
                parent_indices[i] = None; // Root item
            } else {
                // Find the nearest preceding directory at level current_level - 1
                let mut parent_found = false;
                for j in (0..i).rev() {
                    if items[j].indent_level == current_level - 1 && items[j].is_directory() {
                        parent_indices[i] = Some(j);
                        parent_found = true;
                        break;
                    } else if items[j].indent_level < current_level - 1 {
                        // Gone too far up the hierarchy
                        break;
                    }
                }

                if !parent_found {
                    return Err(SyntaxError::InvalidIndentationLevel {
                        line: items[i].line_number,
                    }
                    .into());
                }
            }
        }

        // Second pass: build the tree
        // We need to process items in reverse order to ensure children are complete before adding to parents
        let mut processed_items: Vec<Option<NavigationGuideLine>> =
            items.into_iter().map(Some).collect();

        // Process from last to first
        for i in (0..processed_items.len()).rev() {
            if let Some(item) = processed_items[i].take() {
                if let Some(parent_idx) = parent_indices[i] {
                    // Add this item to its parent's children
                    if let Some(ref mut parent) = processed_items[parent_idx] {
                        match &mut parent.item {
                            FilesystemItem::Directory { children, .. } => {
                                // Insert at the beginning to maintain order
                                children.insert(0, item);
                            }
                            _ => {
                                return Err(SyntaxError::InvalidIndentationLevel {
                                    line: item.line_number,
                                }
                                .into());
                            }
                        }
                    }
                } else {
                    // Root item - add to result
                    result.insert(0, item);
                }
            }
        }

        Ok(result)
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_guide() {
        let content = r#"<agentic-navigation-guide>
- src/
  - main.rs
- Cargo.toml
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();
        assert_eq!(guide.items.len(), 2); // src/ and Cargo.toml at root level

        // Check that src/ contains main.rs as a child
        let src_item = &guide.items[0];
        assert!(src_item.is_directory());
        assert_eq!(src_item.path(), "src");

        if let Some(children) = src_item.children() {
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].path(), "main.rs");
        } else {
            panic!("src/ should have children");
        }
    }

    #[test]
    fn test_missing_opening_marker() {
        let content = r#"- src/
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let result = parser.parse(content);
        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::MissingOpeningMarker { .. }
            ))
        ));
    }

    #[test]
    fn test_multiple_guide_blocks_second_block_after_first_close() {
        let content = r#"<agentic-navigation-guide>
- src/
</agentic-navigation-guide>

<agentic-navigation-guide>
- docs/
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let result = parser.parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::MultipleGuideBlocks { line: 5 }
            ))
        ));
    }

    #[test]
    fn test_multiple_guide_blocks_second_opening_before_first_close() {
        let content = r#"<agentic-navigation-guide>
- src/
<agentic-navigation-guide>
- docs/
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let result = parser.parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::MultipleGuideBlocks { line: 3 }
            ))
        ));
    }

    #[test]
    fn test_multiple_guide_blocks_extra_closing_marker() {
        let content = r#"<agentic-navigation-guide>
- src/
</agentic-navigation-guide>
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let result = parser.parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::MultipleGuideBlocks { line: 4 }
            ))
        ));
    }

    #[test]
    fn test_parse_with_comments() {
        let content = r#"<agentic-navigation-guide>
- src/ # source code
- Cargo.toml # project manifest
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();
        assert_eq!(guide.items.len(), 2);
        assert_eq!(guide.items[0].comment(), Some("source code"));
        assert_eq!(guide.items[1].comment(), Some("project manifest"));
    }

    #[test]
    fn test_parse_with_escaped_hash_in_path() {
        let content = r#"<agentic-navigation-guide>
- docs/issue\#123.md # ticket
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 1);
        assert_eq!(guide.items[0].path(), "docs/issue#123.md");
        assert_eq!(guide.items[0].comment(), Some("ticket"));
    }

    #[test]
    fn test_parse_comment_uses_first_unescaped_hash() {
        let content = r#"<agentic-navigation-guide>
- docs/issue\#123.md#ticket
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 1);
        assert_eq!(guide.items[0].path(), "docs/issue#123.md");
        assert_eq!(guide.items[0].comment(), Some("ticket"));
    }

    #[test]
    fn test_trailing_whitespace_allowed() {
        let content = r#"<agentic-navigation-guide>
- foo.rs  
- bar.rs          
- baz/     
  - qux.rs      
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();
        assert_eq!(guide.items.len(), 3);
        assert_eq!(guide.items[0].path(), "foo.rs");
        assert_eq!(guide.items[1].path(), "bar.rs");
        assert_eq!(guide.items[2].path(), "baz");

        if let Some(children) = guide.items[2].children() {
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].path(), "qux.rs");
        } else {
            panic!("baz/ should have children");
        }
    }

    #[test]
    fn test_parse_placeholder() {
        let content = r#"<agentic-navigation-guide>
- src/
  - main.rs
  - ... # other source files
- docs/
  - README.md
  - ...
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();
        assert_eq!(guide.items.len(), 2); // src/ and docs/ at root level

        // Check src/ contains main.rs and a placeholder
        let src_item = &guide.items[0];
        if let Some(children) = src_item.children() {
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].path(), "main.rs");
            assert!(children[1].is_placeholder());
            assert_eq!(children[1].comment(), Some("other source files"));
        } else {
            panic!("src/ should have children");
        }

        // Check docs/ contains README.md and a placeholder
        let docs_item = &guide.items[1];
        if let Some(children) = docs_item.children() {
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].path(), "README.md");
            assert!(children[1].is_placeholder());
            assert_eq!(children[1].comment(), None);
        } else {
            panic!("docs/ should have children");
        }
    }

    #[test]
    fn test_parse_ignore_attribute_unquoted() {
        let content = r#"<agentic-navigation-guide ignore=true>
- src/
  - main.rs
- Cargo.toml
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();
        assert!(guide.ignore);
        assert_eq!(guide.items.len(), 2);
    }

    #[test]
    fn test_parse_ignore_attribute_quoted() {
        let content = r#"<agentic-navigation-guide ignore="true">
- src/
  - main.rs
- Cargo.toml
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();
        assert!(guide.ignore);
        assert_eq!(guide.items.len(), 2);
    }

    #[test]
    fn test_parse_without_ignore_attribute() {
        let content = r#"<agentic-navigation-guide>
- src/
  - main.rs
- Cargo.toml
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();
        assert!(!guide.ignore);
        assert_eq!(guide.items.len(), 2);
    }

    #[test]
    fn test_parse_ignore_attribute_with_spaces() {
        let content = r#"<agentic-navigation-guide  ignore=true  >
- src/
  - main.rs
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();
        assert!(guide.ignore);
        assert_eq!(guide.items.len(), 1);
    }

    #[test]
    fn test_parse_wildcard_expands_multiple_files() {
        let content = r#"<agentic-navigation-guide>
- FooCoordinator[.h, .cpp] # Coordinates foo interactions
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 2);
        assert_eq!(guide.items[0].path(), "FooCoordinator.h");
        assert_eq!(guide.items[1].path(), "FooCoordinator.cpp");
        assert_eq!(
            guide.items[0].comment(),
            Some("Coordinates foo interactions")
        );
        assert_eq!(
            guide.items[1].comment(),
            Some("Coordinates foo interactions")
        );
    }

    #[test]
    fn test_parse_wildcard_with_empty_choice_and_whitespace() {
        let content = r#"<agentic-navigation-guide>
- Config[, .local].json
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 2);
        assert_eq!(guide.items[0].path(), "Config.json");
        assert_eq!(guide.items[1].path(), "Config.local.json");
    }

    #[test]
    fn test_parse_wildcard_with_escapes_and_quotes() {
        let content = r#"<agentic-navigation-guide>
- data["with , comma", \,space, "literal []"] # variations
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 3);
        // Note: Quote characters are not included in output, and whitespace outside
        // quotes is trimmed. Inside quotes, content (including commas and spaces) is preserved.
        // - "with , comma" → with , comma (quotes removed, content preserved)
        // - \,space → ,space (escape processed, whitespace outside quotes trimmed)
        // - "literal []" → literal [] (quotes removed, brackets preserved)
        assert_eq!(guide.items[0].path(), "datawith , comma");
        assert_eq!(guide.items[1].path(), "data,space");
        assert_eq!(guide.items[2].path(), "dataliteral []");
    }

    #[test]
    fn test_parse_wildcard_literal_brackets_without_expansion() {
        let content = r#"<agentic-navigation-guide>
- Foo\[bar\].txt
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 1);
        assert_eq!(guide.items[0].path(), "Foo[bar].txt");
    }

    #[test]
    fn test_parse_wildcard_multiple_blocks_error() {
        let content = r#"<agentic-navigation-guide>
- Foo[.h][.cpp]
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let result = parser.parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidWildcardSyntax { .. }
            ))
        ));
    }

    #[test]
    fn test_parse_choice_block_with_quotes() {
        let parsed =
            Parser::parse_choice_block("\"with , comma\", \\,space, \"literal []\"", "path", 1)
                .unwrap();

        // Note: parse_choice_block now preserves escape sequences
        // They are processed later in expand_wildcard_path
        assert_eq!(parsed, vec!["with , comma", "\\,space", "literal []"]);
    }

    #[test]
    fn test_expand_wildcard_with_escapes_and_quotes() {
        let expanded =
            Parser::expand_wildcard_path("data[\"with , comma\", \\,space, \"literal []\"]", 1)
                .unwrap();

        assert_eq!(
            expanded,
            vec![
                "datawith , comma".to_string(),
                "data,space".to_string(),
                "dataliteral []".to_string(),
            ]
        );
    }

    #[test]
    fn test_parse_wildcard_with_escaped_quotes_in_quoted_strings() {
        let content = r#"<agentic-navigation-guide>
- file[\"test\\\"quote\"].txt
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 1);
        assert_eq!(guide.items[0].path(), r#"file"test\"quote".txt"#);
    }

    #[test]
    fn test_parse_wildcard_empty_choice_block_error() {
        let content = r#"<agentic-navigation-guide>
- Foo[]
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let result = parser.parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidWildcardSyntax { .. }
            ))
        ));

        if let Err(crate::errors::AppError::Syntax(SyntaxError::InvalidWildcardSyntax {
            message,
            ..
        })) = result
        {
            assert_eq!(message, "choice block cannot be empty");
        }
    }

    #[test]
    fn test_parse_wildcard_whitespace_only_choice_block_error() {
        let content = r#"<agentic-navigation-guide>
- Foo[   ,  ,   ]
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let result = parser.parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidWildcardSyntax { .. }
            ))
        ));

        if let Err(crate::errors::AppError::Syntax(SyntaxError::InvalidWildcardSyntax {
            message,
            ..
        })) = result
        {
            assert_eq!(message, "choice block cannot be empty");
        }
    }

    #[test]
    fn test_parse_wildcard_complex_nested_escapes() {
        // Test escaped quotes with actual quoted string to preserve spaces
        let content = r#"<agentic-navigation-guide>
- file["a \"b\" c"].txt
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 1);
        // Note: Escaped quotes inside a quoted string are processed
        assert_eq!(guide.items[0].path(), r#"filea "b" c.txt"#);
    }

    #[test]
    fn test_split_path_comment_ignores_escaped_hashes() {
        assert_eq!(
            Parser::split_path_comment(r#"docs/issue\#123.md#ticket"#),
            (r#"docs/issue\#123.md"#, Some("ticket"))
        );
        assert_eq!(
            Parser::split_path_comment(r#"docs/issue\#123.md"#),
            (r#"docs/issue\#123.md"#, None)
        );
    }
}
