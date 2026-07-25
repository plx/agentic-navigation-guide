# Production-Readiness Work Selection

## Purpose and authority

This document is the operator contract for progressing the
`agentic-navigation-guide` production-readiness backlog one issue and one
reviewable pull request at a time. The selector is implemented by
[`scripts/get_next_production_readiness_issue.py`](../scripts/get_next_production_readiness_issue.py)
and exposed through the root `justfile`.

The selector reads live GitHub state. The following are authoritative
scheduling inputs:

- issue state;
- the production-readiness workflow labels;
- native GitHub `Blocked by` relationships;
- native GitHub sub-issue relationships; and
- open pull requests that GitHub reports as closing an issue.

Issue-body checklists, prose dependency links, milestones, assignees, and
project-board position are useful context, but they are not scheduling inputs.
Do not expect a body edit alone to change selection order.

The command answers “what remediation work can be started next?” It does not
certify that the project is production-ready. In particular, a `complete`
selection result may mean that every remaining open issue has a valid closing
pull request; those changes may still be unmerged. The independent audit in
[`production-readiness-reassessment-playbook.md`](production-readiness-reassessment-playbook.md)
remains the production-readiness decision procedure.

## Quick start

Prerequisites:

- Python 3.10 or newer;
- [`just`](https://just.systems/);
- [GitHub CLI](https://cli.github.com/) installed and authenticated with read
  access to repository issues and pull requests; and
- a checkout from which `gh repo view` resolves to
  `plx/agentic-navigation-guide`, or an explicit `--repo` argument.

Confirm GitHub access, then ask for the next issue:

```sh
gh auth status
just get-next-production-readiness-issue
```

The default output is deliberately one line so it can be passed directly to an
agent or shell controller.

| State | Standard output | Exit status |
| --- | --- | ---: |
| Selected | The URL of exactly one issue | `0` |
| Complete | A sentence saying all workflow issues are closed or covered | `0` |
| Waiting | A sentence explaining that no issue is currently actionable | `0` |
| Invalid/error | Nothing | nonzero |

Invalid-state and GitHub diagnostics are written to standard error with an
`error:` prefix. `waiting` is a valid workflow state, not an execution error.

Use an explicit repository when running outside its checkout:

```sh
just get-next-production-readiness-issue \
  --repo plx/agentic-navigation-guide
```

Use machine-readable output when a controller must distinguish states without
parsing prose:

```sh
just get-next-production-readiness-issue --json
```

The JSON object includes `status`, `message`, `issue`, `open_count`,
`covered_count`, and `ready_count`. `issue` is `null` for `complete` and
`waiting`. `covered_count` counts open workflow issues claimed by recognized
closing pull requests; it does not include already closed issues.

If a returned issue cannot safely be started in the current working context,
exclude it for that invocation:

```sh
just get-next-production-readiness-issue \
  --exclude 34 \
  --exclude 35
```

`--exclude` is repeatable, is applied only after dependency readiness is
computed, and does not mutate GitHub. If every ready issue is excluded, the
result is `waiting`. An exclusion is an operator convenience, not a mechanism
for waiving or completing work.

Run the offline selector test suite with:

```sh
just test-production-readiness-selector
```

The implementation has no third-party Python dependencies.

## Current workflow universe

The production-readiness program contains issues
[#26](https://github.com/plx/agentic-navigation-guide/issues/26) through
[#74](https://github.com/plx/agentic-navigation-guide/issues/74), inclusive.
The issue numbers and counts below describe the initial program; the selector
does not hard-code them.

- `production-readiness` has 49 issues and defines the canonical universe:
  every issue that must remain visible.
- `workflow:production-readiness` has the same 49 issues and defines redundant
  automatic-selection membership.
- `workflow:production-readiness-leaf` has 39 independently actionable issues:
  #34–#71 and #74.
- `workflow:production-readiness-gate` has 10 aggregate issues: #26–#33, #72,
  and #73.
- Each of the 49 issues has exactly one of `P0`–`P3`, ordered from highest to
  lowest scheduling urgency.

The two 49-issue cohorts must match by GitHub node identity. Redundant
membership is intentional: a missing canonical label must not silently remove
work, and a stray workflow label must not silently add work.

The initial native graph has the following aggregate shape:

```text
#34–#71 remediation leaves
          │
          ▼
#27–#33 component/topic gates
          │
          ▼
#72 independent reassessment gate
          │
          ▼
#73 v0.2.0 publication gate
          │
          ▼
#74 post-publication verification leaf
          │
          ▼
#26 top-level program gate
```

[#26](https://github.com/plx/agentic-navigation-guide/issues/26) is the
top-level native parent for the component gates and the audit, release, and
post-release work. Because #74 is one of its native children, the program gate
cannot become ready merely because the `v0.2.0` publication gate has closed:
post-publication verification must also land. This graph is expressed in
GitHub, not in the script, so live relationship changes take effect on the
next run.

On the implementation baseline of 2026-07-25, a production-label run selected
[#34](https://github.com/plx/agentic-navigation-guide/issues/34) from 49 open
issues, with zero covered and 11 ready issues. That is a recorded snapshot, not
a permanently expected issue number.

## Selection semantics

### Covered, sequenced, and landed

These terms are deliberately different:

- **Landed:** the issue state is `CLOSED`.
- **Claimed or covered:** an open workflow issue has an open pull request that
  GitHub recognizes as closing it.
- **Sequenced coverage:** the issue is covered and its own dependency chain is
  valid for planning downstream leaf work.
- **Ready:** the issue is open, uncovered, not excluded, and its applicable
  dependency rules are satisfied.

An open pull request counts as coverage only when:

- it targets this repository;
- its base is the repository’s current default branch;
- it is open; and
- GitHub includes the workflow issue in the PR’s
  `closingIssuesReferences`.

Draft PRs count. A body that merely says `References #34`, a closed-unmerged
PR, a PR against a non-default branch, or a textual link GitHub does not
recognize as a closing reference does not count.

Coverage is computed from roots outward. A premature PR can claim an issue, but
it cannot unlock dependent work until its own prerequisites form a valid
sequence. This prevents a set of closing keywords from bypassing the native
dependency graph.

### Leaf readiness

A leaf uses its native `Blocked by` relationships as prerequisites.

- A closed blocker is satisfied.
- An open blocker may be treated as scheduling-complete when it is a workflow
  **leaf** with a recognized closing PR and its own leaf prerequisites are
  already landed or validly covered.
- An open gate blocker is not satisfied by a PR; the gate must be closed.

This transitive leaf rule allows a deliberate chain of reviewable PRs to be
prepared before all upstream reviews merge. It does not grant permission to
merge out of order. If a dependent implementation cannot be made independently
against the default branch, either document and manage an intentional PR stack
or use `--exclude` until its prerequisite lands.

A leaf may not have native sub-issues. Such an issue is structurally an
aggregate and must be relabeled as a gate or have its children removed.

### Gate readiness

A gate’s requirements are the deduplicated union of:

- native `Blocked by` issues; and
- native sub-issues.

Every gate requirement must be actually `CLOSED`. An open closing PR on a
child or blocker is not enough. A premature closing PR on the gate itself also
cannot make an invalid sequence complete.

When all requirements have landed, the gate becomes selectable so an operator
can perform its aggregate acceptance criteria. The operator may then close the
gate with retained evidence or open one focused default-branch PR that closes
it.

GitHub does not enforce this lifecycle when someone closes an issue manually.
The selector validates readiness for an **open** gate; it is not a historical
auditor that can prove a closed gate was closed at the right time. Gate owners
must not manually close a gate before its requirements land, and the final
reassessment must review gate evidence independently.

### Deterministic ordering

After readiness and exclusions are evaluated, the selected issue is the
minimum by:

1. priority: `P0`, then `P1`, `P2`, and `P3`;
2. work kind: leaf before gate at equal priority; and
3. ascending issue number.

Issue number does not override dependency readiness or priority.

### Complete and waiting

The selector returns `complete` when every workflow issue is either closed or
covered by a validly sequenced open closing PR. This is queue completion, not
proof that all work has landed and not a production-readiness verdict.

The selector returns `waiting` when, for example:

- all uncovered work is blocked on prerequisites;
- every ready issue was explicitly excluded; or
- all open issues are claimed by PRs, but at least one claim is premature and
  cannot be placed into a valid dependency sequence.

Do not work around `waiting` by deleting a dependency or relabeling an issue.
Inspect the native graph and the PR closing relationships, then wait for the
necessary merge or correct invalid metadata.

## Closing-pull-request contract

Each implementation PR should close exactly one workflow issue. Put a GitHub
closing keyword in the PR body, for example:

```text
Closes #34
```

Target the repository’s default branch, then verify GitHub’s interpretation:

```sh
gh pr view <pr-number> --json baseRefName,closingIssuesReferences,isDraft,state
```

The selector intentionally fails closed when:

- one open workflow issue has more than one open closing PR; or
- one open PR closes more than one open workflow issue.

Those relationships are ambiguous for one-ticket-at-a-time work selection.
Correct the PR bodies or close the superseded PR before rerunning. A single PR
may mention other issues, but it should have exactly one workflow issue in
GitHub’s closing references.

If a closing keyword is removed or its PR closes without merge, the issue
becomes selectable again after GitHub updates its index. GitHub can take a few
seconds to index a new or changed closing reference; verify the field above and
rerun before changing issue metadata.

The closing relationship marks work in progress. Ticket acceptance criteria,
tests, review, and merge requirements still apply. Native issue dependencies
also do not enforce GitHub merge order: a dependent or stacked PR must not
merge before every prerequisite that its correctness relies upon has landed.

## Fail-closed validation

The selector refuses to choose work when the scheduling model is incomplete,
ambiguous, inaccessible, or internally inconsistent. Its checks include:

- the canonical universe is nonempty;
- canonical and redundant membership cohorts match exactly;
- every member has the work label;
- every member has exactly one leaf/gate label;
- workflow role-label names cannot collide with the `P0`–`P3` namespace;
- every member has exactly one case-sensitive, recognized `P0`–`P3` label;
- unknown priority-shaped labels such as `P4` are rejected;
- every leaf has no native sub-issues;
- every gate has at least one native blocker or native sub-issue;
- every native sub-issue is in the same repository and in the workflow
  universe;
- cross-repository blockers are rejected;
- an open same-repository blocker outside the workflow universe is rejected;
- one workflow issue cannot have multiple open closing PRs;
- one PR cannot close multiple open workflow issues;
- issue, label, blocker, sub-issue, pull-request, and closing-reference
  connections must be complete rather than truncated;
- paginated issue, membership, and PR counts and identities must remain stable;
- the default branch must exist and remain unchanged during selection; and
- GraphQL errors, inaccessible objects, malformed responses, repeated cursors,
  and unstable live state produce an error instead of a guess.

A closed same-repository blocker outside the universe can be accepted as an
already landed prerequisite. New production-remediation dependencies should
nevertheless be added to the workflow universe so their work and evidence
remain visible.

GitHub connections embedded in individual issues are capped at 100 by the
query. If any label, blocker, sub-issue, or closing-reference connection
exceeds that bound, the reported total no longer matches the fetched nodes and
the selector fails rather than ignoring the remainder.

## Freshness, stabilization, and concurrency

Before printing a selected URL, the selector requires the same issue and its
exact covered-prerequisite proof to win two consecutive full dependency
snapshots. The candidate and every covered leaf in that proof also receive
targeted freshness queries that confirm:

- repository and issue identity;
- default branch;
- open state and `updatedAt`;
- universe, membership, kind, and priority labels;
- blocker and sub-issue identities and states; and
- the selected issue still has no default-branch closing PR; and
- each covered prerequisite still has exactly the expected eligible closing
  PR relationship.

A targeted race causes one retry. Repeated selected-issue changes or failure to
stabilize within four snapshots exits nonzero. A `complete` result likewise
requires two consecutive complete snapshots. A `waiting` result is returned
from the current internally consistent snapshot.

These checks narrow the race window; they do not make the command an atomic
claim service. Two agents can receive the same issue before either creates its
closing PR. The supported operating model is one coordinating selector loop.
Before parallel autonomous workers are introduced, add a serialized claim
mechanism rather than treating labels or assignees as compare-and-swap locks.

Immediately before starting work, confirm that the selected issue is still
open and has no closing PR. If another worker claimed it, rerun the selector.

## Operator loop

For each remediation unit:

1. Run `just get-next-production-readiness-issue`.
2. If it returns a URL, read the entire issue, its native relationships, and
   linked prior art.
3. Confirm the issue is still open and unclaimed.
4. Implement exactly that ticket’s scope and its required failing-before-fix
   regression where applicable.
5. Run the issue-specific validation plus the repository-wide checks required
   by the ticket.
6. Open one reviewable default-branch PR containing `Closes #N`.
7. Verify `closingIssuesReferences` with `gh pr view`.
8. Rerun the selector.
9. On `waiting`, inspect prerequisites and merge state; do not bypass the
   graph.
10. On `complete`, distinguish open covered issues from landed issues before
    proceeding to an audit or release action.

Gate tickets require their own aggregate validation after all requirements
close. Never auto-close a gate solely because its child count reached zero.

## Maintaining the workflow

Add or split work atomically. Before moving acceptance criteria into a new
ticket:

1. create a self-contained issue with the expected behavior, implementation
   direction, validation, and acceptance criteria;
2. add `production-readiness`;
3. add `workflow:production-readiness`;
4. add exactly one of `workflow:production-readiness-leaf` and
   `workflow:production-readiness-gate`;
5. add exactly one of `P0`, `P1`, `P2`, and `P3`;
6. add its native `Blocked by` relationships;
7. add it as a native sub-issue of every aggregate gate whose completion
   depends on it; and
8. rerun the selector in JSON mode to validate the entire taxonomy.

A gate must have at least one native relationship. A prose-only link,
milestone, or checkbox does not preserve scheduling semantics. When work is
removed, split, reprioritized, or moved between gates, update all labels and
native relationships in the same maintenance operation and rerun the
selector.

Do not change the four workflow labels merely to obtain a preferred answer.
The labels and graph are release-control metadata and should receive the same
review as code.

## Alternate-label integration testing

The CLI accepts alternate labels so its GraphQL behavior can be exercised
against real GitHub objects without changing the production cohort:

```sh
just get-next-production-readiness-issue \
  --repo plx/agentic-navigation-guide \
  --universe-label test:selector:<run-id>:universe \
  --work-label test:selector:<run-id>:work \
  --leaf-label test:selector:<run-id>:leaf \
  --gate-label test:selector:<run-id>:gate \
  --json
```

Use four distinct labels. Test issues must carry exactly one `P0`–`P3` label
and exactly one alternate kind label. Keep their branches and titles namespaced
by the run ID, use harmless commits, never merge the fixture PRs, and record
the pre-test default-branch SHA and production scheduling state.

An integration run must demonstrate at least:

1. priority and issue-number ordering;
2. a native blocked-by relationship between leaves;
3. a gate whose native sub-issues are its hard requirements;
4. a reference-only PR not counting as coverage;
5. a draft default-branch PR with a closing keyword counting as coverage;
6. transitive leaf sequencing through covered prerequisites;
7. covered-but-open children not satisfying a gate;
8. a premature gate PR not bypassing its open children;
9. the gate becoming selectable after its children close;
10. a valid gate PR yielding queue-complete while the gate is still open; and
11. full completion after every fixture closes.

### Recorded live smoke test: `20260725T200337Z`

The alternate-label test was executed against
`plx/agentic-navigation-guide` on 2026-07-25. Production state was isolated
from the fixtures with these labels:

```text
test:selector:20260725T200337Z:universe
test:selector:20260725T200337Z:work
test:selector:20260725T200337Z:leaf
test:selector:20260725T200337Z:gate
```

The fixture graph was:

- P1 leaf [#76](https://github.com/plx/agentic-navigation-guide/issues/76),
  named A;
- P0 leaf [#77](https://github.com/plx/agentic-navigation-guide/issues/77),
  named B and natively blocked by A;
- independent P0 leaf
  [#78](https://github.com/plx/agentic-navigation-guide/issues/78), named C;
  and
- P0 gate [#79](https://github.com/plx/agentic-navigation-guide/issues/79),
  named G, with A, B, and C as native sub-issues.

Four draft, default-branch fixture PRs were opened:

- [#80](https://github.com/plx/agentic-navigation-guide/pull/80) for C;
- [#81](https://github.com/plx/agentic-navigation-guide/pull/81) for A;
- [#82](https://github.com/plx/agentic-navigation-guide/pull/82) for B; and
- [#83](https://github.com/plx/agentic-navigation-guide/pull/83) for G.

Each used a namespaced branch and a harmless `[skip ci]` fixture commit. No
fixture PR was merged.

The following results were observed and asserted:

| Transition | Status | Selected | Open | Covered | Ready |
| --- | --- | --- | ---: | ---: | ---: |
| Initial A/B/C/G state | selected | C #78 | 4 | 0 | 2 |
| PR #80 said only `References #78` | selected | C #78 | 4 | 0 | 2 |
| PR #80 changed to `Closes #78` | selected | A #76 | 4 | 1 | 1 |
| PR #81 added `Closes #76` | selected | B #77 | 4 | 2 | 1 |
| PR #82 added `Closes #77` | waiting | — | 4 | 3 | 0 |
| Premature PR #83 added `Closes #79` | waiting | — | 4 | 4 | 0 |
| PR #83 closed without merge | waiting | — | 4 | 3 | 0 |
| Leaves #76–#78 closed as completed | selected | G #79 | 1 | 0 | 1 |
| PR #83 reopened after the leaves landed | complete | — | 1 | 1 | 0 |
| PR #83 closed again | selected | G #79 | 1 | 0 | 1 |
| Gate #79 closed as completed | complete | — | 0 | 0 | 0 |

This sequence proves both sides of the covered/landed distinction: covered
leaves can sequence another leaf, while covered children and a premature gate
PR cannot unlock or complete a gate. It also proves that `complete` can be a
queue state with one still-open but validly covered gate.

Before fixture creation, the recorded default-branch commit was
`bf40806993ce0e0f2de931cabd105e3a0e063789`. The production scheduling
snapshot hash was
`03d12772138055582a2926fdf1a044a1adb16bcb0967b2a18176939cbc2f6a25`,
computed from the sorted production issue identities, states, labels, native
blockers, and native sub-issues.

Cleanup completed after the terminal assertion. PRs #80–#83 were closed
without merge, while closed issues #76–#79 and closed PRs #80–#83 were
retained as durable evidence. The four temporary labels, the P0/P1 label
associations on the fixture issues, and the four namespaced remote branches
were removed, leaving no fake work or branch active. Verification confirmed:

- no fixture PR was merged;
- no fixture issue or PR remains open;
- no `test:selector:20260725T200337Z:*` label or branch remains;
- the default branch SHA is unchanged by the smoke test;
- the production scheduling snapshot hash is unchanged; and
- a normal production-label run still selects the same production issue as
  immediately before the smoke test.

The default branch remained at
`bf40806993ce0e0f2de931cabd105e3a0e063789`, the production snapshot hash
remained
`03d12772138055582a2926fdf1a044a1adb16bcb0967b2a18176939cbc2f6a25`,
and the production selector again returned #34 with 49 open, zero covered,
and 11 ready issues. The pre-existing open PR set (#9, #21, and #75) was
restored, and no Actions run remained for a fixture branch.

Closed fixture URLs are intentionally retained because GitHub issues and PRs
are the evidence needed to inspect the tested native graph and closing
relationships.

## Provenance

This workflow is adapted from the automatic production-readiness selector and
`just` recipe introduced by
[`plx/ferric-rules` PR #226](https://github.com/plx/ferric-rules/pull/226).
The core policies retained here are live GitHub state, redundant cohort
validation, one issue per closing PR, deterministic priority ordering,
transitive covered-leaf sequencing, landed-only gates, stable snapshots, and
fail-closed handling of ambiguous data.

The repository-specific adaptation is material:

- it is a standalone standard-library Python program rather than part of the
  `ferric-tools` package;
- it uses this repository’s `P0`–`P3` and workflow label taxonomy; and
- native GitHub sub-issues are authoritative gate prerequisites in addition
  to `Blocked by` relationships.

The last point is essential for issues #26–#74: their organizing epics are
native parents, so treating only `Blocked by` edges as hard requirements would
allow a gate to become actionable before all of its children had landed.
