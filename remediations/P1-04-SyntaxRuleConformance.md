# P1-04: Syntax Rule Conformance

## Problem

`check` currently accepts some inputs that are documented as invalid, including missing trailing slash for directories and un-delimited comment-like text.

## Current Evidence

- `- src` can pass syntax checks in contexts where it is intended to denote a directory.
- `- src/ source code` can pass despite comment delimiter rules requiring `#`.
- Some syntax error variants exist but are not actively used (`DirectoryMissingSlash`, `InvalidCommentFormat`).

## Desired Behavior

- Syntax enforcement should be coherent and explicit.
- If a rule exists in active docs, parser/validator should enforce it.
- If a rule is intentionally relaxed, docs should be updated to match.

## Proposed Remediation

1. Decide authoritative syntax contract (README + implementation unless incoherent).
2. Implement one consistent strategy:
  - strict directory slash/comment delimiter enforcement, or
  - explicit relaxed grammar with matching docs and tests.
3. Remove dead syntax variants or make them reachable with tests.

## File Targets

- `src/parser.rs`
- `src/validator.rs`
- `src/errors.rs`
- `README.md`
- `Specification.md` (only if explicitly chosen to keep synced)
- `tests/cli_tests.rs`

## Acceptance Criteria

- Behavior and docs agree for slash/comment syntax.
- Tests cover both valid and invalid forms for chosen policy.
- No unreachable syntax variants remain without justification.

## Suggested Tests

- `- src` behavior test aligned with chosen policy.
- `- src/ source code` behavior test aligned with chosen policy.
- Tests asserting expected error variant when strict policy is chosen.

