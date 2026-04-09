---
name: describe-dir
description: Read a directory and produce a terse description suitable for a navigation guide comment (under 60 chars).
context: fork
agent: nav-guide-evaluator
user-invocable: false
allowed-tools: Read Glob Grep Bash
---

Generate a terse description for the following directory, suitable for use as a navigation guide comment.

**Directory to describe:** $ARGUMENTS

## Instructions

1. List the directory contents and read a few representative files to understand its purpose
2. Write a description that:
   - Is under 60 characters
   - Captures the directory's organizational purpose
   - Uses sentence fragments (no trailing period)
   - Avoids redundancy with the directory name
   - Distinguishes this directory from siblings with similar names

## Examples of Good Descriptions

- `# REST API route handlers`
- `# Database migration scripts`
- `# Shared React UI components`
- `# Integration test suites`
- `# gRPC service definitions and generated stubs`

## Examples of Bad Descriptions

- `# Source code` (too vague for `src/`)
- `# Tests` (restates the directory name `tests/`)
- `# Contains the components used in the frontend application` (too wordy)

## Response Format

Respond with exactly one line:

```
DESCRIPTION: <the description, without the leading #>
```
