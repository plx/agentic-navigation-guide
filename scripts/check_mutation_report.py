#!/usr/bin/env python3
"""Fail closed unless the reviewed issue #59 mutation campaign is complete."""

from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import sys
from typing import Any


EXPECTED_VERSION = "27.1.0"
EXPECTED_MUTANTS = 15
EXPECTED_FILES = {
    "src/dumper.rs",
    "src/parser.rs",
    "src/recursive.rs",
    "src/validator.rs",
    "src/verifier.rs",
}
ACCEPTED_SUMMARIES = {"CaughtMutant", "Unviable"}


def evaluate(report: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    if report.get("cargo_mutants_version") != EXPECTED_VERSION:
        failures.append(
            "cargo-mutants version "
            f"{report.get('cargo_mutants_version')!r} is not {EXPECTED_VERSION!r}"
        )

    outcomes = report.get("outcomes")
    if not isinstance(outcomes, list):
        return [*failures, "mutation outcomes must be a list"]

    baseline = [
        outcome
        for outcome in outcomes
        if isinstance(outcome, dict) and outcome.get("scenario") == "Baseline"
    ]
    if len(baseline) != 1 or baseline[0].get("summary") != "Success":
        failures.append("mutation report must contain one successful baseline")

    mutants = [
        outcome
        for outcome in outcomes
        if isinstance(outcome, dict)
        and isinstance(outcome.get("scenario"), dict)
        and "Mutant" in outcome["scenario"]
    ]
    if len(mutants) != EXPECTED_MUTANTS:
        failures.append(
            f"mutation report has {len(mutants)} mutants, expected {EXPECTED_MUTANTS}"
        )
    if report.get("total_mutants") != EXPECTED_MUTANTS:
        failures.append(
            "mutation total "
            f"{report.get('total_mutants')!r} is not {EXPECTED_MUTANTS}"
        )

    summary_values = [outcome.get("summary") for outcome in mutants]
    summaries = Counter(
        summary for summary in summary_values if isinstance(summary, str)
    )
    unacceptable = sorted(
        repr(summary)
        for summary in summary_values
        if not isinstance(summary, str) or summary not in ACCEPTED_SUMMARIES
    )
    if unacceptable:
        failures.append(f"unacceptable mutation outcomes: {unacceptable!r}")
    for field in ("missed", "timeout"):
        if report.get(field) != 0:
            failures.append(f"mutation report {field} count is not zero")
    for field, summary in (
        ("caught", "CaughtMutant"),
        ("unviable", "Unviable"),
    ):
        if report.get(field) != summaries[summary]:
            failures.append(
                f"mutation report {field} count does not match its outcomes"
            )
    if summaries["CaughtMutant"] < 1:
        failures.append("mutation campaign caught no mutants")

    files = {
        outcome["scenario"]["Mutant"].get("file")
        for outcome in mutants
        if isinstance(outcome["scenario"].get("Mutant"), dict)
    }
    if files != EXPECTED_FILES:
        failures.append(
            f"mutation report files {sorted(map(repr, files))!r} do not match "
            f"{sorted(EXPECTED_FILES)!r}"
        )

    return failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path)
    arguments = parser.parse_args(argv)

    try:
        report = json.loads(arguments.report.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"mutation report unreadable: {error}", file=sys.stderr)
        return 1

    failures = evaluate(report)
    if failures:
        for failure in failures:
            print(f"mutation failure: {failure}", file=sys.stderr)
        return 1
    print(
        "mutation gate: "
        f"{report['caught']} caught, {report['unviable']} unviable, "
        f"{report['total_mutants']} total"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
