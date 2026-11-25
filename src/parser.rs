//! Parser for navigation guide markdown files

use crate::errors::{Result, SyntaxError};
use crate::path_codec::render_utf8_component;
use crate::proven::choice::{ChoiceError, MAX_CHOICE_ALTERNATIVES, MIN_CHOICE_ALTERNATIVES};
use crate::proven::document::{self, EnvelopeError, EnvelopeScan};
use crate::proven::entry::{self, EntryErrorKind, EntryFailure, EntryShape, ParsedEntry};
use crate::proven::hierarchy;
use crate::proven::layout::{self, IndentState, LayoutError};
use crate::proven::lines::{self, split_lines};
use crate::proven::marker::{self, MarkerClass};
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};

const MAX_LOGICAL_DEPTH: usize = layout::MAX_LOGICAL_DEPTH;

#[derive(Clone, Copy)]
enum MarkerLine {
    Ordinary,
    Opening { ignore: bool },
    Closing,
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
pub(crate) struct Parser;

/// Map a verified envelope-scan error to its diagnostic.
fn envelope_syntax_error(error: EnvelopeError) -> SyntaxError {
    match error {
        EnvelopeError::MultipleGuideBlocks { line } => SyntaxError::MultipleGuideBlocks { line },
        EnvelopeError::MissingOpeningMarker { line } => SyntaxError::MissingOpeningMarker { line },
        EnvelopeError::MissingClosingMarker { line } => SyntaxError::MissingClosingMarker { line },
    }
}

/// The `SyntaxError::InvalidWildcardSyntax` message for a verified
/// choice-list error.
fn choice_message(error: ChoiceError) -> String {
    let message = match error {
        ChoiceError::IncompleteEscape => "incomplete escape sequence",
        ChoiceError::InvalidBareEscape => "invalid escape in bare path",
        ChoiceError::MultipleBlocks => "multiple wildcard choice blocks are not supported",
        ChoiceError::UnmatchedClosingBracket => "unmatched closing bracket",
        ChoiceError::UnescapedQuote => "unescaped quote in bare path",
        ChoiceError::UnterminatedBlock => "unterminated wildcard choice block",
        ChoiceError::InvalidUnquotedEscape => "invalid escape in unquoted choice",
        ChoiceError::UnescapedBracket => "unescaped bracket in unquoted choice",
        ChoiceError::MisplacedQuote => "quote must begin a choice alternative",
        ChoiceError::InvalidQuotedEscape => "invalid escape in quoted choice",
        ChoiceError::QuotedTrailingText => "quoted choice must end before the separator",
        ChoiceError::UnterminatedQuote => "unterminated quoted string in wildcard choices",
        ChoiceError::EmptyBlock => "choice block cannot be empty",
        ChoiceError::AlternativeCount => {
            return format!(
                "choice list must contain between {MIN_CHOICE_ALTERNATIVES} and \
                 {MAX_CHOICE_ALTERNATIVES} alternatives"
            );
        }
        ChoiceError::InvalidExpansion => "choice expansions must be valid regular-file paths",
        ChoiceError::NotSiblings => "choice expansions must be sibling files",
        ChoiceError::DuplicateExpansion => "choice expansions must be unique",
    };
    message.to_string()
}

/// Map a verified entry-text error on `line` to its diagnostic. The kernel
/// supplies the exact text each diagnostic renders: the whole entry for a
/// malformed quoted path, the bare expression for a choice error, the
/// decoded path for a bare-path rule, or empty text.
fn entry_syntax_error(failure: EntryFailure, line: usize) -> SyntaxError {
    let path = render_utf8_component(&failure.subject);
    match failure.kind {
        EntryErrorKind::InvalidPath => SyntaxError::InvalidPathFormat { line, path },
        EntryErrorKind::SpecialDirectory => SyntaxError::InvalidSpecialDirectory { line, path },
        EntryErrorKind::Choice(error) => SyntaxError::InvalidWildcardSyntax {
            line,
            path,
            message: choice_message(error),
        },
    }
}

/// Map a verified body-layout error on `line` to its diagnostic.
fn layout_syntax_error(error: LayoutError, line: usize) -> SyntaxError {
    match error {
        LayoutError::BlankLine => SyntaxError::BlankLineInGuide { line },
        LayoutError::InvalidListFormat => SyntaxError::InvalidListFormat { line },
        LayoutError::InvalidIndentationLevel => SyntaxError::InvalidIndentationLevel { line },
    }
}

impl Parser {
    /// Create a new parser instance
    pub(crate) fn new() -> Self {
        Self
    }

    /// Parse navigation guide content from a markdown string.
    ///
    /// The exact document envelope and marker-candidate scan are always
    /// validated. When the opening marker contains `ignore=true`, the body is
    /// opaque and may be empty; the returned guide has no parsed items.
    pub(crate) fn parse(&self, content: &str) -> Result<NavigationGuide> {
        // Find the guide block
        let (prologue, body_lines, epilogue, line_offset, ignore) =
            self.extract_guide_block(content)?;

        // The exact envelope and global marker-candidate scan above always
        // run. Once that scan recognizes ignore=true, however, the body is
        // intentionally opaque UTF-8 text and may be empty.
        let items = if ignore {
            Vec::new()
        } else {
            self.parse_guide_content(&body_lines, line_offset)?
        };

        Ok(NavigationGuide {
            items,
            prologue,
            epilogue,
            ignore,
        })
    }

    /// Extract the guide block from the markdown content: the joined
    /// prologue, the physical body lines, the joined epilogue, the number of
    /// lines before the body, and the ignore flag.
    #[allow(clippy::type_complexity)]
    fn extract_guide_block<'a>(
        &self,
        content: &'a str,
    ) -> Result<(Option<String>, Vec<&'a str>, Option<String>, usize, bool)> {
        Self::validate_line_endings(content)?;

        // The Verus-verified splitter is exactly `str::lines` (see
        // `crate::proven::lines`); after the lone-CR check above, no line
        // contains CR or LF.
        let lines = split_lines(content);

        // Find and validate opening/closing markers across the full document.
        // The Verus-verified envelope scan requires exactly one opening marker
        // and exactly one closing marker after it, and reports the first
        // offending line (see `crate::proven::document`).
        let mut scan = EnvelopeScan::BeforeOpening;
        for (idx, line) in lines.iter().enumerate() {
            let line_number = idx + 1;
            let class = match Self::classify_marker_line(line, line_number)? {
                MarkerLine::Ordinary => MarkerClass::Ordinary,
                MarkerLine::Opening { ignore: false } => MarkerClass::Opening,
                MarkerLine::Opening { ignore: true } => MarkerClass::IgnoredOpening,
                MarkerLine::Closing => MarkerClass::Closing,
            };
            scan = document::scan_marker_line(scan, line_number, class)
                .map_err(envelope_syntax_error)?;
        }
        let envelope =
            document::finish_envelope(scan, lines.len()).map_err(envelope_syntax_error)?;

        // The scan reports 1-based marker lines with opening < closing.
        let start = envelope.opening_line - 1;
        let end = envelope.closing_line - 1;
        let ignore = envelope.ignore;

        // Extract prologue, guide content, and epilogue
        let prologue = if start > 0 {
            Some(lines[..start].join("\n"))
        } else {
            None
        };

        let body_lines = lines[start + 1..end].to_vec();

        let epilogue = if end + 1 < lines.len() {
            Some(lines[end + 1..].join("\n"))
        } else {
            None
        };

        // Calculate line offset: prologue lines + opening tag line
        let line_offset = start + 1;

        Ok((prologue, body_lines, epilogue, line_offset, ignore))
    }

    /// Reject lone carriage returns before line-oriented parsing.
    ///
    /// The scan is the Verus-verified
    /// [`crate::proven::document::first_lone_cr_line`] kernel.
    fn validate_line_endings(content: &str) -> Result<()> {
        match document::first_lone_cr_line(content.as_bytes()) {
            Some(line) => Err(SyntaxError::MissingOpeningMarker { line }.into()),
            None => Ok(()),
        }
    }

    /// Classify a complete line using the exact v0.2 marker grammar.
    ///
    /// The decision is made by the Verus-verified
    /// [`crate::proven::marker::classify_marker_line`] kernel, whose
    /// specification restates the contract marker productions.
    fn classify_marker_line(line: &str, line_number: usize) -> Result<MarkerLine> {
        match marker::classify_marker_line(line.as_bytes()) {
            MarkerClass::Ordinary => Ok(MarkerLine::Ordinary),
            MarkerClass::Opening => Ok(MarkerLine::Opening { ignore: false }),
            MarkerClass::IgnoredOpening => Ok(MarkerLine::Opening { ignore: true }),
            MarkerClass::Closing => Ok(MarkerLine::Closing),
            MarkerClass::MalformedOpening => {
                Err(SyntaxError::MissingOpeningMarker { line: line_number }.into())
            }
            MarkerClass::MalformedClosing => {
                Err(SyntaxError::MissingClosingMarker { line: line_number }.into())
            }
        }
    }

    /// Parse the physical body lines into navigation guide lines
    fn parse_guide_content(
        &self,
        body_lines: &[&str],
        line_offset: usize,
    ) -> Result<Vec<NavigationGuideLine>> {
        // A body with no line, or only whitespace lines, is empty. Verus
        // proves this per-line check equal to the whitespace-only test of the
        // body joined with LF (see `crate::proven::lines::is_blank_body`).
        if lines::is_blank_body(body_lines) {
            return Err(SyntaxError::EmptyGuideBlock.into());
        }

        let mut items = Vec::new();
        let mut indentation = IndentState::new();

        // Every physical body line, an empty last line included, reaches the
        // layout kernel, so a blank line before the closing marker is
        // rejected by its blank-line rule.
        for (idx, line) in body_lines.iter().enumerate() {
            // Calculate the actual line number in the file
            let line_number = idx + 1 + line_offset;

            // Blank lines, the list-item shape, space-only indentation, the
            // indentation unit, the depth bound, and parentage are decided by
            // the Verus-verified layout kernel (see `crate::proven::layout`).
            let entry = layout::layout_entry_line(indentation, line)
                .map_err(|error| layout_syntax_error(error, line_number))?;
            let indent_level = entry.depth;
            let content = entry.content;

            // The entry text (placeholder, quoted path, bare path, or choice
            // list, and its comment) is decided by the Verus-verified entry
            // kernel (see `crate::proven::entry`).
            let (parsed_items, line_can_own_children) =
                Self::parse_entry_items(content, line_number, indent_level == 0)?;

            for item in parsed_items {
                items.push(NavigationGuideLine {
                    line_number,
                    indent_level,
                    item,
                });
            }

            indentation = layout::advance(&entry, line_can_own_children);
        }

        // Build the hierarchy
        let hierarchical_items = self.build_hierarchy(items)?;

        Ok(hierarchical_items)
    }

    /// Parse one entry's text into the items it contributes and whether its
    /// line may own indented children.
    ///
    /// The decision is the Verus-verified
    /// [`crate::proven::entry::parse_entry`] kernel, whose specification
    /// restates the contract's entry, path-expression, choice-list, and
    /// placeholder grammar; this glue only builds the items and maps a
    /// rejection to its diagnostic.
    fn parse_entry_items(
        content: &str,
        line_number: usize,
        at_root: bool,
    ) -> Result<(Vec<FilesystemItem>, bool)> {
        let entry = entry::parse_entry(content, at_root)
            .map_err(|failure| entry_syntax_error(failure, line_number))?;
        let can_own_children = entry.can_own_children();
        let ParsedEntry { shape, comment } = entry;
        let items = match shape {
            EntryShape::Placeholder => vec![FilesystemItem::Placeholder { comment }],
            EntryShape::Single {
                path,
                directory: true,
            } => vec![FilesystemItem::Directory {
                path,
                comment,
                children: Vec::new(),
            }],
            EntryShape::Single {
                path,
                directory: false,
            } => vec![FilesystemItem::File { path, comment }],
            EntryShape::Choice(paths) => paths
                .into_iter()
                .map(|path| FilesystemItem::File {
                    path,
                    comment: comment.clone(),
                })
                .collect(),
        };
        Ok((items, can_own_children))
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
            // The Verus-verified plan rejects a depth above 256 or beyond the
            // open directories, and otherwise closes exactly the directories
            // deeper than the item, so it is attached to its preorder parent
            // (see `crate::proven::hierarchy`).
            let Some(closes) = hierarchy::plan_item(open_directories.len(), item.indent_level)
            else {
                return Err(SyntaxError::InvalidIndentationLevel {
                    line: item.line_number,
                }
                .into());
            };

            for _ in 0..closes {
                Self::close_directory(&mut roots, &mut open_directories, observer)?;
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
    use crate::errors::AppError;
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
        for indent in [1, layout::MAX_INDENT_UNIT] {
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

    /// Every observer event, in order.
    #[derive(Default)]
    struct HierarchyEventLog(Vec<(u8, usize)>);

    impl HierarchyObserver for HierarchyEventLog {
        fn observe(&mut self, event: HierarchyEvent) {
            self.0.push(match event {
                HierarchyEvent::VisitItem => (0, 0),
                HierarchyEvent::AppendItem => (1, 0),
                HierarchyEvent::CloseDirectory => (2, 0),
                HierarchyEvent::OpenDirectory { stack_depth } => (3, stack_depth),
            });
        }
    }

    /// The hierarchy loop as it was before the verified plan.
    fn reference_build_hierarchy<O: HierarchyObserver>(
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
                Parser::close_directory(&mut roots, &mut open_directories, observer)?;
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
                Parser::append_hierarchy_item(&mut roots, &mut open_directories, item, observer)?;
            }
        }
        while !open_directories.is_empty() {
            Parser::close_directory(&mut roots, &mut open_directories, observer)?;
        }
        Ok(roots)
    }

    /// Items whose path is their index, at the given depths and kinds.
    fn indexed_items(shape: &[(usize, bool)]) -> Vec<NavigationGuideLine> {
        shape
            .iter()
            .enumerate()
            .map(|(index, &(depth, directory))| NavigationGuideLine {
                line_number: index + 2,
                indent_level: depth,
                item: if directory {
                    FilesystemItem::Directory {
                        path: index.to_string(),
                        comment: None,
                        children: Vec::new(),
                    }
                } else {
                    FilesystemItem::File {
                        path: index.to_string(),
                        comment: None,
                    }
                },
            })
            .collect()
    }

    /// The first item that breaks the contract's depth rules, if any.
    fn first_depth_rule_violation(shape: &[(usize, bool)]) -> Option<usize> {
        shape.iter().enumerate().position(|(index, &(depth, _))| {
            let within_bound = depth <= MAX_LOGICAL_DEPTH;
            let follows = match index.checked_sub(1).map(|previous| shape[previous]) {
                None => depth == 0,
                Some((previous, directory)) => {
                    depth <= previous || (depth == previous + 1 && directory)
                }
            };
            !(within_bound && follows)
        })
    }

    /// Record each node's parent index (`None` for roots), checking that
    /// children appear in source order.
    fn record_parents(
        nodes: &[NavigationGuideLine],
        parent: Option<usize>,
        parents: &mut Vec<Option<Option<usize>>>,
    ) {
        let mut previous = None;
        for node in nodes {
            let index: usize = node.path().parse().unwrap();
            assert!(previous < Some(index), "children out of source order");
            previous = Some(index);
            assert_eq!(parents[index], None, "item {index} placed twice");
            parents[index] = Some(parent);
            if let Some(children) = node.children() {
                record_parents(children, Some(index), parents);
            }
        }
    }

    #[test]
    fn test_hierarchy_plan_matches_previous_loop_and_preorder_parents() {
        const DEPTHS: usize = 3;
        let options: Vec<(usize, bool)> = (0..DEPTHS)
            .flat_map(|depth| [(depth, false), (depth, true)])
            .collect();
        let parser = Parser::new();
        let mut shape = Vec::new();
        for len in 0..=7u32 {
            for mut code in 0..options.len().pow(len) {
                shape.clear();
                for _ in 0..len {
                    shape.push(options[code % options.len()]);
                    code /= options.len();
                }
                let mut log = HierarchyEventLog::default();
                let result = parser.build_hierarchy_with_observer(indexed_items(&shape), &mut log);
                let mut reference_log = HierarchyEventLog::default();
                let reference =
                    reference_build_hierarchy(indexed_items(&shape), &mut reference_log);
                assert_eq!(format!("{result:?}"), format!("{reference:?}"), "{shape:?}");
                assert_eq!(log.0, reference_log.0, "{shape:?}");

                match (result, first_depth_rule_violation(&shape)) {
                    (
                        Err(AppError::Syntax(SyntaxError::InvalidIndentationLevel { line })),
                        Some(index),
                    ) => {
                        assert_eq!(line, index + 2, "{shape:?}");
                    }
                    (Ok(roots), None) => {
                        let mut parents = vec![None; shape.len()];
                        record_parents(&roots, None, &mut parents);
                        for (index, recorded) in parents.into_iter().enumerate() {
                            let depth = shape[index].0;
                            // The preorder parent: the latest earlier item one
                            // level shallower.
                            let expected = depth.checked_sub(1).map(|level| {
                                (0..index).rev().find(|&p| shape[p].0 == level).unwrap()
                            });
                            assert_eq!(recorded, Some(expected), "{shape:?} item {index}");
                        }
                    }
                    (other, violation) => {
                        panic!("{shape:?}: {other:?} but first violation {violation:?}")
                    }
                }
            }
        }
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
    fn test_malformed_marker_glue_matches_verified_envelope_step() {
        // `Parser::classify_marker_line` returns malformed-marker errors before
        // the verified envelope fold sees the line. This glue must report
        // exactly what `document::scan_marker_line` reports for that class in
        // every scan state, so the envelope theorems describe shipped output.
        let states = [
            EnvelopeScan::BeforeOpening,
            EnvelopeScan::InBlock {
                opening_line: 1,
                ignore: false,
            },
            EnvelopeScan::AfterClosing {
                opening_line: 1,
                closing_line: 2,
                ignore: true,
            },
        ];
        for (line, class) in [
            (
                "<agentic-navigation-guide ignore=false>",
                MarkerClass::MalformedOpening,
            ),
            (
                "  </agentic-navigation-guide >",
                MarkerClass::MalformedClosing,
            ),
            (
                "\u{feff}<agentic-navigation-guide>",
                MarkerClass::MalformedOpening,
            ),
        ] {
            assert_eq!(marker::classify_marker_line(line.as_bytes()), class);
            let glue = Parser::classify_marker_line(line, 7)
                .err()
                .expect("malformed marker lines are rejected");
            for state in states {
                let kernel = document::scan_marker_line(state, 7, class)
                    .expect_err("the envelope step rejects malformed markers");
                assert_eq!(
                    format!("{glue:?}"),
                    format!("{:?}", AppError::from(envelope_syntax_error(kernel))),
                    "{line:?} in {state:?}"
                );
            }
        }
    }

    #[test]
    fn test_rejects_blank_body_line_before_closing_marker() {
        // The contract forbids blank body lines; the last body line used to
        // be lost when the body was re-split with `str::lines`.
        for (content, line) in [
            (
                "<agentic-navigation-guide>\n- a\n\n</agentic-navigation-guide>\n",
                3,
            ),
            (
                "<agentic-navigation-guide>\r\n- a\r\n\r\n</agentic-navigation-guide>\r\n",
                3,
            ),
            (
                "# Title\n<agentic-navigation-guide>\n- a\n- b\n\n</agentic-navigation-guide>",
                5,
            ),
        ] {
            let error = Parser::new()
                .parse(content)
                .expect_err("a trailing blank body line must be rejected");
            assert!(
                matches!(
                    error,
                    AppError::Syntax(SyntaxError::BlankLineInGuide { line: reported })
                        if reported == line
                ),
                "{content:?} produced {error:?}"
            );
        }

        // A whitespace-only body is still the empty-guide error.
        for content in [
            "<agentic-navigation-guide>\n\n</agentic-navigation-guide>",
            "<agentic-navigation-guide>\n  \n\n</agentic-navigation-guide>",
        ] {
            assert!(matches!(
                Parser::new().parse(content),
                Err(AppError::Syntax(SyntaxError::EmptyGuideBlock))
            ));
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
    fn test_expand_wildcard_with_escapes_and_quotes() {
        let (items, can_own_children) =
            Parser::parse_entry_items("data[\"with , comma\", \\,space, \"literal []\"]", 1, true)
                .unwrap();

        let paths: Vec<&str> = items.iter().map(item_path).collect();
        assert_eq!(
            paths,
            vec!["datawith , comma", "data,space", "dataliteral []"]
        );
        assert!(!can_own_children);
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
        let (items, _) =
            Parser::parse_entry_items(r#"docs/issue\#123.md#ticket"#, 1, true).unwrap();
        assert_eq!(
            items,
            vec![FilesystemItem::File {
                path: "docs/issue#123.md".to_string(),
                comment: Some("ticket".to_string()),
            }]
        );

        let (items, _) = Parser::parse_entry_items(r#"docs/issue\#123.md"#, 1, true).unwrap();
        assert_eq!(
            items,
            vec![FilesystemItem::File {
                path: "docs/issue#123.md".to_string(),
                comment: None,
            }]
        );
    }

    fn item_path(item: &FilesystemItem) -> &str {
        match item {
            FilesystemItem::File { path, .. } | FilesystemItem::Directory { path, .. } => path,
            FilesystemItem::Placeholder { .. } => "...",
        }
    }

    #[test]
    fn test_entry_diagnostics_render_the_text_each_error_names() {
        fn error(content: &str) -> SyntaxError {
            match Parser::parse_entry_items(content, 7, true) {
                Err(crate::errors::AppError::Syntax(error)) => error,
                other => panic!("{content:?} must be rejected: {other:?}"),
            }
        }

        // A malformed quoted path renders the whole entry text.
        assert_eq!(
            error(r#""a\q" # c"#),
            SyntaxError::InvalidPathFormat {
                line: 7,
                path: render_utf8_component(r#""a\q" # c"#),
            }
        );
        // An empty path renders empty text.
        assert_eq!(
            error(r#""""#),
            SyntaxError::InvalidPathFormat {
                line: 7,
                path: render_utf8_component(""),
            }
        );
        // Bare-path rules render the decoded path without its marker.
        assert_eq!(
            error(r"a/../b\ c/"),
            SyntaxError::InvalidSpecialDirectory {
                line: 7,
                path: render_utf8_component("a/../b c"),
            }
        );
        assert_eq!(
            error("C:/x"),
            SyntaxError::InvalidPathFormat {
                line: 7,
                path: render_utf8_component("C:/x"),
            }
        );
        // Choice errors render the comment-stripped, right-trimmed expression.
        assert_eq!(
            error("x[a, a]  # c"),
            SyntaxError::InvalidWildcardSyntax {
                line: 7,
                path: render_utf8_component("x[a, a]"),
                message: "choice expansions must be unique".to_string(),
            }
        );
    }

    #[test]
    fn test_every_choice_error_has_its_message() {
        for (content, message) in [
            (r"a\", "incomplete escape sequence"),
            (r"a\q", "invalid escape in bare path"),
            (
                "a[b, c][d, e]",
                "multiple wildcard choice blocks are not supported",
            ),
            ("a]", "unmatched closing bracket"),
            ("a\"b", "unescaped quote in bare path"),
            ("a[b, c", "unterminated wildcard choice block"),
            (r"a[b\q, c]", "invalid escape in unquoted choice"),
            (
                "a[\"b\"[, c]",
                "quoted choice must end before the separator",
            ),
            ("a[b\"c\", d]", "quote must begin a choice alternative"),
            ("a[\"[\", b[]", "unescaped bracket in unquoted choice"),
            (r#"a["\q", b]"#, "invalid escape in quoted choice"),
            ("a[, ]", "choice block cannot be empty"),
            (
                "a[b]",
                "choice list must contain between 2 and 256 alternatives",
            ),
            (
                "a[b, c/]",
                "choice expansions must be valid regular-file paths",
            ),
            ("a[/b, c]", "choice expansions must be sibling files"),
            ("a[b, b]", "choice expansions must be unique"),
        ] {
            match Parser::parse_entry_items(content, 1, true) {
                Err(crate::errors::AppError::Syntax(SyntaxError::InvalidWildcardSyntax {
                    message: actual,
                    ..
                })) => assert_eq!(actual, message, "{content:?}"),
                other => panic!("{content:?} must report {message:?}: {other:?}"),
            }
        }
    }

    /// Deterministic xorshift generator for the differential corpus.
    struct Corpus(u64);

    impl Corpus {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: usize) -> usize {
            (self.next() % bound as u64) as usize
        }

        fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
            items[self.below(items.len())]
        }
    }

    /// Outcome classes seen by a differential run, so a corpus that stops
    /// reaching some behavior fails loudly.
    #[derive(Default)]
    struct Tally {
        outcomes: std::collections::BTreeSet<String>,
    }

    fn assert_matches_legacy(content: &str, tally: &mut Tally) {
        for at_root in [true, false] {
            let expected =
                crate::proven::entry::legacy_oracle::legacy_entry_items(content, 3, at_root);
            let actual = match Parser::parse_entry_items(content, 3, at_root) {
                Ok(result) => Ok(result),
                Err(crate::errors::AppError::Syntax(error)) => Err(error),
                Err(other) => panic!("unexpected non-syntax error {other:?}"),
            };
            assert_eq!(actual, expected, "entry {content:?} at_root={at_root}");
            tally.outcomes.insert(match actual {
                Ok((items, owns)) => {
                    let kind = match items.first() {
                        Some(FilesystemItem::Placeholder { .. }) => "placeholder",
                        Some(FilesystemItem::Directory { .. }) => "directory",
                        _ if items.len() > 1 => "choice",
                        _ => "file",
                    };
                    let comment = match items.first() {
                        Some(
                            FilesystemItem::File { comment, .. }
                            | FilesystemItem::Directory { comment, .. }
                            | FilesystemItem::Placeholder { comment },
                        ) => comment.is_some(),
                        None => false,
                    };
                    format!("ok {kind} owns={owns} comment={comment}")
                }
                Err(SyntaxError::InvalidWildcardSyntax { message, .. }) => {
                    format!("wildcard {message}")
                }
                Err(SyntaxError::InvalidPathFormat { path, .. }) => {
                    format!("path empty={}", path == "\"\"")
                }
                Err(SyntaxError::InvalidSpecialDirectory { .. }) => "special".to_string(),
                Err(other) => format!("other {other:?}"),
            });
        }
    }

    #[test]
    fn test_entry_kernel_matches_the_previous_parser_exhaustively() {
        // Every entry text of up to four scalars over the syntax-relevant
        // alphabet, including controls, a drive letter, and non-ASCII.
        const ALPHABET: [char; 15] = [
            'a', '.', '/', '\\', '#', '[', ']', ',', '"', ' ', '\t', ':', '\u{1}', 'é', '\r',
        ];
        let mut current: Vec<char> = Vec::new();
        fn visit(current: &mut Vec<char>, depth: usize, tally: &mut Tally) {
            let text: String = current.iter().collect();
            assert_matches_legacy(&text, tally);
            if depth == 0 {
                return;
            }
            for ch in ALPHABET {
                current.push(ch);
                visit(current, depth - 1, tally);
                current.pop();
            }
        }
        let mut tally = Tally::default();
        visit(&mut current, 4, &mut tally);
        for outcome in [
            "ok placeholder owns=false comment=false",
            "ok directory owns=true comment=false",
            "ok file owns=false comment=true",
            "path empty=true",
            "path empty=false",
            "special",
            "wildcard incomplete escape sequence",
            "wildcard unmatched closing bracket",
        ] {
            assert!(tally.outcomes.contains(outcome), "unreached {outcome}");
        }
    }

    #[test]
    fn test_entry_kernel_matches_the_previous_parser_on_a_generated_corpus() {
        const PIECES: [&str; 40] = [
            "a",
            "b",
            "src",
            "main.rs",
            ".",
            "..",
            "...",
            "/",
            "//",
            "\\",
            "\\\\",
            "\\#",
            "\\[",
            "\\]",
            "\\,",
            "\\\"",
            "\\ ",
            "\\q",
            "#",
            " # note ",
            "[",
            "]",
            ",",
            ", ",
            "\"",
            " ",
            "  ",
            "\t",
            "C:",
            "x:",
            "\u{7f}",
            "\u{0}",
            "\n",
            "\r",
            "é",
            "…",
            "-",
            "\u{1F600}",
            "a b",
            "\\\t",
        ];
        const ALTERNATIVES: [&str; 25] = [
            "",
            " ",
            "a",
            "b",
            ".h",
            ".cpp",
            "/main",
            "/lib",
            "\"a, b\"",
            "\" x \"",
            "\"\"",
            "\\,c",
            "\\ ",
            "\"q\\\"\"",
            "\"#\\#\"",
            "a/b",
            "c/d",
            "..",
            "...",
            "\t\tz\t",
            "\"[]\"",
            "é",
            "\"\\q\"",
            "b\"c\"",
            "\"a\"b",
        ];
        let mut corpus = Corpus(0x9E37_79B9_7F4A_7C15);
        let mut tally = Tally::default();
        for _ in 0..40_000 {
            let mut text = String::new();
            let pieces = corpus.below(7);
            for _ in 0..pieces {
                text.push_str(corpus.pick(&PIECES));
            }
            assert_matches_legacy(&text, &mut tally);
        }
        for _ in 0..40_000 {
            // Structured choice lists: prefix, alternatives, suffix, and an
            // optional directory marker, padding, and comment.
            let mut text = String::new();
            if corpus.below(4) == 0 {
                text.push('"');
            }
            for _ in 0..corpus.below(3) {
                text.push_str(corpus.pick(&PIECES[..10]));
            }
            text.push('[');
            let count = 1 + corpus.below(4);
            for index in 0..count {
                if index > 0 {
                    text.push_str(corpus.pick(&[",", ", ", " ,", ",\t"]));
                }
                text.push_str(corpus.pick(&ALTERNATIVES));
            }
            if corpus.below(8) != 0 {
                text.push(']');
            }
            for _ in 0..corpus.below(3) {
                text.push_str(corpus.pick(&PIECES[..10]));
            }
            text.push_str(corpus.pick(&["", "/", " ", "  # c", " #", "# x #y", "\\"]));
            assert_matches_legacy(&text, &mut tally);
        }
        for _ in 0..20_000 {
            // Quoted paths with tails.
            let mut text = String::from("\"");
            for _ in 0..corpus.below(5) {
                text.push_str(corpus.pick(&PIECES));
            }
            text.push_str(
                corpus.pick(&["\"", "\"/", "\" ", "\"/ # c ", "\"x", "\"//", "", "\"\t#"]),
            );
            if corpus.below(3) == 0 {
                text.push_str(corpus.pick(&PIECES));
            }
            assert_matches_legacy(&text, &mut tally);
        }
        let reached: Vec<&String> = tally.outcomes.iter().collect();
        for outcome in [
            "ok choice owns=false comment=true",
            "ok choice owns=false comment=false",
            "ok directory owns=true comment=true",
            "special",
            "wildcard choice block cannot be empty",
            "wildcard choice expansions must be sibling files",
            "wildcard choice expansions must be unique",
            "wildcard choice expansions must be valid regular-file paths",
            "wildcard choice list must contain between 2 and 256 alternatives",
            "wildcard incomplete escape sequence",
            "wildcard invalid escape in bare path",
            "wildcard invalid escape in quoted choice",
            "wildcard invalid escape in unquoted choice",
            "wildcard multiple wildcard choice blocks are not supported",
            "wildcard quote must begin a choice alternative",
            "wildcard quoted choice must end before the separator",
            "wildcard unescaped bracket in unquoted choice",
            "wildcard unescaped quote in bare path",
            "wildcard unmatched closing bracket",
            "wildcard unterminated wildcard choice block",
        ] {
            assert!(
                tally.outcomes.contains(outcome),
                "unreached {outcome}; reached {reached:?}"
            );
        }
    }

    #[test]
    fn test_entry_kernel_matches_the_previous_parser_at_the_choice_bound() {
        let mut tally = Tally::default();
        for count in [254, 255, 256, 257, 258] {
            let alternatives: Vec<String> = (0..count).map(|index| format!("v{index}")).collect();
            assert_matches_legacy(&format!("x[{}].rs", alternatives.join(", ")), &mut tally);
            assert_matches_legacy(&format!("x[{}]", ",".repeat(count)), &mut tally);
            assert_matches_legacy(&format!("x[{}\\]", "a,".repeat(count)), &mut tally);
            assert_matches_legacy(&format!("x[{}]", "a,".repeat(count)), &mut tally);
        }
        assert!(tally
            .outcomes
            .contains("ok choice owns=false comment=false"));
        assert!(tally
            .outcomes
            .contains("wildcard choice list must contain between 2 and 256 alternatives"));
    }
}
