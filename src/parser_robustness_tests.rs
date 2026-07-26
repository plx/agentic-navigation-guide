use crate::parser::Parser;
use crate::path_codec::serialize_component;
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use crate::validator::Validator;
use std::collections::BTreeSet;

const MAX_INPUT_BYTES: usize = 65_536;
const MAX_CHOICE_ALTERNATIVES: usize = 256;
const MAX_LOGICAL_DEPTH: usize = 256;

#[derive(Debug, PartialEq, Eq)]
enum Observation {
    Accepted(NavigationGuide),
    Rejected(String),
}

fn observe(source: &str) -> Observation {
    match Parser::new().parse(source) {
        Ok(guide) if guide.ignore => Observation::Accepted(guide),
        Ok(guide) => match Validator::new().validate_syntax(&guide) {
            Ok(()) => Observation::Accepted(guide),
            Err(error) => Observation::Rejected(error.to_string()),
        },
        Err(error) => Observation::Rejected(error.to_string()),
    }
}

fn observe_twice(source: &str) -> Observation {
    assert!(
        source.len() <= MAX_INPUT_BYTES,
        "robustness fixture exceeded its reviewed 64 KiB test bound"
    );
    let first = observe(source);
    let second = observe(source);
    assert_eq!(first, second, "the same parser input changed outcome");
    first
}

fn assert_guide_bounds(guide: &NavigationGuide, source_lines: usize) {
    assert!(!guide.ignore, "active-guide checks received ignore=true");
    let mut pending = guide
        .items
        .iter()
        .rev()
        .map(|line| (line, 0_usize, String::new()))
        .collect::<Vec<_>>();
    let mut identities = BTreeSet::new();
    let mut item_count = 0_usize;

    while let Some((line, depth, parent)) = pending.pop() {
        assert!(depth <= MAX_LOGICAL_DEPTH);
        assert_eq!(line.indent_level, depth);
        item_count = item_count
            .checked_add(1)
            .expect("accepted item count overflowed usize");

        let full_path = if parent.is_empty() {
            line.path().to_string()
        } else {
            format!("{parent}/{}", line.path())
        };
        if !line.is_placeholder() {
            assert!(
                identities.insert(full_path.clone()),
                "validation accepted a collapsed filesystem identity"
            );
        }
        if let FilesystemItem::Directory { children, .. } = &line.item {
            for child in children.iter().rev() {
                pending.push((child, depth + 1, full_path.clone()));
            }
        }
    }

    let maximum_items = source_lines
        .max(1)
        .checked_mul(MAX_CHOICE_ALTERNATIVES)
        .expect("source-line expansion bound overflowed usize");
    assert!(item_count <= maximum_items);
}

fn assert_active(source: &str) -> NavigationGuide {
    match observe_twice(source) {
        Observation::Accepted(guide) if !guide.ignore => {
            assert_guide_bounds(&guide, source.lines().count());
            guide
        }
        other => panic!("expected an active guide, got {other:?}"),
    }
}

fn assert_rejected(source: &str) {
    assert!(
        matches!(observe_twice(source), Observation::Rejected(_)),
        "unexpectedly accepted input: {source:?}"
    );
}

#[derive(Clone, Copy)]
struct DeterministicSequence(u64);

impl DeterministicSequence {
    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }
}

#[test]
fn issue_57_parser_robustness_utf8_document_matrix_is_deterministic() {
    const FRAGMENTS: [&str; 30] = [
        "",
        "a",
        "-",
        "- ",
        "src/",
        "...",
        ".",
        "..",
        "/",
        "//",
        "#",
        "[",
        "]",
        ",",
        "\\",
        "\"",
        " ",
        "\t",
        "\n",
        "\r",
        "\r\n",
        "<agentic-navigation-guide>",
        "<agentic-navigation-guide ignore=true>",
        "<agentic-navigation-guideTYPO>",
        "</agentic-navigation-guide>",
        "</agentic-navigation-guides>",
        "\u{feff}",
        "\u{7f}",
        "café",
        "🧭",
    ];

    let long_document = format!(
        "<agentic-navigation-guide>\n- {}\n</agentic-navigation-guide>",
        "a".repeat(65_000)
    );
    let reviewed_documents = [
        "",
        "<agentic-navigation-guide>\n\n</agentic-navigation-guide>",
        "<agentic-navigation-guide>\r\n- src/\r\n</agentic-navigation-guide>",
        "<agentic-navigation-guide>\r- src/\n</agentic-navigation-guide>",
        "<agentic-navigation-guide>\n- src/\n</agentic-navigation-guide>",
        "<agentic-navigation-guide ignore=true>\nopaque\n</agentic-navigation-guide>",
        "<agentic-navigation-guide>\n- first\n</agentic-navigation-guide>\n<agentic-navigation-guide>\n- second\n</agentic-navigation-guide>",
        "\u{feff}<agentic-navigation-guide>\n- src\n</agentic-navigation-guide>",
    ];
    for source in reviewed_documents
        .into_iter()
        .chain(std::iter::once(long_document.as_str()))
    {
        let observation = observe_twice(source);
        if let Observation::Accepted(guide) = observation {
            if !guide.ignore {
                assert_guide_bounds(&guide, source.lines().count());
            }
        }
    }

    let mut sequence = DeterministicSequence(0x0057_2026_0726);
    for case in 0..2_048 {
        let fragment_count = 1 + (sequence.next() as usize % 64);
        let mut body = String::new();
        for _ in 0..fragment_count {
            let fragment = FRAGMENTS[sequence.next() as usize % FRAGMENTS.len()];
            if body.len() + fragment.len() > 4_096 {
                break;
            }
            body.push_str(fragment);
        }

        let source = match case % 4 {
            0 => body,
            1 => format!("<agentic-navigation-guide>\n- {body}\n</agentic-navigation-guide>"),
            2 => {
                format!("{body}\n<agentic-navigation-guide>\n- stable\n</agentic-navigation-guide>")
            }
            _ => format!(
                "<agentic-navigation-guide ignore=true>\n{body}\n</agentic-navigation-guide>"
            ),
        };
        let observation = observe_twice(&source);
        if let Observation::Accepted(guide) = observation {
            if !guide.ignore {
                assert_guide_bounds(&guide, source.lines().count());
            }
        }
    }
}

#[test]
fn issue_57_parser_robustness_marker_attribute_grammar_is_exact() {
    for outer in ["", " ", "\t", " \t"] {
        let bare = format!("{outer}<agentic-navigation-guide>{outer}");
        let source = format!("{bare}\n- stable\n</agentic-navigation-guide>");
        let guide = assert_active(&source);
        assert!(!guide.ignore);

        for name_gap in [" ", "\t", "  \t"] {
            for before_equals in ["", " ", "\t "] {
                for after_equals in ["", " ", "\t "] {
                    for value in ["true", "\"true\""] {
                        for before_close in ["", " ", "\t"] {
                            let opening = format!(
                                "{outer}<agentic-navigation-guide{name_gap}ignore{before_equals}={after_equals}{value}{before_close}>{outer}"
                            );
                            let source =
                                format!("{opening}\nopaque body\n</agentic-navigation-guide>");
                            match observe_twice(&source) {
                                Observation::Accepted(guide) => {
                                    assert!(guide.ignore);
                                    assert!(guide.items.is_empty());
                                }
                                other => panic!(
                                    "valid exact ignore marker was rejected: {opening:?}: {other:?}"
                                ),
                            }
                        }
                    }
                }
            }
        }
    }

    for malformed in [
        "<agentic-navigation-guideignore=true>",
        "<agentic-navigation-guideTYPO>",
        "<agentic-navigation-guides>",
        "<agentic-navigation-guide mode=example>",
        "<agentic-navigation-guide IGNORE=true>",
        "<agentic-navigation-guide ignore=true ignore=true>",
        "<agentic-navigation-guide ignore=false>",
        "<agentic-navigation-guide ignore=TRUE>",
        "<agentic-navigation-guide ignore='true'>",
        "<agentic-navigation-guide ignore=yes>",
        "<agentic-navigation-guide ignore=>",
        "<agentic-navigation-guide \t>",
        "<agentic-navigation-guide ignore=\"true>",
        "<agentic-navigation-guide ignore=\"true\"extra>",
        "<agentic-navigation-guide><agentic-navigation-guide>",
        "<agentic-navigation-guide>suffix",
        "<agentic-navigation-guide ignore=true",
        "<agentic-navigation-guide",
        "\u{feff}<agentic-navigation-guide ignore=true>",
    ] {
        let source = format!("{malformed}\n- stable\n</agentic-navigation-guide>");
        assert_rejected(&source);
    }
}

#[test]
fn issue_57_parser_robustness_choices_escapes_and_identity_are_bounded() {
    let guide = assert_active(
        "<agentic-navigation-guide>\n- data[\"with , comma\", \\,space, \"literal []\"] # choices, escapes, and comment\n</agentic-navigation-guide>",
    );
    assert_eq!(
        guide
            .items
            .iter()
            .map(NavigationGuideLine::path)
            .collect::<Vec<_>>(),
        vec!["datawith , comma", "data,space", "dataliteral []"]
    );

    let escaped = assert_active(
        "<agentic-navigation-guide>\n- Foo\\[bar\\].txt\n</agentic-navigation-guide>",
    );
    assert_eq!(escaped.items[0].path(), "Foo[bar].txt");

    let at_limit = (0..MAX_CHOICE_ALTERNATIVES)
        .map(|index| format!("alternative-{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let source =
        format!("<agentic-navigation-guide>\n- file[{at_limit}]\n</agentic-navigation-guide>");
    assert_eq!(assert_active(&source).items.len(), MAX_CHOICE_ALTERNATIVES);

    let over_limit = (0..=MAX_CHOICE_ALTERNATIVES)
        .map(|index| format!("alternative-{index}"))
        .collect::<Vec<_>>()
        .join(",");
    assert_rejected(&format!(
        "<agentic-navigation-guide>\n- file[{over_limit}]\n</agentic-navigation-guide>"
    ));

    let empty_alternative =
        assert_active("<agentic-navigation-guide>\n- file[a,,b]\n</agentic-navigation-guide>");
    assert_eq!(
        empty_alternative
            .items
            .iter()
            .map(NavigationGuideLine::path)
            .collect::<Vec<_>>(),
        vec!["filea", "file", "fileb"]
    );

    for invalid in [
        "file[]",
        "file[   ]",
        "file[a,b][c,d]",
        "file[a,b",
        "filea,b]",
        "file[\"unterminated,b]",
        "file[a,b]\\",
    ] {
        assert_rejected(&format!(
            "<agentic-navigation-guide>\n- {invalid}\n</agentic-navigation-guide>"
        ));
    }

    assert_rejected(
        "<agentic-navigation-guide>\n- report\\#draft\n- \"report#draft\"\n</agentic-navigation-guide>",
    );
}

fn nested_source(deepest_depth: usize, indent_size: usize) -> String {
    let mut source = String::from("<agentic-navigation-guide>\n");
    for depth in 0..=deepest_depth {
        source.push_str(
            &" ".repeat(
                depth
                    .checked_mul(indent_size)
                    .expect("test indentation width overflowed"),
            ),
        );
        source.push_str(&format!("- directory-{depth}/\n"));
    }
    source.push_str("</agentic-navigation-guide>");
    source
}

#[test]
fn issue_57_parser_robustness_indentation_and_hierarchy_are_bounded() {
    for indent_size in 1..=16 {
        for depth in [1, 2, 8, 16] {
            let source = nested_source(depth, indent_size);
            let guide = assert_active(&source);
            assert_guide_bounds(&guide, source.lines().count());
        }
    }

    let at_limit = nested_source(MAX_LOGICAL_DEPTH, 1);
    assert!(at_limit.len() <= MAX_INPUT_BYTES);
    let guide = assert_active(&at_limit);
    assert_guide_bounds(&guide, at_limit.lines().count());

    let over_limit = nested_source(MAX_LOGICAL_DEPTH + 1, 1);
    assert!(over_limit.len() <= MAX_INPUT_BYTES);
    assert_rejected(&over_limit);

    for body in [
        "- file\n  - child",
        "- ...\n  - child",
        "- [first,second]\n  - child",
        " - indented-first",
        "- root/\n \t- mixed-indentation",
        "- root/\n   - depth-one\n      - skipped-depth",
        "- parent//child",
    ] {
        assert_rejected(&format!(
            "<agentic-navigation-guide>\n{body}\n</agentic-navigation-guide>"
        ));
    }

    let mut wide = String::from("<agentic-navigation-guide>\n");
    for index in 0..1_024 {
        wide.push_str(&format!("- item-{index}\n"));
    }
    wide.push_str("</agentic-navigation-guide>");
    assert!(wide.len() <= MAX_INPUT_BYTES);
    let guide = assert_active(&wide);
    assert_eq!(guide.items.len(), 1_024);
    assert_eq!(guide.items[0].path(), "item-0");
    assert_eq!(guide.items[1_023].path(), "item-1023");
}

#[test]
fn issue_57_parser_robustness_serialized_components_round_trip_exactly() {
    let names = [
        "plain",
        "report#draft",
        "[draft]",
        "comma,name",
        "quote\"name",
        "back\\slash",
        " leading",
        "trailing ",
        "...",
        "café",
        "cafe\u{301}",
        "emoji-🧭",
    ];
    let mut source = String::from("<agentic-navigation-guide>\n");
    for name in names {
        let serialized = serialize_component(name).expect("reviewed component is representable");
        source.push_str(&format!("- {serialized}\n"));
    }
    source.push_str("</agentic-navigation-guide>");

    let guide = assert_active(&source);
    let observed = guide
        .items
        .iter()
        .map(NavigationGuideLine::path)
        .collect::<Vec<_>>();
    assert_eq!(observed, names);
}
