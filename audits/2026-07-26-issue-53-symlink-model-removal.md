# Issue #53 Symlink-Model API-Removal Evidence

Date: 2026-07-26

## Scope and Selected Outcome

Issue #53 owns two exact dispositions from the approved #36 Rust-surface
decision:

```text
api-variant-filesystem-item-symlink
FilesystemItem::Symlink {
    path: String,
    comment: Option<String>,
    target: Option<String>,
}
RemoveUnsupportedLinkModel
owner #53

api-variant-semantic-error-symlink-target-mismatch
SemanticError::SymlinkTargetMismatch {
    line: usize,
    path: String,
    expected: String,
    actual: String,
}
RemoveUnsupportedLinkModel
owner #53
```

Both variants are removed without replacement. Neither remains hidden,
private-looking, feature-gated, deprecated, or available as an unsupported
construct.

The parser had no textual link or target-matching syntax, so it could never
construct the public model. The dumper rejected links instead of emitting the
model. The legacy verifier branch also treated dangling links inconsistently,
silently ignored `read_link` failures, converted targets lossily, and
conflicted with the approved Windows reparse classification.

## Revisions

- Audited pre-fix production commit:
  `c40588644df69c6438f40a6fce64ddc840b59059`
- Tests-first commit:
  `af327df83c9ebd9ac400deb4e8ed56a79dd1512d`
- Removal implementation commit:
  `31b1fb32acb16fdc9b2f2c183cf589cf03278e20`

## Red-Before Evidence

The tests-first commit made both selected removals normative before production
source changed.

```sh
cargo test --test issue_53_api_removal --locked -- --nocapture
```

The command exited 101. The real locked, offline `cargo package` workflow and
surviving-variant positive control succeeded. Both negative consumers also
compiled, proving both selected variants were still available:

```text
the packaged crate still exposes at least one selected variant
FilesystemItem::Symlink status: exit status: 0
SemanticError::SymlinkTargetMismatch status: exit status: 0
```

The exact source/ledger test also exited 101:

```sh
cargo test --test v0_2_contract --locked issue_53 -- --nocapture
```

It reported:

```text
#53's unsupported variant is still exported:
FilesystemItem::Symlink {
    path: String,
    comment: Option<String>,
    target: Option<String>,
}
```

The broader current-source comparison independently exited 101 because the
observed source contained both variants beyond the selected realized-removal
set.

## Executable Removal Evidence

### Whole-definition absence

`issue_53_removed_symlink_model_is_absent_but_its_ledger_rows_remain`
parses both owning source files with `syn`. It rejects either variant name
remaining in `FilesystemItem` or `SemanticError`, independent of rustdoc,
features, or call reachability.

The broader source collector omits only the three exact realized IDs: #52's
previous method removal and #53's two variant removals. It still requires every
other transitional Rust symbol and the relative order of every remaining enum
variant to match the audited #36 snapshot.

### Frozen 132-row decision ledger

`tests/fixtures/v0_2_api.rs` remains byte-identical to the pre-change file. Its
SHA-256 is:

```text
1e2515e134a61e2dd297d3de3127daee5074c75f70e29f53ca176af4fe60110a
```

It still contains:

- 132 `ApiCase` rows;
- 39 historical variant rows;
- exactly two #53-owned rows;
- the exact two selected IDs, signatures,
  `RemoveUnsupportedLinkModel` dispositions, and owner; and
- the unchanged ownership split: one row for #52, two for #53, and 129 for
  #54.

The source ledger subtracts the realized variants without deleting their
historical dispositions. The library target, root re-exports, public enum
types, and all other #54-owned rows remain for #54.

### Packaged downstream positive control and negative consumers

`issue_53_packaged_downstream_consumers_cannot_name_removed_variants` runs a
real locked, offline `cargo package` in an isolated target directory and
resolves its verified unpacked artifact as a path dependency.

A positive consumer imports both enums and pattern-matches surviving
`FilesystemItem::File` and `SemanticError::TypeMismatch` variants. It must
compile first. Two independent negative sources then change only one selected
variant name at a time. Each must fail with a color-disabled diagnostic
containing:

```text
error[E0599]
no variant named `<selected variant>`
<owning enum name>
```

The positive control prevents a missing package, re-export, enum, or library
target from masquerading as successful removal. Independent negative checks
prove each selected name is unavailable rather than accepting the compiler's
first error from a source that names both.

This is temporary negative migration evidence for the linkable source
package, not a supported v0.2 facade. #54 owns migrating or retiring this
test when it removes the library target; #62 owns the final binary-only
packaged-consumer proof.

## Runtime Trust Boundary Preserved

Deleting the programmatic model does not make filesystem links acceptable.
The unchanged internal `entry_type` classifier still distinguishes symbolic
links and Windows reparse points so the CLI can reject them without following
or traversal. The #42 generation and textual verification operations, #49
safe guide opening, and #51 containment/revalidation tests remain executable.

The removal deletes only:

- the two enum definitions;
- their `path`, `comment`, and `line_number` match alternatives;
- the legacy dangling-link `exists()` precheck;
- the unreachable verifier target-matching branch, including its swallowed
  `read_link` error and lossy target conversion; and
- exhaustive test arms that became impossible.

It does not alter `src/entry_type.rs`, the #42 operation ledger, guide input,
output handling, containment policy, or link/reparse diagnostics for textual
file and directory entries.

## Migration and Documentation

The README's dated v0.2 divergence record names both removals and records that
they have no replacement. Existing Rust consumers may migrate supported
regular-file and directory workflows to the installed CLI's documented
process or machine contract, or remain pinned to immutable, unsupported
`0.1.4` at their own risk.

The guide language has no link-inventory or target-matching form. Callers must
not encode links as ordinary files or directories. Internal link/reparse
classification remains rejection-only and is not a replacement public API.

The normative contract preserves both historical rows while marking #53's
disposition realized. #64 owns the complete published-to-`0.2.0` changelog;
issue #66 owns documentation hosting and package metadata.

## Prior Art and Scope Boundaries

Draft PR [#21](https://github.com/plx/agentic-navigation-guide/pull/21) is not
symlink-model prior art: it removes `get_full_path` but leaves both #53
variants. Its `Specification.md` and unrelated documentation changes remain
issue #68 work.

The [PR #99 Windows review finding](https://github.com/plx/agentic-navigation-guide/pull/99#discussion_r3653033516)
observed that the retained programmatic branch no longer accepted Windows
symlinks after reparse-first classification. The approved resolution is to
delete that unsupported branch, not to add a Windows compatibility shim.
Real-platform link/reparse execution remains #55.

Historical descriptions in `Specification.md` and earlier dated audits remain
unchanged as provenance. No standalone follow-up issue is required.

## Validation

| Command | Result |
| --- | --- |
| `cargo test --locked issue_53 -- --nocapture` | Pass: packaged positive control, two negative consumers, and exact source/ledger test |
| Focused #42, #49, and #51 tests | Pass: rejection, guide-input, and containment boundaries preserved |
| `GUIDE_FORMAT_REQUIRE_CONFORMANCE=all cargo test --test v0_2_contract --locked -- --nocapture` | Pass: 44 tests; frozen 132-row ledger |
| Debug all-target/all-feature suite | Pass: 331 tests, 2 intentional ignores |
| Release all-target/all-feature suite | Pass: 331 tests, 2 intentional ignores |
| Strict all-target/all-feature Clippy | Pass |
| Warning-denied rustdoc and generated-doc search | Pass: no live removed-variant entries |
| Windows GNU all-target check and strict Clippy | Pass |
| CLI check and verification of `AGENTIC_NAVIGATION_GUIDE.md` | Pass |
| `just test-production-readiness-selector` | Pass |
| `actionlint`, `just --fmt --check`, and `git diff --check` | Pass |
| `lychee` on the changed maintained documents | Pass |
| CI-equivalent `cargo about generate` comparison | Pass: `THIRD_PARTY_LICENSES.md` unchanged |
