# P1-01: Silent Failures

## Problem

Several error paths return a non-zero exit code without printing any actionable error message. This makes failures hard to diagnose in local use and in automated environments.

## Current Evidence

- `src/main.rs` drops the actual error value in `Err(_e)` and assumes command handlers already printed details.
- Some command paths return `Err(...)` without printing:
  - `src/cli/init.rs` when output file already exists
  - `src/dumper.rs` glob parsing errors propagated through callers

## Desired Behavior

- Any non-zero exit path must emit a deterministic, human-readable error message to stderr.
- Error formatting should be centralized and consistent across subcommands and modes.

## Proposed Remediation

1. Centralize fallback error printing in `src/main.rs`.
2. Ensure command handlers return typed errors, not mixed print-and-return behavior.
3. Keep mode-specific formatting only where required (`post-tool-use`, `github-actions`) and use a shared fallback formatter otherwise.
4. Add regression tests for common silent-failure scenarios.

## File Targets

- `src/main.rs`
- `src/cli/init.rs`
- `src/cli/dump.rs`
- `src/cli/check.rs`
- `src/cli/verify.rs`
- `src/errors.rs`
- `tests/cli_tests.rs`

## Acceptance Criteria

- Existing-output `init` failure prints a clear stderr message and exits non-zero.
- Invalid glob pattern for `dump`/`verify --recursive --exclude` prints clear stderr message and exits non-zero.
- No tested error path exits non-zero with empty stderr.

## Suggested Tests

- `init --output <existing file>` asserts non-empty stderr containing reason.
- `dump --exclude "["` asserts non-empty stderr containing invalid glob.
- `verify --recursive --exclude "["` asserts non-empty stderr containing invalid glob.

