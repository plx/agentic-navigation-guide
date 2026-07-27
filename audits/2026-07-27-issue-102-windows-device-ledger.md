# Issue #102 Windows guide namespace evidence

Date: 2026-07-27

Issue: [#102](https://github.com/plx/agentic-navigation-guide/issues/102)

## Result

The retained #49 trust-ledger row
`trust-guide-windows-device-namespace` now records the already-selected and
implemented outcome: Windows device and named-service namespace spellings are
invalid configuration and reject before filesystem access. Its ID,
`GuideInput` surface, owner issue 49, and surrounding row set remain unchanged.
No Windows namespace support was added.

This correction does not depend on a GitHub organization, another maintainer,
or any other organizational control. It also uses only fixed, reviewable path
spellings; no generated inputs or fuzzing are involved.

## Red-before-change evidence

Before changing either ledger source,
`v0_2_contract_tests::documentation_and_fixture_are_a_bijection` passed. That
green result proved only that the normative Markdown and Rust fixture shared
the same weaker `RejectBeforeRead` wording.

The focused
`issue_102_windows_device_namespace_requires_pre_access_rejection` assertion
was then added alone. It failed with:

```text
left: RejectBeforeRead
right: RejectUsage
```

The failing assertion was committed before the fixture or normative table was
changed.

## Ledger correction

Exactly one normative row and its matching machine-readable fixture changed:

```text
trust-guide-windows-device-namespace
Guide input
Reject as invalid configuration before filesystem access
#49
```

The contract test additionally selects that exact ID and requires
`TrustOutcome::RejectUsage`. The existing bijection and focused-owner tests
continue to enforce row count, exact Markdown/fixture agreement, surface, and
owner.

## Real Windows behavioral evidence

The focused Windows matrix uses these fixed spellings:

```text
\\.\NUL
\\.\pipe\agentic-navigation-guide-test
\\localhost\pipe\agentic-navigation-guide-test
\\localhost\mailslot\agentic-navigation-guide-test
\\localhost\IPC$\agentic-navigation-guide-test
\\?\GLOBALROOT\Device\HarddiskVolume1\agentic-navigation-guide-test
\??\C:\agentic-navigation-guide-test.md
```

For every spelling, the Windows runner exercises:

- `check --guide`;
- `verify --guide --root` with a missing root;
- `check` with `AGENTIC_NAVIGATION_GUIDE_PATH`;
- `verify --root` with the same environment variable and a missing root; and
- the retained binary-internal `GuideLocation` verification route with a
  missing root.

Every case requires the typed `invalid explicit guide path` result and forbids
trust-anchor, filesystem-walk, missing-root, or guide-content diagnostics.
That precedence distinguishes configuration validation from a weaker
did-not-read assertion: invalid guide spelling must win before root
construction or any namespace, metadata, or guide access.

The three-platform CI build matrix has a Windows-only `issue_102` step. A
cross-target build or an unavailable runner cannot satisfy this evidence.

## Active handoff disposition

The closed #49 issue body is historical handoff evidence and retains the
weaker copied row. After this correction merges, #49 receives an explicit
comment identifying #102 and this audit as the superseding active oracle.
Merged PR #85 review history remains unchanged.

## Validation

The focused portable contract, complete debug/release suites, strict Clippy,
formatting, workflow lint/security audit, guide self-verification, exact
package boundary, and real Windows `issue_102` run are required before merge.
