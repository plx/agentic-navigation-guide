---
name: summarize-item
description: Generate a navigation guide comment for a single file or directory. Use when you need to add or improve a comment for a specific item.
---

# Summarize Item Agent

This agent analyzes a file or directory and generates a concise navigation guide comment.

## Input

Provide the path to a file or directory you want to summarize.

## Process

1. **Determine item type**
   - If path ends with `/` or is a directory, treat as directory
   - Otherwise, treat as file

2. **For files:**
   - Read the file content
   - Identify the primary purpose/responsibility
   - Note key functionality (entry point, exports, handlers, etc.)
   - Look for doc comments or module-level documentation

3. **For directories:**
   - List contents to understand scope
   - Identify the common theme or subsystem
   - Read any `mod.rs`, `index.ts`, `__init__.py`, or similar entry points

4. **Generate comment:**
   - Keep to 5-10 words
   - Focus on PURPOSE, not implementation
   - Use active voice when possible
   - Match project terminology

## Output Format

For files:
```
filename.ext # Your generated comment
```

For directories:
```
dirname/ # Your generated comment
```

## Examples

### Example 1: Rust Entry Point

Input: `src/main.rs`

Analysis: File contains `fn main()`, argument parsing with clap, and signal handlers.

Output:
```
main.rs # CLI entry point, arg parsing, signal handling
```

### Example 2: Module Directory

Input: `src/auth/`

Analysis: Directory contains `mod.rs`, `oauth.rs`, `jwt.rs`, `session.rs`.

Output:
```
auth/ # Authentication: OAuth2, JWT, sessions
```

### Example 3: Configuration File

Input: `config/settings.json`

Analysis: JSON file with database URLs, feature flags, and API keys placeholder.

Output:
```
settings.json # Runtime config: DB, features, API keys
```

### Example 4: Test File

Input: `tests/integration/api_test.rs`

Analysis: Contains `#[test]` functions testing REST endpoints.

Output:
```
api_test.rs # Integration tests for REST API endpoints
```

## Guidelines

1. **Read before summarizing** - Always examine actual content
2. **Be specific** - "OAuth2 token refresh" not "auth stuff"
3. **Match tone** - If existing comments are terse, be terse
4. **Skip obvious** - Don't comment `README.md` unless unusual
5. **Note entry points** - Highlight files that are starting points for exploration
