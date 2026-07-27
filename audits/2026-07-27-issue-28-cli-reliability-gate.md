# Issue #28 dump, init, and CLI reliability gate

Date: 2026-07-27

Issue: [#28](https://github.com/plx/agentic-navigation-guide/issues/28)

## Gate result

**PASS.** The dump, init, and CLI reliability workstream satisfies its five
completion criteria at component baseline
`571880aeed0fcdb86541f553efb058b1b77dce3d`.

The live production-readiness selector chose #28 from that clean `main`
revision after excluding only #56 for the invocation. GitHub reported no
blockers and exactly seven native children: #41 through #47. Every child is
closed through one dedicated merged pull request.

This is an aggregate component-gate result. It does not add product behavior,
certify the final release candidate, close the parent program gate #26, or
claim completion of property, performance, documentation-completeness,
platform-certification, or independent-reassessment work owned elsewhere.

## Landed child graph

| Child | Realized outcome | Merged PR | Merge commit |
| --- | --- | --- | --- |
| [#41](https://github.com/plx/agentic-navigation-guide/issues/41) | Canonical syntax-sensitive-name encoding, exact round trip, and fail-before-delivery rejection | [#94](https://github.com/plx/agentic-navigation-guide/pull/94) | `63018febb07f1f12806a9ab33c78ed7ba6499a86` |
| [#42](https://github.com/plx/agentic-navigation-guide/issues/42) | Non-following entry classification; only representable regular files and directories generate or verify | [#95](https://github.com/plx/agentic-navigation-guide/pull/95) | `bcccd5143808eeeab9eb0b55f6c4159706de809f` |
| [#43](https://github.com/plx/agentic-navigation-guide/issues/43) | Root, empty-output, indentation, depth, and traversal-resource preflight before delivery | [#96](https://github.com/plx/agentic-navigation-guide/pull/96) | `52ac6ebc96f0fe698faadb78539377d8492a44a9` |
| [#44](https://github.com/plx/agentic-navigation-guide/issues/44) | Shared validated nested exclusion semantics with pre-descent pruning | [#97](https://github.com/plx/agentic-navigation-guide/pull/97) | `3355a62b37220670980f7332ba65a4c68fa5e6dd` |
| [#45](https://github.com/plx/agentic-navigation-guide/issues/45) | Shared create-new output sink with no link following, overwrite, or racing-creator replacement | [#89](https://github.com/plx/agentic-navigation-guide/pull/89) | `6cfdacebbd485f02d23cd809bf8965da65ba9c6b` |
| [#46](https://github.com/plx/agentic-navigation-guide/issues/46) | Scope-aware `CLI > environment > built-in` resolution without artificial conflicts or value disclosure | [#98](https://github.com/plx/agentic-navigation-guide/pull/98) | `bb8949b393dafadfad42b1209e32604c6e678a79` |
| [#47](https://github.com/plx/agentic-navigation-guide/issues/47) | Fallible process output, quiet-mode policy, Unix broken-pipe handling, and stable recursive GitHub diagnostics | [#123](https://github.com/plx/agentic-navigation-guide/pull/123) | `571880aeed0fcdb86541f553efb058b1b77dce3d` |

At gate evaluation, GitHub reported all seven issues `CLOSED`, each with the
one PR shown above as its closing pull request. Issue #28 remained open with no
closing pull request before this record branch began.

## Completion-criteria mapping

### Supported generated names and entries round-trip exactly

**Pass.** #41 supplies the canonical serializer, reversible name diagnostics,
and complete pre-delivery name validation. #42 classifies before traversal and
rejects links, reparse points, special entries, and unknown/transient types
without falling back to “file.” #43 requires nonempty checkable generation,
and the binding operation ledger executes generation through parser,
validator, verifier, and CLI paths.

The all-owner contract has zero pending rows. Current three-platform CI runs
the focused syntax-sensitive-name and entry-type suites, including the
Windows-only ill-formed UTF-16 diagnostic.

### `init` cannot follow a dangling link or overwrite a racing creator

**Pass.** #45 gives `init --output` and `dump --output` one private create-new
sink. It rejects every existing final entry, validates parent authority,
creates without following the final component, validates the created handle,
and performs identity-safe cleanup.

Retained tests cover dangling links, ancestor links, existing regular and hard
link names, 100 synchronized sink races, and 100 process-level `init` races.
Exactly one racing creator may win; an existing or replacement entry is never
overwritten or removed.

### Invalid roots, empty output, and numeric extremes are explicit and bounded

**Pass.** #43 rejects missing, non-directory, capability-enforced unreadable,
empty, and fully excluded roots before stdout or destination creation.
Indentation is limited to 1–16, explicit depth to 0–256, and omitted depth
rejects a tree requiring logical depth 257. Directory enumeration handles are
closed before descent, so valid depth does not create an unbounded live-handle
chain.

The focused preflight suites run in debug and release mode on Linux, macOS,
and Windows.

### Exclusions, configuration, quiet mode, CI diagnostics, and broken pipes match the contract

**Pass.**

- #44 implements one case-sensitive component-aware matcher for `dump`,
  `init`, and recursive discovery, with validation and pre-descent pruning.
- #46 applies relevant environment defaults only after parsing explicit CLI
  intent and preserves native path values without echoing rejected values.
- #47 centralizes fallible stdout/stderr behavior, suppresses only ordinary
  quiet-mode chatter, treats Unix stdout `BrokenPipe` as normal pipeline
  termination, and shares a control-safe `path:line: typed reason` diagnostic
  between single and recursive GitHub execution.

The permanent #47 matrix covers five command paths, three log modes, four
execution modes, success and failure, and paths containing spaces and Unicode.

### Every leaf closed with a retained red-before-fix regression

**Pass.** Each child PR records its own tests-first evidence:

- #41: nine owned grammar/operation divergences and focused diagnostic/name
  regressions failed before the canonical serializer landed.
- #42: all executable owned entry-type divergences and partial-delivery cases
  failed before the shared classifier landed.
- #43: five owned generation operations plus focused root/range/handle tests
  failed before preflight landed.
- #44: eight of nine focused exclusion integration tests failed, and excluded
  nested entries reached classification/enumeration.
- #45: dangling final links, an existing output, and an in-root linked ancestor
  were followed or overwritten by the audited implementation.
- #46: five of eight focused precedence tests failed on false conflicts,
  wrong scope, and live-value disclosure.
- #47: all four fixed output regressions failed with exit 101, quiet chatter,
  a split recursive location, or the corresponding matrix failure.

Those failures belong to their leaf revisions and are preserved in their PR
records and permanent tests. No new behavioral defect is fixed by this
aggregate gate, so manufacturing a gate-only red commit would provide no
additional evidence.

## Retained evidence

- [Normative v0.2 contract](../docs/v0.2-contract.md):
  `1c34a08d2a91fdf21efb77bd380a356fea661fc250386114b6d4ba131454c4f5`
- [CLI integration matrix](../tests/cli_tests.rs):
  `ed92759ffb6bf992d770b5a6354186cfee15d9d19c68d7652bbabe06e4998108`
- [Environment-precedence matrix](../tests/environment_precedence.rs):
  `5cc944b5e74f782ccaf065f7a86ee0058b0ed14a03b6458c51c86ea0dc8c4164`
- [Output-contract matrix](../tests/issue_47_output_contract.rs):
  `1baf168cce9c0d2efe4241e0b5776570747950c3002e784ea0954720496d201b`
- [#47 output-contract audit](./2026-07-27-issue-47-output-contract.md):
  `7d260361bde58e52b65612462e37fd0f89184a613d69040bef7f01b48e2920d9`

These are the exact content hashes at the evaluated component baseline. The
landed-child table separately pins each leaf to its immutable merge commit,
while these hashes prevent later incidental edits from silently replacing
the mutable contract and regression evidence this gate evaluated.

## Validation

The exact #47 merge tree matched its reviewed PR head, and both its PR checks
and post-merge `main` workflows passed. The post-merge run
[30262760698](https://github.com/plx/agentic-navigation-guide/actions/runs/30262760698)
completed successfully across:

- Rust 1.85, current supported stable lines, and informational beta;
- debug and release builds;
- Linux, macOS, and Windows;
- the all-owner contract and focused #41–#47 suites;
- strict lint and formatting;
- exact binary-only package installation and smoke;
- release identity, dependency attribution, workflow policy, and selector
  tests; and
- the repository navigation-guide check.

The gate branch additionally requires:

```sh
GUIDE_FORMAT_REQUIRE_CONFORMANCE=all \
  cargo test --locked --bin agentic-navigation-guide \
  'v0_2_contract_tests::' -- --nocapture
cargo test --locked --test cli_tests
cargo test --locked --test environment_precedence
cargo test --locked --test issue_47_output_contract
cargo test --locked --all-targets --all-features
cargo test --locked --all-targets --all-features --release
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo package --locked
cargo run --locked -- check --guide AGENTIC_NAVIGATION_GUIDE.md
cargo run --locked -- verify --guide AGENTIC_NAVIGATION_GUIDE.md --root .
just --fmt --check
markdownlint --disable MD010 MD013 MD038 -- \
  audits/2026-07-27-issue-28-cli-reliability-gate.md
lychee --no-progress \
  AGENTIC_NAVIGATION_GUIDE.md \
  audits/2026-07-27-issue-28-cli-reliability-gate.md
git diff --check
```

## Residual ownership

Closing #28 does not waive or complete:

- #55's complete claimed-platform evidence;
- #56's generated property work, which was explicitly excluded from this
  selector invocation and is not exercised or claimed here;
- #59's broader coverage and performance effectiveness;
- #63, #66, and #67's remaining release and documentation controls;
- #69's public security policy;
- #72's independent immutable-candidate reassessment; or
- parent program gate #26.

This gate establishes only that the seven reliability leaves assigned to #28
are landed, integrated, and green together.
