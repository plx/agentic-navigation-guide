use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn repository_file(path: &str) -> String {
    fs::read_to_string(repository_root().join(path))
        .unwrap_or_else(|error| panic!("read repository file {path}: {error}"))
}

fn normalized_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn issue_70_contributor_guide_is_complete_and_uses_real_pinned_commands() {
    let guide = repository_file("CONTRIBUTING.md");
    let normalized = normalized_whitespace(&guide);
    let ci = repository_file(".github/workflows/ci.yml");
    let justfile = repository_file("justfile");
    let workflow_lint = ci
        .split_once("  workflow-lint:\n")
        .expect("CI must contain the workflow-lint job")
        .1
        .split_once("\n  issue-selector:\n")
        .expect("workflow-lint must precede the issue-selector job")
        .0;
    let workflow_sources = format!(
        "{ci}\n{}",
        repository_file(".github/workflows/verify-guide.yml")
    );

    for required in [
        "# Contributing",
        "## Scope and supported environment",
        "## Prepare a trusted checkout",
        "## Choose one issue",
        "## Red-before-fix workflow",
        "## Test and fixture rules",
        "## Validation matrix",
        "## Dependencies, licenses, and release-sensitive files",
        "## Security and sensitive data",
        "## Pull requests and review",
        "## Maintainer triage",
        "personal repository with one maintainer and no GitHub organization",
        "The static site is outside this CLI audit",
        "Rust `1.85.0`",
        "Rust `1.96.1`",
        "Rust `1.97.1`",
        "Python `3.12`",
        "`just 1.51.0`",
        "cargo +1.85.0 fetch --locked",
        "cargo +1.85.0 check --locked --all-targets --all-features",
        "cargo +1.85.0 test --locked --all-targets --all-features",
        "cargo +1.85.0 clippy --locked --all-targets --all-features -- -D warnings",
        "cargo +1.97.1 check --locked --all-targets --all-features",
        "cargo +1.97.1 test --locked --all-targets --all-features",
        "cargo +1.97.1 clippy --locked --all-targets --all-features -- -D warnings",
        "cargo fmt -- --check",
        "GUIDE_FORMAT_REQUIRE_CONFORMANCE=all cargo test --workspace --all-targets --all-features --locked -- --nocapture",
        "cargo run --release --locked -- verify --github-actions-check --deny-ignored",
        "cargo package --locked",
        "cargo publish --dry-run --locked",
        "just test-production-readiness-selector",
        "just test-release-identity",
        "just test-github-protections",
        "just test-quality-gates",
        "just test-contributor-templates",
        "cargo test --locked parser_robustness_tests:: -- --nocapture",
        "No fuzz target or corpus exists",
        "Issue #56 remains deferred",
        "fixed 15-sentinel mutation job",
        "test fails for the reported reason before the implementation changes",
        "Do not commit a deliberately failing state to `main`",
        "base commit SHA, exact command, nonzero status, and expected failure reason",
        "native `blocked by` and sub-issue relationships",
        "exactly one issue-scoped closing directive",
        "Closes #NUMBER",
        "TempDir",
        "process-global current directory or environment",
        "Linux, macOS, and Windows",
        "resolved review conversations",
    ] {
        assert!(
            normalized.contains(required),
            "CONTRIBUTING.md omits required workflow text {required:?}"
        );
    }

    for (documented, workflow_pin) in [
        ("Rust `1.85.0`", "rust: \"1.85.0\""),
        ("Rust `1.96.1`", "rust: \"1.96.1\""),
        ("Rust `1.97.1`", "rust: \"1.97.1\""),
        ("`just 1.51.0`", "tool: just@1.51.0"),
        ("`cargo-llvm-cov 0.8.7`", "tool: cargo-llvm-cov@0.8.7"),
        ("`cargo-mutants 27.1.0`", "tool: cargo-mutants@27.1.0"),
        ("`cargo-about 0.9.0`", "tool: cargo-about@0.9.0"),
        ("`actionlint 1.7.12`", "ACTIONLINT_VERSION: 1.7.12"),
        ("`zizmor 1.25.2`", "ZIZMOR_VERSION: 1.25.2"),
        ("`rumdl 0.2.43`", "RUMDL_VERSION: 0.2.43"),
        ("`lychee 0.24.2`", "LYCHEE_VERSION: 0.24.2"),
    ] {
        assert!(
            guide.contains(documented),
            "CONTRIBUTING.md omits documented pin {documented:?}"
        );
        assert!(
            ci.contains(workflow_pin),
            "CI omits contributor-guide pin source {workflow_pin:?}"
        );
    }
    assert!(guide.contains("Python `3.12`"));
    assert!(workflow_lint.contains("python-version: \"3.12\""));
    assert!(workflow_lint.contains("tool: just@1.51.0"));
    assert!(workflow_lint.contains("just test-contributor-templates"));
    assert!(justfile.contains("test-contributor-templates:"));
    assert!(
        justfile.contains(
            "PYTHONDONTWRITEBYTECODE=1 python3 -m unittest tests/test_check_contributor_templates.py -v"
        )
    );
    assert!(justfile.contains("python3 scripts/check_contributor_templates.py"));

    for command in [
        "cargo check --locked --all-targets --all-features",
        "cargo test --locked --all-targets --all-features",
        "cargo clippy --locked --all-targets --all-features -- -D warnings",
        "cargo fmt -- --check",
        "cargo package --locked",
        "cargo publish --dry-run --locked",
        "cargo run --release --locked -- verify --github-actions-check --deny-ignored",
    ] {
        assert!(
            workflow_sources.contains(command),
            "documented validation command is absent from CI: {command:?}"
        );
    }
}

#[test]
fn issue_70_templates_capture_required_issue_and_pull_request_evidence() {
    let bug = repository_file(".github/ISSUE_TEMPLATE/01-bug.yml");
    let proposal = repository_file(".github/ISSUE_TEMPLATE/02-contract-proposal.yml");
    let chooser = repository_file(".github/ISSUE_TEMPLATE/config.yml");
    let pull_request = repository_file(".github/pull_request_template.md");
    let normalized_pr = normalized_whitespace(&pull_request);

    for required in [
        "id: \"version\"",
        "id: \"platform\"",
        "id: \"observed\"",
        "id: \"expected\"",
        "id: \"reproduction\"",
        "id: \"regression\"",
        "id: \"compatibility\"",
        "id: \"security\"",
        "security/advisories/new",
    ] {
        assert!(bug.contains(required), "bug form omits {required:?}");
    }
    for required in [
        "id: \"current-contract\"",
        "id: \"proposed-contract\"",
        "id: \"compatibility\"",
        "id: \"platforms\"",
        "id: \"security\"",
        "id: \"dependencies\"",
        "native `blocked by`",
    ] {
        assert!(
            proposal.contains(required),
            "contract-proposal form omits {required:?}"
        );
    }
    for required in [
        "blank_issues_enabled: false",
        "https://github.com/plx/agentic-navigation-guide/security/advisories/new",
        "Private vulnerability report",
    ] {
        assert!(
            chooser.contains(required),
            "template chooser omits {required:?}"
        );
    }
    for heading in [
        "## Problem",
        "## Before behavior",
        "## After behavior",
        "## Red-before-fix evidence",
        "## Validation",
        "## Documentation and compatibility",
        "## Security and sensitive data",
        "## Dependencies and issue graph",
        "## Checklist",
    ] {
        assert!(
            pull_request.contains(heading),
            "pull request template omits heading {heading:?}"
        );
    }
    for required in [
        "base commit SHA",
        "exact pre-fix command",
        "focused post-fix",
        "full post-fix",
        "Linux, macOS, and Windows",
        "documentation impact",
        "compatibility impact",
        "security impact",
        "dependency or license impact",
        "Closes #NUMBER",
    ] {
        assert!(
            normalized_pr.contains(required),
            "pull request template omits evidence prompt {required:?}"
        );
    }
}

#[test]
fn issue_70_ci_docs_and_audit_keep_the_contributor_contract_current() {
    let ci = repository_file(".github/workflows/ci.yml");
    let justfile = repository_file("justfile");
    let readme = repository_file("README.md");
    let changelog = repository_file("CHANGELOG.md");
    let navigation = repository_file("AGENTIC_NAVIGATION_GUIDE.md");
    let audit = repository_file("audits/2026-07-27-issue-70-contributor-workflow.md");

    assert!(ci.contains("just test-contributor-templates"));
    assert!(justfile.contains("python3 scripts/check_contributor_templates.py"));
    assert!(justfile.contains("tests/test_check_contributor_templates.py -v"));
    assert!(
        ci.contains("rumdl check --disable MD013 CONTRIBUTING.md .github/pull_request_template.md")
    );
    assert!(ci.contains(
        "lychee --no-progress README.md CONTRIBUTING.md SECURITY.md .github/pull_request_template.md"
    ));
    assert!(readme.contains("[contribution guide](CONTRIBUTING.md)"));
    assert!(changelog.contains("structured bug and contract-proposal issue forms"));

    for path in [
        "- CONTRIBUTING.md",
        "- ISSUE_TEMPLATE/",
        "- pull_request_template.md",
        "- check_contributor_templates.py",
        "- issue_70_contributor_workflow.rs",
        "- test_check_contributor_templates.py",
        "- 2026-07-27-issue-70-contributor-workflow.md",
    ] {
        assert!(
            navigation.contains(path),
            "navigation guide omits contributor artifact {path:?}"
        );
    }
    for evidence in [
        "Deterministic red-before evidence",
        "Clean-checkout command exercise",
        "Template validation and preview",
        "Representative-ticket cold read",
        "No fuzz target",
        "personal repository",
        "Issue #63",
    ] {
        assert!(
            audit.contains(evidence),
            "issue #70 audit omits evidence {evidence:?}"
        );
    }
}
