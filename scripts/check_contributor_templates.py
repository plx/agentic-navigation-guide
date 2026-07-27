#!/usr/bin/env python3
"""Validate the repository's constrained contributor-template contract."""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import NoReturn

ISSUE_FORMS = (
    Path(".github/ISSUE_TEMPLATE/01-bug.yml"),
    Path(".github/ISSUE_TEMPLATE/02-contract-proposal.yml"),
)
CHOOSER = Path(".github/ISSUE_TEMPLATE/config.yml")
PULL_REQUEST_TEMPLATE = Path(".github/pull_request_template.md")
CONTRIBUTING = Path("CONTRIBUTING.md")
SECURITY_ROUTE = (
    "https://github.com/plx/agentic-navigation-guide/security/advisories/new"
)
FORM_TYPES = frozenset({"markdown", "input", "textarea", "dropdown"})
PR_HEADINGS = (
    "## Problem",
    "## Before behavior",
    "## After behavior",
    "## Red-before-fix evidence",
    "## Validation",
    "## Documentation and compatibility",
    "## Security and sensitive data",
    "## Dependencies and issue graph",
    "## Checklist",
)
CONTRIBUTING_HEADINGS = (
    "# Contributing",
    "## Scope and supported environment",
    "## Prepare a trusted checkout",
    "## Choose one issue",
    "## Red-before-fix workflow",
    "## Test and fixture rules",
    "## Validation matrix",
    "## Dependencies, licenses, and release-sensitive files",
    "## Security and sensitive data",
    "## Pull requests and review",
    "## Maintainer triage",
)
CLOSING_DIRECTIVE = re.compile(
    r"(?im)^\s*(?:closes|fixes|resolves)\s+#[0-9]+\s*$"
)
FORM_ID = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")


class TemplateContractError(ValueError):
    """A contributor artifact does not satisfy the checked contract."""


@dataclass
class FormItem:
    kind: str
    identifier: str | None = None
    attributes: dict[str, str | bool] = field(default_factory=dict)
    options: list[str] = field(default_factory=list)
    validations: dict[str, bool] = field(default_factory=dict)


@dataclass
class IssueForm:
    name: str
    description: str
    title: str
    labels: tuple[str, ...]
    items: tuple[FormItem, ...]


def fail(path: Path, line: int | None, message: str) -> NoReturn:
    location = str(path) if line is None else f"{path}:{line}"
    raise TemplateContractError(f"{location}: {message}")


def checked_lines(path: Path, source: str, *, yaml: bool = True) -> list[str]:
    if not source.endswith("\n"):
        fail(path, None, "file must end with one newline")
    if "\r" in source:
        fail(path, None, "carriage returns are not allowed")

    lines = source.splitlines()
    for number, line in enumerate(lines, start=1):
        if "\t" in line:
            fail(path, number, "tabs are not allowed")
        if line.rstrip() != line:
            fail(path, number, "trailing whitespace is not allowed")
        if yaml:
            indentation = len(line) - len(line.lstrip(" "))
            if indentation % 2:
                fail(path, number, "indentation must use two-space units")
            stripped = line.lstrip()
            if stripped.startswith(("&", "*", "!", "<<:", "- &", "- *", "- !")):
                fail(
                    path,
                    number,
                    "YAML aliases, anchors, tags, and merges are forbidden",
                )
    return lines


def quoted_scalar(path: Path, number: int, raw: str) -> str:
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as error:
        fail(path, number, f"scalar must be one JSON-style quoted string: {error.msg}")
    if not isinstance(value, str) or not value.strip():
        fail(path, number, "quoted scalar must be a non-empty string")
    return value


def boolean_scalar(path: Path, number: int, raw: str) -> bool:
    if raw == "true":
        return True
    if raw == "false":
        return False
    fail(path, number, "boolean scalar must be true or false")


def split_mapping(path: Path, number: int, content: str) -> tuple[str, str]:
    if ": " in content:
        key, raw = content.split(": ", 1)
        return key, raw
    if content.endswith(":"):
        return content[:-1], ""
    fail(path, number, "expected a YAML mapping entry")


def validate_issue_form(path: Path, source: str) -> IssueForm:
    lines = checked_lines(path, source)
    top: dict[str, str] = {}
    labels: list[str] = []
    items: list[FormItem] = []
    seen_top: set[str] = set()
    section = "top"
    subsection: str | None = None

    for number, line in enumerate(lines, start=1):
        if not line:
            continue
        indentation = len(line) - len(line.lstrip(" "))
        content = line[indentation:]

        if indentation == 0:
            key, raw = split_mapping(path, number, content)
            if key in seen_top:
                fail(path, number, f"duplicate top-level key {key!r}")
            seen_top.add(key)
            if key in {"name", "description", "title"}:
                top[key] = quoted_scalar(path, number, raw)
                section = "top"
            elif key == "labels" and not raw:
                section = "labels"
            elif key == "body" and not raw:
                section = "body"
            else:
                fail(path, number, f"unsupported top-level key {key!r}")
            subsection = None
            continue

        if section == "labels" and indentation == 2 and content.startswith("- "):
            labels.append(quoted_scalar(path, number, content[2:]))
            continue

        if section != "body":
            fail(path, number, f"unexpected content in {section!r} section")

        if indentation == 2 and content.startswith("- type: "):
            kind = quoted_scalar(path, number, content.removeprefix("- type: "))
            if kind not in FORM_TYPES:
                fail(path, number, f"unsupported issue-form input type {kind!r}")
            items.append(FormItem(kind=kind))
            subsection = None
            continue

        if not items:
            fail(path, number, "body content must start with an input type")
        item = items[-1]

        if indentation == 4:
            key, raw = split_mapping(path, number, content)
            if key == "id":
                if item.identifier is not None:
                    fail(path, number, "duplicate input id")
                item.identifier = quoted_scalar(path, number, raw)
                subsection = None
            elif key in {"attributes", "validations"} and not raw:
                subsection = key
            else:
                fail(path, number, f"unsupported body-item key {key!r}")
            continue

        if indentation == 6 and subsection == "attributes":
            key, raw = split_mapping(path, number, content)
            if key in {"label", "description", "placeholder", "render", "value"}:
                if key in item.attributes:
                    fail(path, number, f"duplicate attribute {key!r}")
                item.attributes[key] = quoted_scalar(path, number, raw)
                continue
            if key == "multiple":
                item.attributes[key] = boolean_scalar(path, number, raw)
                continue
            if key == "options" and not raw:
                subsection = "options"
                continue
            fail(path, number, f"unsupported input attribute {key!r}")

        if indentation == 8 and subsection == "options" and content.startswith("- "):
            item.options.append(quoted_scalar(path, number, content[2:]))
            continue

        if indentation == 6 and subsection == "validations":
            key, raw = split_mapping(path, number, content)
            if key != "required" or key in item.validations:
                fail(path, number, f"unsupported or duplicate validation {key!r}")
            item.validations[key] = boolean_scalar(path, number, raw)
            continue

        fail(path, number, "line is outside the constrained issue-form grammar")

    if tuple(top) != ("name", "description", "title"):
        fail(path, None, "top-level string keys must be name, description, then title")
    if len(top["name"]) <= 3:
        fail(path, None, "form name must contain more than three characters")
    if not labels:
        fail(path, None, "form must apply at least one existing repository label")
    if not items:
        fail(path, None, "form body must not be empty")

    identifiers: set[str] = set()
    for item in items:
        if item.kind == "markdown":
            if item.identifier is not None:
                fail(path, None, "markdown items must not have an id")
            if set(item.attributes) != {"value"}:
                fail(path, None, "markdown items must contain exactly one value")
            if item.validations:
                fail(path, None, "markdown items must not contain validations")
            continue

        if item.identifier is None or not FORM_ID.fullmatch(item.identifier):
            fail(path, None, "every input id must be unique lowercase kebab case")
        if item.identifier in identifiers:
            fail(path, None, f"duplicate input id {item.identifier!r}")
        identifiers.add(item.identifier)
        if "label" not in item.attributes or "description" not in item.attributes:
            fail(path, None, f"input {item.identifier!r} needs label and description")
        if set(item.validations) != {"required"}:
            fail(path, None, f"input {item.identifier!r} needs an explicit required flag")
        if item.kind == "dropdown" and not item.options:
            fail(path, None, f"dropdown {item.identifier!r} needs options")
        if item.kind != "dropdown" and item.options:
            fail(path, None, f"non-dropdown {item.identifier!r} cannot have options")

    return IssueForm(
        name=top["name"],
        description=top["description"],
        title=top["title"],
        labels=tuple(labels),
        items=tuple(items),
    )


def validate_chooser(path: Path, source: str) -> None:
    lines = checked_lines(path, source)
    expected = [
        "blank_issues_enabled: false",
        "contact_links:",
        '  - name: "Private vulnerability report"',
        f'    url: "{SECURITY_ROUTE}"',
        '    about: "Report suspected vulnerabilities privately; do not use a public issue."',
    ]
    if lines != expected:
        fail(path, None, "chooser must disable blank issues and expose only the private route")


def ordered_headings(
    path: Path, source: str, expected: tuple[str, ...], level_prefix: str
) -> None:
    checked_lines(path, source, yaml=False)
    headings = tuple(
        line for line in source.splitlines() if line.startswith(level_prefix)
    )
    if headings != expected:
        fail(path, None, f"headings must be exactly {expected!r}, found {headings!r}")


def validate_pull_request_template(path: Path, source: str) -> None:
    ordered_headings(path, source, PR_HEADINGS, "## ")
    if source.count("Closes #NUMBER") != 1:
        fail(path, None, "template must contain one issue-number placeholder")
    if CLOSING_DIRECTIVE.search(source):
        fail(path, None, "template must not contain a live numeric closing directive")
    for prompt in (
        "base commit SHA",
        "exact pre-fix command",
        "focused post-fix",
        "full post-fix",
        "documentation impact",
        "compatibility impact",
        "security impact",
        "dependency or license impact",
    ):
        if prompt not in source:
            fail(path, None, f"pull request template omits prompt {prompt!r}")


def validate_contributing(path: Path, source: str) -> None:
    checked_lines(path, source, yaml=False)
    headings = tuple(
        line for line in source.splitlines() if line.startswith(("# ", "## "))
    )
    if headings != CONTRIBUTING_HEADINGS:
        fail(path, None, "contributor-guide headings changed without contract review")
    if CLOSING_DIRECTIVE.search(source):
        fail(path, None, "contributor guide must not contain a live closing directive")


def check_repository(root: Path) -> None:
    forms: list[IssueForm] = []
    for relative in ISSUE_FORMS:
        path = root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except OSError as error:
            fail(relative, None, f"cannot read required issue form: {error}")
        forms.append(validate_issue_form(relative, source))

    names = [form.name for form in forms]
    if len(names) != len(set(names)):
        fail(Path(".github/ISSUE_TEMPLATE"), None, "issue-form names must be unique")

    required_ids = {
        ISSUE_FORMS[0]: {
            "version",
            "platform",
            "observed",
            "expected",
            "reproduction",
            "regression",
            "compatibility",
            "security",
        },
        ISSUE_FORMS[1]: {
            "current-contract",
            "proposed-contract",
            "compatibility",
            "platforms",
            "security",
            "dependencies",
        },
    }
    for relative, form in zip(ISSUE_FORMS, forms, strict=True):
        actual = {
            item.identifier for item in form.items if item.identifier is not None
        }
        if actual != required_ids[relative]:
            fail(relative, None, f"input ids must be exactly {sorted(required_ids[relative])}")

    artifacts = (
        (CHOOSER, validate_chooser),
        (PULL_REQUEST_TEMPLATE, validate_pull_request_template),
        (CONTRIBUTING, validate_contributing),
    )
    for relative, validator in artifacts:
        try:
            source = (root / relative).read_text(encoding="utf-8")
        except OSError as error:
            fail(relative, None, f"cannot read required contributor artifact: {error}")
        validator(relative, source)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate contributor guidance and GitHub templates."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root (defaults to this script's checkout)",
    )
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        check_repository(args.root.resolve())
    except TemplateContractError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print("contributor template contract OK: 2 issue forms, chooser, PR template, guide")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
