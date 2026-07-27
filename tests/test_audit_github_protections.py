from __future__ import annotations

import copy
import importlib.util
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "audit_github_protections.py"
SPEC = importlib.util.spec_from_file_location(
    "audit_github_protections", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
audit = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = audit
SPEC.loader.exec_module(audit)


def ruleset(payload: dict) -> dict:
    value = copy.deepcopy(payload)
    value["id"] = hash(payload["name"]) & 0xFFFF
    return value


class RepositoryProtectionAuditTests(unittest.TestCase):
    def setUp(self) -> None:
        self.expected = audit.expected_configuration(
            ROOT / ".github" / "repository-protections"
        )
        environment_update = self.expected["environment"]["update"]
        reviewer = environment_update["reviewers"][0]
        self.snapshot = {
            "rulesets": {
                "main": ruleset(self.expected["main"]),
                "tag_creation": ruleset(self.expected["tag_creation"]),
                "tag_immutability": ruleset(
                    self.expected["tag_immutability"]
                ),
            },
            "environment": {
                "id": 123,
                "name": "release",
                "can_admins_bypass": False,
                "deployment_branch_policy": copy.deepcopy(
                    environment_update["deployment_branch_policy"]
                ),
                "protection_rules": [
                    {
                        "type": "required_reviewers",
                        "prevent_self_review": False,
                        "reviewers": [
                            {
                                "type": reviewer["type"],
                                "reviewer": {"id": reviewer["id"]},
                            }
                        ],
                    }
                ],
            },
            "environment_policies": [
                copy.deepcopy(self.expected["environment_policy"])
            ],
            "repository_secret_names": ["CLAUDE_CODE_OAUTH_TOKEN"],
            "environment_secret_names": [],
        }

    def validate(self, *, admin: bool = True):
        return audit.validate_snapshot(
            self.snapshot,
            self.expected,
            require_admin_visibility=admin,
        )

    def test_exact_admin_visible_snapshot_passes(self) -> None:
        result = self.validate()
        self.assertEqual(result.failures, [])
        self.assertEqual(result.warnings, [])

    def test_missing_required_check_fails_closed(self) -> None:
        status_rule = next(
            rule
            for rule in self.snapshot["rulesets"]["main"]["rules"]
            if rule["type"] == "required_status_checks"
        )
        status_rule["parameters"]["required_status_checks"].pop()
        result = self.validate()
        self.assertFalse(result.ok)
        self.assertTrue(
            any("required_status_checks" in failure for failure in result.failures)
        )

    def test_unexpected_main_bypass_fails(self) -> None:
        self.snapshot["rulesets"]["main"]["bypass_actors"].append(
            {
                "actor_id": 5,
                "actor_type": "RepositoryRole",
                "bypass_mode": "always",
            }
        )
        result = self.validate()
        self.assertFalse(result.ok)
        self.assertTrue(
            any("bypass_actors differs" in failure for failure in result.failures)
        )

    def test_public_audit_warns_when_bypass_visibility_is_absent(self) -> None:
        for value in self.snapshot["rulesets"].values():
            value.pop("bypass_actors")
        result = self.validate(admin=False)
        self.assertTrue(result.ok)
        self.assertEqual(len(result.warnings), 3)

    def test_wrong_release_ref_policy_fails(self) -> None:
        self.snapshot["environment_policies"][0]["type"] = "branch"
        result = self.validate()
        self.assertFalse(result.ok)
        self.assertTrue(
            any(
                "deployment policies differ" in failure
                for failure in result.failures
            )
        )

    def test_environment_admin_bypass_fails(self) -> None:
        self.snapshot["environment"]["can_admins_bypass"] = True
        result = self.validate()
        self.assertFalse(result.ok)
        self.assertTrue(
            any("can_admins_bypass" in failure for failure in result.failures)
        )

    def test_publication_like_repository_secret_fails(self) -> None:
        self.snapshot["repository_secret_names"].append(
            "CARGO_REGISTRY_TOKEN"
        )
        result = self.validate()
        self.assertFalse(result.ok)
        self.assertTrue(
            any("publication-like secrets" in failure for failure in result.failures)
        )

    def test_release_environment_secret_fails_before_issue_63(self) -> None:
        self.snapshot["environment_secret_names"].append("UNEXPECTED")
        result = self.validate()
        self.assertFalse(result.ok)
        self.assertTrue(
            any(
                "release environment must contain no secrets" in failure
                for failure in result.failures
            )
        )

    def test_non_sensitive_report_summarizes_without_secret_names(self) -> None:
        result = self.validate()
        report = audit.report_document(
            "plx/agentic-navigation-guide",
            result,
            self.snapshot,
            require_admin_visibility=True,
        )
        self.assertTrue(report["ok"])
        self.assertEqual(
            report["observed"]["release_environment_secret_count"], 0
        )
        self.assertEqual(
            report["observed"][
                "publication_like_repository_secret_count"
            ],
            0,
        )
        self.assertNotIn("CLAUDE_CODE_OAUTH_TOKEN", repr(report))


if __name__ == "__main__":
    unittest.main()
