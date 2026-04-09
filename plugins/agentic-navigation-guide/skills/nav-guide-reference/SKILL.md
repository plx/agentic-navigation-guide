---
name: nav-guide-reference
description: Reference for the agentic-navigation-guide CLI tool and navigation guide format. Activates when working with AGENTIC_NAVIGATION_GUIDE.md files or the agentic-navigation-guide CLI.
user-invocable: false
---

# Agentic Navigation Guide — Quick Reference

## What It Is

A **navigation guide** is a hand-written partial file listing that helps AI coding assistants navigate codebases efficiently. It lives in an `AGENTIC_NAVIGATION_GUIDE.md` file and is typically referenced from `CLAUDE.md` via the `@` syntax.

Unlike auto-generated file trees, navigation guides are *selective*: they list only the items an AI assistant needs to know about, with terse descriptions explaining each item's purpose.

## Guide Format

```
<agentic-navigation-guide>
- src/
  - main.rs # Entry point
  - lib.rs # Core library logic
  - utils/ # Shared helpers
- Cargo.toml
- README.md
- ... # Other project files
</agentic-navigation-guide>
```

- Directories end with `/`
- Comments follow the first unescaped `#`
- `...` = placeholder for unlisted items (with comment) or omitted existing items (without)
- Consistent indentation for nesting (2 spaces recommended; any consistent unit accepted)
- No blank lines within the block

For the full format specification, see `format-reference.md` in this skill directory.

## CLI Commands

| Command | Purpose |
|---------|---------|
| `agentic-navigation-guide init --output AGENTIC_NAVIGATION_GUIDE.md` | Generate a starting-point guide from current directory |
| `agentic-navigation-guide check` | Validate guide syntax (no filesystem check) |
| `agentic-navigation-guide verify --guide <guide-path> --root <guide-parent-directory>` | Validate guide against actual filesystem |
| `agentic-navigation-guide dump` | Dump directory tree in guide format |

### Common Flags

```bash
# Verify with specific guide and root
agentic-navigation-guide verify --guide path/to/guide.md --root /project

# Dump with depth limit and exclusions
agentic-navigation-guide dump --depth 3 --exclude target --exclude .git

# Verify recursively (monorepos)
agentic-navigation-guide verify --recursive --exclude node_modules

# Post-tool-use hook mode (exit code 2 on failure)
agentic-navigation-guide verify --post-tool-use-hook

# GitHub Actions mode (concise output, file:line format)
agentic-navigation-guide verify --github-actions-check
```

## Typical Workflow

1. `agentic-navigation-guide init --output AGENTIC_NAVIGATION_GUIDE.md` — scaffold initial guide
2. Hand-edit to add descriptions and remove noise
3. `agentic-navigation-guide verify --guide AGENTIC_NAVIGATION_GUIDE.md --root .` — check for errors
4. Commit and reference from `CLAUDE.md` with `@AGENTIC_NAVIGATION_GUIDE.md`

For advanced usage (environment variables, execution modes, recursive verification, choice expansions), see `advanced-usage.md` in this skill directory.
