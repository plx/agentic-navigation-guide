---
name: audit-guide
description: Audit an existing navigation guide for structural errors and stale descriptions. Runs validation, checks file descriptions against contents, and reports issues.
context: fork
agent: nav-guide-worker
disable-model-invocation: true
argument-hint: [guide-path]
allowed-tools: Read Glob Grep Bash Edit Write Agent
---

Audit an existing agentic navigation guide for accuracy and completeness.

**Guide file:** $ARGUMENTS (defaults to `AGENTIC_NAVIGATION_GUIDE.md` in current directory)

## Workflow

### Phase 1: Structural Validation

1. Run `agentic-navigation-guide verify --guide <guide-path> --root <guide-parent-directory>` against the guide file, substituting the actual guide path and its parent directory.
2. If there are structural errors (missing files, wrong types), collect them as the first set of issues.

### Phase 2: Description Audit

3. Parse the guide to extract all file and directory entries with their descriptions.
4. For each entry that has a description:
   - Read the file or directory
   - Compare the description against the current contents
   - Flag descriptions that are materially wrong, misleading, or refer to functionality that no longer exists
5. Do NOT flag descriptions for minor wording preferences — only material inaccuracies.

### Phase 3: Completeness Check

6. Compare the guide against the actual directory structure:
   - Look for high-value files that exist but are not in the guide (entry points, core types, important configs)
   - Look for directories that have grown significantly since the guide was written
7. Only flag truly important omissions — the guide is intentionally selective.

### Phase 4: Report

8. Present findings in three sections:

```
## Structural Issues
<issues from verify, if any>

## Stale Descriptions
<entries where description doesn't match content>
- line N: path — current: "old description" → suggested: "new description"

## Suggested Additions
<high-value items missing from the guide>
- path — reason for inclusion

## Summary
X structural issues, Y stale descriptions, Z suggested additions
```

9. If the user confirms, fix the issues:
   - Structural issues: remove entries for deleted files, add entries for missing ones
   - Stale descriptions: update descriptions in place
   - Additions: add new entries at appropriate positions
10. Re-run `agentic-navigation-guide verify --guide <guide-path> --root <guide-parent-directory>` to confirm all fixes are valid.

## Guidelines

- Be conservative with "stale" verdicts — descriptions that are directionally correct but imprecise should not be flagged
- For completeness checks, consider recent git history (`git log --oneline -20 --name-only`) to prioritize recently-added files
- Never remove entries just because they seem unimportant — the author may have included them for a reason
