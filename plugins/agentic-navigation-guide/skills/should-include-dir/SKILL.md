---
name: should-include-dir
description: Decide whether a directory should be included in a navigation guide. Returns a structured include/exclude verdict.
context: fork
agent: nav-guide-evaluator
user-invocable: false
allowed-tools: Read Glob Grep Bash
---

Evaluate whether the following directory should be included in a navigation guide.

**Directory to evaluate:** $ARGUMENTS

## Instructions

1. List the directory contents (immediate children)
2. Assess navigational value using these criteria:

**Include if the directory:**
- Contains source code central to the project
- Groups related functionality (e.g., `src/auth/`, `lib/parsers/`)
- Contains important configuration or infrastructure code
- Would help an AI assistant understand project organization
- Has non-obvious contents that its name doesn't fully explain

**Exclude if the directory:**
- Is a build output directory (`target/`, `dist/`, `build/`, `node_modules/`)
- Contains only auto-generated files
- Is a cache or temporary directory (`.cache/`, `tmp/`)
- Is a hidden directory with standard tooling config (`.git/`, `.vscode/`)
- Contains only test fixtures or sample data with no architectural significance
- Is empty or near-empty with no navigational value

3. If including, also note whether the directory warrants its own entries (children listed in the guide) or just a top-level mention with a description.

## Response Format

```
VERDICT: include | exclude
DEPTH: top-level-only | with-children
REASON: <one sentence explaining why>
```
