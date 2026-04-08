---
name: guide-search
description: Use a navigation guide to find relevant files and directories for a given topic or task. Returns a prioritized list of places to examine.
context: fork
agent: nav-guide-evaluator
user-invocable: false
allowed-tools: Read Glob Grep
---

Given a search target, use the navigation guide to identify the most relevant files and directories to examine.

**Guide file:** $0
**Search target:** $1

## Instructions

1. Read the navigation guide file
2. For each entry in the guide, assess its relevance to the search target based on:
   - The file/directory path (name matching)
   - The description comment (semantic matching)
   - The position in the hierarchy (structural context)
3. Return a prioritized list of the most relevant entries, ordered by likelihood of containing relevant code

## Response Format

Return up to 10 entries, most relevant first:

```
RESULTS:
1. <path> — <reason for relevance>
2. <path> — <reason for relevance>
...
```

If nothing in the guide seems relevant, respond with:

```
RESULTS: none — <brief suggestion for where else to look>
```
