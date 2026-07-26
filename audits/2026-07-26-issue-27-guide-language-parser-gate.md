# Issue #27 Guide Language and Parser Gate Evidence

Date: 2026-07-26

## Status

PRELIMINARY PASS — the local aggregate evidence satisfies the issue #27
component-gate criteria for candidate
`69a94c1b522b269d50a5f4b34b18d49f2cfa70bd`.

The evidence PR that carries this record also adds a focused
logical-backslash regression to the existing three-OS build matrix. The final
component verdict is withheld until that exact test has executed successfully
on hosted Windows and the PR's exact-head checks are green.

This is a component-gate result only. It is not the independent
production-readiness verdict assigned to issue #72, and it does not authorize
release or distribution.

## Candidate and Environment

| Item | Value |
| --- | --- |
| Repository | `plx/agentic-navigation-guide` |
| Candidate | `69a94c1b522b269d50a5f4b34b18d49f2cfa70bd` |
| Branch point | `origin/main` at the candidate commit |
| Rust | `rustc 1.90.0 (1159e78c4 2025-09-14)` |
| Host | macOS 27.0 build 26A5378n, arm64 |
| Hardware | Apple M4 Max, 64 GiB RAM |
| Windows compile target | `x86_64-pc-windows-gnu` |

## Contract and Product Boundary

PR [#84](https://github.com/plx/agentic-navigation-guide/pull/84)
records the binding v0.2 language and filesystem-representation contract in
[`docs/v0.2-contract.md`](../docs/v0.2-contract.md). It connects 61 grammar
examples and 35 operation examples to machine-readable fixtures. Its binding
gate selector and test target, run with the locked dependency graph, are:

```sh
GUIDE_FORMAT_REQUIRE_CONFORMANCE=all \
  cargo test --test v0_2_contract --locked
```

The local evidence below runs the same gate on the explicitly recorded Rust
1.90.0 toolchain; hosted CI uses the workflow's installed stable toolchain.

The approved gate-graph correction is retained in the
[#27 decision record](https://github.com/plx/agentic-navigation-guide/issues/27#issuecomment-5083755928).
It requires every pending owner to land and the all-owner command to pass with
zero pending rows.

Issue #27 predates the approved supported-product decision. PR
[#86](https://github.com/plx/agentic-navigation-guide/pull/86), merge
`805dd11ba57ea4fd90d80365102c927a5ee8227c`, resolves its old “CLI and library
entry points” wording as follows:

- the installed CLI is the sole supported v0.2 product;
- the CLI and its internal engine use the same parser, validator, and verifier
  contract;
- the set of supported v0.2 Rust facades is non-vacuously empty; and
- the current legacy library remains an unsupported migration surface assigned
  to issue #54.

The executable API test inventories all 132 current-source entries and proves
that none is a supported v0.2 facade. This gate does not remove, reinterpret,
or edit that inventory.

## Landed Graph

All five native children and all five additional blockers named by the
approved all-owner correction are closed through dedicated merged PRs:

| Role | Issue | Behavior | Dedicated PR | Merge commit |
| --- | ---: | --- | ---: | --- |
| Native child | [#34](https://github.com/plx/agentic-navigation-guide/issues/34) | Normative grammar and executable ledger | [#84](https://github.com/plx/agentic-navigation-guide/pull/84) | `324498f7fbbcd8b4431cb920e3396c01e4d5e199` |
| Native child | [#37](https://github.com/plx/agentic-navigation-guide/issues/37) | Deterministic fail-closed hierarchy | [#87](https://github.com/plx/agentic-navigation-guide/pull/87) | `610a8ad3e8f6e0a3e95288d5a98c014b46a0df4c` |
| Native child | [#38](https://github.com/plx/agentic-navigation-guide/issues/38) | Exact marker grammar | [#88](https://github.com/plx/agentic-navigation-guide/pull/88) | `6557d484a971dcb26eaf065c04d17807561690f7` |
| Native child | [#39](https://github.com/plx/agentic-navigation-guide/issues/39) | Consistent ignored-guide outcome and policy | [#92](https://github.com/plx/agentic-navigation-guide/pull/92) | `a27832cc801b2a14a0f9a5fbda24bb99629aa0d4` |
| Native child | [#40](https://github.com/plx/agentic-navigation-guide/issues/40) | Lossless parser structure and normalization | [#93](https://github.com/plx/agentic-navigation-guide/pull/93) | `6025d0de99d20918ee52e2667298ce85a82f8b79` |
| Blocker | [#41](https://github.com/plx/agentic-navigation-guide/issues/41) | Syntax-sensitive names and canonical round trip | [#94](https://github.com/plx/agentic-navigation-guide/pull/94) | `63018febb07f1f12806a9ab33c78ed7ba6499a86` |
| Blocker | [#42](https://github.com/plx/agentic-navigation-guide/issues/42) | Entry-type classification | [#95](https://github.com/plx/agentic-navigation-guide/pull/95) | `bcccd5143808eeeab9eb0b55f6c4159706de809f` |
| Blocker | [#43](https://github.com/plx/agentic-navigation-guide/issues/43) | Generation preflight before delivery | [#96](https://github.com/plx/agentic-navigation-guide/pull/96) | `52ac6ebc96f0fe698faadb78539377d8492a44a9` |
| Blocker | [#44](https://github.com/plx/agentic-navigation-guide/issues/44) | Nested exclusion semantics | [#97](https://github.com/plx/agentic-navigation-guide/pull/97) | `3355a62b37220670980f7332ba65a4c68fa5e6dd` |
| Blocker | [#50](https://github.com/plx/agentic-navigation-guide/issues/50) | Exact identity and one-snapshot verification | [#99](https://github.com/plx/agentic-navigation-guide/pull/99) | `69a94c1b522b269d50a5f4b34b18d49f2cfa70bd` |

GitHub reported each issue `CLOSED` with exactly one associated merged PR when
this component baseline was assessed. Issue #27 remained `OPEN` with no
associated PR before this evidence branch began.

## Acceptance-Criteria Map

### 1. Normative grammar and executable examples

PR #84 records the grammar, source-of-truth precedence, representability
rules, 61 parser rows, and 35 operation rows. The
`documentation_and_fixture_are_a_bijection` test proves exact document/fixture
coverage, while `contract_cases` and `operation_cases` execute the real parser,
validator, verifier, dumper, and CLI paths.

### 2. Impossible parentage and malformed markers fail closed

PR #87 replaced the audited hierarchy reconstruction and retained the exact
false-success regression in
`parser::tests::test_rejects_audited_child_under_intervening_file`. PR #88
strictly recognizes the envelope, and PR #92 retains the CLI integration
regression `test_issue39_malformed_marker_never_activates_ignore`.

Before PR #87's implementation, the exact `a/c` directory plus regular `b`
fixture made `verify` exit 0 after the parser attached `c` beneath stale
directory `a`; its owner gate also failed 2 of 16 cases. Before PR #88's
implementation,
`GUIDE_FORMAT_REQUIRE_CONFORMANCE=38 cargo test --test v0_2_contract --locked`
failed 2 of 16 cases, and all three concatenated-marker CLI regressions
unexpectedly exited 0. The retained focused regressions pass on the gate
candidate.

### 3. Ignore, choices, escaping, whitespace, and separators share one contract

- PR #92 implements the distinct ignored-guide outcome and allow/deny policy.
- PR #93 preserves choices, quoted whitespace, escaped text, duplicate paths,
  and slash-only logical structure.
- PR #94 provides canonical syntax-sensitive-name serialization, exact
  diagnostics, and round-trip tests.
- PRs #95, #96, #97, and #99 carry the same contract through entry
  classification, generation, exclusions, and exact filesystem identity.
- `issue_27_native_separators_cannot_reinterpret_logical_backslashes` proves
  that native separators cannot satisfy a logical literal-backslash name,
  including the parent-traversal-shaped `dir\..\name`.

The supported entry point is the CLI. Its internal engine shares these exact
paths; the complete API inventory proves that no legacy Rust symbol is a
supported v0.2 facade. The integration test reaches the current internal
engine through the temporarily linkable legacy target. That is transitional
test access, not supported-library coverage or a support promise; issue #54
owns its migration when the target is removed.

### 4. Hierarchy construction is not quadratic in sibling count

`parser::tests::test_hierarchy_work_is_linear_and_stack_is_bounded` observes
exactly two work units per flat item at 10,000, 20,000, and 40,000 siblings and
checks the depth bound. The ignored release benchmark used three warmups and
ten measured samples per workload:

| Flat siblings | Median | Ratio to prior size | 2.5× limit |
| ---: | ---: | ---: | --- |
| 10,000 | 2.499875 ms | — | Pass |
| 20,000 | 5.141083 ms | 2.057× | Pass |
| 40,000 | 10.858292 ms | 2.112× | Pass |
| 80,000 | 21.644291 ms | 1.993× | Pass |
| 120,000 | 41.988042 ms | 1.940× | Pass |

The executable benchmark asserts the 2.5× threshold for every adjacent pair,
including the final sub-doubling step.

### 5. Leaf completion and user-visible documentation

The landed graph above accounts for every native child and approved all-owner
blocker. The contract, README, migration notes, command behavior, and focused
fixtures landed with their owning PRs. No gate-only product behavior is
introduced here.

## Aggregate Gate

The all-owner command was intentionally red on the 0.1.4 implementation in
PR #84 with exit status 101. That staged result proved that the normative
ledger could not pass by observing legacy behavior.

On the candidate plus this evidence branch:

```text
GUIDE_FORMAT_REQUIRE_CONFORMANCE=all cargo +1.90.0 test \
  --test v0_2_contract --locked -- --nocapture

42 passed; 0 failed; 0 ignored
```

The fixture scan finds zero `pending_issue: Some` rows, and
`ALLOWED_PENDING_OWNERS` is empty. The gate also verifies a complete, nonempty
132-row current-source API inventory before concluding that the supported
v0.2 facade set is empty.

## Focused Validation

| Command | Result |
| --- | --- |
| `cargo +1.90.0 test --locked issue_27_native_separators_cannot_reinterpret_logical_backslashes -- --exact --nocapture` | Pass |
| `cargo +1.90.0 test --locked parser::tests::test_hierarchy_work_is_linear_and_stack_is_bounded -- --exact --nocapture` | Pass |
| `cargo +1.90.0 test --locked parser::tests::test_rejects_audited_child_under_intervening_file -- --exact --nocapture` | Pass |
| `cargo +1.90.0 test --test cli_tests --locked test_issue39_malformed_marker_never_activates_ignore -- --exact --nocapture` | Pass |
| `cargo +1.90.0 test --release --locked parser::tests::benchmark_flat_hierarchy_scaling -- --exact --ignored --nocapture --test-threads=1` | Pass; all reported ratios at or below 2.112× |
| `cargo +1.90.0 check --tests --locked --target x86_64-pc-windows-gnu` | Pass |
| `cargo +1.90.0 clippy --tests --locked --target x86_64-pc-windows-gnu -- -D warnings` | Pass |
| `cargo +1.90.0 run --locked -- check --guide AGENTIC_NAVIGATION_GUIDE.md` | Pass |
| `cargo +1.90.0 run --locked -- verify --guide AGENTIC_NAVIGATION_GUIDE.md --root .` | Pass |
| `cargo +1.90.0 fmt --all -- --check` | Pass |
| `actionlint .github/workflows/ci.yml` | Pass |
| `cargo +1.90.0 test --workspace --all-targets --all-features --locked --no-fail-fast` | Pass; 311 passed and 2 intentional manual benchmarks ignored |
| `cargo +1.90.0 test --release --workspace --all-targets --all-features --locked --no-fail-fast` | Pass; 311 passed and 2 intentional manual benchmarks ignored |
| `cargo +1.90.0 clippy --workspace --all-targets --all-features --locked -- -D warnings` | Pass |
| `RUSTDOCFLAGS='-D warnings' cargo +1.90.0 doc --workspace --no-deps --all-features --locked` | Pass |
| `just --fmt --check` | Pass |
| `just test-production-readiness-selector` | Pass; 61 tests |
| `just get-next-production-readiness-issue --json` | Selected issue #27; 33 open, 11 ready, 0 covered |
| `markdownlint --disable MD010 MD013 MD038 -- audits/2026-07-26-issue-27-guide-language-parser-gate.md` | Pass |
| `lychee --no-progress audits/2026-07-26-issue-27-guide-language-parser-gate.md` | Pass; 0 errors |
| `git diff --check` | Pass |

Three independent read-only reviews covered gate-claim precision, the
logical-backslash regression, and CI/Windows portability. They reported no
unresolved blocker.

## Post-Merge Review Lineage

Three gate-relevant review findings arrived after their subject PRs had
already merged. Their historical UI thread state is not treated as proof by
itself:

- PR #84's
  [syntax-sensitive-name thread](https://github.com/plx/agentic-navigation-guide/pull/84#discussion_r3651352872)
  is covered by PR #94's canonical serializer and focused round-trip suite.
- PR #84's
  [ignored-execution-mode thread](https://github.com/plx/agentic-navigation-guide/pull/84#discussion_r3651352874)
  is covered by PR #92's distinct ignored outcome across the supported CLI
  modes.
- PR #93's
  [native-separator thread](https://github.com/plx/agentic-navigation-guide/pull/93#discussion_r3652277209)
  is covered mechanistically by PR #99's exact snapshot preflight and directly
  by the new `issue_27_native_separators_cannot_reinterpret_logical_backslashes`
  regression. The three-OS workflow executes that regression on Windows rather
  than relying only on cross-compilation.

PR #94's ill-formed UTF-16 diagnostic was previously cross-compiled but not
executed on Windows. The evidence PR adds an exact Windows-only runtime step
for
`path_codec::tests::ill_formed_windows_name_diagnostic_preserves_every_utf16_unit`.

A fourth
[post-merge PR #99 finding](https://github.com/plx/agentic-navigation-guide/pull/99#discussion_r3653033516)
concerns direct Windows construction of the unsupported legacy
`FilesystemItem::Symlink` variant. Parser and CLI text cannot construct that
variant, and issue #53 already owns removal of the two exact symlink API rows
before issue #54 removes the legacy library target. The finding therefore does
not alter #27's supported CLI/internal-engine contract, but remains explicitly
owned rather than waived.

The three gate-relevant historical threads have landed or newly executable
coverage and are not waived acceptance gaps.

## Residual Scope

- Hosted Windows execution and the evidence PR's exact-head checks remain
  required before the status can become a final component PASS.
- Issue #53 owns the incomplete unsupported symlink model, including the late
  PR #99 Windows finding.
- Issue #54 still owns removal of the unsupported legacy library target; this
  gate preserves its 132-row inventory.
- Issue #59 owns broader runtime, memory, sparse-guide, and filesystem
  performance characterization.
- Issue #72 independently reassesses the eventual immutable release candidate.
  This record neither performs nor predicts that verdict.
