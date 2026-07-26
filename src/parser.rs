//! Parser for navigation guide markdown files

use crate::errors::{Result, SyntaxError};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use regex::Regex;
use std::collections::HashSet;

const MAX_INDENT_SIZE: usize = 16;
const MAX_LOGICAL_DEPTH: usize = 256;
const MIN_CHOICE_ALTERNATIVES: usize = 2;
const MAX_CHOICE_ALTERNATIVES: usize = 256;
const OPENING_MARKER_PREFIX: &str = "<agentic-navigation-guide";
const CLOSING_MARKER_PREFIX: &str = "</agentic-navigation-guide";
const CLOSING_MARKER: &str = "</agentic-navigation-guide>";

#[derive(Clone, Copy)]
enum MarkerLine {
    Ordinary,
    Opening { ignore: bool },
    Closing,
}

#[derive(Clone, Copy)]
enum ChoiceState {
    LeadingLayout,
    Unquoted,
    Quoted,
    QuotedClosed,
}

#[derive(Clone, Copy)]
enum HierarchyEvent {
    VisitItem,
    AppendItem,
    CloseDirectory,
    OpenDirectory { stack_depth: usize },
}

trait HierarchyObserver {
    fn observe(&mut self, event: HierarchyEvent);
}

impl HierarchyObserver for () {
    #[inline(always)]
    fn observe(&mut self, event: HierarchyEvent) {
        if let HierarchyEvent::OpenDirectory { stack_depth } = event {
            let _ = stack_depth;
        }
    }
}

/// Parser for navigation guide markdown content
pub struct Parser {
    /// Regular expression for detecting list items
    list_item_regex: Regex,
}

impl Parser {
    /// Create a new parser instance
    pub fn new() -> Self {
        Self {
            list_item_regex: Regex::new(r"^(\s*)- (.+)$").unwrap(),
        }
    }

    /// Parse navigation guide content from a markdown string.
    ///
    /// The exact document envelope and marker-candidate scan are always
    /// validated. When the opening marker contains `ignore=true`, the body is
    /// opaque and may be empty; the returned guide has no parsed items.
    pub fn parse(&self, content: &str) -> Result<NavigationGuide> {
        // Find the guide block
        let (prologue, guide_content, epilogue, line_offset, ignore) =
            self.extract_guide_block(content)?;

        // The exact envelope and global marker-candidate scan above always
        // run. Once that scan recognizes ignore=true, however, the body is
        // intentionally opaque UTF-8 text and may be empty.
        let items = if ignore {
            Vec::new()
        } else {
            self.parse_guide_content(&guide_content, line_offset)?
        };

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
        Self::validate_line_endings(content)?;

        let lines: Vec<&str> = content.lines().collect();
        let mut start_idx = None;
        let mut end_idx = None;
        let mut ignore = false;

        // Find and validate opening/closing markers across the full document.
        // We require exactly one opening marker and exactly one closing marker.
        for (idx, line) in lines.iter().enumerate() {
            match Self::classify_marker_line(line, idx + 1)? {
                MarkerLine::Ordinary => {}
                MarkerLine::Opening {
                    ignore: marker_ignore,
                } => {
                    if start_idx.is_some() || end_idx.is_some() {
                        return Err(SyntaxError::MultipleGuideBlocks { line: idx + 1 }.into());
                    }
                    start_idx = Some(idx);
                    ignore = marker_ignore;
                }
                MarkerLine::Closing => match (start_idx, end_idx) {
                    (Some(_), None) => end_idx = Some(idx),
                    (None, None) => {
                        return Err(SyntaxError::MissingOpeningMarker { line: idx + 1 }.into());
                    }
                    _ => {
                        return Err(SyntaxError::MultipleGuideBlocks { line: idx + 1 }.into());
                    }
                },
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

    /// Reject lone carriage returns before line-oriented parsing.
    fn validate_line_endings(content: &str) -> Result<()> {
        let bytes = content.as_bytes();
        let mut idx = 0;
        let mut line = 1;

        while idx < bytes.len() {
            match bytes[idx] {
                b'\r' if bytes.get(idx + 1) == Some(&b'\n') => {
                    idx += 2;
                    line += 1;
                }
                b'\r' => {
                    return Err(SyntaxError::MissingOpeningMarker { line }.into());
                }
                b'\n' => {
                    idx += 1;
                    line += 1;
                }
                _ => idx += 1,
            }
        }

        Ok(())
    }

    /// Classify a complete line using the exact v0.2 marker grammar.
    fn classify_marker_line(line: &str, line_number: usize) -> Result<MarkerLine> {
        let candidate = Self::trim_outer_wsp(line);

        // A BOM is not outer whitespace and cannot be silently discarded to
        // turn the remainder of the line into a marker. Reject a run of BOMs
        // as well as one BOM, including SP/HTAB between them.
        let mut after_bom = candidate;
        let mut saw_bom = false;
        while let Some(remainder) = after_bom.strip_prefix('\u{feff}') {
            saw_bom = true;
            after_bom = remainder.trim_start_matches([' ', '\t']);
        }
        if saw_bom {
            if after_bom.starts_with(OPENING_MARKER_PREFIX) {
                return Err(SyntaxError::MissingOpeningMarker { line: line_number }.into());
            }
            if after_bom.starts_with(CLOSING_MARKER_PREFIX) {
                return Err(SyntaxError::MissingClosingMarker { line: line_number }.into());
            }
        }

        if candidate.starts_with(OPENING_MARKER_PREFIX) {
            return match Self::parse_opening_marker(candidate) {
                Some(ignore) => Ok(MarkerLine::Opening { ignore }),
                None => Err(SyntaxError::MissingOpeningMarker { line: line_number }.into()),
            };
        }

        if candidate.starts_with(CLOSING_MARKER_PREFIX) {
            return if candidate == CLOSING_MARKER {
                Ok(MarkerLine::Closing)
            } else {
                Err(SyntaxError::MissingClosingMarker { line: line_number }.into())
            };
        }

        Ok(MarkerLine::Ordinary)
    }

    fn trim_outer_wsp(line: &str) -> &str {
        line.trim_matches([' ', '\t'])
    }

    /// Parse the only two permitted opening-marker forms.
    fn parse_opening_marker(candidate: &str) -> Option<bool> {
        let suffix = candidate.strip_prefix(OPENING_MARKER_PREFIX)?;
        if suffix == ">" {
            return Some(false);
        }

        let bytes = suffix.as_bytes();
        let mut idx = 0;

        // Attributed markers require at least one SP or HTAB after the name.
        if !bytes
            .get(idx)
            .is_some_and(|byte| Self::is_marker_wsp(*byte))
        {
            return None;
        }
        while bytes
            .get(idx)
            .is_some_and(|byte| Self::is_marker_wsp(*byte))
        {
            idx += 1;
        }

        if !suffix[idx..].starts_with("ignore") {
            return None;
        }
        idx += "ignore".len();

        while bytes
            .get(idx)
            .is_some_and(|byte| Self::is_marker_wsp(*byte))
        {
            idx += 1;
        }
        if bytes.get(idx) != Some(&b'=') {
            return None;
        }
        idx += 1;

        while bytes
            .get(idx)
            .is_some_and(|byte| Self::is_marker_wsp(*byte))
        {
            idx += 1;
        }
        if suffix[idx..].starts_with("\"true\"") {
            idx += "\"true\"".len();
        } else if suffix[idx..].starts_with("true") {
            idx += "true".len();
        } else {
            return None;
        }

        while bytes
            .get(idx)
            .is_some_and(|byte| Self::is_marker_wsp(*byte))
        {
            idx += 1;
        }

        (bytes.get(idx) == Some(&b'>') && idx + 1 == bytes.len()).then_some(true)
    }

    fn is_marker_wsp(byte: u8) -> bool {
        matches!(byte, b' ' | b'\t')
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
        let mut previous_depth = None;
        let mut previous_line_can_own_children = false;
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
                let indent_text = captures.get(1).unwrap().as_str();
                if !indent_text.bytes().all(|byte| byte == b' ') {
                    return Err(SyntaxError::InvalidIndentationLevel { line: line_number }.into());
                }
                let indent = indent_text.len();
                let content = captures.get(2).unwrap().as_str();
                if content.starts_with(' ') {
                    return Err(SyntaxError::InvalidListFormat { line: line_number }.into());
                }

                // Determine indent size from first indented item
                if indent > 0 && indent_size.is_none() {
                    if indent > MAX_INDENT_SIZE {
                        return Err(
                            SyntaxError::InvalidIndentationLevel { line: line_number }.into()
                        );
                    }
                    indent_size = Some(indent);
                }

                // Validate indentation
                let indent_level = if indent == 0 {
                    0
                } else if let Some(size) = indent_size {
                    let depth = indent
                        .checked_div(size)
                        .ok_or(SyntaxError::InvalidIndentationLevel { line: line_number })?;
                    if depth.checked_mul(size) != Some(indent) {
                        return Err(
                            SyntaxError::InvalidIndentationLevel { line: line_number }.into()
                        );
                    }
                    depth
                } else {
                    unreachable!("a positive first indentation always establishes its unit")
                };

                if indent_level > MAX_LOGICAL_DEPTH {
                    return Err(SyntaxError::InvalidIndentationLevel { line: line_number }.into());
                }

                match previous_depth {
                    None if indent_level != 0 => {
                        return Err(
                            SyntaxError::InvalidIndentationLevel { line: line_number }.into()
                        );
                    }
                    Some(depth)
                        if indent_level > depth
                            && (depth.checked_add(1) != Some(indent_level)
                                || !previous_line_can_own_children) =>
                    {
                        return Err(
                            SyntaxError::InvalidIndentationLevel { line: line_number }.into()
                        );
                    }
                    _ => {}
                }

                // Parse path and comment
                let (path, comment) = self.parse_path_comment(content, line_number)?;
                let (expanded_paths, is_choice) =
                    Self::expand_wildcard_path_with_kind(&path, line_number)?;
                let line_can_own_children =
                    !is_choice && expanded_paths.len() == 1 && expanded_paths[0].ends_with('/');

                for expanded in expanded_paths {
                    // Determine item type
                    let item = if expanded == "..." {
                        FilesystemItem::Placeholder {
                            comment: comment.clone(),
                        }
                    } else if expanded.ends_with('/') {
                        FilesystemItem::Directory {
                            path: expanded
                                .strip_suffix('/')
                                .expect("ends_with established one directory marker")
                                .to_string(),
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

                previous_depth = Some(indent_level);
                previous_line_can_own_children = line_can_own_children;
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
        let comment = raw_comment.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

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
    #[cfg(test)]
    fn expand_wildcard_path(path: &str, line_number: usize) -> Result<Vec<String>> {
        let (paths, _) = Self::expand_wildcard_path_with_kind(path, line_number)?;
        Ok(paths)
    }

    /// Expand a path and report whether it contained a physical choice list.
    ///
    /// Hierarchy validation needs the source-level distinction because even a
    /// valid choice is not exactly one directory entry and therefore cannot
    /// own an indented child.
    fn expand_wildcard_path_with_kind(
        path: &str,
        line_number: usize,
    ) -> Result<(Vec<String>, bool)> {
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
                ']' => {
                    return Err(SyntaxError::InvalidWildcardSyntax {
                        line: line_number,
                        path: path.to_string(),
                        message: "unmatched closing bracket".to_string(),
                    }
                    .into());
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
            return Ok((vec![Self::process_escapes(&prefix)], false));
        }

        let choices = Self::parse_choice_block(&block_content, path, line_number)?;
        let mut results = Vec::with_capacity(choices.len());

        // Process escapes in prefix and suffix once
        let processed_prefix = Self::process_escapes(&prefix);
        let processed_suffix = Self::process_escapes(&suffix);
        let mut unique_results = HashSet::with_capacity(choices.len());
        let mut expected_parent = None;

        for choice in choices {
            // Process escapes in each choice and combine with prefix/suffix
            let processed_choice = Self::process_escapes(&choice);
            let mut expanded = processed_prefix.clone();
            expanded.push_str(&processed_choice);
            expanded.push_str(&processed_suffix);

            if !Self::valid_choice_expansion(&expanded) {
                return Err(SyntaxError::InvalidWildcardSyntax {
                    line: line_number,
                    path: path.to_string(),
                    message: "choice expansions must be valid regular-file paths".to_string(),
                }
                .into());
            }

            let parent = expanded
                .rsplit_once('/')
                .map_or("", |(parent, _)| parent)
                .to_string();
            if expected_parent
                .as_ref()
                .is_some_and(|expected: &String| expected != &parent)
            {
                return Err(SyntaxError::InvalidWildcardSyntax {
                    line: line_number,
                    path: path.to_string(),
                    message: "choice expansions must be sibling files".to_string(),
                }
                .into());
            }
            expected_parent.get_or_insert(parent);

            if !unique_results.insert(expanded.clone()) {
                return Err(SyntaxError::InvalidWildcardSyntax {
                    line: line_number,
                    path: path.to_string(),
                    message: "choice expansions must be unique".to_string(),
                }
                .into());
            }
            results.push(expanded);
        }

        Ok((results, true))
    }

    fn valid_choice_expansion(path: &str) -> bool {
        if path.is_empty() || path == "..." || path.starts_with(['/', '\\']) || path.ends_with('/')
        {
            return false;
        }

        let mut components = path.split('/');
        let Some(first) = components.next() else {
            return false;
        };
        if Self::invalid_decoded_component(first) || Self::has_windows_drive_prefix(first) {
            return false;
        }

        components.all(|component| !Self::invalid_decoded_component(component))
    }

    fn invalid_decoded_component(component: &str) -> bool {
        component.is_empty() || component == "." || component == ".."
    }

    fn has_windows_drive_prefix(component: &str) -> bool {
        let bytes = component.as_bytes();
        bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
    }

    /// Parse the contents of a wildcard choice block into individual options.
    ///
    /// Takes the content between `[` and `]` and splits it into individual choices.
    /// This is a helper function for `expand_wildcard_path`.
    ///
    /// # Parsing Rules
    /// - Choices are separated by commas (`,`)
    /// - Commas inside quoted strings (`"..."`) are not treated as separators
    /// - Surrounding spaces and tabs outside quotes are ignored
    /// - Interior unquoted whitespace is preserved
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
    /// - The choice count falls outside 2–256
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
        let mut pending_layout = String::new();
        let mut chars = content.chars().peekable();
        let mut state = ChoiceState::LeadingLayout;

        while let Some(ch) = chars.next() {
            match state {
                ChoiceState::LeadingLayout => match ch {
                    ' ' | '\t' => {}
                    ',' => choices.push(String::new()),
                    '"' => state = ChoiceState::Quoted,
                    '\\' => {
                        let next =
                            chars
                                .next()
                                .ok_or_else(|| SyntaxError::InvalidWildcardSyntax {
                                    line: line_number,
                                    path: path.to_string(),
                                    message: "incomplete escape sequence".to_string(),
                                })?;
                        if !Self::valid_unquoted_choice_escape(next) {
                            return Err(SyntaxError::InvalidWildcardSyntax {
                                line: line_number,
                                path: path.to_string(),
                                message: "invalid escape in unquoted choice".to_string(),
                            }
                            .into());
                        }
                        current.push('\\');
                        current.push(next);
                        state = ChoiceState::Unquoted;
                    }
                    '[' | ']' => {
                        return Err(SyntaxError::InvalidWildcardSyntax {
                            line: line_number,
                            path: path.to_string(),
                            message: "unescaped bracket in unquoted choice".to_string(),
                        }
                        .into());
                    }
                    _ => {
                        current.push(ch);
                        state = ChoiceState::Unquoted;
                    }
                },
                ChoiceState::Unquoted => match ch {
                    ',' => {
                        pending_layout.clear();
                        choices.push(std::mem::take(&mut current));
                        state = ChoiceState::LeadingLayout;
                    }
                    ' ' | '\t' => pending_layout.push(ch),
                    '\\' => {
                        let next =
                            chars
                                .next()
                                .ok_or_else(|| SyntaxError::InvalidWildcardSyntax {
                                    line: line_number,
                                    path: path.to_string(),
                                    message: "incomplete escape sequence".to_string(),
                                })?;
                        if !Self::valid_unquoted_choice_escape(next) {
                            return Err(SyntaxError::InvalidWildcardSyntax {
                                line: line_number,
                                path: path.to_string(),
                                message: "invalid escape in unquoted choice".to_string(),
                            }
                            .into());
                        }
                        current.push_str(&pending_layout);
                        pending_layout.clear();
                        current.push('\\');
                        current.push(next);
                    }
                    '"' => {
                        return Err(SyntaxError::InvalidWildcardSyntax {
                            line: line_number,
                            path: path.to_string(),
                            message: "quote must begin a choice alternative".to_string(),
                        }
                        .into());
                    }
                    '[' | ']' => {
                        return Err(SyntaxError::InvalidWildcardSyntax {
                            line: line_number,
                            path: path.to_string(),
                            message: "unescaped bracket in unquoted choice".to_string(),
                        }
                        .into());
                    }
                    _ => {
                        current.push_str(&pending_layout);
                        pending_layout.clear();
                        current.push(ch);
                    }
                },
                ChoiceState::Quoted => match ch {
                    '"' => state = ChoiceState::QuotedClosed,
                    '\\' => {
                        let next =
                            chars
                                .next()
                                .ok_or_else(|| SyntaxError::InvalidWildcardSyntax {
                                    line: line_number,
                                    path: path.to_string(),
                                    message: "incomplete escape sequence".to_string(),
                                })?;
                        if !matches!(next, '"' | '\\' | '#') {
                            return Err(SyntaxError::InvalidWildcardSyntax {
                                line: line_number,
                                path: path.to_string(),
                                message: "invalid escape in quoted choice".to_string(),
                            }
                            .into());
                        }
                        current.push('\\');
                        current.push(next);
                    }
                    _ => current.push(ch),
                },
                ChoiceState::QuotedClosed => match ch {
                    ' ' | '\t' => {}
                    ',' => {
                        choices.push(std::mem::take(&mut current));
                        state = ChoiceState::LeadingLayout;
                    }
                    _ => {
                        return Err(SyntaxError::InvalidWildcardSyntax {
                            line: line_number,
                            path: path.to_string(),
                            message: "quoted choice must end before the separator".to_string(),
                        }
                        .into());
                    }
                },
            }
        }

        if matches!(state, ChoiceState::Quoted) {
            return Err(SyntaxError::InvalidWildcardSyntax {
                line: line_number,
                path: path.to_string(),
                message: "unterminated quoted string in wildcard choices".to_string(),
            }
            .into());
        }

        pending_layout.clear();
        choices.push(current);

        // Validate that the choice block is not empty
        if choices.is_empty() || choices.iter().all(|c| c.is_empty()) {
            return Err(SyntaxError::InvalidWildcardSyntax {
                line: line_number,
                path: path.to_string(),
                message: "choice block cannot be empty".to_string(),
            }
            .into());
        }

        if !(MIN_CHOICE_ALTERNATIVES..=MAX_CHOICE_ALTERNATIVES).contains(&choices.len()) {
            return Err(SyntaxError::InvalidWildcardSyntax {
                line: line_number,
                path: path.to_string(),
                message: format!(
                    "choice list must contain between {MIN_CHOICE_ALTERNATIVES} and \
                     {MAX_CHOICE_ALTERNATIVES} alternatives"
                ),
            }
            .into());
        }

        Ok(choices)
    }

    fn valid_unquoted_choice_escape(ch: char) -> bool {
        matches!(ch, '#' | '\\' | '[' | ']' | ',' | '"' | ' ')
    }

    /// Build a hierarchical structure from flat list items
    fn build_hierarchy(&self, items: Vec<NavigationGuideLine>) -> Result<Vec<NavigationGuideLine>> {
        self.build_hierarchy_with_observer(items, &mut ())
    }

    fn build_hierarchy_with_observer<O: HierarchyObserver>(
        &self,
        items: Vec<NavigationGuideLine>,
        observer: &mut O,
    ) -> Result<Vec<NavigationGuideLine>> {
        let mut roots = Vec::new();
        let mut open_directories = Vec::with_capacity(MAX_LOGICAL_DEPTH + 1);

        for item in items {
            observer.observe(HierarchyEvent::VisitItem);
            let depth = item.indent_level;
            if depth > MAX_LOGICAL_DEPTH {
                return Err(SyntaxError::InvalidIndentationLevel {
                    line: item.line_number,
                }
                .into());
            }

            while open_directories.len() > depth {
                Self::close_directory(&mut roots, &mut open_directories, observer)?;
            }

            if open_directories.len() != depth {
                return Err(SyntaxError::InvalidIndentationLevel {
                    line: item.line_number,
                }
                .into());
            }

            if item.is_directory() {
                open_directories.push(item);
                observer.observe(HierarchyEvent::OpenDirectory {
                    stack_depth: open_directories.len(),
                });
            } else {
                Self::append_hierarchy_item(&mut roots, &mut open_directories, item, observer)?;
            }
        }

        while !open_directories.is_empty() {
            Self::close_directory(&mut roots, &mut open_directories, observer)?;
        }

        Ok(roots)
    }

    fn close_directory<O: HierarchyObserver>(
        roots: &mut Vec<NavigationGuideLine>,
        open_directories: &mut Vec<NavigationGuideLine>,
        observer: &mut O,
    ) -> Result<()> {
        observer.observe(HierarchyEvent::CloseDirectory);
        let directory = open_directories
            .pop()
            .expect("close_directory is called only for a non-empty stack");
        Self::append_hierarchy_item(roots, open_directories, directory, observer)
    }

    fn append_hierarchy_item<O: HierarchyObserver>(
        roots: &mut Vec<NavigationGuideLine>,
        open_directories: &mut [NavigationGuideLine],
        item: NavigationGuideLine,
        observer: &mut O,
    ) -> Result<()> {
        observer.observe(HierarchyEvent::AppendItem);
        let Some(parent) = open_directories.last_mut() else {
            roots.push(item);
            return Ok(());
        };

        let FilesystemItem::Directory { children, .. } = &mut parent.item else {
            return Err(SyntaxError::InvalidIndentationLevel {
                line: item.line_number,
            }
            .into());
        };
        children.push(item);
        Ok(())
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
    use std::hint::black_box;
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct HierarchyMetrics {
        visits: usize,
        appends: usize,
        closes: usize,
        opens: usize,
        max_stack_depth: usize,
    }

    impl HierarchyMetrics {
        fn work(&self) -> usize {
            self.visits + self.appends + self.closes + self.opens
        }
    }

    impl HierarchyObserver for HierarchyMetrics {
        fn observe(&mut self, event: HierarchyEvent) {
            match event {
                HierarchyEvent::VisitItem => self.visits += 1,
                HierarchyEvent::AppendItem => self.appends += 1,
                HierarchyEvent::CloseDirectory => self.closes += 1,
                HierarchyEvent::OpenDirectory { stack_depth } => {
                    self.opens += 1;
                    self.max_stack_depth = self.max_stack_depth.max(stack_depth);
                }
            }
        }
    }

    fn flat_file_items(count: usize) -> Vec<NavigationGuideLine> {
        (0..count)
            .map(|index| NavigationGuideLine {
                line_number: index + 2,
                indent_level: 0,
                item: FilesystemItem::File {
                    path: format!("file-{index}.txt"),
                    comment: None,
                },
            })
            .collect()
    }

    fn nested_directory_items(deepest_depth: usize) -> Vec<NavigationGuideLine> {
        (0..=deepest_depth)
            .map(|depth| NavigationGuideLine {
                line_number: depth + 2,
                indent_level: depth,
                item: FilesystemItem::Directory {
                    path: format!("directory-{depth}"),
                    comment: None,
                    children: Vec::new(),
                },
            })
            .collect()
    }

    fn flat_guide_source(count: usize) -> String {
        let mut source = String::from("<agentic-navigation-guide>\n");
        for index in 0..count {
            source.push_str(&format!("- file-{index}.txt\n"));
        }
        source.push_str("</agentic-navigation-guide>");
        source
    }

    fn median_flat_hierarchy_time(parser: &Parser, count: usize) -> Duration {
        const WARMUPS: usize = 3;
        const SAMPLES: usize = 10;

        let source = flat_guide_source(count);
        for _ in 0..WARMUPS {
            let guide = parser.parse(&source).unwrap();
            black_box(guide.items.len());
        }

        let mut samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let started = Instant::now();
            let guide = parser.parse(&source).unwrap();
            samples.push(started.elapsed());
            black_box(guide.items.len());
        }
        samples.sort_unstable();
        samples[SAMPLES / 2]
    }

    fn assert_missing_opening_error_on_line(source: &str, expected_line: usize) {
        match Parser::new().parse(source) {
            Err(crate::errors::AppError::Syntax(SyntaxError::MissingOpeningMarker { line })) => {
                assert_eq!(line, expected_line)
            }
            other => {
                panic!("expected an opening-marker error on line {expected_line}, got {other:?}")
            }
        }
    }

    fn assert_missing_closing_error_on_line(source: &str, expected_line: usize) {
        match Parser::new().parse(source) {
            Err(crate::errors::AppError::Syntax(SyntaxError::MissingClosingMarker { line })) => {
                assert_eq!(line, expected_line)
            }
            other => {
                panic!("expected a closing-marker error on line {expected_line}, got {other:?}")
            }
        }
    }

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
    fn test_rejects_audited_child_under_intervening_file() {
        let content = r#"<agentic-navigation-guide>
- a/
- b
  - c
</agentic-navigation-guide>"#;

        let result = Parser::new().parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidIndentationLevel { line: 4 }
            ))
        ));
    }

    #[test]
    fn test_rejects_direct_child_beneath_file() {
        let content = r#"<agentic-navigation-guide>
- parent.txt
  - child.txt
</agentic-navigation-guide>"#;

        let result = Parser::new().parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidIndentationLevel { line: 3 }
            ))
        ));
    }

    #[test]
    fn test_rejects_child_after_intervening_file_sequence() {
        let content = r#"<agentic-navigation-guide>
- a/
- first.txt
- second.txt
  - child.txt
</agentic-navigation-guide>"#;

        let result = Parser::new().parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidIndentationLevel { line: 5 }
            ))
        ));
    }

    #[test]
    fn test_rejects_child_beneath_placeholder() {
        let content = r#"<agentic-navigation-guide>
- a/
- ... # unlisted roots
  - child.txt
</agentic-navigation-guide>"#;

        let result = Parser::new().parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidIndentationLevel { line: 4 }
            ))
        ));
    }

    #[test]
    fn test_rejects_child_after_physical_choice_line() {
        for content in [
            r#"<agentic-navigation-guide>
- root[a,b].txt
  - child.txt
</agentic-navigation-guide>"#,
            r#"<agentic-navigation-guide>
- root[,b].txt
  - child.txt
</agentic-navigation-guide>"#,
        ] {
            let result = Parser::new().parse(content);

            assert!(matches!(
                result,
                Err(crate::errors::AppError::Syntax(
                    SyntaxError::InvalidIndentationLevel { line: 3 }
                ))
            ));
        }
    }

    #[test]
    fn test_rejects_stale_parent_after_dedent_to_file() {
        let content = r#"<agentic-navigation-guide>
- directory/
  - nested.txt
- root.txt
  - misplaced.txt
</agentic-navigation-guide>"#;

        let result = Parser::new().parse(content);

        assert!(matches!(
            result,
            Err(crate::errors::AppError::Syntax(
                SyntaxError::InvalidIndentationLevel { line: 5 }
            ))
        ));
    }

    #[test]
    fn test_rejects_indented_first_entry_and_non_space_indentation() {
        for (content, line) in [
            (
                "<agentic-navigation-guide>\n - first.txt\n</agentic-navigation-guide>",
                2,
            ),
            (
                "<agentic-navigation-guide>\n- root/\n \t- child.txt\n</agentic-navigation-guide>",
                3,
            ),
            (
                "<agentic-navigation-guide>\n- root/\n\u{a0}- child.txt\n</agentic-navigation-guide>",
                3,
            ),
        ] {
            assert!(matches!(
                Parser::new().parse(content),
                Err(crate::errors::AppError::Syntax(
                    SyntaxError::InvalidIndentationLevel { line: actual }
                )) if actual == line
            ));
        }
    }

    #[test]
    fn test_accepts_one_and_sixteen_space_indentation_units() {
        for indent in [1, MAX_INDENT_SIZE] {
            let content = format!(
                "<agentic-navigation-guide>\n- root/\n{}- child.txt\n</agentic-navigation-guide>",
                " ".repeat(indent)
            );
            let guide = Parser::new().parse(&content).unwrap();
            assert_eq!(guide.items[0].children().unwrap()[0].path(), "child.txt");
        }
    }

    #[test]
    fn test_dedent_then_reindent_uses_the_immediate_directory() {
        let content = r#"<agentic-navigation-guide>
- first/
  - nested/
    - leaf.txt
- second/
  - child.txt
- root.txt
</agentic-navigation-guide>"#;

        let guide = Parser::new().parse(content).unwrap();

        assert_eq!(
            guide
                .items
                .iter()
                .map(NavigationGuideLine::path)
                .collect::<Vec<_>>(),
            vec!["first", "second", "root.txt"]
        );
        let first_children = guide.items[0].children().unwrap();
        assert_eq!(first_children.len(), 1);
        assert_eq!(first_children[0].path(), "nested");
        assert_eq!(first_children[0].children().unwrap()[0].path(), "leaf.txt");
        assert_eq!(guide.items[1].children().unwrap()[0].path(), "child.txt");
    }

    #[test]
    fn test_wide_hierarchies_preserve_append_order() {
        const SIBLINGS: usize = 4_096;

        let mut content = String::from("<agentic-navigation-guide>\n- root/\n");
        for index in 0..SIBLINGS {
            content.push_str(&format!("  - child-{index}.txt\n"));
        }
        for index in 0..SIBLINGS {
            content.push_str(&format!("- root-{index}.txt\n"));
        }
        content.push_str("</agentic-navigation-guide>");

        let guide = Parser::new().parse(&content).unwrap();
        let children = guide.items[0].children().unwrap();
        assert_eq!(children.len(), SIBLINGS);
        assert_eq!(children[0].path(), "child-0.txt");
        assert_eq!(
            children[SIBLINGS - 1].path(),
            format!("child-{}.txt", SIBLINGS - 1)
        );
        assert_eq!(guide.items.len(), SIBLINGS + 1);
        assert_eq!(guide.items[1].path(), "root-0.txt");
        assert_eq!(
            guide.items[SIBLINGS].path(),
            format!("root-{}.txt", SIBLINGS - 1)
        );
    }

    #[test]
    fn test_hierarchy_work_is_linear_and_stack_is_bounded() {
        let parser = Parser::new();
        let mut previous_work = None;

        for count in [10_000, 20_000, 40_000] {
            let mut metrics = HierarchyMetrics::default();
            let result = parser
                .build_hierarchy_with_observer(flat_file_items(count), &mut metrics)
                .unwrap();

            assert_eq!(result.len(), count);
            assert_eq!(metrics.visits, count);
            assert_eq!(metrics.appends, count);
            assert_eq!(metrics.closes, 0);
            assert_eq!(metrics.opens, 0);
            assert_eq!(metrics.work(), count * 2);
            if let Some(previous) = previous_work {
                assert_eq!(metrics.work(), previous * 2);
            }
            previous_work = Some(metrics.work());
        }

        let mut depth_metrics = HierarchyMetrics::default();
        parser
            .build_hierarchy_with_observer(
                nested_directory_items(MAX_LOGICAL_DEPTH),
                &mut depth_metrics,
            )
            .unwrap();
        assert_eq!(depth_metrics.max_stack_depth, MAX_LOGICAL_DEPTH + 1);
        assert_eq!(depth_metrics.visits, MAX_LOGICAL_DEPTH + 1);
        assert_eq!(depth_metrics.opens, MAX_LOGICAL_DEPTH + 1);
        assert_eq!(depth_metrics.closes, MAX_LOGICAL_DEPTH + 1);
        assert_eq!(depth_metrics.appends, MAX_LOGICAL_DEPTH + 1);

        let mut over_limit_metrics = HierarchyMetrics::default();
        assert!(parser
            .build_hierarchy_with_observer(
                nested_directory_items(MAX_LOGICAL_DEPTH + 1),
                &mut over_limit_metrics,
            )
            .is_err());
        assert_eq!(over_limit_metrics.max_stack_depth, MAX_LOGICAL_DEPTH + 1);
    }

    #[test]
    #[ignore = "manual release-mode hierarchy scaling evidence"]
    fn benchmark_flat_hierarchy_scaling() {
        let parser = Parser::new();
        let mut previous: Option<(usize, Duration)> = None;

        for count in [10_000, 20_000, 40_000, 80_000, 120_000] {
            let median = median_flat_hierarchy_time(&parser, count);
            eprintln!("flat hierarchy {count}: median {median:?}");

            if let Some((previous_count, previous_median)) = previous {
                let ratio = median.as_secs_f64() / previous_median.as_secs_f64();
                assert!(
                    ratio <= 2.5,
                    "flat hierarchy {previous_count}->{count} scaled by {ratio:.3}x"
                );
            }
            previous = Some((count, median));
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
    fn test_rejects_malformed_opening_marker_candidates() {
        for (name, opening) in [
            (
                "concatenated attribute",
                "<agentic-navigation-guideignore=true>",
            ),
            ("prefix typo", "<agentic-navigation-guideTYPO>"),
            ("plural name", "<agentic-navigation-guides>"),
            (
                "unknown attribute",
                "<agentic-navigation-guide mode=example>",
            ),
            (
                "wrong-case attribute",
                "<agentic-navigation-guide IGNORE=true>",
            ),
            (
                "duplicate attribute",
                "<agentic-navigation-guide ignore=true ignore=true>",
            ),
            ("false attribute", "<agentic-navigation-guide ignore=false>"),
            ("wrong-case value", "<agentic-navigation-guide ignore=TRUE>"),
            (
                "single quoted attribute",
                "<agentic-navigation-guide ignore='true'>",
            ),
            (
                "unquoted non-boolean attribute",
                "<agentic-navigation-guide ignore=yes>",
            ),
            ("valueless attribute", "<agentic-navigation-guide ignore=>"),
            (
                "whitespace without attribute",
                "<agentic-navigation-guide \t>",
            ),
            (
                "unterminated quote",
                "<agentic-navigation-guide ignore=\"true>",
            ),
            (
                "trailing quoted text",
                "<agentic-navigation-guide ignore=\"true\"extra>",
            ),
            (
                "same-line trailing text",
                "<agentic-navigation-guide> trailing",
            ),
            (
                "attributed marker without terminator",
                "<agentic-navigation-guide ignore=true",
            ),
            ("incomplete marker", "<agentic-navigation-guide"),
        ] {
            let source = format!("{opening}\n- missing.txt\n</agentic-navigation-guide>");
            match Parser::new().parse(&source) {
                Err(crate::errors::AppError::Syntax(SyntaxError::MissingOpeningMarker {
                    line: 1,
                })) => {}
                other => panic!("{name} should be rejected, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_rejects_malformed_closing_marker_candidates() {
        for (name, closing) in [
            (
                "closing attribute",
                "</agentic-navigation-guide ignore=true>",
            ),
            ("closing suffix", "</agentic-navigation-guides>"),
            ("closing typo", "</agentic-navigation-guideTYPO>"),
            ("incomplete closing marker", "</agentic-navigation-guide"),
        ] {
            let source = format!("<agentic-navigation-guide>\n- file.txt\n{closing}");
            match Parser::new().parse(&source) {
                Err(crate::errors::AppError::Syntax(SyntaxError::MissingClosingMarker {
                    line: 3,
                })) => {}
                other => panic!("{name} should be rejected, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_marker_substrings_are_markdown_but_candidates_fail_globally() {
        let source = r#"Prose mentions <agentic-navigation-guideTYPO>.
<agentic-navigation-guide>
- file.txt # mentions </agentic-navigation-guideTYPO>
</agentic-navigation-guide>
More prose mentions </agentic-navigation-guides>."#;
        let guide = Parser::new().parse(source).unwrap();
        assert_eq!(guide.items.len(), 1);
        assert!(guide
            .prologue
            .as_deref()
            .is_some_and(|text| text.contains("<agentic-navigation-guideTYPO>")));
        assert!(guide
            .epilogue
            .as_deref()
            .is_some_and(|text| text.contains("</agentic-navigation-guides>")));

        assert_missing_opening_error_on_line(
            "<agentic-navigation-guideTYPO>\n<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide>",
            1,
        );
        assert_missing_closing_error_on_line(
            "<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide>\n</agentic-navigation-guideTYPO>",
            4,
        );
    }

    #[test]
    fn test_rejects_bom_prefixed_marker_before_later_valid_block() {
        for invalid_prologue in [
            "\u{feff}<agentic-navigation-guide>",
            "\u{feff}\u{feff}<agentic-navigation-guide>",
            "\u{feff} \t\u{feff} <agentic-navigation-guide>",
        ] {
            let source = format!(
                "{invalid_prologue}\n<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide>"
            );
            assert_missing_opening_error_on_line(&source, 1);
        }

        assert_missing_closing_error_on_line(
            "<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide>\n\u{feff}\u{feff}</agentic-navigation-guide>",
            4,
        );
    }

    #[test]
    fn test_rejects_non_horizontal_outer_marker_whitespace() {
        assert_missing_opening_error_on_line(
            "\u{00a0}<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide>",
            3,
        );
        assert_missing_opening_error_on_line(
            "<agentic-navigation-guide>\u{00a0}\n- file.txt\n</agentic-navigation-guide>",
            1,
        );
    }

    #[test]
    fn test_rejects_malformed_marker_candidate_inside_ignored_envelope() {
        assert_missing_opening_error_on_line(
            "<agentic-navigation-guide ignore=true>\n<agentic-navigation-guideTYPO>\n</agentic-navigation-guide>",
            2,
        );
    }

    #[test]
    fn test_accepts_every_approved_marker_whitespace_and_quoting_form() {
        for opening in [
            "<agentic-navigation-guide>",
            "<agentic-navigation-guide ignore=true>",
            "<agentic-navigation-guide ignore=\"true\">",
            "<agentic-navigation-guide   ignore = true  >",
            "<agentic-navigation-guide\tignore\t=\t\"true\"\t>",
        ] {
            let source = format!(" \t{opening}\t \n- file.txt\n\t</agentic-navigation-guide> ");
            let guide = Parser::new().parse(&source).unwrap();
            assert_eq!(guide.ignore, opening != "<agentic-navigation-guide>");
            assert_eq!(guide.items.len(), usize::from(!guide.ignore));

            let crlf = source.replace('\n', "\r\n");
            let crlf_guide = Parser::new().parse(&crlf).unwrap();
            assert_eq!(crlf_guide.ignore, guide.ignore);
            assert_eq!(crlf_guide.items, guide.items);
        }
    }

    #[test]
    fn test_rejects_lone_carriage_return_anywhere_in_document() {
        for (source, line) in [
            (
                "prologue\rsuffix\n<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide>",
                1,
            ),
            (
                "<agentic-navigation-guide>\n- file.txt # before\rafter\n</agentic-navigation-guide>",
                2,
            ),
            (
                "<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide>\nepilogue\rsuffix",
                4,
            ),
            (
                "<agentic-navigation-guide\r>\n- file.txt\n</agentic-navigation-guide>",
                1,
            ),
            (
                "<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide\r>",
                3,
            ),
            (
                "<agentic-navigation-guide ignore=true>\nopaque\rbody\n</agentic-navigation-guide>",
                2,
            ),
        ] {
            assert_missing_opening_error_on_line(source, line);
        }
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
    fn test_parse_whitespace_only_comment_normalizes_to_none() {
        let content = r#"<agentic-navigation-guide>
- src/ #    
- ... # 	
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 2);
        assert_eq!(guide.items[0].comment(), None);
        assert_eq!(guide.items[1].comment(), None);
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
        assert!(guide.items.is_empty());
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
        assert!(guide.items.is_empty());
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
        assert!(guide.items.is_empty());
    }

    #[test]
    fn test_parse_ignore_attribute_rejects_mixed_attributes() {
        let content = r#"<agentic-navigation-guide foo=bar ignore="true" notignore=true>
- src/
  - main.rs
</agentic-navigation-guide>"#;

        assert_missing_opening_error_on_line(content, 1);
    }

    #[test]
    fn test_parse_ignore_attribute_rejects_partial_key() {
        let content = r#"<agentic-navigation-guide notignore=true>
- src/
  - main.rs
</agentic-navigation-guide>"#;

        assert_missing_opening_error_on_line(content, 1);
    }

    #[test]
    fn test_parse_ignore_attribute_rejects_false_value() {
        let content = r#"<agentic-navigation-guide ignore=false>
- src/
  - main.rs
</agentic-navigation-guide>"#;

        assert_missing_opening_error_on_line(content, 1);
    }

    #[test]
    fn test_parse_ignore_attribute_rejects_unterminated_quote() {
        let content = r#"<agentic-navigation-guide ignore="true>
- src/
  - main.rs
</agentic-navigation-guide>"#;

        assert_missing_opening_error_on_line(content, 1);
    }

    #[test]
    fn test_parse_ignore_attribute_with_spaces_around_equals() {
        let content = r#"<agentic-navigation-guide ignore = "true">
- src/
  - main.rs
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();
        assert!(guide.ignore);
        assert!(guide.items.is_empty());
    }

    #[test]
    fn test_parse_ignore_attribute_rejects_duplicate_keys() {
        let content = r#"<agentic-navigation-guide ignore=false ignore=true>
- src/
  - main.rs
</agentic-navigation-guide>"#;

        assert_missing_opening_error_on_line(content, 1);
    }

    #[test]
    fn test_parse_ignore_attribute_rejects_malformed_quoted_value() {
        let content = r#"<agentic-navigation-guide ignore="tr"ue">
- src/
  - main.rs
</agentic-navigation-guide>"#;

        assert_missing_opening_error_on_line(content, 1);
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
    fn test_parse_wildcard_with_long_choice_list() {
        let choices: Vec<String> = (0..128).map(|idx| format!(".v{idx}")).collect();
        let content = format!(
            "<agentic-navigation-guide>\n- config[{}].toml\n</agentic-navigation-guide>",
            choices.join(", ")
        );

        let parser = Parser::new();
        let guide = parser.parse(&content).unwrap();

        assert_eq!(guide.items.len(), 128);
        assert_eq!(guide.items[0].path(), "config.v0.toml");
        assert_eq!(guide.items[127].path(), "config.v127.toml");
    }

    #[test]
    fn test_choice_limit_rejects_before_tokenizing_unbounded_alternatives() {
        let content = format!("{}\\", "a,".repeat(MAX_CHOICE_ALTERNATIVES));
        let result = Parser::parse_choice_block(&content, "path", 1);

        let Err(crate::errors::AppError::Syntax(SyntaxError::InvalidWildcardSyntax {
            message,
            ..
        })) = result
        else {
            panic!("an over-limit choice must fail with a wildcard syntax error")
        };
        assert!(
            message.contains("between 2 and 256"),
            "the count limit must win before the trailing token is parsed: {message}"
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
- file[\"test\\\"quote\", plain].txt
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 2);
        assert_eq!(guide.items[0].path(), r#"file"test\"quote".txt"#);
        assert_eq!(guide.items[1].path(), "fileplain.txt");
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
- file["a \"b\" c", plain].txt
</agentic-navigation-guide>"#;

        let parser = Parser::new();
        let guide = parser.parse(content).unwrap();

        assert_eq!(guide.items.len(), 2);
        // Note: Escaped quotes inside a quoted string are processed
        assert_eq!(guide.items[0].path(), r#"filea "b" c.txt"#);
        assert_eq!(guide.items[1].path(), "fileplain.txt");
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
