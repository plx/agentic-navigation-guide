---
name: summarizing-items
description: Write concise, helpful comments for files and directories in a navigation guide. Use when adding or improving comments.
---

# Summarizing Files and Directories

This skill teaches you how to write effective comments for navigation guide entries.

## Comment Structure

Comments appear after the `#` separator:

```markdown
- path/to/file.rs # Your comment here
- directory/ # Description of what's inside
```

Whitespace between the path and `#` is flexible - use what looks clean.

## Writing Effective Comments

### Focus on Purpose, Not Name

**Ask:** "Why would an agent need to look here?"

| Bad (repeats name) | Good (explains purpose) |
|-------------------|-------------------------|
| `main.rs # main file` | `main.rs # CLI entry point, arg parsing` |
| `utils.rs # utilities` | `utils.rs # String manipulation helpers` |
| `auth/ # authentication` | `auth/ # OAuth2 and JWT token handling` |

### Use Active Voice

| Passive/Noun | Active |
|--------------|--------|
| `# Authentication handling` | `# Handles OAuth2 authentication` |
| `# Database connection` | `# Connects to PostgreSQL` |
| `# Error definitions` | `# Defines app-wide error types` |

### Be Specific

| Vague | Specific |
|-------|----------|
| `# Config stuff` | `# Runtime config from env vars` |
| `# API code` | `# REST handlers for /users endpoint` |
| `# Tests` | `# Unit tests for parser module` |

### Keep It Short

Aim for 5-10 words. If you need more, the file might be doing too much.

| Too Long | Better |
|----------|--------|
| `# This file handles all the authentication logic including OAuth2 flows and JWT token validation` | `# OAuth2 and JWT authentication` |

## Directory vs File Comments

**Directories** describe the category or subsystem:
```markdown
- api/ # HTTP REST API layer
- db/ # Database access and migrations
- auth/ # Authentication and authorization
```

**Files** describe specific responsibilities:
```markdown
- api/
  - handlers.rs # Request handlers for all endpoints
  - middleware.rs # Logging and auth middleware
  - routes.rs # URL routing configuration
```

## Use Project Terminology

Match the vocabulary used in the codebase:

```markdown
# If the project calls them "coordinators"
- FooCoordinator.rs # Coordinates foo lifecycle

# If the project uses "manager"
- SessionManager.rs # Manages user sessions

# If the project has domain terms
- ledger.rs # Double-entry transaction ledger
```

## Anti-Patterns to Avoid

### 1. Repeating the filename
```markdown
# BAD
- parser.rs # parser
- config.json # configuration file

# GOOD
- parser.rs # Converts markdown to AST
- config.json # Build settings and feature flags
```

### 2. Being too vague
```markdown
# BAD
- utils.rs # utility functions
- helpers/ # helper code

# GOOD
- utils.rs # Date formatting and string helpers
- helpers/ # Shared test fixtures and mocks
```

### 3. Multi-sentence comments
```markdown
# BAD
- auth.rs # This handles authentication. It supports both OAuth2 and basic auth.

# GOOD
- auth.rs # OAuth2 and basic auth support
```

### 4. Implementation details
```markdown
# BAD
- cache.rs # Uses HashMap with LRU eviction

# GOOD
- cache.rs # In-memory response caching
```

## Examples of Good Comments

### Rust/Systems Project
```markdown
- src/
  - main.rs # CLI entry, signal handlers
  - lib.rs # Public API exports
  - parser/
    - mod.rs # Parser module root
    - lexer.rs # Tokenizes input stream
    - ast.rs # Abstract syntax tree types
  - codegen/
    - mod.rs # Code generation module
    - emit.rs # Emits target assembly
```

### Web Application
```markdown
- src/
  - app/
    - routes/ # URL route definitions
    - controllers/ # Request handlers
    - models/ # Database models (Prisma)
    - services/ # Business logic layer
  - lib/
    - auth.ts # JWT and session handling
    - db.ts # Database connection pool
```

### Python Package
```markdown
- src/
  - mypackage/
    - __init__.py # Package exports
    - core.py # Main algorithm implementation
    - io.py # File reading and writing
    - cli.py # Click command definitions
```

## When to Skip Comments

Some entries are self-explanatory:
- `Cargo.toml` - obvious for Rust projects
- `package.json` - obvious for JS projects
- `README.md` - universal

You can omit comments for these, or add them for consistency.
