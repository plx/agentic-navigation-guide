---
name: should-include-file
description: Decide whether a file (or file set like .cc/.h pair) should be included in a navigation guide. Returns a structured include/exclude verdict.
context: fork
agent: nav-guide-evaluator
user-invocable: false
allowed-tools: Read Glob Grep
---

Evaluate whether the following file(s) should be included in a navigation guide.

**File(s) to evaluate:** $ARGUMENTS

## Instructions

1. Read the file(s) to understand their purpose and content
2. Assess navigational value using these criteria:

**Include if the file:**
- Defines core abstractions, types, or interfaces
- Is an entry point (main, index, app bootstrap)
- Contains important configuration or constants
- Defines public APIs or module boundaries
- Would be non-obvious to find by name alone
- Is critical for understanding project architecture

**Exclude if the file:**
- Is auto-generated (lock files, compiled output, bundled assets)
- Contains only test fixtures, sample data, or mocks
- Is standard boilerplate that follows framework conventions (e.g., `__init__.py` in every Python package)
- Is an IDE/editor config file
- Is a build artifact or cache file
- Has a name that fully explains its purpose with no additional context needed

3. For file sets (e.g., `.h/.cpp` pairs): evaluate as a unit. If one file in the set has navigational value, include the set.

## Response Format

Respond with exactly this structure:

```
VERDICT: include | exclude
REASON: <one sentence explaining why>
```
