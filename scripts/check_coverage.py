#!/usr/bin/env python3
"""Fail-closed policy checks for LLVM source and branch coverage."""

from __future__ import annotations

import argparse
from collections import OrderedDict
import json
from pathlib import Path
import sys
from typing import Any


OVERALL_LINE_FLOOR = 85.0
OVERALL_BRANCH_FLOOR = 80.0
CRITICAL_LINE_FLOOR = 85.0
CRITICAL_MODULES = OrderedDict(
    [
        ("parser", "parser.rs"),
        ("dumper", "dumper.rs"),
        ("validator", "validator.rs"),
        ("verifier", "verifier.rs"),
        ("recursive", "recursive.rs"),
    ]
)


def aggregate(metrics: list[dict[str, Any]]) -> dict[str, dict[str, float | int]]:
    result: dict[str, dict[str, float | int]] = {}
    for kind in ("lines", "branches"):
        count = sum(int(metric[kind]["count"]) for metric in metrics)
        covered = sum(int(metric[kind]["covered"]) for metric in metrics)
        result[kind] = {
            "count": count,
            "covered": covered,
            "notcovered": count - covered,
            "percent": covered / count * 100 if count else 0.0,
        }
    return result


def source_suffix(filename: str) -> str:
    normalized = filename.replace("\\", "/")
    marker = "/src/"
    if marker not in normalized:
        return ""
    return normalized.rsplit(marker, 1)[1]


def evaluate(
    report: dict[str, Any],
) -> tuple[OrderedDict[str, dict[str, Any]], list[str]]:
    failures: list[str] = []
    data = report.get("data")
    if not isinstance(data, list) or len(data) != 1:
        return OrderedDict(), ["coverage report must contain exactly one data set"]

    payload = data[0]
    totals = payload.get("totals", {})
    files = payload.get("files", [])
    if not isinstance(files, list):
        return OrderedDict(), ["coverage report files must be a list"]

    summaries: OrderedDict[str, dict[str, Any]] = OrderedDict()
    summaries["overall"] = {
        "lines": totals.get("lines", {}),
        "branches": totals.get("branches", {}),
    }

    file_metrics: dict[str, dict[str, Any]] = {}
    cli_metrics: list[dict[str, Any]] = []
    for item in files:
        suffix = source_suffix(str(item.get("filename", "")))
        summary = item.get("summary", {})
        if suffix:
            file_metrics[suffix] = summary
        if suffix.startswith("cli/"):
            cli_metrics.append(summary)

    for label, filename in CRITICAL_MODULES.items():
        summary = file_metrics.get(filename)
        if summary is None:
            failures.append(f"missing critical module {label} ({filename})")
            continue
        summaries[label] = summary

    if not cli_metrics:
        failures.append("missing critical module cli (src/cli/*.rs)")
    else:
        summaries["cli"] = aggregate(cli_metrics)

    overall_lines = summaries["overall"]["lines"]
    overall_branches = summaries["overall"]["branches"]
    if not overall_lines or int(overall_lines.get("count", 0)) == 0:
        failures.append("line instrumentation is absent")
    elif float(overall_lines.get("percent", 0.0)) < OVERALL_LINE_FLOOR:
        failures.append(
            "overall line coverage "
            f"{float(overall_lines.get('percent', 0.0)):.2f}% "
            f"is below {OVERALL_LINE_FLOOR:.2f}%"
        )

    if not overall_branches or int(overall_branches.get("count", 0)) == 0:
        failures.append("branch instrumentation is absent")
    elif float(overall_branches.get("percent", 0.0)) < OVERALL_BRANCH_FLOOR:
        failures.append(
            "overall branch coverage "
            f"{float(overall_branches.get('percent', 0.0)):.2f}% "
            f"is below {OVERALL_BRANCH_FLOOR:.2f}%"
        )

    for label in (*CRITICAL_MODULES.keys(), "cli"):
        summary = summaries.get(label)
        if summary is None:
            continue
        lines = summary.get("lines", {})
        percent = float(lines.get("percent", 0.0))
        if int(lines.get("count", 0)) == 0:
            failures.append(f"{label} line instrumentation is absent")
        elif percent < CRITICAL_LINE_FLOOR:
            failures.append(
                f"{label} line coverage {percent:.2f}% "
                f"is below {CRITICAL_LINE_FLOOR:.2f}%"
            )

    return summaries, failures


def print_summaries(summaries: OrderedDict[str, dict[str, Any]]) -> None:
    for label, summary in summaries.items():
        lines = summary.get("lines", {})
        branches = summary.get("branches", {})
        print(
            f"{label}: "
            f"lines={float(lines.get('percent', 0.0)):.2f}% "
            f"({int(lines.get('covered', 0))}/{int(lines.get('count', 0))}) "
            f"branches={float(branches.get('percent', 0.0)):.2f}% "
            f"({int(branches.get('covered', 0))}/{int(branches.get('count', 0))})"
        )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path, help="LLVM coverage JSON report")
    arguments = parser.parse_args(argv)

    try:
        report = json.loads(arguments.report.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"coverage report unreadable: {error}", file=sys.stderr)
        return 1

    summaries, failures = evaluate(report)
    print_summaries(summaries)
    if failures:
        for failure in failures:
            print(f"coverage failure: {failure}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
