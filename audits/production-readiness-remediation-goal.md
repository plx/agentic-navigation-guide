# Production-Readiness Remediation Goal

## Copy-paste goal

After PR #75 has merged, start a new Codex session from a clean, current
`main` checkout and submit:

```text
/goal Complete the work in `audits/production-readiness-remediation-goal.md`.
```

This is an execution goal, not a request to edit or summarize this document.
The goal remains active until the terminal completion criteria below are met.
Do not mark it complete merely because all remaining issues have open pull
requests.

## Required outcome

Complete the production-readiness program rooted at
[#26](https://github.com/plx/agentic-navigation-guide/issues/26), including
every current or subsequently discovered issue in its workflow universe.
Execute the program end to end as an ordered sequence of small, reviewable pull
requests:

- work on exactly one selected issue at a time;
- use intentional PR stacks when selected work depends on an open prerequisite
  PR;
- keep independent work in separate, shallow stacks based on current `main`;
- merge every stack in dependency order;
- close every workflow issue through its own merged PR; and
- finish the independent audit, formal release, post-release distribution, and
  top-level program gate only when their prerequisites and evidence permit it.

The static documentation site and landing page remain outside the audit scope
except where a ticket explicitly concerns package boundaries, release
contents, or links to live distribution. Do not broaden an issue merely
because adjacent cleanup is convenient.

## Scope and authority

Invoking this goal authorizes the normal repository work needed to complete
the program:

- inspect and modify files in this repository;
- run local and GitHub-hosted validation;
- create ticket branches, commits, and draft PRs;
- update a PR in response to review or CI;
- create self-contained workflow issues for genuinely new findings and
  maintain their required labels, parents, blockers, and sub-issue
  relationships under the work-selection maintenance contract;
- merge a remediation PR after every prerequisite, required review, and
  required check is satisfied; and
- delete the merged ticket branch when it is no longer needed by a descendant
  stack.

That authority does **not** permit:

- bypassing branch protection, required reviews, or failing checks;
- force-merging, using administrator overrides, or weakening a gate to make
  progress;
- directly changing any workflow issue's state to closed;
- exposing credentials or committing sensitive audit evidence;
- publishing an irreversible crate, tag, GitHub Release, or external tap
  without the explicit release checkpoint below; or
- writing to a repository or service outside the confirmed release/tap scope.

Do not stop simply because the program spans many turns or compactions.
Persist through ordinary implementation, CI, review, merge, and stack
maintenance. Stop and request user direction only at a defined approval
checkpoint or a genuine unresolved blocker.

## Preconditions

Before selecting the first remediation issue:

1. Confirm PR
   [#75](https://github.com/plx/agentic-navigation-guide/pull/75) has merged.
   It installs the issue taxonomy, selector, audit records, and this runbook.
   Do not build remediation branches on the unmerged audit branch.
2. Start from a clean checkout of the current remote `main`. Preserve
   unrelated user changes and use a separate worktree if necessary.
3. Confirm `gh auth status`, repository identity, the default branch, and
   write access.
4. Run the selector's offline tests:

   ```sh
   just test-production-readiness-selector
   ```

5. Run the live selector once:

   ```sh
   just get-next-production-readiness-issue --json
   ```

6. Confirm the canonical and workflow cohorts are valid. Do not repair labels
   or native relationships merely to obtain a preferred first issue.

If PR #75 is not merged, GitHub authentication is unavailable, or the
selector fails closed, report that condition and wait. Do not substitute a
hand-written issue order.

## Required guidance

At the beginning of the goal, read:

1. [`CLAUDE.md`](../CLAUDE.md);
2. [`AGENTIC_NAVIGATION_GUIDE.md`](../AGENTIC_NAVIGATION_GUIDE.md);
3. the baseline
   [production-readiness audit](2026-07-25-production-readiness-audit.md);
4. the
   [work-selection operator contract](production-readiness-work-selection.md);
5. the
   [post-remediation reassessment playbook](production-readiness-reassessment-playbook.md);
6. top-level epic
   [#26](https://github.com/plx/agentic-navigation-guide/issues/26); and
7. the complete body, comments, native blockers, native parents/children, and
   linked prior art for the issue selected in the current loop.

Also read any `AGENTS.md`, contributor, security, release, support, or
normative-contract guidance added by earlier remediation PRs. Shared guidance
can evolve during this goal.

Re-read the selected ticket and relevant shared guidance after compaction,
handoff, a material review change, or a changed GitHub dependency graph. Do
not rely on remembered acceptance criteria.

Apply instructions in this order:

1. current user, system, and repository safety instructions;
2. the selected issue's explicit required behavior and acceptance criteria;
3. `CLAUDE.md` and repository conventions;
4. the work-selection contract and this runbook;
5. the audit report and reassessment playbook; and
6. historical specifications or prior-art PRs.

When two sources appear to conflict, inspect the implementation, tests, issue
history, and linked decisions. Resolve the conflict explicitly in the PR or
ask the user when it would materially change the public contract. Do not pick
the interpretation that merely makes the ticket easiest to close.

The 2026-07-25 audit is immutable historical evidence about its named
revision, not a substitute for inspecting current `main`.

## Non-negotiable workflow rules

1. **The live selector chooses work.** Do not choose a more attractive issue
   manually and do not use `--exclude` to evade priority or dependencies.
   A temporary exclusion is acceptable only for a documented, issue-specific
   constraint while another genuinely ready issue can progress.
2. **One issue is implemented at a time.** Previously opened PRs may be
   awaiting review or merge, but do not implement multiple tickets
   concurrently. Subagents may perform bounded research or independent review;
   they must not each implement a different workflow issue.
3. **One PR closes exactly one workflow issue.** Split combined fixes unless
   the selected ticket itself requires inseparable work.
4. **Every workflow PR targets `main`.** A branch may be based on an open
   prerequisite branch, but do not retarget its PR to that branch. The selector
   recognizes closing coverage only on the repository's default branch.
5. **GitHub closes issues through merged PRs.** Never use `gh issue close`, an
   issue-state REST or GraphQL mutation that sets `closed`, the UI close
   action, or equivalent automation for #26 or any workflow child.
6. **Open PR coverage is not landed work.** It may sequence dependent leaves,
   but it never permits an out-of-order merge and never satisfies a gate.
7. **Tests prove the defect when applicable.** Add a regression that fails for
   the intended reason before the fix and passes afterward. Preserve the
   red-before-fix command and result in the PR.
8. **Do not weaken evidence.** Never delete or relax a test, dependency,
   acceptance criterion, label, branch rule, or audit gate merely to obtain a
   green check or a different selector result.
9. **Keep user-facing contracts aligned.** Update the README and other
   authoritative documentation in the same PR whenever behavior changes.
10. **No silent scope absorption.** New actionable defects receive their own
    self-contained issues and dependency metadata unless they are necessary to
    satisfy the selected ticket's stated acceptance criteria.

For this goal, the no-direct-close rule is stricter than the work-selection
guide's general allowance for evidence-backed manual gate closure. This
runbook controls: every leaf and every gate closes through a dedicated merged
PR. Reopening invalidated evidence is permitted only where this runbook
requires it; reopening is not completion, and the issue must later close again
through a new dedicated PR.

## Pull-request stack contract

The program is a sequence of small, default-branch-targeted ancestry stacks,
not one giant PR and not a 49-branch chain. Git ancestry may stack on a
predecessor head; the GitHub PR base may not.

### Starting a branch

- If the selected issue has no open prerequisite PR whose changes it needs,
  create its branch from current remote `main`. This starts a new stack.
- If the selector made a leaf ready through covered prerequisite leaves and
  the new implementation needs those unmerged changes, create the branch from
  the exact head of the nearest prerequisite PR. Record the full ancestry.
- If the selected issue can be implemented and tested against current `main`,
  prefer a new stack from `main` even when another independent PR is open.
- Use an issue-specific branch name such as `agent/issue-34-guide-grammar`.
  Never reuse a branch from a merged or abandoned ticket.

Every PR in either case must use `main` as its GitHub base. A dependent PR may
temporarily show its ancestors' commits and diff; its Stack section must make
that explicit.

Keep at most one unmerged descendant above a predecessor PR. Before preparing
a third level, merge and restack from the bottom so every open diff remains
reviewable. An ancestry rewrite requires revalidation of every affected
descendant and an updated branch-point record.

### Stack metadata

Every stacked PR body must identify:

- the immediate predecessor PR, or `none`;
- all earlier PRs whose commits are present;
- the required merge order;
- whether its tests require the predecessor's code; and
- the exact commit or branch from which it was created.

Use ordinary references such as `Refs #N` for related workflow issues. Only
the selected issue receives a closing keyword.

### Merge order

- Merge from the bottom of a stack upward.
- Never merge a dependent PR while a semantic prerequisite issue is open.
- Require the predecessor PR to be merged, not merely approved or green.
- After each predecessor merge, update the next PR on current `main`, remove
  already-landed ancestor commits from its diff, resolve conflicts, and rerun
  all affected tests.
- If history rewriting is necessary, use `--force-with-lease` only on the
  goal's own verified ticket branch and only after confirming no other work
  depends on an unpublished head. Never use an unguarded force push.
- Re-verify the child PR's base, diff, closing reference, checks, and review
  state after any rebase or base update.
- Merge an independent stack whenever it is approved and green; do not keep a
  deep global stack merely for the appearance of continuous sequencing.

Gates are never stacked on merely covered requirements. A gate branch begins
only after every blocker and native child required by the selector is actually
closed.

## The one-issue loop

Repeat this loop until the terminal criteria are satisfied.

### 1. Reconcile live state

Sync remote state and run:

```sh
just get-next-production-readiness-issue --json
```

Interpret the result carefully:

- `selected`: work only on the returned issue.
- `waiting`: inspect open PRs, reviews, CI, and merge order. Finish or merge
  the blocking stack; do not relabel work to manufacture readiness.
- `complete` with a nonzero `open_count`: the queue is fully claimed, not
  finished. Complete reviews and merge the remaining PRs in order.
- `complete` with `open_count: 0`: proceed to the terminal cross-checks.
- error/nonzero exit: diagnose the taxonomy, graph, pagination, or GitHub
  state. Do not guess.

If the selector returns an issue that already has implementation in progress,
verify whether GitHub has indexed the intended closing PR. Repair the PR
metadata or wait for indexing instead of opening a duplicate.

### 2. Establish the ticket contract

Read the issue and all linked guidance. Write a private working checklist that
maps:

- each acceptance criterion to a code, test, documentation, or evidence
  change;
- each required validation command to a planned run;
- each dependency to a closed issue or named stack predecessor;
- each non-goal to a scope boundary; and
- any decision that requires user input.

Inspect current source and tests rather than assuming the audited revision
still describes `main`. Search for overlapping open PRs, especially prior art
explicitly linked from the ticket.

### 3. Capture the before state

Before implementation:

- reproduce the defect or missing control on the appropriate vulnerable
  revision when the ticket requires it;
- add or design the regression that will fail for the intended reason;
- record the exact command, exit status, and concise result;
- distinguish environmental failure from proof of the defect; and
- explain in the PR when red-before-fix testing is genuinely not applicable,
  such as a pure contract-decision or governance ticket.

Do not leave the final PR red. Use a separate worktree or reversible local
step when proving the old behavior would otherwise disrupt the implementation
branch.

### 4. Implement only the selected issue

Make the smallest complete change that satisfies the ticket. Preserve
unrelated user work. Follow repository formatting and architecture, keep
errors fail-closed where required, and update all affected user-facing or
maintainer documentation.

If implementation reveals a separate defect:

- determine whether it is necessary for the current acceptance criteria;
- if not, create a self-contained issue with reproduction, impact, required
  direction, failing-before-fix test expectations, validation, acceptance
  criteria, labels, parent, and native dependencies;
- add it to the workflow universe according to the work-selection contract;
  and
- rerun the selector after the current ticket reaches a stable PR boundary.

Do not hide substantive audit findings inside an unrelated PR.

### 5. Validate before publication

Run every ticket-specific command and the relevant repository-wide checks.
The usual minimum for Rust changes is:

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- check --guide AGENTIC_NAVIGATION_GUIDE.md
cargo run --locked -- verify --guide AGENTIC_NAVIGATION_GUIDE.md --root .
```

Also run platform, feature, package, security, fuzz, property, performance, or
documentation checks required by the ticket. Do not claim an unavailable
environment passed. Record the limitation and use CI or request the required
environment.

Inspect the final diff for unrelated changes, generated artifacts, secrets,
debugging output, and stale documentation.

### 6. Commit and open one draft PR

Commit only the selected ticket's files. Push its issue-specific branch and
open a draft PR targeting `main`.

For an ordinary implementation or final evidence PR, use this body structure:

```markdown
Closes #<selected-issue>

## Scope

<What this ticket changes and why>

## Stack

- Immediate predecessor: <PR URL or none>
- Earlier included PRs: <URLs or none>
- Required merge order: <bottom to top>
- Branch point: <commit>

## Red-before-fix evidence

<Command and concise failing result, or reason not applicable>

## Validation

- `<command>` — <result>

## Acceptance criteria

<Map every issue criterion to evidence in this PR>

## Residual risks

<None, or explicit limitations and follow-up issue links>
```

Except for the staged #72–#74 workflows described below, the initial PR body
must contain exactly one GitHub closing keyword for exactly the selected
workflow issue.

For #72, #73, and #74, preparatory, audit-in-progress, or external PRs use only
non-closing references. Remain on the selected gate workflow until its later
in-repository closing evidence PR is valid; do not treat a preparatory PR as
the ticket's selector claim.

The sole closing keyword appears exactly once and only in the designated PR
body. Never put `Closes`, `Fixes`, `Resolves`, or another closing keyword for
any workflow issue—including the selected one—in a commit message, PR title,
comment, review, stack metadata, or external PR. Use `Refs #N` there.

### 7. Verify GitHub's closing relationship

After opening or editing the PR, wait for GitHub indexing and run:

```sh
gh pr view <pr-number> \
  --json baseRefName,headRefName,closingIssuesReferences,isDraft,state
```

Require all of the following before moving to another issue:

- state is `OPEN`;
- base is `main`;
- the PR is a draft until it is ready for review; and
- `closingIssuesReferences` contains exactly the selected issue and no other
  workflow issue.

Also confirm that the selected issue remains `OPEN`. The PR claims work; it
does not complete it. Rerun the selector and confirm it observes the claim
before beginning another selected leaf.

For a staged #72–#74 preparatory or external PR, require
`closingIssuesReferences` to contain no workflow issue. The exact-one-closing
assertion applies only to the final in-repository evidence PR.

If any assertion fails, correct the PR before continuing. Do not close the
issue manually as a substitute.

### 8. Complete review and CI

Monitor every required check. Read all review comments and inline threads,
implement actionable corrections, rerun affected tests, and keep the PR body
and stack metadata current.

Mark the PR ready only when:

- its final diff is limited to the selected ticket;
- every stack predecessor has merged;
- it has been updated on current `main` so no ancestor-only change remains in
  its diff;
- all acceptance criteria have evidence;
- local and required hosted checks pass;
- the closing reference remains exact; and
- every unresolved review concern is either fixed or answered with a concrete
  rationale.

Do not dismiss a failing check as flaky without reproducing and documenting
the evidence. Do not merge around a review request.

### 9. Merge safely

Normal in-repository merges are within this goal's scope. Merge the PR only
when:

- every semantic and stack predecessor has merged;
- branch protection and required approvals are satisfied;
- all required checks are green on the final head;
- the final PR still targets `main` and closes exactly one issue; and
- no release-specific approval checkpoint applies.

If GitHub requires no human approval, this goal intentionally permits routine
implementation and evidence PRs to merge after all checks pass and a separate
agent/session has performed an independent diff-and-acceptance review with no
unresolved blocker. Contract decisions, protected-setting attestations, legal
judgments, #72's verdict, #73 publication, and #74 ownership remain subject to
their explicit human or independent-review checkpoints.

Use the repository's configured merge method. Never use an administrator
bypass. After merge:

1. poll GitHub for a bounded period to allow closing-reference and timeline
   indexing;
2. verify GitHub automatically changed the selected issue to closed;
3. inspect `closedByPullRequestsReferences` and the issue timeline to confirm
   the merged PR caused closure;
4. if the relationship remains absent after bounded refetching, **do not close
   it yourself**—report the failure and request user direction;
5. update and revalidate the next descendant PR, if one exists;
6. remove the merged branch when no descendant needs it; and
7. return to the live selector.

An open PR, merged commit, checked checkbox, or passing test is not enough if
the issue remains open.

## Decision and administrative checkpoints

The selector determines readiness, but it does not supply maintainer
decisions, legal judgment, credentials, or additional repository owners.

- Issues #34, #35, #36, #68, and #71 carry `needs-decision`. Research and
  propose a concrete decision PR, but require explicit maintainer approval of
  that decision before merging it or implementing dependent behavior.
- #34's grammar, #35's filesystem trust model, and #36's supported library/CLI
  surface are binding public contracts. Do not select the easiest
  implementation by default.
- #68 determines normative-document ownership and history. Preserve historical
  evidence until its reviewed disposition lands.
- #71's backup owner, two-factor authentication, and recovery evidence must
  refer to real people and controls. Never invent an owner or attest to an
  account setting that was not verified.
- #63, #65, and #71 may require credentialed repository or publishing settings.
  Ask the owner to perform or authorize protected settings changes; never
  expose secrets or claim an unavailable setting was checked.
- #64 may require licensing or release-identity judgment. Record the evidence
  and obtain the appropriate maintainer or legal decision rather than making
  an unsupported attestation.

A decision checkpoint is not permission to abandon the goal. Once the
decision or owner action is recorded, resume the same ticket loop and selector.

## Gate and release rules

### Component and program gates

Issues #27–#33 and #26 are evidence gates, not implementation shortcuts. Start
one only when selected by the live tool after all of its requirements close.
Execute its aggregate acceptance criteria and create a real, reviewable
in-repository evidence PR that closes only that gate.

Use the artifact named by the gate ticket. If it names no repository artifact,
add a concise dated record under `audits/` mapping every acceptance criterion
to merged PRs, commands, and retained evidence. Do not open an empty or no-op
PR merely to obtain a closing reference.

If newly discovered work invalidates an already-closed component gate, reopen
that gate, attach the new issue through the correct native relationships, and
block downstream audit or publication work. After the new work closes, rerun
the gate's aggregate criteria and close it again through a new evidence PR.
Never leave a stale closed gate as apparent proof.

### Independent reassessment gate #72

Run #72 from a fresh checkout and a fresh session/context. Use a reviewer
independent of the remediation sequence where practical. The active
implementation context must hand off the immutable candidate and must not
author the verdict.

Follow every applicable section of the reassessment playbook. An audit PR
opened before its verdict is final uses only `Refs #72`. Add `Closes #72` to
the dedicated report PR only after the committed report states an
unconditional `PASS — production-ready` for release and contains all required
evidence.

A `CONDITIONAL` or `FAIL` verdict must not close #72. New substantive defects
receive separate workflow issues and fixes; do not repair them in the audit
PR. Such a report PR must never acquire a closing keyword. After those fixes
merge, rerun affected evidence and the final gate.

Only the exact verdict `PASS — production-ready` for the exact immutable
candidate may produce a merge that closes #72.

Every candidate-affecting preparation must land before the #72 candidate is
frozen. This includes source, tests, manifests, lockfiles, version metadata,
release workflows, package inputs, and release controls. The audit report may
be committed afterward as evidence while continuing to identify the audited
candidate by its exact earlier commit.

After #72 closes, no candidate-affecting change may land before publication.
If one becomes necessary, stop the release, reopen #72 and every affected
component gate, land and validate the new work, freeze a new candidate, and
run a fresh independent audit. The old PASS must not authorize a changed
candidate.

### Publication gate #73

Issue #73 is selected only after #72 and its release controls have actually
closed. Treat crates.io publication, a protected tag, and a GitHub Release as
irreversible external actions.

Immediately before publication, present the user with:

- the exact audited candidate commit;
- the unconditional #72 PASS report;
- the intended version, tag, crate, artifacts, and checksums;
- the protected workflow/environment that will publish;
- the dry-run and clean-install evidence; and
- any remaining risk or deviation.

Obtain explicit maintainer confirmation unless the protected release
environment itself supplies the required human approval for this exact
candidate. Do not interpret the broad `/goal` invocation as permission to
bypass that final release authorization.

Every candidate-affecting release-preparation PR must merge before the #72
freeze and uses only `Refs #73`; it must not contain `Closes #73`. A
candidate-affecting preparation PR after the PASS invalidates that PASS, and a
closing PR before publication would falsely mark the release gate complete.

After successful publication and live verification, create an in-repository
release-evidence PR targeting `main` and containing `Closes #73`. The
publication action or an external workflow alone does not satisfy the
one-issue/one-PR closure contract.

### Post-release distribution #74

Do not begin #74 before #73 is closed by its merged evidence PR. Confirm the
tap repository, ownership, credentials, and support model before writing
outside this repository. Obtain user direction if the destination or authority
is not already explicit.

Complete and merge the external tap PR first. Then create a dedicated
in-repository evidence/documentation PR targeting `main`, link the external
artifact and validation, and include `Closes #74`. An external-repository PR
cannot be recognized by this repository's selector.

The external tap PR itself must use only the non-closing full reference
`Refs plx/agentic-navigation-guide#74`. Reserve the sole `Closes #74` for the
later in-repository evidence PR.

### Final program gate #26

Issue #26 is last. It must not close merely because `v0.2.0` exists: all native
children, including #74 and any newly discovered program work, must be closed.
Run its completion criteria, commit a final program evidence summary, and
close #26 only through that merged PR.

## Continuity across turns and compaction

GitHub and committed files are the durable source of truth. Never rely only on
conversation memory or an untracked note.

At every handoff or resumed turn:

1. reread this runbook and the work-selection contract;
2. inspect `git status`, current branch, upstream, and worktree ownership;
3. inspect the selected issue and any current PR;
4. record the current issue number, branch, PR URL, stack predecessor, final
   test status, review status, and next action in the goal progress update;
5. verify those facts against GitHub rather than assuming they remained
   unchanged; and
6. continue the current one-issue loop before selecting more work.

Keep every unfinished change on a named, pushed ticket branch or in a clearly
reported local worktree. Do not leave critical progress only in temporary
files.

## Stop and ask conditions

Pause for user direction when:

- the selected ticket contains a contract decision with materially different
  valid outcomes and no decision has already been recorded;
- satisfying the ticket requires destructive migration or external state not
  authorized here;
- branch protection, required review, or a genuine failing check cannot be
  satisfied without an override;
- the selector repeatedly fails closed and safe read-only investigation
  cannot establish why;
- a dependency or closing relationship appears incorrect and changing it
  would alter program scope;
- a required credential, platform, hardware environment, repository, or owner
  is unavailable;
- #73 reaches the irreversible publication checkpoint; or
- #74 lacks an explicit tap destination or authority.

Do not ask merely because a ticket is difficult, a stack needs rebasing, CI
takes time, or the program is long.

## Terminal completion criteria

Mark the goal complete only when all of the following are true:

- every issue in the live `workflow:production-readiness` cohort, including
  #26 and any issues discovered during reassessment, is closed;
- each issue timeline shows closure by its dedicated merged PR, not a direct
  state change;
- the selector returns `status: complete`, `open_count: 0`,
  `covered_count: 0`, and `ready_count: 0`;
- no remediation PR or intentional stack remains open;
- merged branches are removed unless repository policy retains them;
- the independent audit records `PASS — production-ready` for the exact
  released candidate;
- crates.io `0.2.0`, the protected tag, GitHub Release, checksums, provenance,
  and live install smoke evidence agree;
- the third-party Homebrew tap and its in-repository evidence are complete;
- a clean checkout of final `main` passes all required repository, guide,
  package, and release verification; and
- the final response provides issue, PR, audit, release, artifact, and
  validation links sufficient for another maintainer to reproduce the result.

Queue `complete` with covered open issues is not terminal completion. A
published release while #74 or #26 remains open is not terminal completion.
Do not mark the goal achieved early.
