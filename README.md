# Overview of `agentic-navigation-guide`

[![CI](https://github.com/plx/agentic-navigation-guide/workflows/CI/badge.svg)](https://github.com/plx/agentic-navigation-guide/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/agentic-navigation-guide.svg)](https://crates.io/crates/agentic-navigation-guide)
[![Documentation](https://docs.rs/agentic-navigation-guide/badge.svg)](https://docs.rs/agentic-navigation-guide)
[![License](https://img.shields.io/crates/l/agentic-navigation-guide.svg)](#license)

Coding assistants often have trouble navigating large repositories (...and often burn a lot of time and tokens trying to locate files).
One way to mitigate this difficulty is to include (partial) file listings inside the assistant's memory files, but these listings can be challenging to maintain over time. Worse, once they become outdated, their presence is more harmful than helpful.

This crate provides a CLI tool to assist with both:

- hand-authoring helpful navigation guides
- validating them against the current state of the filesystem

The validation can be done in a stand-alone way, and also has special support for being used as a "post-tool-use-hook" by Claude Code.

## Support Boundary

The installed CLI is the sole supported v0.2 product. The v0.2 package will be
binary-only and will not expose a supported Rust library facade. The current
`0.1.4` library target is a legacy migration surface scheduled for removal at
the `0.2.0` boundary; new Rust integrations should not depend on it. The
complete decision, export disposition, and `0.2.x` compatibility policy are in
the [normative contract](docs/v0.2-contract.md#supported-product-and-rust-api).
Existing Rust consumers have no v0.2 in-process shim; they must migrate to the
documented CLI process contract or remain pinned to unsupported `0.1.4` at
their own risk. The docs.rs badge above still describes that legacy published
library; #66 must remove or retarget it and set maintained documentation
metadata before the binary-only release.

## Docs and Implementation Alignment Policy

### Source-of-Truth Precedence

1. [`docs/v0.2-contract.md`](docs/v0.2-contract.md) is the normative target
   for the v0.2 guide language, filesystem representation, and
   stable-filesystem trust and supported-product boundaries.
2. Current implementation (`src/` plus tests) is authoritative for realized
   `0.1.4` runtime behavior while the contract's explicitly owned v0.2
   divergences are being implemented.
3. `README.md` is the concise user entry point and should match released
   behavior.
4. `Specification.md` captures original intent and historical context. It is
   not normative.

An unrecorded contradiction is a defect. The staged conformance ledger keeps
known v0.2 differences explicit until their focused implementation issues
land.

### Update Process for Behavior Changes

When user-facing behavior changes, update user-facing docs in the same change. This includes:
- CLI commands, flags, defaults, and environment-variable behavior
- output format, error messaging, and exit-code behavior
- syntax and validation behavior for navigation guides

### Known Intentional Divergences

- **2026-07-25 — Legacy library target remains during staged cleanup.** The
  current source still builds a linkable library so #52–#54 can remove and
  privatize the audited current-source surface in focused changes. The
  immutable published `0.1.4` artifact is #64's separate migration baseline.
  Neither library is part of the supported v0.2 product.

## Navigation Guide Format

The full, versioned v0.2 format and filesystem contract is
[`docs/v0.2-contract.md`](docs/v0.2-contract.md). The summary below tracks the
currently implemented behavior as focused v0.2 conformance work lands; the
contract's staged conformance ledger identifies the remaining known
differences until #66's final documentation pass.

A "navigation guide" looks like this:

```
<agentic-navigation-guide ignore=true>
- src/
  - main.rs # Main entry point
  - lib.rs # Core logic goes here
  - types.rs # Core data types
  - errors.rs # errors and error messages
  - parser.rs # Parse guides from markdown
  - cli/
    - init.rs # init subcommand
    - dump.rs # dump subcommand
    - output.rs # shared exclusive filesystem-output sink
    - verify.rs # verify subcommand
- Cargo.toml
- README.md
</agentic-navigation-guide>
```

The main rules are:

- each entry must be a list item (start with `-`)
- the first entry is unindented; nesting then uses one inferred space-only
  indentation unit of 1–16 spaces, may increase by one level only when the
  immediately preceding physical line represents exactly one directory, and
  is limited to depth 256
- a trailing `/` marks a directory entry; without the trailing slash, an entry is parsed as a regular-file path
- comments are optional; the first unescaped `#` starts the comment portion
- use `\#` to include a literal `#` character in a path
- if an entry has no unescaped `#`, everything after `-` is part of the path (for example, `- src/ source code` is a literal path, not a comment)
- blank lines are not allowed within the guide block
- paths are validated structurally (must be relative, cannot contain empty components like `//`, and cannot contain `.` or `..` components)
- no ordering requirement is imposed
- placeholder entries (`...`) can be used to indicate unlisted items (see below)

Note that it's *not* an error to omit files and directories from the guide, but it *is* an error to include incorrect entries—the guide *must* be accurate*.

### UTF-8 Scope

- UTF-8 paths are supported, including non-ASCII names.
- Non-UTF-8 filesystem names are explicitly out of scope.
- Commands that enumerate filesystem entries (for example `dump`, or placeholder checks during `verify`) will return an error if they encounter non-UTF-8 names.

### Placeholder Entries

You can use `...` as a placeholder to indicate that there are additional files or directories not explicitly listed:

```
<agentic-navigation-guide ignore=true>
- src/
  - main.rs # Entry point
  - ... # Other source files
- docs/
  - README.md
  - api.md
  - ... # Additional documentation
</agentic-navigation-guide>
```

Rules for placeholders:
- Written as `...` (three dots)
- May have an optional comment after it
- Cannot have child elements nested under them
- **With a comment**: Allowed in any directory, even if all items are listed or the directory is empty (useful for indicating future items)
- **Without a comment**: Must refer to at least one unlisted item in the parent directory (useful for omitting existing items)
- Cannot be adjacent to another `...` entry (must have at least one non-placeholder between them)

The distinction between commented and uncommented placeholders enables two important use cases:

```
<agentic-navigation-guide ignore=true>
- src/
  - main.rs
  - ... # Represents lib.rs, utils.rs, etc. that exist but aren't listed
- plans/
  - phases/
    - phase-01-scaffolding.md # Phase 1 - COMPLETED
    - ... # Plans for future phases will appear here
</agentic-navigation-guide>
```

In this example:
- The first `...` in `src/` has a comment and there ARE unmentioned files (lib.rs, utils.rs) - represents omitted existing items
- The second `...` in `phases/` has a comment but phase-01-scaffolding.md is the ONLY file - represents future items that don't exist yet

### Limited Choice Expansions

To keep related paths together while avoiding duplication, a single guide entry may include a *choice list* written with square
brackets. For example:

```
- FooCoordinator[.h, .cpp] # Coordinates foo interactions
```

is equivalent to writing:

```
- FooCoordinator.h # Coordinates foo interactions
- FooCoordinator.cpp # Coordinates foo interactions
```

Each entry may contain at most one choice list and it expands into one concrete item for every option in the brackets. The same
comment is attached to every expanded item.

Choice lists follow these rules:

- Whitespace inside the brackets is ignored unless it appears inside a quoted string.
- An empty string may be included by leaving an empty slot (e.g. `[, .local]`).
- Use a backslash to escape individual characters (e.g. `\,` for a literal comma, `\ ` for a literal space, `\#` for a literal `#`, `\[` for a literal
  `[` character).
- Surround complex values with double quotes to preserve punctuation or embedded brackets. Within quotes, escape `"` to include
  a literal quote character.

**Examples:**

```markdown
- FooCoordinator[.h, .cpp]        # expands to FooCoordinator.h and FooCoordinator.cpp
- Config[, .local].json           # expands to Config.json and Config.local.json
- src[/main, /lib].rs             # expands to src/main.rs and src/lib.rs
```

These expansions are intended for small sets of closely related alternatives—typically filename suffixes or prefixes—so that
the guide stays concise without sacrificing clarity.

### Ignoring Guides

You can mark a navigation guide to be ignored during verification by adding an `ignore` attribute to the opening tag:

```markdown
<agentic-navigation-guide ignore=true>
- example/
  - file.rs
</agentic-navigation-guide>
```

This is particularly useful for:
- **Documentation examples**: Example guides in README files that should not be validated
- **Invalid examples**: Intentionally incorrect guides used to demonstrate error cases
- **Template files**: Guide templates that may not match the current filesystem

The opening marker is exact: it may be bare, or contain exactly one
`ignore=true` or `ignore="true"` attribute. Spaces or tabs may surround `=`,
and the attributed form requires at least one space or tab after the marker
name. Unknown, duplicate, concatenated, false-valued, single-quoted, and
malformed attributes are rejected. The closing marker accepts no attributes.
Only spaces and tabs may surround either complete marker on its line.

LF and CRLF line endings are equivalent; a lone carriage return is rejected.
After permitted outer spaces or tabs are removed, marker-like lines that begin
with the exact opening or closing prefix are validated everywhere in the
document, including the prologue, guide body, and epilogue.

In `0.1.4`, the body must still pass the parser's list grammar before the
ignored result is available. After that parse, the tool will:
- Skip validator and filesystem verification
- Emit a warning that the guide was skipped
- Provide an additional note if the ignored guide is in a standalone `AGENTIC_NAVIGATION_GUIDE.md` file

The v0.2 target makes the ignored body opaque and adds an explicit deny policy;
see the [normative contract](docs/v0.2-contract.md#ignored-guides).

## Suggested Usage

To use this tool, I would suggest you do this:

- put your navigation guide in a file named `AGENTIC_NAVIGATION_GUIDE.md` in the root of your project
- use the `@` syntax to include it in your `CLAUDE.md` file (etc.)

For a fuller example, you can review the [`CLAUDE.md`](./CLAUDE.md) file within this repository. 

The advantage of this workflow is it keeps your navigation guide content physically-isolated from your CLAUDE.md (etc.)—helpful for editing and reviewing!—while still bringing the guide into context for each session.

## Tool Overview

The tool provides the following commands:

- `init --output <path>`: initialize a new navigation guide file with the current directory structure
- `check`: check that the contents of a hand-written navigation guide are *syntactically* correct (i.e. adhere to the format specified above)
- `verify`: verify that the contents of a hand-written navigation guide accurately reflect the current state of the file system
- `dump [--output <path>]`: dump the current directory contents in the intended markdown format, to stdout by default

If you're adding a navigation guide to your repository, I'd suggest:

- run `agentic-navigation-guide init --output AGENTIC_NAVIGATION_GUIDE.md` to generate a starting point
- hand-edit the file to add comments and omit extraneous details
- run `agentic-navigation-guide verify` to check for errors
- commit the file to your repository
- update your CLAUDE.md (etc.) to include the guide using the `@` syntax

### Filesystem Output Safety

`init --output` and `dump --output` share the same create-new policy. The
destination name must be absent: the commands never follow or replace an
existing regular file, hard-link name, directory, symbolic link (including a
dangling link), Windows reparse entry, FIFO, socket, device, or other special
entry. There is no force or overwrite mode.

The destination parent must already exist and be writable; the commands do not
create parent directories. Below the selected generation-root spelling, every
descendant ancestor must be a real directory rather than a link or reparse
point. A path explicitly selected outside that spelling may use a stable
resolved link ancestor because `--output` grants authority to that external
parent. These checks assume a stable filesystem while the command runs and are
not a sandbox against hostile concurrent ancestor, name, or identity
replacement—including replacement between a cleanup identity check and
removal.

The complete guide is generated in memory before the final name is created.
One exclusive create selects the owner, the complete buffer is written and
flushed, and the file data is synchronized before success. If delivery fails,
the command removes its artifact only after verifying that the entry still has
the identity it created; a cleanup failure is reported with the caller-selected
residual path. An observed identity mismatch is never removed. A competing
creator is never overwritten.

Exclusive creation makes ownership of the name atomic, not content
publication. A concurrent reader may observe an in-progress prefix before the
command succeeds. The parent directory is not synchronized, so no crash
durability across an immediate power or kernel failure is promised.

On Unix and macOS, final entries and in-root descendants are inspected without
following links and creation uses exclusive, no-follow semantics. On Windows,
link-like reparse descendants, alternate data streams, device and named-pipe
namespaces, reserved DOS device aliases, and unsupported verbatim prefixes are
rejected; a newly created handle must be a regular non-reparse disk file.

## Post-Tool-Use Hook

To set it up as a post-tool-use-hook, you can update your `~/.claude/settings.json` file to include the following:

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

## GitHub Actions Integration

To use the tool as a CI check in GitHub Actions, add a job to your workflow:

```yaml
verify-navigation-guide:
  name: Verify Navigation Guide
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: actions-rust-lang/setup-rust-toolchain@v1
    - name: Install agentic-navigation-guide
      run: cargo install agentic-navigation-guide
    - name: Verify installation
      run: agentic-navigation-guide --version
    - name: Verify navigation guide
      run: agentic-navigation-guide verify --github-actions-check
```

The `--github-actions-check` flag provides:
- Concise output on success ("✓ Navigation guide verified")
- Detailed error messages with file:line references
- Exit code 1 on failure (standard for CI checks)
- Visual indicators (emoji) for quick scanning

You can also set the execution mode via environment variable:
```yaml
- name: Verify navigation guide
  run: agentic-navigation-guide verify
  env:
    AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE: github-actions
```

## Recursive Verification for Monorepos

For monorepos or projects with nested navigation guides, you can use the `--recursive` flag to automatically discover and verify all guide files:

```bash
# Recursively verify all AGENTIC_NAVIGATION_GUIDE.md files
agentic-navigation-guide verify --recursive

# Use a custom guide name (e.g., GUIDE.md)
agentic-navigation-guide verify --recursive --guide-name GUIDE.md

# Exclude directories from the search
agentic-navigation-guide verify --recursive --exclude target --exclude node_modules
```

### Example Monorepo Structure

```
AGENTIC_NAVIGATION_GUIDE.md         # Root-level guide
CLAUDE.md
/backend/
  AGENTIC_NAVIGATION_GUIDE.md       # Backend guide (verified relative to /backend/)
  CLAUDE.md
  /services/
    /sso/
      AGENTIC_NAVIGATION_GUIDE.md   # SSO service guide (verified relative to /backend/services/sso/)
      CLAUDE.md
    /taskrunner/
      AGENTIC_NAVIGATION_GUIDE.md   # Taskrunner guide (verified relative to /backend/services/taskrunner/)
      CLAUDE.md
/frontend/
  AGENTIC_NAVIGATION_GUIDE.md       # Frontend guide (verified relative to /frontend/)
  CLAUDE.md
  /consumer/
    AGENTIC_NAVIGATION_GUIDE.md     # Consumer app guide (verified relative to /frontend/consumer/)
    CLAUDE.md
  /internal/
    AGENTIC_NAVIGATION_GUIDE.md     # Internal app guide (verified relative to /frontend/internal/)
    CLAUDE.md
```

Each guide is verified relative to its parent directory, allowing you to maintain focused navigation guides for different parts of your codebase.

### Recursive Verification Features

- **Automatic Discovery**: Finds all guide files matching the specified name throughout the directory tree
- **Relative Verification**: Each guide is verified against its parent directory as the root
- **Root Boundary Enforcement**: On a stable filesystem, listed item paths that resolve outside the guide root are rejected; this consistency check is not a filesystem sandbox
- **Custom Names**: Support for uniform custom guide filenames (e.g., `--guide-name GUIDE.md`)
- **Exclusion Patterns**: Skip directories like `target`, `node_modules`, `.git` using glob patterns
- **Aggregated Results**: Shows summary of all verified guides with pass/fail counts
- **Execution Modes**: Works with all execution modes (default, post-tool-use, pre-commit-hook, GitHub Actions)

## Future Roadmap

This is an early preview of the tool, so there are a few rough edges. Potential future steps:

- [ ] support for auto-installing the hook (e.g. auto-editing your settings to include it)
- [ ] support for auto-generating the hook (e.g. suggested prompts/commands to have your agent write the guide comments)
- [x] support for nested guides (completed - use `--recursive` flag)
- [ ] inspecting the post-tool-use-hook json and skipping unnecessary work

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Third-party dependencies bundled into release builds carry their own permissive licenses; see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) for the full set of attributions. That file is regenerated from the dependency tree by [`cargo-about`](https://github.com/EmbarkStudios/cargo-about) — running `cargo licenses` (a shorthand for `cargo about generate about.hbs --output-file THIRD_PARTY_LICENSES.md`) rewrites it locally, and CI runs the same command on every push and pull request and fails the build if the committed file does not match.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
