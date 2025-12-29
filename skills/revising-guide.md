---
name: revising-guide
description: Update an existing navigation guide when the codebase changes. Use when files have been added, removed, renamed, or restructured.
---

# Revising an Existing Navigation Guide

This skill teaches you how to update a navigation guide to keep it in sync with filesystem changes.

## When to Update

Update the guide after:
- **Adding** new files or directories
- **Deleting** files or directories
- **Renaming** or moving files
- **Restructuring** the codebase (e.g., reorganizing modules)
- **Changing file purposes** (update comments)

## Update Process

### Step 1: Identify Issues

Run verification to see what's wrong:

```bash
agentic-navigation-guide verify
```

Common errors:
- `path does not exist` - File was deleted or moved
- `expected directory, found file` - Type mismatch
- `expected file, found directory` - Type mismatch
- `placeholder has no unlisted items` - All items now listed

### Step 2: Gather Current State

See what actually exists:

```bash
agentic-navigation-guide dump --depth 3
```

Compare this output with your guide to identify:
- New files not in the guide
- Guide entries with no matching files
- Structural changes

### Step 3: Make Updates

**For deleted files:** Remove the entry from the guide.

**For renamed files:** Update the path, keep the comment if still accurate.

**For moved files:** Update the path to new location, adjust indentation.

**For new files:** Add entries in the appropriate location with comments.

**For restructured directories:** May need to reorganize multiple entries.

### Step 4: Verify Again

```bash
agentic-navigation-guide verify
```

Repeat until no errors remain.

## Maintaining Consistency

### Match Existing Style

If existing comments use a pattern, follow it:

```markdown
# If existing comments are terse:
- new-file.rs # Token validation

# If existing comments are more detailed:
- new-file.rs # Validates JWT tokens and checks expiration
```

### Preserve Comment Tone

```markdown
# If existing uses active voice:
- auth.rs # Handles OAuth2 flow
- new.rs # Validates user input   # <-- match this style

# If existing uses nouns:
- auth.rs # OAuth2 authentication
- new.rs # Input validation        # <-- match this style
```

### Keep Ordering Conventions

Some guides order entries:
- Alphabetically
- By importance
- Directories first, then files
- By module dependency

Maintain whatever convention exists.

## Handling Different Scenarios

### Scenario: File Renamed

Before:
```markdown
- src/
  - old_name.rs # Does something
```

After:
```markdown
- src/
  - new_name.rs # Does something
```

### Scenario: File Moved to New Directory

Before:
```markdown
- src/
  - utils.rs # String helpers
```

After:
```markdown
- src/
  - helpers/
    - utils.rs # String helpers
```

### Scenario: New Module Added

Before:
```markdown
- src/
  - main.rs
  - lib.rs
```

After:
```markdown
- src/
  - main.rs
  - lib.rs
  - auth/
    - mod.rs # Auth module root
    - oauth.rs # OAuth2 implementation
    - jwt.rs # JWT handling
```

### Scenario: Directory Flattened

Before:
```markdown
- src/
  - handlers/
    - api.rs
    - web.rs
```

After:
```markdown
- src/
  - api_handlers.rs # Combined API handlers
  - web_handlers.rs # Combined web handlers
```

### Scenario: Using Placeholders for Partial Updates

When you don't want to enumerate everything:

```markdown
- src/
  - main.rs # Entry point
  - core/
    - important.rs # Key module - document this
    - ... # Other core modules
  - utils/
    - ... # Utility modules
```

## Dealing with Large Changes

For major restructuring:

1. **Option A: Incremental updates**
   - Fix errors one at a time
   - Verify after each change
   - Slower but safer

2. **Option B: Regenerate and merge**
   - Run `agentic-navigation-guide dump` for new structure
   - Manually merge comments from old guide
   - Faster but requires care to preserve comments

3. **Option C: Start fresh**
   - Run `agentic-navigation-guide init`
   - Re-add comments (may be needed for major rewrites)

## Common Mistakes to Avoid

### 1. Forgetting to update comments

When a file's purpose changes, update the comment:

```markdown
# Wrong - file now does more
- auth.rs # Basic auth     # (but now also does OAuth)

# Right
- auth.rs # Basic auth and OAuth2
```

### 2. Leaving stale placeholders

If you listed everything, remove the placeholder:

```markdown
# Wrong - no other files exist
- src/
  - main.rs
  - lib.rs
  - ...     # ERROR: no unlisted items

# Right
- src/
  - main.rs
  - lib.rs
```

Or add a comment to make it valid:
```markdown
- src/
  - main.rs
  - lib.rs
  - ... # Future modules
```

### 3. Inconsistent indentation

All children must be indented the same amount:

```markdown
# Wrong
- src/
  - main.rs
   - lib.rs   # <-- different indent

# Right
- src/
  - main.rs
  - lib.rs
```

## Quick Reference

| Change | Action |
|--------|--------|
| File deleted | Remove entry |
| File added | Add entry with comment |
| File renamed | Update path |
| File moved | Update path and indentation |
| Directory deleted | Remove directory and all children |
| Directory added | Add directory and children |
| Purpose changed | Update comment |
