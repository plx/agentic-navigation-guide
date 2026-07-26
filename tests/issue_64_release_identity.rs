use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const EXPECTED_VERSION: &str = "0.2.0";
const EXPECTED_LICENSE: &str = "MIT OR Apache-2.0";
const PRODUCT_BINARY: &str = "agentic-navigation-guide";
const PUBLISHED_API_COUNT: usize = 128;
const PUBLISHED_API_START: &str = "<!-- published-v0.1.4-api:start -->";
const PUBLISHED_API_END: &str = "<!-- published-v0.1.4-api:end -->";
const CATEGORY_COUNTS: &[(&str, usize)] = &[
    ("PackageTarget", 1),
    ("Module", 7),
    ("ReExport", 17),
    ("TypeAlias", 1),
    ("Struct", 10),
    ("Enum", 6),
    ("Variant", 38),
    ("Field", 19),
    ("Function", 7),
    ("Method", 22),
];

#[derive(Debug)]
struct PublishedApiCase {
    id: String,
    kind: String,
    symbol: String,
}

fn parse_published_api_fixture() -> (BTreeMap<String, String>, Vec<PublishedApiCase>) {
    let mut metadata = BTreeMap::new();
    let mut cases = Vec::new();
    let mut saw_header = false;
    for (index, line) in include_str!("fixtures/v0_1_4_published_api.tsv")
        .lines()
        .enumerate()
    {
        if let Some(metadata_line) = line.strip_prefix("# ") {
            let (key, value) = metadata_line
                .split_once('=')
                .unwrap_or_else(|| panic!("invalid baseline metadata on line {}", index + 1));
            assert!(
                metadata
                    .insert(key.to_string(), value.to_string())
                    .is_none(),
                "duplicate baseline metadata key {key:?}"
            );
            continue;
        }
        if line == "id|kind|symbol" && !saw_header && cases.is_empty() {
            saw_header = true;
            continue;
        }
        assert!(
            saw_header,
            "published baseline row precedes its header on line {}",
            index + 1
        );
        let mut fields = line.splitn(3, '|');
        let id = fields.next().unwrap_or_default();
        let kind = fields.next().unwrap_or_default();
        let symbol = fields.next().unwrap_or_default();
        assert!(
            !id.is_empty() && !kind.is_empty() && !symbol.is_empty(),
            "invalid published baseline row on line {}",
            index + 1
        );
        cases.push(PublishedApiCase {
            id: id.to_string(),
            kind: kind.to_string(),
            symbol: symbol.to_string(),
        });
    }
    assert!(saw_header, "published baseline fixture has no row header");
    (metadata, cases)
}

fn toml_string_value(source: &str, section: &str, key: &str) -> Option<String> {
    let mut current_section = "";
    for line in source.lines() {
        let line = line.trim();
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            current_section = name;
            continue;
        }
        if current_section != section {
            continue;
        }
        let prefix = format!("{key} = \"");
        if let Some(value) = line.strip_prefix(&prefix) {
            return value.strip_suffix('"').map(ToOwned::to_owned);
        }
    }
    None
}

fn render_published_api(cases: &[PublishedApiCase]) -> String {
    let mut rendered = String::new();
    for (kind, expected_count) in CATEGORY_COUNTS {
        let category: Vec<_> = cases.iter().filter(|case| case.kind == *kind).collect();
        assert_eq!(
            category.len(),
            *expected_count,
            "published API category {kind} drifted"
        );
        rendered.push_str(&format!("#### {kind} ({expected_count})\n\n```text\n"));
        for case in category {
            rendered.push_str(&case.symbol);
            rendered.push('\n');
        }
        rendered.push_str("```\n\n");
    }
    rendered
}

fn run_cargo(current_dir: &Path, arguments: &[&str], environment: &[(&str, &Path)]) -> Output {
    let mut command = Command::new(env!("CARGO"));
    command.args(arguments).current_dir(current_dir);
    for (name, value) in environment {
        command.env(name, value);
    }
    command
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .unwrap_or_else(|error| panic!("run cargo {}: {error}", arguments.join(" ")))
}

fn assert_success(output: &Output, operation: &str) {
    assert!(
        output.status.success(),
        "{operation} failed with {}:\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn normalized_package_field(manifest: &str, key: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        if line == "[package]" {
            in_package = true;
            continue;
        }
        if in_package && line.starts_with('[') {
            break;
        }
        if in_package {
            let prefix = format!("{key} = \"");
            if let Some(value) = line.strip_prefix(&prefix) {
                return value.strip_suffix('"').map(ToOwned::to_owned);
            }
        }
    }
    None
}

fn executable_path(install_root: &Path) -> PathBuf {
    let executable = install_root.join("bin").join(PRODUCT_BINARY);
    if cfg!(windows) {
        executable.with_extension("exe")
    } else {
        executable
    }
}

#[test]
fn issue_64_published_api_baseline_is_complete_and_changeloged() {
    let (metadata, cases) = parse_published_api_fixture();
    let expected_metadata = [
        ("published_version", "0.1.4"),
        (
            "artifact_sha256",
            "d08fefac88faf8d737eea273f86bfbc80aaac1eb80ff3a57bde5add824fe5da0",
        ),
        ("vcs_revision", "560ce399e1e28e8e0d6b87988956893796d2dfab"),
        (
            "normalized_manifest_sha256",
            "1dc83730531459a1fcae387cc5e5f625a3ff498659915d58fa875dd14c9fab3b",
        ),
        (
            "library_source_sha256",
            "c2107c1948025e592e4af33a39b8f80ce7f02b8160d48c12acf6a4c67963d656",
        ),
        (
            "ordered_id_sha256",
            "3b1fa66f32a32aa48430993d9e69a7fa0b9566942efd17f8dfe657b6d1e8ddb7",
        ),
        (
            "ordered_symbol_sha256",
            "7d6f9b7f320cb6394bfbf4b54657e4bddece662b15cc5b24cd1e409aab39ef88",
        ),
        (
            "ordered_row_sha256",
            "ab476288fae6998d16ee2a500825cf04a26b5564c3e59a9ed95824ed0193611f",
        ),
    ];
    for (key, expected) in expected_metadata {
        assert_eq!(metadata.get(key).map(String::as_str), Some(expected));
    }
    assert_eq!(metadata.len(), expected_metadata.len());
    assert_eq!(cases.len(), PUBLISHED_API_COUNT);

    let ids: HashSet<_> = cases.iter().map(|case| case.id.as_str()).collect();
    let symbols: HashSet<_> = cases.iter().map(|case| case.symbol.as_str()).collect();
    assert_eq!(ids.len(), cases.len(), "published API IDs must be unique");
    assert_eq!(
        symbols.len(),
        cases.len(),
        "published API symbols must be unique"
    );

    let mut category_counts = BTreeMap::new();
    for case in &cases {
        *category_counts.entry(case.kind.as_str()).or_insert(0_usize) += 1;
    }
    let expected_counts: BTreeMap<_, _> = CATEGORY_COUNTS.iter().copied().collect();
    assert_eq!(category_counts, expected_counts);

    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let identity = std::fs::read_to_string(manifest_root.join("release/identity.toml"))
        .expect("read release identity");
    assert_eq!(
        toml_string_value(&identity, "", "version").as_deref(),
        Some(EXPECTED_VERSION)
    );
    assert_eq!(
        toml_string_value(&identity, "", "tag_prefix").as_deref(),
        Some("v")
    );
    for key in [
        "published_version",
        "artifact_sha256",
        "vcs_revision",
        "normalized_manifest_sha256",
        "library_source_sha256",
    ] {
        assert_eq!(
            toml_string_value(&identity, "migration_baseline", key),
            metadata.get(key).cloned(),
            "release identity and published fixture disagree on {key}"
        );
    }

    let changelog = std::fs::read_to_string(manifest_root.join("CHANGELOG.md"))
        .expect("read the 0.2.0 changelog");
    let inventory = changelog
        .split_once(PUBLISHED_API_START)
        .expect("CHANGELOG.md has published API start marker")
        .1
        .split_once(PUBLISHED_API_END)
        .expect("CHANGELOG.md has published API end marker")
        .0
        .trim_matches('\n');
    assert_eq!(
        inventory,
        render_published_api(&cases).trim_end(),
        "CHANGELOG.md published API inventory must exactly match the pinned fixture"
    );

    for trait_commitment in [
        "Serialize and Deserialize",
        "Display and Error",
        "Send, Sync, and Unpin",
        "UnwindSafe and RefUnwindSafe",
    ] {
        assert!(
            changelog.contains(trait_commitment),
            "CHANGELOG.md omits published trait commitment {trait_commitment:?}"
        );
    }
}

#[test]
fn issue_64_exact_package_has_the_prepared_identity_and_installs() {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let package_temp = TempDir::new().expect("isolated package target");
    let package_target = package_temp.path().join("target");

    // Exercise the exact working tree during local development. CI separately
    // runs the acceptance command `cargo package --locked` on a clean checkout.
    let package = run_cargo(
        manifest_root,
        &["package", "--locked", "--offline", "--allow-dirty"],
        &[("CARGO_TARGET_DIR", &package_target)],
    );
    assert_success(&package, "cargo package");

    let observed_source_version = env!("CARGO_PKG_VERSION");
    let package_directory = package_target
        .join("package")
        .join(format!("{PRODUCT_BINARY}-{observed_source_version}"));
    let package_archive = package_target
        .join("package")
        .join(format!("{PRODUCT_BINARY}-{observed_source_version}.crate"));
    assert!(
        package_archive.is_file(),
        "cargo package did not create {}",
        package_archive.display()
    );
    assert!(
        package_directory.is_dir(),
        "cargo package did not verify and unpack {}",
        package_directory.display()
    );

    let normalized_manifest = std::fs::read_to_string(package_directory.join("Cargo.toml"))
        .expect("read normalized packaged Cargo.toml");
    let mut failures = Vec::new();
    let packaged_version = normalized_package_field(&normalized_manifest, "version");
    if packaged_version.as_deref() != Some(EXPECTED_VERSION) {
        failures.push(format!(
            "packaged manifest version: expected {EXPECTED_VERSION:?}, observed {packaged_version:?}"
        ));
    }
    let packaged_license = normalized_package_field(&normalized_manifest, "license");
    if packaged_license.as_deref() != Some(EXPECTED_LICENSE) {
        failures.push(format!(
            "packaged manifest license: expected {EXPECTED_LICENSE:?}, observed {packaged_license:?}"
        ));
    }

    let original_manifest = std::fs::read_to_string(package_directory.join("Cargo.toml.orig"))
        .expect("read original packaged Cargo.toml");
    for (field, expected) in [("version", EXPECTED_VERSION), ("license", EXPECTED_LICENSE)] {
        let observed = normalized_package_field(&original_manifest, field);
        if observed.as_deref() != Some(expected) {
            failures.push(format!(
                "packaged Cargo.toml.orig {field}: expected {expected:?}, observed {observed:?}"
            ));
        }
    }

    for legal_file in [
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "NOTICE",
        "THIRD_PARTY_LICENSES.md",
        "LICENSING.md",
    ] {
        let source_path = manifest_root.join(legal_file);
        let packaged_path = package_directory.join(legal_file);
        match (std::fs::read(&source_path), std::fs::read(&packaged_path)) {
            (Ok(source), Ok(packaged)) if source == packaged && !packaged.is_empty() => {}
            (Ok(_), Ok(_)) => failures.push(format!(
                "packaged {legal_file} is empty or differs from reviewed source"
            )),
            (Err(error), _) => {
                failures.push(format!("cannot read reviewed source {legal_file}: {error}"))
            }
            (_, Err(error)) => {
                failures.push(format!(
                    "packaged crate omits or cannot read {legal_file}: {error}"
                ));
            }
        }
    }
    if package_directory.join("LICENSE").exists() {
        failures.push("packaged crate contains the obsolete ambiguous root LICENSE".to_string());
    }
    for (legal_file, required_text) in [
        ("LICENSE-MIT", "Permission is hereby granted"),
        ("LICENSE-APACHE", "Apache License"),
        ("NOTICE", "MIT License or the Apache"),
        ("THIRD_PARTY_LICENSES.md", "# Third-Party Licenses"),
        (
            "LICENSING.md",
            "Historical `0.1.x` licensing-metadata clarification",
        ),
    ] {
        match std::fs::read_to_string(package_directory.join(legal_file)) {
            Ok(content) if content.contains(required_text) => {}
            Ok(_) => failures.push(format!(
                "packaged {legal_file} omits reviewed marker {required_text:?}"
            )),
            Err(error) => failures.push(format!("cannot inspect packaged {legal_file}: {error}")),
        }
    }

    let install_temp = TempDir::new().expect("isolated Cargo install directories");
    let cargo_home = install_temp.path().join("cargo-home");
    let install_target = install_temp.path().join("target");
    let install_root = install_temp.path().join("root");
    std::fs::create_dir_all(&cargo_home).expect("create isolated CARGO_HOME");

    let fetch = run_cargo(
        &package_directory,
        &["fetch", "--locked"],
        &[
            ("CARGO_HOME", &cargo_home),
            ("CARGO_TARGET_DIR", &install_target),
        ],
    );
    assert_success(&fetch, "fetch exact packaged dependencies");

    let install = run_cargo(
        &package_directory,
        &[
            "install",
            "--path",
            ".",
            "--locked",
            "--offline",
            "--debug",
            "--root",
            install_root
                .to_str()
                .expect("temporary install root is UTF-8"),
        ],
        &[
            ("CARGO_HOME", &cargo_home),
            ("CARGO_TARGET_DIR", &install_target),
        ],
    );
    assert_success(&install, "install exact unpacked package");

    let installed_version = Command::new(executable_path(&install_root))
        .arg("--version")
        .output()
        .expect("run installed packaged binary");
    assert!(
        installed_version.status.success(),
        "installed binary --version failed with {}:\n{}",
        installed_version.status,
        String::from_utf8_lossy(&installed_version.stderr)
    );
    let observed_cli = String::from_utf8(installed_version.stdout)
        .expect("installed binary version output is UTF-8");
    let expected_cli = format!("{PRODUCT_BINARY} {EXPECTED_VERSION}\n");
    if observed_cli != expected_cli {
        failures.push(format!(
            "installed CLI --version: expected {expected_cli:?}, observed {observed_cli:?}"
        ));
    }

    assert!(
        failures.is_empty(),
        "#64 packaged release identity is inconsistent:\n- {}",
        failures.join("\n- ")
    );
}
