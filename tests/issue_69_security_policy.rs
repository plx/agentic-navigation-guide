use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn repository_file(path: &str) -> String {
    fs::read_to_string(repository_root().join(path))
        .unwrap_or_else(|error| panic!("read repository file {path}: {error}"))
}

fn parse_flat_string_record(source: &str) -> BTreeMap<String, String> {
    let mut record = BTreeMap::new();
    for (index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, raw_value) = line
            .split_once('=')
            .unwrap_or_else(|| panic!("record line {} has no equals sign", index + 1));
        let key = key.trim();
        let value = raw_value
            .trim()
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or_else(|| panic!("record value {key:?} must be one quoted string"));
        assert!(
            record.insert(key.to_owned(), value.to_owned()).is_none(),
            "duplicate status-record key {key:?}"
        );
    }
    record
}

fn normalized_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn quoted_assignment(source: &str, key: &str) -> String {
    let prefix = format!("{key} = \"");
    source
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix(&prefix)
                .and_then(|value| value.strip_suffix('"'))
        })
        .unwrap_or_else(|| panic!("missing quoted assignment {key:?}"))
        .to_owned()
}

#[test]
fn issue_69_policy_has_a_real_route_supported_versions_and_feasible_response() {
    let policy = repository_file("SECURITY.md");
    let normalized = normalized_whitespace(&policy);
    let version = quoted_assignment(&repository_file("release/identity.toml"), "version");

    for required in [
        "# Security Policy",
        "## Supported versions",
        "## Report a vulnerability privately",
        "## What to include",
        "## Response and coordinated disclosure",
        "## Security model and limitations",
        "GitHub private vulnerability reporting",
        "https://github.com/plx/agentic-navigation-guide/security/advisories/new",
        "Do not open a public GitHub issue",
        "best-effort targets, not service-level guarantees",
        "seven calendar days",
        "fourteen calendar days",
        "newest non-yanked `0.2.x` release",
        "`0.1.4` and every other `0.1.x` release",
        "not a filesystem sandbox",
        "hostile concurrent replacement",
        "raw guide source lines",
        "resolved external targets",
        "operating-system time, memory, and filesystem-access limits",
    ] {
        assert!(
            normalized.contains(required),
            "SECURITY.md omits required policy text {required:?}"
        );
    }
    assert!(
        normalized.contains(&format!("`{version}` (prepared, not yet published)")),
        "supported-version table must include the exact prepared release identity"
    );

    for placeholder in [
        "security@example",
        "example.com/security",
        "<email>",
        "TODO",
        "TBD",
    ] {
        assert!(
            !policy.contains(placeholder),
            "SECURITY.md contains placeholder contact text {placeholder:?}"
        );
    }
}

#[test]
fn issue_69_runbook_and_status_record_are_complete_and_non_secret() {
    let runbook = normalized_whitespace(&repository_file("docs/security-response-runbook.md"));
    for required in [
        "# Vulnerability response runbook",
        "## Private intake and record ownership",
        "## Severity and scope decision",
        "## Embargoed fix",
        "## Release and advisory coordination",
        "## Disclosure and follow-up",
        "GitHub Security Advisory",
        "temporary private fork",
        "CVSS",
        "GHSA",
        "CVE",
        "CHANGELOG.md",
        "single maintainer",
        "publication stops",
    ] {
        assert!(
            runbook.contains(required),
            "response runbook omits {required:?}"
        );
    }

    let record_source = repository_file("release/security-response.toml");
    let record = parse_flat_string_record(&record_source);
    let expected = [
        ("schema_version", "1"),
        ("recorded_on", "2026-07-27"),
        ("repository", "plx/agentic-navigation-guide"),
        ("policy_path", "SECURITY.md"),
        ("route", "github-private-vulnerability-reporting"),
        ("route_status", "enabled"),
        ("owner", "plx"),
        ("backup_owner", "none"),
        ("private_record_owner", "plx"),
        ("test_advisory", "GHSA-5qph-7jv3-m93c"),
        ("test_advisory_status", "closed-unpublished-empty-products"),
        ("test_private_fork", "none"),
        (
            "owner_report_probe",
            "denied-as-designed-use-owner-draft-path",
        ),
    ];
    for (key, value) in expected {
        assert_eq!(
            record.get(key).map(String::as_str),
            Some(value),
            "security-response record field {key:?}"
        );
    }
    for forbidden in [
        "ghp_",
        "github_pat_",
        "CARGO_REGISTRY_TOKEN=",
        "BEGIN OPENSSH PRIVATE KEY",
    ] {
        assert!(
            !record_source.contains(forbidden),
            "security-response record contains token-like material {forbidden:?}"
        );
    }
}

#[test]
fn issue_69_docs_ci_and_audit_keep_every_security_claim_in_scope() {
    let readme = repository_file("README.md");
    let contract = repository_file("docs/v0.2-contract.md");
    let continuity = repository_file("docs/maintainer-continuity.md");
    let continuity_record = repository_file("release/maintainer-continuity.toml");
    let changelog = repository_file("CHANGELOG.md");
    let guide = repository_file("AGENTIC_NAVIGATION_GUIDE.md");
    let ci = repository_file(".github/workflows/ci.yml");
    let audit = repository_file("audits/2026-07-27-issue-69-security-policy.md");

    assert!(readme.contains("[security policy](SECURITY.md)"));
    assert!(!readme.contains("No private vulnerability-report route is currently published"));
    assert!(contract.contains("[security policy](../SECURITY.md)"));
    assert!(!contract.contains("At the completion of issue #67, no private"));
    assert!(continuity.contains("GitHub private vulnerability reporting is enabled"));
    assert!(continuity_record.contains(
        "security_report_route_status = \"github-private-vulnerability-reporting-enabled\""
    ));
    assert!(changelog.contains("private vulnerability reporting"));

    for path in [
        "- SECURITY.md",
        "- security-response-runbook.md",
        "- security-response.toml",
        "- issue_69_security_policy.rs",
        "- 2026-07-27-issue-69-security-policy.md",
    ] {
        assert!(guide.contains(path), "navigation guide omits {path:?}");
    }
    assert!(
        ci.contains("rumdl check --disable MD013 CONTRIBUTING.md .github/pull_request_template.md")
    );
    assert!(ci.contains("rumdl check SECURITY.md docs/security-response-runbook.md"));
    assert!(ci.contains(
        "lychee --no-progress README.md CONTRIBUTING.md SECURITY.md .github/pull_request_template.md"
    ));

    for evidence in [
        "Issue #35",
        "Issue #49",
        "Issue #51",
        "Issue #61",
        "trust-guide-default-link-outside-relative",
        "trust-containment-target-redaction",
        "workflow_lint_is_fail_closed_and_checksum_pins_every_tool",
        "tabletop",
        "GHSA-5qph-7jv3-m93c",
        "No fuzzing",
    ] {
        assert!(
            audit.contains(evidence),
            "issue #69 audit omits evidence {evidence:?}"
        );
    }
}
