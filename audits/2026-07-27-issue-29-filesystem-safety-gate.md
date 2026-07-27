# Issue #29 verification, discovery, and filesystem safety gate

Date: 2026-07-27

Issue: [#29](https://github.com/plx/agentic-navigation-guide/issues/29)

## Gate result

**PASS.** The verification, discovery, and filesystem-safety workstream
satisfies its five completion criteria at product baseline
`2929981982808c3d6f8f8f8367602309796af10a`.

The live production-readiness selector chose #29 from that clean `main`
revision after excluding only #56 for the invocation. GitHub reported no
blockers and exactly seven native children: #35, #48 through #51, #101, and
issue #102. Every child is closed through one dedicated merged pull request.

This aggregate gate adds permanent three-platform invocations for the #49
guide-input trust oracle and the #101 parent-containing explicit-guide matrix.
The #101 leaf PR's Windows job compiled its six-test integration binary but
filtered those tests out through unrelated name filters. The gate therefore
does not treat that successful build as behavioral evidence; its new
`issue_101` step must execute the fixed matrix, including a real Windows
junction. The analogous `issue_49` step executes the platform-specific
symlink/reparse observations behind the exact trust-row oracle.

No product behavior changes here. This record does not certify the final
release candidate, close parent program gate #26, establish a filesystem
sandbox, or complete property, comprehensive platform, public security-policy,
or independent-reassessment work owned elsewhere. It is independent of any
GitHub organization, uses no generated hostile inputs, and performs no
fuzzing.

## Landed child graph

| Child | Realized outcome | Merged PR | Merge commit |
| --- | --- | --- | --- |
| [#35](https://github.com/plx/agentic-navigation-guide/issues/35) | Untrusted-repository/trusted-host model; stable-filesystem containment; explicit guide, disclosure, link, output, and concurrency decisions | [#85](https://github.com/plx/agentic-navigation-guide/pull/85) | `b425921ec7a3c610c70b833f25f95725ad848485` |
| [#48](https://github.com/plx/agentic-navigation-guide/issues/48) | Typed zero-discovery failure by default; recursive-only explicit `--allow-empty` opt-out | [#90](https://github.com/plx/agentic-navigation-guide/pull/90) | `0cb83f13e13725788304b428bbc550225ced5293` |
| [#49](https://github.com/plx/agentic-navigation-guide/issues/49) | Shared non-following guide opener, unsafe-link/reparse rejection, and source/target confidentiality | [#91](https://github.com/plx/agentic-navigation-guide/pull/91) | `fa449f84be0606539b8ac0931c8b0bcb1efe0467` |
| [#50](https://github.com/plx/agentic-navigation-guide/issues/50) | Exact enumerated filesystem identity and one parent snapshot for lookup and placeholder accounting | [#99](https://github.com/plx/agentic-navigation-guide/pull/99) | `69a94c1b522b269d50a5f4b34b18d49f2cfa70bd` |
| [#51](https://github.com/plx/agentic-navigation-guide/issues/51) | Component-ordered containment, observed-change rejection, target redaction, and an explicit no-sandbox boundary | [#103](https://github.com/plx/agentic-navigation-guide/pull/103) | `6b82b06bf2de4acb3000445fdf5274a2319a024b` |
| [#101](https://github.com/plx/agentic-navigation-guide/issues/101) | Parent-containing explicit paths cannot turn an in-anchor link/reparse ancestor into external authority | [#122](https://github.com/plx/agentic-navigation-guide/pull/122) | `edcdd5584d2d24cc513d519034f318874b78c742` |
| [#102](https://github.com/plx/agentic-navigation-guide/issues/102) | Windows device namespaces reject as invalid configuration before root or filesystem access | [#128](https://github.com/plx/agentic-navigation-guide/pull/128) | `2929981982808c3d6f8f8f8367602309796af10a` |

At gate evaluation, GitHub reported all seven issues `CLOSED`, each with the
one PR shown above as its closing pull request. Issue #29 remained open with no
closing pull request before this record branch began.

## Completion-criteria mapping

### Zero discovered guides fail by default

**Pass.** #48 represents a completed recursive search with zero matching
guides as a typed absent result and exits nonzero in default, quiet,
post-tool-use, pre-commit, and GitHub execution modes. Empty trees, a
nonmatching or misspelled guide name, complete exclusion, and deletion of the
last guide all take that path.

Only explicit recursive `--allow-empty` converts genuine absence to success.
It cannot convert an invalid root, invalid name, traversal failure, or unsafe
matching entry to empty success. Ignored guides remain discovered, and the
aggregate distinguishes discovered, passed, failed, ignored, and absent
counts.

### Implicit discovery never reads or echoes an out-of-bound target

**Pass.** #49 routes default, recursive, explicit, environment, hook, and
retained internal guide reads through one safe opener. It classifies without
following the final entry, rejects link/reparse descendants beneath the
anchor, validates implicit names before access, and applies exclusions before
unsafe-match classification. Explicit external regular files remain
caller-granted authority; final guide links do not.

The exact 29-row trust harness covers relative and absolute external targets,
in-root links, dangling links, chains, loops, root aliases, unsafe ancestors,
special entries, exclusions, Windows reparse and namespace cases, and direct
internal access. Cross-mode sentinel checks forbid raw guide source and
resolved-target disclosure. #102 strengthens the retained Windows
device-namespace row to configuration rejection before any filesystem access
without changing its #49 ownership.

### Placeholder identity cannot double-count one filesystem entry

**Pass.** #50 resolves every component against exact UTF-8 names from one
parent-directory snapshot and reuses that snapshot for lookup,
classification, recursion, and placeholder accounting. Comparison is
case-sensitive and performs no Unicode normalization on every host.

A case or Unicode alias therefore cannot satisfy a listed entry while the
enumerated spelling is simultaneously reported by a placeholder. For a
multi-component guide path, the exact first component is the mentioned
directory entry. Retained capability-aware APFS evidence exercises
case-insensitive and normalization-insensitive lookup behavior, while all
three supported CI hosts run the focused identity suite in debug and release.

### The documented containment guarantee matches the implementation

**Pass.** #35 selects stable-filesystem containment on a trusted host and
explicitly rejects a sandbox or hostile-concurrent-replacement claim. #51
implements that boundary with one canonical caller-selected root anchor,
component-ordered resolution, preserved unresolved parent components,
non-following intermediate classification, and identity/type revalidation
around dependent work.

Every observed change fails. In-root, escaping, dangling, chained, and looping
link/reparse ancestors fail without target disclosure. The root itself may be
a caller-selected alias. #101 closes the later parent-containing-path gap by
distinguishing proven in-anchor, proven external, and parent-containing
spellings before external authority can be granted. Real-directory parent
reduction and genuinely external regular guide authority remain supported.

The contract and concise README say that the verifier is a consistency
checker under a stable tree, not a filesystem sandbox or access-control
boundary, and that hostile replacement is unsupported beyond observed-change
checks.

### Platform-dependent semantics have real-platform coverage

**Pass.** The protected build matrix runs on Linux, macOS, and Windows. It
executes #50 and #51 focused suites in debug and release on all three systems.
This gate adds fixed `issue_49` and `issue_101` invocations on every matrix
host rather than relying on cross-compilation or filtered test binaries.

On Windows, the #49 oracle creates and classifies real reparse behavior, and
the #101 integration matrix requires successful `mklink /J` capability and
executes its six tests instead of skipping when unavailable. The existing
Windows-only #102 step exercises seven fixed namespace spellings across CLI,
environment, and retained internal routes, with missing-root precedence
proving rejection occurs before access.

Hosted #102 evidence already passed in
[run 30272766923](https://github.com/plx/agentic-navigation-guide/actions/runs/30272766923),
Windows job
[89999163659](https://github.com/plx/agentic-navigation-guide/actions/runs/30272766923/job/89999163659).
The gate PR's protected three-platform checks must supply the new #49/#101
execution evidence before merge.

## Red-before-fix evidence

The behavioral failures belong to their leaf revisions and remain preserved
in their PRs and permanent regressions:

- #35 recorded the audited external-guide source disclosure and dangling
  output-link creation while assigning each selected outcome to an
  implementation owner.
- #48's test-only commit failed because default empty discovery exited zero
  and a regular-file root could be converted to empty success.
- #49 reproduced an implicitly discovered link reading external sentinel
  bytes before centralized safe opening and diagnostic redaction.
- #50's owner gate failed on case/Unicode aliases, first-component placeholder
  accounting, and repeated directory enumeration before snapshot reuse.
- #51's tests-first commit had seven failures covering target disclosure,
  link/dangling/loop precedence, missing observed-change evidence, and hostile
  race characterization.
- #101's tests-only commit had four rejection groups accept and read a guide
  through a parent-containing link ancestor.
- #102's focused assertion failed because the retained namespace ledger used
  `RejectBeforeRead` instead of the selected `RejectUsage` outcome.

The aggregate review also found an evidence failure, not a product defect:
the #101 Windows test binary reported six tests filtered out. The permanent
named CI steps above correct that coverage gap. No new product behavior is
being fixed, so manufacturing a gate-only behavioral red commit would add no
evidence.

## Retained evidence

- [Normative v0.2 contract](../docs/v0.2-contract.md):
  `9a35db27b8a00d5176689cc2c13113f92f7740f7a899c158538d178a430aca33`
- [Machine-readable trust ledger](../tests/fixtures/v0_2_trust.rs):
  `fa4d8e455eb4bea6cce060d03b1716809b41d9013626c48ba614869d9b614245`
- [CLI discovery and guide-input matrix](../tests/cli_tests.rs):
  `e0bde828496bd00bc37bf48d20c71484de9bdb5c1933bc252f4d97d285e2e8cc`
- [Filesystem-identity snapshot tests](../src/filesystem_identity_snapshot_tests.rs):
  `86670ca130ecc4434daafd06a4b32ccaedeff8c42a3c2387f7e6db2b9602ff8d`
- [Containment-guarantee tests](../src/containment_guarantee_tests.rs):
  `a71072d74349fa8eae38b20a19bd4c6ad0ccacdd1912893bdf7ccae17a15fa93`
- [Parent-containing explicit-guide matrix](../tests/issue_101_parent_explicit_guide.rs):
  `86f7136f86ae6e49edc1d35158d7d8bf739764d406af22ca4f4ff33a267370d0`
- [#50 identity evidence](./2026-07-26-issue-50-filesystem-identity-evidence.md):
  `f568ecd5c03e7541b02b934950089b64450c3a5120379809b95327ceb42f73a5`
- [#51 containment evidence](./2026-07-26-issue-51-containment-evidence.md):
  `b89518e1a25b8830419f231c317f66792057792de61b20ca72fddcfd0c948207`
- [#101 parent-path evidence](./2026-07-27-issue-101-parent-explicit-guide.md):
  `eaed7351b92baa56c665c23d7a3057cbd7473b582e8fd05505c931c08aa9d9b9`
- [#102 Windows namespace evidence](./2026-07-27-issue-102-windows-device-ledger.md):
  `bf256c294f42dc0edd617299eeb717b325e214cbf483a8f050910f9865ab8526`

These are exact content hashes at the evaluated product baseline. The landed
child table separately pins every leaf to its immutable merge commit. The
hashes prevent incidental edits from silently replacing the mutable contract,
fixtures, regression suites, and leaf evidence evaluated by this gate.

## Validation

The exact #102 merge tree matched its reviewed PR head. Its PR checks and the
post-merge `main`
[CI run 30273482092](https://github.com/plx/agentic-navigation-guide/actions/runs/30273482092)
and
[guide run 30273482038](https://github.com/plx/agentic-navigation-guide/actions/runs/30273482038)
passed at product baseline
`2929981982808c3d6f8f8f8367602309796af10a`.

The gate branch additionally requires these fixed, deterministic commands:

```sh
GUIDE_FORMAT_REQUIRE_CONFORMANCE=all \
  cargo test --locked --bin agentic-navigation-guide \
  'v0_2_contract_tests::' -- --nocapture
cargo test --locked --test cli_tests
cargo test --locked --test issue_101_parent_explicit_guide
cargo test --locked issue_49 -- --nocapture
cargo test --locked issue_50 -- --nocapture
cargo test --release --locked issue_50 -- --nocapture
cargo test --locked issue_51 -- --nocapture
cargo test --release --locked issue_51 -- --nocapture
cargo test --locked --all-targets --all-features
cargo test --locked --all-targets --all-features --release
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo package --locked
cargo run --locked -- check --guide AGENTIC_NAVIGATION_GUIDE.md
cargo run --locked -- verify --guide AGENTIC_NAVIGATION_GUIDE.md --root .
just --fmt --check
markdownlint --disable MD010 MD013 MD038 -- \
  audits/2026-07-27-issue-29-filesystem-safety-gate.md
lychee --no-progress \
  AGENTIC_NAVIGATION_GUIDE.md \
  audits/2026-07-27-issue-29-filesystem-safety-gate.md
git diff --check
```

No command performs fuzzing, random input generation, or
organization-dependent validation. The existing fixed 128-attempt #51
containment characterization retains its documented unsupported outcome; it
does not generate inputs or claim hostile-replacement safety.

## Residual ownership

Closing #29 does not waive, subsume, or complete:

- #55's complete claimed-platform evidence;
- #56's generated property work, which was explicitly excluded from this
  selector invocation and is not exercised or claimed here;
- #69's public `SECURITY.md` publication;
- #72's independent immutable-candidate reassessment; or
- parent program gate #26.

The repository's approved sole-maintainer exception is recorded separately by
issue #71. This component gate neither assumes nor requires a GitHub organization.
It establishes only that the seven filesystem-safety leaves assigned to #29
are landed, integrated, and green together with explicit real-platform
execution where their semantics differ.
