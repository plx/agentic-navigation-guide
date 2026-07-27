from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check_contributor_templates.py"
SPEC = importlib.util.spec_from_file_location(
    "check_contributor_templates", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = checker
SPEC.loader.exec_module(checker)


def valid_form() -> str:
    return """\
name: "Test form"
description: "Exercise the constrained form grammar."
title: "[Test]: "
labels:
  - "testing"
body:
  - type: "markdown"
    attributes:
      value: "Read the instructions."
  - type: "textarea"
    id: "observed"
    attributes:
      label: "Observed behavior"
      description: "Describe what happened."
    validations:
      required: true
  - type: "dropdown"
    id: "platform"
    attributes:
      label: "Platform"
      description: "Select the operating system."
      options:
        - "Linux"
        - "macOS"
        - "Windows"
    validations:
      required: true
"""


def valid_pull_request_template() -> str:
    prompts = "\n".join(
        (
            "base commit SHA",
            "exact pre-fix command",
            "focused post-fix",
            "full post-fix",
            "documentation impact",
            "compatibility impact",
            "security impact",
            "dependency or license impact",
        )
    )
    sections = "\n\n".join(
        f"{heading}\n\n{prompts if index == 0 else 'Complete this section.'}"
        for index, heading in enumerate(checker.PR_HEADINGS)
    )
    return f"{sections}\n\nCloses #NUMBER\n"


class ContributorTemplateCheckerTests(unittest.TestCase):
    def test_checked_in_repository_artifacts_pass(self) -> None:
        checker.check_repository(ROOT)

    def test_valid_constrained_form_passes(self) -> None:
        form = checker.validate_issue_form(Path("form.yml"), valid_form())
        self.assertEqual(form.name, "Test form")
        self.assertEqual(
            [item.identifier for item in form.items],
            [None, "observed", "platform"],
        )

    def test_unquoted_scalar_fails(self) -> None:
        source = valid_form().replace(
            'name: "Test form"', "name: Test form", 1
        )
        with self.assertRaisesRegex(
            checker.TemplateContractError, "quoted string"
        ):
            checker.validate_issue_form(Path("form.yml"), source)

    def test_duplicate_id_fails(self) -> None:
        source = valid_form().replace(
            'id: "platform"', 'id: "observed"', 1
        )
        with self.assertRaisesRegex(
            checker.TemplateContractError, "duplicate input id"
        ):
            checker.validate_issue_form(Path("form.yml"), source)

    def test_missing_explicit_required_flag_fails(self) -> None:
        source = valid_form().replace(
            "    validations:\n      required: true\n"
            "  - type: \"dropdown\"",
            "  - type: \"dropdown\"",
            1,
        )
        with self.assertRaisesRegex(
            checker.TemplateContractError, "explicit required flag"
        ):
            checker.validate_issue_form(Path("form.yml"), source)

    def test_yaml_alias_and_odd_indentation_fail(self) -> None:
        for source, expected in (
            (valid_form().replace('"testing"', "&labels", 1), "aliases"),
            (valid_form().replace("  - type:", "   - type:", 1), "two-space"),
        ):
            with self.subTest(expected=expected):
                with self.assertRaisesRegex(
                    checker.TemplateContractError, expected
                ):
                    checker.validate_issue_form(Path("form.yml"), source)

    def test_pull_request_headings_and_placeholder_pass(self) -> None:
        checker.validate_pull_request_template(
            Path(".github/pull_request_template.md"),
            valid_pull_request_template(),
        )

    def test_pull_request_numeric_closer_and_missing_heading_fail(self) -> None:
        source = valid_pull_request_template().replace(
            "Closes #NUMBER", "Closes #70"
        )
        with self.assertRaisesRegex(
            checker.TemplateContractError, "placeholder"
        ):
            checker.validate_pull_request_template(
                Path(".github/pull_request_template.md"), source
            )
        source = valid_pull_request_template().replace(
            "## Before behavior", "## Before"
        )
        with self.assertRaisesRegex(
            checker.TemplateContractError, "headings must be exactly"
        ):
            checker.validate_pull_request_template(
                Path(".github/pull_request_template.md"), source
            )

    def test_chooser_requires_the_exact_private_route(self) -> None:
        valid = "\n".join(
            (
                "blank_issues_enabled: false",
                "contact_links:",
                '  - name: "Private vulnerability report"',
                f'    url: "{checker.SECURITY_ROUTE}"',
                '    about: "Report suspected vulnerabilities privately; do not use a public issue."',
                "",
            )
        )
        checker.validate_chooser(
            Path(".github/ISSUE_TEMPLATE/config.yml"), valid
        )
        with self.assertRaisesRegex(
            checker.TemplateContractError, "private route"
        ):
            checker.validate_chooser(
                Path(".github/ISSUE_TEMPLATE/config.yml"),
                valid.replace("security/advisories/new", "issues/new"),
            )

    def test_missing_repository_artifacts_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(
                checker.TemplateContractError, "cannot read required"
            ):
                checker.check_repository(Path(temporary))


if __name__ == "__main__":
    unittest.main()
