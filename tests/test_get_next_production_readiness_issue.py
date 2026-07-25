"""Tests for the production-readiness issue selector."""

from __future__ import annotations

import contextlib
import io
import json
import unittest
from unittest.mock import patch

from scripts import get_next_production_readiness_issue as selector
from scripts.get_next_production_readiness_issue import (
    ClosingPullRequest,
    GitHubClient,
    Requirement,
    SelectionStatus,
    WorkflowError,
    WorkIssue,
    WorkKind,
    collect_closing_pull_requests,
    get_selection,
    normalize_work_issues,
    select_next,
    validate_workflow_membership,
)

REPO = "plx/agentic-navigation-guide"
UNIVERSE_LABEL = "test:selector-universe"
WORK_LABEL = "test:selector-work"
LEAF_LABEL = "test:selector-leaf"
GATE_LABEL = "test:selector-gate"
UPDATED_AT = "2026-07-25T12:00:00Z"


def _closing_pr(number: int = 900) -> ClosingPullRequest:
    return ClosingPullRequest(
        number=number,
        title=f"Close test issue from PR {number}",
        url=f"https://github.com/{REPO}/pull/{number}",
        is_draft=True,
    )


def _requirement(
    number: int,
    *,
    state: str = "OPEN",
    repository: str = REPO,
) -> Requirement:
    return Requirement(
        node_id=f"I_issue_{number}",
        number=number,
        title=f"Issue {number}",
        url=f"https://github.com/{repository}/issues/{number}",
        state=state,
        repository=repository,
    )


def _issue(
    number: int,
    *,
    priority: int = 1,
    kind: WorkKind = WorkKind.LEAF,
    state: str = "OPEN",
    updated_at: str = UPDATED_AT,
    blockers: tuple[Requirement, ...] = (),
    sub_issues: tuple[Requirement, ...] = (),
    closing_pull_requests: tuple[ClosingPullRequest, ...] = (),
) -> WorkIssue:
    return WorkIssue(
        node_id=f"I_issue_{number}",
        number=number,
        title=f"Issue {number}",
        url=f"https://github.com/{REPO}/issues/{number}",
        state=state,
        updated_at=updated_at,
        priority=priority,
        kind=kind,
        blockers=blockers,
        sub_issues=sub_issues,
        closing_pull_requests=closing_pull_requests,
    )


def _raw_requirement(
    number: int,
    *,
    state: str = "OPEN",
    repository: str = REPO,
) -> dict:
    return {
        "id": f"I_issue_{number}",
        "number": number,
        "title": f"Issue {number}",
        "url": f"https://github.com/{repository}/issues/{number}",
        "state": state,
        "repository": {"nameWithOwner": repository},
    }


def _raw_issue(
    number: int,
    *,
    labels: list[str] | None = None,
    blockers: list[dict] | None = None,
    sub_issues: list[dict] | None = None,
    state: str = "OPEN",
    updated_at: str = UPDATED_AT,
) -> dict:
    label_names = labels or [WORK_LABEL, LEAF_LABEL, "P1"]
    blocker_nodes = blockers or []
    child_nodes = sub_issues or []
    return {
        "id": f"I_issue_{number}",
        "number": number,
        "title": f"Issue {number}",
        "url": f"https://github.com/{REPO}/issues/{number}",
        "state": state,
        "updatedAt": updated_at,
        "labels": {
            "totalCount": len(label_names),
            "nodes": [{"name": label} for label in label_names],
        },
        "blockedBy": {
            "totalCount": len(blocker_nodes),
            "nodes": blocker_nodes,
        },
        "subIssues": {
            "totalCount": len(child_nodes),
            "nodes": child_nodes,
        },
    }


class PureSelectionTests(unittest.TestCase):
    def test_priority_rank_precedes_issue_number(self):
        result = select_next([_issue(10, priority=1), _issue(20, priority=0)])

        self.assertEqual(result.status, SelectionStatus.SELECTED)
        self.assertEqual(result.issue, _issue(20, priority=0))

    def test_issue_number_is_stable_tiebreaker(self):
        result = select_next([_issue(20, priority=0), _issue(10, priority=0)])

        self.assertEqual(result.issue, _issue(10, priority=0))

    def test_repeatable_exclusion_can_skip_winner(self):
        result = select_next(
            [_issue(10, priority=0), _issue(20, priority=0)],
            excluded_numbers=frozenset({10}),
        )

        self.assertEqual(result.issue, _issue(20, priority=0))

    def test_excluding_every_ready_issue_reports_waiting(self):
        result = select_next(
            [_issue(10, priority=0), _issue(20, priority=1)],
            excluded_numbers=frozenset({10, 20}),
        )

        self.assertEqual(result.status, SelectionStatus.WAITING)
        self.assertIn("excluded", result.message.lower())

    def test_leaf_precedes_gate_at_same_priority(self):
        gate = _issue(
            10,
            priority=0,
            kind=WorkKind.GATE,
            blockers=(_requirement(1, state="CLOSED"),),
        )
        leaf = _issue(20, priority=0)

        self.assertEqual(select_next([gate, leaf]).issue, leaf)

    def test_open_closing_pr_covers_its_issue(self):
        result = select_next(
            [
                _issue(10, priority=0, closing_pull_requests=(_closing_pr(),)),
                _issue(20, priority=1),
            ]
        )

        self.assertEqual(result.issue, _issue(20, priority=1))
        self.assertEqual(result.covered_count, 1)

    def test_leaf_can_follow_covered_leaf_blocker(self):
        prerequisite = _issue(
            10,
            closing_pull_requests=(_closing_pr(),),
        )
        dependent = _issue(
            20,
            priority=0,
            blockers=(_requirement(10),),
        )

        self.assertEqual(select_next([prerequisite, dependent]).issue, dependent)

    def test_prematurely_covered_leaf_does_not_unlock_downstream(self):
        root = _issue(10)
        claimed = _issue(
            20,
            priority=0,
            blockers=(_requirement(10),),
            closing_pull_requests=(_closing_pr(),),
        )
        downstream = _issue(
            30,
            priority=0,
            blockers=(_requirement(20),),
        )

        self.assertEqual(select_next([root, claimed, downstream]).issue, root)

    def test_valid_covered_leaf_chain_unlocks_downstream(self):
        root = _issue(10, closing_pull_requests=(_closing_pr(900),))
        middle = _issue(
            20,
            blockers=(_requirement(10),),
            closing_pull_requests=(_closing_pr(901),),
        )
        downstream = _issue(
            30,
            priority=0,
            blockers=(_requirement(20),),
        )

        self.assertEqual(select_next([root, middle, downstream]).issue, downstream)

    def test_gate_waits_for_open_blocker_even_when_blocker_is_covered(self):
        prerequisite = _issue(10, closing_pull_requests=(_closing_pr(),))
        gate = _issue(
            20,
            priority=0,
            kind=WorkKind.GATE,
            blockers=(_requirement(10),),
        )

        result = select_next([prerequisite, gate])

        self.assertEqual(result.status, SelectionStatus.WAITING)
        self.assertIsNone(result.issue)

    def test_gate_waits_for_open_native_sub_issue(self):
        child = _issue(10, closing_pull_requests=(_closing_pr(),))
        gate = _issue(
            20,
            priority=0,
            kind=WorkKind.GATE,
            sub_issues=(_requirement(10),),
        )

        result = select_next([child, gate])

        self.assertEqual(result.status, SelectionStatus.WAITING)
        self.assertIsNone(result.issue)

    def test_gate_becomes_ready_after_native_sub_issue_closes(self):
        gate = _issue(
            20,
            priority=0,
            kind=WorkKind.GATE,
            sub_issues=(_requirement(10, state="CLOSED"),),
        )

        self.assertEqual(select_next([gate]).issue, gate)

    def test_premature_gate_closing_pr_does_not_report_complete(self):
        child = _issue(10, closing_pull_requests=(_closing_pr(900),))
        gate = _issue(
            20,
            priority=0,
            kind=WorkKind.GATE,
            sub_issues=(_requirement(10),),
            closing_pull_requests=(_closing_pr(901),),
        )

        result = select_next([child, gate])

        self.assertEqual(result.status, SelectionStatus.WAITING)

    def test_covered_gate_is_complete_only_after_requirements_land(self):
        gate = _issue(
            20,
            priority=0,
            kind=WorkKind.GATE,
            sub_issues=(_requirement(10, state="CLOSED"),),
            closing_pull_requests=(_closing_pr(),),
        )

        self.assertEqual(select_next([gate]).status, SelectionStatus.COMPLETE)

    def test_closed_workflow_reports_complete(self):
        result = select_next(
            [_issue(10, state="CLOSED")],
            work_label=WORK_LABEL,
        )

        self.assertEqual(result.status, SelectionStatus.COMPLETE)
        self.assertEqual(result.open_count, 0)

    def test_unready_work_reports_waiting_not_false_completion(self):
        result = select_next(
            [_issue(20, blockers=(_requirement(10),))],
            work_label=WORK_LABEL,
        )

        self.assertEqual(result.status, SelectionStatus.WAITING)
        self.assertIn("waiting", result.message)

    def test_json_contains_kind_and_counts(self):
        payload = json.loads(select_next([_issue(20, priority=0)]).as_json())

        self.assertEqual(
            set(payload),
            {
                "status",
                "message",
                "issue",
                "open_count",
                "covered_count",
                "ready_count",
            },
        )
        self.assertEqual(payload["status"], "selected")
        self.assertEqual(payload["issue"]["kind"], "leaf")
        self.assertEqual(payload["issue"]["number"], 20)
        self.assertEqual(payload["ready_count"], 1)


class NormalizationTests(unittest.TestCase):
    def test_requires_exactly_one_kind_label(self):
        raw = _raw_issue(
            10,
            labels=[WORK_LABEL, LEAF_LABEL, GATE_LABEL, "P1"],
        )

        with self.assertRaisesRegex(WorkflowError, "exactly one"):
            normalize_work_issues(
                [raw],
                repository=REPO,
                work_label=WORK_LABEL,
                leaf_label=LEAF_LABEL,
                gate_label=GATE_LABEL,
                closing_pull_requests={},
            )

    def test_role_labels_must_not_collide_with_priority_namespace(self):
        for role, labels in (
            (
                {
                    "work_label": "P0",
                    "leaf_label": LEAF_LABEL,
                    "gate_label": GATE_LABEL,
                },
                ["P0", LEAF_LABEL],
            ),
            (
                {
                    "work_label": WORK_LABEL,
                    "leaf_label": "p1",
                    "gate_label": GATE_LABEL,
                },
                [WORK_LABEL, "p1"],
            ),
            (
                {
                    "work_label": WORK_LABEL,
                    "leaf_label": LEAF_LABEL,
                    "gate_label": "P99",
                },
                [WORK_LABEL, LEAF_LABEL, "P1"],
            ),
        ):
            with self.subTest(role=role):
                raw = _raw_issue(10, labels=labels)
                with self.assertRaisesRegex(WorkflowError, "priority namespace"):
                    normalize_work_issues(
                        [raw],
                        repository=REPO,
                        closing_pull_requests={},
                        **role,
                    )

    def test_requires_exactly_one_known_priority(self):
        for labels in (
            [WORK_LABEL, LEAF_LABEL],
            [WORK_LABEL, LEAF_LABEL, "P4"],
            [WORK_LABEL, LEAF_LABEL, "P0", "P1"],
            [WORK_LABEL, LEAF_LABEL, "P0", "p4"],
        ):
            with self.subTest(labels=labels):
                raw = _raw_issue(10, labels=labels)
                with self.assertRaisesRegex(WorkflowError, "P0..P3"):
                    normalize_work_issues(
                        [raw],
                        repository=REPO,
                        work_label=WORK_LABEL,
                        leaf_label=LEAF_LABEL,
                        gate_label=GATE_LABEL,
                        closing_pull_requests={},
                    )

    def test_membership_rejects_universe_only_issue(self):
        with self.assertRaisesRegex(WorkflowError, f"missing {WORK_LABEL!r}"):
            validate_workflow_membership(
                [_raw_issue(10), _raw_issue(20)],
                [_raw_issue(10)],
                universe_label=UNIVERSE_LABEL,
                work_label=WORK_LABEL,
            )

    def test_membership_rejects_workflow_only_issue(self):
        with self.assertRaisesRegex(WorkflowError, f"missing {UNIVERSE_LABEL!r}"):
            validate_workflow_membership(
                [_raw_issue(10)],
                [_raw_issue(10), _raw_issue(20)],
                universe_label=UNIVERSE_LABEL,
                work_label=WORK_LABEL,
            )

    def test_rejects_truncated_blockers(self):
        raw = _raw_issue(10)
        raw["blockedBy"]["totalCount"] = 1

        with self.assertRaisesRegex(WorkflowError, "truncated"):
            normalize_work_issues(
                [raw],
                repository=REPO,
                work_label=WORK_LABEL,
                leaf_label=LEAF_LABEL,
                gate_label=GATE_LABEL,
                closing_pull_requests={},
            )

    def test_rejects_truncated_sub_issues(self):
        raw = _raw_issue(10)
        raw["subIssues"]["totalCount"] = 1

        with self.assertRaisesRegex(WorkflowError, "truncated"):
            normalize_work_issues(
                [raw],
                repository=REPO,
                work_label=WORK_LABEL,
                leaf_label=LEAF_LABEL,
                gate_label=GATE_LABEL,
                closing_pull_requests={},
            )

    def test_leaf_with_sub_issues_is_invalid(self):
        raw = _raw_issue(10, sub_issues=[_raw_requirement(20)])

        with self.assertRaisesRegex(WorkflowError, "classify it as a gate"):
            normalize_work_issues(
                [raw, _raw_issue(20)],
                repository=REPO,
                work_label=WORK_LABEL,
                leaf_label=LEAF_LABEL,
                gate_label=GATE_LABEL,
                closing_pull_requests={},
            )

    def test_gate_without_native_requirements_is_invalid(self):
        raw = _raw_issue(
            10,
            labels=[WORK_LABEL, GATE_LABEL, "P0"],
        )

        with self.assertRaisesRegex(WorkflowError, "no native blockers"):
            normalize_work_issues(
                [raw],
                repository=REPO,
                work_label=WORK_LABEL,
                leaf_label=LEAF_LABEL,
                gate_label=GATE_LABEL,
                closing_pull_requests={},
            )

    def test_gate_sub_issue_must_belong_to_universe(self):
        raw = _raw_issue(
            10,
            labels=[WORK_LABEL, GATE_LABEL, "P0"],
            sub_issues=[_raw_requirement(20)],
        )

        with self.assertRaisesRegex(WorkflowError, "outside the workflow universe"):
            normalize_work_issues(
                [raw],
                repository=REPO,
                work_label=WORK_LABEL,
                leaf_label=LEAF_LABEL,
                gate_label=GATE_LABEL,
                closing_pull_requests={},
            )

    def test_rejects_multiple_closing_prs_for_one_open_issue(self):
        raw = _raw_issue(10)

        with self.assertRaisesRegex(WorkflowError, "multiple open closing"):
            normalize_work_issues(
                [raw],
                repository=REPO,
                work_label=WORK_LABEL,
                leaf_label=LEAF_LABEL,
                gate_label=GATE_LABEL,
                closing_pull_requests={raw["id"]: (_closing_pr(900), _closing_pr(901))},
            )

    def test_rejects_one_pr_closing_multiple_open_workflow_issues(self):
        first = _raw_issue(10)
        second = _raw_issue(20)
        closing_pr = _closing_pr(900)

        with self.assertRaisesRegex(WorkflowError, "closes multiple workflow issues"):
            normalize_work_issues(
                [first, second],
                repository=REPO,
                work_label=WORK_LABEL,
                leaf_label=LEAF_LABEL,
                gate_label=GATE_LABEL,
                closing_pull_requests={
                    first["id"]: (closing_pr,),
                    second["id"]: (closing_pr,),
                },
            )

    def test_normalizes_gate_blockers_and_sub_issues(self):
        child = _raw_issue(20, state="CLOSED")
        blocker = _raw_issue(30, state="CLOSED")
        gate = _raw_issue(
            10,
            labels=[WORK_LABEL, GATE_LABEL, "P0"],
            blockers=[_raw_requirement(30, state="CLOSED")],
            sub_issues=[_raw_requirement(20, state="CLOSED")],
        )

        normalized = normalize_work_issues(
            [gate, child, blocker],
            repository=REPO,
            work_label=WORK_LABEL,
            leaf_label=LEAF_LABEL,
            gate_label=GATE_LABEL,
            closing_pull_requests={},
        )

        normalized_gate = next(issue for issue in normalized if issue.number == 10)
        self.assertEqual(normalized_gate.kind, WorkKind.GATE)
        self.assertEqual(
            [requirement.number for requirement in normalized_gate.gate_requirements],
            [20, 30],
        )


def _raw_pr(
    number: int,
    *,
    base_branch: str = "main",
    base_repository: str = REPO,
    state: str = "OPEN",
    issue_repository: str = REPO,
    reference_count: int = 1,
) -> dict:
    references = [
        {
            "id": "I_issue_10",
            "number": 10,
            "state": "OPEN",
            "url": f"https://github.com/{issue_repository}/issues/10",
            "repository": {"nameWithOwner": issue_repository},
        }
    ]
    return {
        "number": number,
        "title": f"PR {number}",
        "url": f"https://github.com/{REPO}/pull/{number}",
        "state": state,
        "isDraft": True,
        "baseRefName": base_branch,
        "baseRepository": {"nameWithOwner": base_repository},
        "closingIssuesReferences": {
            "totalCount": reference_count,
            "nodes": references,
        },
    }


class ClosingPullRequestTests(unittest.TestCase):
    def test_collects_draft_default_branch_closing_pr(self):
        indexed = collect_closing_pull_requests(
            [_raw_pr(900)],
            repository=REPO,
            default_branch="main",
        )

        self.assertEqual(indexed["I_issue_10"][0].number, 900)
        self.assertTrue(indexed["I_issue_10"][0].is_draft)

    def test_ignores_prs_that_cannot_close_local_default_branch_issue(self):
        cases = [
            _raw_pr(900, base_branch="release"),
            _raw_pr(900, base_repository="plx/other"),
            _raw_pr(900, state="CLOSED"),
            _raw_pr(900, issue_repository="plx/other"),
        ]
        for pull_request in cases:
            with self.subTest(pull_request=pull_request):
                self.assertEqual(
                    collect_closing_pull_requests(
                        [pull_request],
                        repository=REPO,
                        default_branch="main",
                    ),
                    {},
                )

    def test_rejects_truncated_closing_references(self):
        with self.assertRaisesRegex(WorkflowError, "truncated"):
            collect_closing_pull_requests(
                [_raw_pr(900, reference_count=2)],
                repository=REPO,
                default_branch="main",
            )


def _issues_page(
    nodes: list[dict],
    *,
    total_count: int,
    has_next_page: bool,
    end_cursor: str | None,
    default_branch: str = "main",
) -> str:
    return json.dumps(
        {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": default_branch},
                    "issues": {
                        "totalCount": total_count,
                        "nodes": nodes,
                        "pageInfo": {
                            "hasNextPage": has_next_page,
                            "endCursor": end_cursor,
                        },
                    },
                }
            }
        }
    )


class GitHubClientPaginationTests(unittest.TestCase):
    def test_resolve_repository_canonicalizes_explicit_name(self):
        calls: list[list[str]] = []

        def runner(args):
            calls.append(list(args))
            return f"{REPO}\n"

        client = GitHubClient(runner)

        self.assertEqual(
            client.resolve_repository("PLX/agentic-navigation-guide"), REPO
        )
        self.assertEqual(
            calls[0][:4], ["gh", "repo", "view", "PLX/agentic-navigation-guide"]
        )

    def test_fetch_universe_paginates_and_forwards_cursor(self):
        responses = iter(
            [
                _issues_page(
                    [{"id": "I_1"}],
                    total_count=2,
                    has_next_page=True,
                    end_cursor="cursor-1",
                ),
                _issues_page(
                    [{"id": "I_2"}],
                    total_count=2,
                    has_next_page=False,
                    end_cursor=None,
                ),
            ]
        )
        calls: list[list[str]] = []

        def runner(args):
            calls.append(list(args))
            return next(responses)

        branch, issues = GitHubClient(runner).fetch_work_universe(REPO, UNIVERSE_LABEL)

        self.assertEqual(branch, "main")
        self.assertEqual([issue["id"] for issue in issues], ["I_1", "I_2"])
        self.assertNotIn("endCursor=cursor-1", calls[0])
        self.assertIn("endCursor=cursor-1", calls[1])

    def test_fetch_universe_rejects_count_change(self):
        responses = iter(
            [
                _issues_page(
                    [{"id": "I_1"}],
                    total_count=2,
                    has_next_page=True,
                    end_cursor="cursor-1",
                ),
                _issues_page(
                    [{"id": "I_2"}],
                    total_count=3,
                    has_next_page=False,
                    end_cursor=None,
                ),
            ]
        )

        with self.assertRaisesRegex(WorkflowError, "count changed"):
            GitHubClient(lambda _args: next(responses)).fetch_work_universe(
                REPO, UNIVERSE_LABEL
            )

    def test_fetch_universe_rejects_missing_next_page_cursor(self):
        response = _issues_page(
            [{"id": "I_1"}],
            total_count=2,
            has_next_page=True,
            end_cursor=None,
        )

        with self.assertRaisesRegex(WorkflowError, "no end cursor"):
            GitHubClient(lambda _args: response).fetch_work_universe(
                REPO, UNIVERSE_LABEL
            )

    def test_fetch_universe_rejects_invalid_json(self):
        with self.assertRaisesRegex(WorkflowError, "invalid JSON"):
            GitHubClient(lambda _args: "not JSON").fetch_work_universe(
                REPO, UNIVERSE_LABEL
            )

    def test_fetch_work_membership_paginates_and_forwards_label(self):
        responses = iter(
            [
                _issues_page(
                    [{"id": "I_1", "number": 1, "url": "https://example.test/1"}],
                    total_count=2,
                    has_next_page=True,
                    end_cursor="cursor-1",
                ),
                _issues_page(
                    [{"id": "I_2", "number": 2, "url": "https://example.test/2"}],
                    total_count=2,
                    has_next_page=False,
                    end_cursor=None,
                ),
            ]
        )
        calls: list[list[str]] = []

        def runner(args):
            calls.append(list(args))
            return next(responses)

        issues = GitHubClient(runner).fetch_work_membership(REPO, WORK_LABEL)

        self.assertEqual([issue["id"] for issue in issues], ["I_1", "I_2"])
        self.assertIn(f"workLabel={WORK_LABEL}", calls[0])
        self.assertNotIn("endCursor=cursor-1", calls[0])
        self.assertIn("endCursor=cursor-1", calls[1])


def _guard_requirement(requirement: Requirement) -> dict:
    return {
        "id": requirement.node_id,
        "state": requirement.state,
        "repository": {"nameWithOwner": requirement.repository},
    }


def _guard_closing_pull_request(issue: WorkIssue) -> dict:
    return {
        "number": 900,
        "state": "OPEN",
        "baseRefName": "main",
        "baseRepository": {"nameWithOwner": REPO},
        "closingIssuesReferences": {
            "totalCount": 1,
            "nodes": [
                {
                    "id": issue.node_id,
                    "repository": {"nameWithOwner": REPO},
                }
            ],
        },
    }


def _guard_response(
    issue: WorkIssue,
    *,
    state: str = "OPEN",
    updated_at: str | None = None,
    pull_requests: list[dict] | None = None,
    blockers: list[dict] | None = None,
    sub_issues: list[dict] | None = None,
) -> str:
    priority_label = f"P{issue.priority}"
    kind_label = LEAF_LABEL if issue.kind is WorkKind.LEAF else GATE_LABEL
    blocker_nodes = (
        blockers
        if blockers is not None
        else [_guard_requirement(requirement) for requirement in issue.blockers]
    )
    child_nodes = (
        sub_issues
        if sub_issues is not None
        else [_guard_requirement(requirement) for requirement in issue.sub_issues]
    )
    pull_request_nodes = pull_requests or []
    return json.dumps(
        {
            "data": {
                "repository": {
                    "defaultBranchRef": {"name": "main"},
                    "issue": {
                        "id": issue.node_id,
                        "number": issue.number,
                        "state": state,
                        "updatedAt": updated_at or issue.updated_at,
                        "repository": {"nameWithOwner": REPO},
                        "labels": {
                            "totalCount": 4,
                            "nodes": [
                                {"name": UNIVERSE_LABEL},
                                {"name": WORK_LABEL},
                                {"name": kind_label},
                                {"name": priority_label},
                            ],
                        },
                        "blockedBy": {
                            "totalCount": len(blocker_nodes),
                            "nodes": blocker_nodes,
                        },
                        "subIssues": {
                            "totalCount": len(child_nodes),
                            "nodes": child_nodes,
                        },
                        "closedByPullRequestsReferences": {
                            "totalCount": len(pull_request_nodes),
                            "nodes": pull_request_nodes,
                        },
                    },
                }
            }
        }
    )


def _guard_issue(
    client: GitHubClient,
    issue: WorkIssue,
    *,
    expected_closing_pull_requests: tuple[ClosingPullRequest, ...] = (),
) -> bool:
    return client.selection_is_current(
        repository=REPO,
        default_branch="main",
        universe_label=UNIVERSE_LABEL,
        work_label=WORK_LABEL,
        leaf_label=LEAF_LABEL,
        gate_label=GATE_LABEL,
        issue=issue,
        expected_closing_pull_requests=expected_closing_pull_requests,
    )


class FreshnessGuardTests(unittest.TestCase):
    def test_accepts_unchanged_uncovered_issue(self):
        issue = _issue(10)

        self.assertTrue(
            _guard_issue(GitHubClient(lambda _args: _guard_response(issue)), issue)
        )

    def test_rejects_closed_or_updated_issue(self):
        issue = _issue(10)
        cases = [
            {"state": "CLOSED"},
            {"updated_at": "2026-07-25T12:01:00Z"},
        ]
        for changes in cases:
            with self.subTest(changes=changes):
                client = GitHubClient(
                    lambda _args, changes=changes: _guard_response(issue, **changes)
                )
                self.assertFalse(_guard_issue(client, issue))

    def test_rejects_new_closing_pull_request(self):
        issue = _issue(10)
        response = _guard_response(
            issue,
            pull_requests=[_guard_closing_pull_request(issue)],
        )

        self.assertFalse(_guard_issue(GitHubClient(lambda _args: response), issue))

    def test_accepts_exact_expected_closing_pull_request(self):
        expected_pull_request = _closing_pr()
        issue = _issue(
            10,
            closing_pull_requests=(expected_pull_request,),
        )
        response = _guard_response(
            issue,
            pull_requests=[_guard_closing_pull_request(issue)],
        )

        self.assertTrue(
            _guard_issue(
                GitHubClient(lambda _args: response),
                issue,
                expected_closing_pull_requests=(expected_pull_request,),
            )
        )

    def test_rejects_disappeared_expected_closing_pull_request(self):
        expected_pull_request = _closing_pr()
        issue = _issue(
            10,
            closing_pull_requests=(expected_pull_request,),
        )
        response = _guard_response(issue)

        self.assertFalse(
            _guard_issue(
                GitHubClient(lambda _args: response),
                issue,
                expected_closing_pull_requests=(expected_pull_request,),
            )
        )

    def test_rejects_changed_gate_sub_issue_state(self):
        issue = _issue(
            20,
            kind=WorkKind.GATE,
            sub_issues=(_requirement(10, state="CLOSED"),),
        )
        response = _guard_response(
            issue,
            sub_issues=[_guard_requirement(_requirement(10, state="OPEN"))],
        )

        self.assertFalse(_guard_issue(GitHubClient(lambda _args: response), issue))

    def test_rejects_truncated_nested_closing_references(self):
        issue = _issue(10)
        pull_request = _guard_closing_pull_request(issue)
        pull_request["closingIssuesReferences"]["totalCount"] = 2
        response = _guard_response(issue, pull_requests=[pull_request])

        with self.assertRaisesRegex(WorkflowError, "truncated"):
            _guard_issue(GitHubClient(lambda _args: response), issue)


class _SnapshotClient:
    def __init__(self, snapshots, guard_results=()):
        self._snapshots = iter(snapshots)
        self._guard_results = iter(guard_results)
        self._current_raw_issues = None
        self._current_pull_requests = []
        self.fetch_count = 0
        self.membership_fetch_count = 0
        self.pull_request_fetch_count = 0
        self.guarded_numbers: list[int] = []
        self.guarded_coverage: list[tuple[int, tuple[int, ...]]] = []

    def resolve_repository(self, _repository):
        return REPO

    def fetch_work_universe(self, _repository, _universe_label):
        self.fetch_count += 1
        snapshot = next(self._snapshots)
        default_branch, self._current_raw_issues = snapshot[:2]
        self._current_pull_requests = snapshot[2] if len(snapshot) == 3 else []
        return default_branch, self._current_raw_issues

    def fetch_work_membership(self, _repository, _work_label):
        self.membership_fetch_count += 1
        return self._current_raw_issues

    def fetch_open_pull_requests(self, _repository):
        self.pull_request_fetch_count += 1
        return self._current_pull_requests

    def selection_is_current(self, **kwargs):
        self.guarded_numbers.append(kwargs["issue"].number)
        self.guarded_coverage.append(
            (
                kwargs["issue"].number,
                tuple(
                    pull_request.number
                    for pull_request in kwargs["expected_closing_pull_requests"]
                ),
            )
        )
        return next(self._guard_results)


def _get_selection(client) -> selector.Selection:
    return get_selection(
        client=client,
        repository=REPO,
        universe_label=UNIVERSE_LABEL,
        work_label=WORK_LABEL,
        leaf_label=LEAF_LABEL,
        gate_label=GATE_LABEL,
    )


class StableSnapshotTests(unittest.TestCase):
    def test_returns_same_selected_issue_after_two_snapshots(self):
        client = _SnapshotClient(
            [
                ("main", [_raw_issue(10)]),
                ("main", [_raw_issue(10)]),
            ],
            guard_results=[True, True],
        )

        result = _get_selection(client)

        self.assertEqual(result.issue, _issue(10))
        self.assertEqual(client.fetch_count, 2)
        self.assertEqual(client.guarded_numbers, [10, 10])

    def test_stabilizes_after_one_stale_guard(self):
        client = _SnapshotClient(
            [
                ("main", [_raw_issue(10)]),
                ("main", [_raw_issue(10)]),
                ("main", [_raw_issue(10)]),
                ("main", [_raw_issue(10)]),
            ],
            guard_results=[True, False, True, True],
        )

        result = _get_selection(client)

        self.assertEqual(result.issue, _issue(10))
        self.assertEqual(client.fetch_count, 4)

    def test_rejects_oscillating_selected_issues(self):
        client = _SnapshotClient(
            [
                ("main", [_raw_issue(10)]),
                ("main", [_raw_issue(20)]),
                ("main", [_raw_issue(10)]),
                ("main", [_raw_issue(20)]),
            ],
            guard_results=[True, True, True, True],
        )

        with self.assertRaisesRegex(WorkflowError, "did not stabilize"):
            _get_selection(client)

    def test_same_number_with_new_node_identity_requires_another_snapshot(self):
        original = _raw_issue(10)
        replacement = {**_raw_issue(10), "id": "I_replacement_10"}
        client = _SnapshotClient(
            [
                ("main", [original]),
                ("main", [replacement]),
                ("main", [replacement]),
            ],
            guard_results=[True, True, True],
        )

        result = _get_selection(client)

        self.assertEqual(result.issue.number, 10)
        self.assertEqual(result.issue.node_id, "I_replacement_10")
        self.assertEqual(client.fetch_count, 3)

    def test_complete_requires_two_snapshots(self):
        client = _SnapshotClient(
            [
                ("main", [_raw_issue(10, state="CLOSED")]),
                ("main", [_raw_issue(10, state="CLOSED")]),
            ]
        )

        result = _get_selection(client)

        self.assertEqual(result.status, SelectionStatus.COMPLETE)
        self.assertEqual(client.fetch_count, 2)

    def test_retries_when_first_selected_issue_is_stale(self):
        client = _SnapshotClient(
            [
                ("main", [_raw_issue(10)]),
                ("main", [_raw_issue(20)]),
                ("main", [_raw_issue(20)]),
            ],
            guard_results=[False, True, True],
        )

        self.assertEqual(_get_selection(client).issue, _issue(20))

    def test_errors_when_two_selected_snapshots_are_stale(self):
        client = _SnapshotClient(
            [
                ("main", [_raw_issue(10)]),
                ("main", [_raw_issue(20)]),
            ],
            guard_results=[False, False],
        )

        with self.assertRaisesRegex(WorkflowError, "changed repeatedly"):
            _get_selection(client)

    def test_confirms_complete_after_selected_issue_goes_stale(self):
        client = _SnapshotClient(
            [
                ("main", [_raw_issue(10)]),
                ("main", [_raw_issue(10, state="CLOSED")]),
                ("main", [_raw_issue(10, state="CLOSED")]),
            ],
            guard_results=[False],
        )

        result = _get_selection(client)

        self.assertEqual(result.status, SelectionStatus.COMPLETE)
        self.assertEqual(client.fetch_count, 3)

    def test_supports_complete_stale_complete_sequence(self):
        client = _SnapshotClient(
            [
                ("main", [_raw_issue(10, state="CLOSED")]),
                ("main", [_raw_issue(10)]),
                ("main", [_raw_issue(10, state="CLOSED")]),
                ("main", [_raw_issue(10, state="CLOSED")]),
            ],
            guard_results=[False],
        )

        result = _get_selection(client)

        self.assertEqual(result.status, SelectionStatus.COMPLETE)
        self.assertEqual(client.fetch_count, 4)

    def test_does_not_return_dependent_if_blocker_reopens(self):
        closed_blocker = _raw_requirement(10, state="CLOSED")
        open_blocker = _raw_requirement(10)
        client = _SnapshotClient(
            [
                (
                    "main",
                    [
                        _raw_issue(10, state="CLOSED"),
                        _raw_issue(20, blockers=[closed_blocker]),
                    ],
                ),
                (
                    "main",
                    [
                        _raw_issue(10),
                        _raw_issue(20, blockers=[open_blocker]),
                    ],
                ),
                (
                    "main",
                    [
                        _raw_issue(10),
                        _raw_issue(20, blockers=[open_blocker]),
                    ],
                ),
            ],
            guard_results=[True, True, True],
        )

        result = _get_selection(client)

        self.assertEqual(result.issue, _issue(10))
        self.assertEqual(client.guarded_numbers, [20, 10, 10])

    def test_upstream_coverage_loss_prevents_returning_dependent(self):
        blocker = _raw_issue(10)
        dependent = _raw_issue(
            20,
            labels=[WORK_LABEL, LEAF_LABEL, "P0"],
            blockers=[_raw_requirement(10)],
        )
        covered_snapshot = ("main", [blocker, dependent], [_raw_pr(900)])
        uncovered_snapshot = ("main", [blocker, dependent], [])
        client = _SnapshotClient(
            [
                covered_snapshot,
                covered_snapshot,
                uncovered_snapshot,
                uncovered_snapshot,
            ],
            guard_results=[True, True, True, False, True, True],
        )

        result = _get_selection(client)

        self.assertEqual(result.issue, _issue(10))
        self.assertEqual(
            client.guarded_coverage,
            [
                (20, ()),
                (10, (900,)),
                (20, ()),
                (10, (900,)),
                (10, ()),
                (10, ()),
            ],
        )

    def test_changed_upstream_coverage_proof_requires_another_snapshot(self):
        blocker = _raw_issue(10)
        dependent = _raw_issue(
            20,
            labels=[WORK_LABEL, LEAF_LABEL, "P0"],
            blockers=[_raw_requirement(10)],
        )
        client = _SnapshotClient(
            [
                ("main", [blocker, dependent], [_raw_pr(900)]),
                ("main", [blocker, dependent], [_raw_pr(901)]),
                ("main", [blocker, dependent], [_raw_pr(901)]),
            ],
            guard_results=[True, True, True, True, True, True],
        )

        result = _get_selection(client)

        self.assertEqual(result.issue.number, 20)
        self.assertEqual(client.fetch_count, 3)


class CommandLineTests(unittest.TestCase):
    def test_selected_output_is_exactly_one_url(self):
        selected = select_next([_issue(20, priority=0)])
        stdout = io.StringIO()
        stderr = io.StringIO()

        with (
            patch.object(selector, "get_selection", return_value=selected),
            contextlib.redirect_stdout(stdout),
            contextlib.redirect_stderr(stderr),
        ):
            exit_code = selector.main([])

        self.assertEqual(exit_code, 0)
        self.assertEqual(stdout.getvalue(), f"{selected.issue.url}\n")
        self.assertEqual(stderr.getvalue(), "")

    def test_json_and_repeatable_exclusions_are_forwarded(self):
        selected = select_next([_issue(20, priority=0)])
        stdout = io.StringIO()

        with (
            patch.object(selector, "get_selection", return_value=selected) as mocked,
            contextlib.redirect_stdout(stdout),
        ):
            exit_code = selector.main(["--json", "--exclude", "10", "--exclude", "20"])

        self.assertEqual(exit_code, 0)
        self.assertEqual(json.loads(stdout.getvalue())["status"], "selected")
        self.assertEqual(
            mocked.call_args.kwargs["excluded_numbers"],
            frozenset({10, 20}),
        )

    def test_workflow_error_is_one_clean_stderr_line(self):
        stdout = io.StringIO()
        stderr = io.StringIO()

        with (
            patch.object(
                selector,
                "get_selection",
                side_effect=WorkflowError("bad\nstate"),
            ),
            contextlib.redirect_stdout(stdout),
            contextlib.redirect_stderr(stderr),
        ):
            exit_code = selector.main([])

        self.assertEqual(exit_code, 1)
        self.assertEqual(stdout.getvalue(), "")
        self.assertEqual(stderr.getvalue(), "error: bad state\n")


if __name__ == "__main__":
    unittest.main()
