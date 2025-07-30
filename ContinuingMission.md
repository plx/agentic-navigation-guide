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

### ✅ 8. Testing Infrastructure - COMPREHENSIVE
- **9 unit tests** covering core modules (parser, validator, dumper, verifier)
- **16 integration tests** in `tests/cli_tests.rs` covering:
  - All CLI commands with various options
  - Edge cases (empty guides, type mismatches, invalid paths)
  - Execution modes (post-tool-use, pre-commit-hook, quiet)
  - Complex scenarios (nested directories, glob patterns)
- **Total: 25 tests** providing robust coverage
- All tests passing ✅

### ✅ 9. Code Quality
- All code formatted with rustfmt
- No clippy warnings
- Consistent error handling throughout
- Clean module separation

## Next Work To Be Done

### 1. Add Symlink Support (MEDIUM PRIORITY)

The verifier currently doesn't handle symlinks. Implementation needed:

```rust
// In src/verifier.rs, update verify_item() method
FilesystemItem::Symlink { path, target, .. } => {
    let full_path = current_path.join(path);
    
    // Check if symlink exists
    if !full_path.exists() && !full_path.symlink_metadata().is_ok() {
        errors.push(SemanticError {
            line: item.line_number,
            message: format!("symlink not found: '{}'", full_path.display()),
        });
        return;
    }
    
    // Verify it's actually a symlink
    match full_path.symlink_metadata() {
        Ok(metadata) if metadata.is_symlink() => {
            // If target is specified, verify it matches
            if let Some(expected_target) = target {
                match std::fs::read_link(&full_path) {
                    Ok(actual_target) => {
                        if actual_target != Path::new(expected_target) {
                            errors.push(SemanticError {
                                line: item.line_number,
                                message: format!(
                                    "symlink target mismatch: expected '{}', found '{}'",
                                    expected_target,
                                    actual_target.display()
                                ),
                            });
                        }
                    }
                    Err(e) => {
                        errors.push(SemanticError {
                            line: item.line_number,
                            message: format!("cannot read symlink target: {}", e),
                        });
                    }
                }
            }
        }
        Ok(_) => {
            errors.push(SemanticError {
                line: item.line_number,
                message: format!("expected symlink but found regular file/directory"),
            });
        }
        Err(e) => {
            errors.push(SemanticError {
                line: item.line_number,
                message: format!("cannot read symlink metadata: {}", e),
            });
        }
    }
}
```

Also update parser to support inline symlink syntax:
```rust
// In src/parser.rs, update line parsing regex
// Support syntax like: "- link -> /target/path"
```

### 2. Create CI/CD Pipeline (MEDIUM PRIORITY)

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        rust: [stable, beta]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
          components: rustfmt, clippy
      
      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Cache cargo index
        uses: actions/cache@v3
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-cargo-index-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Cache cargo build
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-cargo-build-target-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Check formatting
        run: cargo fmt -- --check
      
      - name: Run clippy
        run: cargo clippy -- -D warnings
      
      - name: Run tests
        run: cargo test --verbose
      
      - name: Run integration tests
        run: cargo test --test cli_tests --verbose

  release:
    name: Release
    needs: test
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/')
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      
      - name: Build release binary
        run: cargo build --release
      
      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          files: target/release/agentic-navigation-guide
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### 3. Git Hooks Integration (LOW PRIORITY)

Create example hooks in a `hooks/` directory:

```bash
#!/bin/bash
# hooks/pre-commit
# Check all navigation guides in the repository

set -e

# Find all markdown files with navigation guides
for file in $(git diff --cached --name-only --diff-filter=ACM | grep '\.md$'); do
    if grep -q '<agentic-navigation-guide>' "$file"; then
        echo "Checking navigation guide in $file..."
        agentic-navigation-guide verify --guide "$file" || {
            echo "Navigation guide verification failed!"
            exit 1
        }
    fi
done

echo "All navigation guides verified successfully!"
```

Also create a setup script:
```bash
#!/bin/bash
# hooks/install.sh
# Install git hooks

HOOKS_DIR="$(git rev-parse --git-dir)/hooks"
cp hooks/pre-commit "$HOOKS_DIR/pre-commit"
chmod +x "$HOOKS_DIR/pre-commit"
echo "Git hooks installed successfully!"
```

### 4. Performance Optimizations (FUTURE)

For very large codebases:
- Implement parallel directory traversal using rayon
- Add caching for parsed guides (store in `.agentic-guide-cache/`)
- Stream processing for very large markdown files
- Optimize regex compilation (compile once, reuse)

### 5. Enhanced Documentation (FUTURE)

- Expand README.md with:
  - Installation instructions (cargo install, binary releases)
  - Integration guides (VS Code, CI/CD, git hooks)
  - Performance tips for large codebases
  - Troubleshooting guide
- Generate man pages using clap_mangen
- Create video tutorials

## Known Issues

1. **Symlink Support**: Not implemented yet
2. **Performance**: No optimizations for very large directory trees (100k+ files)
3. **Windows**: Path handling might need adjustments for Windows-style paths

## Development Commands

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run all tests (unit + integration)
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests  
cargo test --test cli_tests

# Test specific functionality
cargo test parser
cargo test validator

# Manual CLI testing
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
The parser correctly builds a tree structure from flat indented lists. Key algorithm:
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
3. **Property tests**: Could add proptest for fuzzing
4. **Benchmark tests**: For performance testing

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

2. **Pick a task** from "Next Work To Be Done" - symlink support is the most impactful

3. **Verify current functionality** with:
   ```bash
   cargo run -- verify  # Should pass with the project's own guide
   cargo run -- dump --exclude target --exclude .git
   ```

4. **Check git status** - there may be uncommitted changes from the last session

## Current Project State

The project is **production-ready** with comprehensive test coverage. The core functionality is complete, well-tested, and documented. The remaining tasks are enhancements that would make the tool more versatile but aren't required for basic functionality.

Key achievements:
- ✅ All core features implemented
- ✅ 25 tests providing comprehensive coverage
- ✅ Clean, maintainable architecture
- ✅ Proper error handling throughout
- ✅ Multiple execution modes for different use cases

The tool is ready for real-world use!