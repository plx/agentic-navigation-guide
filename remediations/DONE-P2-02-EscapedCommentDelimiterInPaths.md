# P2-02: Escaped Comment Delimiter in Paths

## Problem

Guide parsing currently uses `#` as an unescaped comment splitter, which makes filenames containing `#` difficult or impossible to express faithfully.

## Desired Behavior

- `#` should remain the comment delimiter.
- Paths should be able to include literal `#` via escaping (`\#`).
- Parser should unescape path content consistently with existing wildcard escape handling.

## Proposed Remediation

1. Replace regex-only path/comment split with a scanner that respects escapes.
2. Treat the first unescaped `#` as comment delimiter.
3. Unescape path escape sequences after splitting.
4. Preserve existing comment semantics for normal paths.

## File Targets

- `src/parser.rs`
- `src/validator.rs`
- `README.md`
- `tests/cli_tests.rs`

## Acceptance Criteria

- `- file\#name.txt` is parsed as path `file#name.txt` without comment.
- `- file\#name.txt # comment` parses path plus comment correctly.
- Existing non-escaped comment syntax continues to work.

## Suggested Tests

- Parser unit tests for escaped and unescaped `#`.
- CLI `check`/`verify` tests using escaped `#` in paths.

