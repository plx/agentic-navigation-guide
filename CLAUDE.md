# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Navigation Guide

@AGENTIC_NAVIGATION_GUIDE.md

## Documentation Alignment

- `docs/v0.2-contract.md` is the normative v0.2 guide-language,
  filesystem-representation, stable-filesystem trust-boundary, and
  supported-product target.
- The installed CLI is the sole supported v0.2 product. The current source
  package is binary-only; do not add a public, hidden, feature-gated, unstable,
  or test-only Rust library facade.
- Treat immutable published `0.1.4` as the historical library baseline,
  distinct from current source. Preserve the frozen export-ledger IDs,
  signatures, and implementation owners in `tests/fixtures/v0_2_api.rs`.
- Treat repositories and guide text as untrusted, but do not describe the
  verifier as a sandbox or as hostile-concurrent-mutation safe.
- Implementation plus tests define realized unreleased source behavior while
  explicitly owned v0.2 conformance rows remain pending.
- `README.md` is the concise released-behavior entry point.
- `Specification.md` is non-normative original intent/history.
- If user-facing behavior changes, update `README.md` in the same change.
- If divergence is intentional, record it in `README.md` under "Known Intentional Divergences" with date and rationale.

## Commands

### Build and Check
```bash
cargo build          # Build the project
cargo check          # Check for compilation errors without building
cargo build --release # Build optimized release version
```

### Code Quality
```bash
cargo fmt            # Format code using rustfmt
cargo clippy         # Run Clippy linter for code quality checks
cargo clippy -- -D warnings  # Treat warnings as errors
```

### Testing
```bash
cargo test           # Run all tests
cargo test -- --nocapture  # Run tests showing println! output
cargo test <test_name>     # Run specific test by name
```

### Production-Readiness Remediation

```bash
# Print the next actionable production-readiness issue from live GitHub state
just get-next-production-readiness-issue

# Emit the selection as JSON or temporarily skip one or more issues
just get-next-production-readiness-issue --json
just get-next-production-readiness-issue --exclude 34 --exclude 35

# Run the selector's offline regression suite
just test-production-readiness-selector
```

### Running the CLI Tool
```bash
# Dump directory structure
cargo run -- dump --depth 2 --exclude target --exclude .git

# Check navigation guide syntax
cargo run -- check
cargo run -- check --guide path/to/guide.md
cargo run -- check --deny-ignored

# Verify guide against filesystem
cargo run -- verify
cargo run -- verify --guide path/to/guide.md --root /path/to/root

# Verify in GitHub Actions mode (concise output, file:line format)
cargo run -- verify --github-actions-check --deny-ignored

# Recursively verify all navigation guides (for monorepos)
cargo run -- verify --recursive
cargo run -- verify --recursive --guide-name GUIDE.md --exclude target --exclude node_modules
cargo run -- verify --recursive --deny-ignored

# Only for an intentionally optional recursive search; required CI must omit it
cargo run -- verify --recursive --allow-empty

# Initialize new guide file
cargo run -- init --output AGENTIC_NAVIGATION_GUIDE.md
```

## Architecture

This is a CLI tool for verifying hand-written navigation guides against filesystem structure. The architecture follows these principles:

### Core Components

1. **Parser** (`src/parser.rs`): Extracts navigation guide blocks and always
   validates the exact envelope plus global marker-candidate scan. Active
   bodies are parsed into a hierarchy. A valid `ignore=true` body is opaque,
   may be empty, and produces no parsed items.

2. **Validator** (`src/validator.rs`): Performs syntax validation on parsed guides, checking for proper formatting, consistent indentation, and valid path formats.

3. **Guide Input** (`src/guide_input.rs`): Binary-private shared guide-path
   validation used by CLI commands and internal recursive verification.
   Validates path authority, anchors implicit reads, rejects final
   links/reparse entries and unsafe descendants, and opens regular guide
   handles without following the final entry.

4. **Verifier** (`src/verifier.rs`): Validates guides against actual filesystem state, checking that all referenced paths exist and are of the correct type (file/directory).

5. **Dumper** (`src/dumper.rs`): Generates navigation guides from directory structures, with support for depth limiting and glob exclusion patterns.

6. **Recursive** (`src/recursive.rs`): Provides recursive guide discovery and
   batch verification for monorepos with nested navigation guides. It performs
   bounded directory enumeration and verifies each guide relative to its parent
   directory.

### Data Flow

1. **Selection**: Resolve explicit versus implicit guide provenance and the
   effective trust anchor.
2. **Safe opening**: Validate the configured spelling, classify without
   following links, and read only through a validated regular-file handle.
3. **Parsing**: Extract guide content and parse into `NavigationGuide`
   structure.
4. **Validation**: Check syntax rules (paths ending with `/` for directories,
   proper indentation).
5. **Verification**: Compare against filesystem (if using `verify`).
6. **Guide-input failures**: Report bounded logical locations and reasons
   without complete source lines or rejected guide-link targets.

Every guide-reading route must use `src/guide_input.rs`; do not add direct
`read_to_string` calls for guide paths or reopen a discovered path without the
shared validation. CLI and recursive formatters must not pass guide content to
`ErrorFormatter::format_with_context`.

### Key Types

- `FilesystemItem`: Enum representing File, Directory, or Placeholder
- `NavigationGuideLine`: Parsed line with indent level and filesystem item
- `NavigationGuide`: Complete guide with items, optional prologue/epilogue, and ignore flag
  - The `ignore` field represents a distinct internal ignored outcome, not checked or verified success
  - Set using `<agentic-navigation-guide ignore=true>` in the opening tag
  - The exact envelope is still validated; its body skips list, validator, and filesystem checks
- `CommandOutcome` (binary-private): Distinguishes completed work from one or more ignored guides
- `ExecutionMode`: Default, PostToolUse (exit code 2), PreCommitHook, or GitHubActions

`check` and `verify` allow ignored guides by default and report them in
non-quiet modes. Required hooks and CI must pass `--deny-ignored` explicitly;
execution mode and quiet mode do not change the policy. Recursive totals count
ignored guides separately from passed, failed, and absent outcomes.

### Error Handling

- `SyntaxError`: Format violations in the guide
- `SemanticError`: Filesystem mismatches
- All errors include line numbers for easy debugging

### Environment Variables

These environment variables are used to configure the `agentic-navigation-guide` tool's behavior:

- `AGENTIC_NAVIGATION_GUIDE_LOG_MODE`: Set to "quiet", "verbose", or "default"
- `AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE`: Set to "post-tool-use", "pre-commit-hook", "github-actions", or "default"
- `AGENTIC_NAVIGATION_GUIDE_PATH`: Default path to guide file
- `AGENTIC_NAVIGATION_GUIDE_ROOT`: Default root directory for operations
- `AGENTIC_NAVIGATION_GUIDE_NAME`: Default guide filename for recursive mode (e.g., "GUIDE.md")
