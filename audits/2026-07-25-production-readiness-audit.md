# Production-Readiness Due-Diligence Audit

## Document status

- **Project:** `agentic-navigation-guide`
- **Repository:** <https://github.com/plx/agentic-navigation-guide>
- **Audited revision:** `bf40806993ce0e0f2de931cabd105e3a0e063789`
- **Audit date:** 2026-07-25
- **Audit environment:** macOS, `America/Chicago`
- **Disposition:** **Not ready for the next formal release or for use as a
  load-bearing workplace verification gate**
- **Remediation target:** a focused `0.2.0` production-readiness hardening
  program
- **Tracking:** GitHub milestone and epic links will be added after the
  remediation issue hierarchy is created.

This document records the broad release-readiness review performed before
turning the repository from a lightly used personal project into a formally
released open-source tool. It is intentionally commit-specific: findings may
become stale as remediation work lands, but the evidence and conclusions here
should remain unchanged as an historical record.

The static documentation site and landing page under `site/` were explicitly
out of scope. Their accidental inclusion in the Cargo package was examined
because package boundaries are part of the Rust release surface; their content,
implementation, appearance, and behavior were not reviewed.

## Executive verdict

The project is a credible pre-1.0 Rust foundation, not a rewrite candidate. It
has sensible module boundaries, typed errors, deterministic output, substantial
tests, a clean locked dependency graph, and no production `unsafe` code.

The current implementation nevertheless has several purpose-defeating
fail-open and false-positive paths. In particular, it can:

1. verify a hierarchy different from the hierarchy written by the user;
2. generate listings that do not round-trip through its own parser;
3. accept a malformed opening marker that activates `ignore=true`;
4. report success when recursive mode finds no guide at all; and
5. follow guide and output symlinks across an unexpected trust boundary.

Those behaviors are release blockers because the product's central promise is
to detect inaccurate navigation guides. A green result must mean that the
intended guide was actually parsed and checked.

A focused `0.2.0` hardening cycle should be sufficient. The highest-value work
is to define the authoritative format and filesystem contract, make parsing and
discovery fail closed, establish a generator/parser round-trip invariant,
finalize the public API, and put cross-platform and release gates around those
contracts.

## Scope and methodology

The review covered:

- parser, validator, verifier, dumper, recursive traversal, CLI, and public API;
- correctness, adversarial inputs, filesystem behavior, performance, and
  portability;
- unit and integration tests, coverage, and CI workflows;
- dependency and workflow supply-chain posture;
- Cargo packaging, crates.io state, SemVer compatibility, licensing, MSRV, and
  release reproducibility;
- README and repository-level release/governance documentation; and
- practical crates.io and Homebrew distribution readiness.

The review combined source inspection with disposable filesystem fixtures and
direct CLI reproductions. No repository files were modified during the audit.

### Baseline checks

| Check | Result |
| --- | --- |
| `cargo test --all-targets --locked` | 68 unit + 59 integration passed |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | Passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked` | Passed |
| `cargo audit --file Cargo.lock` | No known advisories |
| `cargo package --locked --allow-dirty` | Passed |
| Raw LLVM line coverage | 83.03% |
| `cargo +1.78 check --locked` | Passed |
| `cargo +1.70 check --locked` | Failed: lockfile v4 is not parseable |
| `cargo-semver-checks` against published `0.1.4` | Failed |

The coverage percentage includes inline unit tests and does not include a
branch-coverage measurement. Passing tests therefore provide useful baseline
confidence but do not establish the absence of the edge cases below.

## Strengths worth preserving

- The code has clear parser, validator, verifier, dumper, recursive, CLI, and
  type boundaries.
- Syntax and semantic errors are typed and generally retain useful line
  context.
- Dump ordering is deterministic.
- Non-UTF-8 filesystem enumeration errors are handled explicitly rather than
  silently coerced.
- No production `unsafe` code was found.
- The locked dependency graph contains no Git dependencies and had no known
  RustSec advisories at the time of the audit.
- Rustdoc builds with warnings denied.
- Cargo packaging and publish verification compile successfully.
- Third-party license attribution regeneration is deterministic.
- Root containment is thoughtfully implemented for a stable filesystem. It
  rejects lexical traversal, existing symlink escapes, and missing paths under
  an escaping symlink ancestor. See
  [`src/verifier.rs`](../src/verifier.rs#L64).

These are strong reasons to harden the existing implementation instead of
replacing it wholesale.

## Release blockers

### 1. Indented children can be silently attached to the wrong directory

The hierarchy builder searches backward for any directory at the required
parent indentation instead of stopping at the nearest item at that level.

Given:

```text
- a/
- b
  - c
```

and a filesystem containing directory `a/c` and file `b`, both syntax checking
and verification succeed. The parser silently attaches `c` to `a`, although the
text makes `c` a child of the file `b`. The input should instead be rejected
because a file cannot own nested entries.

Relevant implementation:
[`src/parser.rs`](../src/parser.rs#L643).

This is a direct false-assurance defect in the core verification path.

Required direction:

- replace the backward-search/front-insertion hierarchy builder with a
  stack-based construction;
- reject indentation whose immediate logical parent is not a directory;
- retain source order without repeated `Vec::insert(0, ...)`; and
- add a regression test using the exact fixture above that fails before the
  fix and succeeds only when the malformed guide is rejected.

### 2. `dump` and `init` do not round-trip through the parser

The dumper emits path names verbatim at
[`src/dumper.rs`](../src/dumper.rs#L216), while the parser assigns grammar
meaning to `#`, `\`, square brackets, whitespace, and `...`.

Confirmed examples:

- With files `report` and `report#draft`, dump emits both names. The second
  line parses as a duplicate reference to `report` with comment `draft`, and
  verification still succeeds.
- `Foo[a,b].rs` is parsed as a choice that expands to two different files.
- A file literally named `...` is parsed as a placeholder.
- Leading or trailing whitespace alters a path.
- Embedded newlines create malformed output.
- Directory symlinks, dangling symlinks, FIFOs, sockets, and other special
  entries are emitted as ordinary files.
- A directory symlink generated by dump immediately fails verification as a
  file/directory type mismatch.
- An empty directory, a fully excluded directory, or even a regular file
  supplied as `--root` produces an empty wrapped block that the parser rejects
  as `EmptyGuideBlock`.
- `--indent 0` flattens children into root entries and produces a guide that
  fails verification.

Required direction:

- define the exact set of representable filesystem names and the escaping
  rules shared by parser and dumper;
- escape every representable grammar metacharacter;
- reject names and filesystem entry types that the format cannot represent
  without loss;
- reject invalid/non-directory roots and invalid numeric settings;
- decide an explicit policy for empty output; and
- add generated filesystem-tree tests asserting
  `filesystem -> dump -> parse -> verify`.

### 3. A malformed opening marker can disable verification

Opening-marker recognition uses a prefix test at
[`src/parser.rs`](../src/parser.rs#L54). The prefix is later stripped without
requiring a delimiter before attribute parsing.

This input exits successfully:

```text
<agentic-navigation-guideignore=true>
- definitely-missing.txt
</agentic-navigation-guide>
```

The malformed marker is interpreted as an opening marker with `ignore=true`.
Lookalikes such as `<agentic-navigation-guideXYZ>` are also accepted.

Required direction:

- accept only the exact opening name followed immediately by `>` or valid
  attribute whitespace;
- define a strict attribute grammar;
- reject unknown, duplicate, concatenated, or malformed attributes;
- keep the closing marker exact; and
- add table-driven regressions for the bypass above and other prefix
  lookalikes.

Draft PR [#21](https://github.com/plx/agentic-navigation-guide/pull/21)
contains prior-art code for the tag-boundary portion of this fix. It is based on
an older revision, is currently conflicting, and does not cover the full audit
scope.

### 4. Recursive verification succeeds after finding zero guides

When recursive discovery finds no guide, the CLI prints a warning and returns
success at [`src/cli/verify.rs`](../src/cli/verify.rs#L249). Under `--quiet`,
the success is silent. An existing integration test explicitly codifies this
behavior.

A deleted last guide, incorrect root, typo in `--guide-name`, or overly broad
exclusion can therefore disable a CI verification gate while keeping it green.

Required direction:

- make empty discovery fail by default;
- provide an explicit `--allow-empty` opt-out if a successful no-op is needed;
- preserve mode-appropriate diagnostics; and
- add tests for default, quiet, GitHub Actions, and explicit allow-empty modes.

### 5. Guide-file symlinks can cross the expected read boundary

Recursive traversal configures `follow_links(false)` but uses
`path.is_file()`, which follows a final file symlink, at
[`src/recursive.rs`](../src/recursive.rs#L54). A symlink named like a guide can
therefore point outside the search root and be read at
[`src/recursive.rs`](../src/recursive.rs#L124).

If parsing fails, the error formatter can echo the external target's first line
to logs at [`src/errors.rs`](../src/errors.rs#L251). This is particularly
concerning when verification runs automatically on an untrusted checkout.

Required direction:

- reject guide-file symlinks during implicit and recursive discovery by
  default, or constrain their canonical targets to an explicit trust boundary;
- decide separately whether an explicitly supplied guide symlink is an
  intentional opt-in;
- avoid echoing content from an untrusted/out-of-bound source; and
- add tests using in-root, out-of-root, relative, absolute, and dangling guide
  symlinks.

### 6. `init` can write through a dangling output symlink

`init` uses `Path::exists()` at
[`src/cli/init.rs`](../src/cli/init.rs#L44), which returns false for a dangling
symlink, and then calls `fs::write`. The write follows the symlink and creates
its unexpected target.

The separate existence check also has an ordinary check/write race that can
overwrite a file created between the two operations.

Required direction:

- use a single atomic `OpenOptions::create_new(true)` creation operation;
- reject symlink output paths, including dangling links;
- define behavior for existing directories and special files; and
- add regressions for dangling symlinks and a competing creator.

### 7. Current HEAD has no publishable release identity

Version `0.1.4` is already published on
[crates.io](https://crates.io/crates/agentic-navigation-guide), while
[`Cargo.toml`](../Cargo.toml#L3) still declares `0.1.4`. Current HEAD cannot be
republished under that version.

`cargo-semver-checks` against published `0.1.4` found:

- public exhaustive enum variants added;
- public exhaustive enum variants removed; and
- implicit public enum discriminants changed.

Under [Cargo's SemVer guidance][cargo-semver], this work should be released as
`0.2.0` unless the published `0.1.4` API is restored.

Published `0.1.4` also has contradictory licensing metadata: its Cargo metadata
declares MIT while its packaged root `LICENSE` contains BSD-3-Clause text.
Current HEAD is internally coherent as `MIT OR Apache-2.0`, but published crate
artifacts are immutable.

Required direction:

- decide whether the library API is intentionally supported;
- establish the intended `0.2.0` public surface;
- add a changelog and migration note for source-breaking changes;
- publish an explicit clarification covering the `0.1.x` licensing mismatch;
- make a considered maintainer/legal decision about whether yanking is useful;
  and
- require exact version/tag/source agreement for future releases.

## High-priority correctness and contract findings

### `ignore=true` semantics are inconsistent

The parser performs substantial list and grammar processing before the CLI
notices `ignore`, but the standalone public verifier does not inspect the flag.
Some invalid ignored examples fail, some semantic checks are skipped, and
library and CLI behavior diverge. This contradicts the README promise that all
syntax and semantic checks are skipped.

Relevant code:
[`src/parser.rs`](../src/parser.rs#L22),
[`src/verifier.rs`](../src/verifier.rs#L23), and
[`README.md`](../README.md#L178).

Define one contract and enforce it at a single architectural boundary. Tests
must cover every CLI execution mode and the public library entry points.

### Repeated trailing separators are normalized before validation

`trim_end_matches('/')` removes all trailing separators before the validator
can detect empty path components. As a result, forms such as `foo///` pass
syntax checking despite the documented path rules.

Relevant code:
[`src/parser.rs`](../src/parser.rs#L259).

### Quoted choice whitespace is not preserved

The parser trims choice tokens even when they came from quoted strings.
`x[" foo "]y` therefore refers to `xfooy`, not `x foo y`, contrary to the
documented grammar.

Relevant code:
[`src/parser.rs`](../src/parser.rs#L590).

### Placeholder matching can give platform-dependent false positives

Mentioned names are stored as exact strings, while normal filesystem lookup can
be case- or normalization-insensitive. On the case-insensitive audit
filesystem, a guide listing `README.md` plus an uncommented placeholder passed
against a filesystem containing only `Readme.md`: path lookup treated the named
entry as present, while placeholder bookkeeping treated the actual spelling as
unmentioned.

Relevant code:
[`src/verifier.rs`](../src/verifier.rs#L331).

The format must define whether exact directory-entry spelling is required or
whether both lookup and bookkeeping use a documented filesystem-aware
normalization policy.

### Exclusion patterns do not match nested basename expectations

Patterns are applied only to cumulative root-relative paths. Therefore:

- `--exclude target` does not exclude `project/target`; and
- the default `.git` exclusion does not exclude `project/.git`.

Relevant code:
[`src/dumper.rs`](../src/dumper.rs#L94) and
[`src/recursive.rs`](../src/recursive.rs#L80).

Choose basename-aware behavior for simple patterns, use explicit patterns such
as `**/.git`, or document exact root-relative glob semantics.

### Environment-backed arguments conflict instead of acting as defaults

Clap treats environment-provided values as if the corresponding command-line
arguments were explicitly supplied. Confirmed failures include:

- `AGENTIC_NAVIGATION_GUIDE_NAME` making non-recursive `verify` require
  `--recursive`;
- `AGENTIC_NAVIGATION_GUIDE_PATH` conflicting with recursive mode;
- an environment execution mode conflicting with an explicit execution-mode
  flag; and
- an environment log mode conflicting with `--quiet`.

Relevant definitions:
[`src/cli/mod.rs`](../src/cli/mod.rs#L20) and
[`src/cli/verify.rs`](../src/cli/verify.rs#L31).

Define and test explicit precedence: command line, then environment, then
built-in default.

### Public symlink verification is unfinished

`FilesystemItem::Symlink` is public, but the text parser never constructs it.
Its verifier branch is untested, treats dangling links as absent before
`symlink_metadata` can classify them, silently ignores some `read_link`
failures, and lossily converts targets.

Relevant code:
[`src/verifier.rs`](../src/verifier.rs#L140).

Either define textual symlink syntax and fully support it or remove the
premature public variant in the `0.2.0` API break.

### A public full-path helper returns only a local path

`NavigationGuide::get_full_path` claims to return the full hierarchical path
but returns only `item.path()`. A nested `src/main.rs` item returns `main.rs`.

Relevant code:
[`src/types.rs`](../src/types.rs#L116).

Draft PR [#21](https://github.com/plx/agentic-navigation-guide/pull/21)
contains prior art that removes this method. The `0.2.0` API decision should
determine whether it is removed or correctly implemented.

## Robustness, portability, and performance findings

### Numeric CLI parameters can panic or produce invalid output

- `--depth usize::MAX` overflows `max_depth + 1`; it panics in debug and wraps
  in release.
- `--indent usize::MAX` can panic with an allocation-capacity overflow.
- `--indent 0` silently flattens hierarchy.

Use checked arithmetic and sensible Clap value ranges.

### Broken pipe is handled as a panic

`dump` uses `print!` for stdout at
[`src/cli/dump.rs`](../src/cli/dump.rs#L63). If a downstream consumer closes
early, the release binary can panic and exit 101. Write through a fallible
buffered stdout handle and treat `BrokenPipe` as a normal termination.

### Quiet and GitHub Actions output contracts are inconsistent

- `--quiet init` still prints an unconditional success line.
- Recursive GitHub Actions errors print the guide path and line information on
  separate lines instead of the promised `path:line:` diagnostic.

Output modes need end-to-end snapshot or predicate tests for every command.

### Full tests run only on Linux

The CI test job uses Ubuntu at
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml#L16). Windows and
macOS jobs only build. This is inadequate for a product whose core behavior
depends on path parsing, separators, drive/UNC prefixes, case behavior,
normalization, symlinks, and permission semantics.

Run the behavioral suite on every supported OS and make platform-specific
expectations explicit.

### Hierarchy building and placeholder checks are quadratic

Hierarchy construction repeatedly scans backward and inserts at the front of a
vector. Optimized audit timings for a flat guide were:

| Entries | Time |
| ---: | ---: |
| 10,000 | 96 ms |
| 20,000 | 357 ms |
| 40,000 | 1.13 s |
| 80,000 | 4.43 s |
| 120,000 / approximately 1.09 MiB | 10.05 s |

Each placeholder also reopens and rescans its directory. A fixture with 1,000
plain files verified in approximately 16 ms, while 1,000 alternating
placeholders took approximately 358 ms.

Use a stack-based hierarchy builder and cache each directory's entry set once
per verification.

### Root containment is check-then-use

The verifier canonicalizes and checks containment, then performs later
operations through the original lexical path. An intermediate symlink can be
swapped between those operations.

For trusted, stable project trees this is a reasonable documented limitation.
If containment is intended as a security boundary against concurrent mutation,
use handle-relative traversal or revalidate filesystem identity after access.
The tool must not be described as a security sandbox without that stronger
implementation.

### One integration test is not hermetic

The `init` integration test omits `--root`, so it walks the whole checkout
instead of its temporary fixture. It failed during the audit while another
Cargo process was mutating `target/`, then passed alone.

Relevant test:
[`tests/cli_tests.rs`](../tests/cli_tests.rs#L454).

Pass the fixture root explicitly and separately decide how product code should
handle entries that disappear during enumeration.

## Public API and architecture assessment

The internal separation is suitable for continued development. The public
surface is too broad and unfinished to stabilize as-is:

- all core modules are public through [`src/lib.rs`](../src/lib.rs#L23);
- public error enums are exhaustive and have already changed incompatibly;
- the symlink variant is not constructible from the textual format;
- the full-path helper is incorrect; and
- convenience library functions have no direct coverage.

Before `0.2.0`, choose one of two explicit product contracts:

1. **Supported library and CLI.** Narrow and document the library surface,
   correct all exported behavior, use `#[non_exhaustive]` or private
   fields/builders where evolution is expected, add library-level tests, and
   gate future releases with `cargo-semver-checks`.
2. **Supported CLI only.** Make implementation modules and unfinished types
   private, document that the binary is the supported interface, and treat any
   retained library facade as deliberately minimal.

## CI and supply-chain findings

- Main CI pins actions to immutable commit SHAs, but
  [`.github/workflows/verify-guide.yml`](../.github/workflows/verify-guide.yml#L14)
  and both Claude workflows use mutable action tags.
- The Claude workflows pass an OAuth token and grant `id-token: write` to a
  mutable third-party action.
- Checkout credentials are not explicitly disabled.
- Workflow permissions, timeouts, and concurrency are not consistently
  minimized.
- There is no automated MSRV, advisory, package, SemVer, fuzz, property-test,
  or release-artifact smoke-test gate.
- Repository rules prevent force-push/deletion but do not require CI or review
  on the default branch.

Pin reviewed actions, use `persist-credentials: false`, minimize permissions,
and add release-quality gates before workplace adoption.

## MSRV, dependency, and package findings

### MSRV is neither declared nor enforced

[`Cargo.toml`](../Cargo.toml#L1) has no `rust-version`, while
[`.clippy.toml`](../.clippy.toml#L2) claims Rust 1.70. Cargo 1.70 cannot parse
the current lockfile. The locked project passed on Rust 1.78, while a fully
refreshed dependency graph required Rust 1.85 during the audit.

Choose a real floor, declare it with Cargo's
[`package.rust-version`][cargo-rust-version], align Clippy, and test the floor
in CI. Installation documentation should recommend an exact version with
`--locked`.

### The package contains unrelated repository content

`cargo package --list` returned 79 files. The package unnecessarily contains:

- the entire in-progress `site/`;
- `.github` workflows;
- agent/context documentation; and
- completed remediation planning notes.

Add a focused `[package].include` allowlist or explicit exclusions. This
finding concerns the published Rust artifact only; the site itself was not
reviewed.

### Dependency health is otherwise good

- The locked graph had no known RustSec advisories.
- Dependencies were checksummed crates.io packages; no Git dependencies were
  present.
- No duplicate normal-dependency versions were found.
- Regenerated `THIRD_PARTY_LICENSES.md` matched the committed file.
- A dry-run compatible dependency refresh and attribution regeneration
  succeeded during the audit.

Add Dependabot or Renovate and an advisory/license policy gate so this state is
maintained.

## Release engineering and distribution findings

- `0.1.4` is already on crates.io.
- There are no Git tags or GitHub Releases.
- There is no reproducible release workflow.
- There are no binary archives, checksums, SBOM, provenance, or signatures.
- The sole crates.io owner is a recovery and continuity risk.
- No Homebrew formula or cask was found.

The pre-release workflow should:

1. enforce version/tag agreement;
2. run locked tests on all supported operating systems;
3. run formatting, all-target Clippy, and rustdoc with warnings denied;
4. audit dependencies and regenerate license attribution;
5. verify the Cargo package contents;
6. run SemVer checks against the prior release;
7. smoke-test the exact release artifacts;
8. publish through crates.io Trusted Publishing where practical; and
9. create immutable GitHub release artifacts with SHA-256 checksums.

Homebrew/core is not presently realistic. At the audit date, the repository had
four stars and no forks, below the official
[Homebrew package-acceptance thresholds][homebrew-acceptance]. A personal
third-party tap is the practical route after `0.2.0`; its formula should use an
immutable checksummed source, the packaged Cargo lockfile, a Rust build
dependency, and a functional verification test.

## Documentation and governance findings

- The README quickstart invokes `init` without the required `--output` at
  [`README.md`](../README.md#L205).
- The README lacks complete installation, upgrade, uninstall, supported
  platform, MSRV, CLI/environment/default, exit-code, and stability policy
  documentation.
- Its CI example uses mutable action tags and an unlocked, unversioned
  `cargo install`.
- A sentence has a stray trailing asterisk.
- The project is still described as an early preview; that language needs to be
  reconciled with the intended `0.2.0` support promise.
- `Specification.md` is historical but easy to mistake for a normative
  contract and contradicts current behavior.
- There is no `CHANGELOG.md`, `SECURITY.md`, or `CONTRIBUTING.md`.
- There are no issue/PR templates, protected release tags, or required status
  checks.
- The crate has one owner.

Draft PR [#21](https://github.com/plx/agentic-navigation-guide/pull/21)
contains useful prior art for README cleanup and retiring the historical
specification. It should be reconciled with, extracted into, or superseded by
the issue-driven remediation program rather than silently duplicated.

## Highest-value missing tests

1. Property-generated filesystem trees asserting
   `dump -> parse -> validate -> verify`, including grammar metacharacters,
   Unicode, whitespace, symlinks, special files, empty roots, and exclusions.
2. Marker and attribute fuzzing, especially exact prefix boundaries and
   malformed/duplicate `ignore`.
3. Arbitrary UTF-8 parser fuzzing with never-panic and bounded-time assertions.
4. Platform-matrix tests for case, Unicode normalization, drive and UNC paths,
   separators, permissions, and symlinks.
5. Atomic-output tests involving dangling symlinks and concurrent creators.
6. Full `FilesystemItem::Symlink` tests if that public surface is retained.
7. Benchmarks for flat, wide, and deep guides, choice expansion, wide directory
   dumps, and many placeholders.
8. Recursive discovery tests for file symlinks, nested basename exclusions,
   deterministic ordering, permission errors, transient disappearance, and
   empty discovery.

Every bug ticket created from this audit should name at least one regression
test that fails on the audited revision and passes only after the fix.

## Recommended remediation sequence

1. Define the authoritative grammar and filesystem contract:
   representable names, escaping, entry types, symlinks, case behavior,
   `ignore`, exclusions, and empty discovery.
2. Fix the fail-open and false-positive release blockers and add exact
   regressions.
3. Establish generated round-trip/property tests and cross-platform behavioral
   CI.
4. Finalize the supported library/API surface and make the intentional breaking
   changes for `0.2.0`.
5. Declare the MSRV and harden workflows, packaging, and release automation.
6. Repair user-facing documentation and add the minimum governance/security
   documents.
7. Execute the repository's post-remediation production-readiness reassessment
   playbook independently.
8. Publish `0.2.0` only if that reassessment returns a production-suitable
   verdict.
9. Publish a third-party Homebrew tap; consider Homebrew/core only after
   independent adoption satisfies its acceptance policy.

## Audit limitations

- The static site and landing page were out of scope.
- Local adversarial tests ran on macOS. CI history was inspected, but the audit
  did not execute the full suite on actual Windows and Linux hosts.
- The review was not a formal penetration test.
- Licensing observations are technical release findings, not legal advice.
- Concurrent filesystem mutation was analyzed and selectively reproduced, not
  exhaustively model-checked.
- External service state, ecosystem policy, repository metrics, dependencies,
  and advisories are time-sensitive and must be rechecked at release time.

## Final conclusion

The implementation is substantially stronger than an unreviewed experimental
project, and ordinary happy paths are already well covered. The blocking
concern is narrower but fundamental: several edge cases return success after
parsing a different guide, generating a non-round-trippable guide, honoring a
malformed ignore marker, or discovering no guide.

Do not abandon or rewrite the project. Complete the issue-driven `0.2.0`
hardening program, then perform the rigorous reassessment described in
`audits/production-readiness-reassessment-playbook.md`. A successful
reassessment should require evidence that every false-positive and fail-open
path has a regression test, generator output is safe by construction, supported
platforms run behavioral tests, the public API and format contracts are
explicit, and the exact release artifacts are reproducible and traceable.

[cargo-rust-version]: https://doc.rust-lang.org/stable/cargo/reference/rust-version.html
[cargo-semver]: https://doc.rust-lang.org/cargo/reference/semver.html
[homebrew-acceptance]: https://docs.brew.sh/Package-Acceptance-Policy
