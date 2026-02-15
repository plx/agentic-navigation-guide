# P2-05: Docs and Implementation Alignment Policy

## Problem

`README.md`, `Specification.md`, and implementation behavior have diverged in details. This increases ambiguity and rework cost for future changes.

## Agreed Working Policy

- `Specification.md` is a strong signal of original intent.
- Current implementation and `README.md` are default canonical for current realized behavior, unless they are incoherent or contradictory.

## Desired Behavior

- Establish explicit precedence and update workflow to avoid future drift.
- Keep user-facing docs aligned with actual CLI behavior.

## Proposed Remediation

1. Add a short policy section in `README.md` (and optionally `CLAUDE.md`) documenting source-of-truth precedence.
2. Require behavior changes to include doc updates in same PR when user-facing behavior changes.
3. Add a lightweight “known intentional divergences” section (if any) with date and rationale.
4. Decide whether `Specification.md` remains historical or gets maintained; document that decision explicitly.

## File Targets

- `README.md`
- `Specification.md` (optional, based on policy choice)
- `CLAUDE.md` (optional)
- `.github/pull_request_template.md` (optional process support)

## Acceptance Criteria

- Source-of-truth policy is documented in-repo.
- At least currently known contradictions are either resolved or listed as intentional divergences.

## Suggested Follow-Up

- Add a periodic docs consistency audit checklist item.

