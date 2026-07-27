# Contributing

Thank you for improving `agentic-navigation-guide`. This repository values
small, evidence-backed changes over broad cleanup. Contributions submitted
intentionally are licensed under the repository's MIT OR Apache-2.0 terms.

## Scope and supported environment

The supported product is the installed Rust CLI. The
[v0.2 contract](docs/v0.2-contract.md) is normative; the
[README](README.md) is the concise entry point. The static site is outside this
CLI audit and should not be changed as part of ordinary product work.

This is a personal repository with one maintainer and no GitHub organization,
team, backup administrator, or independent reviewer. Follow the
[dated sole-maintainer policy](docs/maintainer-continuity.md) rather than
assuming an organization-only approval or recovery path.

The supported toolchain and host matrix is:

| Tool or host | Required use |
| --- | --- |
| Rust `1.85.0` | MSRV check, tests, Clippy, package, and installed-package smoke |
| Rust `1.96.1` | Stable-minus-one check and tests |
| Rust `1.97.1` | Current-stable check, tests, and Clippy |
| Linux, macOS, and Windows | Complete debug/release behavior and platform capability gates |
| Python `3.12` | Repository policy checkers and their standard-library tests |
| `just 1.51.0` | Versioned repository task entry points |

CI additionally pins nightly `2025-11-04`, `cargo-llvm-cov 0.8.7`,
`cargo-mutants 27.1.0`, `cargo-about 0.9.0`, `actionlint 1.7.12`,
`zizmor 1.25.2`, `rumdl 0.2.43`, and `lychee 0.24.2`. Use those exact
versions when reproducing the corresponding hosted job. Do not silently
upgrade an auxiliary tool inside an unrelated contribution.

## Prepare a trusted checkout

Clone only into a directory you control. Repository contents and guide text
are untrusted inputs; the executable, command-line arguments, environment,
operating system, and process credentials remain trusted under the
[security policy](SECURITY.md).

```sh
git clone https://github.com/plx/agentic-navigation-guide.git
cd agentic-navigation-guide
rustup toolchain install 1.85.0 --profile minimal --component clippy,rustfmt
rustup toolchain install 1.96.1 --profile minimal
rustup toolchain install 1.97.1 --profile minimal --component clippy,rustfmt
cargo +1.85.0 fetch --locked
python3 --version
just --version
```

`cargo fetch --locked` obtains the graph reviewed in `Cargo.lock`; do not
remove `--locked` merely to make resolution succeed. Do not redirect Cargo or
git configuration into a contributor's home directory, and do not require a
registry token for ordinary build or test work.

## Choose one issue

Use `just get-next-production-readiness-issue` when working through the
release-readiness queue, or select one explicitly approved issue. Read its
body, comments, native `blocked by` and sub-issue relationships, and linked
decisions before editing. An open closing pull request covers work in progress;
it does not make an external prerequisite complete.

Create one topic branch and keep one issue per pull request. Do not bundle a
new finding, opportunistic refactor, dependency refresh, formatting sweep, or
site change. Open a separate issue with the appropriate labels and native
dependency relationship. If a contract decision is missing, add
`needs-decision` and escalate it publicly instead of guessing.

The pull request body ends with exactly one issue-scoped closing directive.
Replace the template placeholder `Closes #NUMBER` with the selected issue
number. Do not add a second `Fixes`, `Closes`, or `Resolves` directive.

## Red-before-fix workflow

Every defect fix begins with the smallest deterministic regression at the
narrowest useful layer:

1. Branch from current `main` and record the base commit SHA.
2. Add only the proposed regression. Before implementation changes, run its
   focused command and confirm the test fails for the reported reason before
   the implementation changes. An unrelated compile, setup, timeout, or
   assertion failure is not evidence.
3. Record the base commit SHA, exact command, nonzero status, and expected
   failure reason in the issue audit or pull request.
4. Implement the smallest correction, rerun the focused test, and then run the
   complete relevant validation matrix.
5. Commit and publish the final green test and implementation together. Do not
   commit a deliberately failing state to `main`; a pull request may describe
   the observed test-first working-tree failure without preserving a broken
   commit.

Documentation, policy, and hosted-control issues use the same method: add a
deterministic contract check that fails because the required artifact or
control is absent, then implement it. Never weaken an existing test to
manufacture red-before evidence.

## Test and fixture rules

- Put pure behavior tests next to the private module and CLI/package or
  cross-document contracts under `tests/`. Use an issue-numbered name when an
  acceptance contract owns the evidence.
- Build fixed, minimal fixtures. There is no network, clock, process-global
  state, developer checkout, or assertion-free smoke test in a regression.
- Use `tempfile::TempDir` or the existing owned subprocess harnesses. A test
  must not mutate the process-global current directory or environment, or a
  developer's Cargo, rustup, git, credential, registry, home, or system state.
- Remove the five guide-configuration variables from child processes unless
  the test explicitly owns one. Give every subprocess an owned working
  directory and reap it before the temporary root is dropped.
- Gate symlink, reparse, non-UTF-8, permission, and other capability-specific
  cases with the real platform capability. Unsupported hosts must record an
  explicit unavailable result; a silent skip cannot satisfy a supported-host
  claim.
- Keep fixtures bounded and deterministic. Preserve exact names, bytes, order,
  seed labels, and expected diagnostics. Do not use random input to replace a
  complete equivalence-class matrix.

No fuzz target or corpus exists. Do not add or run fuzzing as part of ordinary
contributor work. Issue #56 remains deferred and requires a new explicit
maintainer decision before any generated-property target, seed, or corpus is
introduced. Parser reliability is currently exercised by the deterministic
issue #57 matrices:

```sh
cargo test --locked parser_robustness_tests:: -- --nocapture
```

The only mutation process is the fixed 15-sentinel mutation job in CI. It is a
reviewed deterministic original-blocker set, not authorization for a broader
mutation campaign. Changes to its modules must preserve the exact filter,
bounded jobs/timeouts, complete report, and zero survivors or timeouts.

## Validation matrix

Run the focused regression first. The following example selects the issue #70
contract; replace only the final test filter for another issue:

```sh
cargo +1.85.0 test --locked --test issue_70_contributor_workflow -- --nocapture
```

For every Rust or maintained-contract change, run:

```sh
cargo fmt -- --check
cargo +1.85.0 check --locked --all-targets --all-features
cargo +1.85.0 test --locked --all-targets --all-features
cargo +1.85.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.96.1 check --locked --all-targets --all-features
cargo +1.96.1 test --locked --all-targets --all-features
cargo +1.97.1 check --locked --all-targets --all-features
cargo +1.97.1 test --locked --all-targets --all-features
cargo +1.97.1 clippy --locked --all-targets --all-features -- -D warnings
GUIDE_FORMAT_REQUIRE_CONFORMANCE=all cargo test --workspace --all-targets --all-features --locked -- --nocapture
cargo run --release --locked -- verify --github-actions-check --deny-ignored
```

Run the exact package commands from a clean committed tree when package,
manifest, maintained packaged documentation, or release-sensitive inputs
change:

```sh
cargo package --locked
cargo publish --dry-run --locked
```

Run the standard-library policy suites when their surfaces are affected:

```sh
just test-production-readiness-selector
just test-release-identity
just test-github-protections
just test-quality-gates
just test-contributor-templates
```

For maintained Markdown or GitHub workflow changes, install the exact pinned
tools listed above and run the same commands as CI:

```sh
actionlint .github/workflows/*.yml .github/examples/*.yml
zizmor --pedantic --no-ignores .github/workflows/ .github/examples/readme-verify.yml
rumdl check --disable MD010,MD013,MD038 README.md
rumdl check --disable MD013 CONTRIBUTING.md .github/pull_request_template.md
rumdl check SECURITY.md docs/security-response-runbook.md
rumdl check --disable MD010,MD013,MD018,MD031,MD038 docs/v0.2-contract.md docs/release-policy.md docs/maintainer-continuity.md docs/repository-protections.md docs/history/README.md
lychee --no-progress README.md CONTRIBUTING.md SECURITY.md .github/pull_request_template.md .github/ISSUE_TEMPLATE/*.yml .github/examples/readme-verify.yml docs/v0.2-contract.md docs/release-policy.md docs/security-response-runbook.md docs/maintainer-continuity.md docs/repository-protections.md docs/history/README.md
```

One workstation cannot establish the supported host matrix. The pull request
must pass the complete locked debug and release suites on Linux, macOS, and
Windows, the exact MSRV/current stable lanes, coverage, the fixed mutation
sentinels, package identity, license generation, performance baseline,
workflow lint, and navigation-guide verification. Rust beta is informational.

## Dependencies, licenses, and release-sensitive files

Dependency changes must be intentional, compatible with Rust 1.85, and
reviewed in `Cargo.lock`. Explain every direct dependency or feature change,
inspect the activated graph, run the MSRV/current-stable matrix, scan the
locked graph with pinned `cargo-audit 0.22.1`, and regenerate attribution with:

```sh
cargo about generate about.hbs --output-file THIRD_PARTY_LICENSES.md
git diff --exit-code -- THIRD_PARTY_LICENSES.md
```

Commit the lockfile and attribution change together. Never treat a Dependabot
proposal, successful unlocked build, absent library target, or inactive
optional feature as sufficient review.

Treat `Cargo.toml`, `Cargo.lock`, `release/`, `docs/release-policy.md`,
`SECURITY.md`, `CHANGELOG.md`, `.github/workflows/`, package allowlists,
license/notice files, checksums, tags, and provenance inputs as
release-sensitive. Review their exact diff and run their owning gates.
Issue #63 remains the authority for the release workflow and Trusted Publisher;
until its external criterion is complete, do not publish, create a release
tag, or introduce a long-lived registry token.

## Security and sensitive data

Send suspected vulnerabilities only through the
[private report route](SECURITY.md#report-a-vulnerability-privately). Do not
put exploit details, secrets, personal data, private repository contents, or
embargoed findings in an issue, ordinary pull request, commit, fixture,
workflow log, benchmark, or audit.

Use synthetic, control-safe paths and data. Tests and diagnostics must not
echo raw guide lines, resolved external link targets, tokens, credential
names, or path-sensitive fixtures. A security fix follows the public
[response runbook](docs/security-response-runbook.md) and ordinary
fail-closed release gates.

## Pull requests and review

Open a draft pull request early enough to expose hosted checks, but keep its
scope to the selected issue. Complete every template section with concrete
before/after behavior, red-before-fix evidence, focused/full commands and
platforms, documentation/compatibility/security impact, dependency/license
impact, and issue-graph links.

Update `README.md` with user-facing behavior, `docs/v0.2-contract.md` with
normative behavior, `CHANGELOG.md` with prepared-release impact, and the
navigation guide with new or removed files. Record intentional divergence in
the README with a date and rationale. Historical design text is not normative.

The personal-repository ruleset requires `Required CI`, `Verify Navigation
Guide`, current-main testing, immutable history, and resolved review
conversations. GitHub does not allow the author to self-approve, and the
approved sole-maintainer model has zero required approving reviews. This does
not make review comments optional: answer actionable feedback with code or
evidence and resolve every conversation before merge.

## Maintainer triage

The maintainer:

1. routes vulnerability details to the private advisory and removes sensitive
   public material when possible;
2. applies the narrowest component/domain/risk/priority labels supported by
   evidence, using `needs-decision` when a contract choice is missing;
3. confirms one issue, its native blockers/sub-issues, and exactly one closing
   directive before review;
4. rejects unrelated cleanup and creates a separate issue or dependency edge;
5. checks red-before-fix evidence, deterministic placement, all supported
   platforms, documentation alignment, compatibility, security, dependencies,
   licensing, package, and release-sensitive changes;
6. resolves or explicitly dispositions every review conversation;
7. merges only after required checks pass against current `main`; and
8. verifies the closed issue, default-branch artifacts, and any hosted setting
   after merge.

If authority, intent, platform behavior, compatibility, or security scope is
uncertain, stop and request a public decision. Do not infer an organization,
second maintainer, emergency bypass, or release exception that does not exist.
