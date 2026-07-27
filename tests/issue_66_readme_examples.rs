use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const PRODUCT_BINARY: &str = "agentic-navigation-guide";
const EXPECTED_VERSION: &str = "0.2.0";
const DOCUMENTATION_URL: &str =
    "https://github.com/plx/agentic-navigation-guide/blob/main/docs/v0.2-contract.md";
const SOURCE_LIFECYCLE: &str = r#"# From the root of a trusted source checkout:
cargo install --path . --locked
cargo install --path . --locked --force
cargo uninstall agentic-navigation-guide"#;
const RELEASE_LIFECYCLE: &str = r#"# Available after 0.2.0 is published:
cargo install agentic-navigation-guide --version 0.2.0 --locked
cargo install agentic-navigation-guide --version 0.2.0 --locked --force
cargo uninstall agentic-navigation-guide"#;
const QUICKSTART: &str = r#"cargo new --bin navigation-demo --vcs none
cd navigation-demo
agentic-navigation-guide init --output AGENTIC_NAVIGATION_GUIDE.md
agentic-navigation-guide check
agentic-navigation-guide verify"#;
const EXPECTED_QUICKSTART_OUTPUT: &str = r#"Navigation guide created at: AGENTIC_NAVIGATION_GUIDE.md
✓ Navigation guide syntax is valid
✓ Navigation guide is valid and matches filesystem"#;
const GUIDE_ENVIRONMENT_VARIABLES: &[&str] = &[
    "AGENTIC_NAVIGATION_GUIDE_PATH",
    "AGENTIC_NAVIGATION_GUIDE_ROOT",
    "AGENTIC_NAVIGATION_GUIDE_NAME",
    "AGENTIC_NAVIGATION_GUIDE_LOG_MODE",
    "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE",
];

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn fenced_blocks(source: &str, language: &str) -> Vec<String> {
    let opening = format!("```{language}");
    let mut blocks = Vec::new();
    let mut current = None::<Vec<&str>>;

    for line in source.lines() {
        if current.is_none() && line == opening {
            current = Some(Vec::new());
        } else if line == "```" {
            if let Some(lines) = current.take() {
                blocks.push(lines.join("\n"));
            }
        } else if let Some(lines) = current.as_mut() {
            lines.push(line);
        }
    }

    assert!(
        current.is_none(),
        "README has an unterminated {language} fence"
    );
    blocks
}

fn run_command(command: &mut Command, operation: &str) -> Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{operation}: {error}"));
    assert!(
        output.status.success(),
        "{operation} failed with {}:\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn product_command(binary: &Path, current_dir: &Path) -> Command {
    let mut command = Command::new(binary);
    command.current_dir(current_dir);
    for variable in GUIDE_ENVIRONMENT_VARIABLES {
        command.env_remove(variable);
    }
    command
}

fn cargo_command(current_dir: &Path, target_dir: Option<&Path>) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(current_dir)
        .env("CARGO_TERM_COLOR", "never");
    if let Some(target_dir) = target_dir {
        command.env("CARGO_TARGET_DIR", target_dir);
    }
    command
}

fn test_binary() -> PathBuf {
    PathBuf::from(assert_cmd::cargo::cargo_bin!("agentic-navigation-guide"))
}

fn installed_binary(install_root: &Path) -> PathBuf {
    let path = install_root.join("bin").join(PRODUCT_BINARY);
    if cfg!(windows) {
        path.with_extension("exe")
    } else {
        path
    }
}

fn package_field(manifest: &str, key: &str) -> Option<String> {
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

#[test]
fn issue_66_readme_is_a_concise_versioned_entry_point() {
    let root = repository_root();
    let readme = fs::read_to_string(root.join("README.md")).expect("read README");
    let shell_blocks = fenced_blocks(&readme, "sh");
    assert_eq!(
        shell_blocks,
        [SOURCE_LIFECYCLE, RELEASE_LIFECYCLE, QUICKSTART],
        "every README shell example must remain in the executable smoke contract"
    );
    assert_eq!(
        fenced_blocks(&readme, "text"),
        [EXPECTED_QUICKSTART_OUTPUT],
        "the quickstart result must remain explicit"
    );

    for required in [
        "Cargo/crates.io is the only supported release installation channel",
        "Linux, macOS, and Windows",
        "Rust `1.85.0`",
        "installed CLI is the sole supported v0.2 product",
        "docs/v0.2-contract.md",
        "docs/release-policy.md",
        "docs/maintainer-continuity.md",
        "PR #21",
        "audits/2026-07-27-issue-68-specification-disposition.md",
    ] {
        assert!(
            readme.contains(required),
            "README omits required concise contract text {required:?}"
        );
    }
    for stale in [
        "docs.rs/agentic-navigation-guide",
        "early preview",
        "accurate*.",
        "Future Roadmap",
        "actions/checkout@v",
        "actions-rust-lang/setup-rust-toolchain@v",
    ] {
        assert!(
            !readme.contains(stale),
            "README retains stale or mutable text {stale:?}"
        );
    }

    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
    assert_eq!(
        package_field(&manifest, "documentation").as_deref(),
        Some(DOCUMENTATION_URL),
        "package documentation metadata must target maintained CLI/contract docs"
    );
}

#[test]
fn issue_66_quickstart_and_shown_cli_modes_run_in_a_clean_workspace() {
    let temp = TempDir::new().expect("clean README workspace");
    let cargo_new = run_command(
        cargo_command(temp.path(), None).args(["new", "--bin", "navigation-demo", "--vcs", "none"]),
        "README cargo new",
    );
    assert!(
        String::from_utf8_lossy(&cargo_new.stderr).contains("navigation-demo"),
        "cargo new did not report the documented project"
    );

    let workspace = temp.path().join("navigation-demo");
    let binary = test_binary();

    let init = run_command(
        product_command(&binary, &workspace).args([
            "init",
            "--output",
            "AGENTIC_NAVIGATION_GUIDE.md",
        ]),
        "README init",
    );
    assert_eq!(
        String::from_utf8(init.stdout).expect("init stdout is UTF-8"),
        "Navigation guide created at: AGENTIC_NAVIGATION_GUIDE.md\n"
    );
    assert!(
        workspace.join("AGENTIC_NAVIGATION_GUIDE.md").is_file(),
        "README init did not create the documented guide"
    );

    let check = run_command(
        product_command(&binary, &workspace).arg("check"),
        "README check",
    );
    assert_eq!(
        String::from_utf8(check.stdout).expect("check stdout is UTF-8"),
        "✓ Navigation guide syntax is valid\n"
    );

    let verify = run_command(
        product_command(&binary, &workspace).arg("verify"),
        "README verify",
    );
    assert_eq!(
        String::from_utf8(verify.stdout).expect("verify stdout is UTF-8"),
        "✓ Navigation guide is valid and matches filesystem\n"
    );

    run_command(
        product_command(&binary, &workspace).args([
            "verify",
            "--recursive",
            "--github-actions-check",
            "--deny-ignored",
        ]),
        "README required recursive CI check",
    );
    run_command(
        product_command(&binary, &workspace).args([
            "verify",
            "--post-tool-use-hook",
            "--deny-ignored",
        ]),
        "README required post-tool-use hook",
    );
}

#[test]
fn issue_66_package_install_upgrade_and_uninstall_are_executable() {
    let root = repository_root();
    let temp = TempDir::new().expect("isolated README package lifecycle");
    let package_target = temp.path().join("package-target");
    run_command(
        cargo_command(root, Some(&package_target)).args([
            "package",
            "--locked",
            "--offline",
            "--allow-dirty",
        ]),
        "README cargo package",
    );

    let package_directory = package_target
        .join("package")
        .join(format!("{PRODUCT_BINARY}-{EXPECTED_VERSION}"));
    assert!(
        package_directory.is_dir(),
        "cargo package did not unpack {}",
        package_directory.display()
    );
    let packaged_readme =
        fs::read_to_string(package_directory.join("README.md")).expect("read packaged README");
    for command in SOURCE_LIFECYCLE.lines().chain(RELEASE_LIFECYCLE.lines()) {
        if !command.starts_with('#') {
            assert!(
                packaged_readme.contains(command),
                "packaged README omits lifecycle command {command:?}"
            );
        }
    }

    let install_root = temp.path().join("install-root");
    let install_target = temp.path().join("install-target");
    let install_root_text = install_root
        .to_str()
        .expect("temporary install root is UTF-8");
    for force in [false, true] {
        let mut command = cargo_command(&package_directory, Some(&install_target));
        command.args([
            "install",
            "--path",
            ".",
            "--locked",
            "--offline",
            "--debug",
            "--root",
            install_root_text,
        ]);
        if force {
            command.arg("--force");
        }
        run_command(
            &mut command,
            if force {
                "README package upgrade"
            } else {
                "README package install"
            },
        );
    }

    let binary = installed_binary(&install_root);
    let version = run_command(
        product_command(&binary, temp.path()).arg("--version"),
        "README installed version",
    );
    assert_eq!(
        String::from_utf8(version.stdout).expect("version stdout is UTF-8"),
        format!("{PRODUCT_BINARY} {EXPECTED_VERSION}\n")
    );

    run_command(
        cargo_command(&package_directory, None).args([
            "uninstall",
            PRODUCT_BINARY,
            "--root",
            install_root_text,
        ]),
        "README cargo uninstall",
    );
    assert!(
        !binary.exists(),
        "cargo uninstall left the binary installed"
    );
}

#[test]
fn issue_66_ci_example_is_exact_parseable_and_immutable() {
    let root = repository_root();
    let readme = fs::read_to_string(root.join("README.md")).expect("read README");
    let example = fs::read_to_string(root.join(".github/examples/readme-verify.yml"))
        .expect("read checked README workflow example");
    assert_eq!(
        fenced_blocks(&readme, "yaml"),
        [example.trim_end()],
        "README CI YAML must exactly mirror the actionlint-checked example"
    );

    let mut action_count = 0;
    for line in example.lines() {
        let trimmed = line.trim();
        if let Some(reference) = trimmed.strip_prefix("- uses: ") {
            action_count += 1;
            let (_, revision_and_comment) = reference
                .split_once('@')
                .expect("action reference includes @");
            let revision = revision_and_comment
                .split_ascii_whitespace()
                .next()
                .expect("action reference includes revision");
            assert_eq!(revision.len(), 40, "action revision must be a full SHA");
            assert!(
                revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "action revision must be hexadecimal"
            );
        }
    }
    assert_eq!(action_count, 2, "README workflow action set drifted");
    for required in [
        "persist-credentials: false",
        "toolchain: \"1.85.0\"",
        "cargo install agentic-navigation-guide --version 0.2.0 --locked",
        "verify --recursive --github-actions-check --deny-ignored",
    ] {
        assert!(
            example.contains(required),
            "README workflow omits {required:?}"
        );
    }

    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read CI workflow");
    assert!(
        ci.contains("actionlint .github/workflows/*.yml .github/examples/*.yml"),
        "CI must parse the checked README YAML with actionlint"
    );
    assert!(
        ci.contains("cargo test --locked --test issue_66_readme_examples -- --nocapture"),
        "every OS job must execute the README smoke harness"
    );
}
