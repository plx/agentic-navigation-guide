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
- Post-fix implementation commit: `110b96997658ce97cbfa2e27b9f382baf447a909`
- Detected capabilities: host case aliases = true; host Unicode-normalization
  aliases = true
- Workloads: 500, 1,000, and 2,000 regular files; the alternating workload
  adds one meaningful-comment placeholder after every listed file
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

| State | Workload | Entries | Median (ms) | p95 (ms) | Alternating/plain |
|---|---|---:|---:|---:|---:|
| Before | Plain | 500 | 5.045 | 5.150 | — |
| Before | Alternating placeholders | 500 | 101.175 | 106.988 | 20.053× |
| Before | Plain | 1,000 | 10.411 | 11.092 | — |
| Before | Alternating placeholders | 1,000 | 402.067 | 442.957 | 38.619× |
| Before | Plain | 2,000 | 21.617 | 23.817 | — |
| Before | Alternating placeholders | 2,000 | 1,553.991 | 1,591.496 | 71.887× |
| After | Plain | 500 | 5.567 | 6.027 | — |
| After | Alternating placeholders | 500 | 5.580 | 6.063 | 1.002× |
| After | Plain | 1,000 | 13.760 | 15.770 | — |
| After | Alternating placeholders | 1,000 | 13.799 | 35.632 | 1.003× |
| After | Plain | 2,000 | 23.576 | 27.328 | — |
| After | Alternating placeholders | 2,000 | 22.763 | 24.946 | 0.966× |

| State | Alternating size change | Median scaling | 2.5× threshold |
|---|---|---:|---|
| Before | 500 → 1,000 | 3.974× | Fail |
| Before | 1,000 → 2,000 | 3.865× | Fail |
| After | 500 → 1,000 | 2.473× | Pass |
| After | 1,000 → 2,000 | 1.650× | Pass |

The deterministic regression also recorded two parent enumerations for two
nonadjacent placeholders before the fix. After the fix, focused tests prove
exactly one enumeration for the root and exactly one for each visited nested
parent, including flat siblings that share intermediate components.

## Conformance Result

On the capable APFS fixture, the pre-fix owner gate observed both the case and
Unicode aliases verifying successfully, and it observed `src/main.rs` leaving
`src` incorrectly unmentioned. After the implementation, all three #50-owned
operations pass their normative outcomes while the pending markers are still
present. The handoff then activates only those three operation rows.
