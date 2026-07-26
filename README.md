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
- **2026-07-25 — File output is intentionally create-only.** Unlike `0.1.4`,
  v0.2 `dump --output` does not overwrite an existing destination; it shares
  `init`'s create-new policy and has no force mode. This breaking CLI change
  prevents scripts or races from silently replacing a file. Callers that
  intentionally replace output must remove or rename the old entry first.
- **2026-07-25 — Empty recursive verification is intentionally fail-closed.**
  Unlike `0.1.4`, `verify --recursive` exits nonzero after discovering zero
  guides, including when a root/name is wrong, exclusions remove every match,
  or the last guide was deleted. An intentionally optional search must opt in
  with `--allow-empty`; required CI gates must omit that flag.
- **2026-07-25 — Guide files are opened fail-closed.** Unlike `0.1.4`, v0.2
  never follows a final guide-file link or reparse entry, including an
  explicitly selected or in-root link. Callers may still select an external
  regular file directly. This breaking correction prevents an untrusted
  checkout from redirecting guide reads and source-context diagnostics to a
  different local file.
- **2026-07-25 — Ignored guide bodies are opaque.** Unlike `0.1.4`, a valid
  `ignore=true` envelope may contain arbitrary UTF-8 text or an empty body.
  The CLI reports a distinct ignored outcome instead of checked or verified
  success. Ignored guides are allowed by default; automation that forbids the
  opt-out must pass `--deny-ignored`.
- **2026-07-26 — Generated and listed entry types fail closed.** Unlike
  `0.1.4`, v0.2 never emits an included link, Windows reparse entry, FIFO,
  socket, device, or unknown entry as a regular file. `dump` and `init` abort
  before delivering guide bytes, and verification does not let a final link
  satisfy a textual file or directory.
- **2026-07-26 — Generation rejects invalid or empty inputs and unbounded
  numeric options.** Unlike `0.1.4`, v0.2 requires a readable directory root
  and at least one included, representable entry. Empty, fully excluded,
  regular-file, missing, or unreadable roots fail before delivery.
  `--indent` is limited to 1–16 and `--depth` to 0–256 instead of flattening,
  wrapping, panicking, or allocating pathologically. This is an intentional
  tightening under #43.
- **2026-07-26 — Exclusions use one depth-aware glob dialect.** Unlike
  `0.1.4`, a no-slash pattern such as `target` or `.git` matches basenames at
  every depth, while a slash pattern matches one complete root-relative path.
  `dump`, `init`, and recursive discovery now validate and use the same
  case-sensitive, platform-independent matcher. This corrects the nested
  traversal bug under #44.
- **2026-07-26 — CLI options override environment defaults.** Unlike `0.1.4`,
  environment-backed path, root, name, log, and execution settings no longer
  participate in unrelated CLI requirements or conflicts. Resolution is
  consistently CLI, then environment, then built-in default; genuine
  CLI-vs-CLI conflicts remain usage errors. This corrects the precedence bug
  under #46.
- **2026-07-26 — Verification uses exact enumerated filesystem identities.**
  Unlike `0.1.4`, a case or Unicode-normalization alias accepted by the host
  filesystem does not satisfy a differently spelled guide entry. Each visited
  parent is enumerated once per verification and that snapshot drives exact
  component lookup, type classification, and placeholder accounting. This
  corrects the identity and repeated-scan bugs under #50.
- **2026-07-26 — Verification containment is anchored to a stable tree.**
  Unlike `0.1.4`, verification traverses from the caller-selected root's
  once-canonicalized directory, rejects every link or reparse ancestor below
  that anchor without resolving its target, and fails on identity or type
  changes it observes. This is a stable-filesystem consistency guarantee, not
  a sandbox or a hostile-concurrent-replacement guarantee.

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

- each entry uses the exact list delimiter `- `; a tab or a second unescaped
  space after the dash is invalid
- the first entry is unindented; nesting then uses one inferred space-only
  indentation unit of 1–16 spaces, may increase by one level only when the
  immediately preceding physical line represents exactly one directory, and
  is limited to depth 256
- exactly one trailing `/` marks a directory entry; repeated separators and
  empty path components are invalid before the marker is removed
- comments are optional; outside a whole quoted path, the first unescaped `#`
  starts the comment portion
- if an entry has no unescaped `#`, everything after the exact list delimiter
  is part of the path (for example, `- src/ source code` is a literal path,
  not a comment)
- blank lines are not allowed within the guide block
- `/` is the only logical separator; paths must be relative, cannot contain
  empty, `.` or `..` components, and cannot use a leading slash/backslash or a
  first-component Windows drive prefix
- decoded file and directory paths must be unique across the complete guide,
  including equivalent flat and nested spellings
- no ordering requirement is imposed on hand-authored guides
- placeholder entries (`...`) can be used to indicate unlisted items (see below)

Note that it's *not* an error to omit files and directories from the guide, but it *is* an error to include incorrect entries—the guide *must* be accurate*.

### Filesystem Names and Quoting

A path expression may be bare or whole quoted. In a bare path, the complete
escape set is `\#`, `\\`, `\[`, `\]`, `\,`, `\"`, and `\ `; any other or
dangling escape is invalid. Leading or trailing U+0020 spaces require the
whole quoted form.

A whole quoted path preserves syntax characters and edge spaces literally.
Inside it, only `\"` and `\\` are escapes. A directory marker remains outside
the closing quote, as in `- "dir#draft"/`. Quoted `"..."` is the literal
filesystem name, while bare `...` remains a placeholder. Supported Unicode
scalar sequences are preserved exactly without normalization.

`dump` and `init` emit canonical names: an ordinary component is bare, while a
syntax-sensitive component, an edge-space component, or literal `...` is
whole quoted. Each physical line represents one immediate child, deeper
components use indentation, and siblings are sorted by ascending UTF-8 bytes.
Only regular files, directories, and hard-linked regular files are
representable. After exclusions are applied, an included symbolic link,
Windows reparse entry, FIFO, socket, device, unknown type, or transient
classification failure aborts generation without following or traversing the
entry. The complete included tree is classified before stdout or a new output
file receives guide bytes.

Names containing NUL, CR, LF, HTAB, another C0 character, or DEL are rejected,
as are non-UTF-8 filesystem names. Rejected-name diagnostics are
double-quoted and reversible: valid UTF-8 uses `\"`, `\\`, `\0`, `\t`, `\n`,
`\r`, or uppercase `\u{XXXX}`; undecodable Unix bytes use uppercase `\xNN`,
and ill-formed Windows names preserve every UTF-16 unit as uppercase
`\u{XXXX}`. Generation validates the complete included name set before
writing guide bytes or creating its destination.

### Generation Preconditions and Bounds

`dump` and `init` require an existing, readable directory root. A
caller-selected root symbolic-link, junction, or reparse alias is resolved and
accepted as the generation anchor; that exception does not authorize links
among included descendants. Empty roots and roots left with no representable
entry after depth and exclusion rules fail actionably. An included empty
directory is itself a representable entry and therefore is not empty output.

`--indent` accepts 1 through 16 spaces and defaults to 2. `--depth` accepts 0
through 256; zero includes root children but not their children. An explicit
depth intentionally produces a partial listing and does not inspect deeper
entries. With no explicit depth, an included tree requiring logical depth 257
fails instead of being silently truncated. Values outside either range are
rejected during argument parsing, and the legacy library path also rejects
them with an error rather than clamping, wrapping, panicking, flattening, or
attempting an unbounded indentation allocation.

All root, traversal, classification, name, depth, indentation, and
serialization checks complete before stdout delivery or filesystem
destination creation. Every successful generated body is nonempty and can be
parsed and checked as an active guide. These checks assume a stable filesystem
while the command runs; they are consistency protections, not a sandbox
against hostile concurrent replacement.

### UTF-8 Scope

- UTF-8 paths are supported, including non-ASCII names.
- Non-UTF-8 filesystem names are explicitly out of scope.
- Commands that enumerate filesystem entries, including every parent visited
  by `verify`, return an error if they encounter non-UTF-8 names.

### Filesystem Identity During Verification

Guide components match the exact UTF-8 scalar sequence returned while
enumerating their parent directory. Matching is case-sensitive on every host
and performs no Unicode normalization, even when ordinary host path lookup
would accept an alias. Each visited parent is enumerated at most once in one
verification; the resulting immediate-child names and types are reused for
listed lookup, recursion, and placeholder accounting. A later verification
constructs fresh snapshots.

### Verification Containment and Concurrent Mutation

The caller-selected verification root is canonicalized once. The root itself
may be a symbolic-link, junction, or reparse alias; its canonical directory is
the anchor used for every later item lookup. Guide paths are validated as
relative logical components before access. From the anchor, verification
matches each component in its exact parent snapshot, observes it without
following links, and descends only through a real directory. An intermediate
symbolic link, junction, or other link-like reparse entry is rejected before
its target is resolved, whether that target is in-root, external, dangling,
chained, or looping.

The verifier records filesystem identity and type for each visited parent and
listed component. It rechecks parents around enumeration and listed entries
before and after dependent verification, failing when it observes
disappearance, identity replacement, or a file/directory/type change.
Containment diagnostics retain the safe logical guide path but omit canonical
root aliases and resolved external targets.

These checks assume the relevant tree remains stable from root resolution
through the last dependent lookup or enumeration. A process that can replace
the root, an ancestor, or an item between checks is outside the v0.2
guarantee; identity rechecks are defense in depth and do not close every race.
The verifier is a consistency checker, not a filesystem sandbox, access
control boundary, malware scanner, or safe way to execute an untrusted
process.

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

A placeholder asserts only that an unlisted UTF-8-named immediate child
exists. It remains type-agnostic, so a special or link-like sibling may satisfy
the placeholder even though that same entry cannot be listed as a textual file
or directory and cannot be emitted by `dump` or `init`.

Rules for placeholders:
- Written as `...` (three dots)
- May have an optional comment after it
- Cannot have child elements nested under them
- **With a comment**: Allowed in any directory, even if all items are listed or the directory is empty (useful for indicating future items)
- **Without a comment**: Must refer to at least one unlisted item in the parent directory (useful for omitting existing items)
- Cannot be adjacent to another `...` entry (must have at least one non-placeholder between them)
- Multiple nonadjacent placeholders in one parent share the same snapshot and
  do not consume unlisted entries.
- A listed multi-component path such as `src/main.rs` mentions `src` in the
  current parent.

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

Each bare regular-file entry may contain at most one choice list with 2–256
alternatives. It expands into one sibling file for every option in source
order, and the same comment is attached to every expanded item.

Choice lists follow these rules:

- Spaces and tabs surrounding an unquoted alternative are layout. Interior
  unquoted whitespace and every character inside a quoted alternative are
  preserved.
- An empty alternative may be included by leaving an empty slot (for example,
  `[, .local]`) when at least one other alternative is nonempty.
- Use a backslash to escape individual characters (e.g. `\,` for a literal comma, `\ ` for a literal space, `\#` for a literal `#`, `\[` for a literal
  `[` character).
- Surround complex values with double quotes to preserve punctuation or embedded brackets. Within quotes, escape `"` to include
  a literal quote character.
- Decoded expansions must be unique, valid regular-file paths with the same
  parent components. A choice cannot produce a directory or placeholder or
  own indented children.
- A second list, unmatched bracket or quote, dangling/invalid escape,
  all-empty list, or malformed quoted alternative is rejected.

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

After the exact envelope and full-document marker-candidate scan succeed, an
ignored body is opaque UTF-8 text and may be empty. List, indentation, path,
choice, placeholder, syntax-validator, and filesystem checks are skipped. A
malformed marker candidate anywhere in the document still fails before ignore
can apply.

`check`, single-guide `verify`, and recursive `verify` all produce a distinct
ignored outcome. The command exits successfully by default and reports the
ignored guide in non-quiet modes; `--quiet` suppresses that ordinary chatter
without changing the result. Ignored work is never described or counted as
checked or verified success.

Pass `--deny-ignored` to `check` or `verify` when ignored guides must make the
run nonzero:

```bash
agentic-navigation-guide check --deny-ignored
agentic-navigation-guide verify --recursive --deny-ignored
```

Recursive totals count ignored guides separately. An ignored-only search is
discovered work, not zero discovery, and reports `Passed: 0`, `Ignored: 1`,
and `Absent: 0`. Denial preserves those categories and counts. Execution-mode
flags do not silently enable denial; CI and hooks must opt in explicitly.
There is no supported v0.2 Rust library facade from which to obtain an ignored
result. See the [normative contract](docs/v0.2-contract.md#ignored-guides).

## Suggested Usage

To use this tool, I would suggest you do this:

- put your navigation guide in a file named `AGENTIC_NAVIGATION_GUIDE.md` in the root of your project
- use the `@` syntax to include it in your `CLAUDE.md` file (etc.)

For a fuller example, you can review the [`CLAUDE.md`](./CLAUDE.md) file within this repository. 

The advantage of this workflow is it keeps your navigation guide content physically-isolated from your CLAUDE.md (etc.)—helpful for editing and reviewing!—while still bringing the guide into context for each session.

## Tool Overview

The tool provides the following commands:

- `init --output <path>`: initialize a new navigation guide file from a
  nonempty, readable directory tree
- `check`: check that the contents of a hand-written navigation guide are *syntactically* correct (i.e. adhere to the format specified above)
- `verify`: verify that the contents of a hand-written navigation guide accurately reflect the current state of the file system
- `dump [--output <path>]`: dump a nonempty, readable directory tree in the
  intended markdown format, to stdout by default

If you're adding a navigation guide to your repository, I'd suggest:

- run `agentic-navigation-guide init --output AGENTIC_NAVIGATION_GUIDE.md` to generate a starting point
- hand-edit the file to add comments and omit extraneous details
- run `agentic-navigation-guide verify` to check for errors
- commit the file to your repository
- update your CLAUDE.md (etc.) to include the guide using the `@` syntax

### Environment Defaults

Configuration uses one precedence rule: **CLI > environment > built-in**. A
lower-precedence value is applied only when it is relevant and no
higher-precedence value selected that setting.

| Variable | Scope | CLI override | Built-in fallback |
| --- | --- | --- | --- |
| `AGENTIC_NAVIGATION_GUIDE_PATH` | `check` and non-recursive `verify` | `--guide` | No explicit path; use the implicit guide name |
| `AGENTIC_NAVIGATION_GUIDE_ROOT` | `dump`, `init`, and all `verify` modes | `--root` | Current directory |
| `AGENTIC_NAVIGATION_GUIDE_NAME` | Implicit `check`, implicit single `verify`, and recursive `verify` | `--guide` selects an explicit path for single-guide commands; `--guide-name` overrides the recursive name | `AGENTIC_NAVIGATION_GUIDE.md` |
| `AGENTIC_NAVIGATION_GUIDE_LOG_MODE` | Global; `quiet`, `default`, or `verbose` | `--quiet`, `--verbose`, or `--log-level` | `default` |
| `AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE` | Global; `default`, `post-tool-use`, `pre-commit-hook`, or `github-actions` | `--execution-mode` or an applicable hook/check flag | `default` |

When both path and name environment variables are set, the explicit
`AGENTIC_NAVIGATION_GUIDE_PATH` wins for `check` and single `verify`.
Recursive verification ignores that path variable and resolves only its guide
name. The path variable, like `--guide`, grants explicit guide-file authority;
the name variable always remains one implicit filename beneath the applicable
root.

Irrelevant and shadowed environment values are not parsed. Environment-default
resolution rejects an empty path/root, invalid single-component name, or
invalid mode before command work begins. Those configuration diagnostics
identify the variable and expected form without printing its value. Rejection
at this pre-execution layer is a usage error with status 2, regardless of the
requested execution mode. A nonempty path/root is preserved as an
operating-system path; existence, access, and trust checks then follow the
command's ordinary filesystem rules. Explicit CLI contradictions—such as
`--quiet --verbose` or `verify --guide ... --recursive`—remain actionable usage
errors.

### Exclusion Patterns

`dump`, `init`, and `verify --recursive` use the same exclusion language.
Every match input is a UTF-8 path relative to the selected root, with `/` as
the logical separator on every operating system. Matching is case-sensitive,
performs no Unicode normalization, and consumes the complete basename or
root-relative path.

- A pattern without `/`, such as `target` or `*.tmp`, is matched against every
  basename at every depth.
- A pattern with `/`, such as `project/target`, matches only that complete
  root-relative path; it has no implicit `**/` prefix.
- `*` consumes zero or more Unicode scalars within one component, `?` consumes
  exactly one, and a complete `**` component consumes zero or more path
  components.
- Classes support sets (`[abc]`), inclusive ranges (`[a-z]`), and leading-`!`
  negation (`[!0-9]`). Outside classes, the documented escapes are `\\`,
  `\*`, `\?`, `\[`, and `\]`; inside classes they are `\\`, `\]`, and `\-`.
- Repeated `--exclude` options form a union. A leading `!` outside a class and
  braces are literals, not re-inclusion or alternation syntax.

For example:

| Pattern | Matches | Does not match |
| --- | --- | --- |
| `target` | `target`, `project/target` | `targets` |
| `*.tmp` | `a.tmp`, `nested/a.tmp` | `a.tmp.keep` |
| `project/target` | exactly that root-relative path | `other/project/target` |
| `projects/*/target` | one component between | `projects/a/b/target` |
| `projects/**/target` | `projects/target`, `projects/a/b/target` | `projects/a/b/target/file` |

Empty patterns, invalid separators or dot components, malformed `**`, invalid
classes, and unknown or dangling escapes fail before traversal. A matched
directory is pruned before its children are read; a non-UTF-8 entry encountered
before pruning fails actionably. `init` excludes `.git`, `.svn`, `.hg`, `.bzr`,
`CVS`, and `_darcs` as nested basenames by default unless
`--include-vcs-directories` is passed. `.gitignore` files are not interpreted.
The [normative contract](docs/v0.2-contract.md#exclusion-patterns) contains the
complete grammar.

### Guide-Input Safety

Default `check` treats the current working directory as its guide trust
anchor. A default single-guide `verify` resolves the guide from its effective
verification root, and recursive verification anchors discovery at its search
root. An implicit guide name must be exactly one nonempty filename component.
On Windows, stream syntax, device aliases and namespaces, and unsupported
prefixes are rejected, and the implicit name must exactly match an enumerated
entry.

A guide's final entry must be a regular file opened without following links.
Final symbolic links and link-like Windows reparse entries are rejected even
when they are explicit, remain in-root, or point to a valid guide. Hard-linked
regular files remain regular. A caller-selected root alias is accepted as the
anchor; guide-path link or reparse ancestors below that anchor are rejected.
An explicitly selected external regular guide may use its stable external
ancestor chain because the exact configured path grants read authority.

Recursive discovery does not traverse descendant links or reparse points. An
explicit exclusion is applied before an unsafe matching entry is classified;
without that exclusion, an unsafe match is an error rather than an empty
search, and `--allow-empty` cannot suppress it.

Guide-opening diagnostics use a bounded, control-safe logical path and reason
without revealing rejected guide bytes or a resolved guide-link target.
Complete guide source lines are not echoed by CLI parsing or validation
errors. These checks assume a stable filesystem while the command runs. They
are consistency protections, not a sandbox against hostile concurrent
replacement.

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
following links and creation uses exclusive, no-follow semantics. A parent
whose mode exposes no write bit or no search bit is treated as read-only even
when a privileged identity could bypass discretionary access control; an
otherwise eligible parent must also pass the current identity's access check.
On Windows, link-like reparse descendants, alternate data streams, device and
named-pipe namespaces, reserved DOS device aliases, and unsupported verbatim
prefixes are rejected; a newly created handle must be a regular non-reparse
disk file.

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
            "command": "agentic-navigation-guide verify --post-tool-use-hook --deny-ignored"
          }
        ]
      }
    ]
  }
}
```

This required hook passes `--deny-ignored` so an opt-out cannot silently
disable it. Omit that flag only when ignored guides are intentionally allowed.

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
      run: agentic-navigation-guide verify --github-actions-check --deny-ignored
```

The `--github-actions-check` flag provides:
- Concise output on success ("✓ Navigation guide verified")
- Detailed error messages with file:line references
- Exit code 1 on failure (standard for CI checks)
- Visual indicators (emoji) for quick scanning

The examples also pass `--deny-ignored` because they are required gates.
Execution mode alone does not forbid ignored guides.

You can also set the execution mode via environment variable:
```yaml
- name: Verify navigation guide
  run: agentic-navigation-guide verify --deny-ignored
  env:
    AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE: github-actions
```

For a monorepo, make recursive discovery part of the required gate without an
empty-search opt-out:

```yaml
- name: Verify every navigation guide
  run: agentic-navigation-guide verify --recursive --github-actions-check --deny-ignored
```

This command fails if discovery verifies zero guides, so a wrong root, a
misspelled guide name, an overbroad exclusion, or deletion of the final guide
cannot silently disable the check.

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

Recursive verification fails by default when discovery returns zero matching
guides. The required diagnostic reports the selected root and guide name,
suggests checking `--root`, `--guide-name`, and `--exclude`, and distinguishes
the absent search from passed, failed, and ignored guides. Quiet mode suppresses
ordinary success output but never suppresses this failure.

Use `--allow-empty` only when a recursive search is deliberately optional:

```bash
agentic-navigation-guide verify --recursive --allow-empty
```

That explicit flag permits only a successfully completed search with zero
matches. Missing, inaccessible, or non-directory roots, invalid exclusion
patterns, traversal failures, and failures from a discovered guide remain
nonzero. Non-quiet output still reports that zero guides were verified;
`--quiet --allow-empty` succeeds silently.

Unsafe matching guide entries also remain nonzero with `--allow-empty`.
Excluded entries are pruned before unsafe-entry classification, while
nonmatching descendant links are never traversed.

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
- **Fail-Closed Discovery**: Zero matches fail unless `--allow-empty` is explicit
- **Aggregated Results**: Separately reports discovered, passed, failed, ignored, and absent outcomes
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
