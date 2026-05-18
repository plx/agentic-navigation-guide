---
title: Guide Format
description: Syntax rules for agentic navigation guide blocks.
---

A navigation guide is a markdown block with one list item per file, directory, or placeholder.

```md
<agentic-navigation-guide>
- src/
  - main.rs # Main entry point
  - lib.rs # Core logic
  - cli/
    - verify.rs # verify subcommand
- Cargo.toml
- ... # Additional project files
</agentic-navigation-guide>
```

## Rules

- Each entry starts with `-`.
- Indentation expresses nesting.
- A trailing `/` marks a directory.
- Comments start at the first unescaped `#`.
- Use `\#` to include a literal hash in a path.
- Blank lines are not allowed inside a guide block.
- Paths must be relative and cannot contain `.`, `..`, or empty path components.

## Placeholders

Use `...` to indicate omitted entries.

```md
- src/
  - main.rs
  - ... # Other source files
```

A commented placeholder can also reserve room for future files. An uncommented placeholder must correspond to at least one currently unlisted item.

## Choice lists

Small filename alternatives can be grouped with one choice list.

```md
- FooCoordinator[.h, .cpp] # Coordinates foo interactions
- Config[, .local].json
```
