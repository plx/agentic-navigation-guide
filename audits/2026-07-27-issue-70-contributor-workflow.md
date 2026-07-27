# Issue #70 contributor-workflow evidence

Date: 2026-07-27

Repository: `plx/agentic-navigation-guide`

## Scope decisions

This issue establishes a contributor workflow for the repository that exists,
not for a hypothetical organization:

- the repository remains personal, with `plx` as its only maintainer and no
  GitHub organization, team, backup administrator, or independent reviewer;
- the static site remains outside this CLI audit;
- Issue #63 still owns the externally blocked release workflow and crates.io
  Trusted Publisher, so contributor guidance does not claim publication
  authority;
- no `CODEOWNERS` file or cosmetic organization control is introduced; and
- no fuzz target, corpus, generated-property runner, random fixture, or new
  mutation campaign is added or run.

No fuzz target exists. The guide keeps issue #56 deferred and directs ordinary
parser reliability work to the deterministic issue #57 matrices. It documents
the already-reviewed fixed 15-sentinel mutation job without broadening it.

## Deterministic red-before evidence

The issue branch started at exact merged-main commit
`b2e3ec848ba6e2553fdecfa5f7e6f98bcefb482e`. Before any contributor artifact
or CI implementation existed, the new fixed-file contract was run:

```sh
cargo test --locked --test issue_70_contributor_workflow -- --nocapture
```

It exited 101 with all three tests failing for the intended reason:

- `CONTRIBUTING.md` did not exist;
- `.github/ISSUE_TEMPLATE/01-bug.yml` did not exist; and
- `audits/2026-07-27-issue-70-contributor-workflow.md` did not exist.

The test compiled successfully and did not fail because of setup, networking,
random input, or unrelated runtime behavior.

## Contributor artifacts and fail-closed contract

The delivered surface is:

- root `CONTRIBUTING.md`, which GitHub can surface to issue and pull request
  authors;
- ordered structured issue forms for public bugs and contract proposals;
- a chooser that disables contributor blank issues and exposes the real
  private vulnerability-report route;
- one pull request template with exact evidence and impact headings;
- `scripts/check_contributor_templates.py`, a Python-standard-library checker
  for the repository's deliberately constrained YAML subset, chooser, pull
  request headings, closing placeholder, and contributor-guide headings;
- negative checker regressions for unquoted values, aliases/anchors, odd
  indentation, duplicate IDs, missing validation, wrong private route,
  numeric template closers, missing headings, and missing artifacts; and
- an all-platform Rust contract that binds setup, issue-graph, red-before,
  hermetic-test, validation, documentation, and maintainer instructions to CI.

The checker accepts only two-space YAML indentation, JSON-style quoted string
scalars, explicit booleans, unique lowercase kebab-case input IDs, known input
types, and explicit required flags. This smaller grammar is sufficient for the
checked-in forms and rules out YAML features the repository does not need.

The file locations and schema follow GitHub's maintained documentation for
[issue forms](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-issue-forms),
[template chooser configuration](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/configuring-issue-templates-for-your-repository),
[pull request templates](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/creating-a-pull-request-template-for-your-repository),
and
[contributor guidelines](https://docs.github.com/en/enterprise-cloud@latest/communities/setting-up-your-project-for-healthy-contributions/setting-guidelines-for-repository-contributors).

## Clean-checkout command exercise

The exact candidate source passed:

| Surface | Result |
| --- | --- |
| Rust `1.85.0` | Locked all-target/all-feature check, test, and warnings-denied Clippy passed |
| Rust `1.96.1` | Locked all-target/all-feature check and test passed |
| Rust `1.97.1` | Locked all-target/all-feature check, test, and warnings-denied Clippy passed |
| Full conformance suite | `GUIDE_FORMAT_REQUIRE_CONFORMANCE=all` passed 393 tests; the two manual benchmarks and explicit package test were the three documented ignores |
| Explicit packaged-artifact acceptance | The ignored issue #62 exact-package install, smoke, and no-library-consumer test passed separately |
| Issue #70 contracts | Three Rust tests, nine Python negative/unit tests, and the live constrained-template check passed |
| Deterministic parser matrix | The five issue #57 fixed equivalence-matrix tests passed; no generated input was used |
| Related and repository policy | Eleven issue #61/#66/#69 Rust contracts and 101 selector, identity, protection, coverage, fixed-mutation-report, and performance-policy tests passed |
| Package | The exact 35-file allowlist packaged and verified; `cargo publish --dry-run --locked` reached only Cargo's expected pre-upload abort |
| GitHub files | Ruby `Psych.safe_load` accepted all three YAML files; exact `actionlint` and pedantic `zizmor` commands passed with no findings |
| Maintained documentation | All exact `rumdl` commands passed; `lychee` checked 95 links (59 unique) with zero errors and four followed redirects |
| Formatting and navigation | `cargo fmt -- --check`, `just --fmt --check`, `git diff --check`, and release-mode guide verification passed |

These commands targeted the exact candidate source in the issue working tree.
The first package and dry-run exercise used `--allow-dirty` only because the
candidate audit had not yet been committed. The exact non-`--allow-dirty`
package commands and focused contracts are repeated on the clean committed
tree before publication, and that commit/result is appended here.

Hosted CI remains authoritative for Linux, macOS, and Windows. A single local
machine is not represented as three-platform evidence.

## Template validation and preview

The constrained local validator, its nine negative/unit regressions, and a
second parse with Ruby's maintained YAML implementation pass before
publication. GitHub only makes repository issue and pull request templates
available from the default branch, so the rendered chooser, each issue form,
the pull request template, and the community-profile contribution surface must
be checked immediately after merge. No draft test issue containing placeholder
content will be left open.

## Representative-ticket cold read

The contributor guide was read as the only process reference alongside a
representative issue-scoped remediation ticket. The cold-read record is
completed after the final guide text and automated review are available.
Known prerequisites that must remain discoverable without oral context are:

- the sole normative contract and concise README roles;
- exact MSRV/stable/tool pins and locked commands;
- native blocker/sub-issue selection and one-issue scope;
- deterministic red-before evidence without a broken `main`;
- hermetic temporary-root, environment, and platform-capability rules;
- the absence of a fuzz target or corpus;
- private security routing and sensitive-fixture limits;
- dependency/license/package/release-sensitive review; and
- the personal repository's zero-independent-review constraint.

This is an instruction-completeness exercise, not an independent technical
approval. The hosted automated review supplies an additional cold reader; any
missing prerequisite it identifies must be corrected or recorded before merge.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Setup and complete relevant validation are discoverable | Pinned environment table, trusted-checkout setup, focused/full commands, package/policy/doc gates, and hosted platform explanation |
| Defects require red-before-fix evidence | Five-step workflow plus PR prompts require base SHA, exact command, nonzero status, reported reason, focused green, and full green |
| Templates capture every impact | Bug/contract forms and PR template cover compatibility, security, documentation, dependency/license, platform, validation, and issue graph |
| Issue graph needs no hidden process | Selector, one-issue rule, native blocker/sub-issue reading, separate-finding rule, and one closer are explicit |
| Commands and templates stay current in CI | Rust contract, strict standard-library checker, negative tests, rumdl, lychee, workflow lint, and all-platform suite |

Post-merge closure requires successful main CI, GitHub-rendered template
recognition, a public issue note with those URLs/results, and no open draft test
artifact.
