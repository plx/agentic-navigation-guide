# Issue #51 Stable-Filesystem Containment Evidence

Date: 2026-07-26

## Scope

This evidence covers the ten adversarial trust rows owned by issue #51:
canonical verification-root anchoring, intermediate descendant containment,
resolved-target redaction, observed identity/type changes, and the documented
hostile-replacement boundary.

The selected v0.2 policy is a stable-filesystem containment guarantee. It is
not a filesystem sandbox or a hostile-concurrent-replacement guarantee.
Handle-relative hostile traversal is therefore neither implemented nor
claimed.

## Environment and Revisions

- Hardware: Apple M4 Max, 16 cores (12 performance and 4 efficiency), 64 GB
  RAM
- OS: macOS 27.0 (build 26A5378n), arm64, APFS on an internal SSD
- Rust: `rustc 1.90.0 (1159e78c4 2025-09-14)`
- Pre-fix production commit:
  `4d0e881dbbb7dc3acdb205990d231d5a02a9fb55`
- Tests-first harness commit:
  `be9a581aa14b5add2fc57c54a27be780f42bfa8b`
- Post-fix implementation commit:
  `e08000fbaf355ae7411dcadb1688d7dbf6ca7dc6`
- Windows compile target: `x86_64-pc-windows-gnu`

## Red-Before Evidence

The exact tests-first commit was checked out in a detached temporary worktree
and run against the pre-fix implementation:

```sh
cargo test --test containment_guarantee --locked -- --nocapture
```

The command exited 101: 5 tests passed and 7 failed. The failures were:

- the external-link and shared-route cases received containment errors after
  target resolution instead of rejecting the intermediate link;
- dangling and looping link ancestors produced raw I/O behavior;
- `PathEscapesRoot` displayed both supplied canonical-path fields, including
  the external sentinel;
- the exact evidence-set test could not find the deterministic observed-change
  regressions in the implementation; and
- the bounded hostile root-alias race disclosed the resolved external sentinel
  in an error, illustrating why the pre-fix check-then-use flow could not
  support a stronger concurrency claim.

The same run retained the existing positive cases: ordinary in-root paths,
in-root link rejection, a link ancestor followed by a missing final component,
caller-selected root aliases, and a root spelling containing unresolved
`..`.

## Implemented Guarantee

### One canonical verification anchor

`Verifier` resolves the caller-selected root once, before item traversal. The
selected root may itself be a link, junction, or reparse alias. Every later
verifier lookup starts from the resulting canonical directory rather than
reopening the original root spelling.

Guide item paths have already passed the grammar's relative-component checks.
The verifier appends only that validated relative tail to its canonical
anchor. For a root spelling containing `..`, each preceding spelling prefix is
canonicalized before the parent step is applied. Thus `alias/..` means the
parent of the alias target even on Windows, whose ordinary whole-path
canonicalization would otherwise normalize the parent first. The regression
proves both the expected target-parent entry and rejection of a decoy that
exists only under the alias's lexical parent.

Arbitrary explicit guide and output spellings have their own owners and are
not a `Verifier` item-path surface.

### One non-following component walker

The former exact-name preflight and separate containment resolver were
replaced with one component walker. For every component it:

1. obtains the exact UTF-8 entry from the parent's one-per-run snapshot;
2. compares a non-following type-and-identity observation before dependent
   use;
3. descends only through an observed real directory;
4. rejects a symbolic link or Windows reparse entry before target
   canonicalization; and
5. retains canonical containment checks for supported regular files and
   directories.

This gives the same rejection precedence for an external, in-root, dangling,
chained, or looping intermediate link and does not inspect link targets.

### Observed-change checks

Each snapshot records the parent's observation and every enumerated entry's
observation. An observation combines type classification with filesystem
identity. Parents are compared around enumeration, again when a cached
snapshot is reused, and after their sibling set has been verified. Listed
entries are compared before dependent use and after it; every flat-path
ancestor is also compared after the final dependent use.

These comparisons are defense in depth for the stable-tree contract. The
metadata, classification, identity, and later recheck operations are not one
atomic hostile-race primitive.

### Platform identity

- Unix identity is `(st_dev, st_ino)` from the non-following metadata result.
- Windows opens the observed final component with
  `FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS`, attribute-only
  access, and read/write/delete sharing. It prefers
  `(VolumeSerialNumber, FILE_ID_128)` from `FileIdInfo`, falls back to the
  volume serial plus 64-bit legacy file index, and rejects a zero legacy
  index.
- The Windows unsafe calls use initialized structures of the exact API-requested
  type while the owning handle remains live. The Windows GNU all-target build
  checks these bindings; real Windows behavior remains part of #55's
  three-platform execution gate.

### Diagnostic and route behavior

Intermediate link diagnostics contain the bounded logical guide path and
observed entry kind, not the target. The transitional
`PathEscapesRoot { root, resolved, .. }` shape is preserved for #54, but its
display omits those fields and the verifier stores only `<redacted>` in them.

The command-line `verify` route, direct `Verifier::verify`, and transitional
`verify_guide` route all reach this same component walker. Safe opening of the
guide file itself remains the separate #49 boundary.

## Exact Trust-Row Evidence

`issue_51_trust_evidence_is_an_exact_set` derives the owner-51 IDs from
`tests/fixtures/v0_2_trust.rs`, rejects duplicate IDs and test names, requires
exact equality with ten rows, and confirms every declared evidence function is
compiled into the integration or verifier test source.

| Trust row | Executable evidence |
| --- | --- |
| `trust-containment-root-alias` | `issue_51_caller_selected_root_alias_is_the_canonical_anchor` |
| `trust-containment-root-parent-spelling` | `issue_51_root_parent_spelling_does_not_broaden_authority` |
| `trust-containment-existing-link-escape` | `issue_51_out_of_root_link_ancestor_is_rejected_without_target_disclosure` |
| `trust-containment-existing-link-in-root` | `issue_51_in_root_link_ancestor_is_rejected_without_traversal` |
| `trust-containment-link-ancestor-missing-final` | `issue_51_link_ancestor_with_nonexistent_final_is_rejected_without_resolution` |
| `trust-containment-dangling-ancestor` | `issue_51_dangling_link_ancestor_is_rejected_without_resolution` |
| `trust-containment-link-chain-or-loop` | `issue_51_link_chain_and_loop_ancestors_are_rejected_without_resolution` |
| `trust-containment-target-redaction` | External-link and CLI sentinel checks, constructed display check, and `issue_51_path_escape_errors_do_not_retain_resolved_targets` |
| `trust-containment-observed-identity-change` | Deterministic item, parent-enumeration, and ancestor-replacement tests |
| `trust-containment-hostile-replacement` | `issue_51_hostile_replacement_is_characterized_as_unsupported` plus README/contract assertions |

On Windows, the focused suite additionally constructs a real `mklink /J`
junction and requires rejection as a reparse point. The ordinary link
fixtures require Windows symbolic-link capability rather than reporting a
passing skip.

## Deterministic Mutation Matrix

Test-only private checkpoints make observed changes deterministic without
adding or widening a public API.

| Initial observation | Injected change | Checkpoint | Required result |
| --- | --- | --- | --- |
| Regular file | Different regular-file identity | After snapshot selection | Reject |
| Regular file | Directory | After snapshot selection | Reject |
| Directory | Regular file | After snapshot selection | Reject |
| Directory | Different directory identity | After snapshot selection | Reject |
| Regular file | Disappearance | After snapshot selection | Reject |
| Parent directory | Different directory identity | After enumeration | Reject |
| Real intermediate directory | External directory link | Before final revalidation | Reject without target disclosure |

The old entry is retained under a tombstone name during replacement cases so
the platform cannot immediately recycle its identity.

## Hostile-Replacement Characterization

The Unix race harness performs 128 finite atomic replacements of the
caller-selected root alias between an in-root directory and a sentinel-named
external directory while verification runs. Both trees contain the same
listed empty file. Every attempt must finish without panic; a rejection must
not disclose the external sentinel. Success and rejection counts are both
accepted because changing the trusted root while it is being resolved is
explicitly outside the supported guarantee.

The recorded focused run was:

```text
attempts=128 successes=128 rejections=0 expected=unsupported
```

This result is evidence of the limitation, not evidence that external
concurrent replacement is safe.

## Performance Boundary

The existing ignored #50 release benchmark was rerun after the containment
changes. Alternating-placeholder versus plain-workload median ratios were
0.973×, 0.978×, and 0.981× for 500, 1,000, and 2,000 listed files.
Alternating-workload scaling was 1.992× and 2.079×, within its 2.5× threshold.

The verifier now performs additional non-following identity observations and
canonical containment checks. Comprehensive constant-factor, sparse-guide,
network-filesystem, RSS, and throughput analysis remains assigned to #59.

## Documentation and Scope Handoffs

The README now describes the realized stable-tree behavior, observed-change
defense in depth, target redaction, hostile-replacement limitation, and
no-sandbox/non-execution boundary. The normative v0.2 contract already carries
the approved #35 decision.

`SECURITY.md` is intentionally not created here. Issue #69 depends on #51 and
owns the complete hostile-repository/trusted-host policy after the guide-input
and output boundaries have also landed. The focused test will prevent a future
`SECURITY.md` from weakening the stable/no-sandbox language.

Other exclusions are explicit:

- #42 owns final textual file/directory classification.
- #45 owns filesystem destination creation.
- #49 owns guide-file opening and external explicit-path authority.
- #53 removes the legacy programmatic `FilesystemItem::Symlink` branch and its
  target comparison.
- #54 preserves the 132-row API ledger before removing the legacy library
  surface.
- #55 owns real Linux, macOS, and Windows execution.
- #59 owns the comprehensive performance baseline.
- #69 owns final security-policy publication.
- #101 owns the separately reproduced parent-containing explicit guide-path
  authority defect; it is blocked by #51 and was not absorbed here.
- #102 owns the separate Windows guide device-namespace ledger mismatch.

## Prior Art

- PR #85 / commit `b425921ec7a3c610c70b833f25f95725ad848485`
  selected the stable-filesystem/no-sandbox policy.
- PR #91 / commit `fa449f84be0606539b8ac0931c8b0bcb1efe0467`
  centralized guide-file opening; #51 does not duplicate that authority.
- PR #95 / commit `bcccd5143808eeeab9eb0b55f6c4159706de809f`
  supplied the shared non-following entry classifier.
- PR #99 / commit `69a94c1b522b269d50a5f4b34b18d49f2cfa70bd`
  supplied exact-name parent snapshots and the retained performance harness.

## Validation

| Command | Result |
| --- | --- |
| `cargo test --locked issue_51 -- --nocapture` | Pass: 4 unit and 12 integration tests; race recorded above |
| `cargo test --all-targets --all-features --locked --no-fail-fast` | Pass: 327 tests, 2 intentional ignores |
| `cargo test --all-targets --all-features --locked --release --no-fail-fast` | Pass: 327 tests, 2 intentional ignores |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | Pass |
| `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --locked` | Pass |
| Windows GNU all-target check and strict Clippy | Pass |
| `GUIDE_FORMAT_REQUIRE_CONFORMANCE=all cargo test --test v0_2_contract --locked` | Pass: 42 tests and the exact 132-row API ledger |
| CLI check and verification of `AGENTIC_NAVIGATION_GUIDE.md` | Pass |
| `just test-production-readiness-selector` | Pass: 61 tests |
| `cargo about generate` comparison | Pass: `THIRD_PARTY_LICENSES.md` unchanged |
| `actionlint`, focused `markdownlint`, `lychee`, `just --fmt --check`, and `git diff --check` | Pass; 20 links checked with 0 errors |
| Three independent security, platform/API, and evidence reviews | READY after the Windows root-parent finding was fixed |
