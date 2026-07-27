# Issue #60 MSRV and Dependency Policy Evidence

Date: 2026-07-26

## Scope and outcome

Issue #60 establishes one supportable Rust floor and a reviewable dependency
maintenance policy for the complete CLI:

- `Cargo.toml` declares Rust `1.85`; `.clippy.toml` declares the matching
  `1.85.0` floor;
- Cargo resolution prefers releases compatible with that declaration;
- CI enforces locked check, tests, Clippy, package, and install on the MSRV;
- exact current-stable and stable-minus-one toolchains run the complete locked
  tests, while beta is explicitly informational;
- user installation documentation requires an exact release and `--locked`;
- the lockfile and bundled third-party attributions are intentionally
  refreshed; and
- weekly Dependabot proposals cover Cargo and GitHub Actions without publish
  or merge authority.

This work uses ordinary bounded builds and deterministic tests. It adds no
fuzzer, nightly-only test workspace, or external model implementation.

Implementation commits:

```text
9bf463784c8fbe335be96744e9113a9dc4ed975f
006cb3752eccda3b3e91d4e2345e5de29e9191dd
c12c3b9674ad44e3332934be85e7027d3831da46
1679d77f7a4f2f64ea413caf364aa93cb700f54a
```

Base revision:

```text
741db489d95c74ca6c5d03d2655d1cf1356b47ee
```

## Baseline and red-before evidence

The issue's historical claim was reproduced from the unmodified base:

```sh
cargo +1.70.0 check --locked
```

It exited `101` before compilation because Cargo 1.70 cannot parse lockfile
version 4. This is a package-and-lock failure, so the old Clippy-only
`msrv = "1.70.0"` claim was not a supported product floor.

Two isolated Rust 1.85 source installs were then compared. The locked install
succeeded with the committed graph. The unlocked install also compiled, but
resolved a materially different 62-package release graph, including
`anstream 1.0.0`, `clap 4.6.4`, `env_filter 2.0.0`, `env_logger 0.11.11`,
`regex 1.13.1`, and `serde 1.0.229`. This demonstrates why the documented
reproducible path requires `--locked`; successful compilation alone does not
make the two graphs identical.

Before policy implementation, the new focused contract ran on Rust 1.85:

```sh
cargo +1.85.0 test --locked --test issue_60_msrv_dependency_policy -- --nocapture
```

All three tests failed for the intended missing controls:

- no manifest Rust floor;
- no exact supported-stable CI lanes; and
- no Dependabot configuration.

The final three-test contract passes in both the working tree and Cargo's
verified unpacked package. It ties the manifest floor to Clippy and the MSRV
CI lane, requires every MSRV gate, checks supported stable and informational
beta lanes, and prevents install and dependency-automation policy drift.

## Chosen floor and compatible graph

The exact minimum compiler is:

```text
rustc 1.85.0 (4d91de4e4 2025-02-17)
commit: 4d91de4e48198da2e33413efdcd9cd2cc0c46688
```

Rust 1.85 is an observed whole-product floor rather than a source-only claim:

- the refreshed metadata reports `rust_version = "1.85"` for the package;
- the compatible graph includes `clap 4.6.4`, `assert_cmd 2.2.2`, and
  `getrandom 0.4.3`, each declaring Rust 1.85;
- all targets/features check and test on 1.85;
- Clippy with warnings denied passes on 1.85;
- Cargo packages and verifies the source on 1.85; and
- an isolated locked source install builds on 1.85 and reports
  `agentic-navigation-guide 0.2.0`.

The project-local resolver setting is:

```toml
[resolver]
incompatible-rust-versions = "fallback"
```

Running `cargo +1.85.0 update` refreshed 69 packages to the latest releases
compatible with the declared floor. The resulting metadata contains 84 crate
dependencies. Two pre-existing `map_or` expressions surfaced under the now
enforced MSRV Clippy gate and were replaced with the equivalent
MSRV-supported `is_none_or` form; no product behavior changed.

The first hosted current-stable Clippy run then enforced Rust 1.97's newer
`io_other_error` lint and identified 11 Linux-visible constructions of
`io::ErrorKind::Other`. The review fix converted those and seven additional
platform-gated occurrences to the equivalent `io::Error::other` constructor,
which has been stable since before the declared floor. Both Rust 1.85 and Rust
1.97 Clippy pass after the cross-platform sweep.

## Supported and forward toolchains

The exact locally validated toolchains are:

| Lane | Compiler commit | Policy | Result |
| --- | --- | --- | --- |
| Rust `1.85.0` | `4d91de4e48198da2e33413efdcd9cd2cc0c46688` | Minimum supported | Pass |
| Rust `1.96.1` | `31fca3adb283cc9dfd56b49cdee9a96eb9c96ffd` | Stable minus one | Pass |
| Rust `1.97.1` | `8bab26f4f68e0e26f0bb7960be334d5b520ea452` | Current stable | Pass |
| Rust `1.98.0-beta.6` | `0c45ca314d60447c808f437c9df49ac81c0dc23d` | Informational | Pass |

Each lane ran the complete locked, all-targets, all-features test suite. The
MSRV additionally ran check, Clippy, package verification, and an isolated
locked install. Current stable also runs Clippy so new lints are visible at
the supported endpoints. CI carries the same exact supported pins, does not
cancel sibling supported lanes after one failure, and gives its beta job
`continue-on-error: true`.

The final full-suite counts per toolchain are:

```text
binary unit tests:       220 passed; 2 intentionally ignored
CLI integration tests:  106 passed
environment tests:        8 passed
package-shape tests:       1 passed
MSRV-policy tests:         3 passed
release-identity tests:    2 passed
total:                   340 passed; 2 intentionally ignored
```

One intermediate Rust 1.85 rerun observed a non-reproduced `Bad file
descriptor` from the existing 100-iteration exclusive-create race test while
it asked the operating system for the process current directory. That test
passed earlier on every toolchain, passed immediately when isolated, and the
complete Rust 1.85 suite passed on the immediate rerun. This record does not
mischaracterize the transient observation as a dependency incompatibility or
claim that it did not occur.

## Dependency security and licensing

`cargo audit 0.22.1` updated the RustSec database, loaded 1,169 advisories, and
scanned all 84 locked crate dependencies without reporting a vulnerability.

Pinned `cargo-about 0.9.0` regenerated `THIRD_PARTY_LICENSES.md` successfully
against the refreshed graph and existing reviewed clarifications. The update
removes superseded package versions and records the selected releases,
including both Syn 2 and Syn 3 and the consolidated Windows target packages.
The generated attribution contains the reviewed MIT and Unicode v3 texts.

The lockfile also records the optional `defmt`, `defmt-macros`,
`defmt-parser`, and `thiserror 2.0.19` feature closure of Jiff. Review with
`cargo tree --locked -i` confirmed those packages are not in the activated
build graph: Jiff has only its `alloc` and `std` features enabled, while its
`defmt` dependency is optional behind the disabled `defmt` feature.
`cargo-about` therefore excludes that inactive feature closure intentionally.
The two active Syn majors are expected: Syn 2 serves the repository's direct
test dependency and `thiserror 1`, while Syn 3 serves the Clap and Serde
derive macros.

Two consecutive generations produced the same SHA-256:

```text
2a9f1e4eb44effff926ae4ddb482ad68f1f6b36a48e1620d6b75c15e158dfd06
```

The existing CI regeneration-and-diff gate remains in place.

## Automation authority and validation

`.github/dependabot.yml` schedules weekly review proposals for the Cargo
lockfile and immutable GitHub Actions references. Its labels were checked
against the live repository. It contains no registries or credentials.
Repository CI now defaults to read-only `contents` permission, and Dependabot
has no workflow that can publish or merge a proposal. Actionlint `1.7.12` is
now a standing hosted check. Its upstream Linux release archive is pinned by
SHA-256
`8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8`
and verified before extraction. Review also consolidated the redundant
standalone default-toolchain test and Clippy jobs into the exact support
matrix: every supported lane tests fully, while the MSRV and current stable
endpoints lint.

The following final gates passed:

```sh
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 check --locked --all-targets --all-features
cargo +1.85.0 test --locked --all-targets --all-features
cargo +1.85.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.97.1 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.85.0 package --locked --allow-dirty
cargo +1.85.0 install --path . --locked --root <isolated-root>
cargo +1.96.1 test --locked --all-targets --all-features
cargo +1.97.1 test --locked --all-targets --all-features
cargo +beta check --locked --all-targets --all-features
cargo +beta test --locked --all-targets --all-features
cargo audit
cargo about generate about.hbs --output-file THIRD_PARTY_LICENSES.md
actionlint .github/workflows/*.yml
just --fmt --check
cargo +1.85.0 run --locked -- check --guide AGENTIC_NAVIGATION_GUIDE.md
cargo +1.85.0 run --locked -- verify \
  --guide AGENTIC_NAVIGATION_GUIDE.md --root .
```

This establishes a maintained floor for the complete distributable CLI,
rather than claiming a compiler that only happens to accept part of the
source tree.
