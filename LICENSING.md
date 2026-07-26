# Licensing clarification

## Historical `0.1.x` licensing-metadata clarification

The published crates.io archives for `agentic-navigation-guide` versions
`0.1.0`, `0.1.1`, `0.1.2`, `0.1.3`, and `0.1.4` contain contradictory
licensing information. In every archive, both `Cargo.toml` and
`Cargo.toml.orig` declare `license = "MIT"`, while the top-level `LICENSE`
file is headed `BSD 3-Clause License` and contains the BSD 3-Clause license
text. The five packaged `LICENSE` files are byte-identical (SHA-256
`d21281a8f9984e0da59ddbf9a101a0d89b5e39f4c55fdfc59de8055cba7a464a`).

| Version | crates.io archive checksum (SHA-256) |
| --- | --- |
| `0.1.0` | `893214ce69c162d7ffbbe7de89186b5a67062162573453a459aeb2a8f9793229` |
| `0.1.1` | `3ee43099cbf9792b90db356b4f0dff5c9cdfb5bacd1c4e8fb12279b7a075f0d4` |
| `0.1.2` | `3a81184291dcd65e4d073ecf0e08ff37085714da3acf4cf4505c200addc94b2b` |
| `0.1.3` | `d37c9c8e57e9a90aa53bc4a57d0b7272c2caa46ad9e1df09503b71282d53b16b` |
| `0.1.4` | `d08fefac88faf8d737eea273f86bfbc80aaac1eb80ff3a57bde5add824fe5da0` |

Published crate archives are immutable, so this clarification does not change
their files or metadata. It does not delete or relicense any `0.1.x` artifact,
and it does not state a legal conclusion about which terms govern use of
those historical artifacts.

Version `0.2.0` declares `MIT OR Apache-2.0` in its Cargo metadata and packages
`LICENSE-MIT` and `LICENSE-APACHE`. Those `0.2.0` facts do not alter the
contents or metadata of the published `0.1.x` archives.

## Maintainer decision for published `0.1.x` versions

Decision date: 2026-07-26

Decision maker: `plx`, repository owner

Approval record:
[`issue #64 owner comment`](https://github.com/plx/agentic-navigation-guide/issues/64#issuecomment-5085632943)

Decision: do not yank `agentic-navigation-guide` versions `0.1.0`, `0.1.1`,
`0.1.2`, `0.1.3`, or `0.1.4` in response to the licensing-metadata discrepancy
documented above. Yanking would not modify, delete, or relicense the immutable
archives, so it would not correct the conflicting manifest and packaged-file
contents. The project will preserve the historical artifacts and publish the
factual clarification instead.

Keeping these versions unyanked is not a compatibility, maintenance, or
support promise. This decision addresses only the recorded
licensing-metadata discrepancy. Any later decision to yank a version for new
licensing, security, or release-management facts requires a separate recorded
decision naming the exact version and reason. No yank action is authorized or
performed by issue #64.

## Current source licensing

Current source and the prepared `0.2.0` package are licensed under either:

- Apache License, Version 2.0
  ([`LICENSE-APACHE`](LICENSE-APACHE)); or
- the MIT license ([`LICENSE-MIT`](LICENSE-MIT)).

[`NOTICE`](NOTICE) records the current dual-license notice.
[`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md) records dependency
licenses and attributions. This section describes current source only and
does not resolve or reinterpret the historical facts above.
