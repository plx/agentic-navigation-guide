use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;
use walkdir::WalkDir;

const PRODUCT_BINARY: &str = "agentic-navigation-guide";
const LINKABLE_TARGET_KINDS: &[&str] =
    &["lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"];

fn run_cargo(current_dir: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO"))
        .args(arguments)
        .current_dir(current_dir)
        .output()
        .unwrap_or_else(|error| panic!("run cargo {}: {error}", arguments.join(" ")))
}

fn successful_stdout(output: Output, operation: &str) -> String {
    assert!(
        output.status.success(),
        "{operation} failed with {}:\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("{operation} returned non-UTF-8 stdout: {error}"))
}

fn metadata_failures(label: &str, metadata: &str) -> Vec<String> {
    let mut failures = Vec::new();

    for target_kind in LINKABLE_TARGET_KINDS {
        for key in ["kind", "crate_types"] {
            let marker = format!("\"{key}\":[\"{target_kind}\"]");
            if metadata.contains(&marker) {
                failures.push(format!("{label} exposes linkable target marker {marker}"));
            }
        }
    }

    let product_bins = metadata.matches("\"kind\":[\"bin\"]").count();
    if product_bins != 1 {
        failures.push(format!(
            "{label} reports {product_bins} product binary targets instead of exactly one"
        ));
    }

    let intended_binary =
        format!("\"kind\":[\"bin\"],\"crate_types\":[\"bin\"],\"name\":\"{PRODUCT_BINARY}\"");
    let intended_binary_count = metadata.matches(&intended_binary).count();
    if intended_binary_count != 1 {
        failures.push(format!(
            "{label} reports the exact intended named binary {intended_binary_count} times"
        ));
    }

    failures
}

fn public_visibility_failures(label: &str, source_root: &Path) -> Vec<String> {
    let mut failures = Vec::new();

    for entry in WalkDir::new(source_root) {
        let entry = entry.expect("walk Rust source tree");
        if !entry.file_type().is_file() || entry.path().extension().map_or(true, |ext| ext != "rs")
        {
            continue;
        }

        let source = std::fs::read_to_string(entry.path())
            .unwrap_or_else(|error| panic!("read {}: {error}", entry.path().display()));
        for (index, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed == "pub" || trimmed.starts_with("pub ") || trimmed.starts_with("pub\t") {
                failures.push(format!(
                    "{label} contains externally public Rust visibility at {}:{}",
                    entry.path().display(),
                    index + 1
                ));
            }
        }
    }

    failures
}

#[test]
fn issue_54_workspace_and_packaged_metadata_are_binary_only() {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let package_target = TempDir::new().expect("isolated #54 package target");
    let mut failures = Vec::new();

    if manifest_root.join("src/lib.rs").exists() {
        failures.push("workspace still contains the auto-discovered src/lib.rs target".to_string());
    }

    let workspace_metadata = successful_stdout(
        run_cargo(
            manifest_root,
            &[
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version",
                "1",
            ],
        ),
        "workspace cargo metadata",
    );
    failures.extend(metadata_failures("workspace metadata", &workspace_metadata));
    failures.extend(public_visibility_failures(
        "workspace source",
        &manifest_root.join("src"),
    ));

    let package = Command::new(env!("CARGO"))
        .args(["package", "--locked", "--offline", "--allow-dirty"])
        .current_dir(manifest_root)
        .env("CARGO_TARGET_DIR", package_target.path())
        .output()
        .expect("run real #54 cargo package");
    assert!(
        package.status.success(),
        "cargo package failed with {}:\nstdout:\n{}\nstderr:\n{}",
        package.status,
        String::from_utf8_lossy(&package.stdout),
        String::from_utf8_lossy(&package.stderr)
    );

    let unpacked_package = package_target.path().join("package").join(format!(
        "{}-{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    ));
    assert!(
        unpacked_package.join("Cargo.toml").is_file(),
        "cargo package did not create the expected unpacked package at {}",
        unpacked_package.display()
    );
    if unpacked_package.join("src/lib.rs").exists() {
        failures.push("packaged crate still contains src/lib.rs".to_string());
    }

    let packaged_metadata = successful_stdout(
        run_cargo(
            &unpacked_package,
            &[
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version",
                "1",
            ],
        ),
        "packaged cargo metadata",
    );
    failures.extend(metadata_failures("packaged metadata", &packaged_metadata));
    failures.extend(public_visibility_failures(
        "packaged source",
        &unpacked_package.join("src"),
    ));

    assert!(
        failures.is_empty(),
        "#54 binary-only package contract is not realized:\n- {}",
        failures.join("\n- ")
    );
}
