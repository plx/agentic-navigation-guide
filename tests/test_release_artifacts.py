from __future__ import annotations

import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock
import urllib.error


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

    def test_checksums_reject_missing_subjects_and_duplicate_basenames(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            first_directory = directory / "first"
            second_directory = directory / "second"
            first_directory.mkdir()
            second_directory.mkdir()
            first = first_directory / "artifact.bin"
            second = second_directory / "artifact.bin"
            first.write_bytes(b"first")
            second.write_bytes(b"second")
            with self.assertRaisesRegex(
                RELEASE.ReleaseError,
                "duplicate checksum subject name",
            ):
                RELEASE.checksum_lines([first, second])

            checksums = directory / "SHA256SUMS"
            RELEASE.write_checksums(checksums, [first])
            first.unlink()
            with self.assertRaisesRegex(
                RELEASE.ReleaseError,
                "checksum subject is missing",
            ):
                RELEASE.verify_checksums(first_directory, checksums)

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

    def test_smoke_archive_extracts_the_exact_member_set(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            binary = directory / "agentic-navigation-guide"
            comparison = directory / "comparison"
            binary.write_bytes(b"native executable")
            comparison.write_bytes(binary.read_bytes())
            archive = RELEASE.create_archive(
                binary,
                comparison,
                "x86_64-unknown-linux-gnu",
                "v0.2.0",
                directory / "dist",
            )

            def inspect_extracted(
                extracted_binary: Path,
                tag: str,
                inject_failure: bool,
            ) -> None:
                self.assertEqual(extracted_binary.read_bytes(), b"native executable")
                self.assertEqual(extracted_binary.name, "agentic-navigation-guide")
                self.assertEqual(tag, "v0.2.0")
                self.assertFalse(inject_failure)

            with mock.patch.object(
                RELEASE,
                "smoke_binary",
                side_effect=inspect_extracted,
            ) as smoke:
                RELEASE.smoke_archive(archive, "v0.2.0")
            smoke.assert_called_once()

    def test_windows_archive_smoke_selects_the_exe_member(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            binary = directory / "agentic-navigation-guide.exe"
            comparison = directory / "comparison.exe"
            binary.write_bytes(b"windows executable")
            comparison.write_bytes(binary.read_bytes())
            archive = RELEASE.create_archive(
                binary,
                comparison,
                "x86_64-pc-windows-msvc",
                "v0.2.0",
                directory / "dist",
            )

            def inspect_extracted(
                extracted_binary: Path,
                _tag: str,
                _inject_failure: bool,
            ) -> None:
                self.assertEqual(extracted_binary.read_bytes(), b"windows executable")
                self.assertEqual(extracted_binary.name, "agentic-navigation-guide.exe")

            with mock.patch.object(
                RELEASE,
                "smoke_binary",
                side_effect=inspect_extracted,
            ):
                RELEASE.smoke_archive(archive, "v0.2.0")

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

    def test_bundle_rejects_wrong_archive_count_and_missing_crate(self) -> None:
        def prepare(directory: Path, names: list[str]) -> tuple[Path, Path, Path]:
            for name in names:
                (directory / name).write_bytes(name.encode())
            checksums = directory / "SHA256SUMS"
            RELEASE.write_checksums(
                checksums,
                [directory / name for name in names],
            )
            sbom = directory / "agentic-navigation-guide-0.2.0.spdx.json"
            provenance = directory / "agentic-navigation-guide-0.2.0.intoto.json"
            provenance.write_text("{}\n", encoding="utf-8")
            return checksums, sbom, provenance

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            checksums, sbom, provenance = prepare(
                directory,
                [
                    "agentic-navigation-guide-0.2.0.crate",
                    "agentic-navigation-guide-0.2.0-linux.tar.gz",
                    "agentic-navigation-guide-0.2.0-macos.tar.gz",
                    "agentic-navigation-guide-0.2.0.spdx.json",
                ],
            )
            with self.assertRaisesRegex(
                RELEASE.ReleaseError,
                "must contain 3 native archives",
            ):
                RELEASE.verify_bundle(
                    directory,
                    checksums,
                    sbom,
                    provenance,
                    "v0.2.0",
                    COMMIT,
                    "refs/heads/main",
                )

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            checksums, sbom, provenance = prepare(
                directory,
                [
                    "agentic-navigation-guide-0.2.0-linux.tar.gz",
                    "agentic-navigation-guide-0.2.0-macos.tar.gz",
                    "agentic-navigation-guide-0.2.0-windows.zip",
                    "agentic-navigation-guide-0.2.0.spdx.json",
                ],
            )
            with self.assertRaisesRegex(
                RELEASE.ReleaseError,
                "missing agentic-navigation-guide-0.2.0.crate",
            ):
                RELEASE.verify_bundle(
                    directory,
                    checksums,
                    sbom,
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

    def test_sbom_verifier_rejects_each_required_identity_boundary(self) -> None:
        valid = {
            "spdxVersion": "SPDX-2.3",
            "documentNamespace": (
                "https://github.com/plx/agentic-navigation-guide/sbom/0.2.0/"
                f"{COMMIT}"
            ),
            "packages": [
                {
                    "name": "agentic-navigation-guide",
                    "versionInfo": "0.2.0",
                }
            ],
            "relationships": [{"relationshipType": "DESCRIBES"}],
        }
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sbom.json"
            RELEASE.write_json(path, valid)
            RELEASE.verify_sbom(path, COMMIT)
            mutations = {
                "SPDX 2.3": {"spdxVersion": "SPDX-2.2"},
                "candidate commit": {"documentNamespace": "https://example.invalid"},
                "root package": {"packages": []},
                "dependency relationships": {"relationships": []},
            }
            for expected_error, mutation in mutations.items():
                with self.subTest(expected_error=expected_error):
                    invalid = dict(valid)
                    invalid.update(mutation)
                    RELEASE.write_json(path, invalid)
                    with self.assertRaisesRegex(
                        RELEASE.ReleaseError,
                        expected_error,
                    ):
                        RELEASE.verify_sbom(path, COMMIT)

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
        self.assertIn('[[ "$RUNNER_OS" == "Windows" ]]', before_publish)
        self.assertIn("-C link-arg=/Brepro", before_publish)

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
        RELEASE.check_pipeline_workflow()

    def test_pipeline_rejects_identity_drift_at_runtime(self) -> None:
        drifted = RELEASE.load_toml(RELEASE.PIPELINE_PATH)
        drifted["candidate_tag"] = "v9.9.9"
        real_load_toml = RELEASE.load_toml

        def load_with_drift(path: Path):
            if path == RELEASE.PIPELINE_PATH:
                return drifted
            return real_load_toml(path)

        with (
            mock.patch.object(
                RELEASE,
                "load_toml",
                side_effect=load_with_drift,
            ),
            self.assertRaisesRegex(
                RELEASE.ReleaseError,
                "candidate_tag",
            ),
        ):
            RELEASE.pipeline()

    def test_crates_version_state_is_fail_closed_and_recoverable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            archive = (
                Path(temporary)
                / "agentic-navigation-guide-0.2.0.crate"
            )
            archive.write_bytes(b"exact crate bytes")
            checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
            def raise_not_found(*_args, **_kwargs):
                error = urllib.error.HTTPError(
                    "https://crates.io/example",
                    404,
                    "not found",
                    {},
                    io.BytesIO(),
                )
                error.close()
                raise error

            with mock.patch.object(
                RELEASE.urllib.request,
                "urlopen",
                side_effect=raise_not_found,
            ):
                self.assertEqual(
                    RELEASE.crates_version_state(archive),
                    "publish-required",
                )

            matching = io.BytesIO(
                json.dumps({"version": {"checksum": checksum}}).encode()
            )
            with mock.patch.object(
                RELEASE.urllib.request,
                "urlopen",
                return_value=matching,
            ):
                self.assertEqual(
                    RELEASE.crates_version_state(archive),
                    "already-published-matching",
                )

            mismatched = io.BytesIO(
                json.dumps({"version": {"checksum": "0" * 64}}).encode()
            )
            with (
                mock.patch.object(
                    RELEASE.urllib.request,
                    "urlopen",
                    return_value=mismatched,
                ),
                self.assertRaisesRegex(
                    RELEASE.ReleaseError,
                    "different archive checksum",
                ),
            ):
                RELEASE.crates_version_state(archive)


if __name__ == "__main__":
    unittest.main()
