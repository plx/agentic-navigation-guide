# Navigation Guide Format Reference

## Structure

A navigation guide is a markdown block wrapped in `<agentic-navigation-guide>` tags containing a hierarchical list of filesystem items.

## Line Format

```
- [path][/] [# comment]
```

- **Path**: Relative path component (no `.`, `..`, `//`, or leading `/`)
- **Trailing `/`**: Marks the entry as a directory
- **`#` comment**: Optional description (first unescaped `#` starts it; use `\#` for literal)
- **Indentation**: 2 spaces per nesting level, consistent throughout

## Entry Types

| Syntax | Type | Example |
|--------|------|---------|
| `- file.rs` | File | `- main.rs # Entry point` |
| `- dir/` | Directory | `- src/ # Source code` |
| `- ...` | Placeholder | `- ... # Other files` |

## Placeholder Rules

| Variant | Requirement |
|---------|-------------|
| `... # comment` | Allowed anywhere (even empty dirs) |
| `...` (no comment) | Parent must have unlisted items |

- No children under `...`
- No adjacent `...` entries

## Choice Expansions

```
- Prefix[suffix1, suffix2] # Shared comment
```

Expands into one entry per option. Max one choice list per line.

## Validation Errors

**Syntax errors** (caught by `check`):
- Invalid indentation (not a multiple of indent unit)
- Missing `-` prefix
- Empty path
- Path with `.` or `..` components
- Path with `//` or leading `/`
- Adjacent placeholders
- Children under placeholder

**Semantic errors** (caught by `verify`):
- Path does not exist on filesystem
- Entry marked as file but is a directory (or vice versa)
- Uncommented `...` with no unlisted siblings
- Duplicate entries
