---
name: authoring-guide
description: Create a new navigation guide from scratch. Use when setting up a guide for a repository that doesn't have one yet.
---

# Authoring a Navigation Guide

This skill teaches you how to create an effective navigation guide from scratch.

## File Format Basics

A navigation guide is embedded in markdown using XML-like sentinel tags:

```markdown
<agentic-navigation-guide>
- src/
  - main.rs # Entry point
  - lib.rs # Core library
- Cargo.toml
- README.md
</agentic-navigation-guide>
```

### Core Syntax Rules

1. **Directories** must end with `/`:
   - `- src/` (correct)
   - `- src` (incorrect - will be treated as a file)

2. **Files** do NOT end with `/`:
   - `- main.rs` (correct)
   - `- main.rs/` (incorrect - will be treated as a directory)

3. **Indentation** shows hierarchy:
   - Use consistent spacing (2 spaces recommended)
   - Children are indented one level deeper than their parent
   - The first indented item sets the indent size for the entire guide

4. **Comments** come after `#`:
   - `- main.rs # Entry point for the application`
   - Whitespace between path and `#` is optional

5. **No blank lines** inside the guide block

6. **No special directories**: `.`, `..`, `./`, `../` are forbidden

## What to Include

**DO include:**
- Source code directories (`src/`, `lib/`, `app/`)
- Configuration files (`Cargo.toml`, `package.json`, `pyproject.toml`)
- Documentation (`README.md`, `docs/`)
- Key entry points and core modules
- Test directories if they have complex structure

**DO NOT include:**
- Build artifacts (`target/`, `dist/`, `build/`, `out/`)
- Dependency directories (`node_modules/`, `vendor/`, `.venv/`)
- Cache directories (`.cache/`, `__pycache__/`)
- IDE/editor directories (`.idea/`, `.vscode/` unless project-specific)
- Generated files (`.d.ts` from TypeScript, etc.)
- Git internals (`.git/`)

## Writing Good Comments

Comments should explain **purpose**, not repeat the filename.

**Good comments:**
```markdown
- src/
  - auth/
    - oauth.rs # OAuth2 flow with token refresh
    - jwt.rs # JWT creation and validation
  - api/
    - handlers.rs # HTTP request handlers for REST endpoints
    - middleware.rs # Request logging and auth middleware
```

**Bad comments:**
```markdown
- src/
  - auth/
    - oauth.rs # OAuth file
    - jwt.rs # JWT code
  - api/
    - handlers.rs # Handler functions
    - middleware.rs # Middleware
```

Guidelines for comments:
- Keep to 5-10 words
- Use active voice ("Handles X" not "X handling")
- Focus on what an agent needs to know to navigate
- Use project-specific terminology

## Advanced Features

### Placeholders (`...`)

Use `...` to indicate there are additional items not listed:

```markdown
<agentic-navigation-guide>
- src/
  - main.rs # Entry point
  - ... # Other source files
- tests/
  - integration/
    - auth_test.rs
    - ... # Additional integration tests
</agentic-navigation-guide>
```

**Two types of placeholders:**

1. **Without comment** - Must represent actual unlisted items:
   ```markdown
   - src/
     - main.rs
     - ...   # There MUST be other files in src/ not listed
   ```

2. **With comment** - Can be used anywhere, even if all items are listed:
   ```markdown
   - plans/
     - phase-01.md # Completed
     - ... # Future phases will appear here
   ```

Rules:
- Placeholders cannot have children
- Cannot have two adjacent placeholders

### Choice Expansion (`[option1, option2]`)

Group related files on a single line:

```markdown
- FooCoordinator[.h, .cpp] # Coordinates foo operations
```

Expands to:
```markdown
- FooCoordinator.h # Coordinates foo operations
- FooCoordinator.cpp # Coordinates foo operations
```

More examples:
```markdown
- Config[, .local].json     # Config.json and Config.local.json
- src[/main, /lib].rs       # src/main.rs and src/lib.rs
```

Rules:
- At most one `[...]` block per line
- Whitespace inside brackets is trimmed
- Use quotes for values with spaces/commas: `["with , comma"]`
- Escape special chars: `\,` `\"` `\\` `\[` `\]`

### Ignoring Guides (`ignore=true`)

Mark guides to skip verification (useful for documentation examples):

```markdown
<agentic-navigation-guide ignore=true>
- example/
  - fictional-file.rs
</agentic-navigation-guide>
```

## Complete Example

Here's a well-structured guide for a Rust project:

```markdown
<agentic-navigation-guide>
- src/
  - main.rs # CLI entry point, argument parsing
  - lib.rs # Public API and module exports
  - types.rs # Core domain types and structs
  - errors.rs # Error types and conversion impls
  - parser.rs # Config file parsing logic
  - cli/
    - mod.rs # CLI module root
    - commands.rs # Subcommand implementations
    - ... # Additional CLI utilities
- tests/
  - integration/
    - ... # Integration test files
- Cargo.toml # Project manifest and dependencies
- README.md # Project documentation
- ... # License, CI configs, etc.
</agentic-navigation-guide>
```

## Workflow

1. Run `agentic-navigation-guide init` to generate a starting point
2. Edit to add meaningful comments
3. Remove entries for build artifacts and generated files
4. Add placeholders where appropriate
5. Run `agentic-navigation-guide verify` to validate
6. Include in your CLAUDE.md: `@AGENTIC_NAVIGATION_GUIDE.md`
