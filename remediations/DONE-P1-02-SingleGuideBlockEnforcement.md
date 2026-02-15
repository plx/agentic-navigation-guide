# P1-02: Single Guide Block Enforcement

## Problem

The parser can accept files with multiple `<agentic-navigation-guide>` blocks when a second block appears after the first closing tag. This violates intended format constraints.

## Current Evidence

- `src/parser.rs` stops scanning after the first closing marker.
- A second opening marker later in file is not detected.

## Desired Behavior

- Exactly one opening marker and one closing marker per document.
- Any additional opening or closing marker should be a syntax error with line context.

## Proposed Remediation

1. Rework block extraction in `Parser::extract_guide_block` to scan full document.
2. Track all marker occurrences and validate:
  - one opening marker
  - one closing marker
  - opening before closing
  - no extra markers
3. Extend `SyntaxError` variants if needed for clearer diagnostics.

## File Targets

- `src/parser.rs`
- `src/errors.rs`
- `tests/cli_tests.rs`

## Acceptance Criteria

- A document with two guide blocks fails `check`.
- A document with stray extra closing marker fails `check`.
- Error message includes line number of conflicting marker.

## Suggested Tests

- New parser unit tests for:
  - second block after first close
  - second opening before first close
  - extra closing marker
- CLI test: `check --guide <file-with-2-blocks>` fails with marker-related error text.

