//! Parser for navigation guide markdown files

use crate::errors::{Result, SyntaxError};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use regex::Regex;

/// Parser for navigation guide markdown content
pub struct Parser {
    /// Regular expression for detecting list items
    list_item_regex: Regex,
    /// Regular expression for parsing path and comment
    path_comment_regex: Regex,
}

impl Parser {
    /// Create a new parser instance
    pub fn new() -> Self {
        Self {
            list_item_regex: Regex::new(r"^(\s*)-\s+(.+)$").unwrap(),
            path_comment_regex: Regex::new(r"^([^#]+?)(?:\s*#\s*(.*))?$").unwrap(),
        }
    }

    /// Parse navigation guide content from a markdown string
    pub fn parse(&self, content: &str) -> Result<NavigationGuide> {
        // Find the guide block
        let (prologue, guide_content, epilogue) = self.extract_guide_block(content)?;

        // Parse the guide content
        let items = self.parse_guide_content(&guide_content)?;

        Ok(NavigationGuide {
            items,
            prologue,
            epilogue,
        })
    }

    /// Extract the guide block from the markdown content
    fn extract_guide_block(
        &self,
        content: &str,
    ) -> Result<(Option<String>, String, Option<String>)> {
        let lines: Vec<&str> = content.lines().collect();
        let mut start_idx = None;
        let mut end_idx = None;

        // Find the opening and closing markers
        for (idx, line) in lines.iter().enumerate() {
            if line.trim() == "<agentic-navigation-guide>" {
                if start_idx.is_some() {
                    return Err(SyntaxError::MultipleGuideBlocks { line: idx + 1 }.into());
                }
                start_idx = Some(idx);
            } else if line.trim() == "</agentic-navigation-guide>" {
                end_idx = Some(idx);
                break;
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

        Ok((prologue, guide_content, epilogue))
    }

    /// Parse the guide content into navigation guide lines
    fn parse_guide_content(&self, content: &str) -> Result<Vec<NavigationGuideLine>> {
        if content.trim().is_empty() {
            return Err(SyntaxError::EmptyGuideBlock.into());
        }

        let mut items = Vec::new();
        let mut indent_size = None;
        let lines: Vec<&str> = content.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let line_number = idx + 1;

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

                // Determine item type
                let item = if path.ends_with('/') {
                    FilesystemItem::Directory {
                        path: path.trim_end_matches('/').to_string(),
                        comment,
                        children: Vec::new(),
                    }
                } else {
                    // Could be a file or symlink - we'll treat as file for now
                    FilesystemItem::File { path, comment }
                };

                items.push(NavigationGuideLine {
                    line_number,
                    indent_level,
                    item,
                });
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
        if let Some(captures) = self.path_comment_regex.captures(content) {
            let path = captures.get(1).unwrap().as_str().trim().to_string();
            let comment = captures.get(2).map(|m| m.as_str().trim().to_string());

            // Validate path
            if path.is_empty() {
                return Err(SyntaxError::InvalidPathFormat {
                    line: line_number,
                    path: String::new(),
                }
                .into());
            }

            // Check for special directories
            if path == "." || path == ".." || path == "./" || path == "../" {
                return Err(SyntaxError::InvalidSpecialDirectory {
                    line: line_number,
                    path,
                }
                .into());
            }

            Ok((path, comment))
        } else {
            Err(SyntaxError::InvalidPathFormat {
                line: line_number,
                path: content.to_string(),
            }
            .into())
        }
    }

    /// Build a hierarchical structure from flat list items
    fn build_hierarchy(&self, items: Vec<NavigationGuideLine>) -> Result<Vec<NavigationGuideLine>> {
        // For now, return the flat list - hierarchical building will be implemented later
        // This is a complex operation that requires careful handling of parent-child relationships
        Ok(items)
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
        assert_eq!(guide.items.len(), 3);
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
}
