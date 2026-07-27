import importlib.util
from pathlib import Path
import unittest


SCRIPT = (
    Path(__file__).resolve().parents[1]
    / "scripts"
    / "run_performance_baseline.py"
)
SPEC = importlib.util.spec_from_file_location("run_performance_baseline", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
BASELINE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASELINE)


def sample(size, median, *, p95=None, rss=32.0, success=True):
    return {
        "size": size,
        "median_seconds": median,
        "p95_seconds": median if p95 is None else p95,
        "max_rss_mib": rss,
        "success": success,
    }


def passing_report():
    return {
        "metadata": {
            "binary_sha256": "a" * 64,
            "fixture_seed": "fixed-sequential-v1",
            "filesystem": "apfs",
            "os": "Darwin",
            "toolchain": "rustc 1.90.0",
        },
        "cases": {
            "flat": [
                sample(10_000, 0.10),
                sample(20_000, 0.18),
                sample(40_000, 0.32),
                sample(100_000, 0.70, p95=0.75, rss=64.0),
            ],
            "deep": [
                {**sample(256, 0.02), "variant": "valid"},
                {**sample(257, 0.02, success=False), "variant": "invalid"},
            ],
            "placeholders": [
                sample(500, 0.05),
                sample(1_000, 0.08),
                sample(2_000, 0.14),
            ],
            "recursive": [sample(200, 0.20)],
            "self_verify": [sample(1, 0.05)],
        },
    }


class PerformanceBaselineTests(unittest.TestCase):
    def test_percentile_is_nearest_rank(self):
        self.assertEqual(BASELINE.percentile([0.4, 0.1, 0.3, 0.2], 0.50), 0.2)
        self.assertEqual(BASELINE.percentile([0.4, 0.1, 0.3, 0.2], 0.95), 0.4)

    def test_complete_fixed_baseline_passes(self):
        self.assertEqual(BASELINE.validate_report(passing_report()), [])

    def test_missing_case_or_metadata_fails_closed(self):
        report = passing_report()
        del report["cases"]["recursive"]
        del report["metadata"]["binary_sha256"]

        failures = BASELINE.validate_report(report)

        self.assertTrue(any("metadata binary_sha256" in item for item in failures))
        self.assertTrue(any("missing benchmark case recursive" in item for item in failures))

    def test_scaling_absolute_time_and_rss_regressions_fail(self):
        report = passing_report()
        report["cases"]["flat"][1]["median_seconds"] = 0.30
        report["cases"]["flat"][-1]["median_seconds"] = 5.01
        report["cases"]["flat"][-1]["max_rss_mib"] = 256.01
        report["cases"]["placeholders"][1]["median_seconds"] = 0.20

        failures = BASELINE.validate_report(report)

        self.assertTrue(any("flat 10000->20000" in item for item in failures))
        self.assertTrue(any("100000-entry flat median" in item for item in failures))
        self.assertTrue(any("100000-entry flat RSS" in item for item in failures))
        self.assertTrue(any("placeholders 500->1000" in item for item in failures))

    def test_expected_status_and_self_verification_regressions_fail(self):
        report = passing_report()
        report["cases"]["deep"][0]["success"] = False
        report["cases"]["deep"][1]["success"] = True
        report["cases"]["self_verify"][0]["median_seconds"] = 1.01

        failures = BASELINE.validate_report(report)

        self.assertTrue(any("deep valid" in item for item in failures))
        self.assertTrue(any("deep invalid" in item for item in failures))
        self.assertTrue(any("self verification median" in item for item in failures))


if __name__ == "__main__":
    unittest.main()
