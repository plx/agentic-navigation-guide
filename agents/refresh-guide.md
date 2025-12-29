---
name: refresh-guide
description: Update an existing navigation guide to match the current filesystem state. Use when verification fails or after significant codebase changes.
---

# Refresh Guide Agent

This agent updates an existing navigation guide to reflect filesystem changes while preserving existing comments and style.

## Process

### Step 1: Run Verification

Check current state:

```bash
agentic-navigation-guide verify
```

If verification passes, the guide is already up to date. Report success and exit.

### Step 2: Parse Verification Errors

Common error types to handle:

| Error | Meaning | Action |
|-------|---------|--------|
| `path does not exist` | File/dir was deleted | Remove entry |
| `expected directory, found file` | Type changed | Update entry (remove `/`) |
| `expected file, found directory` | Type changed | Update entry (add `/`) |
| `placeholder has no unlisted items` | All items now listed | Remove placeholder or add comment |

### Step 3: Get Current Filesystem State

Generate fresh structure:

```bash
agentic-navigation-guide dump --depth 3 --exclude target --exclude node_modules --exclude .git
```

Compare with existing guide to identify:
- New files/directories not in guide
- Structural changes (moves, renames)

### Step 4: Read Existing Guide

Read `AGENTIC_NAVIGATION_GUIDE.md` and note:
- Existing comment style (terse vs. detailed)
- Ordering convention (alphabetical, by importance, etc.)
- Which items have comments vs. which don't
- Placeholder usage patterns

### Step 5: Make Updates

**For each verification error:**

1. **Deleted items**: Remove the entry from the guide
2. **Type changes**: Update the path (add/remove trailing `/`)
3. **Stale placeholders**: Either remove or add a comment

**For new files/directories:**

1. Determine if important enough to add (entry points, core modules)
2. Generate comment matching existing style
3. Insert in appropriate location maintaining order

**For moved/renamed items:**

1. Find the old entry
2. Update path to new location
3. Preserve the existing comment if still accurate
4. Adjust indentation if hierarchy changed

### Step 6: Preserve Style

Match the existing guide's conventions:

```markdown
# If existing comments are short:
- new_file.rs # Token validation

# If existing comments are descriptive:
- new_file.rs # Validates JWT tokens and checks expiry dates
```

### Step 7: Verify Again

```bash
agentic-navigation-guide verify
```

Repeat steps 4-6 until verification passes.

### Step 8: Report Changes

Summarize what was updated:
- Files/directories removed
- Files/directories added
- Paths updated
- Comments modified

## Example Refresh

### Before (with errors)

Guide:
```markdown
<agentic-navigation-guide>
- src/
  - main.rs # Entry point
  - old_module.rs # Old module
  - utils/
    - helpers.rs
</agentic-navigation-guide>
```

Errors:
```
line 4: path 'src/old_module.rs' does not exist
line 6: path 'src/utils/helpers.rs' does not exist
```

Current filesystem:
```
src/
  main.rs
  new_module.rs
  lib/
    helpers.rs
```

### After (fixed)

```markdown
<agentic-navigation-guide>
- src/
  - main.rs # Entry point
  - new_module.rs # Replaces old_module
  - lib/
    - helpers.rs # Moved from utils/
</agentic-navigation-guide>
```

Changes made:
- Removed `old_module.rs` (deleted)
- Added `new_module.rs` (new file)
- Renamed `utils/` to `lib/` (directory renamed)
- Moved `helpers.rs` under `lib/` (file moved)

## Handling Edge Cases

### Large-Scale Restructuring

If many files changed, consider:
1. Running `agentic-navigation-guide dump` for complete new structure
2. Manually merging comments from old guide to new structure
3. This is faster than updating entry by entry

### New Subdirectory with Many Files

Instead of listing all files:
```markdown
- new_feature/
  - mod.rs # Feature module root
  - ... # Implementation files
```

### Entire Directory Deleted

Remove the directory entry AND all its children from the guide.

### File Became Directory (or vice versa)

This usually indicates significant restructuring. Check if the purpose changed:
```markdown
# Before: single file
- auth.rs # Authentication logic

# After: became a module directory
- auth/
  - mod.rs # Auth module root
  - oauth.rs # OAuth2 implementation
  - jwt.rs # JWT handling
```

## Output

The agent produces:
1. Updated `AGENTIC_NAVIGATION_GUIDE.md`
2. Successful verification
3. Summary of changes made

## Guidelines

1. **Preserve comments** - Existing comments represent human knowledge
2. **Match style** - New entries should look like existing ones
3. **Don't over-add** - Only add entries that help navigation
4. **Verify iteratively** - Fix one category of errors at a time
5. **Report clearly** - Tell the user exactly what changed
