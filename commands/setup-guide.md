---
description: Set up a repository with a navigation guide, including initialization, customization, and CLAUDE.md integration
---

# Setup Navigation Guide

Set up a complete navigation guide for this repository.

## What This Command Does

1. **Checks for existing guide**
   - If `AGENTIC_NAVIGATION_GUIDE.md` exists, offers to update or replace it

2. **Generates initial structure**
   - Runs `agentic-navigation-guide init` with sensible defaults
   - Excludes common build artifacts (`target/`, `node_modules/`, `.git/`, etc.)

3. **Adds meaningful comments**
   - Analyzes key files to generate helpful navigation comments
   - Focuses on entry points, core modules, and configuration

4. **Verifies the guide**
   - Runs `agentic-navigation-guide verify` to ensure accuracy

5. **Integrates with CLAUDE.md**
   - If CLAUDE.md exists, suggests adding `@AGENTIC_NAVIGATION_GUIDE.md`
   - If not, offers to create a minimal CLAUDE.md with the reference

## Usage

Simply run:
```
/setup-guide
```

## Options

You may be asked:
- Whether to overwrite an existing guide
- Which files deserve detailed comments
- Whether to create/update CLAUDE.md

## After Setup

Once complete, you should:
1. Review the generated comments and adjust as needed
2. Remove any entries that shouldn't be tracked
3. Add comments to important files that were missed
4. Run `/verify-guide` to confirm everything is correct

## See Also

- `/verify-guide` - Verify guide accuracy
- `/update-guide` - Update an existing guide
- `/check-guide` - Check guide syntax only
