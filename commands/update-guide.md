---
description: Update an existing navigation guide to match current filesystem state
---

# Update Navigation Guide

Update the navigation guide to reflect recent changes to the codebase.

## What This Command Does

1. **Verifies current state**
   - Runs `agentic-navigation-guide verify` to check for mismatches

2. **If guide is valid**
   - Reports "Navigation guide is up to date"
   - No changes needed

3. **If errors found**
   - Identifies what changed (deleted files, new files, renames)
   - Makes targeted updates to fix mismatches
   - Preserves existing comments and style
   - Adds comments for significant new files

4. **Re-verifies**
   - Confirms all issues are resolved

5. **Reports changes**
   - Summarizes what was added, removed, or modified

## Usage

Simply run:
```
/update-guide
```

## When to Use

Run this command after:
- Adding new files or directories
- Deleting files or directories
- Renaming or moving files
- Restructuring modules
- The post-tool-use hook reports guide issues

## What Gets Updated

| Change Type | Action |
|-------------|--------|
| Deleted file | Entry removed |
| New file | Entry added (with comment if important) |
| Renamed file | Path updated, comment preserved |
| Moved file | Path and indentation updated |
| Type change | Updated (file to dir or vice versa) |

## Preserving Your Work

The command preserves:
- Existing comments (won't overwrite your descriptions)
- Comment style (terse vs. detailed)
- Ordering conventions
- Placeholder patterns

## See Also

- `/verify-guide` - Check guide without making changes
- `/setup-guide` - Create a new guide from scratch
- `/check-guide` - Check syntax only
