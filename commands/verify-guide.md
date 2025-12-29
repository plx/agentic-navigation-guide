---
description: Verify navigation guide accuracy against current filesystem
---

# Verify Navigation Guide

Run `agentic-navigation-guide verify` to check that the navigation guide in `AGENTIC_NAVIGATION_GUIDE.md` (or specified guide file) matches the current state of the filesystem.

This command will:
- Parse the navigation guide from the markdown file
- Check for syntax errors (proper formatting, indentation, path conventions)
- Verify that all referenced paths exist in the filesystem
- Ensure directories end with `/` and files don't
- Report any mismatches or errors with line numbers

## Options

You can customize the verification by running the command with additional flags:

```bash
# Verify a specific guide file
agentic-navigation-guide verify --guide path/to/guide.md

# Verify relative to a specific root directory
agentic-navigation-guide verify --root /path/to/root

# Recursively verify all guides in a monorepo
agentic-navigation-guide verify --recursive

# Use custom guide name for recursive verification
agentic-navigation-guide verify --recursive --guide-name GUIDE.md
```

<!-- TODO: Customize this command description, add examples, or modify the default behavior -->
