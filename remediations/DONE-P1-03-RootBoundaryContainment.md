# P1-03: Root Boundary Containment

## Problem

Guide verification can currently escape the declared `--root` boundary via traversal paths (for example `../x`) and via symlinked directories that resolve outside root.

## Current Evidence

- `src/verifier.rs` joins parent and item path directly (`parent_path.join(item.path())`).
- No canonical boundary check ensures resolved path remains under root.
- Directory symlink traversal can lead verification to external filesystem locations.

## Desired Behavior

- Verification must be root-contained by default.
- Any path resolution that escapes root is rejected.
- Symlink traversal outside root is rejected unless an explicit future opt-out mode is introduced.

## Proposed Remediation

1. Canonicalize root once during verifier setup.
2. For each item:
  - resolve candidate path
  - canonicalize existing path (or existing ancestor where needed)
  - enforce `resolved.starts_with(canonical_root)`
3. Add a dedicated semantic error for root escape attempts.
4. Ensure checks apply to nested children and recursive validation paths.

## File Targets

- `src/verifier.rs`
- `src/errors.rs`
- `src/cli/verify.rs`
- `src/recursive.rs`
- `tests/cli_tests.rs`

## Acceptance Criteria

- `../outside.txt` entries fail verification with explicit root-boundary error.
- Symlink directory inside root pointing outside root cannot be used to satisfy nested guide entries.
- Existing in-root paths continue to pass.

## Suggested Tests

- Integration test for direct traversal escape (`../...`).
- Integration test for symlinked directory to external location.
- Integration test confirming normal symlink-to-in-root behavior (if supported).

