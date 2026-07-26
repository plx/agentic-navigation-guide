use crate::errors::{AppError, Result};
use crate::path_codec::{render_os_component, render_utf8_component};
use std::iter::Peekable;
use std::path::{Component, Path};
use std::str::Chars;

const MAX_PATTERN_DIAGNOSTIC_CHARS: usize = 320;

#[derive(Default)]
pub(crate) struct ExclusionMatcher {
    patterns: Vec<CompiledPattern>,
}

enum CompiledPattern {
    Basename(SegmentPattern),
    RootRelative(Vec<PathComponentPattern>),
}

enum PathComponentPattern {
    Recursive,
    Segment(SegmentPattern),
}

struct SegmentPattern {
    tokens: Vec<SegmentToken>,
}

enum SegmentToken {
    Literal(char),
    ZeroOrMore,
    Any,
    Class(CharacterClass),
}

struct CharacterClass {
    negated: bool,
    ranges: Vec<(char, char)>,
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

        if components.is_empty() {
            return Ok(false);
        }
        Ok(self.matches_components(&components))
    }

    fn matches_components(&self, components: &[&str]) -> bool {
        self.patterns.iter().any(|pattern| match pattern {
            CompiledPattern::Basename(pattern) => components
                .iter()
                .any(|component| pattern.is_match(component)),
            CompiledPattern::RootRelative(pattern) => path_pattern_matches(pattern, components),
        })
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

impl SegmentPattern {
    fn is_match(&self, value: &str) -> bool {
        let characters = value.chars().collect::<Vec<_>>();
        let mut matched = vec![false; characters.len() + 1];
        matched[0] = true;

        for token in &self.tokens {
            let mut next = vec![false; characters.len() + 1];
            match token {
                SegmentToken::ZeroOrMore => {
                    next[0] = matched[0];
                    for index in 1..=characters.len() {
                        next[index] = matched[index] || next[index - 1];
                    }
                }
                SegmentToken::Literal(expected) => {
                    for index in 1..=characters.len() {
                        next[index] = matched[index - 1] && characters[index - 1] == *expected;
                    }
                }
                SegmentToken::Any => {
                    next[1..(characters.len() + 1)].copy_from_slice(&matched[..characters.len()]);
                }
                SegmentToken::Class(class) => {
                    for index in 1..=characters.len() {
                        next[index] = matched[index - 1] && class.matches(characters[index - 1]);
                    }
                }
            }
            matched = next;
        }

        matched[characters.len()]
    }
}

impl CharacterClass {
    fn matches(&self, value: char) -> bool {
        let contained = self
            .ranges
            .iter()
            .any(|(start, end)| *start <= value && value <= *end);
        contained != self.negated
    }
}

fn compile_pattern(pattern: &str) -> std::result::Result<CompiledPattern, String> {
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
            components.push(PathComponentPattern::Segment(compile_segment(component)?));
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

fn compile_segment(component: &str) -> std::result::Result<SegmentPattern, String> {
    let mut characters = component.chars().peekable();
    let mut tokens = Vec::new();
    let mut previous_was_star = false;

    while let Some(character) = characters.next() {
        let token = match character {
            '*' => {
                if previous_was_star {
                    return Err("`**` is recursive only as a complete path component".to_string());
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
                SegmentToken::Class(parse_class(&mut characters)?)
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

fn parse_class(
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

        let start = parse_class_character(characters)?;
        if characters.next_if_eq(&'-').is_some() {
            if matches!(characters.peek(), None | Some(']')) {
                return Err("a character-class range requires an endpoint".to_string());
            }
            let end = parse_class_character(characters)?;
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

fn parse_class_character(
    characters: &mut Peekable<Chars<'_>>,
) -> std::result::Result<char, String> {
    match characters
        .next()
        .ok_or_else(|| "unterminated character class".to_string())?
    {
        '/' => Err("`/` cannot be matched by a character class".to_string()),
        '-' => Err("a literal `-` in a character class must be escaped".to_string()),
        '\\' => {
            let escaped = characters
                .next()
                .ok_or_else(|| "a dangling character-class escape is not allowed".to_string())?;
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

fn path_pattern_matches(pattern: &[PathComponentPattern], components: &[&str]) -> bool {
    let mut matched = vec![false; components.len() + 1];
    matched[0] = true;

    for component_pattern in pattern {
        let mut next = vec![false; components.len() + 1];
        match component_pattern {
            PathComponentPattern::Recursive => {
                next[0] = matched[0];
                for index in 1..=components.len() {
                    next[index] = matched[index] || next[index - 1];
                }
            }
            PathComponentPattern::Segment(segment) => {
                for index in 1..=components.len() {
                    next[index] = matched[index - 1] && segment.is_match(components[index - 1]);
                }
            }
        }
        matched = next;
    }

    matched[components.len()]
}

#[cfg(test)]
mod tests {
    use super::ExclusionMatcher;
    use std::path::PathBuf;

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
