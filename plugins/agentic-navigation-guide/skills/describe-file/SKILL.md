---
name: describe-file
description: Read a file and produce a terse description suitable for a navigation guide comment (under 60 chars).
context: fork
agent: nav-guide-evaluator
user-invocable: false
allowed-tools: Read Glob Grep
---

Generate a terse description for the following file, suitable for use as a navigation guide comment.

**File to describe:** $ARGUMENTS

## Instructions

1. Read the file to understand its primary purpose
2. Write a description that:
   - Is under 60 characters
   - Captures the file's primary purpose, not implementation details
   - Uses sentence fragments, not full sentences (no period at end)
   - Avoids redundancy with the filename (don't restate what the name already says)
   - Is specific enough to help decide whether to read the file
   - Uses active voice when possible

## Examples of Good Descriptions

- `# OAuth2 token refresh and session management`
- `# CLI argument parsing and subcommand dispatch`
- `# Templated reusable-resource pool`
- `# Error types and conversion impls`
- `# Webpack config for production builds`

## Examples of Bad Descriptions

- `# This file contains utility functions` (too vague)
- `# Main` (too terse, restates filename)
- `# Handles various authentication-related operations including login, logout, and token management` (too long)
- `# Helper file` (meaningless)

## Response Format

Respond with exactly one line:

```
DESCRIPTION: <the description, without the leading #>
```
