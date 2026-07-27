import importlib.util
from pathlib import Path
import unittest


SCRIPT = (
    Path(__file__).resolve().parents[1]
    / "scripts"
    / "check_mutation_report.py"
)
SPEC = importlib.util.spec_from_file_location("check_mutation_report", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def mutant(file, summary="CaughtMutant"):
    return {
        "scenario": {"Mutant": {"file": file}},
        "summary": summary,
    }


def passing_report():
    files = sorted(CHECKER.EXPECTED_FILES)
    outcomes = [{"scenario": "Baseline", "summary": "Success"}]
    outcomes.extend(mutant(file) for file in files)
    outcomes.extend(
        mutant(files[index % len(files)], "Unviable")
        for index in range(CHECKER.EXPECTED_MUTANTS - len(files))
    )
    return {
        "cargo_mutants_version": CHECKER.EXPECTED_VERSION,
        "outcomes": outcomes,
        "total_mutants": CHECKER.EXPECTED_MUTANTS,
        "caught": len(files),
        "unviable": CHECKER.EXPECTED_MUTANTS - len(files),
        "missed": 0,
        "timeout": 0,
    }


class MutationReportCheckerTests(unittest.TestCase):
    def test_complete_reviewed_campaign_passes(self):
        self.assertEqual(CHECKER.evaluate(passing_report()), [])

    def test_missing_tool_run_or_module_fails_closed(self):
        report = passing_report()
        report["cargo_mutants_version"] = None
        report["outcomes"] = report["outcomes"][:-1]
        report["total_mutants"] -= 1

        failures = CHECKER.evaluate(report)

        self.assertTrue(any("version" in failure for failure in failures))
        self.assertTrue(any("expected 15" in failure for failure in failures))

    def test_survivor_timeout_and_failed_baseline_fail(self):
        report = passing_report()
        report["outcomes"][0]["summary"] = "Failure"
        report["outcomes"][1]["summary"] = "MissedMutant"
        report["missed"] = 1
        report["timeout"] = 1

        failures = CHECKER.evaluate(report)

        self.assertTrue(any("successful baseline" in failure for failure in failures))
        self.assertTrue(any("unacceptable mutation" in failure for failure in failures))
        self.assertTrue(any("missed count" in failure for failure in failures))
        self.assertTrue(any("timeout count" in failure for failure in failures))


if __name__ == "__main__":
    unittest.main()
