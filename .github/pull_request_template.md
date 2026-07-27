# Pull request

## Problem

<!-- Link one selected issue and explain the reported reason for this change. -->

## Before behavior

<!-- State the parent/base behavior. Avoid claims broader than the evidence. -->

## After behavior

<!-- State the smallest issue-scoped result and any intentionally unchanged behavior. -->

## Red-before-fix evidence

<!--
Record the base commit SHA, exact pre-fix command, nonzero status, and expected
failure reason. For a defect, the test must fail for the reported reason before
the implementation changes; do not use an unrelated failure.
-->

## Validation

<!--
List the focused post-fix and full post-fix commands and results. Identify which
of Linux, macOS, and Windows were exercised locally and which were exercised by
hosted CI.
-->

## Documentation and compatibility

<!--
State the documentation impact and compatibility impact. Name every maintained
contract, README, changelog, package, or release record changed, or write None.
-->

## Security and sensitive data

<!--
State the security impact. Confirm that logs, fixtures, paths, and public text
contain no token, secret, private report detail, personal data, or resolved
external target.
-->

## Dependencies and issue graph

<!--
State the dependency or license impact. Record native blockers, sub-issues, and
related findings that remain separate from this pull request.
-->

## Checklist

- [ ] This pull request addresses one issue and contains no unrelated cleanup.
- [ ] The regression is deterministic, hermetic, bounded, and placed at the
      narrowest useful layer.
- [ ] Focused and full validation results are recorded above.
- [ ] User-facing and normative documentation are aligned where applicable.
- [ ] Dependency, license, package, release, and security impacts are explicit.
- [ ] Every review conversation is resolved before merge.

<!-- Replace NUMBER and keep exactly one issue-scoped closing directive. -->
Closes #NUMBER
