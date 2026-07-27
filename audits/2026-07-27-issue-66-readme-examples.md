# Issue #66 README and executable-example evidence

Date: 2026-07-27

Issue: [#66](https://github.com/plx/agentic-navigation-guide/issues/66)

## Result

**PASS.** The README is now a concise, copy/paste entry point whose complete
shell-example set, expected quickstart output, package lifecycle, CLI modes,
and GitHub Actions example are continuously checked.

The change does not publish `0.2.0`, create an installation channel, finish the
complete platform matrix, or replace the normative v0.2 contract. It documents
the prepared candidate accurately and leaves those release-program controls
with their existing owners.

## Tests-first evidence

The first issue commit, `b7510f3`, added the fixed four-test README contract
without changing the README, manifest, workflow, or example files. Against the
component baseline `9f43ffea50a4bcfe3cfc3d8cdd28af120726b122`:

- the checked workflow example was absent;
- the README exposed only one shell block instead of the source lifecycle,
  release lifecycle, and clean quickstart contract;
- the packaged README omitted the force-upgrade lifecycle command; and
- the existing executable quickstart behavior itself passed.

The focused result was 1 passed and 3 failed. The original issue's
`init`-without-`--output` defect had already been corrected by intervening CLI
work, so the retained red test does not manufacture that old runtime bug. It
proves the defects still present at #66's baseline: incomplete lifecycle
instructions, no complete copy/paste quickstart, mutable CI guidance, and no
continuous documentation harness.

After the documentation and workflow change, all four fixed tests pass.

## Concise entry point

The README now:

- identifies the installed binary-only CLI as the sole supported v0.2
  product and points Rust `0.1.4` consumers to the complete migration record;
- states that `0.2.0` is prepared but unpublished, Rust `1.85.0` is the floor,
  and Linux, macOS, and Windows are the prepared platform target subject to
  #55's final conformance gate;
- states candidly that the personal repository intentionally has no
  organization or backup owner and links the time-bounded sole-maintainer
  exception;
- makes Cargo/crates.io the only supported `0.2.0` release installation
  channel and gives exact locked install, force-upgrade, and uninstall
  commands;
- separately gives the trusted source-checkout lifecycle used before
  publication;
- runs a clean `cargo new` quickstart with the required `init --output`,
  followed by exact `check` and `verify` results;
- distinguishes `dump`, `init`, `check`, single-guide `verify`, and required
  recursive verification in one short workflow;
- links the complete grammar, configuration, output, trust, supported-product,
  release, and migration contracts instead of retaining the former exhaustive
  and unstable duplicate;
- retains a concise dated-and-rationalized `Known Intentional Divergences`
  index required by the repository documentation policy, while the complete
  behavior stays in the contract and changelog;
- removes the stale docs.rs target, stray emphasis, early-preview language,
  and roadmap; and
- records how useful cleanup from draft PR #21 was incorporated while the
  #68 audit remains authoritative for the proposals that were superseded.

`Cargo.toml` now points its `documentation` metadata at the maintained
`docs/v0.2-contract.md` source rather than the removed Rust-library docs.rs
surface.

## Executable examples

`tests/issue_66_readme_examples.rs` extracts every `sh`, `text`, and `yaml`
README contract block and fails on an added, removed, reordered, or changed
example.

In isolated temporary roots it:

1. runs `cargo new` and the documented `dump`, `init`, `check`, and `verify`
   commands;
2. asserts the created guide and exact three quickstart output lines;
3. runs the required recursive GitHub Actions mode and post-tool-use mode with
   ignored-guide denial;
4. builds Cargo's exact local package rather than contacting an unpublished
   registry version;
5. installs the unpacked package locked and offline, force-reinstalls it,
   checks the installed version, and uninstalls it; and
6. proves the packaged README retains every lifecycle command and maintained
   documentation metadata.

The test uses fixed documented commands and one deterministic Cargo-created
workspace. It adds no randomized, generated-property, mutation, or fuzzing
input work.

## Immutable CI example and continuous checks

The README YAML exactly mirrors the logical lines in
`.github/examples/readme-verify.yml`; the comparison treats Git's LF and CRLF
checkout forms as the same YAML. The complete sample has:

- workflow and job time bounds;
- read-only contents permission;
- nonpersisted checkout credentials;
- a fixed Rust `1.85.0` toolchain;
- full 40-character reviewed action SHAs with release comments;
- exact `0.2.0 --locked` Cargo installation; and
- required recursive verification with GitHub diagnostics and
  `--deny-ignored`.

The existing three-OS build matrix now runs the issue #66 smoke suite on
Ubuntu, macOS, and Windows. The workflow-lint job parses both real workflows
and the README mirror with `actionlint`, audits both with `zizmor`, lints the
README with `rumdl`, and checks internal and external links with `lychee`.

The two new documentation binaries are installed from exact release assets:

| Tool | Version | Linux x86-64 archive SHA-256 |
| --- | --- | --- |
| `rumdl` | `0.2.43` | `01e0dd2d89c07d244c5c93243f7faf2986d2abec68a7cec458e38c25988fbabc` |
| `lychee` | `0.24.2` | `1f4e0ef7f6554a6ed33dd7ac144fb2e1bbed98598e7af973042fc5cd43951c9a` |

CI verifies each checksum before extraction. The workflow-security regression
pins the versions, hashes, invocation, and fail-closed placement.

## Validation

The branch requires:

```text
cargo test --locked --test issue_66_readme_examples -- --nocapture
cargo test --locked --test issue_61_workflow_security_policy -- --nocapture
cargo test --locked --all-targets --all-features
cargo test --locked --all-targets --all-features --release
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo package --locked
cargo run --locked -- check --guide AGENTIC_NAVIGATION_GUIDE.md
cargo run --locked -- verify --guide AGENTIC_NAVIGATION_GUIDE.md --root .
actionlint .github/workflows/*.yml .github/examples/*.yml
zizmor --pedantic --no-ignores .github/workflows/
zizmor --pedantic --no-ignores .github/examples/readme-verify.yml
rumdl check --disable MD010,MD013,MD038 README.md
lychee --no-progress README.md .github/examples/readme-verify.yml
just --fmt --check
git diff --check
```

## Residual ownership

Closing #66 does not:

- complete #55's full platform/capability support matrix;
- add or rehearse #63's trusted publishing, archive, checksum, SBOM,
  provenance, or GitHub Release channels;
- replace #67's complete normative CLI/security/support documentation;
- create #69's public security-reporting policy; or
- perform #72's independent immutable-candidate reassessment.

Any release-channel or platform-scope change from those issues must update the
README and this executable contract before publication.
