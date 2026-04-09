---
name: audit-dir-description
description: Check if a directory's navigation guide description is still accurate. Returns acceptable/needs-update with suggested replacement.
context: fork
agent: nav-guide-evaluator
user-invocable: false
allowed-tools: Read Glob Grep Bash
---

Evaluate whether an existing navigation guide description for a directory is still accurate.

**Directory path:** $0
**Current description:** $1

## Instructions

1. List the directory contents and sample a few files to understand its current purpose
2. Compare the current description against the directory's actual role
3. A description is **acceptable** if it correctly characterizes the directory's organizational purpose
4. A description **needs update** if the directory's contents have shifted significantly from what the description implies

Minor wording preferences are NOT grounds for update. Only flag descriptions that are materially wrong or misleading.

## Response Format

```
VERDICT: acceptable | needs-update
CURRENT: <the current description>
SUGGESTED: <replacement description, or same as current if acceptable>
REASON: <one sentence if needs-update, omit if acceptable>
```
