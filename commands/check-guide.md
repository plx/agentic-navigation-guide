---
description: Check navigation guide syntax without filesystem verification
---

# Check Navigation Guide Syntax

Run `agentic-navigation-guide check` to validate the syntax and formatting of a navigation guide without checking against the filesystem.

This command will:
- Parse the navigation guide from the markdown file
- Check for proper formatting (directories end with `/`, files don't)
- Verify consistent indentation (2 spaces per level)
- Check for valid path formats (no `.`, `..`, `./`, `../`)
- Validate placeholder (`...`) usage rules
- Report any syntax errors with line numbers

This is useful when:
- You're editing a guide and want quick feedback on formatting
- You're working on a guide for a directory structure that doesn't exist yet
- You want to validate guide syntax in a CI pipeline before filesystem checks

## Difference from `/verify-guide`

- **`/check-guide`**: Only validates syntax and formatting rules
- **`/verify-guide`**: Validates syntax AND checks that paths exist in the filesystem

## Options

```bash
# Check a specific guide file
agentic-navigation-guide check --guide path/to/guide.md
```

<!-- TODO: Customize this command description or add usage examples -->
