# Agentic Navigation Guide

This is a specification for the crate we'll be writing.

## Background: The Problem

Agentic coding assistants often struggle to navigate larger codebases, and often burn time *and* tokens trying to locate files they intend to edit. This is especially true for large codebases with complex directory structures, but can arise even in small codebases with simple directory structures.

Providing the agent with a hand-written "navigation guide" can help, but it can be challenging to maintain over time—it's all too easy to forget to update the guide as files and directories get added, moved, renamed, and removed...and, unfortunately, giving an agent an inaccurate navigation guide can leave it worse off than if you'd given it nothing.

## Background: The Solution

The code we'll be writing in this crate will alleviate the challenge of maintaining a hand-written navigation guide by providing programmatic verification of such hand-written guides vis-a-vis the current state of the file system. 

The core capability will be a CLI tool that verifies that the contents of a hand-written navigation guide accurately reflect the current state of the file system; note that the hand-written navigation guide need not be a *complete* description—it's ok if it *omits* files and directories—but what it does include *must* be *accurate*.

## Component: `AGENTIC_NAVIGATION_GUIDE.md`

The navigation guide *file* must be written in markdown. Its default name is `AGENTIC_NAVIGATION_GUIDE.md`, and it's assumed to be located at the "project root", but both the name and location are configurable; in what follows we'll refer to this file as an `AGENTIC_NAVIGATION_GUIDE.md` file.

The contents of an `AGENTIC_NAVIGATION_GUIDE.md` file must adhere to the following structure:

```markdown
$optional prologue content
<agentic-navigation-guide>
$unordered list of filesystem items (see below)
</agentic-navigation-guide>
$optional epilogue content
```

Thus, for example, a minimal `AGENTIC_NAVIGATION_GUIDE.md` file might look like this:

```markdown
<agentic-navigation-guide>
- src/
  - main.rs
- Cargo.toml
- README.md
</agentic-navigation-guide>
```

...but it could also look like this:

```markdown
# Agentic Navigation Guide

This is a navigation guide for agentic coding agents.
The listing below is incomplete, and highlights the key files and directories.

<agentic-navigation-guide>
- src/
  - main.rs
- Cargo.toml
- README.md
</agentic-navigation-guide>

Note that build files are located in `target/`, which is omitted from the above.
```

### Details: Format Specifics

Here are the rules of the above format, spelled out in much fuller detail:

- the navigation guide document must be written in markdown
- the navigation guide *must* include the "sentinel" markers `<agentic-navigation-guide>` and `</agentic-navigation-guide>`
- there must be only a single `agentic-navigation-guide` block within the document
- the contents of the `agentic-navigation-guide` must:
  - be a single unordered list with no other content 
  - must not include any blank lines
  - each item in the unordered list must be a filesystem item (i.e. a file or a directory)
  - nesting should be indicated by indentation
  - each item in the unordered list should be a *relative* path (relative to its parent)
  - directories *must* end with a `/` (e.g. `src/`, not `src`)
  - items *must* not be the `.` and `..` "pseudo-directories"
  - symlinks *may* be represented as files, or with the referred-to location (e.g. `- latest.a # symlink to latest build`) 
  - items *may* include a "comment" after the item, separated by a `#` character (it is *not* an error to include multiple `#` characters in a comment)
  - we allow arbitrary whitespace between the path and the comment (e.g. `src/    # source code`) 
  - we allow arbitrary whitespace between the comment and the end of the line
  - we *do not* enforce a specific ordering within the unordered list
  - we *do not* require completeness (i.e. it's ok to omit files and directories)

For example, every line of the following is valid:

```markdown
- .github/     # github-specific items
  - workflows/ # github workflows
    - ci.yml # build-and-test workflow
    - cd.yml # release-distribution workflow
- src/       # cli tool source code
  - main.rs  # main entry point for cli tool  
- Cargo.toml # project manifest
- README.md  # human-facing overview
```

...and here are some examples of invalid lines:

- `- src` (directories *must* end with a `/`)
- `- src/ source code` (comments must be separated from the path by a `#` character)
- `- src/ // source code` (comments must be separated from the path by a `#` character)
- `- src/ <- source code` (comments must be separated from the path by a `#` character)
- `- ..` (`.` and `..` are not valid filesystem items)

In addition to the above syntactic requirements, the navigation guide must also adhere to the following *semantic* requirements (when validated):

- all file-system items must exist at the specified location (i.e. the path must resolve to an existing file or directory)
- all file-system items must be of the correct type (i.e. if the guide specifies a path as a file, it must resolve to a file, and similarly for directories)
- the nesting structure must reflect the filesystem contents (i.e. if the guide specifies a path as a child of another path, the former must be a child of the latter in the filesystem)

As has been mentioned, the navigation guide need not be a *complete* description of the filesystem; it's ok if it *omits* files and directories.

## Component: `agentic-navigation-guide`

The `agentic-navigation-guide` crate provides an eponynous CLI tool with several subcommands (which we will outline below). The options common to *all* commands are:

- `--verbose`
  - default: false
  - effect: setting `--verbose` causes the tool to print more verbose output
- `--quiet`
  - default: false
  - effect: setting `--quiet` causes the tool to print no unnecessary output

Internally these can be represented as a 3-way enum like `quiet | default | verbose`; the value of this enum can also be controlled by an environment variable (e.g. `AGENTIC_NAVIGATION_GUIDE_LOG_MODE`).

We should also inherit `clap`s `--help` and `--version` options (etc.).

The full list of subcommands is as follows:

- `dump`: dump the current directory contents in the intended markdown format
- `init`: convenience to save the output of `dump` into a file (e.g. as a starting point for a 
- `check`: check that the contents of a hand-written navigation guide are *syntactically* correct (i.e. adhere to the format specified above)
- `verify`: verify that the contents of a hand-written navigation guide accurately reflect the current state of the file system

### Subcommand: `dump`

The `dump` command prints the contents of the current directory hierarchy to stdout (or an output file, if one is provided), formatted as per the intended markdown format. The command is primarily intended for debugging the output.

Named Arguments:

- `--output <path>`: the path to the output file (default: stdout, instead)
- `--depth <depth>`: the maximum depth to dump (default: unlimited)
- `--exclude <glob>`: a glob pattern to exclude from the output (default: none); can be repeated
- `--indent <indent>`: the number of spaces to use for indentation (default: 2)
- `--omit-xml-wrapper`: omit the `<agentic-navigation-guide>` and `</agentic-navigation-guide>` markers from the output (default: false)
- `--root <root>`: the root directory to dump

If `--root` is not specified, the following defaults are used, in this order of precedence:

- the directory at the path in the `AGENTIC_NAVIGATION_GUIDE_ROOT` environment variable, if set
- the current directory

### Subcommand: `init`

The `init` command behaves almost-identically to the `dump` command, with only the following changes:

- the `--output` argument is required
- there is no `--omit-xml-wrapper` parameter (and the `<agentic-navigation-guide>` and `</agentic-navigation-guide>` markers are always included)

### Subcommand: `check`

The `check` command verifies that the contents of a hand-written navigation guide are *syntactically* correct (i.e. adhere to the format specified above). Errors are reported to stderr, and the exit code is 0 if the guide is syntactically correct, and non-0 otherwise.

Named Arguments:

- `--post-tool-use-hook`: indicates we're being invoked as a post-tool-hook by claude code
- `--pre-commit-hook`: indicates we're being invoked as a pre-commit-hook by git
- `--guide <path>`: the path to the `AGENTIC_NAVIGATION_GUIDE.md` file

If `--guide` is not specified, the following defaults are used, in this order of precedence:

- the file at the path in the `AGENTIC_NAVIGATION_GUIDE_PATH` environment variable, if set
- the file in the current directory named by the `AGENTIC_NAVIGATION_GUIDE_NAME` environment variable, if set 
- `AGENTIC_NAVIGATION_GUIDE.md` in the current directory

Internally these can be represented as a 3-way "mode" enum like `default | post-tool-use | pre-commit-hook`; the value of this enum can also be controlled by an environment variable (e.g. `AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE`).

There is a single positional argument: the path to the `AGENTIC_NAVIGATION_GUIDE.md` file (default, if unspecified: the file at the `AGENTIC_NAVIGATION_GUIDE_PATH` environment variable, if set; otherwise, `AGENTIC_NAVIGATION_GUIDE.md` in the current directory).

If the file is valid, in default mode (and quiet mode) the command prints nothing to stdout, nothing to stderr, and exits with a 0 exit code.

If the file contains errors, each should be reported on a separate line, using a brief message that includes the line number and a brief description of the error; these messages should explain the error, but need not include remediation advice. 

Note again that this subcommand *does not* verify the accuracy of the guide vis-a-vis the current state of the file system—it just checks for valid syntax.

### Subcommand: `verify`

The `verify` command performs all the syntactic checks of `check`, and then goes on to verify the semantics vis-a-vis the current state of the file system. Note that this order is important: if the file fails the syntactic checks, we report the syntactic errors and then halt—we should *not* attempt to verify the semantics of a malformed guide.

The `verify` command accepts all the same arguments as the `check` command, as well as the `--root` argument (with the same semantics as used in `dump` and `init`).

If there are no errors, the command prints nothing to stdout, nothing to stderr, and exits with a 0 exit code.

If there are errors, each should be reported *to stderr* on a separate line, using a brief message that includes the line number and a brief description of the error; these messages should explain the error, but should not include remediation advice.

The messages for syntactic errors should be identical to those for `check`. The messages for semantic errors should be suitable for consumption by both humans and coding assistants.

## Details: Execution Modes

We want to make sure the internals are aware of when they're being run as a pre-commit hook, a post-tool-use hook, or in "default" mode. For now the main distinction is that we need to be careful to return error code **2** when we fail on a post-tool-use hook invocation, since that's what claude code expects.

## Details: Rust

The internals should be setup for maintainability and comprehensibility, e.g.:

- there should be a datatype corresponding to a filesystem item (e.g. `FilesystemItem` w/cases for files, directories, and symlinks)
- there should be a datatype corresponding to a single line of the guide
- there should be a datatype corresponding to the entire guide
- there should be typed errors for syntax errors (e.g. distinct cases for each type of error)
- there should be typed errors for semantic errors (e.g. distinct cases for each type of error)

Additionally, the following basics should be setup properly:

- we should have a "library" with the core logic (`lib.rs`) and a "binary" with the CLI (`main.rs`)
- the "library" should have a well-defined public API
- the "library" should have unit tests thoroughly exercising the format semantics
- rustfmt should be used, and setup to run automatically:
  - on save in vscode
  - on pre-push (as a git hook)
  - after tool use (as a claude code hook)
- clippy should be used, and setup to run automatically after tool use
- tests should be used (and setup as a pre-push hook)
- documentation should be used (and setup as a pre-push hook)
