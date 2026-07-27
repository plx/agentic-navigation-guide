#!/usr/bin/env python3
"""Run fixed release-mode product benchmarks and enforce resource thresholds."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import resource
import statistics
import subprocess
import sys
import tempfile
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target" / "release" / "agentic-navigation-guide"
DEFAULT_OUTPUT = ROOT / "target" / "issue-59-performance.json"
GUIDE_ENVIRONMENT_VARIABLES = (
    "AGENTIC_NAVIGATION_GUIDE_PATH",
    "AGENTIC_NAVIGATION_GUIDE_ROOT",
    "AGENTIC_NAVIGATION_GUIDE_NAME",
    "AGENTIC_NAVIGATION_GUIDE_LOG_MODE",
    "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE",
)
FIXTURE_SEED = "fixed-sequential-v1"
FLAT_SIZES = (10_000, 20_000, 40_000, 100_000)
PLACEHOLDER_SIZES = (500, 1_000, 2_000)
RECURSIVE_ROOTS = 200
MAX_SCALING_RATIO = 2.5
MAX_FLAT_SECONDS = 5.0
MAX_FLAT_RSS_MIB = 256.0
MAX_SELF_VERIFY_SECONDS = 1.0
MAX_BASELINE_REGRESSION = 1.20
TIMING_RESOLUTION_SECONDS = 0.010
RSS_RESOLUTION_MIB = 8.0
EXPECTED_CASES = ("flat", "deep", "placeholders", "recursive", "self_verify")
REQUIRED_METADATA = (
    "binary_sha256",
    "fixture_seed",
    "filesystem",
    "os",
    "toolchain",
)


def percentile(samples: list[float], quantile: float) -> float:
    if not samples:
        raise ValueError("percentile requires at least one sample")
    ordered = sorted(samples)
    index = max(0, min(len(ordered) - 1, math.ceil(len(ordered) * quantile) - 1))
    return ordered[index]


def validate_samples(
    label: str,
    samples: Any,
    expected_sizes: tuple[int, ...],
    failures: list[str],
) -> None:
    if not isinstance(samples, list):
        failures.append(f"benchmark case {label} must be a list")
        return
    sizes = tuple(item.get("size") for item in samples if isinstance(item, dict))
    if sizes != expected_sizes:
        failures.append(
            f"benchmark case {label} sizes {sizes!r} do not match {expected_sizes!r}"
        )
    for item in samples:
        if not isinstance(item, dict):
            failures.append(f"benchmark case {label} contains a non-object sample")
            continue
        for metric in ("median_seconds", "p95_seconds", "max_rss_mib"):
            value = item.get(metric)
            if not isinstance(value, (int, float)) or value < 0:
                failures.append(
                    f"benchmark case {label} size {item.get('size')} "
                    f"has invalid {metric}"
                )


def validate_scaling(
    label: str,
    samples: list[dict[str, Any]],
    failures: list[str],
) -> None:
    for previous, current in zip(samples, samples[1:]):
        if current["size"] != previous["size"] * 2:
            continue
        previous_median = float(previous["median_seconds"])
        current_median = float(current["median_seconds"])
        ratio = current_median / previous_median if previous_median else math.inf
        if ratio > MAX_SCALING_RATIO:
            failures.append(
                f"{label} {previous['size']}->{current['size']} "
                f"median scaled by {ratio:.3f}x, above {MAX_SCALING_RATIO:.1f}x"
            )


def validate_report(report: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    metadata = report.get("metadata", {})
    for name in REQUIRED_METADATA:
        if not metadata.get(name):
            failures.append(f"missing benchmark metadata {name}")
    if metadata.get("fixture_seed") not in (None, FIXTURE_SEED):
        failures.append(
            f"fixture seed {metadata.get('fixture_seed')!r} is not {FIXTURE_SEED!r}"
        )

    cases = report.get("cases", {})
    for name in EXPECTED_CASES:
        if name not in cases:
            failures.append(f"missing benchmark case {name}")

    flat = cases.get("flat")
    validate_samples("flat", flat, FLAT_SIZES, failures)
    if isinstance(flat, list) and len(flat) == len(FLAT_SIZES):
        validate_scaling("flat", flat, failures)
        for item in flat:
            if item.get("success") is not True:
                failures.append(f"flat size {item.get('size')} did not succeed")
        largest = flat[-1]
        if float(largest.get("median_seconds", math.inf)) >= MAX_FLAT_SECONDS:
            failures.append(
                f"100000-entry flat median {largest.get('median_seconds')}s "
                f"is not below {MAX_FLAT_SECONDS:.1f}s"
            )
        if float(largest.get("max_rss_mib", math.inf)) >= MAX_FLAT_RSS_MIB:
            failures.append(
                f"100000-entry flat RSS {largest.get('max_rss_mib')} MiB "
                f"is not below {MAX_FLAT_RSS_MIB:.0f} MiB"
            )

    placeholders = cases.get("placeholders")
    validate_samples("placeholders", placeholders, PLACEHOLDER_SIZES, failures)
    if isinstance(placeholders, list) and len(placeholders) == len(PLACEHOLDER_SIZES):
        validate_scaling("placeholders", placeholders, failures)
        for item in placeholders:
            if item.get("success") is not True:
                failures.append(
                    f"placeholders size {item.get('size')} did not succeed"
                )

    deep = cases.get("deep")
    validate_samples("deep", deep, (256, 257), failures)
    if isinstance(deep, list) and len(deep) == 2:
        variants = tuple(item.get("variant") for item in deep)
        if variants != ("valid", "invalid"):
            failures.append(f"deep variants {variants!r} are not valid/invalid")
        if deep[0].get("success") is not True:
            failures.append("deep valid guide did not succeed")
        if deep[1].get("success") is not False:
            failures.append("deep invalid guide did not fail")

    recursive = cases.get("recursive")
    validate_samples("recursive", recursive, (RECURSIVE_ROOTS,), failures)
    if isinstance(recursive, list) and len(recursive) == 1:
        if recursive[0].get("success") is not True:
            failures.append("recursive discovery benchmark did not succeed")

    self_verify = cases.get("self_verify")
    validate_samples("self_verify", self_verify, (1,), failures)
    if isinstance(self_verify, list) and len(self_verify) == 1:
        if self_verify[0].get("success") is not True:
            failures.append("self verification benchmark did not succeed")
        if (
            float(self_verify[0].get("median_seconds", math.inf))
            >= MAX_SELF_VERIFY_SECONDS
        ):
            failures.append(
                "self verification median "
                f"{self_verify[0].get('median_seconds')}s is not below "
                f"{MAX_SELF_VERIFY_SECONDS:.1f}s"
            )

    return failures


def compare_to_reference(
    report: dict[str, Any],
    reference: dict[str, Any],
) -> list[str]:
    failures: list[str] = []
    for case_name in EXPECTED_CASES:
        current_samples = report["cases"][case_name]
        reference_samples = reference["cases"][case_name]
        for current, previous in zip(current_samples, reference_samples):
            label = f"{case_name} size {current['size']}"
            previous_median = float(previous["median_seconds"])
            allowed_median = max(
                previous_median * MAX_BASELINE_REGRESSION,
                previous_median + TIMING_RESOLUTION_SECONDS,
            )
            if float(current["median_seconds"]) > allowed_median:
                failures.append(
                    f"{label} median {current['median_seconds']}s exceeds "
                    f"reference allowance {allowed_median}s"
                )

            previous_rss = float(previous["max_rss_mib"])
            allowed_rss = max(
                previous_rss * MAX_BASELINE_REGRESSION,
                previous_rss + RSS_RESOLUTION_MIB,
            )
            if float(current["max_rss_mib"]) > allowed_rss:
                failures.append(
                    f"{label} RSS {current['max_rss_mib']} MiB exceeds "
                    f"reference allowance {allowed_rss} MiB"
                )
    return failures


def child_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for name in GUIDE_ENVIRONMENT_VARIABLES:
        environment.pop(name, None)
    return environment


def resource_rss_mib(usage: resource.struct_rusage) -> float:
    rss = usage.ru_maxrss
    if sys.platform == "darwin":
        return rss / (1024 * 1024)
    return rss / 1024


def run_once(command: list[str], cwd: Path) -> tuple[float, float, bool]:
    started = time.perf_counter()
    child = subprocess.Popen(
        command,
        cwd=cwd,
        env=child_environment(),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    _, wait_status, usage = os.wait4(child.pid, 0)
    child.returncode = os.waitstatus_to_exitcode(wait_status)
    elapsed = time.perf_counter() - started
    return elapsed, resource_rss_mib(usage), child.returncode == 0


def benchmark_command(
    command: list[str],
    cwd: Path,
    size: int,
    warmups: int,
    samples: int,
) -> dict[str, Any]:
    for _ in range(warmups):
        run_once(command, cwd)

    durations: list[float] = []
    rss_values: list[float] = []
    statuses: list[bool] = []
    for _ in range(samples):
        elapsed, rss_mib, success = run_once(command, cwd)
        durations.append(elapsed)
        rss_values.append(rss_mib)
        statuses.append(success)
    return {
        "size": size,
        "median_seconds": statistics.median(durations),
        "p95_seconds": percentile(durations, 0.95),
        "max_rss_mib": max(rss_values),
        "success": all(statuses),
    }


def write_flat_guide(path: Path, size: int) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as guide:
        guide.write("<agentic-navigation-guide>\n")
        for index in range(size):
            guide.write(f"- file-{index:06}.txt\n")
        guide.write("</agentic-navigation-guide>\n")


def write_deep_guide(path: Path, deepest_depth: int) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as guide:
        guide.write("<agentic-navigation-guide>\n")
        for depth in range(deepest_depth + 1):
            guide.write(f"{' ' * depth}- directory-{depth}/\n")
        guide.write("</agentic-navigation-guide>\n")


def write_placeholder_fixture(directory: Path, size: int) -> tuple[Path, Path]:
    root = directory / f"placeholder-root-{size}"
    root.mkdir()
    guide_path = directory / f"placeholder-{size}.md"
    with guide_path.open("w", encoding="utf-8", newline="\n") as guide:
        guide.write("<agentic-navigation-guide>\n")
        for index in range(size):
            name = f"file-{index:04}.txt"
            (root / name).write_text("", encoding="utf-8")
            guide.write(f"- {name}\n")
            guide.write("- ... # fixed benchmark annotation\n")
        guide.write("</agentic-navigation-guide>\n")
    return root, guide_path


def write_recursive_fixture(directory: Path) -> Path:
    root = directory / "recursive"
    root.mkdir()
    for index in range(RECURSIVE_ROOTS):
        child = root / f"root-{index:04}"
        child.mkdir()
        (child / "present.txt").write_text("", encoding="utf-8")
        (child / "AGENTIC_NAVIGATION_GUIDE.md").write_text(
            "<agentic-navigation-guide>\n"
            "- present.txt\n"
            "</agentic-navigation-guide>\n",
            encoding="utf-8",
        )
    return root


def command_output(command: list[str]) -> str:
    return subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=True,
    ).stdout.strip()


def filesystem_name(path: Path) -> str:
    if sys.platform.startswith("linux"):
        try:
            return command_output(
                ["findmnt", "--noheadings", "--output", "FSTYPE", "--target", str(path)]
            )
        except (OSError, subprocess.CalledProcessError):
            pass
    return platform.system()


def binary_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as binary:
        for block in iter(lambda: binary.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run_baseline(
    binary: Path,
    warmups: int,
    samples: int,
) -> dict[str, Any]:
    if not binary.is_file():
        raise FileNotFoundError(f"release binary not found: {binary}")
    binary = binary.resolve()

    with tempfile.TemporaryDirectory(prefix="issue59-performance-") as temp_name:
        temp = Path(temp_name)
        flat_results = []
        for size in FLAT_SIZES:
            guide = temp / f"flat-{size}.md"
            write_flat_guide(guide, size)
            flat_results.append(
                benchmark_command(
                    [str(binary), "check", "--guide", str(guide)],
                    temp,
                    size,
                    warmups,
                    samples,
                )
            )

        deep_valid = temp / "deep-valid.md"
        deep_invalid = temp / "deep-invalid.md"
        write_deep_guide(deep_valid, 256)
        write_deep_guide(deep_invalid, 257)
        deep_results = [
            {
                **benchmark_command(
                    [str(binary), "check", "--guide", str(deep_valid)],
                    temp,
                    256,
                    warmups,
                    samples,
                ),
                "variant": "valid",
            },
            {
                **benchmark_command(
                    [str(binary), "check", "--guide", str(deep_invalid)],
                    temp,
                    257,
                    warmups,
                    samples,
                ),
                "variant": "invalid",
            },
        ]

        placeholder_results = []
        for size in PLACEHOLDER_SIZES:
            root, guide = write_placeholder_fixture(temp, size)
            placeholder_results.append(
                benchmark_command(
                    [
                        str(binary),
                        "verify",
                        "--guide",
                        str(guide),
                        "--root",
                        str(root),
                    ],
                    temp,
                    size,
                    warmups,
                    samples,
                )
            )

        recursive_root = write_recursive_fixture(temp)
        recursive_results = [
            benchmark_command(
                [
                    str(binary),
                    "verify",
                    "--recursive",
                    "--root",
                    str(recursive_root),
                ],
                temp,
                RECURSIVE_ROOTS,
                warmups,
                samples,
            )
        ]
        self_verify_results = [
            benchmark_command(
                [
                    str(binary),
                    "verify",
                    "--guide",
                    str(ROOT / "AGENTIC_NAVIGATION_GUIDE.md"),
                    "--root",
                    str(ROOT),
                ],
                ROOT,
                1,
                warmups,
                samples,
            )
        ]

        return {
            "metadata": {
                "binary_sha256": binary_sha256(binary),
                "fixture_seed": FIXTURE_SEED,
                "filesystem": filesystem_name(temp),
                "os": platform.platform(),
                "toolchain": command_output(["rustc", "--version"]),
            },
            "cases": {
                "flat": flat_results,
                "deep": deep_results,
                "placeholders": placeholder_results,
                "recursive": recursive_results,
                "self_verify": self_verify_results,
            },
        }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--validate", type=Path)
    parser.add_argument("--reference", type=Path)
    arguments = parser.parse_args(argv)

    if arguments.validate is not None:
        report = json.loads(arguments.validate.read_text(encoding="utf-8"))
    else:
        if arguments.warmups < 0 or arguments.samples < 1:
            parser.error("warmups must be nonnegative and samples must be positive")
        report = run_baseline(arguments.binary, arguments.warmups, arguments.samples)
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    failures = validate_report(report)
    if arguments.reference is not None:
        reference = json.loads(arguments.reference.read_text(encoding="utf-8"))
        reference_failures = validate_report(reference)
        failures.extend(
            f"reference is invalid: {failure}" for failure in reference_failures
        )
        if not reference_failures and not failures:
            failures.extend(compare_to_reference(report, reference))

    print(json.dumps(report, indent=2, sort_keys=True))
    if failures:
        for failure in failures:
            print(f"performance failure: {failure}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
