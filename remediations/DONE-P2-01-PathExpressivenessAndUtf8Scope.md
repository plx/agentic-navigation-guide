# P2-01: Path Expressiveness and UTF-8 Scope

## Problem

The current validator uses a character whitelist that can reject real filesystem-valid names. This creates false negatives and makes some practical filenames inexpressible.

## Policy Decision

- Non-UTF-8 filenames are explicitly out of scope for this project.
- UTF-8 representable names should be accepted unless they violate structural/safety rules.

## Desired Behavior

- Remove arbitrary character whitelist validation.
- Validate path structure, not subjective character classes.
- Keep guide syntax UTF-8 markdown based.
- Emit explicit diagnostic for non-UTF-8 filesystem names encountered during dump/verify workflows.

## Proposed Remediation

1. Replace per-character allowlist in validator with structural checks:
  - non-empty path
  - not absolute
  - no `.` / `..` components
  - no empty components or duplicated separators
2. Define and document UTF-8 scope:
  - non-UTF-8 names are unsupported
  - tool reports this clearly when encountered
3. Ensure wildcard expansion output flows through same structural checks.

## File Targets

- `src/validator.rs`
- `src/verifier.rs`
- `src/dumper.rs`
- `src/errors.rs`
- `README.md`
- `tests/cli_tests.rs`

## Acceptance Criteria

- UTF-8 filenames with punctuation/symbols no longer fail due to allowlist.
- Structural safety checks still block traversal/absolute cases.
- Encountering non-UTF-8 names yields explicit unsupported-scope error.

## Suggested Tests

- Positive test with unusual but UTF-8-valid name.
- Negative tests for absolute and traversal-containing names.
- Targeted unit test for structural checker.

