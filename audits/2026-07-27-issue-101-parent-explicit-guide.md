# Issue #101 parent-containing explicit-guide evidence

Date: 2026-07-27

Issue: [#101](https://github.com/plx/agentic-navigation-guide/issues/101)

## Result

Explicit guide paths no longer receive external authority merely because a
tail below the configured trust-anchor spelling contains `..`.
`src/guide_input.rs` now classifies paths as proven in-anchor, proven
external, or parent-containing before access. A parent-containing path is
processed in order from the canonical anchor:

- every component removed by `..` is classified without following that
  component and must be a real directory;
- a reduction that remains in-anchor receives the ordinary non-following
  ancestor and canonical-containment checks;
- a spelling that leaves and then lexically returns to the anchor fails
  closed; and
- only a path proven to remain external can use the existing explicit
  external authority.

The configured logical spelling remains the only path in diagnostics.
Resolved targets and guide bytes are not reported. This retains the selected
stable-filesystem guarantee; it does not claim a sandbox or protection from
hostile concurrent replacement.

## Red-before-fix evidence

Tests-only commit `71b5cd3` was based directly on `main` at
`dbc64a5a5b1b44f471f2251c20a3bd625c915f61`.

```text
cargo test --locked --test issue_101_parent_explicit_guide -- --nocapture
exit 101
```

The fixed five-case matrix observed one positive control passing and four
rejection groups failing because the CLI reported success:

- relative and absolute `padding/../linked/guide.md` through `--guide`;
- the equivalent `AGENTIC_NAVIGATION_GUIDE_PATH` configuration;
- an in-anchor link target;
- a link before `..`; and
- root-alias/parent-spelling cases.

No fuzzing, mutation testing, randomized generation, or generated hostile
input was used.

## Acceptance mapping

| ID | Status | Criterion | Evidence |
| --- | --- | --- | --- |
| A101-001 | Implemented | The exact `check` and `verify` reproduction rejects before reading the outside guide. | `issue_101_parent_path_rejects_in_anchor_links_on_every_explicit_surface` exercises both commands and rejects target/sentinel disclosure. |
| A101-002 | Implemented | No missing safe tail becomes external authority by default. | `CandidateClass` separates `ProvenInAnchor`, `ProvenExternal`, and `ParentContaining`; the former `safe_tail` fallback is absent. |
| A101-003 | Verified | Parent-containing symlink and junction/reparse variants reject on each claimed platform. | The same integration matrix uses Unix symlinks and a real Windows `mklink /J` junction. The hosted [`Build (windows-latest)` job](https://github.com/plx/agentic-navigation-guide/actions/runs/30260078628/job/89957645879) passed. |
| A101-004 | Verified | Real-directory reduction and genuine explicit-external paths remain supported. | Positive controls cover `padding/../real/guide.md`, relative external regular files, and the existing stable external link-ancestor authority. |
| A101-005 | Verified | Root aliases and unresolved root spellings retain parent-order guarantees. | The focused matrix covers a caller-selected root alias and `child-alias/..`; #51's exact containment suite remains green. |
| A101-006 | Verified | Diagnostics retain configured spelling and redact resolved targets/content. | Every negative subprocess and the direct shared-opener regression assert the logical components and reject the target path and sentinel. |
| A101-007 | Implemented | Every explicit configuration surface uses the corrected policy. | `--guide`, `AGENTIC_NAVIGATION_GUIDE_PATH`, `check`, `verify`, and a direct binary-internal `GuideAnchor::read` regression share the same classifier. No public/library Rust facade exists in v0.2. |
| A101-008 | Implemented | Documentation states the distinction without a sandbox claim. | `docs/v0.2-contract.md`, `README.md`, and `CHANGELOG.md` describe ordered reduction, retained external authority, and the stable-filesystem limit. |

## Bounded validation

`tests/issue_101_parent_explicit_guide.rs` contains a fixed six-test
cross-platform subprocess matrix. `src/guide_input.rs` adds two direct
shared-opener unit regressions. Together they cover:

- flag and environment provenance;
- `check` and nonrecursive `verify`;
- relative and absolute spellings;
- in-root and out-of-root link targets;
- a link before the parent component;
- real-directory and true-external positive controls;
- root aliases and unresolved-root spellings; and
- control-safe, bounded target/content redaction.

The existing #49 trust fixture IDs, outcomes, and owner remain unchanged.
The existing #51 containment tests remain separately owned and unchanged.

## Validation results

All local validation used fixed, reviewable cases:

- the focused issue #101 integration matrix passed all 6 tests;
- the two direct shared-opener unit regressions passed;
- the focused issue #101 matrix passed with Rust 1.85, the declared MSRV;
- the complete debug and release suites each passed 360 tests with the same
  3 intentional ignores;
- strict Clippy, all-target/all-feature checking, formatting, diff hygiene,
  guide `check`, and guide `verify --deny-ignored` passed;
- #49's exact trust evidence and all 12 #51 containment tests passed;
- all 61 selector tests and all 14 release-identity mutation tests passed;
  the release-identity checker also passed;
- `cargo package --locked` packaged and verified successfully;
- the exact-package install/smoke boundary test passed; and
- `cargo package --locked --list` remained at exactly 33 files.

The real Windows junction case passed in the hosted `Build (windows-latest)`
PR job linked in the acceptance mapping. Capability absence would have failed
the test rather than skipping it.
