# Issue #67 complete v0.2 contract evidence

Date: 2026-07-27

Issue: [#67](https://github.com/plx/agentic-navigation-guide/issues/67)

Branch point: `8dab75556d24bb8cdbaff002d03cb0c382fd325e`

## Selection and scope

The live production-readiness selector first returned issue #56. The
maintainer directed the current loop not to add or run more fuzzing or
generated-input campaigns, so that issue received an explicit temporary
operator-exclusion comment and remains open, unclaimed, and unwaived. The
documented invocation

```sh
just get-next-production-readiness-issue --exclude 56 --json
```

then selected #67 with every native prerequisite closed and no closing pull
request.

This issue changes documentation and contract-drift enforcement only. It does
not change parser, verifier, filesystem, output, or release runtime behavior.
It performs no fuzzing, random generation, mutation campaign, organization
setup, or protected-setting change.

## Red-before-documentation evidence

The tests and complete CLI fixture were added before the normative prose.
Clap introspection passed, proving the fixture matched the realized command
graph. The existing contract then failed its documentation/fixture bijection:

```text
the normative document and CLI command fixture must contain the same rows
left: 0
right: 4
```

The issue-specific integration test separately failed because
`docs/v0.2-contract.md` lacked `## Complete CLI reference`. These failures
demonstrate the documentation gap rather than a manufactured runtime defect.

## Realized contract

`docs/v0.2-contract.md` remains the sole file with the normative v0.2 claimant
marker. It now contains:

- the four-command inventory and built-in help/version behavior;
- all 31 declared product arguments, including hidden direct selectors,
  aliases, value actions, required/default values, and enumerated values;
- command semantics, CLI/environment/built-in precedence, constraints, stable
  streams, execution modes, and exit statuses;
- the existing exact guide grammar, filesystem mapping, exclusion behavior,
  recursive semantics, trust boundary, diagnostic redaction, output policy,
  and supported-platform capability matrix;
- guide/CLI versioning, Rust `1.85.0` MSRV, locked dependency policy,
  supported-version lifecycle, and binary-only compatibility commitment;
- the hostile-repository/trusted-host model, stable-tree limitation,
  no-sandbox statement, explicit resource limits and non-limits, and the
  current absence of a private vulnerability-report route; and
- the zero-supported-symbol Rust API decision and complete historical export
  disposition.

The README remains concise and points to the complete CLI and security
sections. The historical specification and history index continue to point
readers to the sole normative source without rewriting their dated evidence.
The changelog records the completed contract surface.

## Drift prevention

`tests/fixtures/v0_2_cli.rs` is the machine-readable command/argument ledger.
The binary-unit contract test compares it directly with Clap and fails on a
command name/help summary, long or short spelling, action, required/global/
hidden setting, value name, default, or possible-value change. The existing
documentation bijection now also requires one exact normative row for every
CLI fixture entry.

`tests/issue_67_complete_contract.rs` binds the complete-support headings and
limitations to the package MSRV and retained normative links. Its fixed CLI
cases execute help/version, invalid usage, default and post-tool-use failures,
quiet success, and quiet primary dump output.

The workflow documentation gate now:

- lints the README, normative contract, release policy, maintainer-continuity
  policy, and history index with the checksum-pinned Markdown tool; and
- link-checks those maintained files plus the runnable GitHub Actions example
  with the checksum-pinned link checker.

Existing focused gates remain authoritative for behavior instead of being
duplicated or weakened:

| Contract area | Executable evidence |
| --- | --- |
| Guide grammar and examples | 61 exact document cases and 35 operation cases in `v0_2_contract_tests` |
| Trust, containment, and output | 65 trust-ledger rows plus #45/#49/#51 focused regressions |
| CLI modes, streams, diagnostics, and broken pipe | `issue_47_output_contract`, `cli_tests`, and `environment_precedence` |
| Recursive zero/ignored behavior | #39/#48 operation rows and recursive/CLI regressions |
| Linux, macOS, and Windows | #55 complete debug/release matrix and strict capability oracles |
| MSRV and dependency policy | `issue_60_msrv_dependency_policy` and the declared package floor |
| Binary-only Rust surface and package | #54/#62 metadata and negative-consumer gates |
| Normative/history ownership | `issue_68_normative_source` |

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| One clearly named normative v0.2 document | The unique claimant remains `docs/v0.2-contract.md`; README and historical records link to it |
| Every required guide, filesystem, verification, CLI, compatibility, API, and security behavior is specified | Existing normative sections plus the new complete CLI, compatibility/support, and security sections; all staged conformance rows are active |
| CLI help, environment, streams, exits, platform/MSRV, and API status cannot drift silently | Exact CLI fixture/Clap/document bijection, #47/#55/#60/#54/#62 focused gates, and maintained-doc CI |
| Security and containment claims are bounded and testable | Stable-filesystem guarantee, explicit hostile-replacement/no-sandbox exclusions, resource non-limits, trust ledger, and platform oracles |
| README and history identify the normative source without contradiction | README links the exact sections; #68 uniqueness/history checks remain green |

## Explicit residual limitations

- Issue #56 remains open and unwaived under the maintainer's temporary
  non-fuzzing operator constraint. This issue does not claim generated
  property coverage.
- No private vulnerability-report route exists yet. Issue #69 remains the
  release blocker that must publish `SECURITY.md` and concrete private
  reporting instructions; public issue content is explicitly rejected for
  sensitive reports.
- The project remains sole-maintainer under the issue #71 exception. No
  organization, backup owner, independent recovery path, or response-time
  guarantee is claimed.
- Network shares, userspace/foreign filesystems, hostile concurrent
  replacement, sandboxing, atomic content publication, and crash durability
  remain outside the supported boundary.

## Validation

| Command or gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Pass |
| `GUIDE_FORMAT_REQUIRE_CONFORMANCE=all cargo test --bin agentic-navigation-guide --locked v0_2_contract_tests::` | Pass; 46 focused contract tests and zero pending rows |
| `GUIDE_FORMAT_REQUIRE_CONFORMANCE=all cargo test --workspace --all-targets --all-features --locked` | Pass |
| `GUIDE_FORMAT_REQUIRE_CONFORMANCE=all cargo test --workspace --all-targets --all-features --release --locked` | Pass |
| README and maintained-document `rumdl` commands from `.github/workflows/ci.yml` | Pass |
| Maintained-document `lychee --offline` command | Pass; 59 local links valid, 15 external links intentionally excluded offline |
| Maintained-document online `lychee` command | Pass; all 74 checked links valid |
| `actionlint .github/workflows/*.yml .github/examples/*.yml` | Pass |
| `zizmor --pedantic --no-ignores .github/workflows/ .github/examples/readme-verify.yml` | Pass; no findings |
| `just test-production-readiness-selector` | Pass; 61 tests |

Hosted CI supplies the binding three-platform and online-link results.
