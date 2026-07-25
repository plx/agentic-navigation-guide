# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Navigation Guide

@AGENTIC_NAVIGATION_GUIDE.md

## Documentation Alignment

- `docs/v0.2-contract.md` is the normative v0.2 guide-language and
  filesystem-representation target.
- Implementation plus tests define realized `0.1.4` behavior while explicitly
  owned v0.2 conformance rows remain pending.
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

# Verify guide against filesystem
cargo run -- verify
cargo run -- verify --guide path/to/guide.md --root /path/to/root

# Verify in GitHub Actions mode (concise output, file:line format)
cargo run -- verify --github-actions-check

# Recursively verify all navigation guides (for monorepos)
cargo run -- verify --recursive
cargo run -- verify --recursive --guide-name GUIDE.md --exclude target --exclude node_modules

# Initialize new guide file
cargo run -- init --output AGENTIC_NAVIGATION_GUIDE.md
```

## Architecture

This is a CLI tool for verifying hand-written navigation guides against filesystem structure. The architecture follows these principles:

### Core Components

1. **Parser** (`src/parser.rs`): Extracts navigation guide blocks from markdown files and parses the hierarchical structure. Uses regex to parse individual lines and builds a tree structure based on indentation levels.

2. **Validator** (`src/validator.rs`): Performs syntax validation on parsed guides, checking for proper formatting, consistent indentation, and valid path formats.

3. **Verifier** (`src/verifier.rs`): Validates guides against actual filesystem state, checking that all referenced paths exist and are of the correct type (file/directory).

4. **Dumper** (`src/dumper.rs`): Generates navigation guides from directory structures, with support for depth limiting and glob exclusion patterns.

5. **Recursive** (`src/recursive.rs`): Provides recursive guide discovery and batch verification for monorepos with nested navigation guides. Uses WalkDir to find all guide files and verifies each relative to its parent directory.

### Data Flow

1. **Input**: Markdown files containing `<agentic-navigation-guide>` blocks
2. **Parsing**: Extract guide content and parse into `NavigationGuide` structure
3. **Validation**: Check syntax rules (paths ending with `/` for directories, proper indentation)
4. **Verification**: Compare against filesystem (if using `verify` command)
5. **Output**: Error messages to stderr, exit codes based on execution mode

### Key Types

- `FilesystemItem`: Enum representing File, Directory, or Symlink
- `NavigationGuideLine`: Parsed line with indent level and filesystem item
- `NavigationGuide`: Complete guide with items, optional prologue/epilogue, and ignore flag
  - The `ignore` field indicates whether the guide should be skipped during verification
  - Set using `<agentic-navigation-guide ignore=true>` in the opening tag
  - Useful for documentation examples that shouldn't be validated
- `ExecutionMode`: Default, PostToolUse (exit code 2), PreCommitHook, or GitHubActions

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
