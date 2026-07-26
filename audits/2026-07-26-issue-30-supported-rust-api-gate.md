# Issue #30 Supported Rust API and SemVer Gate Evidence

Date: 2026-07-26

## Gate Result

**PASS.** The v0.2 supported-product and Rust API workstream is complete:

- the installed `agentic-navigation-guide` CLI is the sole supported v0.2
  product;
- current source and the packaged crate are binary-only, with exactly one
  named product binary and zero Rust-linkable targets;
- there is no supported, hidden, feature-gated, unstable, or test-only Rust
  facade or in-process shim;
- every entry in the frozen 132-row current-source decision inventory has its
  approved realized disposition;
- the incorrect full-path method and unsupported link model are removed
  without replacement;
- the complete immutable published-`0.1.4` Rust surface is recorded separately
  as a 128-entry migration baseline;
- `0.2.0` is the deliberate breaking boundary, and the documented,
  machine-recorded policy requires every supported-surface incompatibility in
  a later `0.2.x` candidate to fail; and
- adding a supported Rust library or accepting another breaking CLI-contract
  change requires a new `0.3.0` boundary and approved migration record.

This is an aggregate evidence gate. It does not change runtime behavior,
publish a crate, create a tag, or create a GitHub Release.

The evaluated component baseline is clean `main` at
`d0e03017f188bbd6364d2489877bb332db81d109`. The gate branch changes only this
audit and the navigation-guide index. Its final audit-only head is recorded by
the dedicated PR and must pass exact-head local and hosted checks before merge.
Embedding the gate's own commit identity in this file would be
self-referential.

## Native Readiness

The live selector chose issue #30 from current `main` at
`d0e03017f188bbd6364d2489877bb332db81d109`. GitHub reported:

- parent: open program gate #26;
- native blockers: none;
- native children: exactly #36, #52, #53, and #54;
- closed native children: four of four; and
- downstream gate blocked by #30: #72.

Each child closed through exactly one merged default-branch PR:

| Child | Outcome | Merged PR | Squash commit |
| --- | --- | --- | --- |
| #36 | Approved CLI-only/binary-only product and compatibility decision | [#86](https://github.com/plx/agentic-navigation-guide/pull/86) | `805dd11ba57ea4fd90d80365102c927a5ee8227c` |
| #52 | Removed `NavigationGuide::get_full_path` without replacement | [#104](https://github.com/plx/agentic-navigation-guide/pull/104) | `c40588644df69c6438f40a6fce64ddc840b59059` |
| #53 | Removed `FilesystemItem::Symlink` and `SemanticError::SymlinkTargetMismatch` without replacement | [#105](https://github.com/plx/agentic-navigation-guide/pull/105) | `e34399c14683878064cad18e9506186cd7e4fef1` |
| #54 | Removed the library target and made every other owned export implementation-only | [#106](https://github.com/plx/agentic-navigation-guide/pull/106) | `f58323f8fd83860e86ab6628e8630a2fe8c6c923` |

PR #82 is a closed, unmerged selector smoke fixture, not #30 decision or
implementation evidence. The applicable supported-product decision is merged
PR #86.

Supporting release-identity issue #64 subsequently closed through merged
[#107](https://github.com/plx/agentic-navigation-guide/pull/107), squash
`d0e03017f188bbd6364d2489877bb332db81d109`. It supplies the complete
published-`0.1.4` migration notes and future SemVer baseline controls required
by this gate's final criterion.

The decision and implementation also consumed already-landed cross-workstream
inputs: #35 / PR #85 for filesystem trust and link rejection, #37 / PR #87 for
correct hierarchy construction, #39 / PR #92 for a distinct CLI ignored
outcome and non-vacuous no-facade assertion, and #49 / PR #91 for the shared
safe guide opener. They are not extra #30 native children, but their binding
contract rows remain represented in the all-owner run below, and their focused
regressions remain covered by the full workspace suite.

## Approved Product Decision

The repository owner explicitly approved both #36 decision groups in
[PR #86 comment 5081608840](https://github.com/plx/agentic-navigation-guide/pull/86#issuecomment-5081608840):

1. CLI-only and binary-only v0.2, no Rust facade or shim, deliberate `0.2.0`
   break, complete CLI compatibility throughout `0.2.x`, `0.3.0` for a
   supported library or another break, and a narrow restore-not-redefine
   security rule.
2. The exact 132-entry current-source disposition inventory, separation from
   the immutable published baseline, focused implementation owners, and
   package/release/documentation evidence design.

The approved handoff was propagated to every named dependent issue and the
native graph was independently re-audited in
[comment 5081621365](https://github.com/plx/agentic-navigation-guide/pull/86#issuecomment-5081621365).

The normative decision remains in
[`docs/v0.2-contract.md`](../docs/v0.2-contract.md#supported-product-and-rust-api).
Concise product and migration language is maintained in
[`README.md`](../README.md), while
[`release/identity.toml`](../release/identity.toml) is the exact
machine-readable prepared identity and baseline policy.

## Completion-Criteria Mapping

### Supported surfaces and `0.x` policy are explicit

**Pass.** The only supported v0.2 product is the installed CLI. Its documented
commands, option and argument shapes, defaults and precedence, stdout/stderr
placement, semantic outcomes, exit status, machine formats, guide grammar,
platform scope, and trust boundary are the `0.2.x` compatibility surface.

There are zero supported Rust symbols. A later supported library or accepted
breaking CLI-contract change requires `0.3.0`. A narrow security correction
may restore conformance to an existing documented boundary within `0.2.x`; it
may not redefine that boundary.

### Every retained public behavior is correct and directly tested

**Pass.** No Rust library behavior is retained, so there is no downstream Rust
entry point whose correctness can be asserted by an in-process consumer. This
is not an empty or documentation-only inference:

- real workspace and unpacked-package Cargo metadata expose zero linkable
  target kinds and exactly one named binary;
- `src/lib.rs` is absent;
- whole-source AST checks reject unrestricted `pub` definitions;
- the frozen inventory and focused AST regressions prove the complete
  `get_full_path` and link-model definitions are absent;
- the non-vacuous `NoSupportedLibraryFacade` conformance assertion proves
  that none of the complete 132 frozen decision rows retains a supported
  disposition; and
- the exact generated package is installed into an isolated Cargo environment
  and its CLI is executed successfully.

The Cargo target and whole-tree AST checks, rather than that disposition
assertion alone, reject an additional or hidden facade.

Before the library target was removed, #52 and #53 used real packaged positive
controls and focused negative consumers to prove that only their selected
method or variants became unavailable. #54 then replaced those transitional
consumers with exact binary-only workspace/package target-shape proof.

Issue #62 still owns the final release-candidate path-dependent negative
consumer and complete package smoke suite. That release-boundary proof is not
silently claimed by this component gate.

### Unsupported implementation modules and types are private

**Pass.** #54 deleted `src/lib.rs`, moved module ownership into the binary, and
made required definitions private or `pub(crate)`. The package regression
checks every workspace and packaged Rust source file, rejects unrestricted
public definitions, and rejects `lib`, `rlib`, `dylib`, `cdylib`, `staticlib`,
and `proc-macro` in both Cargo `kind` and `crate_types`.

It also rejects a missing, renamed, or additional product binary. Test targets
are not product or library targets.

### Extensible public data uses an intentional compatibility strategy

**Pass by deliberate absence.** v0.2 supports no public Rust structs, enums,
aliases, traits, trait implementations, serde representation, or `Display`
wording. Internal data may evolve without a downstream Rust SemVer promise.

Reintroducing a library requires a separately reviewed contract with opaque
requests/results, non-exhaustive error categories, the shared safe opener,
packaged downstream tests, and a new Rust API baseline under `0.3.0`.

### Release notes and SemVer evidence reflect the transition

**Pass.** The immutable published `0.1.4` crate is the one-time Rust migration
baseline for `0.2.0`, distinct from the last-linkable development revision
`e34399c14683878064cad18e9506186cd7e4fef1`.

Pinned `cargo-semver-checks 0.49.0` exited `100` when comparing that exact
last-linkable source with published `0.1.4`. Its 196 evaluated checks produced
192 passes and four major failures:

```text
enum_no_repr_variant_discriminant_changed
enum_variant_added
enum_variant_missing
inherent_method_missing
```

The final binary-only candidate does not misuse an empty library selection as
a passing SemVer result. Target shape, complete migration inventory, and the
documented CLI contract are the ongoing gates.

[`CHANGELOG.md`](../CHANGELOG.md#rust-source-compatibility-complete-removal-of-published-014)
records every entry in the separate 128-row published `0.1.4` inventory,
published trait commitments, the no-shim process migration, no-replacement
removals, the deliberate break, and future compatible-line/breaking-line
baseline selection.

## Two Deliberately Distinct Inventories

The differing counts are intentional:

- `tests/fixtures/v0_2_api.rs`
  - Rows: 132
  - Purpose: frozen #36 current-source decision snapshot with one disposition
    owner per row
  - SHA-256:
    `1e2515e134a61e2dd297d3de3127daee5074c75f70e29f53ca176af4fe60110a`
- `tests/fixtures/v0_1_4_published_api.tsv`
  - Rows: 128
  - Purpose: immutable published-`0.1.4` to `0.2.0` migration baseline
  - SHA-256:
    `f1263f88e72ae790e62a474f48b18745963c72b921a8f299e0afb9286313f3a7`

The 132-row ledger preserves later development additions and all approved
owners: one row for #52, two for #53, and 129 for #54. The 128-row fixture
instead records exactly what crates.io users could consume from published
`0.1.4`. Neither is regenerated from current source after removal.

## Retained Evidence

- [#52 removal audit](./2026-07-26-issue-52-get-full-path-removal.md):
  `c378e8115b1a526347c4dd51347ec992f9da571c1b7415a39217c89cdb953660`
- [#53 removal audit](./2026-07-26-issue-53-symlink-model-removal.md):
  `9afaf8d11b7b755378a21c81ead22544da97396cecfe70b2b5dc2789ae0cbdcc`
- [#54 binary-only/SemVer audit](./2026-07-26-issue-54-binary-only-package.md):
  `b7db882a03c3f19bf2b194c6fcf2f1ab504a99cbd8425cbde6327d86ad7c2313`
- [#64 release-identity audit](./2026-07-26-issue-64-release-identity.md):
  `2db026fae0b266a109688d32ace317991384df8d844a5f33e77d29a0dc3c1425`
- [Machine-readable release identity](../release/identity.toml):
  `5d940f7a35149b019b89f60111c5d6da4aae780d0dbb27ff89ae533bc738855f`
- [Complete prepared changelog](../CHANGELOG.md):
  `e91f1dbc511bc2f97446742c41dc60f178565e22114321da19196fff33d61e1f`

These are the exact evidence hashes at gate evaluation. The #64 identity file
separately pins the #54 audit and SemVer tool identity so later incidental
editing cannot silently replace the migration evidence.

## Red-Before Evidence

No new behavioral defect is fixed by this aggregate gate PR, so a new
red-before test is not applicable. Creating a failing gate-only commit would
weaken the already green child controls.

The underlying defects retain their exact red-before evidence:

- #52: packaged downstream consumer still called `get_full_path`; the focused
  test exited `101`.
- #53: two packaged consumers could still name both unsupported variants; the
  focused test exited `101`.
- #54: workspace and unpacked-package metadata still exposed a library target,
  and 128 owned exports remained reachable; both focused tests exited `101`.
- #64: Cargo, lockfile, metadata, built CLI, and exact installed package still
  reported `0.1.4`, while the changelog was absent; the checker and Rust test
  exited `1` and `101`.

The dated child audits above preserve each exact command, test overlay or
tests-first revision, exit status, and concise observed failure.

## Validation

The gate branch passes:

```sh
cargo metadata --locked --no-deps --format-version 1
cargo test --locked --bin agentic-navigation-guide issue_52 -- --nocapture
cargo test --locked --bin agentic-navigation-guide issue_53 -- --nocapture
cargo test --locked --bin agentic-navigation-guide issue_54 -- --nocapture
cargo test --locked --test issue_54_binary_only_package -- --nocapture
cargo test --locked --test issue_64_release_identity -- --nocapture
GUIDE_FORMAT_REQUIRE_CONFORMANCE=all \
  cargo test --locked --bin agentic-navigation-guide \
  'v0_2_contract_tests::' -- --nocapture
PYTHONDONTWRITEBYTECODE=1 \
  python3 -m unittest tests/test_check_release_identity.py -v
python3 scripts/check_release_identity.py --tag v0.2.0
cargo test --workspace --all-targets --all-features --locked --no-fail-fast
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo package --locked
cargo run --locked -- check --guide AGENTIC_NAVIGATION_GUIDE.md
cargo run --locked -- verify --guide AGENTIC_NAVIGATION_GUIDE.md --root .
just --fmt --check
just test-production-readiness-selector
actionlint
markdownlint audits/2026-07-26-issue-30-supported-rust-api-gate.md
lychee --no-progress \
  AGENTIC_NAVIGATION_GUIDE.md \
  audits/2026-07-26-issue-30-supported-rust-api-gate.md
git diff --check
```

The focused #52, #53, and #54 binary-unit filters pass 1, 1, and 2 tests. The
package target proof passes 1 test, release-identity proof passes 2, binding
all-owner contract passes 44, and checker mutation suite passes 14. The full
workspace/all-target/all-feature suite passes 332 tests with two intentionally
ignored manual scaling benchmarks.

The exact package command is run from a clean detached worktree because
Conductor's ignored `.context/todos.md` is intentionally preserved in the
active worktree. The detached worktree is pinned to the exact gate head and is
left clean by packaging.

## Residual Ownership

Closing this component gate does not certify the final release candidate or
the whole CLI as production-ready. Existing workflow issues retain:

- #58: hermetic tests and direct binary-only support-boundary coverage;
- #59: coverage, mutation, and performance effectiveness;
- #62: exact final package allowlist, installed smoke suite, and fail-closed
  no-library consumer;
- #63: trusted publishing and release enforcement;
- #66: maintained package documentation metadata and user examples;
- #67: complete combined support contract;
- #68: final PR #21 and `Specification.md` historical disposition; and
- #72: independent post-remediation candidate audit.

Those are release-program evidence layers, not missing #30 child
implementations. This gate makes no claim that they have passed.
