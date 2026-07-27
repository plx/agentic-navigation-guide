# Issue #65 repository-protection evidence

Date: 2026-07-27

Repository: `plx/agentic-navigation-guide`

## Decision boundary

The maintainer is not creating or transferring the project to a GitHub
organization. This implementation therefore consumes the issue #71
single-maintainer exception through 2026-10-31:

- no organization, team, backup administrator, or second reviewer is created
  or implied;
- the `Main` ruleset requires zero independent approvals because the sole
  pull-request author cannot approve their own work;
- the `release` environment names the sole owner as reviewer and permits
  self-review; and
- both limitations remain residual risk and expire with the issue #71
  exception.

Technical gates are not waived. The default branch has no bypass actor,
release-tag immutability has no bypass actor, and the release environment
disallows administrator bypass. No cosmetic single-owner `CODEOWNERS` rule was
added.

## Baseline

The admin-visible API inventory before the change found:

- ruleset `9303605` (`Main`) targeted the default branch but contained only
  deletion and non-fast-forward rules;
- its repository-administrator actor had `always` bypass;
- no tag ruleset targeted the `v*` release convention;
- the only environment was `github-pages`; no `release` environment existed;
- the repository secret-name inventory contained no publication-like name;
  and
- no release-environment secret or Trusted Publisher existed.

No secret value was read or printed.

## Applied hosted configuration

The reviewed payloads in `.github/repository-protections/` produced:

| Control | Live identifier | Result |
| --- | --- | --- |
| Default branch | ruleset `9303605` | PR, current `Required CI`, current `Verify Navigation Guide`, resolved conversations, deletion and non-fast-forward protection; no bypass actor |
| Release-tag creation | ruleset `19838514` | Only GitHub user ID `65440` (`plx`) may create `refs/tags/v*` |
| Release-tag immutability | ruleset `19838515` | No actor may delete or update `refs/tags/v*` |
| Release environment | environment `18830062205` | Reviewer `65440`, self-review allowed, administrator bypass disabled |
| Release ref scope | deployment policy `55759614` | Tag refs matching `v*` only |

The admin-visible audit ran at
`2026-07-27T16:57:25.028083+00:00` and passed without warnings or failures.
The checked-in JSON attestation records only configuration, numeric IDs,
counts, and secret-name classifications. It records zero publication-like
repository secret names and zero release-environment secrets.

## Runtime rejection probes

PR #134 initially pointed to
`c30628530a07aa70bb898c9d8c9269c6cb06a3fb`.

Immediately after opening:

- GitHub reported `mergeStateStatus: BLOCKED`;
- the required `Required CI` context did not yet exist on the head;
- `Verify Navigation Guide` was still queued; and
- the rules API reported both contexts as strict GitHub-Actions-sourced
  requirements.

This demonstrates that an ordinary pull request cannot merge while a required
check is absent.

The same temporary head contained a pull-request job that referenced the
`release` environment. Run
[`30287144728`](https://github.com/plx/agentic-navigation-guide/actions/runs/30287144728)
failed at the environment boundary in two seconds. GitHub reported zero job
steps: the job never reached a runner, and therefore could not access code,
credentials, or an environment secret. The probe workflow is removed from the
next commit; only this non-sensitive run record remains.

The configured positive authorization is mechanical at this stage: only a
tag ref matching `v*` can enter the environment, and it still requires the
named approval. Issue #63 owns the non-publishing tag-shaped release rehearsal
and the exact crates.io Trusted Publisher identity. This issue creates no tag
to avoid leaving a protected probe tag behind.

## Reproduction

Public, read-only drift check:

```sh
python3 scripts/audit_github_protections.py
```

Admin-visible release audit:

```sh
GH_TOKEN=... python3 scripts/audit_github_protections.py \
  --require-admin-visibility \
  --output target/repository-protection-attestation.json
```

The first command needs no privileged secret. GitHub omits bypass actors from
unprivileged ruleset responses, so it emits an explicit warning for that
field. The second command is required before release; it checks exact bypass
actors and non-sensitive secret names without reading or printing values.

The scheduled `Repository protection audit` workflow repeats the public check
weekly with `contents: read`. The offline regression suite proves that missing
checks, unexpected bypasses, wrong ref types, environment admin bypass, and
publication-like secret names fail closed. No fuzzing, randomized input,
mutation campaign, or release publication was added or performed by #65.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| `main` cannot bypass agreed checks through an ordinary push/merge | Active no-bypass ruleset requires PR, strict `Required CI`, and strict `Verify Navigation Guide`; PR #134 was blocked while the aggregate was absent |
| Release tags and publication environment have explicit protection | Separate owner-only creation and no-bypass immutability rulesets; tag-only environment policy, approval, and disabled administrator bypass |
| Publication identity is narrowly scoped | No publication credential exists. Issue #63 is required to create only a Trusted Publisher scoped to this repository, its reviewed workflow, and `release`; until then publication fails closed |
| Small-maintainer recovery path is explicit | `docs/repository-protections.md` requires a public control-change record, synchronized checked-in payload, ruleset history, restoration, and 48-hour retrospective; inability to restore stops changes/releases |
| Reviewer can reproduce without privileged secrets | Public audit script and weekly read-only workflow validate every GitHub-public field and explicitly identify the one visibility limit |
| No release is published | No tag, GitHub Release, crate, deployment, or publication credential was created |

Closure of #65 means the strongest operable personal-repository controls are
configured and auditable. It does not claim organization-backed approval,
human redundancy, a completed Trusted Publisher, or issue #63's release
rehearsal.
