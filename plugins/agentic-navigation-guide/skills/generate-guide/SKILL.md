---
name: generate-guide
description: Generate an initial navigation guide for a project that doesn't have one. Analyzes project structure, evaluates files for inclusion, and writes descriptions.
context: fork
agent: nav-guide-worker
disable-model-invocation: true
argument-hint: [project-root-path]
allowed-tools: Read Glob Grep Bash Write Edit Agent
---

Generate an agentic navigation guide for a project.

**Project root:** $ARGUMENTS (defaults to current working directory if not specified)

## Workflow

### Phase 1: Survey

1. Check if an `AGENTIC_NAVIGATION_GUIDE.md` already exists. If so, warn the user and ask before overwriting.
2. Run `agentic-navigation-guide dump --depth 3 --exclude .git --exclude node_modules --exclude target --exclude build --exclude dist --exclude __pycache__ --exclude .venv` to get the raw directory structure.
3. Count the total number of source files to gauge project size.

### Phase 2: Strategy

4. Evaluate the project structure to decide between a flat or nested guide strategy. Consider:
   - Total file count and directory depth
   - Whether there are distinct subprojects (monorepo indicators: multiple `package.json`, `Cargo.toml`, `go.mod`, etc.)
   - Whether any subdirectory is large enough to warrant its own guide

5. For the chosen strategy, determine:
   - **Flat**: What depth to include, and which top-level items to list
   - **Nested**: Which directories get their own guides, what depth at each level

### Phase 3: Evaluate Candidates

6. For each directory at the target depth, decide whether to include it. Prioritize directories that:
   - Contain source code (not build artifacts, caches, or vendored dependencies)
   - Group related functionality in a non-obvious way
   - Would help an AI assistant navigate the project

7. For each file at the target depth within included directories, decide whether to include it. Prioritize files that:
   - Define core types, interfaces, or entry points
   - Would be hard to find by name alone
   - Are critical for understanding architecture

8. For file sets (e.g., `.h/.cpp` pairs, `index.ts` + co-located files), evaluate as units and use choice expansion syntax where appropriate (e.g., `FooController[.h, .cpp]`).

### Phase 4: Generate Descriptions

9. For each included item, read it and write a terse description (under 60 characters). Descriptions should:
   - Capture primary purpose, not implementation details
   - Avoid redundancy with the filename
   - Use consistent style across the guide

### Phase 5: Assemble and Validate

10. Assemble the guide in proper format:
    - Wrap in `<agentic-navigation-guide>` tags
    - Use consistent indentation for nesting (2 spaces recommended)
    - Add `- ... # Additional project files` at appropriate levels
    - Ensure directories end with `/`

11. Write to `AGENTIC_NAVIGATION_GUIDE.md` (or the nested guide location).

12. Run `agentic-navigation-guide verify --guide <guide-path> --root <guide-parent-directory>` to validate, substituting the actual output path from step 11 and its parent directory. Fix any errors.

13. Report a summary: how many items included, strategy chosen, and any notable exclusions.

## Guidelines

- Aim for 10-30 entries in a single guide. Under 10 is too sparse; over 50 is too noisy.
- When in doubt, exclude. A smaller, accurate guide is better than a large, stale one.
- Match the description style to any existing guides in the project.
- For monorepos, generate the root guide first, then offer to generate nested guides.
