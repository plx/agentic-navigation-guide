#!/usr/bin/env python3
"""Select the next production-readiness issue from live GitHub state.

This is a standard-library adaptation of the selector introduced in
plx/ferric-rules#226. In this repository, native sub-issues are also
authoritative gate prerequisites: epics must not become ready until every
child issue is actually closed.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import asdict, dataclass
from enum import Enum
from typing import Any

DEFAULT_UNIVERSE_LABEL = "production-readiness"
DEFAULT_WORK_LABEL = "workflow:production-readiness"
DEFAULT_LEAF_LABEL = "workflow:production-readiness-leaf"
DEFAULT_GATE_LABEL = "workflow:production-readiness-gate"
PRIORITY_LABELS = {f"P{rank}": rank for rank in range(4)}
PRIORITY_LABEL_PATTERN = re.compile(r"P\d+", re.IGNORECASE)

ISSUES_QUERY = """
query(
  $owner: String!
  $name: String!
  $universeLabel: String!
  $endCursor: String
) {
  repository(owner: $owner, name: $name) {
    defaultBranchRef {
      name
    }
    issues(
      first: 100
      after: $endCursor
      states: [OPEN, CLOSED]
      labels: [$universeLabel]
      orderBy: {field: CREATED_AT, direction: ASC}
    ) {
      totalCount
      nodes {
        id
        number
        title
        url
        state
        updatedAt
        labels(first: 100) {
          totalCount
          nodes {
            name
          }
        }
        blockedBy(first: 100) {
          totalCount
          nodes {
            id
            number
            title
            url
            state
            repository {
              nameWithOwner
            }
          }
        }
        subIssues(first: 100) {
          totalCount
          nodes {
            id
            number
            title
            url
            state
            repository {
              nameWithOwner
            }
          }
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
}
"""

PULL_REQUESTS_QUERY = """
query(
  $owner: String!
  $name: String!
  $endCursor: String
) {
  repository(owner: $owner, name: $name) {
    pullRequests(
      first: 100
      after: $endCursor
      states: OPEN
      orderBy: {field: CREATED_AT, direction: ASC}
    ) {
      totalCount
      nodes {
        number
        title
        url
        state
        isDraft
        baseRefName
        baseRepository {
          nameWithOwner
        }
        closingIssuesReferences(first: 100) {
          totalCount
          nodes {
            id
            number
            state
            url
            repository {
              nameWithOwner
            }
          }
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
}
"""

WORK_MEMBERSHIP_QUERY = """
query(
  $owner: String!
  $name: String!
  $workLabel: String!
  $endCursor: String
) {
  repository(owner: $owner, name: $name) {
    issues(
      first: 100
      after: $endCursor
      states: [OPEN, CLOSED]
      labels: [$workLabel]
      orderBy: {field: CREATED_AT, direction: ASC}
    ) {
      totalCount
      nodes {
        id
        number
        url
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
}
"""

ISSUE_GUARD_QUERY = """
query(
  $owner: String!
  $name: String!
  $number: Int!
) {
  repository(owner: $owner, name: $name) {
    defaultBranchRef {
      name
    }
    issue(number: $number) {
      id
      number
      state
      updatedAt
      repository {
        nameWithOwner
      }
      labels(first: 100) {
        totalCount
        nodes {
          name
        }
      }
      blockedBy(first: 100) {
        totalCount
        nodes {
          id
          state
          repository {
            nameWithOwner
          }
        }
      }
      subIssues(first: 100) {
        totalCount
        nodes {
          id
          state
          repository {
            nameWithOwner
          }
        }
      }
      closedByPullRequestsReferences(first: 100, includeClosedPrs: false) {
        totalCount
        nodes {
          number
          state
          baseRefName
          baseRepository {
            nameWithOwner
          }
          closingIssuesReferences(first: 100) {
            totalCount
            nodes {
              id
              repository {
                nameWithOwner
              }
            }
          }
        }
      }
    }
  }
}
"""


class WorkflowError(RuntimeError):
    """Raised when live workflow data is unavailable or inconsistent."""


class WorkKind(str, Enum):
    """Kinds of work selected by the remediation workflow."""

    LEAF = "leaf"
    GATE = "gate"


class SelectionStatus(str, Enum):
    """Possible selector outcomes."""

    SELECTED = "selected"
    COMPLETE = "complete"
    WAITING = "waiting"


@dataclass(frozen=True)
class ClosingPullRequest:
    """An open default-branch PR that will close an issue when merged."""

    number: int
    title: str
    url: str
    is_draft: bool


@dataclass(frozen=True)
class Requirement:
    """A native blocker or sub-issue that constrains workflow readiness."""

    node_id: str
    number: int
    title: str
    url: str
    state: str
    repository: str

    @property
    def landed(self) -> bool:
        """Return whether the required issue is actually closed."""
        return self.state == "CLOSED"


@dataclass(frozen=True)
class WorkIssue:
    """Normalized issue metadata used by the pure selector."""

    node_id: str
    number: int
    title: str
    url: str
    state: str
    updated_at: str
    priority: int
    kind: WorkKind
    blockers: tuple[Requirement, ...] = ()
    sub_issues: tuple[Requirement, ...] = ()
    closing_pull_requests: tuple[ClosingPullRequest, ...] = ()

    @property
    def covered(self) -> bool:
        """Return whether this issue should be skipped by future selection."""
        return self.state == "CLOSED" or bool(self.closing_pull_requests)

    @property
    def gate_requirements(self) -> tuple[Requirement, ...]:
        """Return deduplicated blocker and child requirements for a gate."""
        by_id = {
            requirement.node_id: requirement
            for requirement in (*self.blockers, *self.sub_issues)
        }
        return tuple(sorted(by_id.values(), key=lambda requirement: requirement.number))


@dataclass(frozen=True)
class Selection:
    """A deterministic issue-selection result."""

    status: SelectionStatus
    message: str
    issue: WorkIssue | None
    open_count: int
    covered_count: int
    ready_count: int

    def as_json(self) -> str:
        """Serialize the result for callers that need machine-readable state."""
        payload = asdict(self)
        payload["status"] = self.status.value
        if self.issue is not None:
            payload["issue"]["kind"] = self.issue.kind.value
        return json.dumps(payload, sort_keys=True)


CommandRunner = Callable[[Sequence[str]], str]
SelectionCandidate = tuple[
    SelectionStatus,
    str | None,
    tuple[tuple[str, tuple[int, ...]], ...],
]


def _default_command_runner(args: Sequence[str]) -> str:
    try:
        completed = subprocess.run(
            args,
            check=False,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError as error:
        raise WorkflowError(
            f"required command {args[0]!r} was not found; install and authenticate GitHub CLI"
        ) from error

    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "unknown error"
        raise WorkflowError(f"{' '.join(args[:3])} failed: {detail}")
    return completed.stdout


class GitHubClient:
    """Small GraphQL client implemented through authenticated GitHub CLI."""

    def __init__(self, runner: CommandRunner = _default_command_runner) -> None:
        self._runner = runner

    def resolve_repository(self, repository: str | None) -> str:
        """Resolve and validate an owner/name repository identifier."""
        args = ["gh", "repo", "view"]
        if repository is not None:
            args.append(repository)
        args.extend(["--json", "nameWithOwner", "--jq", ".nameWithOwner"])
        resolved = self._runner(args).strip()
        if not re.fullmatch(r"[^/\s]+/[^/\s]+", resolved or ""):
            raise WorkflowError(
                f"expected repository in owner/name form, got {resolved!r}"
            )
        return resolved

    def _graphql(
        self,
        query: str,
        *,
        owner: str,
        name: str,
        universe_label: str | None = None,
        work_label: str | None = None,
        cursor: str | None = None,
        issue_number: int | None = None,
    ) -> dict[str, Any]:
        args = [
            "gh",
            "api",
            "graphql",
            "-f",
            f"query={query}",
            "-f",
            f"owner={owner}",
            "-f",
            f"name={name}",
        ]
        if universe_label is not None:
            args.extend(["-f", f"universeLabel={universe_label}"])
        if work_label is not None:
            args.extend(["-f", f"workLabel={work_label}"])
        if cursor is not None:
            args.extend(["-f", f"endCursor={cursor}"])
        if issue_number is not None:
            args.extend(["-F", f"number={issue_number}"])

        raw = self._runner(args)
        try:
            payload = json.loads(raw)
        except json.JSONDecodeError as error:
            raise WorkflowError("GitHub GraphQL returned invalid JSON") from error
        if payload.get("errors"):
            raise WorkflowError(f"GitHub GraphQL returned errors: {payload['errors']}")
        return payload

    @staticmethod
    def _validate_connection(
        connection: dict[str, Any],
        *,
        subject: str,
    ) -> list[dict[str, Any]]:
        nodes = connection["nodes"]
        if connection["totalCount"] != len(nodes):
            raise WorkflowError(f"{subject} were truncated")
        return nodes

    def fetch_work_universe(
        self,
        repository: str,
        universe_label: str,
    ) -> tuple[str, list[dict[str, Any]]]:
        """Fetch every open or closed issue in the canonical workflow universe."""
        owner, name = repository.split("/", 1)
        cursor: str | None = None
        nodes: list[dict[str, Any]] = []
        expected_total: int | None = None
        default_branch: str | None = None
        seen_cursors: set[str] = set()

        while True:
            payload = self._graphql(
                ISSUES_QUERY,
                owner=owner,
                name=name,
                universe_label=universe_label,
                cursor=cursor,
            )
            repo_data = payload.get("data", {}).get("repository")
            if repo_data is None:
                raise WorkflowError(
                    f"repository {repository!r} was not found or is inaccessible"
                )

            branch = (repo_data.get("defaultBranchRef") or {}).get("name")
            if not branch:
                raise WorkflowError(f"repository {repository!r} has no default branch")
            if default_branch is not None and branch != default_branch:
                raise WorkflowError(
                    "repository default branch changed during pagination"
                )
            default_branch = branch

            connection = repo_data["issues"]
            if expected_total is None:
                expected_total = connection["totalCount"]
            elif connection["totalCount"] != expected_total:
                raise WorkflowError("workflow issue count changed during pagination")
            nodes.extend(connection["nodes"])
            page_info = connection["pageInfo"]
            if not page_info["hasNextPage"]:
                break
            cursor = page_info.get("endCursor")
            if not cursor:
                raise WorkflowError("issue pagination had no end cursor")
            if cursor in seen_cursors:
                raise WorkflowError("issue pagination repeated an end cursor")
            seen_cursors.add(cursor)

        if expected_total == 0:
            raise WorkflowError(f"no issues carry universe label {universe_label!r}")
        if expected_total != len({node["id"] for node in nodes}):
            raise WorkflowError(
                "workflow issue set changed or pagination was incomplete; rerun the selector"
            )
        return default_branch, nodes

    def fetch_work_membership(
        self,
        repository: str,
        work_label: str,
    ) -> list[dict[str, Any]]:
        """Fetch every issue carrying the redundant workflow membership label."""
        owner, name = repository.split("/", 1)
        cursor: str | None = None
        nodes: list[dict[str, Any]] = []
        expected_total: int | None = None
        seen_cursors: set[str] = set()

        while True:
            payload = self._graphql(
                WORK_MEMBERSHIP_QUERY,
                owner=owner,
                name=name,
                work_label=work_label,
                cursor=cursor,
            )
            repo_data = payload.get("data", {}).get("repository")
            if repo_data is None:
                raise WorkflowError(
                    f"repository {repository!r} was not found or is inaccessible"
                )
            connection = repo_data["issues"]
            if expected_total is None:
                expected_total = connection["totalCount"]
            elif connection["totalCount"] != expected_total:
                raise WorkflowError(
                    "workflow membership count changed during pagination"
                )
            nodes.extend(connection["nodes"])
            page_info = connection["pageInfo"]
            if not page_info["hasNextPage"]:
                break
            cursor = page_info.get("endCursor")
            if not cursor:
                raise WorkflowError("workflow membership pagination had no end cursor")
            if cursor in seen_cursors:
                raise WorkflowError(
                    "workflow membership pagination repeated an end cursor"
                )
            seen_cursors.add(cursor)

        if expected_total != len({node["id"] for node in nodes}):
            raise WorkflowError(
                "workflow membership changed or pagination was incomplete; rerun the selector"
            )
        return nodes

    def fetch_open_pull_requests(self, repository: str) -> list[dict[str, Any]]:
        """Fetch every open PR so closing issue references can be indexed."""
        owner, name = repository.split("/", 1)
        cursor: str | None = None
        nodes: list[dict[str, Any]] = []
        expected_total: int | None = None
        seen_cursors: set[str] = set()

        while True:
            payload = self._graphql(
                PULL_REQUESTS_QUERY,
                owner=owner,
                name=name,
                cursor=cursor,
            )
            repo_data = payload.get("data", {}).get("repository")
            if repo_data is None:
                raise WorkflowError(
                    f"repository {repository!r} was not found or is inaccessible"
                )
            connection = repo_data["pullRequests"]
            if expected_total is None:
                expected_total = connection["totalCount"]
            elif connection["totalCount"] != expected_total:
                raise WorkflowError("open pull-request count changed during pagination")
            nodes.extend(connection["nodes"])
            page_info = connection["pageInfo"]
            if not page_info["hasNextPage"]:
                break
            cursor = page_info.get("endCursor")
            if not cursor:
                raise WorkflowError("pull-request pagination had no end cursor")
            if cursor in seen_cursors:
                raise WorkflowError("pull-request pagination repeated an end cursor")
            seen_cursors.add(cursor)

        if expected_total != len({node["number"] for node in nodes}):
            raise WorkflowError(
                "open pull-request set changed or pagination was incomplete; rerun the selector"
            )
        return nodes

    def selection_is_current(
        self,
        *,
        repository: str,
        default_branch: str,
        universe_label: str,
        work_label: str,
        leaf_label: str,
        gate_label: str,
        issue: WorkIssue,
        expected_closing_pull_requests: Sequence[ClosingPullRequest] = (),
    ) -> bool:
        """Check that an issue's selection inputs and expected coverage are current."""
        owner, name = repository.split("/", 1)
        payload = self._graphql(
            ISSUE_GUARD_QUERY,
            owner=owner,
            name=name,
            issue_number=issue.number,
        )
        repo_data = payload.get("data", {}).get("repository")
        if repo_data is None:
            raise WorkflowError(
                f"repository {repository!r} was not found or is inaccessible"
            )

        live_default_branch = (repo_data.get("defaultBranchRef") or {}).get("name")
        if live_default_branch != default_branch:
            raise WorkflowError(
                "repository default branch changed after workflow selection"
            )

        raw_issue = repo_data.get("issue")
        if raw_issue is None:
            raise WorkflowError(
                f"selected issue #{issue.number} is no longer accessible"
            )
        issue_repository = (raw_issue.get("repository") or {}).get("nameWithOwner")
        if (
            raw_issue.get("id") != issue.node_id
            or raw_issue.get("number") != issue.number
            or issue_repository != repository
        ):
            raise WorkflowError(
                f"selected issue #{issue.number} changed identity or repository"
            )

        labels = self._validate_connection(
            raw_issue["labels"],
            subject=f"labels for selected issue #{issue.number}",
        )
        label_names = {label["name"] for label in labels}
        expected_kind_label = leaf_label if issue.kind is WorkKind.LEAF else gate_label
        unexpected_kind_label = (
            gate_label if issue.kind is WorkKind.LEAF else leaf_label
        )
        expected_priority_label = f"P{issue.priority}"
        priority_labels = {
            label for label in label_names if PRIORITY_LABEL_PATTERN.fullmatch(label)
        }
        if (
            universe_label not in label_names
            or work_label not in label_names
            or expected_kind_label not in label_names
            or unexpected_kind_label in label_names
            or priority_labels != {expected_priority_label}
        ):
            raise WorkflowError(
                f"workflow taxonomy changed for selected issue #{issue.number}"
            )

        if (
            raw_issue.get("state") != "OPEN"
            or raw_issue.get("updatedAt") != issue.updated_at
        ):
            return False

        live_blockers = self._validate_connection(
            raw_issue["blockedBy"],
            subject=f"native blockers for selected issue #{issue.number}",
        )
        live_sub_issues = self._validate_connection(
            raw_issue["subIssues"],
            subject=f"native sub-issues for selected issue #{issue.number}",
        )
        if not _requirements_unchanged(issue.blockers, live_blockers, repository):
            return False
        if not _requirements_unchanged(issue.sub_issues, live_sub_issues, repository):
            return False

        pull_requests = self._validate_connection(
            raw_issue["closedByPullRequestsReferences"],
            subject=f"closing pull-request references for selected issue #{issue.number}",
        )
        live_closing_pull_request_numbers: set[int] = set()
        for pull_request in pull_requests:
            closing_issues = self._validate_connection(
                pull_request["closingIssuesReferences"],
                subject=f"closing issue references for PR #{pull_request['number']}",
            )
            base_repository = (pull_request.get("baseRepository") or {}).get(
                "nameWithOwner"
            )
            if (
                pull_request.get("state") == "OPEN"
                and base_repository == repository
                and pull_request.get("baseRefName") == default_branch
                and any(
                    closing_issue.get("id") == issue.node_id
                    and (closing_issue.get("repository") or {}).get("nameWithOwner")
                    == repository
                    for closing_issue in closing_issues
                )
            ):
                live_closing_pull_request_numbers.add(pull_request["number"])

        expected_closing_pull_request_numbers = {
            pull_request.number for pull_request in expected_closing_pull_requests
        }
        return (
            live_closing_pull_request_numbers == expected_closing_pull_request_numbers
        )


def _requirements_unchanged(
    expected: Sequence[Requirement],
    raw_requirements: Sequence[dict[str, Any]],
    repository: str,
) -> bool:
    expected_state = {
        requirement.node_id: (requirement.state, requirement.repository)
        for requirement in expected
    }
    live_state = {
        requirement["id"]: (
            requirement["state"],
            (requirement.get("repository") or {}).get("nameWithOwner"),
        )
        for requirement in raw_requirements
    }
    return expected_state == live_state and all(
        requirement_repository == repository
        for _, requirement_repository in live_state.values()
    )


def collect_closing_pull_requests(
    pull_requests: Sequence[dict[str, Any]],
    *,
    repository: str,
    default_branch: str,
) -> dict[str, tuple[ClosingPullRequest, ...]]:
    """Index open default-branch PRs by the issue node IDs they close."""
    indexed: dict[str, list[ClosingPullRequest]] = {}

    for pull_request in pull_requests:
        if pull_request.get("state") != "OPEN":
            continue
        base_repository = (pull_request.get("baseRepository") or {}).get(
            "nameWithOwner"
        )
        if (
            base_repository != repository
            or pull_request.get("baseRefName") != default_branch
        ):
            continue

        connection = pull_request["closingIssuesReferences"]
        references = connection["nodes"]
        if connection["totalCount"] != len(references):
            raise WorkflowError(
                f"closing issue references for PR #{pull_request['number']} were truncated"
            )

        closing_pr = ClosingPullRequest(
            number=pull_request["number"],
            title=pull_request["title"],
            url=pull_request["url"],
            is_draft=bool(pull_request["isDraft"]),
        )
        for issue in references:
            issue_repository = (issue.get("repository") or {}).get("nameWithOwner")
            if issue_repository != repository:
                continue
            indexed.setdefault(issue["id"], []).append(closing_pr)

    return {
        node_id: tuple(
            sorted(issue_pull_requests, key=lambda pull_request: pull_request.number)
        )
        for node_id, issue_pull_requests in indexed.items()
    }


def validate_workflow_membership(
    raw_issues: Sequence[dict[str, Any]],
    raw_work_members: Sequence[dict[str, Any]],
    *,
    universe_label: str,
    work_label: str,
) -> None:
    """Require the canonical universe and workflow membership cohorts to match."""
    universe_by_id = {issue["id"]: issue["number"] for issue in raw_issues}
    work_by_id = {issue["id"]: issue["number"] for issue in raw_work_members}
    universe_ids = set(universe_by_id)
    work_ids = set(work_by_id)
    if universe_ids == work_ids:
        return

    differences: list[str] = []
    missing_work = sorted(
        universe_by_id[node_id] for node_id in universe_ids - work_ids
    )
    outside_universe = sorted(
        work_by_id[node_id] for node_id in work_ids - universe_ids
    )
    if missing_work:
        differences.append(
            f"missing {work_label!r}: "
            + ", ".join(f"#{number}" for number in missing_work[:10])
        )
    if outside_universe:
        differences.append(
            f"missing {universe_label!r}: "
            + ", ".join(f"#{number}" for number in outside_universe[:10])
        )
    raise WorkflowError(
        "canonical universe and workflow membership labels select different issues ("
        + "; ".join(differences)
        + ")"
    )


def _normalize_requirements(
    raw_connection: dict[str, Any],
    *,
    subject: str,
    repository: str,
    universe_node_ids: set[str],
    require_universe_membership: bool,
) -> tuple[Requirement, ...]:
    raw_requirements = raw_connection["nodes"]
    if raw_connection["totalCount"] != len(raw_requirements):
        raise WorkflowError(f"{subject} were truncated")

    normalized: list[Requirement] = []
    for requirement in raw_requirements:
        requirement_repository = (requirement.get("repository") or {}).get(
            "nameWithOwner"
        )
        if not requirement_repository:
            raise WorkflowError(
                f"{subject} include #{requirement['number']} with no accessible repository"
            )
        if requirement_repository != repository:
            raise WorkflowError(
                f"{subject} include unsupported cross-repository issue "
                f"{requirement_repository}#{requirement['number']}"
            )
        if require_universe_membership and requirement["id"] not in universe_node_ids:
            raise WorkflowError(
                f"{subject} include #{requirement['number']} outside the workflow universe"
            )
        if (
            not require_universe_membership
            and requirement["state"] == "OPEN"
            and requirement["id"] not in universe_node_ids
        ):
            raise WorkflowError(
                f"{subject} include open issue #{requirement['number']} "
                "outside the workflow universe"
            )
        normalized.append(
            Requirement(
                node_id=requirement["id"],
                number=requirement["number"],
                title=requirement["title"],
                url=requirement["url"],
                state=requirement["state"],
                repository=requirement_repository,
            )
        )
    return tuple(sorted(normalized, key=lambda requirement: requirement.number))


def normalize_work_issues(
    raw_issues: Sequence[dict[str, Any]],
    *,
    repository: str,
    work_label: str,
    leaf_label: str,
    gate_label: str,
    closing_pull_requests: dict[str, tuple[ClosingPullRequest, ...]],
) -> list[WorkIssue]:
    """Validate workflow taxonomy and normalize GraphQL issue records."""
    if len({work_label, leaf_label, gate_label}) != 3:
        raise WorkflowError("work, leaf, and gate labels must be distinct")
    priority_shaped_role_labels = sorted(
        label
        for label in (work_label, leaf_label, gate_label)
        if PRIORITY_LABEL_PATTERN.fullmatch(label)
    )
    if priority_shaped_role_labels:
        labels = ", ".join(repr(label) for label in priority_shaped_role_labels)
        raise WorkflowError(
            "work, leaf, and gate labels must not use the P<number> priority "
            f"namespace: {labels}"
        )

    normalized: list[WorkIssue] = []
    universe_node_ids = {raw_issue["id"] for raw_issue in raw_issues}
    open_universe_node_ids = {
        raw_issue["id"] for raw_issue in raw_issues if raw_issue["state"] == "OPEN"
    }
    universe_numbers = {
        raw_issue["id"]: raw_issue["number"] for raw_issue in raw_issues
    }

    pull_request_targets: dict[int, set[str]] = {}
    for node_id in open_universe_node_ids:
        issue_pull_requests = closing_pull_requests.get(node_id, ())
        if len(issue_pull_requests) > 1:
            raise WorkflowError(
                f"issue #{universe_numbers[node_id]} has multiple open closing pull requests"
            )
        for pull_request in issue_pull_requests:
            pull_request_targets.setdefault(pull_request.number, set()).add(node_id)
    for pull_request_number, target_ids in pull_request_targets.items():
        if len(target_ids) > 1:
            issue_numbers = sorted(universe_numbers[node_id] for node_id in target_ids)
            raise WorkflowError(
                f"PR #{pull_request_number} closes multiple workflow issues: {issue_numbers}"
            )

    for raw_issue in raw_issues:
        labels_connection = raw_issue["labels"]
        raw_labels = labels_connection["nodes"]
        if labels_connection["totalCount"] != len(raw_labels):
            raise WorkflowError(
                f"labels for issue #{raw_issue['number']} were truncated"
            )
        labels = {label["name"] for label in raw_labels}

        if work_label not in labels:
            raise WorkflowError(
                f"issue #{raw_issue['number']} is missing {work_label!r}"
            )

        kind_labels = labels & {leaf_label, gate_label}
        if kind_labels == {leaf_label}:
            kind = WorkKind.LEAF
        elif kind_labels == {gate_label}:
            kind = WorkKind.GATE
        else:
            raise WorkflowError(
                f"issue #{raw_issue['number']} must carry exactly one of "
                f"{leaf_label!r} and {gate_label!r}"
            )

        priority_labels = {
            label for label in labels if PRIORITY_LABEL_PATTERN.fullmatch(label)
        }
        if len(priority_labels) != 1 or not priority_labels <= PRIORITY_LABELS.keys():
            raise WorkflowError(
                f"issue #{raw_issue['number']} must carry exactly one P0..P3 label"
            )
        priority_label = next(iter(priority_labels))

        blockers = _normalize_requirements(
            raw_issue["blockedBy"],
            subject=f"native blockers for issue #{raw_issue['number']}",
            repository=repository,
            universe_node_ids=universe_node_ids,
            require_universe_membership=False,
        )
        sub_issues = _normalize_requirements(
            raw_issue["subIssues"],
            subject=f"native sub-issues for issue #{raw_issue['number']}",
            repository=repository,
            universe_node_ids=universe_node_ids,
            require_universe_membership=True,
        )

        if kind is WorkKind.LEAF and sub_issues:
            raise WorkflowError(
                f"leaf issue #{raw_issue['number']} has native sub-issues; "
                "classify it as a gate or remove the children"
            )
        if kind is WorkKind.GATE and not blockers and not sub_issues:
            raise WorkflowError(
                f"gate issue #{raw_issue['number']} has no native blockers or sub-issues"
            )

        node_id = raw_issue["id"]
        normalized.append(
            WorkIssue(
                node_id=node_id,
                number=raw_issue["number"],
                title=raw_issue["title"],
                url=raw_issue["url"],
                state=raw_issue["state"],
                updated_at=raw_issue["updatedAt"],
                priority=PRIORITY_LABELS[priority_label],
                kind=kind,
                blockers=blockers,
                sub_issues=sub_issues,
                closing_pull_requests=closing_pull_requests.get(node_id, ()),
            )
        )

    return normalized


def _leaf_blockers_satisfied(
    issue: WorkIssue,
    *,
    sequenced_ids: set[str],
    open_issues_by_id: dict[str, WorkIssue],
) -> bool:
    return all(
        blocker.landed
        or (
            blocker.node_id in sequenced_ids
            and blocker.node_id in open_issues_by_id
            and open_issues_by_id[blocker.node_id].kind is WorkKind.LEAF
        )
        for blocker in issue.blockers
    )


def _gate_requirements_landed(issue: WorkIssue) -> bool:
    return all(requirement.landed for requirement in issue.gate_requirements)


def _sequenced_covered_ids(open_issues: Sequence[WorkIssue]) -> set[str]:
    """Return covered issues whose prerequisite sequence is currently valid."""
    issues_by_id = {issue.node_id: issue for issue in open_issues}
    claimed_ids = {
        issue.node_id for issue in open_issues if issue.closing_pull_requests
    }

    # A closing PR attached to blocked work is a claim, but it must not
    # transitively unlock downstream work until its prerequisite sequence is
    # valid. Compute the valid covered closure from roots outward.
    sequenced_covered_ids: set[str] = set()
    while True:
        newly_sequenced = {
            issue.node_id
            for issue in open_issues
            if issue.node_id in claimed_ids
            and issue.node_id not in sequenced_covered_ids
            and (
                (
                    issue.kind is WorkKind.LEAF
                    and _leaf_blockers_satisfied(
                        issue,
                        sequenced_ids=sequenced_covered_ids,
                        open_issues_by_id=issues_by_id,
                    )
                )
                or (issue.kind is WorkKind.GATE and _gate_requirements_landed(issue))
            )
        }
        if not newly_sequenced:
            break
        sequenced_covered_ids.update(newly_sequenced)

    return sequenced_covered_ids


def _coverage_proof(
    selected_issue: WorkIssue,
    *,
    open_issues: Sequence[WorkIssue],
    sequenced_covered_ids: set[str],
) -> tuple[WorkIssue, ...]:
    """Return the covered leaf ancestry that makes a selected leaf ready."""
    if selected_issue.kind is not WorkKind.LEAF:
        return ()

    open_issues_by_id = {issue.node_id: issue for issue in open_issues}
    proof_by_id: dict[str, WorkIssue] = {}
    pending = [selected_issue]
    while pending:
        issue = pending.pop()
        for blocker in issue.blockers:
            if blocker.landed:
                continue
            prerequisite = open_issues_by_id.get(blocker.node_id)
            if (
                prerequisite is None
                or prerequisite.kind is not WorkKind.LEAF
                or prerequisite.node_id not in sequenced_covered_ids
                or not prerequisite.closing_pull_requests
            ):
                raise WorkflowError(
                    f"selection proof for issue #{selected_issue.number} became "
                    f"inconsistent at blocker #{blocker.number}"
                )
            if prerequisite.node_id in proof_by_id:
                continue
            proof_by_id[prerequisite.node_id] = prerequisite
            pending.append(prerequisite)

    return tuple(sorted(proof_by_id.values(), key=lambda issue: issue.number))


def _selection_coverage_proof(
    selection: Selection,
    issues: Sequence[WorkIssue],
) -> tuple[WorkIssue, ...]:
    if selection.issue is None:
        return ()
    open_issues = [issue for issue in issues if issue.state == "OPEN"]
    return _coverage_proof(
        selection.issue,
        open_issues=open_issues,
        sequenced_covered_ids=_sequenced_covered_ids(open_issues),
    )


def select_next(
    issues: Sequence[WorkIssue],
    *,
    work_label: str = DEFAULT_WORK_LABEL,
    excluded_numbers: frozenset[int] = frozenset(),
) -> Selection:
    """Select the next ready issue from already-normalized workflow state.

    An open closing PR covers its own issue. For leaves, a covered leaf blocker
    is sufficient to continue a planned sequence of PRs. Gates are stricter:
    every blocker and native sub-issue must be actually closed before aggregate
    validation or an externally consequential release action can begin.
    """
    open_issues = [issue for issue in issues if issue.state == "OPEN"]
    uncovered = [issue for issue in open_issues if not issue.covered]
    claimed_ids = {
        issue.node_id for issue in open_issues if issue.closing_pull_requests
    }
    open_issues_by_id = {issue.node_id: issue for issue in open_issues}
    sequenced_covered_ids = _sequenced_covered_ids(open_issues)

    if not uncovered:
        if sequenced_covered_ids != claimed_ids:
            return Selection(
                status=SelectionStatus.WAITING,
                message=(
                    f"No issue labeled {work_label} is currently actionable; "
                    "one or more closing pull requests cover issues whose prerequisites "
                    "are not yet satisfied."
                ),
                issue=None,
                open_count=len(open_issues),
                covered_count=len(claimed_ids),
                ready_count=0,
            )
        return Selection(
            status=SelectionStatus.COMPLETE,
            message=(
                f"All issues labeled {work_label} are complete or covered by an open "
                "pull request that will close them."
            ),
            issue=None,
            open_count=len(open_issues),
            covered_count=len(open_issues),
            ready_count=0,
        )

    ready_before_exclusions: list[WorkIssue] = []
    for issue in uncovered:
        if issue.kind is WorkKind.GATE:
            requirements_satisfied = _gate_requirements_landed(issue)
        else:
            requirements_satisfied = _leaf_blockers_satisfied(
                issue,
                sequenced_ids=sequenced_covered_ids,
                open_issues_by_id=open_issues_by_id,
            )
        if requirements_satisfied:
            ready_before_exclusions.append(issue)

    ready = [
        issue
        for issue in ready_before_exclusions
        if issue.number not in excluded_numbers
    ]

    if ready:
        selected = min(
            ready,
            key=lambda issue: (
                issue.priority,
                0 if issue.kind is WorkKind.LEAF else 1,
                issue.number,
            ),
        )
        return Selection(
            status=SelectionStatus.SELECTED,
            message=selected.url,
            issue=selected,
            open_count=len(open_issues),
            covered_count=len(claimed_ids),
            ready_count=len(ready),
        )

    if ready_before_exclusions:
        excluded = ", ".join(
            f"#{number}"
            for number in sorted(
                issue.number
                for issue in ready_before_exclusions
                if issue.number in excluded_numbers
            )
        )
        return Selection(
            status=SelectionStatus.WAITING,
            message=(
                f"No issue labeled {work_label} is currently actionable because "
                f"ready issue(s) {excluded} were excluded."
            ),
            issue=None,
            open_count=len(open_issues),
            covered_count=len(claimed_ids),
            ready_count=0,
        )

    return Selection(
        status=SelectionStatus.WAITING,
        message=(
            f"No issue labeled {work_label} is currently actionable; "
            f"{len(uncovered)} uncovered issue(s) are waiting for prerequisite "
            "issues or pull requests to merge."
        ),
        issue=None,
        open_count=len(open_issues),
        covered_count=len(claimed_ids),
        ready_count=0,
    )


def get_selection(
    *,
    client: GitHubClient,
    repository: str | None,
    universe_label: str,
    work_label: str,
    leaf_label: str,
    gate_label: str,
    excluded_numbers: frozenset[int] = frozenset(),
) -> Selection:
    """Fetch live GitHub state and produce a stable selection."""
    resolved_repository = client.resolve_repository(repository)
    previous_candidate: SelectionCandidate | None = None
    stale_selection_retries = 0

    # A selected issue must survive two consecutive full dependency snapshots
    # and both targeted freshness guards. Four snapshots cover the longest
    # permitted stabilization sequence when one guard detects stale state.
    for _snapshot_number in range(4):
        default_branch, raw_issues = client.fetch_work_universe(
            resolved_repository, universe_label
        )
        raw_work_members = (
            raw_issues
            if work_label == universe_label
            else client.fetch_work_membership(resolved_repository, work_label)
        )
        validate_workflow_membership(
            raw_issues,
            raw_work_members,
            universe_label=universe_label,
            work_label=work_label,
        )
        raw_pull_requests = client.fetch_open_pull_requests(resolved_repository)
        closing_pull_requests = collect_closing_pull_requests(
            raw_pull_requests,
            repository=resolved_repository,
            default_branch=default_branch,
        )
        issues = normalize_work_issues(
            raw_issues,
            repository=resolved_repository,
            work_label=work_label,
            leaf_label=leaf_label,
            gate_label=gate_label,
            closing_pull_requests=closing_pull_requests,
        )
        selection = select_next(
            issues,
            work_label=work_label,
            excluded_numbers=excluded_numbers,
        )

        if selection.status is SelectionStatus.COMPLETE:
            candidate: SelectionCandidate = (SelectionStatus.COMPLETE, None, ())
            if previous_candidate == candidate:
                return selection
            previous_candidate = candidate
            continue

        if selection.issue is None:
            return selection
        coverage_proof = _selection_coverage_proof(selection, issues)
        proof_signature = tuple(
            (
                proof_issue.node_id,
                tuple(
                    pull_request.number
                    for pull_request in proof_issue.closing_pull_requests
                ),
            )
            for proof_issue in coverage_proof
        )
        selection_is_current = client.selection_is_current(
            repository=resolved_repository,
            default_branch=default_branch,
            universe_label=universe_label,
            work_label=work_label,
            leaf_label=leaf_label,
            gate_label=gate_label,
            issue=selection.issue,
            expected_closing_pull_requests=(),
        )
        for proof_issue in coverage_proof:
            if not selection_is_current:
                break
            selection_is_current = client.selection_is_current(
                repository=resolved_repository,
                default_branch=default_branch,
                universe_label=universe_label,
                work_label=work_label,
                leaf_label=leaf_label,
                gate_label=gate_label,
                issue=proof_issue,
                expected_closing_pull_requests=proof_issue.closing_pull_requests,
            )
        if selection_is_current:
            candidate = (
                SelectionStatus.SELECTED,
                selection.issue.node_id,
                proof_signature,
            )
            if previous_candidate == candidate:
                return selection
            previous_candidate = candidate
            continue

        previous_candidate = None
        stale_selection_retries += 1
        if stale_selection_retries > 1:
            raise WorkflowError(
                "GitHub issue state changed repeatedly during selection; rerun"
            )

    raise WorkflowError("GitHub issue state did not stabilize during selection; rerun")


def build_argument_parser() -> argparse.ArgumentParser:
    """Build the command-line parser."""
    parser = argparse.ArgumentParser(
        description="Select the next production-readiness issue."
    )
    parser.add_argument(
        "--repo",
        dest="repository",
        help="GitHub repository in owner/name form (default: current repo).",
    )
    parser.add_argument(
        "--universe-label",
        default=DEFAULT_UNIVERSE_LABEL,
        help="Canonical label defining every issue that must remain visible.",
    )
    parser.add_argument(
        "--work-label",
        default=DEFAULT_WORK_LABEL,
        help="Redundant label selecting the workflow issue universe.",
    )
    parser.add_argument(
        "--leaf-label",
        default=DEFAULT_LEAF_LABEL,
        help="Label identifying independently actionable leaves.",
    )
    parser.add_argument(
        "--gate-label",
        default=DEFAULT_GATE_LABEL,
        help="Label identifying epics, audits, and release/program gates.",
    )
    parser.add_argument(
        "--json",
        dest="output_json",
        action="store_true",
        help="Emit a machine-readable selection record.",
    )
    parser.add_argument(
        "--exclude",
        action="append",
        type=int,
        default=[],
        metavar="NUMBER",
        help="Issue number to skip for this invocation; may be repeated.",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Print one issue URL, a completion message, or a waiting message."""
    args = build_argument_parser().parse_args(argv)
    try:
        selection = get_selection(
            client=GitHubClient(),
            repository=args.repository,
            universe_label=args.universe_label,
            work_label=args.work_label,
            leaf_label=args.leaf_label,
            gate_label=args.gate_label,
            excluded_numbers=frozenset(args.exclude),
        )
    except WorkflowError as error:
        detail = " ".join(str(error).splitlines())
        print(f"error: {detail}", file=sys.stderr)
        return 1

    print(selection.as_json() if args.output_json else selection.message)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
