# Issue #62: reviewed source-package boundary

Date: 2026-07-26

Issue: [#62 — Use an explicit Cargo package allowlist and smoke-test the
packaged crate](https://github.com/plx/agentic-navigation-guide/issues/62)

## Outcome

The prepared `0.2.0` source crate has a root-anchored, exact 33-path package
allowlist. It contains only:

- Cargo's normalized/original manifest, lockfile, and VCS provenance record;
- the production binary sources;
- README, changelog, normative contract, and release policy; and
- both license texts, the historical licensing clarification, NOTICE, and
  generated third-party attribution.

Repository workflows, the static site, agent/context material, release-control
internals, audit and remediation evidence, integration tests, and test
fixtures are excluded. They remain in the repository and its CI; exclusion
from the crate is not deletion.

The package contract is deterministic. It adds no fuzzing, mutation testing,
or unbounded input generation.

## Required pre-fix evidence

The exact audited base was merge commit
`4ab6e72efec8a1bd598a2c0eac7184691be0ef4c`. Its package metadata only
excluded `.context/**`.

```text
cargo package --list --locked
126 paths
```

Representative unintended paths included:

```text
.github/workflows/ci.yml
AGENTIC_NAVIGATION_GUIDE.md
audits/production-readiness-remediation-goal.md
remediations/DONE-P1-01-SilentFailures.md
site/package-lock.json
tests/fixtures/v0_2_trust.rs
```

After the regression was written, the exact current test binary was pointed at
a detached worktree of `4ab6e72` with `ISSUE_62_PACKAGE_ROOT`. The focused
manifest test exited `101`: the observed 126-path list did not equal the
reviewed 33-path list. This is fixed-revision red-before evidence; it does not
generate or mutate package inputs.

## Allowlist design

Every include pattern starts with `/`. Cargo include patterns use gitignore
matching semantics; an initial unanchored draft demonstrated that names such
as `README.md`, `CHANGELOG.md`, and `NOTICE` would also select matching files
under a locally present ignored `site/node_modules/` tree. Root anchoring
eliminates that ambient-directory dependency, and the exact expanded manifest
test prevents it from returning.

The reviewed expanded manifest is:

```text
.cargo_vcs_info.json
CHANGELOG.md
Cargo.lock
Cargo.toml
Cargo.toml.orig
LICENSE-APACHE
LICENSE-MIT
LICENSING.md
NOTICE
README.md
THIRD_PARTY_LICENSES.md
docs/release-policy.md
docs/v0.2-contract.md
src/cli/check.rs
src/cli/dump.rs
src/cli/environment.rs
src/cli/generation_options.rs
src/cli/init.rs
src/cli/mod.rs
src/cli/output.rs
src/cli/verify.rs
src/dumper.rs
src/entry_type.rs
src/errors.rs
src/exclusion.rs
src/guide_input.rs
src/main.rs
src/parser.rs
src/path_codec.rs
src/recursive.rs
src/types.rs
src/validator.rs
src/verifier.rs
```

The crate deliberately excludes repository integration tests, internal unit
test modules, fixtures, and their audit-only documentation dependencies. The
package is verified and built by Cargo, then its installed binary is exercised
externally from the unpacked artifact. This preserves the published product
boundary without making repository-only conformance machinery part of the
consumer artifact.

## Exact package and smoke contract

`tests/issue_62_package_boundary.rs` provides two layers:

1. an ordinary fast test that compares `cargo package --list` byte-for-byte by
   path against the 33-entry manifest and asserts CI retains every package
   acceptance command; and
2. one explicitly selected acceptance test that builds the real package in an
   isolated target, inspects Cargo metadata, installs from Cargo's unpacked
   `target/package/agentic-navigation-guide-0.2.0`, and exercises only the
   installed executable.

The exact package reports 659.8 KiB of source and 151.2 KiB compressed. The
archive is 154,782 bytes, comfortably below the test's conservative 1,000,000
byte ceiling. Metadata reports exactly one `bin` target and zero `lib`,
`rlib`, `dylib`, `cdylib`, `staticlib`, or `proc-macro` targets.

The installed-artifact smoke suite covers:

- exact `--version`;
- root, `dump`, `init`, `check`, and `verify` help;
- successful and failing `check`;
- successful and failing `verify`;
- default, post-tool-use, pre-commit, and GitHub Actions failure exit modes;
- a real dump/check/verify round-trip;
- successful recursive guide discovery;
- fail-closed zero-guide discovery and explicit `--allow-empty`; and
- required logical-path diagnostics.

An exact path-dependent consumer then resolves the unpacked package and tries
to import `agentic_navigation_guide`. It must fail with both Cargo's
`missing a lib target` warning and Rust error `E0432` for the unresolved crate.
An arbitrary dependency-resolution or compilation failure is not accepted.

## CI enforcement

The release-identity job runs the exact manifest assertion and the explicitly
selected packaged-artifact smoke test, followed by
`cargo publish --dry-run --locked`. The MSRV job first executes
`cargo package --locked` and now installs from its unpacked
`target/package/agentic-navigation-guide-0.2.0` directory instead of the
working tree.

The expensive smoke test is marked ignored in ordinary `cargo test` runs so
the Rust/toolchain and operating-system matrices do not redundantly rebuild
the same source artifact. Its exact ignored invocation is itself asserted by
the fast package-policy test.

## Validation

| Command | Result |
| --- | --- |
| `cargo test --locked --test issue_62_package_boundary issue_62_package_manifest_is_the_exact_reviewed_allowlist -- --exact --nocapture` | Pass; exact 33-path manifest and CI wiring |
| `cargo test --locked --test issue_62_package_boundary issue_62_exact_package_installs_smokes_and_rejects_library_consumers -- --exact --ignored --nocapture` | Pass; package/install/CLI/negative-consumer contract |
| Same manifest test with `ISSUE_62_PACKAGE_ROOT` set to detached `4ab6e72` | Expected failure, exit 101; observed 126 paths |
| Both #62 tests under Rust `1.85.0` | Pass; exact manifest and full packaged-artifact smoke |
| `cargo package --list --locked` | Pass on the clean committed tree; exactly 33 paths |
| `cargo package --locked` | Pass on the clean committed tree; 33 files, 659.8 KiB / 151.2 KiB compressed |
| `cargo publish --dry-run --locked` | Pass on the clean committed tree; upload correctly aborted as a dry run |
| `cargo test --locked --all-targets --all-features` | Pass; 345 passed, 3 intentionally ignored |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass |
| `cargo check --locked --all-targets --all-features` | Pass |
| `cargo fmt -- --check` | Pass |
| `actionlint .github/workflows/*.yml` | Pass |
| `GH_TOKEN=… zizmor --pedantic --no-ignores .github/workflows/` | Pass; zero reported findings with online audits enabled |
| `cargo run --locked -- check/verify AGENTIC_NAVIGATION_GUIDE.md` | Pass |
| `python3 scripts/check_release_identity.py --tag v0.2.0` | Pass |
| `just test-production-readiness-selector` | Pass; 61 tests |

Clean-tree package, publish dry-run, full-suite, lint, guide, and hosted
results are recorded before merge.

The first full-suite attempt encountered one OS `Bad file descriptor` while an
existing contract case spawned an ignored-command subprocess alongside the
suite's concurrent creator tests. The exact case immediately passed alone and
the condition did not recur. A second full run then correctly caught that
#60's CI-policy regression still required the superseded working-tree install
string. That assertion now requires the stronger unpacked-package path; its
focused suite and the final complete suite pass.
