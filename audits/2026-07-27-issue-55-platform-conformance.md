# Issue #55 cross-platform conformance evidence

Date: 2026-07-27

Issue: [#55](https://github.com/plx/agentic-navigation-guide/issues/55)

## Scope

This gate changes no product behavior. It promotes the complete existing
behavioral suite from Ubuntu-only execution to binding Linux, macOS, and
Windows execution in both debug and release modes. It also makes the exact
host-applicable trust handoff, capability policy, and intentional-ignore
allowlist reviewable in source and CI output.

The work is independent of a GitHub organization. It adds no fuzzing,
randomized generation, generated hostile inputs, or hostile-mutation claim.
The deterministic transient-entry fixture and fixed creator-race loops are
retained product regressions, not generated-input testing.

## Tests-first baseline

Commit `a80a20c` added
`tests/issue_55_platform_conformance.rs` before the workflow or documentation
changed. The focused command failed four policy groups:

```text
every matrix host must run the complete locked debug suite with auditable output
prepared release validation must wait for the complete platform matrix
the supported-platform contract must classify intentional ignore "benchmark_flat_hierarchy_scaling"
README must not retain the pre-conformance support disclaimer
```

The same run found exactly three existing `#[ignore]` attributes. Its ignore
test failed only because the normative classification was not yet present,
not because an additional hidden skip existed.

## Combined-suite compatibility finding

The first complete local debug run passed the exactly-one-success assertion
for an output creator race, then failed an older follow-up assertion that
allowed only `Existing` or `Unsafe` as the loser's typed error. The normative
`trust-output-creator-race` outcome requires exactly one creator and forbids
overwrite; it does not prescribe the loser's diagnostic variant when another
I/O stage fails.

The retained regression asserts exactly one success and compares the final
bytes with that unique winner. The former loser-variant assertion was
redundant with exactly-one success and imposed an outcome the contract does
not require. The regression does not accept two winners, zero winners,
replacement, or unexpected content. No product error mapping or runtime
behavior changed.

## Binding matrix

The `Build` job retains the exact runner set:

- `ubuntu-latest`;
- `macos-latest`; and
- `windows-latest`.

Every runner executes:

```sh
GUIDE_FORMAT_REQUIRE_CONFORMANCE=all \
  cargo test --workspace --all-targets --all-features --locked -- --nocapture
GUIDE_FORMAT_REQUIRE_CONFORMANCE=all \
  cargo test --workspace --all-targets --all-features --release --locked -- --nocapture
cargo test --workspace --all-targets --all-features --locked \
  trust_evidence -- --nocapture
```

The first two commands compile and execute every test target and feature from
the committed lockfile. The third deliberately repeats the exact guide,
output, and containment trust oracles so their capability results cannot be
buried among ordinary suite output.

The existing focused invocations remain after these binding commands to keep
per-issue failure attribution visible in the Actions interface. They reuse
the compiled target artifacts; only the complete commands above define
platform coverage.

## Capability and skip audit

The complete suite includes fixed fixtures for:

- exact case and Unicode identity on aliasing and non-aliasing filesystems;
- POSIX and Windows drive, rooted, UNC, separator, reserved-name, stream, and
  device path behavior;
- file, directory, dangling, chained, and looping links;
- Windows junctions/reparse entries and real link privileges;
- Unix permission failures and Windows DACL denial;
- deterministic transient disappearance; and
- exactly-one-winner exclusive creation.

The #45 output oracle and #49 guide oracle require an empty unavailable set on
Windows. Windows symlink, junction/reparse, DACL, stream, device, sentinel, and
creator-race evidence therefore fail closed. Unix jobs declare only the exact
Windows-only trust rows unavailable; Unix links and permission cases execute
for real. Case and Unicode fixtures choose an observed alias or distinct-name
branch and execute either way.

The three intentional ignores are:

1. a manual parser hierarchy benchmark;
2. a manual serialized #50 release benchmark; and
3. the #62 packaged-artifact acceptance test, which CI invokes explicitly
   once with `--ignored`.

`issue_55_intentional_ignore_allowlist_is_exact_and_documented` scans every
Rust source and rejects any addition or changed rationale.

## Release dependency

The prepared `release-identity` job now has `needs: build`. A failure in any
member of the platform matrix prevents package preparation, dry-run
publication, and packaged-CLI smoke from running as a successful release
path.

There is intentionally no actual publication workflow yet; issue #63 owns the
non-publishing rehearsal and later trusted-publishing pipeline. The normative
contract requires that future workflow to depend on or invoke this same
locked matrix. This issue does not create a tag, release, package publication,
organization, or environment.

## First hosted gap discovery

The first expanded [hosted matrix run][first-matrix-run] passed Linux and
failed the [Windows job][first-windows-job] in
`windows_output_reparse_matrix`. The Windows unit suite had executed 203
tests with two intentional ignores; the CLI suite then executed 94 tests and
failed one rather than merely compiling.

The failure exposed cross-case fixture contamination in the Windows
regression. Its first subcase intentionally placed an unsafe directory link
inside the selected input root and proved that an output beneath it was
rejected. The later subcase reused that root while proving that an explicitly
selected external output link is permitted. Source generation correctly
rejected the unsafe link that the first subcase had left behind.

The regression now removes only that directory-link fixture after the
rejection assertions. The external target remains alive, and the later
external-output case runs against a clean input root. No production path,
parser input, trust classification, or expected output changed.

The next [hosted matrix run][second-matrix-run] passed that reparse regression
and exposed a separate [Windows-only test failure][second-windows-job].
`issue_47_recursive_github_error_has_discovery_path_and_line` hard-coded `/`
in its expected root-relative diagnostic, while the product consistently
renders the logical `Path` with the host-native separator and emitted `\` on
Windows. The test now builds its expected location with
`std::path::MAIN_SEPARATOR`. The diagnostic remains root-relative, includes
the Unicode path and exact line, and discloses no resolved external target.
Again, no production behavior changed.

## Hosted evidence

The pull request must retain:

- one successful Linux matrix job;
- one successful macOS matrix job;
- one successful Windows matrix job;
- a deliberate Windows-only invariant failure in which the Windows matrix
  job becomes red while the non-Windows hosts remain green; and
- a final successful run after reverting that injected failure.

Those immutable run and job links are added here before merge. A successful
compile without executed tests is not accepted.

## Residual boundary

The supported matrix covers ordinary local filesystems through the operating
system's standard APIs and the current GitHub-hosted default workspaces. It
does not certify network shares, userspace filesystems, foreign filesystem
drivers, privileged device-node construction, or safety against hostile
concurrent replacement. The product remains a stable-filesystem consistency
checker, not a sandbox or access-control boundary.

[first-matrix-run]: https://github.com/plx/agentic-navigation-guide/actions/runs/30276524161
[first-windows-job]: https://github.com/plx/agentic-navigation-guide/actions/runs/30276524161/job/90011882496
[second-matrix-run]: https://github.com/plx/agentic-navigation-guide/actions/runs/30276909396
[second-windows-job]: https://github.com/plx/agentic-navigation-guide/actions/runs/30276909396/job/90013172856
