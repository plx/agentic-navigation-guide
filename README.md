# Overview of `agentic-navigation-guide`

[![CI](https://github.com/plx/agentic-navigation-guide/workflows/CI/badge.svg)](https://github.com/plx/agentic-navigation-guide/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/agentic-navigation-guide.svg)](https://crates.io/crates/agentic-navigation-guide)
[![Documentation](https://docs.rs/agentic-navigation-guide/badge.svg)](https://docs.rs/agentic-navigation-guide)
[![License](https://img.shields.io/crates/l/agentic-navigation-guide.svg)](https://github.com/plx/agentic-navigation-guide/blob/main/LICENSE)

Coding assistants often have trouble navigating large repositories (...and often burn a lot of time and tokens trying to locate files).
One way to mitigate this difficulty is to include (partial) file listings inside the assistant's memory files, but these listings can be challenging to maintain over time. Worse, once they become outdated, their presence is more harmful than helpful.

This crate provides a CLI tool to assist with both:

- hand-authoring helpful navigation guides
- validating them against the current state of the filesystem

The validation can be done in a stand-alone way, and also has special support for being used as a "post-tool-use-hook" by Claude Code.

## Navigation Guide Format

A "navigation guide" looks like this:

```
<agentic-navigation-guide>
- src/
  - main.rs # Main entry point
  - lib.rs # Core logic goes here
  - types.rs # Core data types
  - errors.rs # errors and error messages
  - parser.rs # Parse guides from markdown
  - cli/
    - init.rs # init subcommand
    - dump.rs # dump subcommand
    - verify.rs # verify subcommand
- Cargo.toml
- README.md
</agentic-navigation-guide>
```

The main rules are:

- directories must end with a `/`
- nesting is indicated by indentation
- comments are optional, and must be separated from the path by a `#` character
- blank lines are not allowed within the guide block
- no special paths (`.`, `..`, `./`, `../`)
- no ordering requirement is imposed
- placeholder entries (`...`) can be used to indicate unlisted items (see below)

Note that it's *not* an error to omit files and directories from the guide, but it *is* an error to include incorrect entries—the guide *must* be accurate*.

### Placeholder Entries

You can use `...` as a placeholder to indicate that there are additional files or directories not explicitly listed:

```
<agentic-navigation-guide>
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
<agentic-navigation-guide>
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

## Suggested Usage

To use this tool, I would suggest you do this:

- put your navigation guide in a file named `AGENTIC_NAVIGATION_GUIDE.md` in the root of your project
- use the `@` syntax to include it in your `CLAUDE.md` file (etc.)

For a fuller example, you can review the [`CLAUDE.md`](./CLAUDE.md) file within this repository. 

The advantage of this workflow is it keeps your navigation guide content physically-isolated from your CLAUDE.md (etc.)—helpful for editing and reviewing!—while still bringing the guide into context for each session.

## Tool Overview

The tool provides the following commands:

- `init`: initialize a new navigation guide file with the current directory structure
- `check`: check that the contents of a hand-written navigation guide are *syntactically* correct (i.e. adhere to the format specified above)
- `verify`: verify that the contents of a hand-written navigation guide accurately reflect the current state of the file system
- `dump`: dump the current directory contents in the intended markdown format

If you're adding a navigation guide to your repository, I'd suggest:

- run `agentic-navigation-guide init` to generate a starting point
- hand-edit the file to add comments and omit extraneous details
- run `agentic-navigation-guide verify` to check for errors
- commit the file to your repository
- update your CLAUDE.md (etc.) to include the guide using the `@` syntax

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

## Future Roadmap

This is an early preview of the tool, so there are a few rough edges. Potential future steps:

- [ ] support for auto-installing the hook (e.g. auto-editing your settings to include it)
- [ ] support for auto-generating the hook (e.g. suggested prompts/commands to have your agent write the guide comments)
- [ ] support for nested guides 
- [ ] inspecting the post-tool-use-hook json and skipping unnecessary work

Note that the tool is already configurable-enough it can be used with nested guides if invoked with the right arguments—the part that's missing is good ergonomics to make it work automagically.
