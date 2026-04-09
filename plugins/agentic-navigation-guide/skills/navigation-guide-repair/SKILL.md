---
name: navigation-guide-repair
description: Fix a navigation guide based on a specific problem description. Used when hooks or audits detect that a guide has fallen out of sync.
context: fork
agent: nav-guide-worker
user-invocable: false
allowed-tools: Read Glob Grep Bash Edit Write
---

Repair a navigation guide to bring it back in sync with the filesystem.

**Guide file:** $0
**Problem description:** $1

## Instructions

1. Read the current guide file
2. Understand the problem from the description (e.g., "src/old.rs was removed", "new directory src/auth/ was added")
3. Make the **minimal edit** needed to fix the issue:
   - **Removed file/directory**: Remove the corresponding line from the guide
   - **Added file/directory**: Add a new entry with an appropriate description if the item has navigational value; skip if it doesn't
   - **Renamed/moved**: Remove old entry, add new entry preserving the description
   - **Type change** (file became directory or vice versa): Update the trailing `/`
4. Preserve existing formatting, indentation, and comment style
5. Run `agentic-navigation-guide verify --guide <guide-path> --root <guide-parent-directory>` to confirm the fix, substituting the guide path from above and its parent directory
6. If verification fails, diagnose and fix remaining issues

## Important

- Do NOT rewrite the entire guide — make targeted fixes only
- Do NOT change descriptions of items that weren't affected by the problem
- Do NOT reorder entries unless necessary to fix the issue
- If adding a new item, match the description style of surrounding entries
