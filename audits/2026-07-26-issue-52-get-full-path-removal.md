# Issue #52 `get_full_path` API-Removal Evidence

Date: 2026-07-26

## Scope and Selected Outcome

Issue #52 owns one exact disposition from the approved #36 Rust-surface
decision:

```text
api-method-navigation-guide-get-full-path
NavigationGuide::get_full_path(&self, item: &NavigationGuideLine) -> PathBuf
RemoveIncorrectMethod
owner #52
```

The method is removed without replacement. It is not made private, hidden,
deprecated, or feature-gated. No backward tree search, pointer-identity lookup,
or path-carrying traversal API is introduced.

The method accepted an arbitrary borrowed `NavigationGuideLine`, which may be
detached or cloned and carries no parent identity. Its implementation returned
only `item.path()`, so a nested `src/main.rs` item produced `main.rs`. Retaining
the signature could not provide an unambiguous full path in the presence of
duplicate local names.

## Revisions

- Audited pre-fix production commit:
  `6b82b06bf2de4acb3000445fdf5274a2319a024b`
- Tests-first commit:
  `309e785f2bcfaa0d430b446f256f0e665637f580`
- Removal implementation commit:
  `319b5d347d7770c59ca62d140283dfcbac6fc675`

## Red-Before Evidence

The tests-first commit made the selected removal normative before changing
production source.

```sh
cargo test --test issue_52_api_removal --locked -- --nocapture
```

The command exited 101. `cargo package` succeeded, its verified unpacked
artifact resolved as a downstream path dependency, and the method-calling
consumer compiled. The focused assertion therefore failed with:

```text
the packaged crate still lets a downstream consumer call get_full_path
```

The independent source/API checks also exited 101:

```sh
cargo test --test v0_2_contract --locked issue_52 -- --nocapture
cargo test --test v0_2_contract --locked \
  api_ledger_matches_current_rust_source_and_cargo_target -- --nocapture
```

They reported that the incorrect method was still present and that current
source had one extra method relative to the selected realized-removal set.

## Executable Removal Evidence

### Whole-definition absence

`issue_52_removed_full_path_method_is_absent_but_its_ledger_row_remains`
parses `src/types.rs` with `syn` and inspects every inherent
`NavigationGuide` method at every visibility. This prevents a passing result
from privatizing or hiding `get_full_path` instead of deleting it.

The broader current-source collector independently omits exactly the IDs in
`REALIZED_API_REMOVAL_IDS`. That exact set currently contains only:

```text
api-method-navigation-guide-get-full-path
```

Every other transitional Rust symbol still has to match the audited #36
snapshot.

### Frozen 132-row decision ledger

`tests/fixtures/v0_2_api.rs` is byte-identical to the pre-change file. It still
contains:

- 132 `ApiCase` rows;
- 25 historical method rows;
- exactly one #52-owned row; and
- the exact `api-method-navigation-guide-get-full-path` ID, signature,
  `RemoveIncorrectMethod` disposition, and owner.

The current Cargo metadata must still expose exactly one legacy library
target. Removing that target or making the other 129 #54-owned rows internal
remains #54 work.

### Packaged downstream positive control and negative consumer

`issue_52_packaged_downstream_consumer_cannot_call_removed_method` runs the
real locked, offline `cargo package` workflow in an isolated target directory.
It resolves the verified unpacked artifact as a path dependency. A positive
control first imports and constructs the transitional model without the
removed call and must compile successfully. The negative consumer changes only
the final expression to call `get_full_path`.

The post-removal build must fail specifically with the compiler diagnostic
containing both:

```text
error[E0599]
no method named `get_full_path`
NavigationGuide
```

The positive control and exact, color-disabled diagnostic prove that the
package and type resolved and the selected method alone is unavailable. This
is negative migration evidence for the temporary linkable source package, not
a supported v0.2 Rust facade. #54 owns migrating or retiring this
method-specific test when it removes the library target; #62 owns the final
binary-only packaged-consumer proof.

## Migration and Documentation

The README's dated v0.2 divergence record now names the removal, explains why
`NavigationGuideLine::path()` is not a replacement, and records both approved
choices for existing Rust callers:

- invoke the installed CLI through its documented process or machine contract;
  or
- remain pinned to immutable, unsupported `0.1.4` at their own risk.

The normative contract preserves the historical row while distinguishing the
frozen #36 inventory from current source after realized removals. Deleting the
method also deletes its stale rustdoc promise; warning-denied rustdoc contains
no `get_full_path` entry.

Formal `0.2.0` release notes remain #64's responsibility. The docs.rs badge and
package documentation metadata remain #66 work.

## Prior Art and Scope Boundaries

Draft PR [#21](https://github.com/plx/agentic-navigation-guide/pull/21) is
non-authoritative prior art. Only its final deletion rationale is adopted:
detached or cloned line identity makes this signature ambiguous, and the
silent local-path fallback is incorrect. Its parser, CLI, README,
`Specification.md`, and other API changes remain outside #52 and under #68's
historical disposition.

This change also does not:

- remove `FilesystemItem::Symlink` or its error variant (#53);
- alter the library target, re-exports, visibility, or other API rows (#54);
- create the complete migration changelog or SemVer baseline (#64); or
- alter the docs.rs/site documentation surface (#66).

## Validation

| Command | Result |
| --- | --- |
| `cargo test --locked issue_52 -- --nocapture` | Pass: packaged positive control, negative consumer, and exact source/ledger test |
| `GUIDE_FORMAT_REQUIRE_CONFORMANCE=all cargo test --test v0_2_contract --locked -- --nocapture` | Pass: 43 tests; frozen 132-row ledger |
| Debug all-target/all-feature suite | Pass: 329 tests, 2 intentional ignores |
| Release all-target/all-feature suite | Pass: 329 tests, 2 intentional ignores |
| Strict all-target/all-feature Clippy | Pass |
| Warning-denied rustdoc and generated-doc search | Pass: no live `get_full_path` entry |
| Windows GNU all-target check and strict Clippy | Pass |
| CLI check and verification of `AGENTIC_NAVIGATION_GUIDE.md` | Pass |
| `just test-production-readiness-selector` | Pass: 61 tests |
| `actionlint`, `just --fmt --check`, and `git diff --check` | Pass |
| `lychee` on the changed maintained documents | Pass: 30 links, 0 errors |
| CI-equivalent `cargo about generate` comparison | Pass: `THIRD_PARTY_LICENSES.md` unchanged |
