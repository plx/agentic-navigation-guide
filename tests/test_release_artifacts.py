from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "release_artifacts.py"
SPEC = importlib.util.spec_from_file_location("release_artifacts", SCRIPT)
assert SPEC and SPEC.loader
RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RELEASE)
COMMIT = "1" * 40


class ReleaseArtifactsTests(unittest.TestCase):
    def test_wrong_tag_fails_before_artifact_access(self) -> None:
        missing = Path("/definitely/missing/release-binary")
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(RELEASE.ReleaseError, "release tag mismatch"):
                RELEASE.create_archive(
                    missing,
                    missing,
                    "x86_64-unknown-linux-gnu",
                    "v9.9.9",
                    Path(temporary),
                )

    def test_mismatched_rebuild_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            first = directory / "agentic-navigation-guide"
            second = directory / "comparison"
            first.write_bytes(b"first")
            second.write_bytes(b"second")
            with self.assertRaisesRegex(
                RELEASE.ReleaseError,
                "not byte reproducible",
            ):
                RELEASE.create_archive(
                    first,
                    second,
                    "x86_64-unknown-linux-gnu",
                    "v0.2.0",
                    directory / "dist",
                )

    def test_normalized_archive_is_byte_reproducible_and_complete(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            binary = directory / "agentic-navigation-guide"
            comparison = directory / "comparison"
            binary.write_bytes(b"reviewed release binary")
            comparison.write_bytes(binary.read_bytes())
            first = RELEASE.create_archive(
                binary,
                comparison,
                "x86_64-unknown-linux-gnu",
                "v0.2.0",
                directory / "first",
            )
            second = RELEASE.create_archive(
                binary,
                comparison,
                "x86_64-unknown-linux-gnu",
                "v0.2.0",
                directory / "second",
            )
            self.assertEqual(first.read_bytes(), second.read_bytes())
            members = RELEASE.archive_members(first)
            expected_leaves = {
                "agentic-navigation-guide",
                "LICENSE-APACHE",
                "LICENSE-MIT",
                "NOTICE",
                "README.md",
                "THIRD_PARTY_LICENSES.md",
            }
            self.assertEqual(
                {Path(member).name for member in members},
                expected_leaves,
            )

    def test_checksum_verification_rejects_tampering_and_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            subject = directory / "artifact.tar.gz"
            subject.write_bytes(b"trusted")
            checksums = directory / "SHA256SUMS"
            RELEASE.write_checksums(checksums, [subject])
            self.assertEqual(
                RELEASE.verify_checksums(directory, checksums),
                {"artifact.tar.gz": hashlib.sha256(b"trusted").hexdigest()},
            )
            subject.write_bytes(b"tampered")
            with self.assertRaisesRegex(RELEASE.ReleaseError, "checksum mismatch"):
                RELEASE.verify_checksums(directory, checksums)
            checksums.write_text(f"{'0' * 64}  ../escape\n", encoding="utf-8")
            with self.assertRaisesRegex(
                RELEASE.ReleaseError,
                "unsafe archive member",
            ):
                RELEASE.parse_checksums(checksums)

    def test_smoke_injection_deliberately_blocks_an_otherwise_valid_binary(
        self,
    ) -> None:
        version = subprocess.CompletedProcess(
            args=[],
            returncode=0,
            stdout="agentic-navigation-guide 0.2.0\n",
            stderr="",
        )
        invalid = subprocess.CompletedProcess(
            args=[],
            returncode=2,
            stdout="",
            stderr=(
                "error: unrecognized subcommand "
                "'__release_smoke_invalid_command__'\n"
                "Usage: agentic-navigation-guide\n"
            ),
        )
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "agentic-navigation-guide"
            binary.write_bytes(b"placeholder")
            with mock.patch.object(
                RELEASE.subprocess,
                "run",
                side_effect=[version, invalid],
            ):
                RELEASE.smoke_binary(binary, "v0.2.0")
            with mock.patch.object(
                RELEASE.subprocess,
                "run",
                return_value=version,
            ):
                with self.assertRaisesRegex(
                    RELEASE.ReleaseError,
                    "version mismatch",
                ):
                    RELEASE.smoke_binary(
                        binary,
                        "v0.2.0",
                        inject_failure=True,
                    )

    def test_provenance_binds_exact_checksums_commit_and_ref(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            checksums = directory / "SHA256SUMS"
            checksums.write_text(f"{'a' * 64}  artifact.tar.gz\n", encoding="utf-8")
            value = RELEASE.build_provenance(
                checksums,
                COMMIT,
                "refs/heads/main",
                "100",
                "2",
            )
            path = directory / "provenance.json"
            RELEASE.write_json(path, value)
            RELEASE.verify_provenance(
                path,
                {"artifact.tar.gz": "a" * 64},
                COMMIT,
                "refs/heads/main",
            )
            value["predicate"]["buildDefinition"]["resolvedDependencies"][0][
                "digest"
            ]["gitCommit"] = "2" * 40
            RELEASE.write_json(path, value)
            with self.assertRaisesRegex(
                RELEASE.ReleaseError,
                "exact commit",
            ):
                RELEASE.verify_provenance(
                    path,
                    {"artifact.tar.gz": "a" * 64},
                    COMMIT,
                    "refs/heads/main",
                )

    def test_bundle_rejects_an_unreviewed_extra_asset(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            checksums = directory / "SHA256SUMS"
            provenance = directory / "provenance.json"
            (directory / "expected.bin").write_bytes(b"expected")
            expected_hash = hashlib.sha256(b"expected").hexdigest()
            checksums.write_text(
                f"{expected_hash}  expected.bin\n",
                encoding="utf-8",
            )
            provenance.write_text("{}\n", encoding="utf-8")
            (directory / "unreviewed.bin").write_bytes(b"unexpected")
            with self.assertRaisesRegex(
                RELEASE.ReleaseError,
                "release bundle file set mismatch",
            ):
                RELEASE.verify_bundle(
                    directory,
                    checksums,
                    directory / "expected.bin",
                    provenance,
                    "v0.2.0",
                    COMMIT,
                    "refs/heads/main",
                )

    def test_sbom_has_one_root_and_resolved_dependencies(self) -> None:
        root_id = "path+file:///root#agentic-navigation-guide@0.2.0"
        dependency_id = (
            "registry+https://github.com/rust-lang/crates.io-index#clap@4.5.0"
        )
        metadata = {
            "packages": [
                {
                    "id": root_id,
                    "name": "agentic-navigation-guide",
                    "version": "0.2.0",
                    "source": None,
                    "license": "MIT OR Apache-2.0",
                },
                {
                    "id": dependency_id,
                    "name": "clap",
                    "version": "4.5.0",
                    "source": "registry+https://github.com/rust-lang/crates.io-index",
                    "license": "MIT OR Apache-2.0",
                },
            ],
            "resolve": {
                "root": root_id,
                "nodes": [
                    {"id": root_id, "dependencies": [dependency_id]},
                    {"id": dependency_id, "dependencies": []},
                ],
            },
        }
        with (
            mock.patch.object(RELEASE, "cargo_metadata", return_value=metadata),
            mock.patch.object(
                RELEASE,
                "lock_checksums",
                return_value={("clap", "4.5.0"): "b" * 64},
            ),
            mock.patch.dict(RELEASE.os.environ, {"SOURCE_DATE_EPOCH": "0"}),
        ):
            sbom = RELEASE.build_sbom(COMMIT)
        self.assertEqual(sbom["spdxVersion"], "SPDX-2.3")
        self.assertEqual(sbom["creationInfo"]["created"], "1970-01-01T00:00:00Z")
        self.assertEqual(len(sbom["packages"]), 2)
        self.assertEqual(
            {relationship["relationshipType"] for relationship in sbom["relationships"]},
            {"DESCRIBES", "DEPENDS_ON"},
        )

    def test_release_workflow_has_one_fail_closed_gate_and_publish_boundary(
        self,
    ) -> None:
        workflow = (ROOT / ".github" / "workflows" / "release.yml").read_text(
            encoding="utf-8"
        )
        gate = workflow.split("  release-gate:", 1)[1].split(
            "  rehearsal:",
            1,
        )[0]
        publish = workflow.split("  publish:", 1)[1]
        before_publish = workflow.split("  publish:", 1)[0]

        for prerequisite in [
            "- identity",
            "- quality",
            "- platform-tests",
            "- msrv",
            "- package",
            "- native-archives",
            "- assemble",
        ]:
            self.assertIn(prerequisite, gate)
        self.assertIn("all(.[]; .result == \"success\")", gate)
        self.assertNotIn("id-token: write", before_publish)
        self.assertIn("environment:\n      name: release", publish)
        self.assertIn("id-token: write", publish)
        self.assertIn("rust-lang/crates-io-auth-action@", publish)
        self.assertNotIn("secrets.", publish)
        self.assertIn(".immutable == true", publish)
        self.assertIn("steps.crate-state.outputs.state == 'publish-required'", publish)
        self.assertIn("github.event_name == 'push'", publish)

    def test_pipeline_identity_is_exact_and_personal(self) -> None:
        configuration = RELEASE.pipeline()
        expected = {
            "repository": "plx/agentic-navigation-guide",
            "environment": "release",
            "candidate_tag": "v0.2.0",
            "trusted_publisher_owner": "plx",
            "trusted_publisher_repository": "agentic-navigation-guide",
            "trusted_publisher_workflow": "release.yml",
            "trusted_publisher_environment": "release",
        }
        self.assertEqual(
            {key: configuration[key] for key in expected},
            expected,
        )


if __name__ == "__main__":
    unittest.main()
