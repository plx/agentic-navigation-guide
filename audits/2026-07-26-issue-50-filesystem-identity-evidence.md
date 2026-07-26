# Issue #50 Filesystem Identity and Snapshot Evidence

Date: 2026-07-26

## Scope

This focused evidence covers exact filesystem-name identity, one
per-verification directory snapshot per visited parent, and the repeated
placeholder performance regression owned by issue #50. The comprehensive
performance and RSS baseline remains assigned to #59.

## Environment

- Hardware: Apple M4 Max, 16 cores (12 performance and 4 efficiency), 64 GB RAM
- OS: macOS 27.0 (build 26A5378n), arm64
- Filesystem: APFS on an internal SSD
- Rust: `rustc 1.90.0 (1159e78c4 2025-09-14)`
- Pre-fix production commit: `bb8949b393dafadfad42b1209e32604c6e678a79`
- Tests-first harness commit: `f7d8cf92fd6fc8c842bc469cb32430b58b4bdbc8`
- Post-fix implementation commit: `81d4b46ef47880ac68da0b28fd0edcbf19f80112`
- Detected capabilities: host case aliases = true; host Unicode-normalization
  aliases = true
- Workloads: 500, 1,000, and 2,000 listed regular files plus two capability
  probe files in each timed root; the alternating workload adds one
  meaningful-comment placeholder after every listed workload file
- Sampling: 3 warmups and 10 measured release-mode runs; fixture creation is
  outside the timed region

The identical command was used before and after:

```sh
cargo +1.90.0 test --release --locked \
  --test filesystem_identity_snapshot \
  issue_50_release_placeholder_scaling_benchmark -- \
  --exact --ignored --nocapture --test-threads=1
```

## Results

| State | Workload | Listed files | Median (ms) | p95 (ms) | Alternating/plain |
|---|---|---:|---:|---:|---:|
| Before | Plain | 500 | 5.045 | 5.150 | — |
| Before | Alternating placeholders | 500 | 101.175 | 106.988 | 20.053× |
| Before | Plain | 1,000 | 10.411 | 11.092 | — |
| Before | Alternating placeholders | 1,000 | 402.067 | 442.957 | 38.619× |
| Before | Plain | 2,000 | 21.617 | 23.817 | — |
| Before | Alternating placeholders | 2,000 | 1,553.991 | 1,591.496 | 71.887× |
| After | Plain | 500 | 5.593 | 5.835 | — |
| After | Alternating placeholders | 500 | 5.516 | 5.904 | 0.986× |
| After | Plain | 1,000 | 11.971 | 12.657 | — |
| After | Alternating placeholders | 1,000 | 11.212 | 12.067 | 0.937× |
| After | Plain | 2,000 | 22.815 | 23.216 | — |
| After | Alternating placeholders | 2,000 | 23.306 | 37.915 | 1.022× |

| State | Alternating size change | Median scaling | 2.5× threshold |
|---|---|---:|---|
| Before | 500 → 1,000 | 3.974× | Fail |
| Before | 1,000 → 2,000 | 3.865× | Fail |
| After | 500 → 1,000 | 2.032× | Pass |
| After | 1,000 → 2,000 | 2.079× | Pass |

The deterministic regression also recorded two parent enumerations for two
nonadjacent placeholders before the fix. After the fix, focused tests prove
exactly one enumeration for the root and exactly one for each visited nested
parent, including flat siblings that share intermediate components.

## Snapshot Cost Boundary

The approved #50 snapshot includes the type of every enumerated immediate
child, including unlisted children. A sparse guide therefore performs one
non-following metadata classification per child instead of classifying only
listed paths. This is an intentional linear-time cost: it keeps names and types
in one per-verification view, records unsupported kinds for deterministic
rejection when a listed path names them, fails closed when an observation
cannot be classified, and avoids widening the interval between enumeration
and type capture. The broader sparse-guide, network-filesystem, RSS, and
constant-factor comparison remains part of the comprehensive performance
baseline assigned to #59.

## Conformance Result

On the capable APFS fixture, the pre-fix owner gate observed both the case and
Unicode aliases verifying successfully, and it observed `src/main.rs` leaving
`src` incorrectly unmentioned. After the implementation, all three #50-owned
operations pass their normative outcomes while the pending markers are still
present. The handoff then activates only those three operation rows.
