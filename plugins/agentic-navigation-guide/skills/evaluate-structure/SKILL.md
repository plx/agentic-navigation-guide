---
name: evaluate-structure
description: Evaluate a project's size and structure to recommend flat vs nested navigation guide strategy, including depth and nesting decisions.
context: fork
agent: nav-guide-evaluator
user-invocable: false
allowed-tools: Read Glob Grep Bash
---

Evaluate the project structure and recommend a navigation guide strategy.

**Project root:** $ARGUMENTS

## Instructions

1. Count total files and directories (excluding `.git/`, `node_modules/`, `target/`, `build/`, `dist/`, `__pycache__/`)
2. Examine the directory tree to depth 3
3. Assess complexity and recommend a strategy:

### Flat (Single Guide)

Recommend when:
- Fewer than ~100 source files
- Shallow directory structure (1-2 levels of nesting)
- Single-language project with straightforward layout
- No independently-navigable subprojects

### Nested (Multiple Guides)

Recommend when:
- Monorepo with distinct subprojects
- More than ~200 source files across multiple deep subdirectories
- Independently-deployable services or packages
- Subdirectories that have their own build systems or configs

For nested strategies, identify which subdirectories should get their own `AGENTIC_NAVIGATION_GUIDE.md`.

4. Recommend depth for the root guide:
   - Depth 1: only top-level items (for very large projects or monorepos)
   - Depth 2: top-level + one level of children (most common)
   - Depth 3: deeper nesting (small to medium projects with rich structure)

## Response Format

```
STRATEGY: flat | nested
ROOT_DEPTH: 1 | 2 | 3
NESTED_GUIDES: <comma-separated list of subdirectory paths, or "none">
TOTAL_FILES: <approximate count>
REASON: <1-2 sentences explaining the recommendation>
```
