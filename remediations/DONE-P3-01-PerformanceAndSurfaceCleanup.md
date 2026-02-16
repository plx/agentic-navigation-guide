# P3-01: Performance and Surface Cleanup

## Problem

There are a few lower-priority implementation inefficiencies and maintenance overhead items:

- Avoidable repeated recursion in nesting validation.
- Potentially expensive hierarchy construction patterns.
- Dead or partially-realized API/error surface.
- Unused dependencies.

## Desired Behavior

- Keep runtime predictable on larger guides.
- Reduce dead code and ambiguous API surface.
- Keep dependency set minimal.

## Proposed Remediation

1. Simplify `validate_nesting` recursion to avoid repeated subtree checks.
2. Consider stack-based hierarchy building in parser to reduce repeated backward scans.
3. Audit and either implement or remove currently-unused error variants and paths.
4. Remove unused dependencies (`anyhow`, `insta`) unless a near-term plan exists.

## File Targets

- `src/validator.rs`
- `src/parser.rs`
- `src/errors.rs`
- `Cargo.toml`
- `Cargo.lock`

## Acceptance Criteria

- No dead error variants without justification.
- No unused dependencies in manifest.
- Equivalent behavior preserved under existing tests.
- Optional: measurable improvement on synthetic large-guide benchmark.

## Suggested Validation

- Run `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt -- --check`.
- Add one benchmark-like stress test if performance changes are significant.

