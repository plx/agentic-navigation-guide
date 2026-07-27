#!/usr/bin/env python3
"""Audit the public GitHub controls required by production-readiness issue #65."""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_EXPECTED_DIR = ROOT / ".github" / "repository-protections"
DEFAULT_REPOSITORY = "plx/agentic-navigation-guide"
DEFAULT_API_URL = "https://api.github.com"
GITHUB_ACTIONS_APP_ID = 15368
PUBLICATION_SECRET_MARKERS = (
    "CARGO_REGISTRY",
    "CRATES",
    "PUBLISH",
    "RELEASE_TOKEN",
)


class AuditError(RuntimeError):
    """Raised when live state cannot be collected or does not match policy."""


@dataclass
class AuditResult:
    failures: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.failures

    def require(self, condition: bool, message: str) -> None:
        if not condition:
            self.failures.append(message)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise AuditError(f"cannot load {path}: {error}") from error


def expected_configuration(expected_dir: Path) -> dict[str, Any]:
    return {
        "main": load_json(expected_dir / "main-ruleset.json"),
        "tag_creation": load_json(
            expected_dir / "release-tag-creation-ruleset.json"
        ),
        "tag_immutability": load_json(
            expected_dir / "release-tag-immutability-ruleset.json"
        ),
        "environment": load_json(expected_dir / "release-environment.json"),
        "environment_policy": load_json(
            expected_dir / "release-environment-tag-policy.json"
        ),
    }


def _normalize_rule(rule: dict[str, Any]) -> dict[str, Any]:
    normalized = json.loads(json.dumps(rule))
    parameters = normalized.get("parameters", {})
    if "allowed_merge_methods" in parameters:
        parameters["allowed_merge_methods"] = sorted(
            parameters["allowed_merge_methods"]
        )
    if "required_status_checks" in parameters:
        parameters["required_status_checks"] = sorted(
            parameters["required_status_checks"],
            key=lambda check: (
                check.get("context", ""),
                check.get("integration_id", -1),
            ),
        )
    return normalized


def _normalize_bypass_actor(actor: dict[str, Any]) -> dict[str, Any]:
    return {
        "actor_id": actor.get("actor_id"),
        "actor_type": actor.get("actor_type"),
        "bypass_mode": actor.get("bypass_mode", "always"),
    }


def _assert_subset(
    result: AuditResult, actual: Any, expected: Any, path: str
) -> None:
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            result.failures.append(f"{path} is not an object")
            return
        for key, value in expected.items():
            if key not in actual:
                result.failures.append(f"{path}.{key} is missing")
                continue
            _assert_subset(result, actual[key], value, f"{path}.{key}")
        return
    if actual != expected:
        result.failures.append(
            f"{path} differs: expected {expected!r}, observed {actual!r}"
        )


def audit_ruleset(
    result: AuditResult,
    actual: dict[str, Any],
    expected: dict[str, Any],
    *,
    require_admin_visibility: bool,
) -> None:
    label = f"ruleset {expected['name']!r}"
    for key in ("name", "target", "enforcement", "conditions"):
        _assert_subset(result, actual.get(key), expected[key], f"{label}.{key}")

    actual_rules = {
        rule.get("type"): _normalize_rule(rule)
        for rule in actual.get("rules", [])
    }
    expected_rules = {
        rule["type"]: _normalize_rule(rule) for rule in expected["rules"]
    }
    result.require(
        set(actual_rules) == set(expected_rules),
        f"{label}.rules types differ: expected {sorted(expected_rules)}, "
        f"observed {sorted(actual_rules)}",
    )
    for rule_type, expected_rule in expected_rules.items():
        if rule_type in actual_rules:
            _assert_subset(
                result,
                actual_rules[rule_type],
                expected_rule,
                f"{label}.rules[{rule_type}]",
            )

    if "bypass_actors" not in actual:
        message = (
            f"{label}.bypass_actors is hidden from this token; rerun with "
            "--require-admin-visibility for a release audit"
        )
        if require_admin_visibility:
            result.failures.append(message)
        else:
            result.warnings.append(message)
        return

    actual_actors = sorted(
        (_normalize_bypass_actor(actor) for actor in actual["bypass_actors"]),
        key=lambda actor: (
            actor["actor_type"] or "",
            actor["actor_id"] or -1,
            actor["bypass_mode"] or "",
        ),
    )
    expected_actors = sorted(
        (
            _normalize_bypass_actor(actor)
            for actor in expected.get("bypass_actors", [])
        ),
        key=lambda actor: (
            actor["actor_type"] or "",
            actor["actor_id"] or -1,
            actor["bypass_mode"] or "",
        ),
    )
    result.require(
        actual_actors == expected_actors,
        f"{label}.bypass_actors differs: expected {expected_actors!r}, "
        f"observed {actual_actors!r}",
    )


def audit_environment(
    result: AuditResult,
    actual: dict[str, Any],
    policies: list[dict[str, Any]],
    expected: dict[str, Any],
    expected_policy: dict[str, Any],
) -> None:
    update = expected["update"]
    response = expected["expected_response"]
    for key, value in response.items():
        _assert_subset(result, actual.get(key), value, f"environment.{key}")
    _assert_subset(
        result,
        actual.get("deployment_branch_policy"),
        update["deployment_branch_policy"],
        "environment.deployment_branch_policy",
    )

    protection_rules = actual.get("protection_rules", [])
    reviewer_rules = [
        rule for rule in protection_rules if rule.get("type") == "required_reviewers"
    ]
    result.require(
        len(reviewer_rules) == 1,
        "environment must have exactly one required-reviewers rule",
    )
    if len(reviewer_rules) == 1:
        reviewer_rule = reviewer_rules[0]
        result.require(
            reviewer_rule.get("prevent_self_review")
            == update["prevent_self_review"],
            "environment.prevent_self_review differs",
        )
        observed_reviewers = sorted(
            (
                reviewer.get("type"),
                reviewer.get("reviewer", {}).get("id"),
            )
            for reviewer in reviewer_rule.get("reviewers", [])
        )
        expected_reviewers = sorted(
            (reviewer["type"], reviewer["id"])
            for reviewer in update["reviewers"]
        )
        result.require(
            observed_reviewers == expected_reviewers,
            "environment reviewers differ: "
            f"expected {expected_reviewers!r}, observed {observed_reviewers!r}",
        )

    wait_rules = [
        rule for rule in protection_rules if rule.get("type") == "wait_timer"
    ]
    expected_wait = update["wait_timer"]
    if expected_wait == 0:
        result.require(not wait_rules, "environment has an unexpected wait timer")
    else:
        result.require(
            len(wait_rules) == 1
            and wait_rules[0].get("wait_timer") == expected_wait,
            f"environment wait timer is not {expected_wait}",
        )

    observed_policies = sorted(
        (
            {"name": policy.get("name"), "type": policy.get("type")}
            for policy in policies
        ),
        key=lambda policy: (policy["type"] or "", policy["name"] or ""),
    )
    result.require(
        observed_policies == [expected_policy],
        "release environment deployment policies differ: "
        f"expected {[expected_policy]!r}, observed {observed_policies!r}",
    )


def audit_publication_secrets(
    result: AuditResult,
    repository_secret_names: list[str],
    environment_secret_names: list[str],
) -> None:
    result.require(
        not environment_secret_names,
        "release environment must contain no secrets before issue #63 "
        f"configures Trusted Publishing; observed names {environment_secret_names!r}",
    )
    publication_names = sorted(
        name
        for name in repository_secret_names
        if any(marker in name.upper() for marker in PUBLICATION_SECRET_MARKERS)
    )
    result.require(
        not publication_names,
        "repository-level publication-like secrets are forbidden; "
        f"observed names {publication_names!r}",
    )


def validate_snapshot(
    snapshot: dict[str, Any],
    expected: dict[str, Any],
    *,
    require_admin_visibility: bool,
) -> AuditResult:
    result = AuditResult()
    for key in ("main", "tag_creation", "tag_immutability"):
        audit_ruleset(
            result,
            snapshot["rulesets"][key],
            expected[key],
            require_admin_visibility=require_admin_visibility,
        )
    audit_environment(
        result,
        snapshot["environment"],
        snapshot["environment_policies"],
        expected["environment"],
        expected["environment_policy"],
    )
    if require_admin_visibility:
        audit_publication_secrets(
            result,
            snapshot["repository_secret_names"],
            snapshot["environment_secret_names"],
        )
    return result


class GitHubClient:
    def __init__(self, api_url: str, token: str | None) -> None:
        self.api_url = api_url.rstrip("/")
        self.token = token

    def get(self, path: str) -> Any:
        headers = {
            "Accept": "application/vnd.github+json",
            "User-Agent": "agentic-navigation-guide-protection-audit",
            "X-GitHub-Api-Version": "2026-03-10",
        }
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        request = urllib.request.Request(f"{self.api_url}{path}", headers=headers)
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            try:
                message = json.load(error).get("message", error.reason)
            except (json.JSONDecodeError, AttributeError):
                message = error.reason
            raise AuditError(
                f"GitHub API GET {path} failed with HTTP {error.code}: {message}"
            ) from error
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as error:
            raise AuditError(f"GitHub API GET {path} failed: {error}") from error


def collect_snapshot(
    client: GitHubClient,
    repository: str,
    expected: dict[str, Any],
    *,
    require_admin_visibility: bool,
) -> dict[str, Any]:
    base = f"/repos/{repository}"
    summaries = client.get(f"{base}/rulesets?per_page=100")
    by_name = {ruleset["name"]: ruleset for ruleset in summaries}
    rulesets: dict[str, Any] = {}
    for key in ("main", "tag_creation", "tag_immutability"):
        name = expected[key]["name"]
        if name not in by_name:
            raise AuditError(f"required ruleset {name!r} does not exist")
        rulesets[key] = client.get(f"{base}/rulesets/{by_name[name]['id']}")

    environment_name = urllib.parse.quote(
        expected["environment"]["expected_response"]["name"], safe=""
    )
    environment = client.get(f"{base}/environments/{environment_name}")
    policy_page = client.get(
        f"{base}/environments/{environment_name}/deployment-branch-policies"
        "?per_page=100"
    )
    snapshot: dict[str, Any] = {
        "rulesets": rulesets,
        "environment": environment,
        "environment_policies": policy_page["branch_policies"],
    }
    if require_admin_visibility:
        repository_secrets = client.get(f"{base}/actions/secrets?per_page=100")
        environment_secrets = client.get(
            f"{base}/environments/{environment_name}/secrets?per_page=100"
        )
        snapshot["repository_secret_names"] = [
            secret["name"] for secret in repository_secrets["secrets"]
        ]
        snapshot["environment_secret_names"] = [
            secret["name"] for secret in environment_secrets["secrets"]
        ]
    return snapshot


def report_document(
    repository: str,
    result: AuditResult,
    snapshot: dict[str, Any],
    *,
    require_admin_visibility: bool,
) -> dict[str, Any]:
    rulesets = snapshot["rulesets"]
    reviewer_rule = next(
        (
            rule
            for rule in snapshot["environment"].get("protection_rules", [])
            if rule.get("type") == "required_reviewers"
        ),
        {},
    )
    return {
        "schema_version": 1,
        "repository": repository,
        "checked_at": datetime.now(timezone.utc).isoformat(),
        "mode": (
            "admin-visible" if require_admin_visibility else "public-read-only"
        ),
        "ok": result.ok,
        "failures": result.failures,
        "warnings": result.warnings,
        "observed": {
            "rulesets": {
                key: {
                    "id": value.get("id"),
                    "name": value.get("name"),
                    "target": value.get("target"),
                    "enforcement": value.get("enforcement"),
                    "rule_types": sorted(
                        rule.get("type") for rule in value.get("rules", [])
                    ),
                    "bypass_actors_visible": "bypass_actors" in value,
                }
                for key, value in rulesets.items()
            },
            "environment": {
                "id": snapshot["environment"].get("id"),
                "name": snapshot["environment"].get("name"),
                "can_admins_bypass": snapshot["environment"].get(
                    "can_admins_bypass"
                ),
                "prevent_self_review": reviewer_rule.get("prevent_self_review"),
                "reviewer_ids": sorted(
                    reviewer.get("reviewer", {}).get("id")
                    for reviewer in reviewer_rule.get("reviewers", [])
                ),
                "deployment_branch_policy": snapshot["environment"].get(
                    "deployment_branch_policy"
                ),
                "deployment_policies": sorted(
                    (
                        {
                            "name": policy.get("name"),
                            "type": policy.get("type"),
                        }
                        for policy in snapshot["environment_policies"]
                    ),
                    key=lambda policy: (
                        policy["type"] or "",
                        policy["name"] or "",
                    ),
                ),
            },
            "publication_like_repository_secret_count": (
                sum(
                    any(marker in name.upper() for marker in PUBLICATION_SECRET_MARKERS)
                    for name in snapshot.get("repository_secret_names", [])
                )
                if require_admin_visibility
                else None
            ),
            "release_environment_secret_count": (
                len(snapshot.get("environment_secret_names", []))
                if require_admin_visibility
                else None
            ),
        },
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Audit repository rulesets and the release environment."
    )
    parser.add_argument("--repository", default=DEFAULT_REPOSITORY)
    parser.add_argument("--api-url", default=DEFAULT_API_URL)
    parser.add_argument(
        "--expected-dir", type=Path, default=DEFAULT_EXPECTED_DIR
    )
    parser.add_argument(
        "--require-admin-visibility",
        action="store_true",
        help="fail unless bypass actors and non-sensitive secret names are visible",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="write the non-sensitive JSON attestation to this path",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    try:
        expected = expected_configuration(args.expected_dir)
        token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
        if args.require_admin_visibility and not token:
            raise AuditError(
                "--require-admin-visibility requires GH_TOKEN or GITHUB_TOKEN"
            )
        snapshot = collect_snapshot(
            GitHubClient(args.api_url, token),
            args.repository,
            expected,
            require_admin_visibility=args.require_admin_visibility,
        )
        result = validate_snapshot(
            snapshot,
            expected,
            require_admin_visibility=args.require_admin_visibility,
        )
        report = report_document(
            args.repository,
            result,
            snapshot,
            require_admin_visibility=args.require_admin_visibility,
        )
    except (AuditError, KeyError, TypeError, ValueError) as error:
        print(f"repository protection audit failed: {error}", file=sys.stderr)
        return 1

    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0 if result.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
