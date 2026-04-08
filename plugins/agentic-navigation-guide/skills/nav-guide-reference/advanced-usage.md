# Advanced Usage

## Environment Variables

| Variable | Purpose | Values |
|----------|---------|--------|
| `AGENTIC_NAVIGATION_GUIDE_LOG_MODE` | Output verbosity | `quiet`, `verbose`, `default` |
| `AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE` | Behavior mode | `post-tool-use`, `pre-commit-hook`, `github-actions`, `default` |
| `AGENTIC_NAVIGATION_GUIDE_PATH` | Default guide file path | Any file path |
| `AGENTIC_NAVIGATION_GUIDE_ROOT` | Default root directory | Any directory path |
| `AGENTIC_NAVIGATION_GUIDE_NAME` | Guide filename for recursive mode | e.g., `GUIDE.md` |

## Execution Modes

- **default**: Standard output, exit code 1 on failure
- **post-tool-use**: Exit code 2 on failure (signals Claude Code hook system to show error to assistant)
- **pre-commit-hook**: Exit code 1 on failure (blocks commit)
- **github-actions**: Concise output with file:line references, emoji indicators

Set via `--post-tool-use-hook`, `--github-actions-check` flags, or `AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE`.

## Recursive Verification (Monorepos)

```bash
# Discover and verify all AGENTIC_NAVIGATION_GUIDE.md files
agentic-navigation-guide verify --recursive

# Custom guide filename
agentic-navigation-guide verify --recursive --guide-name GUIDE.md

# Exclude directories
agentic-navigation-guide verify --recursive --exclude target --exclude node_modules
```

Each guide is verified relative to its parent directory.

## Choice Expansions

Combine related file variants on a single line:

```
- FooController[.h, .cpp] # Manages foo lifecycle
- Config[, .local].json   # App configuration
```

Expands to individual entries sharing the same description. At most one choice list per entry.

Escaping: `\,` for literal comma, `\ ` for literal space, `\"` inside quoted values.

## Ignoring Guides

Add `ignore=true` to skip a guide during verification:

```
<agentic-navigation-guide ignore=true>
- example/
  - demo.rs
</agentic-navigation-guide>
```

Useful for documentation examples that shouldn't be validated.

## Placeholder Rules

- `...` with comment: Allowed anywhere, even if all items are listed (indicates future items)
- `...` without comment: Must have at least one unlisted item in parent directory
- Cannot nest items under `...`
- Cannot have adjacent `...` entries
