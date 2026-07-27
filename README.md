# `agentic-navigation-guide`

[![CI](https://github.com/plx/agentic-navigation-guide/workflows/CI/badge.svg)](https://github.com/plx/agentic-navigation-guide/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/agentic-navigation-guide.svg)](https://crates.io/crates/agentic-navigation-guide)
[Documentation](docs/v0.2-contract.md)
[![License](https://img.shields.io/crates/l/agentic-navigation-guide.svg)](#license)

`agentic-navigation-guide` is a CLI for creating and checking concise,
hand-maintained repository maps for coding agents. A guide may intentionally
omit files, but every path it includes must accurately describe the current
filesystem.

The README is the copy/paste entry point. The
[normative v0.2 contract](docs/v0.2-contract.md) defines the complete guide
format, CLI configuration and output, filesystem trust boundary, and supported
product. The [release policy](docs/release-policy.md) defines version, Rust,
dependency, and compatibility maintenance.

## Release status and support

`0.2.0` is prepared but not published. Until its crates.io release, commands
that name `--version 0.2.0` document the reviewed release path but are not yet
available from the registry.

The installed CLI is the sole supported v0.2 product. The current source
package is binary-only, with no linkable Rust library target or in-process
shim. Published `0.1.4` Rust consumers must invoke the installed CLI through
its process contract or remain pinned to unsupported `0.1.4` at their own
risk. See the [migration record](CHANGELOG.md#migration-from-014).

The prepared operating-system target is Linux, macOS, and Windows. The README
smoke workflow runs on all three; the complete platform-support claim remains
gated on the issue #55 filesystem conformance matrix and must be narrowed
before publication if a claimed invariant cannot pass. Rust `1.85.0` is the
minimum supported toolchain. CI also tests the current and immediately
previous stable Rust releases; beta is informational.

Once published, the documented CLI, guide format, process outcomes, supported
platform scope, and trust boundary form the `0.2.x` compatibility surface.
Breaking that surface requires `0.3.0`; adding a supported Rust library also
requires a new breaking line.

This is intentionally a personal, sole-maintainer project rather than an
organization-owned repository. The
[time-bounded continuity exception](docs/maintainer-continuity.md) records the
single release authority, absence of backup ownership, and best-effort support
without a response-time or recovery guarantee.

## Install, upgrade, and uninstall

Before publication, install only from the root of a trusted source checkout.
The second command deliberately reinstalls the same checked-out candidate and
is the reproducible source-checkout upgrade path.

```sh
# From the root of a trusted source checkout:
cargo install --path . --locked
cargo install --path . --locked --force
cargo uninstall agentic-navigation-guide
```

Cargo/crates.io is the only supported release installation channel for
`0.2.0`. After publication, use the exact version and committed dependency
graph. The `--force` form is the exact upgrade/reinstall command.

```sh
# Available after 0.2.0 is published:
cargo install agentic-navigation-guide --version 0.2.0 --locked
cargo install agentic-navigation-guide --version 0.2.0 --locked --force
cargo uninstall agentic-navigation-guide
```

Omitting `--locked` asks Cargo to resolve a different dependency graph.
Homebrew packages and prebuilt GitHub Release archives are not currently
supported installation channels. If the release pipeline in issue #63 adds a
channel, its exact install, upgrade, uninstall, checksum, and smoke commands
must be added here and to the README test before publication.

## Quickstart

Start in the directory that should contain a new demo project, with Cargo and
the installed `agentic-navigation-guide` executable on `PATH`. These commands
work unchanged in POSIX shells and PowerShell:

```sh
cargo new --bin navigation-demo --vcs none
cd navigation-demo
agentic-navigation-guide init --output AGENTIC_NAVIGATION_GUIDE.md
agentic-navigation-guide check
agentic-navigation-guide verify
```

The three tool commands report:

```text
Navigation guide created at: AGENTIC_NAVIGATION_GUIDE.md
✓ Navigation guide syntax is valid
✓ Navigation guide is valid and matches filesystem
```

`init` requires `--output` and creates, rather than overwrites, the destination.
Edit the generated guide to keep only useful paths and add comments, then rerun
`check` and `verify`.

## One short workflow

1. `dump` previews a generated guide on stdout; it does not write unless
   `--output` is explicit.
2. `init --output AGENTIC_NAVIGATION_GUIDE.md` creates a starting guide and
   refuses to replace an existing destination.
3. `check` parses and validates guide syntax without comparing listed paths to
   the filesystem.
4. `verify` checks one guide against its root.
5. `verify --recursive --github-actions-check --deny-ignored` discovers and
   verifies every guide below the root as a required CI gate. It fails when
   discovery finds zero guides.

Use `agentic-navigation-guide <command> --help` for the exact options.
Configuration precedence is **CLI > environment > built-in**. The five
environment settings are `AGENTIC_NAVIGATION_GUIDE_PATH`,
`AGENTIC_NAVIGATION_GUIDE_ROOT`, `AGENTIC_NAVIGATION_GUIDE_NAME`,
`AGENTIC_NAVIGATION_GUIDE_LOG_MODE`, and
`AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE`. The
[configuration contract](docs/v0.2-contract.md#command-line-and-environment-configuration)
defines every variable, precedence rule, usage error, and exclusion pattern.

## GitHub Actions

The example below is mirrored in
[`.github/examples/readme-verify.yml`](.github/examples/readme-verify.yml).
CI parses that file with `actionlint`, rejects mutable action references, and
runs the README smoke harness on Linux, macOS, and Windows.

```yaml
name: Verify Navigation Guide

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true

jobs:
  verify-navigation-guide:
    name: Verify Navigation Guide
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@93cb6efe18208431cddfb8368fd83d5badbf9bfd # v5.0.1
        with:
          persist-credentials: false
      - uses: actions-rust-lang/setup-rust-toolchain@46268bd060767258de96ed93c1251119784f2ab6 # v1.16.1
        with:
          toolchain: "1.85.0"
      - name: Install exact CLI
        run: cargo install agentic-navigation-guide --version 0.2.0 --locked
      - name: Verify every required guide
        run: agentic-navigation-guide verify --recursive --github-actions-check --deny-ignored
```

This release-channel job becomes runnable only after `0.2.0` is published.
Before then, the repository's own trusted workflow builds and tests the
checked-out candidate. Required automation uses `--deny-ignored`; it must not
use the optional empty-search override.

## Claude Code post-tool-use hook

A required local hook can verify the guide after filesystem-changing tools:

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

The hook exits `2` for verification failure in post-tool-use mode. Omit
`--deny-ignored` only when an ignored guide is an intentional local opt-out.

## Guide and security contract

Active guides contain one nonempty, exact list between the
`<agentic-navigation-guide>` markers. They are partial by default. Logical
paths use `/`; indentation is space-only; quoted paths and documented escapes
represent syntax-sensitive names; and bare `...` is a placeholder. A valid
`ignore=true` guide has an opaque body and produces a distinct ignored outcome,
not verified success.

`dump` and `init` reject invalid roots, empty generated output, unsupported
entries, and unrepresentable names before delivering guide bytes. `check`,
single-guide `verify`, and recursive `verify` share fail-closed guide opening
and bounded diagnostics.

`init --output` and `dump --output` exclusively create a new destination but
do not provide atomic content publication: a concurrent reader may observe an
in-progress prefix, and no parent-directory synchronization or crash durability
is promised.

Repositories and guide text are untrusted, but the selected executable,
operating system, current user, and dependencies are trusted. The CLI provides
a stable-filesystem containment and consistency guarantee. The CLI is not a sandbox against hostile concurrent replacement. It is also not a malware
scanner or access-control boundary.

An unresolved `..` in a path that starts beneath the anchor does not automatically grant external authority. Parent components are classified in order,
and only a spelling proven to remain outside the anchor receives the external
authority explicitly granted by its configured guide path.

Use these maintained references rather than copying detailed rules from this
README:

- [guide grammar and executable examples](docs/v0.2-contract.md#document-and-marker-grammar);
- [process output and automation diagnostics](docs/v0.2-contract.md#process-output-and-automation-diagnostics);
- [filesystem trust and containment](docs/v0.2-contract.md#filesystem-trust-and-containment);
- [supported CLI-only product and compatibility](docs/v0.2-contract.md#supported-product-and-rust-api);
- [release and Rust support policy](docs/release-policy.md); and
- [complete prepared `0.2.0` changes](CHANGELOG.md#020---unreleased).

## Known Intentional Divergences

This is the concise dated index of deliberate v0.2 breaks from published
`0.1.4`. The [changelog](CHANGELOG.md#migration-from-014) and
[normative contract](docs/v0.2-contract.md) retain the complete behavior and
migration details.

- **2026-07-26 — The package is CLI-only.** The Rust library and in-process
  facade were removed so v0.2 has one supported product and process contract.
- **2026-07-26 — `NavigationGuide::get_full_path` has no replacement.** Its
  detached-line input lacked parent context, so retaining a similarly named
  helper would preserve ambiguous behavior.
- **2026-07-26 — The programmatic symlink model has no replacement.**
  `FilesystemItem::Symlink` and `SemanticError::SymlinkTargetMismatch` were
  removed because the guide language has no supported link-target model.
- **2026-07-25 — File output is create-only.** `dump --output` and `init
  --output` refuse existing destinations so scripts and races cannot silently
  replace a file; there is no force mode.
- **2026-07-25 — Empty recursive verification fails closed.** A zero-guide
  search is an error because it usually means a wrong root, exclusion, or
  deleted guide; only an explicitly optional search may use `--allow-empty`.
- **2026-07-25 — Guide files are opened fail closed.** Final guide-file links
  and reparse entries are rejected so an untrusted checkout cannot redirect
  reads or source diagnostics.
- **2026-07-25 — Ignored guide bodies are opaque.** A valid `ignore=true`
  envelope reports a distinct ignored outcome because an opt-out is neither
  checked nor verified success.
- **2026-07-26 — Unsupported entry types fail closed.** Links, reparse entries,
  FIFOs, sockets, devices, and unknown entries are rejected so they cannot be
  emitted or verified as regular files.
- **2026-07-26 — Invalid, empty, and unbounded generation inputs are rejected.**
  Generation now fails before delivery and bounds `--indent` and `--depth` so
  bad inputs cannot flatten, wrap, panic, or allocate pathologically.
- **2026-07-26 — Exclusions use one depth-aware glob dialect.** Generation and
  recursive discovery share a case-sensitive, platform-independent matcher so
  nested paths cannot escape inconsistent rules.
- **2026-07-26 — CLI options override environment defaults.** Resolution is
  consistently CLI, then environment, then built-in so unrelated environment
  values do not create false requirements or conflicts.
- **2026-07-26 — Verification uses exact enumerated filesystem identities.**
  Host filesystem case or Unicode aliases do not satisfy differently spelled
  guide entries, preventing ambiguous matches and repeated scans.
- **2026-07-26 — Containment assumes a stable filesystem tree.** Verification
  anchors once, rejects links below the anchor, and detects observed identity
  changes because it promises stable-tree consistency rather than a sandbox
  against hostile concurrent replacement.

## Documentation history

Draft PR #21 proposed useful cleanup alongside stale and conflicting removals.
Its stray emphasis and preview-roadmap cleanup are incorporated here. Its
historical-specification deletion was explicitly superseded: the complete
proposal-by-proposal disposition is retained in the
[#68 audit](audits/2026-07-27-issue-68-specification-disposition.md), and the
[original specification](docs/history/Specification.md) remains clearly
non-normative.

`NavigationGuide::get_full_path` is removed without replacement in v0.2.
`FilesystemItem::Symlink` and `SemanticError::SymlinkTargetMismatch` are
removed without replacement in v0.2; filesystem links remain unsupported
entries and are classified internally only for fail-closed rejection. Code
that used those published `0.1.4` symbols must invoke the installed CLI or
remain pinned to unsupported `0.1.4`.

If implementation, tests, and this concise entry point disagree, treat that as
a defect and update user-facing documentation with the behavior change. Record
any approved intentional divergence with a date and rationale.

## License

Licensed under either:

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE)); or
- MIT ([LICENSE-MIT](LICENSE-MIT)).

The immutable published `0.1.x` archives contain contradictory manifest and
packaged-license information. [`LICENSING.md`](LICENSING.md) records the exact
artifact facts and the maintainer's no-yank decision without a legal
conclusion. [`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md) contains the
CI-checked dependency attributions.

Unless explicitly stated otherwise, contributions intentionally submitted for
inclusion are dual licensed under the same terms without additional
conditions.
