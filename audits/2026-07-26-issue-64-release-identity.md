# Issue #64 Prepared `0.2.0` Release Identity Evidence

Date: 2026-07-26

## Scope and Outcome

Issue #64 prepares, but does not publish, one coherent `0.2.0` identity:

- `Cargo.toml`, the source-less root package in `Cargo.lock`, real Cargo
  metadata, the sole built binary, the changelog heading, and an externally
  supplied candidate tag all assert `0.2.0`;
- the installed `agentic-navigation-guide` CLI remains the sole supported v0.2
  product, with zero Rust-linkable targets or in-process compatibility shim;
- `CHANGELOG.md` is the cumulative approved v0.2 release contract and complete
  published-`0.1.4` Rust migration inventory;
- immutable published `0.1.4`, rather than a later development commit, is the
  one-time Rust migration baseline;
- future compatible and breaking-line baseline rules fail closed;
- the contradictory immutable `0.1.x` licensing facts and explicit no-yank
  owner decision are recorded without a legal conclusion; and
- no crate is published and no tag or GitHub Release is created.

The changelog distinguishes prepared identity facts from the cumulative v0.2
target contract. Present-tense target behavior is not evidence that every
focused implementation ticket has landed. Publication remains blocked until
the contract handoff is complete and the final candidate is revalidated.

This ticket does not own the final package-file allowlist and binary-only
negative consumer (#62), trusted publishing workflow (#63), maintained
documentation metadata and badge (#66), or formal publication (#73).

## Approved Owner Decisions

The approved #36 Rust-surface handoff is recorded on issue #64:

<https://github.com/plx/agentic-navigation-guide/issues/64#issuecomment-5081615343>

It requires a deliberate `0.2.0` break, complete published-API removal
inventory, no Rust shim, migration to the CLI process or machine contract, and
compatibility of the complete documented CLI contract throughout `0.2.x`.

The repository owner separately approved both historical-license decision
groups:

<https://github.com/plx/agentic-navigation-guide/issues/64#issuecomment-5085632943>

The approved statement is factual: every published `0.1.0` through `0.1.4`
archive declares MIT in its manifests while packaging the same BSD 3-Clause
root `LICENSE`. The clarification cannot change, delete, or relicense an
immutable artifact and makes no legal conclusion.

The owner decision is to leave all five versions unyanked for this discrepancy.
That decision is not a support, maintenance, or compatibility promise. A later
yank requires a separately recorded exact-version reason. Issue #64 performs
no yank.

## Red-Before Evidence

The following checks were run from a working tree rooted at base revision
`f58323f8fd83860e86ab6628e8630a2fe8c6c923`, after installing the provisional
red checker, intended-identity file, published-API fixture, and Rust regression
as an uncommitted test overlay. `Cargo.toml` and `Cargo.lock` still reported
`0.1.4`, `CHANGELOG.md` was absent, and no product fix had been applied. This
was deliberately not described as an untouched or clean base commit: the test
harness files do not exist in `f58323f`, and the harness was tightened further
after the recorded failures.

```sh
python3 scripts/check_release_identity.py --tag v0.2.0
```

The command exited `1`. It reported:

- `Cargo.toml` version `0.1.4`, expected `0.2.0`;
- source-less root `Cargo.lock` version `0.1.4`, expected `0.2.0`;
- missing `CHANGELOG.md`;
- real Cargo metadata version `0.1.4`; and
- built CLI output `agentic-navigation-guide 0.1.4`.

```sh
cargo test --locked --test issue_64_release_identity -- --nocapture
```

The command exited `101`. The published-baseline test reported the missing
changelog. The real exact-package/install path succeeded structurally but
reported `0.1.4` in both packaged manifests and the installed CLI instead of
the required `0.2.0`.

These failures prove the absent version, changelog, packaged identity, and
installed-binary controls rather than an environmental failure.

## One Prepared Identity

[`release/identity.toml`](../release/identity.toml) is the machine-readable
prepared intent. Its fail-closed schema asserts:

```text
package:               agentic-navigation-guide
version:               0.2.0
binary:                agentic-navigation-guide
tag convention:        v{version}
changelog heading:     ## [0.2.0] - Unreleased
license:               MIT OR Apache-2.0
supported product:     cli
Rust-linkable targets: 0
```

The checker rejects missing, extra, or misspelled identity keys. It compares
the intent with both Cargo manifests, real `cargo metadata`, the exact sole
binary reported by Cargo JSON build output, that binary's `--version` output,
the exact changelog heading, and the external `--tag` input. The Clap
declaration must use bare `version`; a second hard-coded source version fails.

The checker validates an external candidate input. It does not create a tag or
publish anything. #63's future trusted publishing workflow must pass its real
tag ref through this check before any release action.

## Published `0.1.4` Migration Baseline

The immutable published crate and exact last-linkable current source are
separate evidence points:

| Evidence | Exact value |
| --- | --- |
| Published version | `0.1.4` |
| Published archive SHA-256 | `d08fefac88faf8d737eea273f86bfbc80aaac1eb80ff3a57bde5add824fe5da0` |
| Published VCS revision | `560ce399e1e28e8e0d6b87988956893796d2dfab` |
| Normalized manifest SHA-256 | `1dc83730531459a1fcae387cc5e5f625a3ff498659915d58fa875dd14c9fab3b` |
| Published `src/lib.rs` SHA-256 | `c2107c1948025e592e4af33a39b8f80ce7f02b8160d48c12acf6a4c67963d656` |
| Last-linkable development revision | `e34399c14683878064cad18e9506186cd7e4fef1` |

The canonical published-API fixture contains 128 explicit rows:

| Kind | Rows |
| --- | ---: |
| Package target | 1 |
| Modules | 7 |
| Root re-exports | 17 |
| Type aliases | 1 |
| Structs | 10 |
| Enums | 6 |
| Variants | 38 |
| Public fields | 19 |
| Free functions | 7 |
| Inherent methods | 22 |

The fixture metadata pins:

```text
ordered IDs SHA-256:
  3b1fa66f32a32aa48430993d9e69a7fa0b9566942efd17f8dfe657b6d1e8ddb7
ordered symbols SHA-256:
  7d6f9b7f320cb6394bfbf4b54657e4bddece662b15cc5b24cd1e409aab39ef88
ordered complete rows SHA-256:
  ab476288fae6998d16ee2a500825cf04a26b5564c3e59a9ed95824ed0193611f
fixture file SHA-256:
  f1263f88e72ae790e62a474f48b18745963c72b921a8f299e0afb9286313f3a7
```

The Rust regression renders the changelog inventory from the fixture and
requires a byte-for-byte match inside dedicated markers. It also checks exact
row and category counts, unique IDs and symbols, all metadata values, and the
published trait commitments. A short or incidental symbol match cannot satisfy
the migration inventory.

## Pinned SemVer Evidence

The #54 pre-removal report remains immutable and is consumed rather than
recomputed against a candidate with no library target:

```text
#54 audit SHA-256:
  b7db882a03c3f19bf2b194c6fcf2f1ab504a99cbd8425cbde6327d86ad7c2313
cargo-semver-checks:       0.49.0
executable SHA-256:
  dd13a57b19aaedcb9d520f3d0cfc6af0005c04b4e1521ac9d81cdc513a13ec16
Rust / Cargo:              1.93.0 / 1.93.0
exit code:                 100
evaluated / passed:        196 / 192
major failures:            4
inapplicable checks:       57
```

The four major lint IDs are:

```text
enum_no_repr_variant_discriminant_changed
enum_variant_added
enum_variant_missing
inherent_method_missing
```

An empty final `cargo-semver-checks` selection is not treated as success. The
final binary-only gate instead checks exact target shape, the complete
published-to-v0.2 migration inventory, and the supported CLI contract.

## Future Compatibility Baselines

A later compatible `0.2.x` candidate selects the most recent preceding
non-yanked published release in the same compatibility line. It compares the
exact package target shape and complete documented CLI, guide-format,
machine-output, exit-status, platform, and trust-boundary contract. Every
incompatibility fails; an approved release note cannot authorize a same-line
break.

A narrow security correction may restore conformance to an already documented
boundary within `0.2.x`, but it cannot redefine that boundary. A supported Rust
library or any accepted breaking CLI-contract change requires `0.3.0`.

The first candidate in a new breaking line freezes the latest non-yanked
published predecessor across lines and records every approved break in a
separate migration record. A line that adds a supported Rust library also
establishes and checks a new pinned Rust API baseline.

## Historical Licensing Evidence

The crates.io archives inspected for the factual clarification are:

| Version | Archive SHA-256 | Yanked |
| --- | --- | --- |
| `0.1.0` | `893214ce69c162d7ffbbe7de89186b5a67062162573453a459aeb2a8f9793229` | no |
| `0.1.1` | `3ee43099cbf9792b90db356b4f0dff5c9cdfb5bacd1c4e8fb12279b7a075f0d4` | no |
| `0.1.2` | `3a81184291dcd65e4d073ecf0e08ff37085714da3acf4cf4505c200addc94b2b` | no |
| `0.1.3` | `d37c9c8e57e9a90aa53bc4a57d0b7272c2caa46ad9e1df09503b71282d53b16b` | no |
| `0.1.4` | `d08fefac88faf8d737eea273f86bfbc80aaac1eb80ff3a57bde5add824fe5da0` | no |

In every archive, normalized `Cargo.toml` and original `Cargo.toml.orig`
declare `license = "MIT"`. Every archive's top-level `LICENSE` is the same BSD
3-Clause text with SHA-256:

```text
d21281a8f9984e0da59ddbf9a101a0d89b5e39f4c55fdfc59de8055cba7a464a
```

[`LICENSING.md`](../LICENSING.md) preserves those facts, the immutable-history
boundary, and the approved no-yank decision. It does not determine which
historical terms govern use.

Current source and the prepared package instead declare
`MIT OR Apache-2.0`. The package regression requires reviewed, nonempty,
byte-identical copies of `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`,
`THIRD_PARTY_LICENSES.md`, and `LICENSING.md`, and rejects the obsolete
ambiguous root `LICENSE`.

## Exact Package and Install Proof

The focused Rust regression:

1. builds the exact working tree with locked, offline package resolution in an
   isolated target directory;
2. inspects Cargo's verified unpacked `.crate`;
3. checks both normalized `Cargo.toml` and original `Cargo.toml.orig` for
   version `0.2.0` and license `MIT OR Apache-2.0`;
4. checks the reviewed legal files and rejects an ambiguous root `LICENSE`;
5. fetches dependencies into a clean temporary `CARGO_HOME`;
6. installs the exact unpacked package locked and offline into an isolated
   target and root; and
7. executes that installed binary and requires exactly
   `agentic-navigation-guide 0.2.0`.

The integration regression uses `cargo package --allow-dirty` so it can test
the exact working tree during normal development. CI separately runs the
ticket's literal acceptance command against its clean checkout:

```sh
cargo package --locked
```

No publish command or credential is used.

## No-Publication Proof

Fresh external checks on 2026-07-26 reported:

- the official crates.io API's `newest_version` and `max_version` were both
  `0.1.4`; the returned version set was exactly `0.1.0` through `0.1.4`, all
  unyanked;
- GitHub's matching-tag API returned `[]` for `v0.2.0`;
- local `git tag --list v0.2.0` and remote
  `git ls-remote --tags origin refs/tags/v0.2.0` returned no ref; and
- GitHub's release-by-tag API returned HTTP `404` for `v0.2.0`.

The issue branch adds a prepared-identity CI job only. It does not add or
invoke `cargo publish`, create a tag, or create a GitHub Release.

## Validation

The final issue branch passes:

```sh
PYTHONDONTWRITEBYTECODE=1 \
  python3 -m unittest tests/test_check_release_identity.py -v
python3 scripts/check_release_identity.py --tag v0.2.0
cargo test --locked --test issue_64_release_identity -- --nocapture
cargo test --workspace --all-targets --all-features --locked --no-fail-fast
GUIDE_FORMAT_REQUIRE_CONFORMANCE=all \
  cargo test --locked --bin agentic-navigation-guide \
  'v0_2_contract_tests::' -- --nocapture
cargo test --locked --test issue_54_binary_only_package -- --nocapture
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo fmt --check
cargo check --locked --all-targets --all-features
cargo package --locked
just --fmt --check
just test-production-readiness-selector
git diff --check
```

The release checker mutation suite contains 14 tests covering manifest,
lockfile, Cargo metadata, external tag, changelog, CLI output, Clap source,
identity schema, baseline evidence, canonical fixture hashes, and exact Cargo
JSON executable selection. The two Rust release-identity regressions prove the
128-entry changelog bijection and exact packaged/install identity.

The frozen decision and conformance ledgers remain byte-identical:

```text
v0_2_api.rs
  1e2515e134a61e2dd297d3de3127daee5074c75f70e29f53ca176af4fe60110a
v0_2_operations.rs
  cc3a401a0883cc8686b0cb59e743f64e323607b91b395a49cfa714162f1b8b56
v0_2_contract.rs
  0e87357dff1fab9afb30d65a4d459b7f6b8be51a278da5b83623049bcf0fa1d7
v0_2_trust.rs
  26162d45509da30737f11893529e05965c53ff70a8fabaceb66a033a615a9d08
```
