---
name: create-guide
description: Create a complete navigation guide for a repository from scratch. Use when setting up a new project with no existing guide.
---

# Create Guide Agent

This agent creates a comprehensive navigation guide for a repository by analyzing its structure and generating meaningful comments.

## Process

### Step 1: Generate Initial Structure

Run the dump command to get the directory structure:

```bash
agentic-navigation-guide dump --depth 3 --exclude target --exclude node_modules --exclude .git --exclude __pycache__ --exclude .venv --exclude dist --exclude build
```

This provides the raw structure to work from.

### Step 2: Identify Key Files

Focus on files that are most important for navigation:

**High priority (always include with comments):**
- Entry points (`main.rs`, `index.ts`, `app.py`, `main.go`)
- Library roots (`lib.rs`, `mod.rs`, `__init__.py`)
- Configuration (`Cargo.toml`, `package.json`, `pyproject.toml`)
- Core business logic files
- API definitions

**Medium priority (include, comment if non-obvious):**
- Module directories
- Test directories (top-level)
- Documentation files

**Low priority (use placeholders or omit):**
- Individual test files
- Generated files
- Build configuration details
- IDE settings

### Step 3: Analyze and Comment

For each important file/directory:

1. Read the file or examine directory contents
2. Identify primary purpose
3. Generate 5-10 word comment
4. Use project-specific terminology

### Step 4: Structure the Guide

Build the guide with:
- Proper indentation (2 spaces)
- Directories ending with `/`
- Comments after `#`
- Placeholders (`...`) for less important areas

### Step 5: Write the Guide

Create `AGENTIC_NAVIGATION_GUIDE.md`:

```markdown
<agentic-navigation-guide>
- src/
  - main.rs # Entry point comment
  - lib.rs # Library root comment
  - module/
    - mod.rs # Module entry
    - ... # Module internals
- tests/
  - ... # Test files
- Cargo.toml
- README.md
</agentic-navigation-guide>
```

### Step 6: Verify

Run verification to catch any errors:

```bash
agentic-navigation-guide verify
```

Fix any issues reported.

### Step 7: Integrate with CLAUDE.md

Check if `CLAUDE.md` exists:
- If yes, suggest adding `@AGENTIC_NAVIGATION_GUIDE.md` to include the guide
- If no, offer to create a minimal CLAUDE.md with the reference

## Output

The agent produces:
1. `AGENTIC_NAVIGATION_GUIDE.md` - The navigation guide file
2. Verification that the guide is valid
3. Suggestion for CLAUDE.md integration

## Example Output

For a typical Rust project:

```markdown
<agentic-navigation-guide>
- src/
  - main.rs # CLI entry point, argument parsing
  - lib.rs # Public API exports
  - config.rs # Configuration loading from env/files
  - db/
    - mod.rs # Database module root
    - connection.rs # PostgreSQL connection pool
    - migrations.rs # Schema migration runner
    - ... # Query implementations
  - api/
    - mod.rs # API module root
    - routes.rs # HTTP route definitions
    - handlers.rs # Request handlers
    - middleware.rs # Auth and logging middleware
  - models/
    - mod.rs # Domain models
    - ... # Individual model definitions
- tests/
  - integration/
    - ... # Integration tests
- migrations/
  - ... # SQL migration files
- Cargo.toml # Project manifest
- README.md
- ... # Config files, CI, etc.
</agentic-navigation-guide>
```

## Guidelines

1. **Don't over-document** - Focus on navigation value, not completeness
2. **Use placeholders liberally** - Better than listing every file
3. **Match project conventions** - Use terminology from the codebase
4. **Verify before delivering** - Always run verification
5. **Consider the audience** - What would help an AI assistant navigate?
