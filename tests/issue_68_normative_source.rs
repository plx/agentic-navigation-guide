use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const NORMATIVE_MARKER: &str = "<!-- normative-v0.2-specification -->";
const HISTORICAL_MARKER: &str = "<!-- historical-v0.2-specification -->";
const NORMATIVE_PATH: &str = "docs/v0.2-contract.md";
const HISTORICAL_PATH: &str = "docs/history/Specification.md";
const AUDIT_PATH: &str = "audits/2026-07-27-issue-68-specification-disposition.md";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repository_file(path: &str) -> String {
    let path = repository_root().join(path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn collect_markdown(directory: &Path, relative: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("failed to enumerate {}: {error}", directory.display()));
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let next_relative = relative.join(file_name.as_ref());
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!("failed to classify {}: {error}", entry.path().display())
        });
        if file_type.is_dir() {
            if matches!(
                file_name.as_ref(),
                ".git" | ".context" | "target" | "node_modules"
            ) {
                continue;
            }
            collect_markdown(&entry.path(), &next_relative, files);
        } else if file_type.is_file()
            && entry.path().extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("mdx")
            })
        {
            files.push(next_relative);
        }
    }
}

fn normalized_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn markdown_outside_fences(source: &str) -> String {
    let mut output = String::new();
    let mut open_fence: Option<(u8, usize)> = None;

    for line in source.lines() {
        let trimmed = line.trim_start();
        let bytes = trimmed.as_bytes();
        let marker = bytes.first().copied();
        let marker_length = marker.map_or(0, |marker| {
            bytes.iter().take_while(|byte| **byte == marker).count()
        });

        if let Some((open_marker, open_length)) = open_fence {
            let is_close = marker == Some(open_marker)
                && marker_length >= open_length
                && trimmed[marker_length..].trim().is_empty();
            if is_close {
                open_fence = None;
            }
            continue;
        }

        if marker.is_some_and(|marker| marker == b'`' || marker == b'~') && marker_length >= 3 {
            open_fence = Some((marker.expect("fence marker"), marker_length));
            continue;
        }

        output.push_str(line);
        output.push('\n');
    }

    output
}

fn assert_local_markdown_links_resolve(path: &str) {
    let source_path = repository_root().join(path);
    let source = repository_file(path);
    let source = markdown_outside_fences(&source);
    let link_pattern =
        Regex::new(r#"\[[^\]]+\]\((?P<target>[^)]+)\)"#).expect("valid Markdown link regex");

    for captures in link_pattern.captures_iter(&source) {
        let target = captures
            .name("target")
            .expect("link target capture")
            .as_str()
            .trim()
            .trim_matches('<')
            .trim_matches('>');
        if target.starts_with("https://")
            || target.starts_with("http://")
            || target.starts_with("mailto:")
            || target.starts_with('#')
        {
            continue;
        }
        let target_path = target.split('#').next().unwrap_or_default();
        assert!(!target_path.is_empty(), "empty local link in {path}");
        let resolved = source_path
            .parent()
            .expect("documentation file parent")
            .join(target_path);
        assert!(
            resolved.is_file(),
            "{path} has a broken local link `{target}` (resolved to {})",
            resolved.display()
        );
    }
}

fn assert_balanced_code_fences(path: &str) {
    let source = repository_file(path);
    assert_eq!(
        source
            .lines()
            .filter(|line| line.trim_start().starts_with("```"))
            .count()
            % 2,
        0,
        "{path} has an unbalanced fenced code block"
    );
}

#[test]
fn issue_68_has_one_normative_claimant_and_one_unmistakable_history_record() {
    let root = repository_root();
    assert!(
        !root.join("Specification.md").exists(),
        "the contradictory historical specification must not remain at the repository root"
    );
    assert!(
        root.join(HISTORICAL_PATH).is_file(),
        "the approved history-preserving destination must exist"
    );

    let mut markdown_files = Vec::new();
    collect_markdown(&root, Path::new(""), &mut markdown_files);

    let mut normative_claimants = Vec::new();
    let mut historical_records = Vec::new();
    for path in markdown_files {
        let contents = fs::read_to_string(root.join(&path))
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let relative = normalized_path(&path);
        for _ in contents.match_indices(NORMATIVE_MARKER) {
            normative_claimants.push(relative.clone());
        }
        for _ in contents.match_indices(HISTORICAL_MARKER) {
            historical_records.push(relative.clone());
        }
    }

    assert_eq!(
        normative_claimants,
        vec![NORMATIVE_PATH],
        "exactly the maintained v0.2 contract must carry the normative claimant marker"
    );
    assert_eq!(
        historical_records,
        vec![HISTORICAL_PATH],
        "exactly the relocated original specification must carry the historical marker"
    );

    let normative = repository_file(NORMATIVE_PATH);
    assert!(normative.contains("## Status and authority"));
    assert!(normative.contains("This document is the normative target for the v0.2 guide language"));
    assert!(normative.contains("Every current normative v0.2 specification MUST carry"));
    assert!(!normative.contains("pending #68's final disposition"));
    assert!(!normative.contains("moving or retiring historical `Specification.md`"));

    let historical = repository_file(HISTORICAL_PATH);
    for required in [
        "**Historical design record — non-normative",
        "Do not use this document to determine current behavior",
        "[`docs/v0.2-contract.md`](../v0.2-contract.md)",
        "[classification and disposition record](../../audits/2026-07-27-issue-68-specification-disposition.md)",
        "## Background: The Problem",
        "## Background: The Solution",
        "## Component: `AGENTIC_NAVIGATION_GUIDE.md`",
        "## Component: `agentic-navigation-guide`",
        "## Details: Execution Modes",
        "## Details: Rust",
    ] {
        assert!(
            historical.contains(required),
            "historical record must preserve and clearly frame `{required}`"
        );
    }

    let history_index = repository_file("docs/history/README.md");
    assert!(history_index.contains("non-normative"));
    assert!(history_index.contains("[original specification](Specification.md)"));
    assert!(history_index.contains("[normative v0.2 contract](../v0.2-contract.md)"));

    for (surface, path) in [
        ("README", "README.md"),
        ("agent memory", "CLAUDE.md"),
        ("navigation guide", "AGENTIC_NAVIGATION_GUIDE.md"),
    ] {
        let contents = repository_file(path);
        assert!(
            contents.contains(HISTORICAL_PATH),
            "{surface} must point to the relocated historical record"
        );
    }

    for path in [
        HISTORICAL_PATH,
        "docs/history/README.md",
        AUDIT_PATH,
        NORMATIVE_PATH,
    ] {
        assert_balanced_code_fences(path);
        assert_local_markdown_links_resolve(path);
    }
}

#[test]
fn issue_68_classifies_every_rule_and_every_pull_request_21_proposal() {
    let audit = repository_file(AUDIT_PATH);
    let start = audit
        .find("<!-- issue-68-classification:start -->")
        .expect("classification start marker");
    let end = audit
        .find("<!-- issue-68-classification:end -->")
        .expect("classification end marker");
    assert!(start < end, "classification markers must be ordered");
    let classification = &audit[start..end];

    let mut rows = BTreeMap::new();
    for line in classification.lines() {
        if !line.starts_with("| H68-") {
            continue;
        }
        let columns = line
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        assert_eq!(
            columns.len(),
            6,
            "classification row must have exactly six columns: {line}"
        );
        assert!(
            rows.insert(columns[0].to_owned(), columns).is_none(),
            "duplicate classification row in {AUDIT_PATH}"
        );
    }

    let expected_ids = (1..=36)
        .map(|number| format!("H68-{number:03}"))
        .collect::<BTreeSet<_>>();
    let actual_ids = rows.keys().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        actual_ids, expected_ids,
        "the historical rule ledger must remain complete and exact"
    );

    let allowed_dispositions = [
        "Implemented/normative",
        "Deliberately changed",
        "Obsolete",
        "Unresolved",
    ];
    let fixture_sources = [
        repository_file("tests/fixtures/v0_2_contract.rs"),
        repository_file("tests/fixtures/v0_2_operations.rs"),
        repository_file("tests/fixtures/v0_2_trust.rs"),
    ]
    .join("\n");
    let fixture_pattern = Regex::new(
        r"`((?:marker|body|indent|path|choice|placeholder|ignore|operation|trust)-[^`]+)`",
    )
    .expect("valid fixture ID regex");

    for (id, columns) in &rows {
        let disposition = columns[2];
        assert!(
            allowed_dispositions.contains(&disposition),
            "{id} has unsupported disposition `{disposition}`"
        );
        assert!(
            !columns[1].is_empty()
                && !columns[3].is_empty()
                && !columns[4].is_empty()
                && !columns[5].is_empty(),
            "{id} must retain rule, authority, evidence, and rationale"
        );
        if disposition == "Implemented/normative" || disposition == "Deliberately changed" {
            let fixture_ids = fixture_pattern
                .captures_iter(columns[4])
                .map(|captures| captures[1].to_owned())
                .collect::<Vec<_>>();
            assert!(
                !fixture_ids.is_empty(),
                "{id} must cite applicable executable v0.2 evidence"
            );
            for fixture_id in fixture_ids {
                assert!(
                    fixture_sources.contains(&format!("id: \"{fixture_id}\"")),
                    "{id} cites unknown fixture ID `{fixture_id}`"
                );
            }
        }
        if disposition == "Unresolved" {
            assert!(
                columns[3].contains("#67"),
                "{id} must link unresolved CLI contract work to focused issue #67"
            );
        }
    }

    let disposition_counts = rows.values().fold(BTreeMap::new(), |mut counts, columns| {
        *counts.entry(columns[2]).or_insert(0_usize) += 1;
        counts
    });
    assert_eq!(
        disposition_counts.values().sum::<usize>(),
        36,
        "every substantive historical rule must have one disposition"
    );
    for required in allowed_dispositions {
        assert!(
            disposition_counts.contains_key(required),
            "the complete ledger must exercise disposition `{required}`"
        );
    }

    for required in [
        "694a1752aec9f3f29836fc9d006ea16f7cd7915b",
        "e6848333db81269bc8e311818c7f5e08058bed0f",
        "324498f7fbbcd8b4431cb920e3396c01e4d5e199",
        "https://github.com/plx/agentic-navigation-guide/issues/68#issuecomment-5088836259",
        "https://github.com/plx/agentic-navigation-guide/pull/21",
        "`operation-parse-tab-name`",
    ] {
        assert!(
            audit.contains(required),
            "disposition record must retain traceability `{required}`"
        );
    }

    let pull_request_ids = (1..=8)
        .map(|number| format!("P21-{number:02}"))
        .collect::<Vec<_>>();
    for id in pull_request_ids {
        assert_eq!(
            audit.matches(&format!("| {id} |")).count(),
            1,
            "PR #21 proposal {id} must have exactly one disposition"
        );
    }
}

#[test]
fn issue_68_contradictory_historical_tab_example_is_executable_and_rejected() {
    let temp = TempDir::new().expect("create temporary directory");
    let guide = temp.path().join("historical-tab-example.md");
    fs::write(
        &guide,
        "<agentic-navigation-guide>\n- Cargo.toml\t# historical text allowed arbitrary whitespace\n</agentic-navigation-guide>\n",
    )
    .expect("write fixed historical contradiction");

    let output = Command::new(env!("CARGO_BIN_EXE_agentic-navigation-guide"))
        .arg("check")
        .arg("--guide")
        .arg(&guide)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .expect("run packaged CLI parser");

    assert!(
        !output.status.success(),
        "the historical arbitrary-whitespace claim must not override the normative parser"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("line 2: invalid path format")
            && (stderr.contains("\\t")
                || stderr.contains("tab")
                || stderr.contains("control character")),
        "the fixed contradictory example must fail for its tab, got:\n{stderr}"
    );
}
