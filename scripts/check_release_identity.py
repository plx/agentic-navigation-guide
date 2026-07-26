#!/usr/bin/env python3
"""Fail closed when the prepared release identity is internally inconsistent."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from typing import Any, Mapping, Sequence


LINKABLE_TARGET_KINDS = {
    "lib",
    "rlib",
    "dylib",
    "cdylib",
    "staticlib",
    "proc-macro",
}
PUBLISHED_API_FIXTURE = Path("tests/fixtures/v0_1_4_published_api.tsv")
EXPECTED_IDENTITY = {
    "schema": 1,
    "package": "agentic-navigation-guide",
    "version": "0.2.0",
    "binary": "agentic-navigation-guide",
    "tag_prefix": "v",
    "changelog_status": "Unreleased",
    "license": "MIT OR Apache-2.0",
    "supported_product": "cli",
    "linkable_rust_targets": 0,
}
EXPECTED_PUBLISHED_METADATA = {
    "published_version": "0.1.4",
    "artifact_sha256": "d08fefac88faf8d737eea273f86bfbc80aaac1eb80ff3a57bde5add824fe5da0",
    "vcs_revision": "560ce399e1e28e8e0d6b87988956893796d2dfab",
    "normalized_manifest_sha256": (
        "1dc83730531459a1fcae387cc5e5f625a3ff498659915d58fa875dd14c9fab3b"
    ),
    "library_source_sha256": (
        "c2107c1948025e592e4af33a39b8f80ce7f02b8160d48c12acf6a4c67963d656"
    ),
    "ordered_id_sha256": (
        "3b1fa66f32a32aa48430993d9e69a7fa0b9566942efd17f8dfe657b6d1e8ddb7"
    ),
    "ordered_symbol_sha256": (
        "7d6f9b7f320cb6394bfbf4b54657e4bddece662b15cc5b24cd1e409aab39ef88"
    ),
    "ordered_row_sha256": (
        "ab476288fae6998d16ee2a500825cf04a26b5564c3e59a9ed95824ed0193611f"
    ),
}
EXPECTED_MIGRATION_EVIDENCE = {
    "published_version": "0.1.4",
    "artifact_sha256": EXPECTED_PUBLISHED_METADATA["artifact_sha256"],
    "vcs_revision": EXPECTED_PUBLISHED_METADATA["vcs_revision"],
    "normalized_manifest_sha256": EXPECTED_PUBLISHED_METADATA[
        "normalized_manifest_sha256"
    ],
    "library_source_sha256": EXPECTED_PUBLISHED_METADATA[
        "library_source_sha256"
    ],
    "published_api_fixture": str(PUBLISHED_API_FIXTURE),
    "last_linkable_revision": "e34399c14683878064cad18e9506186cd7e4fef1",
    "semver_report_audit": "audits/2026-07-26-issue-54-binary-only-package.md",
    "semver_report_audit_sha256": (
        "b7db882a03c3f19bf2b194c6fcf2f1ab504a99cbd8425cbde6327d86ad7c2313"
    ),
    "semver_checks_version": "0.49.0",
    "semver_checks_executable_sha256": (
        "dd13a57b19aaedcb9d520f3d0cfc6af0005c04b4e1521ac9d81cdc513a13ec16"
    ),
    "rust_version": "1.93.0",
    "cargo_version": "1.93.0",
    "exit_code": 100,
    "evaluated_checks": 196,
    "passed_checks": 192,
    "major_failures": 4,
    "inapplicable_checks": 57,
    "major_lints": [
        "enum_no_repr_variant_discriminant_changed",
        "enum_variant_added",
        "enum_variant_missing",
        "inherent_method_missing",
    ],
}
EXPECTED_FUTURE_COMPATIBILITY = {
    "compatibility_line": "0.2.x",
    "next_breaking_version": "0.3.0",
    "compatible_baseline_policy": (
        "most-recent-preceding-non-yanked-release-in-the-same-compatibility-line"
    ),
    "breaking_line_baseline_policy": (
        "latest-non-yanked-published-predecessor-plus-approved-migration-record"
    ),
    "comparison_surface": "documented-cli-contract-and-package-target-shape",
    "compatible_break_policy": (
        "fail-all-incompatibilities-release-notes-do-not-authorize"
    ),
    "security_exception": "restore-not-redefine",
}
EXPECTED_CATEGORY_COUNTS = {
    "PackageTarget": 1,
    "Module": 7,
    "ReExport": 17,
    "TypeAlias": 1,
    "Struct": 10,
    "Enum": 6,
    "Variant": 38,
    "Field": 19,
    "Function": 7,
    "Method": 22,
}


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def load_published_api_fixture(
    path: Path,
) -> tuple[dict[str, str], list[tuple[str, str, str]]]:
    metadata: dict[str, str] = {}
    rows: list[tuple[str, str, str]] = []
    saw_header = False
    for line_number, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(),
        start=1,
    ):
        if line.startswith("# "):
            key, separator, value = line[2:].partition("=")
            if not separator or not key or not value:
                raise ValueError(f"invalid metadata at line {line_number}")
            if key in metadata:
                raise ValueError(f"duplicate metadata key {key!r}")
            metadata[key] = value
            continue
        if line == "id|kind|symbol" and not saw_header and not rows:
            saw_header = True
            continue
        if not saw_header:
            raise ValueError(f"missing row header before line {line_number}")
        parts = line.split("|", 2)
        if len(parts) != 3 or not all(parts):
            raise ValueError(f"invalid API row at line {line_number}")
        rows.append((parts[0], parts[1], parts[2]))
    if not saw_header:
        raise ValueError("missing id|kind|symbol header")
    return metadata, rows


def sha256_lines(values: Sequence[str]) -> str:
    serialized = "".join(f"{value}\n" for value in values).encode()
    return hashlib.sha256(serialized).hexdigest()


def check_published_api_fixture(
    metadata: Mapping[str, str],
    rows: Sequence[tuple[str, str, str]],
) -> list[str]:
    failures: list[str] = []
    for key, expected in EXPECTED_PUBLISHED_METADATA.items():
        observed = metadata.get(key)
        if observed != expected:
            failures.append(
                f"published API metadata {key}: expected {expected!r}, "
                f"observed {observed!r}"
            )
    extra_metadata = sorted(set(metadata).difference(EXPECTED_PUBLISHED_METADATA))
    if extra_metadata:
        failures.append(
            "published API fixture has unrecognized metadata: "
            + ", ".join(extra_metadata)
        )
    if len(rows) != 128:
        failures.append(f"published API fixture must have 128 rows, observed {len(rows)}")

    ids = [row[0] for row in rows]
    kinds = [row[1] for row in rows]
    symbols = [row[2] for row in rows]
    if len(set(ids)) != len(ids):
        failures.append("published API fixture contains duplicate IDs")
    if len(set(symbols)) != len(symbols):
        failures.append("published API fixture contains duplicate symbols")
    observed_counts = dict(Counter(kinds))
    if observed_counts != EXPECTED_CATEGORY_COUNTS:
        failures.append(
            "published API category counts: "
            f"expected {EXPECTED_CATEGORY_COUNTS!r}, observed {observed_counts!r}"
        )
    for label, values in (("id", ids), ("symbol", symbols)):
        expected = EXPECTED_PUBLISHED_METADATA[f"ordered_{label}_sha256"]
        observed = sha256_lines(values)
        if observed != expected:
            failures.append(
                f"published API ordered {label} SHA-256: "
                f"expected {expected}, observed {observed}"
            )
    expected_rows_hash = EXPECTED_PUBLISHED_METADATA["ordered_row_sha256"]
    observed_rows_hash = sha256_lines(["|".join(row) for row in rows])
    if observed_rows_hash != expected_rows_hash:
        failures.append(
            "published API ordered row SHA-256: "
            f"expected {expected_rows_hash}, observed {observed_rows_hash}"
        )
    return failures


def expected_values(identity: Mapping[str, Any]) -> tuple[str, str, str]:
    version = str(identity.get("version", ""))
    tag = f"{identity.get('tag_prefix', '')}{version}"
    heading = f"## [{version}] - {identity.get('changelog_status', '')}"
    return version, tag, heading


def check_identity(
    identity: Mapping[str, Any],
    published_metadata: Mapping[str, str],
) -> list[str]:
    failures: list[str] = []
    expected_root_keys = set(EXPECTED_IDENTITY).union(
        {"migration_baseline", "future_compatibility"}
    )
    if set(identity) != expected_root_keys:
        failures.append(
            "release identity root keys: "
            f"expected {sorted(expected_root_keys)!r}, observed {sorted(identity)!r}"
        )
    for key, expected in EXPECTED_IDENTITY.items():
        observed = identity.get(key)
        if observed != expected:
            failures.append(
                f"release identity {key}: expected {expected!r}, observed {observed!r}"
            )

    migration = identity.get("migration_baseline")
    if not isinstance(migration, Mapping):
        failures.append("release identity migration_baseline must be a TOML table")
    else:
        if set(migration) != set(EXPECTED_MIGRATION_EVIDENCE):
            failures.append(
                "migration baseline keys: "
                f"expected {sorted(EXPECTED_MIGRATION_EVIDENCE)!r}, "
                f"observed {sorted(migration)!r}"
            )
        for key, expected in EXPECTED_MIGRATION_EVIDENCE.items():
            observed = migration.get(key)
            if observed != expected:
                failures.append(
                    f"migration baseline {key}: expected {expected!r}, "
                    f"observed {observed!r}"
                )
        for key in (
            "published_version",
            "artifact_sha256",
            "vcs_revision",
            "normalized_manifest_sha256",
            "library_source_sha256",
        ):
            if migration.get(key) != published_metadata.get(key):
                failures.append(
                    f"migration baseline {key} does not match the published API fixture"
                )

    future = identity.get("future_compatibility")
    if not isinstance(future, Mapping):
        failures.append("release identity future_compatibility must be a TOML table")
    else:
        if set(future) != set(EXPECTED_FUTURE_COMPATIBILITY):
            failures.append(
                "future compatibility keys: "
                f"expected {sorted(EXPECTED_FUTURE_COMPATIBILITY)!r}, "
                f"observed {sorted(future)!r}"
            )
        for key, expected in EXPECTED_FUTURE_COMPATIBILITY.items():
            observed = future.get(key)
            if observed != expected:
                failures.append(
                    f"future compatibility {key}: expected {expected!r}, "
                    f"observed {observed!r}"
                )
    return failures


def check_migration_evidence_files(
    identity: Mapping[str, Any],
    root: Path,
) -> list[str]:
    migration = identity.get("migration_baseline")
    if not isinstance(migration, Mapping):
        return []
    audit_relative = migration.get("semver_report_audit")
    expected_hash = migration.get("semver_report_audit_sha256")
    if not isinstance(audit_relative, str) or not isinstance(expected_hash, str):
        return []
    audit_path = root / audit_relative
    try:
        audit_bytes = audit_path.read_bytes().replace(b"\r\n", b"\n")
        observed_hash = hashlib.sha256(audit_bytes).hexdigest()
    except OSError as error:
        return [f"cannot read pinned SemVer report audit {audit_relative}: {error}"]
    if observed_hash != expected_hash:
        return [
            f"pinned SemVer report audit SHA-256: expected {expected_hash}, "
            f"observed {observed_hash}"
        ]
    return []


def check_manifest(
    manifest: Mapping[str, Any], identity: Mapping[str, Any]
) -> list[str]:
    failures: list[str] = []
    package = manifest.get("package")
    if not isinstance(package, Mapping):
        return ["Cargo.toml has no [package] table"]

    expected_version, _, _ = expected_values(identity)
    for key in ("name", "version", "license"):
        expected = identity.get({"name": "package"}.get(key, key))
        observed = package.get(key)
        if observed != expected:
            failures.append(
                f"Cargo.toml package.{key}: expected {expected!r}, observed {observed!r}"
            )

    binary = identity.get("binary")
    bins = manifest.get("bin")
    bin_entries = (
        [entry for entry in bins if isinstance(entry, Mapping)]
        if isinstance(bins, list)
        else []
    )
    exact_bins = [
        entry
        for entry in bin_entries
        if entry.get("name") == binary
        and entry.get("path") == "src/main.rs"
    ]
    if len(bin_entries) != 1 or len(exact_bins) != 1:
        failures.append(
            "Cargo.toml must declare only the intended [[bin]] "
            f"{binary!r} at src/main.rs; observed {len(bin_entries)} binaries"
        )
    return failures


def check_lock(lock: Mapping[str, Any], identity: Mapping[str, Any]) -> list[str]:
    expected_version, _, _ = expected_values(identity)
    package_name = identity.get("package")
    packages = lock.get("package")
    roots = [
        package
        for package in packages
        if isinstance(package, Mapping)
        and package.get("name") == package_name
        and "source" not in package
    ] if isinstance(packages, list) else []
    if len(roots) != 1:
        return [
            f"Cargo.lock must contain exactly one source-less root package {package_name!r}; "
            f"observed {len(roots)}"
        ]
    observed = roots[0].get("version")
    if observed != expected_version:
        return [
            f"Cargo.lock root version: expected {expected_version!r}, observed {observed!r}"
        ]
    return []


def check_tag(tag: str, identity: Mapping[str, Any]) -> list[str]:
    _, expected_tag, _ = expected_values(identity)
    if tag != expected_tag:
        return [f"release tag input: expected {expected_tag!r}, observed {tag!r}"]
    return []


def check_changelog(text: str | None, identity: Mapping[str, Any]) -> list[str]:
    version, _, expected_heading = expected_values(identity)
    if text is None:
        return [f"CHANGELOG.md is missing; expected heading {expected_heading!r}"]
    version_headings = [
        line
        for line in text.splitlines()
        if line.startswith(f"## [{version}]")
    ]
    if version_headings != [expected_heading]:
        return [
            f"CHANGELOG.md must contain only exact heading {expected_heading!r}; "
            f"observed {version_headings!r}"
        ]
    return []


def check_cli_output(output: str, identity: Mapping[str, Any]) -> list[str]:
    version, _, _ = expected_values(identity)
    expected = f"{identity.get('binary')} {version}"
    observed = output.strip()
    if observed != expected:
        return [f"built CLI --version: expected {expected!r}, observed {observed!r}"]
    return []


def check_cli_source(text: str, identity: Mapping[str, Any]) -> list[str]:
    binary = re.escape(str(identity.get("binary", "")))
    command = re.search(r"#\[command\((.*?)\)\]", text, flags=re.DOTALL)
    if command is None:
        return ["src/cli/mod.rs has no Clap command declaration"]
    body = command.group(1)
    failures: list[str] = []
    if re.search(rf'\bname\s*=\s*"{binary}"', body) is None:
        failures.append("Clap source does not derive the intended binary name")
    if re.search(r"(?:^|,)\s*version\s*(?:,|$)", body) is None:
        failures.append(
            "Clap source must use bare `version` so --version derives from Cargo"
        )
    if re.search(r"\bversion\s*=", body):
        failures.append("Clap source hard-codes a version instead of deriving it from Cargo")
    return failures


def run(
    arguments: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        arguments,
        cwd=cwd,
        env=dict(environment) if environment is not None else None,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def cargo_output(
    arguments: Sequence[str], *, root: Path
) -> tuple[str | None, list[str]]:
    result = run(["cargo", *arguments], cwd=root)
    if result.returncode != 0:
        rendered = " ".join(["cargo", *arguments])
        return None, [
            f"{rendered} failed with {result.returncode}: "
            f"{result.stderr.strip() or result.stdout.strip()}"
        ]
    return result.stdout, []


def built_executable(
    build_output: str,
    identity: Mapping[str, Any],
) -> tuple[Path | None, list[str]]:
    executables: list[Path] = []
    failures: list[str] = []
    for line_number, line in enumerate(build_output.splitlines(), start=1):
        try:
            message = json.loads(line)
        except json.JSONDecodeError as error:
            failures.append(
                f"cargo build JSON line {line_number} is invalid: {error}"
            )
            continue
        if message.get("reason") != "compiler-artifact":
            continue
        target = message.get("target")
        if not isinstance(target, Mapping):
            continue
        if (
            target.get("name") == identity.get("binary")
            and target.get("kind") == ["bin"]
            and target.get("crate_types") == ["bin"]
        ):
            executable = message.get("executable")
            if isinstance(executable, str) and executable:
                executables.append(Path(executable))
    if len(executables) != 1:
        failures.append(
            "cargo build must report exactly one executable for the intended binary; "
            f"observed {len(executables)}"
        )
        return None, failures
    if not executables[0].is_file():
        failures.append(
            f"cargo build reported a missing executable: {executables[0]}"
        )
        return None, failures
    return executables[0], failures


def check_metadata(
    metadata: Mapping[str, Any], identity: Mapping[str, Any], root: Path
) -> list[str]:
    failures: list[str] = []
    expected_version, _, _ = expected_values(identity)
    manifest_path = root.joinpath("Cargo.toml").resolve()
    packages = metadata.get("packages")
    root_packages = [
        package
        for package in packages
        if isinstance(package, Mapping)
        and Path(str(package.get("manifest_path", ""))).resolve() == manifest_path
    ] if isinstance(packages, list) else []
    if len(root_packages) != 1:
        return [
            "cargo metadata must contain exactly one package for the root manifest; "
            f"observed {len(root_packages)}"
        ]

    package = root_packages[0]
    for key, expected in (
        ("name", identity.get("package")),
        ("version", expected_version),
        ("license", identity.get("license")),
    ):
        observed = package.get(key)
        if observed != expected:
            failures.append(
                f"cargo metadata {key}: expected {expected!r}, observed {observed!r}"
            )

    targets = package.get("targets")
    target_list = targets if isinstance(targets, list) else []
    linkable = sorted(
        {
            kind
            for target in target_list
            if isinstance(target, Mapping)
            for key in ("kind", "crate_types")
            for kind in target.get(key, [])
            if kind in LINKABLE_TARGET_KINDS
        }
    )
    if linkable:
        failures.append(f"cargo metadata exposes linkable Rust targets: {linkable}")

    all_bins = [
        target
        for target in target_list
        if isinstance(target, Mapping) and target.get("kind") == ["bin"]
    ]
    product_bins = [
        target
        for target in all_bins
        if target.get("name") == identity.get("binary")
        and target.get("crate_types") == ["bin"]
    ]
    if len(all_bins) != 1 or len(product_bins) != 1:
        failures.append(
            "cargo metadata must expose only the intended binary target; "
            f"observed {len(all_bins)} binaries, {len(product_bins)} exact matches"
        )
    return failures


def parse_arguments(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root (default: script parent)",
    )
    parser.add_argument(
        "--tag",
        required=True,
        help="externally supplied release tag or candidate tag",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    options = parse_arguments(argv if argv is not None else sys.argv[1:])
    root = options.root.resolve()
    failures: list[str] = []

    try:
        identity = load_toml(root / "release" / "identity.toml")
    except (OSError, tomllib.TOMLDecodeError) as error:
        print(f"release identity check failed:\n- cannot read release/identity.toml: {error}")
        return 1

    try:
        published_metadata, published_rows = load_published_api_fixture(
            root / PUBLISHED_API_FIXTURE
        )
        failures.extend(check_published_api_fixture(published_metadata, published_rows))
    except (OSError, ValueError) as error:
        published_metadata = {}
        failures.append(f"cannot read {PUBLISHED_API_FIXTURE}: {error}")

    failures.extend(check_identity(identity, published_metadata))
    failures.extend(check_migration_evidence_files(identity, root))

    try:
        manifest = load_toml(root / "Cargo.toml")
        failures.extend(check_manifest(manifest, identity))
    except (OSError, tomllib.TOMLDecodeError) as error:
        failures.append(f"cannot read Cargo.toml: {error}")

    try:
        lock = load_toml(root / "Cargo.lock")
        failures.extend(check_lock(lock, identity))
    except (OSError, tomllib.TOMLDecodeError) as error:
        failures.append(f"cannot read Cargo.lock: {error}")

    failures.extend(check_tag(options.tag, identity))

    try:
        changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    except FileNotFoundError:
        changelog = None
    except OSError as error:
        failures.append(f"cannot read CHANGELOG.md: {error}")
        changelog = None
    failures.extend(check_changelog(changelog, identity))

    try:
        cli_source = (root / "src" / "cli" / "mod.rs").read_text(encoding="utf-8")
        failures.extend(check_cli_source(cli_source, identity))
    except OSError as error:
        failures.append(f"cannot read src/cli/mod.rs: {error}")

    metadata_text, metadata_failures = cargo_output(
        ["metadata", "--locked", "--no-deps", "--format-version", "1"],
        root=root,
    )
    failures.extend(metadata_failures)
    metadata: Mapping[str, Any] | None = None
    if metadata_text is not None:
        try:
            metadata = json.loads(metadata_text)
            failures.extend(check_metadata(metadata, identity, root))
        except json.JSONDecodeError as error:
            failures.append(f"cargo metadata returned invalid JSON: {error}")

    build_text, build_failures = cargo_output(
        [
            "build",
            "--locked",
            "--bin",
            str(identity.get("binary", "")),
            "--message-format=json-render-diagnostics",
        ],
        root=root,
    )
    failures.extend(build_failures)
    executable: Path | None = None
    if build_text is not None:
        executable, executable_failures = built_executable(build_text, identity)
        failures.extend(executable_failures)
    if executable is not None:
        result = run([str(executable), "--version"], cwd=root)
        if result.returncode != 0:
            failures.append(
                f"built CLI --version failed with {result.returncode}: "
                f"{result.stderr.strip() or result.stdout.strip()}"
            )
        else:
            failures.extend(check_cli_output(result.stdout, identity))

    if failures:
        print("release identity check failed:\n- " + "\n- ".join(failures))
        return 1

    version, expected_tag, heading = expected_values(identity)
    print(
        "release identity check passed: "
        f"{identity.get('package')} {version}, tag {expected_tag}, heading {heading!r}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
