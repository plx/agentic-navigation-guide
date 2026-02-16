# P2-04: Ignore Attribute Parsing Robustness

## Problem

Ignore attribute detection currently uses substring matching and can false-match unrelated attribute text.

## Desired Behavior

- `ignore=true` and `ignore="true"` should be recognized only as explicit `ignore` attribute assignments.
- Unrelated attributes (for example `notignore=true`) must not trigger ignore mode.

## Proposed Remediation

1. Replace string `contains(...)` logic with explicit attribute parsing.
2. Support both quoted and unquoted boolean true.
3. Optionally reject malformed opening tag attributes with a syntax error.

## File Targets

- `src/parser.rs`
- `src/errors.rs` (if malformed-attribute errors are added)
- `tests/cli_tests.rs`
- `src/parser.rs` unit tests

## Acceptance Criteria

- `ignore=true` and `ignore="true"` still work.
- `notignore=true` does not trigger ignore behavior.
- Mixed-attribute tags behave deterministically.

## Suggested Tests

- Parser tests for positive and negative attribute cases.
- CLI test confirming non-ignore behavior for `notignore=true`.

