# Issue #54 Binary-Only Rust Package Evidence

Date: 2026-07-26

## Scope and Approved Outcome

Issue #54 realizes the approved #36 supported-product decision:

- the installed `agentic-navigation-guide` CLI is the sole supported v0.2
  product;
- current source and the packaged crate contain no Rust-linkable library
  target or in-process shim;
- all 129 #54-owned historical inventory rows are realized: one library target
  is removed and the other 128 exports are private, `pub(crate)`, or deleted;
- no hidden, unstable, feature-gated, test-only, or otherwise unsupported
  library target remains; and
- the frozen decision inventory remains byte-identical historical evidence.

The approved owner comment on #54 supersedes the issue body's earlier generic
recommendation to retain and stabilize a smaller public facade. Consequently,
the absence of downstream Rust entry points is the intended result, not missing
documentation or test coverage.

This change does not define the final packaged negative consumer, release
metadata, complete migration guide, or final support-documentation audit.
Those remain separately owned below.

## Evidence Revisions

- Exact last commit whose source exposed a linkable library:
  `e34399c14683878064cad18e9506186cd7e4fef1`
- Binary-only implementation and migrated-test commit:
  `f94f7554968612fe1e30909431c56730aa6c152f`
- Immutable published baseline version: `0.1.4`

The branch was created directly from the exact last-linkable revision. No
intermediate commit retained `src/lib.rs` or another library target.

## Frozen #36 Inventory

`tests/fixtures/v0_2_api.rs` remains byte-identical, with SHA-256:

```text
1e2515e134a61e2dd297d3de3127daee5074c75f70e29f53ca176af4fe60110a
```

The 132 historical rows retain their exact IDs, symbols, dispositions, and
single owners:

| Owner | Rows | Realized disposition |
| --- | ---: | --- |
| #52 | 1 | Deleted `NavigationGuide::get_full_path` |
| #53 | 2 | Deleted the unsupported symlink model and mismatch error |
| #54 | 129 | Removed one library target and made 128 exports implementation-only |

The 128 #54 export rows comprise seven modules, 17 root re-exports, one alias,
ten structs, six enums, 37 variants, 19 fields, seven functions, and 24
methods. Their ordered ID set has SHA-256:

```text
1816c0f15e8b3d11b4ee7ee5098541ef991f57f567be92504b58cf790901059b
```

The other frozen executable-ledger hashes also remain:

```text
v0_2_operations.rs  cc3a401a0883cc8686b0cb59e743f64e323607b91b395a49cfa714162f1b8b56
v0_2_contract.rs    0e87357dff1fab9afb30d65a4d459b7f6b8be51a278da5b83623049bcf0fa1d7
v0_2_trust.rs       26162d45509da30737f11893529e05965c53ff70a8fabaceb66a033a615a9d08
```

The current-source tests do not rewrite the historical ledger. They separately
prove that every approved disposition is realized.

## Pinned Pre-Removal SemVer Evidence

The comparison was captured from untouched last-linkable revision
`e34399c14683878064cad18e9506186cd7e4fef1` before deleting `src/lib.rs`.

### Tool identity

- `cargo-semver-checks`: `0.49.0`
- Rust/Cargo used for the run: `1.93.0`
- Installed executable SHA-256:
  `dd13a57b19aaedcb9d520f3d0cfc6af0005c04b4e1521ac9d81cdc513a13ec16`
- Isolated installation directory:
  `/tmp/ang-semver-0.49.0.b4Oyky`

An attempted installation under the active Rust 1.90 toolchain failed because
`cargo-semver-checks 0.49.0` requires Rust 1.91 or newer. The tool was then
installed and run under the already available pinned Rust 1.93.0 toolchain.
No ambient unversioned installation supplied the report.

### Published baseline identity

The crates.io `0.1.4` artifact used as the immutable baseline had:

```text
SHA-256: d08fefac88faf8d737eea273f86bfbc80aaac1eb80ff3a57bde5add824fe5da0
Size: 48,230 bytes
Published: 2025-11-02T15:32:19.514027Z
Yanked: false
VCS revision: 560ce399e1e28e8e0d6b87988956893796d2dfab
Normalized Cargo.toml SHA-256:
  1dc83730531459a1fcae387cc5e5f625a3ff498659915d58fa875dd14c9fab3b
Published src/lib.rs SHA-256:
  c2107c1948025e592e4af33a39b8f80ce7f02b8160d48c12acf6a4c67963d656
```

### Exact command and result

```sh
PATH=/tmp/ang-semver-0.49.0.b4Oyky/bin:$PATH \
  CARGO_TERM_COLOR=never \
  cargo +1.93.0 semver-checks check-release \
    --manifest-path Cargo.toml \
    --baseline-version 0.1.4 \
    --color never
```

The command exited `100`. It built and parsed both current source and
published `0.1.4`. Of 196 evaluated checks, 192 passed and four failed; another
57 checks were inapplicable and skipped. All four failures were major-version
findings:

```text
enum_no_repr_variant_discriminant_changed
enum_variant_added
enum_variant_missing
inherent_method_missing
```

The report required a new major version and recorded four major, zero minor,
and zero warning failures. This is migration evidence showing that even the
last linkable development source had already diverged from immutable `0.1.4`.
It is not a passing gate for the binary-only candidate: after removal there is
no library target for `cargo-semver-checks` to select.

## Red-Before Evidence and Commit Sequencing

Two tests were written and run against the unmodified last-linkable source
before implementation:

```sh
cargo test --locked --test issue_54_binary_only_package -- --nocapture
```

The command exited `101`. Its real offline `cargo package` workflow found
`src/lib.rs` and `kind=["lib"]` / `crate_types=["lib"]` in both workspace and
unpacked-package metadata.

```sh
cargo test --locked --test v0_2_contract \
  issue_54_binary_only_target_and_owned_dispositions_are_realized \
  -- --exact --nocapture
```

The command exited `101`. It reported `src/lib.rs exists=true` and 128
#54-owned exports still reachable.

The red tests were deliberately not committed as a separate revision. Such a
commit would itself have been a later linkable source commit and would falsify
the approved requirement that `e34399c14683878064cad18e9506186cd7e4fef1`
remain the exact last-linkable revision. The commands, exit statuses, and
failure observations were captured before source changes; the first new commit
is already binary-only.

## Implementation

### Target and visibility boundary

- Deleted `src/lib.rs`.
- Kept the single explicit `[[bin]]` target named
  `agentic-navigation-guide`.
- Moved production module ownership into private declarations in
  `src/main.rs`.
- Converted the remaining unrestricted `pub` definitions to `pub(crate)` or
  private visibility.
- Added a package regression that rejects `lib`, `rlib`, `dylib`, `cdylib`,
  `staticlib`, and `proc-macro` in both `kind` and `crate_types`.
- The same regression rejects any unrestricted `pub` line in workspace and
  packaged Rust source.

The binary crate documentation states that its modules are implementation
details without a downstream Rust SemVer promise.

### Test migration

The engine-importing integration suites moved to binary-owned unit modules:

- `containment_guarantee`
- `exclusion_semantics`
- `filesystem_identity_snapshot`
- `v0_2_contract`

CLI-facing observations remain real subprocess tests. The shared test helper
locates or builds the exact named CLI for the active Cargo profile, including a
clean release-only target directory.

The package-importing #52 and #53 consumer tests were retired. Their surviving
positive-control premise is intentionally impossible once the package has no
library target. Exact historical IDs, signatures, AST deletion checks,
documentation, and runtime link rejection remain covered. #62 owns the final
path-dependent packaged consumer that must fail specifically because the
artifact has no library target.

The historical `trust-guide-direct-library-path` ID remains unchanged in the
frozen trust ledger. It now binds to a binary-unit regression that exercises
discovery plus a manually assembled internal `GuideLocation`, proving the
private route cannot bypass the shared safe opener.

## Workspace and Packaged Target Proof

`issue_54_workspace_and_packaged_metadata_are_binary_only` performs all proof
steps against real artifacts:

1. run locked, offline workspace `cargo metadata`;
2. run locked, offline `cargo package` in an isolated target directory;
3. inspect the verified unpacked package;
4. run locked, offline `cargo metadata` from that unpacked artifact;
5. reject `src/lib.rs`, every Rust-linkable target kind, every unrestricted
   `pub` visibility, a missing or additional product binary, or the wrong
   binary name.

Both workspace and unpacked-package metadata contain zero Rust-linkable target
kinds and exactly one product binary:

```text
name: agentic-navigation-guide
kind: ["bin"]
crate_types: ["bin"]
source: src/main.rs
```

Integration-test targets appear only in workspace test metadata and are not
product/library targets. No source or packaged `src/lib.rs` exists.

#62 owns install/smoke testing of one exact final package archive and its
negative consumer. Recording a hash of an archive that embeds this audit would
be self-referential; #54 instead binds its proof to reproducible metadata and
the committed source revision above.

## Acceptance-Criteria Mapping

The issue body's retained-facade assumptions are resolved under the approved
binary-only handoff:

- Every historical export remains in the frozen inventory with one exact
  stability classification and owner.
- Modules, enums, structs, fields, functions, and methods required by the
  binary are private or crate-visible and have no downstream evolution
  promise.
- No public convenience wrapper remains to document or test. Internal behavior
  is covered through binary-unit and CLI subprocess routes; #59 owns final
  coverage measurement.
- A real packaged positive proof succeeds and exposes only the intended CLI.
  #62 owns the exact negative consumer rather than #54 retaining a temporary
  test-only facade.
- The pre-removal SemVer report is pinned migration evidence. #63 gates future
  releases on target shape and the supported CLI compatibility baseline rather
  than a nonexistent Rust-library baseline.
- README, contributor guidance, the normative contract, and binary crate docs
  state the no-shim support policy. #66 owns docs.rs badge and maintained
  package-documentation metadata.
- Warning-denied rustdoc is documentation hygiene for private implementation,
  not a claim of supported downstream Rust API.

## Security and Behavioral Non-Goals

The target/visibility change does not alter the approved guide grammar,
filesystem representation, safe-opening policy, containment policy, output
authority, diagnostic redaction, or stable-filesystem assumption. The tool is
still not a sandbox and does not claim safety against hostile concurrent
filesystem mutation.

Draft PR [#21](https://github.com/plx/agentic-navigation-guide/pull/21) remains
non-authoritative prior art. Issue #68 owns its final disposition; unrelated
changes were not imported.

## Downstream Ownership

- #58 directly rechecks the supported-facade set and binary-only boundary.
- #59 measures internal engine coverage without representing it as library
  coverage.
- #62 installs/smoke-tests the exact package and owns the no-library consumer.
- #63 gates exact package shape and the supported CLI compatibility baseline.
- #64 consumes the pinned report and records the complete `0.1.4` to `0.2.0`
  removal/migration.
- #66 owns the docs.rs badge and maintained documentation metadata.
- #67 owns the remaining complete support-document and reassessment-playbook
  reconciliation.
- #68 owns PR #21 and `Specification.md` historical disposition.
- #72 performs the final no-external-internal-path candidate audit.

The #67 issue text still names the retired
`cargo test --test v0_2_contract` integration target. Its executable successor
is:

```sh
GUIDE_FORMAT_REQUIRE_CONFORMANCE=all \
  cargo test --bin agentic-navigation-guide --locked \
    v0_2_contract_tests:: -- --nocapture
```

## Validation

| Command or evidence | Result |
| --- | --- |
| Focused #54 binary-unit tests | Pass: exact 129 dispositions and internal no-bypass regression |
| `cargo test --test issue_54_binary_only_package --locked -- --nocapture` | Pass: real workspace and unpacked-package binary-only proof |
| Binding all-owner binary-unit contract | Pass: 44 tests and frozen 132-row ledger |
| Clean release-only subprocess regression | Pass: profile-aware helper built and executed the release CLI |
| Debug all-target/all-feature suite | Pass: 330 tests; 2 intentional benchmark ignores |
| Release all-target/all-feature suite | Pass: 330 tests; 2 intentional benchmark ignores |
| Host and Windows GNU all-target/all-feature checks | Pass |
| Strict host and Windows GNU all-target/all-feature Clippy | Pass with warnings denied, including Clippy's configured Rust 1.70 compatibility lint |
| `cargo package --locked` | Pass: verified 110-file binary package |
| Warning-denied binary/private rustdoc | Pass; generated docs state the no-library boundary |
| CLI check and verification of `AGENTIC_NAVIGATION_GUIDE.md` | Pass |
| `just test-production-readiness-selector` | Pass: 61 tests |
| `actionlint`, `just --fmt --check`, `cargo fmt --check`, and `git diff --check` | Pass |
| `lychee` on changed maintained documents | Pass: 34 links, zero errors |
| CI-equivalent `cargo about generate` comparison | Pass: 18,581 bytes, SHA-256 `4db081ceb158791a2068838b5d1b651cd2817e1c9c1c44ac4c325c90dd7594d9`, unchanged |
