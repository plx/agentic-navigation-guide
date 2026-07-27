<!-- issue-71-single-maintainer-exception -->

# Maintainer continuity and release authority

## Current status

This repository operates under a time-bounded single-maintainer exception.
The maintainer approved that decision on 2026-07-27 in
[issue #71](https://github.com/plx/agentic-navigation-guide/issues/71#issuecomment-5090158814)
after declining to create or transfer the project to a GitHub organization.

`plx` is the sole personal-repository owner, sole repository administrator,
sole named crates.io owner, and sole release authority. There is no backup
maintainer or second administrator. The project must not describe issue #71
as proof of organizational redundancy.

The public machine-readable record is
[`release/maintainer-continuity.toml`](../release/maintainer-continuity.toml).
It contains status and dates only. It is not a credential store.

## Exception and expiry

The exception expires on **2026-10-31**. During the exception, remediation and
an eventual v0.2 release may proceed only through every other applicable
production-readiness, independent-audit, and publication gate.

Publication after 2026-10-31 is blocked unless one of these occurs first:

1. a real backup owner and independent recovery path are verified and tested;
   or
2. a new explicit maintainer decision renews the exception with a new date and
   carries the residual risk into the final readiness record.

Repository CI fails closed after the expiry until the machine-readable record
contains a newly approved deadline. This deliberately makes an expired
exception visible before any publication workflow can remain green.

Closing issue #71 records an accepted exception. It does not make the missing
controls pass, and it does not waive issues #63, #65, #69, #72, or #73.

## Verified and unverified controls

The 2026-07-27 inventory established the following non-secret facts:

| Control | Current status |
| --- | --- |
| GitHub repository ownership | Personal repository; `plx` is the only owner and administrator |
| crates.io ownership | `plx` is the only named owner |
| Backup authority | None |
| GitHub organization | None |
| Protected release environment | `release` is tag-scoped and owner-approved; issue #65 |
| crates.io Trusted Publishing | Not configured; issue #63 owns it |
| Public security-report route | Not present; issue #69 owns it |
| Homebrew tap | None |
| Independent recovery runbook | Not established |
| Independent recovery drill | Not run |

GitHub 2FA status is not verified by this repository. The available API did
not return an attestable value, and no private account view or recovery
material was copied into project evidence. Every current or future privileged
human account is required to use a secure 2FA method, but a later access review
must record the actual non-sensitive verification status before relying on
that control.

Issue #63 owns the Trusted Publishing workflow and its exact repository,
workflow, environment, and OIDC identity. Issue #65 owns the protected release
environment and established the strongest operable personal-repository rules,
required checks, and release-tag controls. Its zero-review `main` rule and
self-review environment gate are consequences of this exception, not
independent human approval.

## Release authority

During this exception:

- `plx` is the only person authorized to approve, tag, publish, yank, change
  crate ownership, or change protected release settings.
- A routine release must use the reviewed issue-#63 workflow from an immutable
  candidate and pass every required check, provenance, package, and
  independent-audit gate. The absence of a second human approver is recorded
  residual risk, not an approval.
- An emergency security release uses the same fail-closed workflow and
  immutable-source requirements. Technical checks, provenance, and Trusted
  Publishing may not be bypassed. A retrospective access and release review
  is required within 48 hours.
- If `plx` is unavailable or the account is compromised, no release can occur.
  Publication stops until GitHub and crates.io access is recovered or a new
  verified owner is established. Credentials must never be shared to work
  around that stop.
- Ownership changes require a public decision record without private contact
  or identity material. A compromised owner or credential is escalated to the
  applicable platform support channel, and publication remains halted while
  authority is uncertain.

## Authentication and publication identity

Crates.io publication is required to use short-lived Trusted Publishing
identity once issue #63 establishes it. Long-lived publication tokens are not
the intended release path. The maintainer must inventory and revoke obsolete
tokens in the private crates.io account view without printing names or values
into logs, git, issues, or pull requests.

If a temporary token is unavoidable before migration, it must be
minimum-scope, stored only in the protected release environment, and removed
after use. This repository does not attest that the private token inventory
has already been completed.

## Recovery and security reporting

No independent recovery drill has been performed. With no backup maintainer,
the issue-requested scenario in which a backup acts without the primary cannot
be executed. No tabletop result is represented as passing.

A future two-owner topology must establish an out-of-band recovery record
accessible independently to both owners. Git may record only its non-secret
label, responsible owners, covered systems, and last verification date. The
private record must cover GitHub, crates.io, the release environment,
provenance or signing systems, the security-report route, and any future
Homebrew tap without containing secrets in this repository.

Issue #69 owns the public security-report route. Until that route lands,
ordinary public GitHub issues are not a private vulnerability-report channel.
No Homebrew tap exists. If one is introduced, its ownership, credentials,
offboarding, and recovery controls must be added here before publication
through that channel.

## Access review and offboarding

Access is reviewed at every minor release and at least every six months. The
review records only usernames or team names, roles, non-sensitive 2FA status,
Trusted Publisher identity, environment/ruleset status, token-inventory
completion, recovery-record verification date, and drill date.

A future backup or other privileged maintainer must be removed from GitHub,
crates.io, release environments, security routes, provenance systems, and
distribution channels within 24 hours of offboarding. Remaining scoped
credentials are rotated or revoked, and the authority record is updated in a
reviewed pull request. The sole owner cannot be offboarded without first
establishing a successor; if access is lost, releases stop.

## Public maintenance expectation

This project offers best-effort maintenance. There is no response-time,
availability, or organizational-redundancy guarantee. Users must not infer
that another maintainer can publish a fix when `plx` is unavailable.

The exception and its expiry are release facts. Every final readiness audit
and release approval performed while it remains active must list the
single-person dependency as residual risk.
