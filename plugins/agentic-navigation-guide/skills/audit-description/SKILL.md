---
name: audit-description
description: Check if a file's navigation guide description is still accurate. Returns acceptable/needs-update with suggested replacement.
context: fork
agent: nav-guide-evaluator
user-invocable: false
allowed-tools: Read Glob Grep
---

Evaluate whether an existing navigation guide description for a file is still accurate.

**File path:** $0
**Current description:** $1

## Instructions

1. Read the file to understand its current purpose and contents
2. Compare the current description against the file's actual role
3. A description is **acceptable** if it:
   - Correctly identifies the file's primary purpose
   - Is not misleading about what the file contains
   - Is still relevant (the file hasn't been substantially rewritten)
4. A description **needs update** if it:
   - Refers to functionality that has been removed or moved
   - Misidentifies the file's purpose
   - Is so vague as to be unhelpful when the file has a clear, specific purpose
   - Describes a secondary concern while missing the primary one

Note: minor wording preferences are NOT grounds for update. Only flag descriptions that are materially wrong or misleading.

## Response Format

```
VERDICT: acceptable | needs-update
CURRENT: <the current description>
SUGGESTED: <replacement description, or same as current if acceptable>
REASON: <one sentence if needs-update, omit if acceptable>
```
