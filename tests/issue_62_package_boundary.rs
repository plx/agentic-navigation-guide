use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const PRODUCT_BINARY: &str = "agentic-navigation-guide";
const EXPECTED_PACKAGE_PATHS: &[&str] = &[
    ".cargo_vcs_info.json",
    "CHANGELOG.md",
    "Cargo.lock",
    "Cargo.toml",
    "Cargo.toml.orig",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "LICENSING.md",
    "NOTICE",
    "README.md",
    "THIRD_PARTY_LICENSES.md",
    "docs/release-policy.md",
    "docs/v0.2-contract.md",
    "src/cli/check.rs",
    "src/cli/dump.rs",
    "src/cli/environment.rs",
    "src/cli/generation_options.rs",
    "src/cli/init.rs",
    "src/cli/mod.rs",
    "src/cli/output.rs",
    "src/cli/verify.rs",
    "src/dumper.rs",
    "src/entry_type.rs",
    "src/errors.rs",
    "src/exclusion.rs",
    "src/guide_input.rs",
    "src/main.rs",
    "src/parser.rs",
    "src/path_codec.rs",
    "src/recursive.rs",
    "src/types.rs",
    "src/validator.rs",
    "src/verifier.rs",
];
const LINKABLE_TARGET_KINDS: &[&str] =
    &["lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"];
const GUIDE_ENVIRONMENT_VARIABLES: &[&str] = &[
    "AGENTIC_NAVIGATION_GUIDE_PATH",
    "AGENTIC_NAVIGATION_GUIDE_ROOT",
    "AGENTIC_NAVIGATION_GUIDE_NAME",
    "AGENTIC_NAVIGATION_GUIDE_LOG_MODE",
    "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE",
];

fn repository_root() -> PathBuf {
    std::env::var_os("ISSUE_62_PACKAGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf())
}

fn run_cargo(current_dir: &Path, arguments: &[&str], target_dir: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO"));
    command
        .args(arguments)
        .current_dir(current_dir)
        .env("CARGO_TERM_COLOR", "never");
    if let Some(target_dir) = target_dir {
        command.env("CARGO_TARGET_DIR", target_dir);
    }
    command
        .output()
        .unwrap_or_else(|error| panic!("run cargo {}: {error}", arguments.join(" ")))
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_success(output: &Output, operation: &str) {
    assert!(
        output.status.success(),
        "{operation} failed with {}:\n{}",
        output.status,
        combined_output(output)
    );
}

fn executable_path(install_root: &Path) -> PathBuf {
    let executable = install_root.join("bin").join(PRODUCT_BINARY);
    if cfg!(windows) {
        executable.with_extension("exe")
    } else {
        executable
    }
}

fn run_binary(binary: &Path, current_dir: &Path, arguments: &[&str]) -> Output {
    let mut command = Command::new(binary);
    command.args(arguments).current_dir(current_dir);
    for variable in GUIDE_ENVIRONMENT_VARIABLES {
        command.env_remove(variable);
    }
    command
        .output()
        .unwrap_or_else(|error| panic!("run installed binary {}: {error}", arguments.join(" ")))
}

fn assert_binary_success(binary: &Path, current_dir: &Path, arguments: &[&str], label: &str) {
    let output = run_binary(binary, current_dir, arguments);
    assert_success(&output, label);
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[test]
fn issue_62_package_manifest_is_the_exact_reviewed_allowlist() {
    let root = repository_root();
    let output = run_cargo(
        &root,
        &[
            "package",
            "--list",
            "--locked",
            "--offline",
            "--allow-dirty",
        ],
        None,
    );
    assert_success(&output, "list packaged paths");
    let observed = String::from_utf8(output.stdout)
        .expect("cargo package --list output is UTF-8")
        .lines()
        .map(|path| path.replace('\\', "/"))
        .collect::<Vec<_>>();
    assert_eq!(
        observed, EXPECTED_PACKAGE_PATHS,
        "the source package boundary must change only through an explicit manifest review"
    );

    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .expect("read package acceptance workflow");
    for required in [
        "--path target/package/agentic-navigation-guide-0.2.0",
        "issue_62_package_manifest_is_the_exact_reviewed_allowlist",
        "issue_62_exact_package_installs_smokes_and_rejects_library_consumers",
        "-- --exact --ignored --nocapture",
        "cargo publish --dry-run --locked",
    ] {
        assert!(
            ci.contains(required),
            "CI does not enforce package acceptance command {required:?}"
        );
    }
}

#[test]
#[ignore = "explicit packaged-artifact acceptance test; CI runs it once"]
fn issue_62_exact_package_installs_smokes_and_rejects_library_consumers() {
    let manifest_root = repository_root();
    let package_temp = TempDir::new().expect("isolated package directories");
    let package_target = package_temp.path().join("package-target");
    let package = run_cargo(
        &manifest_root,
        &["package", "--locked", "--offline", "--allow-dirty"],
        Some(&package_target),
    );
    assert_success(&package, "build and verify exact source package");

    let package_directory = package_target
        .join("package")
        .join(format!("{PRODUCT_BINARY}-{}", env!("CARGO_PKG_VERSION")));
    let package_archive = package_target.join("package").join(format!(
        "{PRODUCT_BINARY}-{}.crate",
        env!("CARGO_PKG_VERSION")
    ));
    assert!(
        package_directory.is_dir(),
        "cargo package did not unpack {}",
        package_directory.display()
    );
    let archive_size = fs::metadata(&package_archive)
        .unwrap_or_else(|error| panic!("inspect {}: {error}", package_archive.display()))
        .len();
    assert!(
        archive_size < 1_000_000,
        "reviewed source package is unexpectedly large: {archive_size} bytes"
    );

    let metadata = run_cargo(
        &package_directory,
        &[
            "metadata",
            "--locked",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
        ],
        None,
    );
    assert_success(&metadata, "inspect exact packaged target metadata");
    let metadata = String::from_utf8(metadata.stdout).expect("cargo metadata output is UTF-8");
    assert_eq!(
        metadata.matches("\"kind\":[\"bin\"]").count(),
        1,
        "the package must expose exactly one binary target"
    );
    for target_kind in LINKABLE_TARGET_KINDS {
        assert!(
            !metadata.contains(&format!("\"kind\":[\"{target_kind}\"]"))
                && !metadata.contains(&format!("\"crate_types\":[\"{target_kind}\"]")),
            "the exact package unexpectedly exposes {target_kind}"
        );
    }

    let install_root = package_temp.path().join("install-root");
    let install_target = package_temp.path().join("install-target");
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
        Some(&install_target),
    );
    assert_success(&install, "install exact unpacked package");
    let binary = executable_path(&install_root);
    assert!(binary.is_file(), "installed binary is missing");

    let smoke = package_temp.path().join("smoke");
    fs::create_dir(&smoke).expect("create smoke root");

    let version = run_binary(&binary, &smoke, &["--version"]);
    assert_success(&version, "installed --version");
    assert_eq!(
        String::from_utf8(version.stdout).expect("version output is UTF-8"),
        format!("{PRODUCT_BINARY} {}\n", env!("CARGO_PKG_VERSION"))
    );
    for help in [
        &["--help"][..],
        &["dump", "--help"],
        &["init", "--help"],
        &["check", "--help"],
        &["verify", "--help"],
    ] {
        assert_binary_success(
            &binary,
            &smoke,
            help,
            &format!("installed {}", help.join(" ")),
        );
    }

    fs::create_dir(smoke.join("valid-root")).expect("create valid root");
    fs::write(smoke.join("valid-root/present.txt"), "").expect("write present fixture");
    fs::write(
        smoke.join("valid.md"),
        "<agentic-navigation-guide>\n- present.txt\n</agentic-navigation-guide>\n",
    )
    .expect("write valid guide");
    assert_binary_success(
        &binary,
        &smoke,
        &["check", "--guide", "valid.md"],
        "installed successful check",
    );
    assert_binary_success(
        &binary,
        &smoke,
        &["verify", "--guide", "valid.md", "--root", "valid-root"],
        "installed successful verify",
    );

    fs::write(smoke.join("invalid.md"), "not a navigation guide\n").expect("write invalid guide");
    let failed_check = run_binary(&binary, &smoke, &["check", "--guide", "invalid.md"]);
    assert_eq!(failed_check.status.code(), Some(1));
    assert!(
        !combined_output(&failed_check).is_empty(),
        "failing packaged check must emit a diagnostic"
    );
    fs::write(
        smoke.join("mismatch.md"),
        "<agentic-navigation-guide>\n- missing.txt\n</agentic-navigation-guide>\n",
    )
    .expect("write mismatching guide");
    for (mode, expected_code) in [
        (None, 1),
        (Some("--post-tool-use-hook"), 2),
        (Some("--pre-commit-hook"), 1),
        (Some("--github-actions-check"), 1),
    ] {
        let mut arguments = vec!["verify", "--guide", "mismatch.md", "--root", "valid-root"];
        if let Some(mode) = mode {
            arguments.push(mode);
        }
        let failed_verify = run_binary(&binary, &smoke, &arguments);
        assert_eq!(
            failed_verify.status.code(),
            Some(expected_code),
            "installed failing verify mode {mode:?} returned unexpected status:\n{}",
            combined_output(&failed_verify)
        );
        assert!(
            combined_output(&failed_verify).contains("missing.txt"),
            "installed failing verify mode {mode:?} omitted the logical mismatch"
        );
    }

    fs::create_dir_all(smoke.join("roundtrip-root/docs")).expect("create round-trip root");
    fs::write(smoke.join("roundtrip-root/README.md"), "").expect("write round-trip file");
    fs::write(smoke.join("roundtrip-root/docs/example.txt"), "")
        .expect("write nested round-trip file");
    let dumped = run_binary(&binary, &smoke, &["dump", "--root", "roundtrip-root"]);
    assert_success(&dumped, "installed dump");
    fs::write(smoke.join("roundtrip.md"), dumped.stdout).expect("write dumped guide");
    assert_binary_success(
        &binary,
        &smoke,
        &["check", "--guide", "roundtrip.md"],
        "installed dump/check round-trip",
    );
    assert_binary_success(
        &binary,
        &smoke,
        &[
            "verify",
            "--guide",
            "roundtrip.md",
            "--root",
            "roundtrip-root",
        ],
        "installed dump/verify round-trip",
    );

    fs::create_dir_all(smoke.join("recursive/nested")).expect("create recursive root");
    fs::write(smoke.join("recursive/nested/present.txt"), "").expect("write recursive fixture");
    fs::write(
        smoke.join("recursive/nested/AGENTIC_NAVIGATION_GUIDE.md"),
        "<agentic-navigation-guide>\n- present.txt\n</agentic-navigation-guide>\n",
    )
    .expect("write recursive guide");
    assert_binary_success(
        &binary,
        &smoke,
        &["verify", "--recursive", "--root", "recursive"],
        "installed recursive discovery",
    );
    fs::create_dir(smoke.join("empty")).expect("create zero-guide root");
    let empty = run_binary(
        &binary,
        &smoke,
        &["verify", "--recursive", "--root", "empty"],
    );
    assert_eq!(empty.status.code(), Some(1));
    assert!(
        combined_output(&empty).contains("zero navigation guides"),
        "default zero-guide behavior omitted its fail-closed diagnostic"
    );
    assert_binary_success(
        &binary,
        &smoke,
        &["verify", "--recursive", "--root", "empty", "--allow-empty"],
        "installed explicit zero-guide opt-out",
    );

    let consumer = package_temp.path().join("consumer");
    fs::create_dir_all(consumer.join("src")).expect("create negative consumer");
    fs::write(
        consumer.join("Cargo.toml"),
        format!(
            "[package]\nname = \"issue-62-negative-consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\nagentic-navigation-guide = {{ path = \"{}\" }}\n",
            toml_path(&package_directory)
        ),
    )
    .expect("write negative consumer manifest");
    fs::write(
        consumer.join("src/main.rs"),
        "use agentic_navigation_guide as _;\nfn main() {}\n",
    )
    .expect("write negative consumer source");
    let consumer_target = package_temp.path().join("consumer-target");
    let lock = run_cargo(
        &consumer,
        &["generate-lockfile", "--offline"],
        Some(&consumer_target),
    );
    assert_success(&lock, "resolve exact packaged path dependency");
    let rejected = run_cargo(
        &consumer,
        &["check", "--locked", "--offline"],
        Some(&consumer_target),
    );
    assert!(
        !rejected.status.success(),
        "binary-only packaged dependency unexpectedly compiled as a library"
    );
    let diagnostic = combined_output(&rejected);
    assert!(
        diagnostic.contains("missing a lib target")
            && diagnostic.contains("error[E0432]")
            && diagnostic.contains("agentic_navigation_guide"),
        "negative consumer failed for an unexpected reason:\n{diagnostic}"
    );

    for required in [
        "Cargo.lock",
        "README.md",
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "NOTICE",
        "THIRD_PARTY_LICENSES.md",
        "LICENSING.md",
    ] {
        assert!(
            package_directory.join(required).is_file(),
            "exact package omits required {required}"
        );
    }
}
