import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = (
    Path(__file__).resolve().parents[1]
    / "scripts"
    / "check_release_identity.py"
)
SPEC = importlib.util.spec_from_file_location("check_release_identity", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def identity():
    intended = copy.deepcopy(CHECKER.EXPECTED_IDENTITY)
    intended["migration_baseline"] = copy.deepcopy(
        CHECKER.EXPECTED_MIGRATION_EVIDENCE
    )
    intended["future_compatibility"] = copy.deepcopy(
        CHECKER.EXPECTED_FUTURE_COMPATIBILITY
    )
    return intended


def published_metadata():
    return copy.deepcopy(CHECKER.EXPECTED_PUBLISHED_METADATA)


class ReleaseIdentityCheckerTests(unittest.TestCase):
    def test_matching_static_inputs_pass(self):
        intended = identity()
        manifest = {
            "package": {
                "name": "agentic-navigation-guide",
                "version": "0.2.0",
                "license": "MIT OR Apache-2.0",
            },
            "bin": [
                {
                    "name": "agentic-navigation-guide",
                    "path": "src/main.rs",
                }
            ],
        }
        lock = {
            "package": [
                {
                    "name": "agentic-navigation-guide",
                    "version": "0.2.0",
                }
            ]
        }
        source = """#[command(
            name = "agentic-navigation-guide",
            version,
            author
        )]"""

        self.assertEqual(
            CHECKER.check_identity(intended, published_metadata()),
            [],
        )
        self.assertEqual(CHECKER.check_manifest(manifest, intended), [])
        self.assertEqual(CHECKER.check_lock(lock, intended), [])
        self.assertEqual(CHECKER.check_tag("v0.2.0", intended), [])
        self.assertEqual(
            CHECKER.check_changelog("## [0.2.0] - Unreleased\n", intended),
            [],
        )
        self.assertEqual(
            CHECKER.check_cli_output(
                "agentic-navigation-guide 0.2.0\n",
                intended,
            ),
            [],
        )
        self.assertEqual(CHECKER.check_cli_source(source, intended), [])

    def test_manifest_version_mismatch_fails(self):
        intended = identity()
        manifest = {
            "package": {
                "name": "agentic-navigation-guide",
                "version": "0.1.4",
                "license": "MIT OR Apache-2.0",
            },
            "bin": [
                {
                    "name": "agentic-navigation-guide",
                    "path": "src/main.rs",
                }
            ],
        }
        failures = CHECKER.check_manifest(manifest, intended)
        self.assertTrue(any("0.1.4" in failure for failure in failures), failures)

    def test_additional_manifest_binary_fails(self):
        intended = identity()
        manifest = {
            "package": {
                "name": "agentic-navigation-guide",
                "version": "0.2.0",
                "license": "MIT OR Apache-2.0",
            },
            "bin": [
                {
                    "name": "agentic-navigation-guide",
                    "path": "src/main.rs",
                },
                {
                    "name": "unexpected",
                    "path": "src/unexpected.rs",
                },
            ],
        }
        failures = CHECKER.check_manifest(manifest, intended)
        self.assertTrue(any("2 binaries" in failure for failure in failures), failures)

    def test_lock_version_mismatch_fails(self):
        failures = CHECKER.check_lock(
            {
                "package": [
                    {
                        "name": "agentic-navigation-guide",
                        "version": "0.1.4",
                    }
                ]
            },
            identity(),
        )
        self.assertEqual(len(failures), 1)
        self.assertIn("0.1.4", failures[0])

    def test_external_tag_mismatch_fails(self):
        failures = CHECKER.check_tag("v0.1.4", identity())
        self.assertEqual(len(failures), 1)
        self.assertIn("v0.2.0", failures[0])

    def test_missing_or_drifted_changelog_fails(self):
        self.assertTrue(CHECKER.check_changelog(None, identity()))
        self.assertTrue(
            CHECKER.check_changelog("## [0.1.4] - Unreleased\n", identity())
        )

    def test_duplicate_identity_version_heading_fails(self):
        failures = CHECKER.check_changelog(
            "## [0.2.0] - Unreleased\n\n## [0.2.0] - 2026-08-01\n",
            identity(),
        )
        self.assertTrue(failures)

    def test_binary_version_mismatch_fails(self):
        failures = CHECKER.check_cli_output(
            "agentic-navigation-guide 0.1.4\n",
            identity(),
        )
        self.assertEqual(len(failures), 1)
        self.assertIn("0.2.0", failures[0])

    def test_hard_coded_source_version_fails(self):
        failures = CHECKER.check_cli_source(
            """#[command(
                name = "agentic-navigation-guide",
                version = "0.2.0",
                author
            )]""",
            identity(),
        )
        self.assertTrue(failures)

    def test_identity_baseline_or_tag_convention_drift_fails(self):
        intended = identity()
        intended["tag_prefix"] = "release-"
        intended["migration_baseline"]["artifact_sha256"] = "0" * 64
        failures = CHECKER.check_identity(intended, published_metadata())
        self.assertTrue(any("tag_prefix" in failure for failure in failures), failures)
        self.assertTrue(
            any("artifact_sha256" in failure for failure in failures),
            failures,
        )

    def test_identity_schema_rejects_unrecognized_keys(self):
        intended = identity()
        intended["unsupported_policy"] = "ignored"
        intended["migration_baseline"]["artifact_sh256"] = "typo"
        intended["future_compatibility"]["baseline_polcy"] = "typo"
        failures = CHECKER.check_identity(intended, published_metadata())
        self.assertTrue(any("root keys" in failure for failure in failures), failures)
        self.assertTrue(
            any("migration baseline keys" in failure for failure in failures),
            failures,
        )
        self.assertTrue(
            any("future compatibility keys" in failure for failure in failures),
            failures,
        )

    def test_published_fixture_hashes_are_computed(self):
        fixture = (
            Path(__file__).resolve().parents[1] / CHECKER.PUBLISHED_API_FIXTURE
        )
        metadata, rows = CHECKER.load_published_api_fixture(fixture)
        self.assertEqual(
            CHECKER.check_published_api_fixture(metadata, rows),
            [],
        )

        drifted = list(rows)
        identifier, kind, symbol = drifted[0]
        drifted[0] = (identifier, kind, f"{symbol}-drift")
        failures = CHECKER.check_published_api_fixture(metadata, drifted)
        self.assertTrue(
            any("ordered symbol SHA-256" in failure for failure in failures),
            failures,
        )

        kind_swapped = list(rows)
        first = kind_swapped[0]
        second = kind_swapped[1]
        kind_swapped[0] = (first[0], second[1], first[2])
        kind_swapped[1] = (second[0], first[1], second[2])
        failures = CHECKER.check_published_api_fixture(metadata, kind_swapped)
        self.assertTrue(
            any("ordered row SHA-256" in failure for failure in failures),
            failures,
        )

    def test_additional_metadata_binary_fails(self):
        root = Path(__file__).resolve().parents[1]
        intended = identity()
        metadata = {
            "packages": [
                {
                    "manifest_path": str(root / "Cargo.toml"),
                    "name": "agentic-navigation-guide",
                    "version": "0.2.0",
                    "license": "MIT OR Apache-2.0",
                    "targets": [
                        {
                            "name": "agentic-navigation-guide",
                            "kind": ["bin"],
                            "crate_types": ["bin"],
                        },
                        {
                            "name": "unexpected",
                            "kind": ["bin"],
                            "crate_types": ["bin"],
                        },
                    ],
                }
            ]
        }
        failures = CHECKER.check_metadata(metadata, intended, root)
        self.assertTrue(any("2 binaries" in failure for failure in failures), failures)

    def test_build_executable_comes_from_cargo_json(self):
        with tempfile.TemporaryDirectory() as temporary:
            executable = Path(temporary) / "custom-target" / "candidate"
            executable.parent.mkdir()
            executable.touch()
            build_output = json.dumps(
                {
                    "reason": "compiler-artifact",
                    "target": {
                        "name": "agentic-navigation-guide",
                        "kind": ["bin"],
                        "crate_types": ["bin"],
                    },
                    "executable": str(executable),
                }
            )
            observed, failures = CHECKER.built_executable(
                build_output,
                identity(),
            )
            self.assertEqual(failures, [])
            self.assertEqual(observed, executable)


if __name__ == "__main__":
    unittest.main()
