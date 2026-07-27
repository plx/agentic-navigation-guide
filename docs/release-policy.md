# Release identity and compatibility policy

## One prepared identity

[`release/identity.toml`](../release/identity.toml) is the machine-readable
prepared-release intent. Its one `version` value is asserted against:

- the root package in `Cargo.toml`;
- the source-less root package in `Cargo.lock`;
- real `cargo metadata`;
- the sole named binary target and its built `--version` output;
- the exact `CHANGELOG.md` heading; and
- an externally supplied candidate or release tag.

Tags use `v{version}`. For the prepared `0.2.0` identity, the only matching tag
input is `v0.2.0`. The CLI uses Clap's bare `version` derivation, so source
code obtains the value from Cargo instead of carrying a second literal.

Run the same fail-closed check used by CI:

```sh
python3 scripts/check_release_identity.py --tag v0.2.0
```

The check validates an input; it does not create a tag, crate, GitHub Release,
or other publication. The trusted publishing workflow passes its real tag ref
to this checker before any release action.

## Maintainer continuity and release authority

The project currently operates under the
[issue #71 single-maintainer exception](https://github.com/plx/agentic-navigation-guide/blob/main/docs/maintainer-continuity.md).
`plx` is the sole GitHub repository owner, crates.io owner, and release
authority; there is no tested independent recovery path or human approval
redundancy. The exception expires on 2026-10-31.

This limitation does not weaken the release identity, package, audit,
provenance, Trusted Publishing, or protected-environment gates. Issue #65
established the strongest operable controls for the personal repository:
pull-request-only `main` changes, current required CI, immutable release tags,
and a tag-scoped owner-approved `release` environment. The absence of an
independent pull-request or deployment approver remains residual risk. Issue
#63's workflow uses short-lived crates.io identity scoped to that environment.
Publication after the expiry date is blocked without a verified backup or a
new explicit maintainer decision.

## Release workflow

[`release/pipeline.toml`](../release/pipeline.toml) records the expected
personal-repository identity. The only production identity is:

- repository `plx/agentic-navigation-guide`;
- workflow `.github/workflows/release.yml`;
- protected environment `release`;
- crate and binary `agentic-navigation-guide`; and
- tag `v0.2.0` for the prepared candidate.

The workflow has two entry points. A manual dispatch is always a
non-publishing rehearsal. A `v*` tag event may reach the `publish` job only
after the one `release-gate` aggregate sees every prerequisite succeed. The
production tag must name the exact prepared version, resolve to the checked-out
commit, and equal current protected `main`; a stale or moved candidate fails
before build, package, attestation, or publication.

The gate requires:

1. source, Cargo, lockfile, CLI, changelog, package-target, and migration
   identity;
2. complete locked debug and release suites on Linux, macOS, and Windows;
3. complete check, test, Clippy, package, and install gates on Rust `1.85.0`;
4. rustfmt, all-target/all-feature Clippy with warnings denied, rustdoc with
   warnings denied, RustSec, license/attribution, workflow, manifest, and
   binary-only compatibility checks;
5. exact `cargo package`, clean-root installation, success/failure smoke, and
   `cargo publish --dry-run`;
6. two isolated release builds on every native runner, requiring byte-identical
   binaries before normalized `.tar.gz` or `.zip` creation;
7. target-OS extraction and success/failure smoke of each exact archive; and
8. re-download, SHA-256 verification, SPDX 2.3 SBOM generation, an in-toto/SLSA
   provenance statement, and complete bundle verification.

The reproducibility claim is deliberately narrow: two release binaries built
on the same hosted runner, toolchain, commit, and locked graph must be byte for
byte identical. Archive timestamps, ownership, order, modes, and gzip/ZIP
metadata are normalized. The project does not claim that different runner
images, operating systems, architectures, or future toolchains produce the
same bytes.

The rehearsal produces the crate archive, three native archives, checksums,
SBOM, provenance statement, installed/smoke evidence, and complete bundle as
short-retention Actions artifacts. It has only `contents: read`; the protected
environment, OIDC permission, attestations API, crates.io, tags, and GitHub
Releases are structurally unavailable. `tag-mismatch` and `package-smoke`
manual choices are deliberate red runs used to prove that early identity or
late installed-artifact failure cannot satisfy `release-gate`.

The tag-only `publish` job re-verifies the downloaded bundle inside the
owner-approved `release` environment. That job alone receives
`id-token: write`, `attestations: write`, and `contents: write`. It creates
Sigstore-backed GitHub build and SPDX attestations, creates or verifies an
immutable GitHub Release, verifies the API's `immutable` result, and only then
exchanges GitHub OIDC identity for a short-lived crates.io token. No repository
or environment publication secret is used.

The crates.io Trusted Publisher must be registered with exactly `plx`,
`agentic-navigation-guide`, `release.yml`, and `release`. GitHub's repository
setting **Enable release immutability** must also be enabled for future
releases. Until both hosted settings are verified, the source mechanism is
ready for rehearsal but real publication remains blocked: OIDC exchange or
the post-release immutability assertion fails before `cargo publish`.

## Non-publishing rehearsal

From the Actions page, run the `Release` workflow on trusted `main` with
candidate tag `v0.2.0` and failure injection `none`. This does not create a tag
or release. Download `release-bundle` from the completed run and verify it:

```sh
python3 scripts/release_artifacts.py verify-checksums \
  --directory target/release-bundle \
  --checksums target/release-bundle/SHA256SUMS
```

The full hosted rehearsal is required before a release decision. Local helper
tests are useful development evidence but do not replace native runner,
protected-environment, OIDC, or hosted artifact behavior.

## Release and recovery runbook

No recovery step may delete, move, recreate, or reuse a `v*` tag.

1. Merge the independently audited candidate through protected `main`; verify
   the continuity exception is active and all hosted controls match their
   checked-in policies.
2. Run the non-publishing rehearsal and both deliberate failure injections.
3. Verify the exact Trusted Publisher and future-release immutability settings.
4. Create `v0.2.0` once at the exact current `main` commit. The tag-triggered
   workflow must pass every gate and pause for `release` approval.
5. If a gate fails before the protected job, fix source through a new pull
   request. The existing immutable tag cannot move, so that candidate is
   abandoned and a new version decision is required.
6. If the protected job fails before an immutable GitHub Release exists,
   correct only the hosted configuration and rerun the same failed workflow
   attempt. Do not push the tag again.
7. If GitHub created a mutable release, the workflow rejects it before
   crates.io authentication. Remove that release record, enable future-release
   immutability, and rerun the same tagged workflow; do not delete or change
   the tag.
8. If an immutable GitHub Release exists but crates.io publication failed,
   configure or restore only the exact Trusted Publisher and rerun the same
   tagged workflow. Recovery downloads and verifies every immutable asset
   against the same checksums, commit, and ref before retrying.
9. If crates.io already contains `0.2.0`, recovery continues only when its
   registry checksum exactly matches the gated `.crate`. A missing or different
   checksum is a stop condition requiring an incident record; it is never
   repaired by moving the tag or replacing release assets.

The workflow intentionally publishes the immutable GitHub Release before the
crate. This ensures a missing immutability control cannot leave a crate
published from a mutable asset set. A short interval where the immutable
release exists before crates.io succeeds is recoverable from the same tag and
bundle.

## Rust and dependency support

Rust `1.85.0` is the minimum supported toolchain for the complete product:
locked dependency resolution, all targets and features, the full test suite,
Clippy, packaging, and installation of the CLI. `Cargo.toml` declares that
floor as `rust-version = "1.85"` and `.clippy.toml` carries the matching
`1.85.0` value. Raising either value is a support-policy change that requires
an intentional pull request, aligned declarations, a refreshed lockfile, and
the same complete validation on the new floor.

The supported stable CI lines are Rust `1.97.1` (current stable) and Rust `1.96.1`
(the immediately previous stable line's latest patch). These exact pins are
updated intentionally when Rust publishes a new stable release. Beta is
informational: its CI job may reveal future incompatibility, but it does not
block an otherwise supported release.

`0.2.0` is prepared but not published, so the crates.io command below is not
available yet. After publication, the release-install command names both the
prepared version and graph:

```sh
cargo install agentic-navigation-guide --version 0.2.0 --locked
```

Before publication, trusted source checkouts use
`cargo install --path . --locked`. After publication, the exact `--version`
prevents a newer compatible release from being selected, while `--locked`
requires the dependency versions reviewed in `Cargo.lock`. An install that
omits either control requests a different candidate or a freshly resolved
graph and is not the reproducible release path.

Project-local Cargo configuration uses
`incompatible-rust-versions = "fallback"` so intentional dependency
resolution prefers releases compatible with the declared Rust floor. The
lockfile is refreshed deliberately, then the MSRV check, tests, Clippy,
package, locked install, `cargo audit`, and third-party license generation are
rerun. Current stable and stable-minus-one run the complete locked test suite;
beta supplies the non-blocking forward signal.

Dependabot opens review-only pull requests each week for Cargo and GitHub
Actions dependencies. It receives no registry or release credentials and cannot
publish or merge its proposals. A maintainer must review the changed graph and
immutable action SHAs, run the same compatibility, security, and license gates,
and merge intentionally.

## `0.2.0` historical Rust baseline

The immutable published `0.1.4` crate is the Rust migration baseline for
`0.2.0`:

- archive SHA-256:
  `d08fefac88faf8d737eea273f86bfbc80aaac1eb80ff3a57bde5add824fe5da0`;
- VCS revision: `560ce399e1e28e8e0d6b87988956893796d2dfab`;
- normalized-manifest SHA-256:
  `1dc83730531459a1fcae387cc5e5f625a3ff498659915d58fa875dd14c9fab3b`;
  and
- published `src/lib.rs` SHA-256:
  `c2107c1948025e592e4af33a39b8f80ce7f02b8160d48c12acf6a4c67963d656`.

The exact last-linkable current-source revision,
`e34399c14683878064cad18e9506186cd7e4fef1`, is a distinct evidence point.
Pinned `cargo-semver-checks 0.49.0` compared that source to the published
artifact and reported the breaks captured in
[`CHANGELOG.md`](../CHANGELOG.md#semver-evidence-and-future-baseline) and the
[#54 evidence audit](../audits/2026-07-26-issue-54-binary-only-package.md).
It must not be substituted for the published baseline.

The prepared package has no library target, so running a library SemVer tool
against the final candidate and treating “no target selected” as success
would be vacuous. The final gate instead asserts zero Rust-linkable targets,
the exact named binary, the complete published-to-v0.2 migration inventory,
and the supported CLI contract.

## Future baseline selection

For every later compatible `0.2.x` candidate:

1. select and record the most recent non-yanked published release in the same
   `0.2.x` compatibility line that precedes the candidate;
2. compare the exact package target shape and the complete documented CLI,
   guide-format, machine-output, exit-status, platform, and trust-boundary
   contract against that release;
3. fail on every incompatibility; an approved release-note entry cannot
   authorize a same-line break; and
4. retain zero Rust-linkable targets.

A first release in a new breaking line has no earlier release in that same
line. It must instead freeze the latest non-yanked published predecessor
across lines, add an approved migration record for every accepted break, and
use the new version to signal that boundary. `0.2.0` follows that bootstrap
rule with the separately pinned `0.1.4` migration baseline above.

A supported Rust library or another breaking change to the complete CLI
contract requires `0.3.0` and a separately approved migration baseline. A
narrow security correction may restore behavior to an already documented
boundary within `0.2.x`, but may not redefine that boundary and must be called
out in release notes or an advisory. Such a restoration is conformance with
the existing boundary, not an accepted incompatibility. The comparison surface
remains the documented CLI contract plus exact package target shape; it is not
an empty Rust API snapshot.

If a future breaking line introduces a supported Rust library, that line must
establish a new published Rust API baseline and use a pinned applicable SemVer
tool in addition to the CLI compatibility gates.
