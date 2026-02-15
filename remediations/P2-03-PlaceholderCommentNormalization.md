# P2-03: Placeholder Comment Normalization

## Problem

Placeholder semantics depend on whether a comment exists, but whitespace-only comments can currently be treated as present and bypass stricter placeholder checks.

## Desired Behavior

- A placeholder comment should be considered present only if it contains non-whitespace content.
- `- ... #` and `- ... #   ` should be treated as no-comment placeholders.

## Proposed Remediation

1. Normalize parsed comments:
  - trim comment
  - convert empty result to `None`
2. Keep semantic rule:
  - comment present => placeholder can represent future or omitted items
  - no comment => must match at least one unlisted existing item

## File Targets

- `src/parser.rs`
- `src/verifier.rs`
- `tests/cli_tests.rs`
- `src/verifier.rs` unit tests

## Acceptance Criteria

- Placeholder with whitespace-only comment follows no-comment rules.
- Placeholder with meaningful comment keeps existing relaxed behavior.

## Suggested Tests

- Verify failure for `- ... #   ` when no unmentioned items exist.
- Verify success for `- ... # future files` when no unmentioned items exist.

