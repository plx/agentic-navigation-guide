# Issue #61: immutable, least-privilege workflow execution

Date: 2026-07-26

Issue: [#61 — Pin GitHub Actions and enforce least-privilege bounded workflow execution](https://github.com/plx/agentic-navigation-guide/issues/61)

## Outcome

Every executable GitHub Action reference is pinned to a reviewed 40-character
commit SHA with a human-readable release comment. All 13 checkout steps disable
credential persistence, all 14 jobs have timeouts, and all six workflows have
explicit permission and concurrency policy.

The CI workflow now installs checksum-verified `actionlint 1.7.12` and
`zizmor 1.25.2`, then runs both tools fail-closed. Zizmor receives the
read-only job token so its online action-reference audits are active, and
`--no-ignores` prevents a future inline or configuration suppression from
silently weakening the gate.

This remediation is entirely deterministic. It adds no fuzzing, mutation
testing, or unbounded input generation.

## Baseline

On the pre-remediation tree:

- `actionlint .github/workflows/*.yml` passed;
- `rg 'uses: .*@(v|main|master|latest)' .github/workflows` found six mutable
  references across the Claude and guide-verification workflows; and
- `zizmor 1.25.2 --pedantic .github/workflows/` exited 14 with 37 findings:
  2 informational, 8 low, 17 medium, and 8 high.

The findings covered unpinned actions, persisted checkout credentials,
workflow-default permissions, workflow-wide Pages/OIDC writes, undocumented
privileges, missing job names, and missing concurrency limits.

The new deterministic contract in
`tests/issue_61_workflow_security_policy.rs` initially failed all four tests:

1. immutable action and checkout-credential policy;
2. explicit permission, concurrency, and timeout policy;
3. trusted-checkout and scoped-token policy for secret-bearing jobs; and
4. fail-closed, checksum-pinned workflow lint policy.

## Reviewed action inventory

There are 30 executable action invocations using these eight reviewed
repository/revision pairs:

| Action | Immutable revision | Review comment |
| --- | --- | --- |
| `actions/checkout` | `93cb6efe18208431cddfb8368fd83d5badbf9bfd` | `v5.0.1` |
| `actions-rust-lang/setup-rust-toolchain` | `46268bd060767258de96ed93c1251119784f2ab6` | `v1.16.1` |
| `actions/setup-python` | `5fda3b95a4ea91299a34e894583c3862153e4b97` | `v7.0.0` |
| `taiki-e/install-action` | `c070f87102a1c75b3183910f391c1cb887fe13c8` | `v2.77.6` |
| `actions/setup-node` | `a0853c24544627f65ddf259abe73b1d18a591444` | `v5.0.0` |
| `actions/upload-pages-artifact` | `7b1f4a764d45c48632c6b24a0339c27f5614fb0b` | `v4.0.0` |
| `actions/deploy-pages` | `d6db90164ac5ed86f2b6aed7e0febac5b3c0c03e` | `v4.0.5` |
| `anthropics/claude-code-action` | `be7b93b1907a4abad570368f3c74b6fe3807510b` | `v1.0.183` |

The annotated upstream `v1.0.183` Claude tag resolves to the pinned
`be7b93b...` commit. All checkout steps use the already-reviewed v5.0.1 pin;
this avoids the hosted Node 20 deprecation warning emitted by checkout v4.

## Permission and trust decisions

| Workflow/job | Effective capability | Bound |
| --- | --- | --- |
| CI, site checks, guide verification | `contents: read` | Per-PR/ref concurrency with cancellation; 10–30 minute job timeouts |
| Claude PR review | `contents: read`, `pull-requests: write` | Internal PRs only; trusted initial base SHA checkout; PR-only allowed shell tools; 30-minute timeout; PR-scoped cancellation |
| Claude mention response | `actions: read`, `contents: write`, `issues: write`, `pull-requests: write` | Workflow-native maintainer association gate plus action-enforced write-access check; issue comments exclude PRs and review events require an internal PR; 30-minute timeout; post-gate issue/PR concurrency |
| Pages build | `contents: read` | 15-minute timeout |
| Pages deploy | `pages: write`, `id-token: write` | Deployment job only; protected `github-pages` environment; 10-minute timeout; serialized deployment concurrency |

Workflow-level permissions are empty for Claude and Pages publishing, so only
the listed jobs receive elevated capabilities. Pages deployment is the sole
remaining OIDC use, and the inline workflow comment records why it is needed.

### Claude authentication decision

The reviewed Claude action source checks its `github_token` input before
requesting an OIDC token. Both Claude workflows now pass the job-scoped
`${{ github.token }}` explicitly, so the action does not perform its default
OIDC-to-App-token exchange and `id-token: write` is removed.

The OAuth-bearing review job runs only for same-repository PRs and initially
checks out `pull_request.base.sha`. The Claude action subsequently obtains the
same-repository PR branch for review, but its allowed shell tools are limited
to reading and commenting on that PR; it cannot execute workspace commands.

The mention workflow requires `OWNER`, `MEMBER`, or `COLLABORATOR` association
for the exact event content that contains `@claude`, then participates in
job-level concurrency only after that trust/mention condition passes. Issue
comments must refer to issues rather than PRs. Review and review-comment events
must target a branch in this repository. An unrelated or untrusted event
therefore cannot cancel a legitimate run or introduce an external PR checkout.
The action retains its independent rule that only actors with repository write
access can trigger execution.

The mention workflow initially checks out the trusted default branch. A live
same-repository `pull_request_review_comment` run confirmed that the action
then selects the PR branch after both trust gates, using the scoped job token.
Both Claude workflows keep `show_full_output: false`.

Secrets are passed only as action inputs. No `run:` command interpolates a
secret, and no `actions/checkout` step persists the job token. Hosted log
inspection showed token-bearing fields only as `***`; no token value appeared.

## Scanner enforcement

The workflow-lint job downloads official Linux archives for:

- `actionlint 1.7.12`, SHA-256
  `8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8`;
- `zizmor 1.25.2`, SHA-256
  `aa1facd105f0d83fe5c55b1adcd9d7417de5d83aa27471f91dc0b66cf3803577`.

Both archives are verified before extraction. Neither scanner step is
informational or `continue-on-error`.

Zizmor's success footer says `(2 suppressed)` even when run with
`--no-ignores`. The repository contains no zizmor configuration or ignore
directive, and the CI command disables both forms of user-authored
suppression. The footer therefore does not represent an accepted repository
finding; reported findings are zero with online audits enabled.

## Validation

| Command | Result |
| --- | --- |
| `cargo test --locked --test issue_61_workflow_security_policy -- --nocapture` | Pass; 4 deterministic policy tests |
| `cargo +1.85.0 test --locked --test issue_61_workflow_security_policy -- --nocapture` | Pass; contract holds on the MSRV |
| `cargo test --locked --all-targets --all-features` | Pass; 344 passed, 2 intentionally ignored |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass |
| `cargo fmt -- --check` | Pass |
| `cargo package --locked --allow-dirty` | Pass; 126 files packaged and verified |
| `actionlint .github/workflows/*.yml` | Pass |
| `GH_TOKEN=… zizmor --pedantic --no-ignores .github/workflows/` | Pass; zero reported findings with online audits enabled |
| Mutable action-reference `rg` check | Pass; no matches |
| `cargo check --locked --all-targets --all-features` | Pass |
| `cargo run --locked -- check --guide AGENTIC_NAVIGATION_GUIDE.md` | Pass |
| `cargo run --locked -- verify --guide AGENTIC_NAVIGATION_GUIDE.md --root .` | Pass |
| `just --fmt --check` | Pass |
| `python3 -m py_compile scripts/get_next_production_readiness_issue.py` | Pass |
| `just test-production-readiness-selector` | Pass; 61 tests |
| Site format, spelling, Astro check/build, and Playwright suite | Pass; 27 browser tests |

The pull request's hosted runs provide event-level proof for normal CI, the
internal pull-request Claude review, and the release dry-run. A trusted
maintainer `pull_request_review_comment` also exercised the mention path on
commit `4a8c62d`; run `30234018781` passed in 94 seconds and the action's
response independently confirmed the post-gate concurrency fix. Pages
deployment retains a dedicated protected-environment job and was not
artificially triggered by this remediation.

One earlier local full-suite attempt overlapped a separate `cargo run` in the
same target directory after Cargo released its build lock. Eight CLI tests in
that invalid concurrent attempt temporarily lost the test binary. The exact
full-suite command was rerun alone and passed with the counts above.
