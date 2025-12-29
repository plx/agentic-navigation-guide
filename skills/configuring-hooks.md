---
name: configuring-hooks
description: Set up automated navigation guide verification with Claude Code hooks, GitHub Actions, or pre-commit hooks.
---

# Configuring Hooks for Navigation Guide Verification

This skill teaches you how to set up automated verification of navigation guides using various hook systems.

## Prerequisites

Install the CLI tool:

```bash
cargo install agentic-navigation-guide
```

Verify installation:

```bash
agentic-navigation-guide --version
```

## Claude Code PostToolUse Hook

The PostToolUse hook automatically verifies your guide after file system changes.

### Using the Plugin (Recommended)

Install the plugin and hooks are configured automatically:

```
/plugin marketplace add https://github.com/plx/agentic-navigation-guide
/plugin install agentic-navigation-guide@agentic-navigation-guide-marketplace
```

### Manual Configuration

Create or edit `hooks/hooks.json` in your project:

```json
{
  "PostToolUse": [
    {
      "matcher": "Write|Edit|Bash",
      "hooks": [
        {
          "type": "command",
          "command": "agentic-navigation-guide verify --post-tool-use-hook"
        }
      ]
    }
  ]
}
```

Or add to your `~/.claude/settings.json` for global use:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit|Bash",
        "hooks": [
          {
            "type": "command",
            "command": "agentic-navigation-guide verify --post-tool-use-hook"
          }
        ]
      }
    ]
  }
}
```

### How It Works

- **Matcher**: `Write|Edit|Bash` triggers on file writes, edits, and bash commands
- **Exit code 2**: The `--post-tool-use-hook` flag causes exit code 2 on failure (Claude Code's hook failure code)
- **Output**: Shows verification result; errors include line numbers

## GitHub Actions

Add a workflow to verify guides on every push and pull request.

### Basic Workflow

Create `.github/workflows/verify-guide.yml`:

```yaml
name: Verify Navigation Guide

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  verify:
    name: Verify Navigation Guide
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Install agentic-navigation-guide
        run: cargo install agentic-navigation-guide

      - name: Verify navigation guide
        run: agentic-navigation-guide verify --github-actions-check
```

### Monorepo Workflow (Recursive)

For projects with multiple guides:

```yaml
name: Verify Navigation Guides

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  verify:
    name: Verify All Navigation Guides
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Install agentic-navigation-guide
        run: cargo install agentic-navigation-guide

      - name: Verify all guides recursively
        run: |
          agentic-navigation-guide verify \
            --recursive \
            --exclude target \
            --exclude node_modules \
            --github-actions-check
```

### Using Environment Variable

Alternative to `--github-actions-check`:

```yaml
- name: Verify navigation guide
  run: agentic-navigation-guide verify
  env:
    AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE: github-actions
```

### GitHub Actions Output

On success:
```
Navigation guide verified
```

On failure:
```
AGENTIC_NAVIGATION_GUIDE.md:15: error: path 'src/deleted.rs' does not exist
```

The `file:line` format integrates with GitHub's annotation system.

## hk (Lefthook)

[hk](https://github.com/evilmartians/lefthook) is a fast Git hooks manager.

### Installation

```bash
# macOS
brew install lefthook

# npm
npm install -g lefthook
```

### Configuration

Create `lefthook.yml` in your project root:

```yaml
pre-commit:
  commands:
    verify-navigation-guide:
      run: agentic-navigation-guide verify --pre-commit-hook
      fail_text: "Navigation guide verification failed"
```

### Enable Hooks

```bash
lefthook install
```

### Pre-push Alternative

To verify only before pushing:

```yaml
pre-push:
  commands:
    verify-navigation-guide:
      run: agentic-navigation-guide verify --pre-commit-hook
```

## Generic Pre-commit Hook

For simple Git hooks without a framework.

### Shell Script

Create `.git/hooks/pre-commit`:

```bash
#!/bin/sh

# Verify navigation guide before commit
if ! agentic-navigation-guide verify --pre-commit-hook; then
    echo ""
    echo "Navigation guide verification failed."
    echo "Please update AGENTIC_NAVIGATION_GUIDE.md to match filesystem."
    echo ""
    echo "Run 'agentic-navigation-guide verify' for details."
    exit 1
fi
```

Make it executable:

```bash
chmod +x .git/hooks/pre-commit
```

### With pre-commit Framework

If using [pre-commit](https://pre-commit.com/), add to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: verify-navigation-guide
        name: Verify Navigation Guide
        entry: agentic-navigation-guide verify --pre-commit-hook
        language: system
        pass_filenames: false
        always_run: true
```

Then install:

```bash
pre-commit install
```

## Execution Modes Comparison

| Flag | Exit Code on Failure | Best For |
|------|---------------------|----------|
| (none) | 1 | Manual CLI use |
| `--post-tool-use-hook` | 2 | Claude Code hooks |
| `--pre-commit-hook` | 1 | Git pre-commit hooks |
| `--github-actions-check` | 1 | CI/CD pipelines |

## Environment Variables

Configure defaults via environment:

| Variable | Values | Purpose |
|----------|--------|---------|
| `AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE` | `default`, `post-tool-use`, `pre-commit-hook`, `github-actions` | Set execution mode |
| `AGENTIC_NAVIGATION_GUIDE_LOG_MODE` | `quiet`, `default`, `verbose` | Control output verbosity |
| `AGENTIC_NAVIGATION_GUIDE_PATH` | Path | Default guide file path |
| `AGENTIC_NAVIGATION_GUIDE_ROOT` | Path | Default root directory |

## Troubleshooting

### Hook Not Running

- **hk/lefthook**: Run `lefthook install` after changing config
- **Git hooks**: Check file is executable (`chmod +x`)
- **pre-commit**: Run `pre-commit install` after config changes

### Command Not Found

Ensure `agentic-navigation-guide` is in PATH:

```bash
# Check installation
which agentic-navigation-guide

# If using cargo install, ensure ~/.cargo/bin is in PATH
export PATH="$HOME/.cargo/bin:$PATH"
```

### False Positives in CI

If CI fails but local passes:
- Check for OS-specific path issues (Windows vs Unix)
- Ensure `.gitignore` patterns match exclude patterns
- Verify the checkout includes all expected files
