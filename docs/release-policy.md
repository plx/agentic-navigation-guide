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
or other publication. The trusted publishing workflow owned by issue #63 must
pass its real tag ref to this checker before any release action.

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
#63 must use short-lived crates.io identity scoped to that environment.
Publication after the expiry date is blocked without a verified backup or a
new explicit maintainer decision.

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
