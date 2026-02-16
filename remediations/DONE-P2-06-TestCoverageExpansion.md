# P2-06: Test Coverage Expansion for Critical Edge Cases

## Problem

The suite is broad but misses several high-risk edge cases discovered during due-diligence review.

## Gaps to Cover

1. Multiple guide blocks after a first valid block.
2. Root escape via traversal components.
3. Root escape via symlinked directories.
4. Silent failure regression checks (non-empty stderr on non-zero exit).
5. False-positive ignore attribute parsing.
6. Placeholder whitespace-only comment behavior.
7. Escaped comment delimiter (`\#`) in paths.

## Proposed Remediation

1. Add focused CLI integration tests in `tests/cli_tests.rs` (or split into targeted test files).
2. Add parser/verifier unit tests for tricky parsing and root containment logic.
3. Keep tests deterministic with temp directories and explicit setup.

## File Targets

- `tests/cli_tests.rs`
- `src/parser.rs` tests module
- `src/verifier.rs` tests module
- Optional split: `tests/security_tests.rs`, `tests/parser_edge_tests.rs`

## Acceptance Criteria

- All listed gaps have at least one dedicated regression test.
- Tests fail against current vulnerable behavior and pass after remediation.

## Suggested Structure

- Keep each test case name explicit about the invariant it protects.
- Prefer one invariant per test to simplify future debugging.

