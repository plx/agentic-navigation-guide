# Issue #63 release-pipeline evidence

Date: 2026-07-27

Repository: `plx/agentic-navigation-guide`

Pull request:
[#135](https://github.com/plx/agentic-navigation-guide/pull/135)

## Decision boundary

This issue builds and rehearses the release mechanism. It does not create the
`v0.2.0` tag, publish a crate, or create a GitHub Release. The repository
remains personal under the issue #71 single-maintainer exception: no
organization, team, second administrator, or independent release approver is
assumed.

The reviewed source and hosted rehearsal are complete. GitHub's
repository-level immutable-release setting is enabled, and the tag-scoped
`release` environment is configured. The exact crates.io Trusted Publisher is
not yet configured, so the pull request and issue remain open and real
publication remains blocked.

No fuzzing, randomized generation, mutation campaign, tag, release, crate
publication, or other external release state was created by this work.

## Implemented release contract

[`release/pipeline.toml`](../release/pipeline.toml) and
[`scripts/release_artifacts.py`](../scripts/release_artifacts.py) define and
verify one release identity:

- repository `plx/agentic-navigation-guide`;
- workflow `.github/workflows/release.yml`;
- protected environment `release`;
- crate and binary `agentic-navigation-guide`;
- prepared version `0.2.0` and candidate tag `v0.2.0`; and
- Rust `1.85.0` as the full-product minimum supported toolchain.

The workflow accepts only a manual non-publishing rehearsal or a `v*` tag. A
production tag must match the prepared version, resolve to the exact checked
out commit, and equal current protected `main`. Every remote action is pinned
to a full commit. All ordinary jobs have read-only contents permission. Only
the tag-only `publish` job, after the aggregate gate and protected-environment
approval, receives OIDC, attestation, and contents-write permissions.

The aggregate requires source identity; release-quality and supply-chain
checks; full debug and release suites on Linux, macOS, and Windows; full MSRV
checks; exact crate packaging, clean installation, success and failure smoke,
and Cargo publication dry run; native archive rebuild and target-OS smoke;
checksums; SPDX 2.3 SBOM; provenance; and downloaded-bundle verification.

The tag-only job reverifies the gated bundle, creates GitHub build and SBOM
attestations, creates or verifies an immutable GitHub Release, and verifies
the release API's immutable result before requesting short-lived crates.io
identity. Recovery never moves, deletes, recreates, or reuses a release tag.

## Hosted positive rehearsal

[Release run
30291537804](https://github.com/plx/agentic-navigation-guide/actions/runs/30291537804)
passed the complete non-publishing DAG on the pull request:

| Gate | Result |
| --- | --- |
| Exact source identity | Passed |
| Quality and supply-chain gates | Passed |
| Full Linux, macOS, and Windows debug/release suites | Passed |
| Full Rust `1.85.0` gates | Passed |
| Exact crate package, clean install, smoke, and dry run | Passed |
| Two native builds and deterministic archives per OS | Passed |
| Download, checksums, SPDX, provenance, and bundle verification | Passed |
| Aggregate release gate | Passed |
| Protected publish job | Skipped by the non-tag event |

The first hosted archive rehearsal detected that independently built Windows
PE binaries differed. The final implementation fixes the source of that
nondeterminism with MSVC `/Brepro` plus a fixed source epoch. Run
`30291537804` then required matching bytes before each of the three normalized
archives was created and smoke-tested.

The uploaded `release-bundle` was downloaded independently after the run.
`verify-bundle` accepted its three native archives, exact crate, SHA-256
manifest, SPDX document, and in-toto provenance. The provenance recorded the
tested pull-request merge commit and `refs/pull/135/merge`, rather than
claiming a tag or protected-main source that did not exist.

## Hosted fail-closed evidence

[Release run
30290452743](https://github.com/plx/agentic-navigation-guide/actions/runs/30290452743)
used a deliberate tag/version mismatch. Exact source identity failed before
any downstream build, artifact, aggregate, or publication job could succeed.

[Release run
30292694917](https://github.com/plx/agentic-navigation-guide/actions/runs/30292694917)
used the deterministic `package-smoke` injection. Exact package construction
and clean installation passed, then the installed success/failure behavior
step rejected the deliberately altered expected version against the observed
`agentic-navigation-guide 0.2.0`. The crate dry run and upload were skipped,
bundle assembly could not run, the aggregate gate failed, and the protected
publish job was skipped.

The temporary pull-request trigger and forced failure selection used to obtain
this hosted evidence were removed after the proof. The final workflow exposes
failure injection only on manual non-publishing rehearsals.

## Hosted controls

| Control | Status on 2026-07-27 |
| --- | --- |
| Personal repository | `plx/agentic-navigation-guide`; no organization required or implied |
| Protected environment | `release`; `v*` tags only, owner approval required, administrator bypass disabled, no publication secret |
| Release-tag controls | Owner-only creation and no-bypass update/deletion prohibition |
| GitHub immutable releases | Enabled and verified through the repository API for future releases |
| crates.io Trusted Publisher | Pending: must be exactly owner `plx`, repository `agentic-navigation-guide`, workflow `release.yml`, environment `release` |

The available local crates.io credential could not authenticate to the Trusted
Publisher configuration endpoint, and no signed-in browser session was
available. No credential value was printed, copied, changed, or committed.
This is an external configuration blocker, not a source fallback: the workflow
contains no long-lived publication token and fails closed until the exact
publisher exists.

## Acceptance mapping

| Acceptance criterion | Evidence and disposition |
| --- | --- |
| One non-publishing rehearsal passes end to end | Complete hosted run `30291537804` passed; independently downloaded bundle passed verification |
| Every gate is fail-closed and required by publish | One aggregate depends on every prerequisite; tag mismatch and installed-package failure both prevented downstream success |
| Crate and binaries trace to one immutable commit/tag | Provenance and checksums bind all rehearsal artifacts to one tested commit/ref; production additionally requires an immutable exact tag at current `main` |
| Checksums, SBOM/provenance, licenses, and smoke results exist | Produced and verified in the successful hosted bundle |
| Trusted publishing and protected approval use no exposed long-lived token | Protected environment is configured and source uses tag-only OIDC; exact crates.io publisher remains pending |
| Failure injection proves publication cannot bypass a failed gate | Hosted tag-mismatch and package-smoke runs both blocked publication |
| Recovery avoids tag mutation | Checked-in runbook requires same-run recovery and forbids moving, deleting, recreating, or reusing a tag |

Issue #63 is not complete until the exact crates.io Trusted Publisher is
configured and verified. No acceptance exception is recorded for that
criterion.
