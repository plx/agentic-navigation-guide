# Continuing Mission: Agentic Navigation Guide

## Project Overview

The Agentic Navigation Guide is a Rust CLI tool designed to verify hand-written navigation guides against actual filesystem structures. These navigation guides help AI coding assistants understand project structure by providing human-curated documentation that can be validated programmatically.

### What We're Trying to Do

Build a production-ready CLI tool that:

1. **Parses** navigation guides from markdown files (extracting content between `<agentic-navigation-guide>` tags)
2. **Validates** syntax according to strict rules (proper indentation, directory markers, etc.)  
3. **Verifies** guides against actual filesystem state
4. **Generates** navigation guides from directory structures (dump/init commands)
5. **Integrates** with development workflows (git hooks, CI/CD, Claude Code hooks)

The tool must support multiple execution modes:
- Default mode: Human-friendly output
- Post-tool-use hook mode: Exit code 2 on errors
- Pre-commit hook mode: Standard git hook behavior

### Example Navigation Guide

```markdown
<agentic-navigation-guide>
- src/
  - main.rs # Main entry point
  - lib.rs # Core logic
  - types.rs # Data structures
- Cargo.toml # Project manifest
- README.md
</agentic-navigation-guide>
```

## What's Already Been Done

### ✅ 1. Project Infrastructure
- Created Rust project with complete Cargo.toml configuration
- Set up all dependencies (clap, thiserror, walkdir, globset, regex, etc.)
- Configured rustfmt and clippy for code quality
- Established module structure with clean separation of concerns

### ✅ 2. Core Data Types (`src/types.rs`)
- `FilesystemItem` enum: Represents files, directories, and symlinks with proper hierarchy
- `NavigationGuide` struct: Complete guide with items and metadata  
- `NavigationGuideLine` struct: Individual parsed lines with hierarchy info
- `ExecutionMode` and `LogLevel` enums for CLI behavior
- Comprehensive error types with `thiserror`

### ✅ 3. Parser Module (`src/parser.rs`) - COMPLETE
- Extracts guide blocks from markdown files
- Parses individual lines with regex
- Handles comments and paths correctly
- **Hierarchical structure building now works** - children are properly nested under parent directories
- Full indentation validation
- Comprehensive error detection with line numbers

### ✅ 4. Dumper Module (`src/dumper.rs`) - COMPLETE  
- Directory traversal with WalkDir
- Max depth limiting works correctly
- Tree structure building with proper indentation
- Markdown formatting with XML wrapper tags
- **Exclusion patterns now work correctly** - can exclude directories like `.git` and `target`
- Handles complex glob patterns (e.g., `*.toml`, `.git*`)

### ✅ 5. Validator Module (`src/validator.rs`) - COMPLETE
- Empty guide detection
- **Path character validation** - rejects pipes, double slashes, etc.
- **Indentation consistency validation** - ensures all indents are multiples of base
- **Directory path validation** - ensures directories don't have trailing slashes internally
- **Nesting validation** - ensures children are exactly one level deeper than parents
- Comprehensive error reporting with line numbers

### ✅ 6. Verifier Module (`src/verifier.rs`) - BASIC FUNCTIONALITY
- File/directory existence checking
- Type mismatch detection (file vs directory)  
- Permission error handling
- Basic hierarchical path building
- Comprehensive error reporting

### ✅ 7. CLI Structure (`src/cli/`)
All four subcommands are implemented and functional:
- `dump`: Generates guides from directories (with depth/exclusion support)
- `init`: Creates new guide files with boilerplate
- `check`: Validates syntax only
- `verify`: Validates syntax + filesystem matching

### ✅ 8. Testing
- 9 unit tests passing (covering parser, validator, dumper, verifier)
- All core functionality has basic test coverage
- Tests verify hierarchical parsing, exclusion patterns, and validation rules

## Next Work To Be Done

### 1. Integration Tests (MEDIUM PRIORITY)

Create comprehensive integration tests in `tests/integration/`:

```rust
// tests/integration/cli_tests.rs
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_dump_command() {
    let temp_dir = TempDir::new().unwrap();
    // Create test structure
    // Run dump command
    // Verify output
}

#[test]
fn test_verify_command_success() {
    // Create matching guide and filesystem
    // Run verify command
    // Assert exit code 0
}

#[test] 
fn test_post_tool_use_mode() {
    // Test exit code 2 behavior
}
```

Key test scenarios:
- Valid and invalid guide files
- Filesystem mismatches
- Different execution modes
- Edge cases (empty directories, symlinks, permissions)

### 2. Add Symlink Support (LOW PRIORITY)

Enhance the verifier to handle symlinks:

```rust
// In src/verifier.rs
FilesystemItem::Symlink { path, target, .. } => {
    // Check if symlink exists
    // Read symlink target
    // Verify target matches if specified
    // Handle broken symlinks gracefully
}
```

Also update parser to support inline symlink syntax: `link -> target`

### 3. Create CI/CD Pipeline (LOW PRIORITY)

Create `.github/workflows/ci.yml`:

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - run: cargo test
      - run: cargo clippy -- -D warnings
      - run: cargo fmt -- --check
  
  release:
    needs: test
    if: startsWith(github.ref, 'refs/tags/')
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - run: cargo build --release
      - uses: actions/upload-artifact@v3
```

### 4. Git Hooks Integration (LOW PRIORITY)

Create example hooks in `.hooks/`:

```bash
#!/bin/bash
# .hooks/pre-commit
# Run verify on all tracked .md files with navigation guides
for file in $(git ls-files '*.md'); do
    if grep -q '<agentic-navigation-guide>' "$file"; then
        agentic-navigation-guide verify --guide "$file" || exit 1
    fi
done
```

### 5. Performance Optimizations (FUTURE)

- Parallel filesystem traversal for large directories
- Caching parsed guides
- Streaming parser for very large files

### 6. Documentation (FUTURE)

- Expand README with detailed usage examples
- Add inline documentation for all public APIs
- Create CONTRIBUTING.md with development setup
- Add man page generation

## Known Issues

1. **Verifier**: Symlink target validation not implemented
2. **Performance**: No optimizations for very large directory trees
3. **Documentation**: Needs more examples and API docs

## Development Commands

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run all tests
cargo test

# Run specific test module
cargo test parser
cargo test validator

# Test CLI commands
cargo run -- dump --depth 2 --exclude target --exclude .git
cargo run -- check
cargo run -- verify
cargo run -- init --output NEW_GUIDE.md

# Build release version
cargo build --release

# Install locally
cargo install --path .
```

## Architecture Notes

The codebase follows clean architecture principles:
- `lib.rs`: Public API surface
- `types.rs`: Shared data structures  
- `errors.rs`: All error types
- Each module has a single responsibility
- CLI module separate from core logic

When implementing fixes, maintain this separation. The CLI should only handle argument parsing and output formatting, not business logic.

## Critical Implementation Details

### Parser Hierarchical Building
The parser now correctly builds a tree structure from flat indented lists. Key algorithm:
1. Track parent indices for each item based on indentation
2. Process items in reverse order to ensure children are complete
3. Insert children at the beginning to maintain order

### Dumper Exclusion Patterns  
Uses WalkDir's `filter_entry` to exclude directories before traversal. Checks both full paths and individual components against glob patterns.

### Validator Rules
- Paths can contain: alphanumeric, `-_./` and space `()[]{}@+~,`
- No double slashes, pipes, or other special characters
- Indentation must be consistent multiples
- Children must be exactly one level deeper than parents

## Testing Strategy

1. **Unit tests**: Each module's public functions
2. **Integration tests**: CLI commands end-to-end
3. **Property tests**: Could add for parser with arbitrary inputs
4. **Benchmark tests**: For large directory performance

## Environment Variables

These environment variables configure the tool's behavior:
- `AGENTIC_NAVIGATION_GUIDE_LOG_MODE`: Set to "quiet", "verbose", or "default"
- `AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE`: Set to "post-tool-use", "pre-commit-hook", or "default"  
- `AGENTIC_NAVIGATION_GUIDE_PATH`: Default path to guide file
- `AGENTIC_NAVIGATION_GUIDE_ROOT`: Default root directory for operations

## Quick Start for Next Session

1. **Run tests** to verify everything still works:
   ```bash
   cargo test
   cargo clippy
   ```

2. **Pick a task** from "Next Work To Be Done" - integration tests are highest priority

3. **Verify current functionality** with:
   ```bash
   cargo run -- verify  # Should pass with the project's own guide
   cargo run -- dump --exclude target --exclude .git
   ```

4. **Check the project's own navigation guide** at `AGENTIC_NAVIGATION_GUIDE.md`

## Summary

The core functionality is complete and working well. The parser builds proper hierarchies, the dumper respects exclusion patterns, and the validator enforces all syntax rules. The main remaining work is adding integration tests to ensure the CLI behaves correctly in all scenarios, followed by nice-to-have features like symlink support and CI/CD setup.

The codebase is clean, well-tested, and ready for the final push to production readiness!