//! Exclusion-pattern compilation and the shared exclusion matcher.
//!
//! The pattern compiler, the compiled pattern types, and the matcher are the
//! Verus-verified kernel in `proven::glob`; this module reports invalid
//! patterns with their usage diagnostics and maps physical root-relative
//! paths to the UTF-8 component sequences that the kernel decides.

use crate::errors::{AppError, Result};
use crate::path_codec::{render_os_component, render_utf8_component};
use crate::proven::glob::{self, CompiledPattern, PatternError};
use std::path::{Component, Path};

const MAX_PATTERN_DIAGNOSTIC_CHARS: usize = 320;

#[derive(Default)]
pub(crate) struct ExclusionMatcher {
    patterns: Vec<CompiledPattern>,
}

impl ExclusionMatcher {
    pub(crate) fn compile(patterns: &[String]) -> Result<Self> {
        let patterns = patterns
            .iter()
            .map(|pattern| {
                compile_pattern(pattern).map_err(|reason| {
                    AppError::Other(format!(
                        "invalid glob pattern {}: {reason}",
                        render_pattern(pattern)
                    ))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { patterns })
    }

    pub(crate) fn is_match(&self, relative_path: &Path) -> Result<bool> {
        let components = relative_path
            .components()
            .map(|component| match component {
                Component::Normal(name) => name.to_str().ok_or_else(|| {
                    AppError::Other(format!(
                        "non-UTF-8 filesystem entry {} cannot be evaluated by the exclusion matcher",
                        render_os_component(name)
                    ))
                }),
                Component::Prefix(_)
                | Component::RootDir
                | Component::CurDir
                | Component::ParentDir => Err(AppError::Other(
                    "exclusion matching requires a normalized root-relative path".to_string(),
                )),
            })
            .collect::<Result<Vec<_>>>()?;

        // `Path::components` yields only nonempty normal components that
        // contain no `/` (every other component kind is rejected above). That
        // is the hypothesis `path_components_normal` under which the kernel
        // proves that `*`, `?`, and classes never match across a separator
        // (`theorem_wildcards_never_cross_separators`). The kernel's segment
        // semantics alone would let `*`, `?`, a negated class such as `[!a]`,
        // or a range straddling `/` such as `[.-0]` accept the scalar `/`;
        // that cannot happen here because no component contains one.

        // The traversal root itself is never excluded. The verified kernel
        // makes the same decision (`theorem_empty_path_never_excluded`).
        if components.is_empty() {
            return Ok(false);
        }
        Ok(self.matches_components(&components))
    }

    fn matches_components(&self, components: &[&str]) -> bool {
        glob::matches_components(&self.patterns, components)
    }
}

fn render_pattern(pattern: &str) -> String {
    let rendered = render_utf8_component(pattern);
    let mut characters = rendered.chars();
    let bounded = characters
        .by_ref()
        .take(MAX_PATTERN_DIAGNOSTIC_CHARS)
        .collect::<String>();
    if characters.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}

fn compile_pattern(pattern: &str) -> std::result::Result<CompiledPattern, &'static str> {
    glob::compile_pattern(pattern).map_err(pattern_error_message)
}

/// The usage diagnostic for each reason a pattern does not compile.
fn pattern_error_message(error: PatternError) -> &'static str {
    match error {
        PatternError::Empty => "the pattern cannot be empty",
        PatternError::EmptyComponent => {
            "leading, trailing, and repeated `/` separators are not allowed"
        }
        PatternError::DotComponent => "`.` and `..` path components are not allowed",
        PatternError::EmbeddedGlobstar => "`**` is recursive only as a complete path component",
        PatternError::TrailingEscape => "a trailing `\\` escape is not allowed",
        PatternError::UnknownEscape => "a backslash may escape only `\\`, `*`, `?`, `[`, or `]`",
        PatternError::UnescapedClose => "`]` must be escaped outside a character class",
        PatternError::SeparatorInSegment => "`/` is only a path-component separator",
        PatternError::UnterminatedClass => "unterminated character class",
        PatternError::EmptyClass => "character classes cannot be empty",
        PatternError::MissingRangeEndpoint => "a character-class range requires an endpoint",
        PatternError::DescendingRange => {
            "character-class ranges must ascend by Unicode scalar value"
        }
        PatternError::SeparatorInClass => "`/` cannot be matched by a character class",
        PatternError::UnescapedDash => "a literal `-` in a character class must be escaped",
        PatternError::DanglingClassEscape => "a dangling character-class escape is not allowed",
        PatternError::UnknownClassEscape => {
            "a character-class backslash may escape only `\\`, `]`, or `-`"
        }
        PatternError::UnescapedCloseInClass => "a literal `]` in a character class must be escaped",
    }
}

#[cfg(test)]
mod tests {
    use super::{compile_pattern, ExclusionMatcher};
    use crate::proven::glob::{
        CharacterClass, CompiledPattern, PathComponentPattern, SegmentPattern, SegmentToken,
    };
    use std::iter::Peekable;
    use std::path::PathBuf;
    use std::str::Chars;

    // The compiler exactly as this module stated it before it moved into the
    // verified kernel, kept as an oracle for the differential tests below.
    fn reference_compile_pattern(pattern: &str) -> std::result::Result<CompiledPattern, String> {
        if pattern.is_empty() {
            return Err("the pattern cannot be empty".to_string());
        }

        let has_separator = pattern.contains('/');
        let mut components = Vec::new();
        for component in pattern.split('/') {
            if component.is_empty() {
                return Err(
                    "leading, trailing, and repeated `/` separators are not allowed".to_string(),
                );
            }
            if matches!(component, "." | "..") {
                return Err("`.` and `..` path components are not allowed".to_string());
            }
            if component == "**" {
                components.push(PathComponentPattern::Recursive);
            } else {
                components.push(PathComponentPattern::Segment(reference_compile_segment(
                    component,
                )?));
            }
        }

        if !has_separator {
            return match components.pop().expect("one nonempty pattern component") {
                PathComponentPattern::Recursive => Ok(CompiledPattern::RootRelative(vec![
                    PathComponentPattern::Recursive,
                ])),
                PathComponentPattern::Segment(pattern) => Ok(CompiledPattern::Basename(pattern)),
            };
        }

        Ok(CompiledPattern::RootRelative(components))
    }

    fn reference_compile_segment(component: &str) -> std::result::Result<SegmentPattern, String> {
        let mut characters = component.chars().peekable();
        let mut tokens = Vec::new();
        let mut previous_was_star = false;

        while let Some(character) = characters.next() {
            let token = match character {
                '*' => {
                    if previous_was_star {
                        return Err(
                            "`**` is recursive only as a complete path component".to_string()
                        );
                    }
                    previous_was_star = true;
                    SegmentToken::ZeroOrMore
                }
                '?' => {
                    previous_was_star = false;
                    SegmentToken::Any
                }
                '[' => {
                    previous_was_star = false;
                    SegmentToken::Class(reference_parse_class(&mut characters)?)
                }
                '\\' => {
                    previous_was_star = false;
                    let escaped = characters
                        .next()
                        .ok_or_else(|| "a trailing `\\` escape is not allowed".to_string())?;
                    if !matches!(escaped, '\\' | '*' | '?' | '[' | ']') {
                        return Err(
                            "a backslash may escape only `\\`, `*`, `?`, `[`, or `]`".to_string()
                        );
                    }
                    SegmentToken::Literal(escaped)
                }
                ']' => {
                    return Err("`]` must be escaped outside a character class".to_string());
                }
                '/' => {
                    return Err("`/` is only a path-component separator".to_string());
                }
                literal => {
                    previous_was_star = false;
                    SegmentToken::Literal(literal)
                }
            };
            tokens.push(token);
        }

        Ok(SegmentPattern { tokens })
    }

    fn reference_parse_class(
        characters: &mut Peekable<Chars<'_>>,
    ) -> std::result::Result<CharacterClass, String> {
        let negated = characters.next_if_eq(&'!').is_some();
        let mut ranges = Vec::new();

        loop {
            match characters.peek().copied() {
                None => return Err("unterminated character class".to_string()),
                Some(']') => {
                    characters.next();
                    if ranges.is_empty() {
                        return Err("character classes cannot be empty".to_string());
                    }
                    return Ok(CharacterClass { negated, ranges });
                }
                Some(_) => {}
            }

            let start = reference_parse_class_character(characters)?;
            if characters.next_if_eq(&'-').is_some() {
                if matches!(characters.peek(), None | Some(']')) {
                    return Err("a character-class range requires an endpoint".to_string());
                }
                let end = reference_parse_class_character(characters)?;
                if start > end {
                    return Err(
                        "character-class ranges must ascend by Unicode scalar value".to_string()
                    );
                }
                ranges.push((start, end));
            } else {
                ranges.push((start, start));
            }
        }
    }

    fn reference_parse_class_character(
        characters: &mut Peekable<Chars<'_>>,
    ) -> std::result::Result<char, String> {
        match characters
            .next()
            .ok_or_else(|| "unterminated character class".to_string())?
        {
            '/' => Err("`/` cannot be matched by a character class".to_string()),
            '-' => Err("a literal `-` in a character class must be escaped".to_string()),
            '\\' => {
                let escaped = characters.next().ok_or_else(|| {
                    "a dangling character-class escape is not allowed".to_string()
                })?;
                if !matches!(escaped, '\\' | ']' | '-') {
                    return Err(
                        "a character-class backslash may escape only `\\`, `]`, or `-`".to_string(),
                    );
                }
                Ok(escaped)
            }
            ']' => Err("a literal `]` in a character class must be escaped".to_string()),
            literal => Ok(literal),
        }
    }

    /// A canonical description of a compiled pattern.
    fn describe(pattern: &CompiledPattern) -> String {
        fn segment(segment: &SegmentPattern) -> String {
            segment
                .tokens
                .iter()
                .map(|token| match token {
                    SegmentToken::Literal(character) => format!("L{character:?}"),
                    SegmentToken::ZeroOrMore => "*".to_string(),
                    SegmentToken::Any => "?".to_string(),
                    SegmentToken::Class(class) => format!(
                        "C{}{:?}",
                        if class.negated { "!" } else { "" },
                        class.ranges
                    ),
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
        match pattern {
            CompiledPattern::Basename(pattern) => format!("B[{}]", segment(pattern)),
            CompiledPattern::RootRelative(components) => format!(
                "R[{}]",
                components
                    .iter()
                    .map(|component| match component {
                        PathComponentPattern::Recursive => "**".to_string(),
                        PathComponentPattern::Segment(pattern) => segment(pattern),
                    })
                    .collect::<Vec<_>>()
                    .join(" / ")
            ),
        }
    }

    fn compare_with_reference(pattern: &str) {
        let observed = compile_pattern(pattern).map(|compiled| describe(&compiled));
        let expected = reference_compile_pattern(pattern)
            .map(|compiled| describe(&compiled))
            .map_err(|reason| reason.to_string());
        assert_eq!(observed.map_err(str::to_string), expected, "{pattern:?}");
    }

    #[test]
    fn every_pattern_error_has_its_usage_diagnostic() {
        use super::pattern_error_message;
        use crate::proven::glob::PatternError;
        let cases = [
            (PatternError::Empty, "the pattern cannot be empty"),
            (
                PatternError::EmptyComponent,
                "leading, trailing, and repeated `/` separators are not allowed",
            ),
            (
                PatternError::DotComponent,
                "`.` and `..` path components are not allowed",
            ),
            (
                PatternError::EmbeddedGlobstar,
                "`**` is recursive only as a complete path component",
            ),
            (
                PatternError::TrailingEscape,
                "a trailing `\\` escape is not allowed",
            ),
            (
                PatternError::UnknownEscape,
                "a backslash may escape only `\\`, `*`, `?`, `[`, or `]`",
            ),
            (
                PatternError::UnescapedClose,
                "`]` must be escaped outside a character class",
            ),
            (
                PatternError::SeparatorInSegment,
                "`/` is only a path-component separator",
            ),
            (
                PatternError::UnterminatedClass,
                "unterminated character class",
            ),
            (
                PatternError::EmptyClass,
                "character classes cannot be empty",
            ),
            (
                PatternError::MissingRangeEndpoint,
                "a character-class range requires an endpoint",
            ),
            (
                PatternError::DescendingRange,
                "character-class ranges must ascend by Unicode scalar value",
            ),
            (
                PatternError::SeparatorInClass,
                "`/` cannot be matched by a character class",
            ),
            (
                PatternError::UnescapedDash,
                "a literal `-` in a character class must be escaped",
            ),
            (
                PatternError::DanglingClassEscape,
                "a dangling character-class escape is not allowed",
            ),
            (
                PatternError::UnknownClassEscape,
                "a character-class backslash may escape only `\\`, `]`, or `-`",
            ),
            (
                PatternError::UnescapedCloseInClass,
                "a literal `]` in a character class must be escaped",
            ),
        ];
        for (error, message) in cases {
            assert_eq!(pattern_error_message(error), message, "{error:?}");
        }
    }

    #[test]
    fn compiler_matches_reference_on_every_short_pattern() {
        let alphabet = ['a', '*', '?', '[', ']', '\\', '-', '!', '/', '.', 'é'];
        let mut frontier = vec![String::new()];
        compare_with_reference("");
        for _ in 0..5 {
            let mut next = Vec::new();
            for prefix in &frontier {
                for character in alphabet {
                    let mut pattern = prefix.clone();
                    pattern.push(character);
                    compare_with_reference(&pattern);
                    next.push(pattern);
                }
            }
            frontier = next;
        }
    }

    #[test]
    fn compiler_matches_reference_on_longer_patterns() {
        for pattern in [
            "projects/**/target",
            "a/**/**/b",
            "**/*.tmp",
            "[a-z0-9]*.rs",
            "[!a-c\\]\\-]x",
            "[\\\\-\\]]",
            "src/[",
            "src/[a-",
            "src/[a-]",
            "src/[z-a]",
            "src/\\",
            "a/b/c/d/e/f",
            "x*y*z?",
            "é/λ/[α-ω]",
        ] {
            compare_with_reference(pattern);
        }
    }

    fn matcher(patterns: &[&str]) -> ExclusionMatcher {
        ExclusionMatcher::compile(
            &patterns
                .iter()
                .map(|pattern| (*pattern).to_string())
                .collect::<Vec<_>>(),
        )
        .expect("valid patterns")
    }

    fn matches(matcher: &ExclusionMatcher, components: &[&str]) -> bool {
        matcher.matches_components(components)
    }

    #[test]
    fn issue_44_basename_and_root_relative_truth_table() {
        let basename = matcher(&["target"]);
        assert!(matches(&basename, &["target"]));
        assert!(matches(&basename, &["project", "target"]));
        assert!(!matches(&basename, &["targets"]));

        let temporary = matcher(&["*.tmp"]);
        assert!(matches(&temporary, &["nested", "a.tmp"]));
        assert!(!matches(&temporary, &["nested", "a.tmp.keep"]));

        let root_relative = matcher(&["project/target"]);
        assert!(matches(&root_relative, &["project", "target"]));
        assert!(!matches(&root_relative, &["other", "project", "target"]));
        assert!(!matches(&root_relative, &["project", "target", "file"]));
    }

    #[test]
    fn issue_44_component_wildcards_and_globstar_are_distinct() {
        let one = matcher(&["projects/*/target"]);
        assert!(matches(&one, &["projects", "a", "target"]));
        assert!(!matches(&one, &["projects", "a", "b", "target"]));

        let recursive = matcher(&["projects/**/target"]);
        assert!(matches(&recursive, &["projects", "target"]));
        assert!(matches(&recursive, &["projects", "a", "b", "target"]));
        assert!(!matches(&recursive, &["projects", "a", "target", "file"]));

        let trailing = matcher(&["project/**"]);
        assert!(matches(&trailing, &["project"]));
        assert!(matches(&trailing, &["project", "a", "b"]));

        let all = matcher(&["**"]);
        assert!(matches(&all, &["one"]));
        assert!(matches(&all, &["one", "two"]));
    }

    #[test]
    fn issue_44_segment_tokens_match_unicode_scalars_exactly() {
        let one = matcher(&["?"]);
        assert!(matches(&one, &["é"]));
        assert!(!matches(&one, &["éé"]));

        let range = matcher(&["[α-ω]"]);
        assert!(matches(&range, &["λ"]));
        assert!(!matches(&range, &["A"]));

        let negated = matcher(&["[!0-9]"]);
        assert!(matches(&negated, &["x"]));
        assert!(!matches(&negated, &["7"]));

        let caret_is_literal = matcher(&["[^]"]);
        assert!(matches(&caret_is_literal, &["^"]));
        assert!(!matches(&caret_is_literal, &["x"]));
    }

    #[test]
    fn issue_44_matching_is_exact_case_sensitive_and_normalization_preserving() {
        let exact = matcher(&["target"]);
        assert!(!matches(&exact, &["TARGET"]));
        assert!(!matches(&exact, &["targets"]));

        let composed = matcher(&["é"]);
        assert!(matches(&composed, &["é"]));
        assert!(!matches(&composed, &["e\u{301}"]));

        let dot = matcher(&["*"]);
        assert!(matches(&dot, &[".git"]));
    }

    #[test]
    fn issue_44_escapes_and_other_literals_follow_only_the_contract() {
        let escaped = matcher(&[r"\*\?\[\]\\"]);
        assert!(matches(&escaped, &[r"*?[]\"]));

        let class_escaped = matcher(&[r"[\]\-\\]"]);
        for value in ["]", "-", r"\"] {
            assert!(matches(&class_escaped, &[value]));
        }
        assert!(!matches(&class_escaped, &["x"]));

        let literals = matcher(&["{one,two}", "!target"]);
        assert!(matches(&literals, &["{one,two}"]));
        assert!(matches(&literals, &["!target"]));
        assert!(!matches(&literals, &["one"]));
        assert!(!matches(&literals, &["target"]));

        let escaped_adjacent_stars = matcher(&[r"\**", r"*\*"]);
        assert!(matches(&escaped_adjacent_stars, &["*suffix"]));
        assert!(matches(&escaped_adjacent_stars, &["prefix*"]));
    }

    #[test]
    fn issue_44_multiple_patterns_are_a_union() {
        let union = matcher(&["target", "*.tmp", "project/generated"]);
        assert!(matches(&union, &["nested", "target"]));
        assert!(matches(&union, &["nested", "file.tmp"]));
        assert!(matches(&union, &["project", "generated"]));
        assert!(!matches(&union, &["project", "keep"]));
    }

    #[test]
    fn issue_44_invalid_forms_are_rejected_during_compilation() {
        for pattern in [
            "",
            "/a",
            "a/",
            "a//b",
            ".",
            "..",
            "a/./b",
            "a/../b",
            "***",
            "a/**b",
            "a/b**",
            "a/***/b",
            "[]",
            "[!]",
            "[",
            "[z-a]",
            "[-a]",
            "[a-]",
            "[a--]",
            "[/]",
            r"[\q]",
            "unescaped]",
            "\\",
            "\\q",
        ] {
            let error = ExclusionMatcher::compile(&[pattern.to_string()])
                .err()
                .unwrap_or_else(|| panic!("invalid pattern {pattern:?} was accepted"));
            assert!(
                error.to_string().contains("invalid glob pattern"),
                "{pattern:?}: {error}"
            );
        }
    }

    #[test]
    fn issue_44_one_invalid_pattern_rejects_the_union() {
        assert!(ExclusionMatcher::compile(&["target".to_string(), "a/**b".to_string()]).is_err());
    }

    #[test]
    fn issue_44_invalid_pattern_diagnostics_escape_controls() {
        let diagnostic = ExclusionMatcher::compile(&["\\\n".to_string()])
            .err()
            .expect("unknown control escape must reject")
            .to_string();
        assert!(!diagnostic.contains('\n'), "{diagnostic:?}");
        assert!(diagnostic.contains("\\n"), "{diagnostic:?}");
    }

    #[test]
    fn issue_44_invalid_pattern_diagnostics_are_bounded() {
        let pattern = format!("{}\\q", "a".repeat(10_000));
        let diagnostic = ExclusionMatcher::compile(&[pattern])
            .err()
            .expect("unknown long escape must reject")
            .to_string();
        assert!(diagnostic.chars().count() < 600, "{}", diagnostic.len());
        assert!(diagnostic.contains('…'), "{diagnostic}");
    }

    #[test]
    fn issue_44_physical_components_use_logical_slash_matching() {
        let matcher = matcher(&["project/target"]);
        let physical = PathBuf::from("project").join("target");
        assert!(matcher.is_match(&physical).expect("UTF-8 physical path"));
    }
}
