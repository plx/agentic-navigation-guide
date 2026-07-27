# Issue #47 stable CLI output evidence

Date: 2026-07-27

Issue: [#47](https://github.com/plx/agentic-navigation-guide/issues/47)

## Result

The CLI now uses one fallible process-output layer for primary data,
informational messages, guide diagnostics, and final error reporting.

- A closed stdout consumer is normal Unix pipeline termination. `BrokenPipe`
  returns success after otherwise successful command work and never reaches a
  Rust `print!` panic or exit 101.
- Quiet mode suppresses ordinary success and progress messages, including the
  former unconditional `init` confirmation. It retains primary `dump` bytes
  and never suppresses required failures.
- Single and recursive guide failures use one source-free structured
  diagnostic. GitHub Actions renders its parseable field as
  `safe-logical-guide-path:line: typed reason` on stderr, with human
  decoration on separate lines.
- Recursive diagnostics retain the path relative to the selected search root,
  rather than collapsing a nested guide to its basename.

All modes retain the existing #35 confidentiality boundary: no raw guide
source line or resolved external target is accepted into the diagnostic
representation. The implementation assumes the documented stable-filesystem
boundary and does not claim sandbox or hostile concurrent-replacement safety.

## Red-before-fix evidence

Tests-only commit `240ebf8` was based directly on `main` at
`edcdd5584d2d24cc513d519034f318874b78c742`.

```text
cargo test --locked --test issue_47_output_contract -- --nocapture
exit 101
```

All four fixed regressions failed on the audited base:

- closing the read end of a sufficiently large bounded `dump` pipe caused the
  standard-library print path to panic on `Broken pipe` and return status 101;
- `init --quiet` created its output but printed
  `Navigation guide created at: ...`;
- a recursive GitHub failure printed `GUIDE Ω.md:` separately from
  `line 2: ...`, omitting `module space Ω/GUIDE Ω.md:2:`; and
- the command/log/execution matrix first failed on the same quiet `init`
  chatter.

The tests use fixed temporary trees and finite loops. No fuzzing, randomized
generation, mutation testing, sanitizer campaign, or generated hostile input
was used.

## Acceptance mapping

| ID | Status | Criterion | Evidence |
| --- | --- | --- | --- |
| A47-001 | Verified | Closed stdout never emits a Rust panic or exit 101. | The Unix regression closes the pipe reader before a fixed 4,096-entry dump is delivered and requires status 0 with no panic or broken-pipe diagnostic. |
| A47-002 | Verified | Quiet success emits no ordinary chatter while failures remain visible. | The `init` regression requires an empty stdout/stderr after creation, also proves quiet `dump` retains primary data, and the complete failure matrix requires nonempty stderr in every log/execution mode. |
| A47-003 | Verified | Recursive and single GitHub diagnostics share the documented path-and-line contract. | `GuideDiagnostic` is used by `check`, single `verify`, and recursive `verify`; the recursive integration case requires `module space Ω/GUIDE Ω.md:2:` and a typed missing-file reason. Existing single-guide GitHub tests remain green. |
| A47-004 | Verified | The output-mode matrix is deterministic and automated. | One fixed matrix covers five command paths, three log modes, four execution modes, and both success and failure, including paths with spaces and Unicode. |
| A47-005 | Implemented | README and help match streams, statuses, and annotation format. | The normative process-output section, README output and GitHub sections, changelog, and global/GitHub flag help state the realized behavior. |
| A47-006 | Verified | Diagnostics preserve the approved disclosure boundary. | The shared representation stores only kind, optional line, and the existing source-free reason. The complete #49 sentinel/confidentiality and #51 target-redaction suites pass unchanged. |

## Deterministic validation

`tests/issue_47_output_contract.rs` contains four ordinary integration tests.
Together their fixed cases cover:

- early Unix pipe closure after bounded generation;
- quiet file creation and quiet primary stdout;
- nested recursive paths with spaces, `ü`, and `Ω`;
- `dump`, `init`, `check`, single `verify`, and recursive `verify`;
- quiet, default, and verbose logging;
- default, post-tool-use, pre-commit, and GitHub Actions execution; and
- success output, required failure diagnostics, and exact mode-dependent
  failure statuses.

The complete debug and release suites each passed 364 tests with the same
three intentional ignores. The focused issue #47 suite passed in both
profiles and on Rust 1.85, the declared minimum supported Rust version. Strict
Clippy, formatting, diff hygiene, package construction, and the exact
installed-package boundary also passed. Cargo packaged exactly 33 reviewed
files.

The repository guide syntax check passed. Its filesystem verification passed
after this evidence file was added to the indexed guide.
