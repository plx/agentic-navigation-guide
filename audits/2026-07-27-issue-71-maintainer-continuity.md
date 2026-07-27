# Issue #71 maintainer-continuity exception

## Decision

The maintainer declined to create or transfer this personal repository to a
GitHub organization and directed the production-readiness process to record
the limitation, close issue #71, and continue with later work adjusted for the
lack of organizational redundancy.

The durable decision is
[issue comment 5090158814](https://github.com/plx/agentic-navigation-guide/issues/71#issuecomment-5090158814).
It approves a single-maintainer exception through 2026-10-31. After that date,
publication is blocked without a verified backup topology or a new explicit
maintainer decision.

This is an exception record, not a successful redundancy attestation.

## Non-sensitive before state

The 2026-07-27 read-only inventory found:

- `cargo owner --list agentic-navigation-guide` reported only `plx`;
- GitHub identified `plx/agentic-navigation-guide` as a personal repository
  with `plx` as its only owner and direct administrator;
- the active `Main` ruleset prohibited deletion and non-fast-forward updates
  but allowed an administrator bypass;
- `github-pages` was the only deployment environment;
- the repository secret-name inventory contained no crates.io publication
  token name; no value was queried;
- no deploy key, webhook, release environment, Homebrew tap, public
  security-report route, or Trusted Publisher configuration was found in the
  available repository evidence; and
- the authenticated GitHub API returned no attestable 2FA value.

The inventory cannot prove that private tokens are absent or revoked, and it
cannot prove a private account security setting that the API did not expose.
Those facts remain unverified rather than inferred.

## Acceptance mapping

| ID | Disposition | Issue #71 acceptance criterion | Evidence and residual risk |
| --- | --- | --- | --- |
| A71-001 | Exception — no tested independent recovery path | Crate, repository, and release access have a tested path that does not share one person's credentials. | No backup exists, no independent runbook is established, and the requested unavailable-primary drill cannot run. The maintainer explicitly accepted this residual risk through 2026-10-31. |
| A71-002 | Policy set; external verification deferred | Privileged accounts use 2FA and publication uses short-lived scoped identity where practical. | The policy requires secure 2FA and issue-#63 Trusted Publishing, but 2FA is not attested and Trusted Publishing is not configured. Issues #63 and #65 retain the external controls. |
| A71-003 | Implemented | Release authority, emergency handling, access review, and offboarding are explicit. | `docs/maintainer-continuity.md` names the sole authority, fail-closed normal and emergency paths, 48-hour retrospective, minor-release/six-month review, and 24-hour offboarding target. |
| A71-004 | Verified | No secret or recovery material is committed or copied into a public ticket. | Only public usernames, roles, statuses, dates, secret names already visible in workflows, and absence findings were inspected. The deterministic test rejects common token prefixes in the new artifacts. |
| A71-005 | Exception accepted | A single-maintainer exception is explicit, justified, time-bounded, and residual risk in the final audit. | The policy, machine-readable record, this audit, README, release policy, and changelog all state the 2026-10-31 expiry and forbid treating closure as redundancy proof. |

No organization, collaborator, crate owner, token, environment, ruleset, or other protected setting was changed.
No tabletop recovery drill was represented as passing.

## Adjustments inherited by later work

- #63 must build Trusted Publishing and cannot assume an independent human
  release approver exists.
- #65 must document the personal-repository administrator bypass and choose
  the strongest release-environment protections that remain operable for one
  maintainer.
- #69 must establish the public security-report route before a report-receipt
  drill can occur.
- #72 and the final production-readiness evidence must carry the untested
  single-person recovery dependency as residual risk.
- #73 may publish only while the exception is active and only after every
  other gate passes. Publication after expiry requires a new decision or a
  verified backup.

## Deterministic validation

`tests/issue_71_maintainer_continuity.rs` checks:

1. the exact public record, including owner count by identity, missing-control
   statuses, and expiry;
2. candid policy language that requires future controls without claiming they
   already exist; and
3. README, release-policy, changelog, navigation-guide, and five-row
   acceptance-ledger alignment.

The validation uses fixed strings and repository files only. It performs no
fuzzing, mutation testing, randomized generation, or generated hostile input.

## Red-before-fix evidence

On exact issue base `16a0b3a8597f2b085b6823a1b9b8bc63d3f28dfa`,
`cargo test --locked --test issue_71_maintainer_continuity -- --nocapture`
exited 101. All three tests failed for their intended reason: the continuity
record, public policy, and acceptance audit did not exist.

## Post-change acceptance

The completed local acceptance was:

- `cargo test --locked --test issue_71_maintainer_continuity` — 3 passed;
- `cargo +1.85.0 test --locked --test issue_71_maintainer_continuity` — 3
  passed on the MSRV;
- `cargo fmt -- --check`, all-target/all-feature `cargo check`, and Clippy
  with warnings denied — passed;
- the full all-target/all-feature suite with one serial test thread — 351
  passed and 3 intentional manual/packaged-artifact tests ignored;
- navigation-guide syntax and filesystem verification with ignored guides
  denied — passed;
- 61 production-readiness selector regressions — passed;
- 14 release-identity regressions plus the `v0.2.0` identity check — passed;
- the issue-#62 exact package-manifest regression — passed; and
- `cargo package --list --locked --offline --allow-dirty` returned exactly 33
  paths, while package construction and verification completed at 662.6 KiB
  source and 152.2 KiB compressed.

No publication, tag, release, organization, ownership, credential, or
protected-setting action was performed.
