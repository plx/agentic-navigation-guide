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

## What's Already Been Done

### 1. Project Infrastructure ✅
- Created Rust project with proper Cargo.toml configuration
- Set up all dependencies (clap, thiserror, walkdir, globset, regex, etc.)
- Configured rustfmt and clippy for code quality
- Established module structure with clean separation of concerns

### 2. Core Data Types ✅
- `FilesystemItem` enum: Represents files, directories, and symlinks
- `NavigationGuide` struct: Complete guide with items and metadata
- `NavigationGuideLine` struct: Individual parsed lines with hierarchy info
- `ExecutionMode` and `LogLevel` enums for CLI behavior
- Comprehensive error types with `thiserror`

### 3. Basic CLI Structure ✅
All four subcommands are implemented with basic functionality:
- `dump`: Generates guides from directories (with depth/exclusion support)
- `init`: Creates new guide files with boilerplate
- `check`: Validates syntax only
- `verify`: Validates syntax + filesystem matching

### 4. Module Implementation Status

#### Parser (`src/parser.rs`) - 60% Complete
- ✅ Extracts guide blocks from markdown
- ✅ Parses individual lines with regex
- ✅ Handles comments and paths
- ✅ Basic syntax error detection
- ❌ Hierarchical structure building (currently returns flat list)
- ❌ Full indentation validation

#### Validator (`src/validator.rs`) - 40% Complete
- ✅ Basic structure validation
- ✅ Empty guide detection
- ❌ Comprehensive syntax checking
- ❌ Indentation consistency validation
- ❌ Path format validation

#### Verifier (`src/verifier.rs`) - 50% Complete
- ✅ File/directory existence checking
- ✅ Type mismatch detection (file vs directory)
- ✅ Permission error handling
- ❌ Symlink target verification
- ❌ Hierarchical path building
- ❌ Comprehensive error reporting

#### Dumper (`src/dumper.rs`) - 70% Complete
- ✅ Directory traversal with WalkDir
- ✅ Max depth limiting
- ✅ Basic tree structure building
- ✅ Markdown formatting with proper indentation
- ❌ Exclusion patterns not working correctly
- ❌ Symlink handling
- ❌ Comment generation for special files

### 5. Testing
- 7 unit tests passing (basic functionality coverage)
- No integration tests yet
- No CI/CD pipeline

## Next Work To Be Done

### Immediate Priority: Fix Core Functionality

#### 1. Fix Parser Hierarchical Building (HIGH PRIORITY)
The parser currently returns a flat list instead of a proper tree structure. You need to:

```rust
// In parser.rs, implement build_hierarchy properly
fn build_hierarchy(&self, items: Vec<NavigationGuideLine>) -> Result<Vec<NavigationGuideLine>> {
    // Algorithm:
    // 1. Create a stack to track parent directories
    // 2. For each item:
    //    - Pop stack until we find parent at correct level
    //    - If item is directory, add children to it
    //    - Push directory items onto stack
    // 3. Return root-level items only
}
```

Key consideration: The indent level determines parent-child relationships. Items at indent_level N+1 are children of the most recent directory at indent_level N.

#### 2. Fix Dumper Exclusion Patterns (HIGH PRIORITY)
The glob exclusion patterns aren't working. The issue is likely in the path matching:

```rust
// In dumper.rs collect_entries()
// Current code checks relative paths, but WalkDir might need different handling
// Test with both relative and absolute paths
// Consider using WalkDir's filter_entry instead of post-filtering
```

#### 3. Complete Syntax Validation
Add these validations to `validator.rs`:
- Directory paths must end with `/` in the source
- No blank lines within guide blocks
- Consistent indentation (all multiples of first indent)
- No special paths (`.`, `..`, `./`, `../`)
- Valid path characters only

#### 4. Implement Integration Tests
Create `tests/integration/` directory with:
- Test fixtures (sample navigation guides)
- CLI invocation tests using `assert_cmd`
- End-to-end scenarios for all commands

### Secondary Priorities

#### 5. Advanced Features
- Symlink target validation in verifier
- Better error messages with line context
- Support for inline symlink syntax: `link -> target`
- Glob pattern documentation and examples

#### 6. CI/CD Pipeline
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
```

#### 7. Git Hooks Integration
Create example hooks in `.hooks/`:
- `pre-commit`: Run verify on tracked guide files
- `post-checkout`: Check if guides match new state

#### 8. Documentation
- Expand README with usage examples
- Add inline documentation for all public APIs
- Create CONTRIBUTING.md with development setup

## Development Commands

```bash
# Run formatter
cargo fmt

# Run linter
cargo clippy

# Run tests
cargo test

# Test individual commands
cargo run -- dump --depth 2 --exclude target --exclude .git
cargo run -- check
cargo run -- verify

# Build release version
cargo build --release
```

## Known Issues to Address

1. **Parser**: Hierarchical structure not built (returns flat list)
2. **Dumper**: Exclusion patterns not filtering correctly
3. **Validator**: Missing several syntax validations
4. **Error Messages**: Need better context (show problematic lines)
5. **Performance**: No optimizations yet for large directories

## Architecture Notes

The codebase follows clean architecture principles:
- `lib.rs`: Public API surface
- `types.rs`: Shared data structures
- `errors.rs`: All error types
- Each module has a single responsibility
- CLI module separate from core logic

When implementing fixes, maintain this separation. The CLI should only handle argument parsing and output formatting, not business logic.

## Testing Strategy

1. Unit tests for each module's public functions
2. Integration tests for CLI commands
3. Property-based tests for parser (using arbitrary guide structures)
4. Benchmark tests for large directory trees

## Final Notes

The foundation is solid, but the tool needs the hierarchical parsing fixed before it's truly useful. Focus on getting the core functionality working correctly before adding advanced features. The existing code structure makes it easy to add new functionality without major refactoring.

Remember to run `cargo fmt` and `cargo clippy` before committing changes. The project uses conservative linting settings appropriate for a first implementation.

Good luck with continuing this mission!