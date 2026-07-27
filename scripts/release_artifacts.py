#!/usr/bin/env python3
"""Build and verify deterministic, traceable release artifacts."""

from __future__ import annotations

import argparse
import datetime
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from typing import Any, Iterable, Mapping, Sequence
import urllib.error
import urllib.request
import zipfile


ROOT = Path(__file__).resolve().parents[1]
IDENTITY_PATH = ROOT / "release" / "identity.toml"
PIPELINE_PATH = ROOT / "release" / "pipeline.toml"
MANIFEST_PATH = ROOT / "Cargo.toml"
LOCKFILE_PATH = ROOT / "Cargo.lock"
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
SPDX_RE = re.compile(r"[^A-Za-z0-9.-]+")
EXPECTED_ARCHIVE_COUNT = 3
EXPECTED_DISTRIBUTION_COUNT = 4


class ReleaseError(RuntimeError):
    """A fail-closed release-artifact validation error."""


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def identity() -> dict[str, Any]:
    value = load_toml(IDENTITY_PATH)
    required = {
        "package",
        "version",
        "binary",
        "tag_prefix",
        "license",
        "supported_product",
        "linkable_rust_targets",
    }
    missing = sorted(required.difference(value))
    if missing:
        raise ReleaseError(f"release identity is missing keys: {', '.join(missing)}")
    return value


def pipeline() -> dict[str, Any]:
    value = load_toml(PIPELINE_PATH)
    if value.get("schema") != 1:
        raise ReleaseError("release pipeline schema must be 1")
    release_identity = identity()
    manifest = load_toml(MANIFEST_PATH).get("package", {})
    repository_url = str(manifest.get("repository", ""))
    repository_prefix = "https://github.com/"
    if not repository_url.startswith(repository_prefix):
        raise ReleaseError("Cargo package repository must be a GitHub HTTPS URL")
    manifest_repository = repository_url.removeprefix(repository_prefix).removesuffix(
        ".git"
    )
    repository = str(value.get("repository", ""))
    owner, separator, repository_name = repository.partition("/")
    expected = {
        "repository": manifest_repository,
        "candidate_tag": expected_tag(release_identity),
        "crate": release_identity["package"],
        "binary": release_identity["binary"],
        "trusted_publisher_owner": owner,
        "trusted_publisher_repository": repository_name,
        "trusted_publisher_workflow": value.get("workflow_filename"),
        "trusted_publisher_environment": value.get("environment"),
    }
    for key, expected_value in expected.items():
        if value.get(key) != expected_value:
            raise ReleaseError(
                f"release pipeline {key}: expected {expected_value!r}, "
                f"observed {value.get(key)!r}"
            )
    if not separator or not owner or not repository_name:
        raise ReleaseError("release pipeline repository must be owner/name")
    workflow_filename = str(value.get("workflow_filename", ""))
    if (
        PurePosixPath(workflow_filename).name != workflow_filename
        or not workflow_filename.endswith(".yml")
    ):
        raise ReleaseError("release pipeline workflow filename must be one .yml basename")
    if value.get("supported_runners") != [
        "ubuntu-latest",
        "macos-latest",
        "windows-latest",
    ]:
        raise ReleaseError("release pipeline supported runner matrix drifted")
    if value.get("reproducibility_claim") != (
        "same-runner-release-binary-byte-for-byte"
    ):
        raise ReleaseError("release pipeline reproducibility claim drifted")
    return value


def check_pipeline_workflow() -> None:
    configuration = pipeline()
    workflow_path = ROOT / ".github" / "workflows" / configuration["workflow_filename"]
    workflow = workflow_path.read_text(encoding="utf-8")
    matrix = ", ".join(configuration["supported_runners"])
    required_fragments = {
        "candidate tag default": f"default: {configuration['candidate_tag']}",
        "protected environment": (
            f"environment:\n      name: {configuration['environment']}"
        ),
        "supported runner matrix": f"os: [{matrix}]",
        "Windows reproducibility linker": "-C link-arg=/Brepro",
        "Trusted Publisher action": "rust-lang/crates-io-auth-action@",
        "immutable release assertion": ".immutable == true",
    }
    for label, fragment in required_fragments.items():
        if fragment not in workflow:
            raise ReleaseError(
                f"release workflow omits configured {label}: {fragment!r}"
            )
    if workflow.count(required_fragments["supported runner matrix"]) != 2:
        raise ReleaseError(
            "release workflow must use the configured runner matrix for tests "
            "and native archives"
        )


def expected_tag(release_identity: Mapping[str, Any]) -> str:
    return f"{release_identity['tag_prefix']}{release_identity['version']}"


def require_tag(tag: str, release_identity: Mapping[str, Any]) -> None:
    expected = expected_tag(release_identity)
    if tag != expected:
        raise ReleaseError(f"release tag mismatch: expected {expected!r}, observed {tag!r}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalized_member(name: str) -> PurePosixPath:
    member = PurePosixPath(name)
    if member.is_absolute() or ".." in member.parts or not member.parts:
        raise ReleaseError(f"unsafe archive member {name!r}")
    return member


def archive_entries(
    binary: Path,
    release_identity: Mapping[str, Any],
    host_triple: str,
) -> list[tuple[str, bytes, int]]:
    configuration = pipeline()
    root_name = (
        f"{release_identity['package']}-{release_identity['version']}-{host_triple}"
    )
    entries = [
        (
            f"{root_name}/{binary.name}",
            binary.read_bytes(),
            0o755,
        )
    ]
    for relative in configuration["archive_license_files"]:
        source = ROOT / relative
        if not source.is_file():
            raise ReleaseError(f"required archive file is missing: {relative}")
        entries.append((f"{root_name}/{relative}", source.read_bytes(), 0o644))
    return sorted(entries)


def write_tar_gz(output: Path, entries: Iterable[tuple[str, bytes, int]]) -> None:
    with output.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as tar:
                for name, contents, mode in entries:
                    normalized_member(name)
                    info = tarfile.TarInfo(name)
                    info.size = len(contents)
                    info.mode = mode
                    info.mtime = 0
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    tar.addfile(info, io.BytesIO(contents))


def write_zip(output: Path, entries: Iterable[tuple[str, bytes, int]]) -> None:
    with zipfile.ZipFile(
        output,
        mode="w",
        compression=zipfile.ZIP_DEFLATED,
        compresslevel=9,
    ) as archive:
        for name, contents, mode in entries:
            normalized_member(name)
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = mode << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, contents, compresslevel=9)


def create_archive(
    binary: Path,
    comparison_binary: Path,
    host_triple: str,
    tag: str,
    output_directory: Path,
) -> Path:
    release_identity = identity()
    require_tag(tag, release_identity)
    for label, path in (
        ("release binary", binary),
        ("comparison release binary", comparison_binary),
    ):
        if not path.is_file():
            raise ReleaseError(f"{label} does not exist: {path}")
    first_hash = sha256_file(binary)
    second_hash = sha256_file(comparison_binary)
    if first_hash != second_hash:
        raise ReleaseError(
            "same-runner release binary is not byte reproducible: "
            f"{first_hash} != {second_hash}"
        )
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", host_triple):
        raise ReleaseError(f"invalid host triple {host_triple!r}")

    output_directory.mkdir(parents=True, exist_ok=True)
    stem = (
        f"{release_identity['package']}-{release_identity['version']}-{host_triple}"
    )
    entries = archive_entries(binary, release_identity, host_triple)
    if "windows" in host_triple:
        if binary.suffix.lower() != ".exe":
            raise ReleaseError("a Windows release binary must have an .exe suffix")
        output = output_directory / f"{stem}.zip"
        write_zip(output, entries)
    else:
        if binary.suffix:
            raise ReleaseError("a non-Windows release binary must not have a suffix")
        output = output_directory / f"{stem}.tar.gz"
        write_tar_gz(output, entries)
    return output


def archive_members(archive: Path) -> list[str]:
    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, mode="r:gz") as handle:
            names = handle.getnames()
    elif archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as handle:
            names = handle.namelist()
    else:
        raise ReleaseError(f"unsupported release archive {archive.name!r}")
    return [str(normalized_member(name)) for name in names]


def extract_archive(archive: Path, destination: Path) -> None:
    members = archive_members(archive)
    if len(members) != len(set(members)):
        raise ReleaseError(f"release archive contains duplicate members: {archive}")
    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, mode="r:gz") as handle:
            handle.extractall(destination, filter="data")
    else:
        with zipfile.ZipFile(archive) as handle:
            handle.extractall(destination)


def expected_version_line(release_identity: Mapping[str, Any]) -> str:
    return f"{release_identity['binary']} {release_identity['version']}"


def smoke_binary(binary: Path, tag: str, inject_failure: bool = False) -> None:
    release_identity = identity()
    require_tag(tag, release_identity)
    if not binary.is_file():
        raise ReleaseError(f"smoke-test binary does not exist: {binary}")
    expected_version = expected_version_line(release_identity)
    if inject_failure:
        expected_version += "-injected-package-smoke-failure"

    version = subprocess.run(
        [str(binary), "--version"],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if version.returncode != 0:
        raise ReleaseError(
            f"release binary --version failed with {version.returncode}: "
            f"{version.stderr.strip()}"
        )
    if version.stdout.strip() != expected_version:
        raise ReleaseError(
            "release binary version mismatch: "
            f"expected {expected_version!r}, observed {version.stdout.strip()!r}"
        )

    invalid = subprocess.run(
        [str(binary), "__release_smoke_invalid_command__"],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if invalid.returncode != 2:
        raise ReleaseError(
            "release binary failure smoke returned "
            f"{invalid.returncode}, expected Clap usage status 2"
        )
    combined = f"{invalid.stdout}\n{invalid.stderr}"
    if "__release_smoke_invalid_command__" not in combined or "Usage:" not in combined:
        raise ReleaseError("release binary failure smoke omitted safe usage diagnostics")


def smoke_archive(archive: Path, tag: str, inject_failure: bool = False) -> None:
    release_identity = identity()
    require_tag(tag, release_identity)
    members = archive_members(archive)
    required_names = set(pipeline()["archive_license_files"])
    root_names = {PurePosixPath(member).parts[0] for member in members}
    if len(root_names) != 1:
        raise ReleaseError("release archive must contain exactly one root directory")
    root_name = next(iter(root_names))
    expected_root_prefix = (
        f"{release_identity['package']}-{release_identity['version']}-"
    )
    if not root_name.startswith(expected_root_prefix):
        raise ReleaseError(
            f"release archive root {root_name!r} does not match {expected_root_prefix!r}"
        )
    leaf_names = {str(PurePosixPath(member).relative_to(root_name)) for member in members}
    binary_name = str(release_identity["binary"])
    if archive.suffix == ".zip":
        binary_name += ".exe"
    expected_members = required_names.union({binary_name})
    if leaf_names != expected_members:
        raise ReleaseError(
            "release archive member set mismatch: "
            f"expected {sorted(expected_members)!r}, observed {sorted(leaf_names)!r}"
        )

    with tempfile.TemporaryDirectory(prefix="release-archive-smoke-") as temporary:
        destination = Path(temporary)
        extract_archive(archive, destination)
        binary = destination / root_name / binary_name
        if os.name != "nt":
            binary.chmod(0o755)
        smoke_binary(binary, tag, inject_failure)


def checksum_lines(files: Sequence[Path]) -> list[str]:
    if not files:
        raise ReleaseError("at least one checksum subject is required")
    by_name: dict[str, Path] = {}
    for path in files:
        if not path.is_file():
            raise ReleaseError(f"checksum subject does not exist: {path}")
        if path.name in by_name:
            raise ReleaseError(f"duplicate checksum subject name: {path.name}")
        if "\n" in path.name or "\r" in path.name:
            raise ReleaseError(f"unsafe checksum subject name: {path.name!r}")
        by_name[path.name] = path
    return [f"{sha256_file(by_name[name])}  {name}" for name in sorted(by_name)]


def write_checksums(output: Path, files: Sequence[Path]) -> None:
    lines = checksum_lines(files)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def parse_checksums(path: Path) -> dict[str, str]:
    checksums: dict[str, str] = {}
    for line_number, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(),
        start=1,
    ):
        digest, separator, name = line.partition("  ")
        if not separator or not SHA256_RE.fullmatch(digest):
            raise ReleaseError(f"invalid checksum line {line_number}")
        member = normalized_member(name)
        if len(member.parts) != 1:
            raise ReleaseError(f"checksum subject must be a basename: {name!r}")
        if name in checksums:
            raise ReleaseError(f"duplicate checksum subject: {name}")
        checksums[name] = digest
    if not checksums:
        raise ReleaseError("checksum manifest is empty")
    return checksums


def verify_checksums(directory: Path, checksums_path: Path) -> dict[str, str]:
    checksums = parse_checksums(checksums_path)
    for name, expected in checksums.items():
        subject = directory / name
        if not subject.is_file():
            raise ReleaseError(f"checksum subject is missing: {name}")
        observed = sha256_file(subject)
        if observed != expected:
            raise ReleaseError(
                f"checksum mismatch for {name}: expected {expected}, observed {observed}"
            )
    return checksums


def spdx_id(*parts: str) -> str:
    value = "-".join(parts)
    normalized = SPDX_RE.sub("-", value).strip("-")
    if not normalized:
        normalized = hashlib.sha256(value.encode()).hexdigest()
    return f"SPDXRef-{normalized}"


def source_date() -> str:
    raw_epoch = os.environ.get("SOURCE_DATE_EPOCH", "0")
    try:
        epoch = int(raw_epoch)
    except ValueError as error:
        raise ReleaseError("SOURCE_DATE_EPOCH must be an integer") from error
    if epoch < 0:
        raise ReleaseError("SOURCE_DATE_EPOCH must not be negative")
    return datetime.datetime.fromtimestamp(
        epoch,
        tz=datetime.timezone.utc,
    ).strftime("%Y-%m-%dT%H:%M:%SZ")


def cargo_metadata() -> dict[str, Any]:
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=120,
    )
    if result.returncode != 0:
        raise ReleaseError(f"cargo metadata failed: {result.stderr.strip()}")
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ReleaseError("cargo metadata returned invalid JSON") from error


def lock_checksums() -> dict[tuple[str, str], str]:
    lock = load_toml(LOCKFILE_PATH)
    result: dict[tuple[str, str], str] = {}
    for package in lock.get("package", []):
        checksum = package.get("checksum")
        if isinstance(checksum, str) and SHA256_RE.fullmatch(checksum):
            result[(str(package["name"]), str(package["version"]))] = checksum
    return result


def build_sbom(commit: str) -> dict[str, Any]:
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ReleaseError("SBOM commit must be a full lowercase Git SHA")
    metadata = cargo_metadata()
    release_identity = identity()
    root_id = metadata.get("resolve", {}).get("root")
    packages = metadata.get("packages", [])
    package_by_id = {str(package["id"]): package for package in packages}
    if root_id not in package_by_id:
        raise ReleaseError("cargo metadata has no root package")
    lock_hashes = lock_checksums()
    document_namespace = (
        "https://github.com/plx/agentic-navigation-guide/"
        f"sbom/{release_identity['version']}/{commit}"
    )
    spdx_packages = []
    id_map: dict[str, str] = {}
    for package_id in sorted(package_by_id):
        package = package_by_id[package_id]
        name = str(package["name"])
        version = str(package["version"])
        source = package.get("source")
        package_spdx_id = spdx_id(name, version, hashlib.sha256(package_id.encode()).hexdigest()[:12])
        id_map[package_id] = package_spdx_id
        item: dict[str, Any] = {
            "SPDXID": package_spdx_id,
            "name": name,
            "versionInfo": version,
            "downloadLocation": str(source) if source else "NOASSERTION",
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": package.get("license") or "NOASSERTION",
            "copyrightText": "NOASSERTION",
            "externalRefs": [
                {
                    "referenceCategory": "PACKAGE-MANAGER",
                    "referenceType": "purl",
                    "referenceLocator": f"pkg:cargo/{name}@{version}",
                }
            ],
        }
        checksum = lock_hashes.get((name, version))
        if checksum:
            item["checksums"] = [{"algorithm": "SHA256", "checksumValue": checksum}]
        spdx_packages.append(item)

    relationships = [
        {
            "spdxElementId": "SPDXRef-DOCUMENT",
            "relationshipType": "DESCRIBES",
            "relatedSpdxElement": id_map[str(root_id)],
        }
    ]
    for node in sorted(
        metadata.get("resolve", {}).get("nodes", []),
        key=lambda value: str(value["id"]),
    ):
        source_id = str(node["id"])
        if source_id not in id_map:
            continue
        for dependency in sorted(str(value) for value in node.get("dependencies", [])):
            if dependency in id_map:
                relationships.append(
                    {
                        "spdxElementId": id_map[source_id],
                        "relationshipType": "DEPENDS_ON",
                        "relatedSpdxElement": id_map[dependency],
                    }
                )

    return {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"{release_identity['package']}-{release_identity['version']}",
        "documentNamespace": document_namespace,
        "creationInfo": {
            "created": source_date(),
            "creators": ["Tool: scripts/release_artifacts.py"],
            "licenseListVersion": "3.27",
        },
        "documentDescribes": [id_map[str(root_id)]],
        "packages": spdx_packages,
        "relationships": relationships,
    }


def write_json(path: Path, value: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def build_provenance(
    checksums_path: Path,
    commit: str,
    ref: str,
    run_id: str,
    run_attempt: str,
) -> dict[str, Any]:
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ReleaseError("provenance commit must be a full lowercase Git SHA")
    if not ref.startswith("refs/"):
        raise ReleaseError("provenance ref must start with refs/")
    release_identity = identity()
    subjects = [
        {"name": name, "digest": {"sha256": digest}}
        for name, digest in sorted(parse_checksums(checksums_path).items())
    ]
    return {
        "_type": "https://in-toto.io/Statement/v1",
        "subject": subjects,
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": (
                    "https://github.com/plx/agentic-navigation-guide/"
                    "blob/main/docs/release-policy.md#release-workflow"
                ),
                "externalParameters": {
                    "ref": ref,
                    "tag": expected_tag(release_identity),
                    "workflow": pipeline()["workflow_filename"],
                },
                "internalParameters": {
                    "runId": str(run_id),
                    "runAttempt": str(run_attempt),
                },
                "resolvedDependencies": [
                    {
                        "uri": (
                            "git+https://github.com/plx/"
                            "agentic-navigation-guide.git"
                        ),
                        "digest": {"gitCommit": commit},
                    }
                ],
            },
            "runDetails": {
                "builder": {
                    "id": (
                        "https://github.com/plx/agentic-navigation-guide/"
                        "actions/workflows/release.yml"
                    )
                },
                "metadata": {
                    "invocationId": (
                        "https://github.com/plx/agentic-navigation-guide/"
                        f"actions/runs/{run_id}/attempts/{run_attempt}"
                    )
                },
            },
        },
    }


def verify_sbom(path: Path, commit: str) -> None:
    value = json.loads(path.read_text(encoding="utf-8"))
    release_identity = identity()
    if value.get("spdxVersion") != "SPDX-2.3":
        raise ReleaseError("release SBOM is not SPDX 2.3")
    expected_namespace_suffix = f"/{release_identity['version']}/{commit}"
    if not str(value.get("documentNamespace", "")).endswith(
        expected_namespace_suffix
    ):
        raise ReleaseError("release SBOM does not identify the candidate commit")
    root_packages = [
        package
        for package in value.get("packages", [])
        if package.get("name") == release_identity["package"]
        and package.get("versionInfo") == release_identity["version"]
    ]
    if len(root_packages) != 1:
        raise ReleaseError("release SBOM must describe the exact root package once")
    if not value.get("relationships"):
        raise ReleaseError("release SBOM has no dependency relationships")


def verify_provenance(
    path: Path,
    checksums: Mapping[str, str],
    commit: str,
    ref: str,
) -> None:
    value = json.loads(path.read_text(encoding="utf-8"))
    if value.get("_type") != "https://in-toto.io/Statement/v1":
        raise ReleaseError("release provenance is not an in-toto v1 statement")
    if value.get("predicateType") != "https://slsa.dev/provenance/v1":
        raise ReleaseError("release provenance is not a SLSA v1 predicate")
    subjects = {
        str(subject.get("name")): str(subject.get("digest", {}).get("sha256"))
        for subject in value.get("subject", [])
    }
    if subjects != dict(checksums):
        raise ReleaseError("release provenance subjects differ from checksums")
    definition = value.get("predicate", {}).get("buildDefinition", {})
    dependencies = definition.get("resolvedDependencies", [])
    if dependencies != [
        {
            "uri": "git+https://github.com/plx/agentic-navigation-guide.git",
            "digest": {"gitCommit": commit},
        }
    ]:
        raise ReleaseError("release provenance does not resolve the exact commit")
    if definition.get("externalParameters", {}).get("ref") != ref:
        raise ReleaseError("release provenance does not resolve the exact ref")


def verify_bundle(
    directory: Path,
    checksums_path: Path,
    sbom_path: Path,
    provenance_path: Path,
    tag: str,
    commit: str,
    ref: str,
) -> None:
    release_identity = identity()
    require_tag(tag, release_identity)
    checksums = verify_checksums(directory, checksums_path)
    observed_files = {
        entry.name
        for entry in directory.iterdir()
        if entry.is_file()
    }
    unexpected_non_files = sorted(
        entry.name
        for entry in directory.iterdir()
        if not entry.is_file()
    )
    expected_files = set(checksums).union(
        {
            checksums_path.name,
            provenance_path.name,
        }
    )
    if observed_files != expected_files or unexpected_non_files:
        raise ReleaseError(
            "release bundle file set mismatch: "
            f"expected {sorted(expected_files)!r}, "
            f"observed files {sorted(observed_files)!r}, "
            f"non-files {unexpected_non_files!r}"
        )
    archive_names = [
        name
        for name in checksums
        if name.endswith(".tar.gz") or name.endswith(".zip")
    ]
    crate_name = f"{release_identity['package']}-{release_identity['version']}.crate"
    if len(archive_names) != EXPECTED_ARCHIVE_COUNT:
        raise ReleaseError(
            f"release bundle must contain {EXPECTED_ARCHIVE_COUNT} native archives"
        )
    if crate_name not in checksums:
        raise ReleaseError(f"release bundle is missing {crate_name}")
    if len(checksums) != EXPECTED_DISTRIBUTION_COUNT + 1:
        raise ReleaseError(
            "release checksums must cover three native archives, one crate, and one SBOM"
        )
    if sbom_path.name not in checksums:
        raise ReleaseError("release checksums do not cover the SBOM")
    verify_sbom(sbom_path, commit)
    verify_provenance(provenance_path, checksums, commit, ref)
    for archive_name in archive_names:
        members = archive_members(directory / archive_name)
        for required in pipeline()["archive_license_files"]:
            if not any(member.endswith(f"/{required}") for member in members):
                raise ReleaseError(f"{archive_name} omits required {required}")


def crates_version_state(crate_archive: Path) -> str:
    release_identity = identity()
    expected_name = (
        f"{release_identity['package']}-{release_identity['version']}.crate"
    )
    if crate_archive.name != expected_name:
        raise ReleaseError(
            f"crate archive must be named {expected_name!r}, observed {crate_archive.name!r}"
        )
    url = (
        "https://crates.io/api/v1/crates/"
        f"{release_identity['package']}/{release_identity['version']}"
    )
    request = urllib.request.Request(
        url,
        headers={
            "User-Agent": (
                "agentic-navigation-guide-release-workflow/0.2 "
                "(plxgithub@gmail.com)"
            )
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return "publish-required"
        raise ReleaseError(
            f"crates.io version lookup failed with HTTP {error.code}"
        ) from error
    except urllib.error.URLError as error:
        raise ReleaseError(f"crates.io version lookup failed: {error.reason}") from error
    published = payload.get("version", {}).get("checksum")
    observed = sha256_file(crate_archive)
    if published != observed:
        raise ReleaseError(
            "crates.io already contains this version with a different archive checksum"
        )
    return "already-published-matching"


def output_github(name: str, value: str) -> None:
    output_path = os.environ.get("GITHUB_OUTPUT")
    if output_path:
        with Path(output_path).open("a", encoding="utf-8", newline="\n") as handle:
            handle.write(f"{name}={value}\n")


def command_archive(arguments: argparse.Namespace) -> None:
    archive = create_archive(
        arguments.binary,
        arguments.comparison_binary,
        arguments.host_triple,
        arguments.tag,
        arguments.output_directory,
    )
    output_github("archive", archive.name)
    print(
        json.dumps(
            {
                "archive": archive.name,
                "sha256": sha256_file(archive),
                "reproducible": True,
            },
            sort_keys=True,
        )
    )


def command_host_triple(_: argparse.Namespace) -> None:
    result = subprocess.run(
        ["rustc", "-vV"],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        raise ReleaseError(f"rustc -vV failed: {result.stderr.strip()}")
    host_lines = [
        line.removeprefix("host: ").strip()
        for line in result.stdout.splitlines()
        if line.startswith("host: ")
    ]
    if len(host_lines) != 1:
        raise ReleaseError("rustc -vV did not report exactly one host triple")
    host = host_lines[0]
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", host):
        raise ReleaseError(f"rustc reported an invalid host triple {host!r}")
    output_github("host-triple", host)
    print(host)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    check_config = commands.add_parser("check-config")
    check_config.set_defaults(handler=lambda _: check_pipeline_workflow())

    host = commands.add_parser("host-triple")
    host.set_defaults(handler=command_host_triple)

    archive = commands.add_parser("archive")
    archive.add_argument("--binary", type=Path, required=True)
    archive.add_argument("--comparison-binary", type=Path, required=True)
    archive.add_argument("--host-triple", required=True)
    archive.add_argument("--tag", required=True)
    archive.add_argument("--output-directory", type=Path, required=True)
    archive.set_defaults(handler=command_archive)

    smoke_binary_parser = commands.add_parser("smoke-binary")
    smoke_binary_parser.add_argument("--binary", type=Path, required=True)
    smoke_binary_parser.add_argument("--tag", required=True)
    smoke_binary_parser.add_argument("--inject-failure", action="store_true")
    smoke_binary_parser.set_defaults(
        handler=lambda arguments: smoke_binary(
            arguments.binary,
            arguments.tag,
            arguments.inject_failure,
        )
    )

    smoke_archive_parser = commands.add_parser("smoke-archive")
    smoke_archive_parser.add_argument("--archive", type=Path, required=True)
    smoke_archive_parser.add_argument("--tag", required=True)
    smoke_archive_parser.add_argument("--inject-failure", action="store_true")
    smoke_archive_parser.set_defaults(
        handler=lambda arguments: smoke_archive(
            arguments.archive,
            arguments.tag,
            arguments.inject_failure,
        )
    )

    checksums = commands.add_parser("checksums")
    checksums.add_argument("--output", type=Path, required=True)
    checksums.add_argument("files", nargs="+", type=Path)
    checksums.set_defaults(
        handler=lambda arguments: write_checksums(arguments.output, arguments.files)
    )

    verify = commands.add_parser("verify-checksums")
    verify.add_argument("--directory", type=Path, required=True)
    verify.add_argument("--checksums", type=Path, required=True)
    verify.set_defaults(
        handler=lambda arguments: verify_checksums(
            arguments.directory,
            arguments.checksums,
        )
    )

    sbom = commands.add_parser("sbom")
    sbom.add_argument("--commit", required=True)
    sbom.add_argument("--output", type=Path, required=True)
    sbom.set_defaults(
        handler=lambda arguments: write_json(
            arguments.output,
            build_sbom(arguments.commit),
        )
    )

    provenance = commands.add_parser("provenance")
    provenance.add_argument("--checksums", type=Path, required=True)
    provenance.add_argument("--commit", required=True)
    provenance.add_argument("--ref", required=True)
    provenance.add_argument("--run-id", required=True)
    provenance.add_argument("--run-attempt", required=True)
    provenance.add_argument("--output", type=Path, required=True)
    provenance.set_defaults(
        handler=lambda arguments: write_json(
            arguments.output,
            build_provenance(
                arguments.checksums,
                arguments.commit,
                arguments.ref,
                arguments.run_id,
                arguments.run_attempt,
            ),
        )
    )

    bundle = commands.add_parser("verify-bundle")
    bundle.add_argument("--directory", type=Path, required=True)
    bundle.add_argument("--checksums", type=Path, required=True)
    bundle.add_argument("--sbom", type=Path, required=True)
    bundle.add_argument("--provenance", type=Path, required=True)
    bundle.add_argument("--tag", required=True)
    bundle.add_argument("--commit", required=True)
    bundle.add_argument("--ref", required=True)
    bundle.set_defaults(
        handler=lambda arguments: verify_bundle(
            arguments.directory,
            arguments.checksums,
            arguments.sbom,
            arguments.provenance,
            arguments.tag,
            arguments.commit,
            arguments.ref,
        )
    )

    state = commands.add_parser("crates-version-state")
    state.add_argument("--crate-archive", type=Path, required=True)

    def handle_state(arguments: argparse.Namespace) -> None:
        value = crates_version_state(arguments.crate_archive)
        output_github("state", value)
        print(value)

    state.set_defaults(handler=handle_state)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    arguments = parser.parse_args(argv)
    try:
        arguments.handler(arguments)
    except (
        ReleaseError,
        OSError,
        json.JSONDecodeError,
        subprocess.SubprocessError,
        tarfile.TarError,
        zipfile.BadZipFile,
    ) as error:
        print(f"release artifact validation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
