use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn workflow_files() -> Vec<(PathBuf, String)> {
    let workflow_dir = repository_root().join(".github/workflows");
    let mut workflows = fs::read_dir(&workflow_dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", workflow_dir.display()))
        .map(|entry| {
            entry
                .expect("failed to read workflow directory entry")
                .path()
        })
        .filter(|path| {
            matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("yml" | "yaml")
            )
        })
        .map(|path| {
            let contents = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            (path, contents)
        })
        .collect::<Vec<_>>();
    workflows.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(!workflows.is_empty(), "expected GitHub Actions workflows");
    workflows
}

fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn step_block(lines: &[&str], start: usize) -> String {
    let step_indent = leading_spaces(lines[start]);
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| {
            (leading_spaces(line) <= step_indent && line.trim_start().starts_with("- "))
                .then_some(index)
        })
        .unwrap_or(lines.len());
    lines[start..end].join("\n")
}

fn job_block(workflow: &str, job_name: &str) -> String {
    let lines = workflow.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| *line == format!("  {job_name}:"))
        .unwrap_or_else(|| panic!("missing job {job_name}"));
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| {
            (leading_spaces(line) == 2
                && !line.trim().is_empty()
                && !line.trim_start().starts_with('#'))
            .then_some(index)
        })
        .unwrap_or(lines.len());
    lines[start..end].join("\n")
}

#[test]
fn every_remote_action_is_immutable_and_checkout_drops_credentials() {
    let uses = Regex::new(r"^\s*(?:-\s+)?uses:\s*([^#\s]+)(?:\s+#\s*(.+))?$").unwrap();
    let immutable_sha = Regex::new(r"^[0-9a-f]{40}$").unwrap();
    let version_comment = Regex::new(r"^v[0-9]").unwrap();

    for (path, workflow) in workflow_files() {
        let lines = workflow.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            let Some(captures) = uses.captures(line) else {
                continue;
            };
            let action = captures.get(1).unwrap().as_str();
            if action.starts_with("./") || action.starts_with("docker://") {
                continue;
            }

            let (repository, revision) = action.rsplit_once('@').unwrap_or_else(|| {
                panic!("{}:{} has no action revision", path.display(), index + 1)
            });
            assert!(
                immutable_sha.is_match(revision),
                "{}:{} uses mutable action reference {action}",
                path.display(),
                index + 1
            );
            let review_comment = captures.get(2).map(|capture| capture.as_str().trim());
            assert!(
                review_comment.is_some_and(|comment| version_comment.is_match(comment)),
                "{}:{} must retain a human-readable release comment for {repository}",
                path.display(),
                index + 1
            );

            if repository == "actions/checkout" {
                assert_eq!(
                    action,
                    "actions/checkout@93cb6efe18208431cddfb8368fd83d5badbf9bfd",
                    "{}:{} must use the reviewed Node 24 checkout release",
                    path.display(),
                    index + 1
                );
                assert_eq!(review_comment, Some("v5.0.1"));
                let block = step_block(&lines, index);
                assert!(
                    block.contains("persist-credentials: false"),
                    "{}:{} must disable checkout credential persistence",
                    path.display(),
                    index + 1
                );
            }
        }
    }
}

#[test]
fn every_workflow_and_job_has_explicit_permissions_and_execution_bounds() {
    for (path, workflow) in workflow_files() {
        assert!(
            workflow
                .lines()
                .any(|line| line == "permissions:" || line == "permissions: {}"),
            "{} must declare workflow-level permissions",
            path.display()
        );
        let lines = workflow.lines().collect::<Vec<_>>();
        let workflow_has_concurrency = lines.contains(&"concurrency:");
        if workflow_has_concurrency {
            assert!(
                lines.iter().any(|line| line.starts_with("  group:"))
                    && lines
                        .iter()
                        .any(|line| line.starts_with("  cancel-in-progress:")),
                "{} workflow concurrency must declare a group and cancellation behavior",
                path.display()
            );
        }
        let jobs_start = lines
            .iter()
            .position(|line| *line == "jobs:")
            .unwrap_or_else(|| panic!("{} has no jobs section", path.display()));
        let job_starts = lines
            .iter()
            .enumerate()
            .skip(jobs_start + 1)
            .filter_map(|(index, line)| {
                (leading_spaces(line) == 2
                    && line.ends_with(':')
                    && !line.trim_start().starts_with('#'))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        assert!(!job_starts.is_empty(), "{} has no jobs", path.display());

        for (position, start) in job_starts.iter().enumerate() {
            let end = job_starts.get(position + 1).copied().unwrap_or(lines.len());
            let block = lines[*start..end].join("\n");
            assert!(
                block
                    .lines()
                    .any(|line| line.starts_with("    timeout-minutes:")),
                "{} job {} must set timeout-minutes",
                path.display(),
                lines[*start].trim_end_matches(':').trim()
            );
            if !workflow_has_concurrency {
                assert!(
                    block.lines().any(|line| line == "    concurrency:")
                        && block.lines().any(|line| line.starts_with("      group:"))
                        && block
                            .lines()
                            .any(|line| line.starts_with("      cancel-in-progress:")),
                    "{} job {} must declare bounded concurrency",
                    path.display(),
                    lines[*start].trim_end_matches(':').trim()
                );
            }
        }
    }
}

#[test]
fn secret_bearing_claude_jobs_use_trusted_checkouts_and_scoped_tokens() {
    let review =
        fs::read_to_string(repository_root().join(".github/workflows/claude-code-review.yml"))
            .expect("failed to read Claude review workflow");
    assert!(!review.contains("id-token: write"));
    assert!(review.contains("github_token: ${{ github.token }}"));
    assert!(
        review.contains("if: github.event.pull_request.head.repo.full_name == github.repository")
    );
    assert!(review.contains("ref: ${{ github.event.pull_request.base.sha }}"));
    assert!(review.contains("show_full_output: false"));
    assert!(!review.contains("pull_request_target:"));

    let mentions = fs::read_to_string(repository_root().join(".github/workflows/claude.yml"))
        .expect("failed to read Claude mention workflow");
    assert!(!mentions.contains("id-token: write"));
    assert!(mentions.contains("github_token: ${{ github.token }}"));
    assert!(mentions.contains("ref: ${{ github.event.repository.default_branch }}"));
    assert!(mentions.contains("show_full_output: false"));
    assert!(!mentions.contains("allowed_non_write_users:"));
    assert!(mentions.contains("github.event.issue.pull_request == null"));
    assert_eq!(
        mentions
            .matches("github.event.pull_request.head.repo.full_name == github.repository")
            .count(),
        2
    );
    assert!(!mentions.lines().any(|line| line == "concurrency:"));
    assert!(job_block(&mentions, "claude").contains("    concurrency:"));
    assert_eq!(
        mentions
            .matches(r#"fromJSON('["OWNER","MEMBER","COLLABORATOR"]')"#)
            .count(),
        4
    );
    for association in [
        "github.event.comment.author_association",
        "github.event.review.author_association",
        "github.event.issue.author_association",
    ] {
        assert!(
            mentions.contains(association),
            "Claude mention workflow must gate {association}"
        );
    }

    for (path, workflow) in workflow_files() {
        assert!(
            !workflow.contains("show_full_output: true"),
            "{} must not expose full model/tool output",
            path.display()
        );
        let lines = workflow.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            if line.trim_start().starts_with("run:") {
                assert!(
                    !step_block(&lines, index).contains("${{ secrets."),
                    "{}:{} must not interpolate a secret into a run command",
                    path.display(),
                    index + 1
                );
            }
        }
    }
}

#[test]
fn workflow_lint_is_fail_closed_and_checksum_pins_every_tool() {
    let ci = fs::read_to_string(repository_root().join(".github/workflows/ci.yml"))
        .expect("failed to read CI workflow");
    let lint = job_block(&ci, "workflow-lint");

    assert!(lint.contains("ACTIONLINT_VERSION: 1.7.12"));
    assert!(lint.contains(
        "ACTIONLINT_SHA256: 8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"
    ));
    assert!(lint.contains("ZIZMOR_VERSION: 1.25.2"));
    assert!(lint.contains(
        "ZIZMOR_SHA256: aa1facd105f0d83fe5c55b1adcd9d7417de5d83aa27471f91dc0b66cf3803577"
    ));
    assert!(lint.contains("LYCHEE_VERSION: 0.24.2"));
    assert!(lint.contains(
        "LYCHEE_SHA256: 1f4e0ef7f6554a6ed33dd7ac144fb2e1bbed98598e7af973042fc5cd43951c9a"
    ));
    assert!(lint.contains("RUMDL_VERSION: 0.2.43"));
    assert!(lint.contains(
        "RUMDL_SHA256: 01e0dd2d89c07d244c5c93243f7faf2986d2abec68a7cec458e38c25988fbabc"
    ));
    assert_eq!(
        lint.matches(r#"echo "$RUNNER_TEMP" >> "$GITHUB_PATH""#)
            .count(),
        3,
        "each scanner installer must export its executable directory"
    );
    assert!(lint.contains("actionlint .github/workflows/*.yml .github/examples/*.yml"));
    assert!(lint.contains("GH_TOKEN: ${{ github.token }}"));
    assert!(lint.contains(
        "zizmor --pedantic --no-ignores .github/workflows/ .github/examples/readme-verify.yml"
    ));
    assert!(lint.contains("rumdl check --disable MD010,MD013,MD038 README.md"));
    assert!(lint.contains("rumdl check SECURITY.md docs/security-response-runbook.md"));
    assert!(lint
        .contains("lychee --no-progress README.md SECURITY.md .github/examples/readme-verify.yml"));
    assert!(!lint.contains("continue-on-error"));
}
