# Repository and release protections

## Scope and personal-repository constraint

This document records the GitHub-hosted controls established by issue #65.
The repository belongs to the personal account `plx`; no GitHub organization,
team, backup administrator, or independent reviewer is represented as
existing. The separately approved
[single-maintainer exception](maintainer-continuity.md) expires on
2026-10-31.

The personal-repository constraint changes the human gate, not the technical
gates. `main` still requires a pull request, current required checks, resolved
review threads, and immutable history. Release tags are creation-restricted
and immutable. The `release` environment accepts only version-shaped tags and
requires an explicit approval. No routine or emergency release may bypass
those technical controls.

GitHub does not allow a pull-request author to approve their own change. With
no second write-capable maintainer, the `Main` ruleset therefore records zero
required approving reviews. It dismisses stale approvals if any are supplied,
and it requires all review conversations to be resolved. This is the strongest
operable pull-request rule that does not make every sole-maintainer change
depend on weakening the ruleset. The lack of an independent approval remains
explicit residual risk through the issue #71 exception.

No `CODEOWNERS` file is added. A rule naming only `plx` would be cosmetic and
would provide no independent control.

## Reviewed configuration

The exact intended API payloads are checked in under
`.github/repository-protections/`:

- `main-ruleset.json` protects the default branch with no bypass actor;
- `release-tag-creation-ruleset.json` makes `plx` the sole actor able to
  create a tag matching `v*`;
- `release-tag-immutability-ruleset.json` prevents every actor, including the
  owner, from deleting or moving a matching release tag;
- `release-environment.json` records the sole-maintainer approval gate and
  forbids administrator deployment bypass; and
- `release-environment-tag-policy.json` permits only tags matching `v*`.

The creation and immutability rules are intentionally separate. The owner
must be able to create a reviewed release tag, but creation authority must not
also grant authority to move or delete an existing release tag.

### `main`

The active `Main` ruleset requires:

- all changes to arrive through a pull request;
- stale approvals to be dismissed after a reviewable push;
- all review conversations to be resolved;
- `Required CI` from the GitHub Actions app;
- `Verify Navigation Guide` from the GitHub Actions app;
- the pull-request head to be tested against current `main`;
- branch deletion to be prohibited; and
- non-fast-forward updates to be prohibited.

`Required CI` is a stable aggregate over every blocking job in `.github/
workflows/ci.yml`. Rust beta remains informational and is not part of the
aggregate. The separate `Verify Navigation Guide` workflow is required
directly so a renamed, missing, cancelled, or failing guide check blocks the
merge.

The ruleset has no bypass actors. The repository owner can administratively
edit the ruleset, but cannot silently push or merge around it. An emergency
ruleset change is a control change, not a merge shortcut: it requires a
public issue, a pull request updating the checked-in payload and this
document, the GitHub ruleset-history record, and a retrospective review within
48 hours. If the controls cannot be restored safely, changes and releases
stop.

### Release tags

Both release-tag rulesets target `refs/tags/v*`. Only GitHub user ID `65440`
(`plx`) may satisfy the creation restriction. Once created, a matching tag
cannot be deleted or updated by any bypass actor because the immutability
ruleset has no bypass list.

Tags use the `v{version}` convention defined by
[`release/identity.toml`](../release/identity.toml). Issue #63 must verify the
exact version/tag/commit identity before publication. Issue #65 creates no
tag and publishes no release.

### `release` environment

The environment:

- allows only tag refs matching `v*`;
- requires approval from `plx`;
- allows self-review only because there is no second maintainer under the
  dated exception;
- disallows administrator bypass of deployment protection rules; and
- contains no environment secret or long-lived publication token.

Self-review is an explicit single-maintainer exception, not independent
approval. The approval still creates a deliberate, auditable pause between a
tag-triggered workflow and access to the environment.

Issue #63's reviewed release workflow scopes the intended OIDC identity to
exactly
`plx/agentic-navigation-guide`, that reviewed workflow filename, and the
`release` environment. Until the matching crates.io Trusted Publisher is
registered, the absence of a short-lived publication credential fails closed.
No long-lived token is installed.

## Inspection and recurring audit

Anyone can reproduce the public portion of the check:

```sh
python3 scripts/audit_github_protections.py
```

The script compares live rules, required checks, environment reviewers, and
tag deployment policies with the checked-in payloads. GitHub hides ruleset
bypass actors from callers without write access; public mode reports that
limitation as a warning instead of claiming the hidden field was checked.

Before a release, the repository owner must run the complete non-sensitive
audit with an administration-capable token:

```sh
GH_TOKEN=... python3 scripts/audit_github_protections.py \
  --require-admin-visibility \
  --output target/repository-protection-attestation.json
```

The token is read only from the process environment and is never printed.
Admin-visible mode additionally verifies exact bypass lists and inventories
secret names without reading values. It rejects any secret in the `release`
environment before issue #63 and any repository-level secret name that looks
like a publication credential.

`.github/workflows/repository-protection-audit.yml` runs the public check every
Monday and on demand with only `actions: read` and `contents: read`. Its token
cannot see bypass actors or secret names, so it cannot replace the
admin-visible release audit. It does make drift in every publicly inspectable
control visible without privileged secrets.

Repository administrators may change the hosted settings. During the current
exception that means only `plx`. Reviewers can inspect the live rulesets at
`https://github.com/plx/agentic-navigation-guide/settings/rules`, the release
environment at
`https://github.com/plx/agentic-navigation-guide/settings/environments`, and
the ruleset history through GitHub's ruleset API.
