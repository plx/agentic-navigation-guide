import importlib.util
from pathlib import Path
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "check_coverage.py"
SPEC = importlib.util.spec_from_file_location("check_coverage", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def metric(count, covered):
    return {
        "count": count,
        "covered": covered,
        "notcovered": count - covered,
        "percent": covered / count * 100,
    }


def coverage_file(path, lines=(100, 90), branches=(20, 17)):
    return {
        "filename": f"/checkout/src/{path}",
        "summary": {
            "lines": metric(*lines),
            "branches": metric(*branches),
        },
    }


def passing_report():
    files = [
        coverage_file("parser.rs"),
        coverage_file("dumper.rs"),
        coverage_file("validator.rs"),
        coverage_file("verifier.rs"),
        coverage_file("recursive.rs"),
        coverage_file("cli/check.rs"),
        coverage_file("cli/output.rs"),
        coverage_file("guide_input.rs", lines=(100, 50), branches=(20, 10)),
    ]
    return {
        "data": [
            {
                "totals": {
                    "lines": metric(1_000, 880),
                    "branches": metric(200, 162),
                },
                "files": files,
            }
        ]
    }


class CoverageCheckerTests(unittest.TestCase):
    def test_matching_branch_aware_report_passes(self):
        summaries, failures = CHECKER.evaluate(passing_report())

        self.assertEqual(failures, [])
        self.assertEqual(
            list(summaries),
            [
                "overall",
                "parser",
                "dumper",
                "validator",
                "verifier",
                "recursive",
                "cli",
            ],
        )
        self.assertEqual(summaries["cli"]["lines"]["count"], 200)

    def test_missing_branch_instrumentation_fails_closed(self):
        report = passing_report()
        report["data"][0]["totals"]["branches"] = metric(0, 0)

        _, failures = CHECKER.evaluate(report)

        self.assertTrue(
            any("branch instrumentation is absent" in item for item in failures),
            failures,
        )

    def test_overall_line_and_branch_regressions_fail(self):
        report = passing_report()
        report["data"][0]["totals"]["lines"] = metric(1_000, 849)
        report["data"][0]["totals"]["branches"] = metric(200, 159)

        _, failures = CHECKER.evaluate(report)

        self.assertTrue(any("overall line coverage" in item for item in failures))
        self.assertTrue(any("overall branch coverage" in item for item in failures))

    def test_missing_or_under_floor_critical_module_fails(self):
        report = passing_report()
        report["data"][0]["files"] = [
            item
            for item in report["data"][0]["files"]
            if not item["filename"].endswith("/src/dumper.rs")
        ]
        validator = next(
            item
            for item in report["data"][0]["files"]
            if item["filename"].endswith("/src/validator.rs")
        )
        validator["summary"]["lines"] = metric(100, 84)

        _, failures = CHECKER.evaluate(report)

        self.assertTrue(any("missing critical module dumper" in item for item in failures))
        self.assertTrue(any("validator line coverage" in item for item in failures))

    def test_cli_floor_uses_aggregate_counts(self):
        report = passing_report()
        cli_files = [
            item
            for item in report["data"][0]["files"]
            if "/src/cli/" in item["filename"]
        ]
        cli_files[0]["summary"]["lines"] = metric(10, 1)
        cli_files[1]["summary"]["lines"] = metric(190, 180)

        summaries, failures = CHECKER.evaluate(report)

        self.assertAlmostEqual(summaries["cli"]["lines"]["percent"], 90.5)
        self.assertFalse(any("cli line coverage" in item for item in failures))


if __name__ == "__main__":
    unittest.main()
