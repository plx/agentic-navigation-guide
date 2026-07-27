#[path = "support/assert_cli.rs"]
mod assert_cli;
#[path = "support/process_cli.rs"]
mod process_cli;

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use assert_cli::{assert_cli_command, HermeticAssertCommand, GUIDE_ENVIRONMENT_VARIABLES};
use process_cli::{process_cli_command, HermeticProcessCommand};

fn removed_environment<'a, I>(environment: I) -> BTreeSet<OsString>
where
    I: Iterator<Item = (&'a OsStr, Option<&'a OsStr>)>,
{
    environment
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| name.to_owned())
        .collect()
}

fn assert_isolated_root(root: Option<&Path>, label: &str) -> PathBuf {
    let root = root.unwrap_or_else(|| panic!("{label} inherits the process current directory"));
    assert_ne!(
        root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "{label} still uses the repository checkout"
    );
    assert!(
        root.is_dir(),
        "{label} root does not exist: {}",
        root.display()
    );
    root.to_path_buf()
}

fn assert_environment_removed(removed: &BTreeSet<OsString>, label: &str) {
    for variable in GUIDE_ENVIRONMENT_VARIABLES {
        assert!(
            removed.contains(OsStr::new(variable)),
            "{label} inherits configuration variable {variable}"
        );
    }
}

#[test]
fn issue_58_assert_cli_harness_is_hermetic_and_cleans_its_default_root() {
    let command: HermeticAssertCommand = assert_cli_command();
    let root = assert_isolated_root(command.get_current_dir(), "assert CLI harness");
    let removed = removed_environment(command.get_envs());
    assert_environment_removed(&removed, "assert CLI harness");

    drop(command);
    assert!(
        !root.exists(),
        "assert CLI harness did not clean its default root: {}",
        root.display()
    );
}

#[test]
fn issue_58_process_cli_harness_is_hermetic_and_cleans_its_default_root() {
    let command: HermeticProcessCommand = process_cli_command();
    let root = assert_isolated_root(command.get_current_dir(), "process CLI harness");
    let removed = removed_environment(command.get_envs());
    assert_environment_removed(&removed, "process CLI harness");

    drop(command);
    assert!(
        !root.exists(),
        "process CLI harness did not clean its default root: {}",
        root.display()
    );
}

fn repository_file(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("read {path}: {error}"))
}

fn function_block<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing function {signature}"));
    let tail = &source[start..];
    let end = tail[signature.len()..]
        .find("\n#[")
        .map_or(tail.len(), |offset| signature.len() + offset);
    &tail[..end]
}

#[test]
fn issue_58_subprocess_inventory_is_explicit_and_process_global_state_is_untouched() {
    let required_harnesses = [
        ("tests/cli_tests.rs", "support/assert_cli.rs"),
        ("tests/environment_precedence.rs", "support/assert_cli.rs"),
        (
            "tests/issue_101_parent_explicit_guide.rs",
            "support/assert_cli.rs",
        ),
        (
            "tests/issue_47_output_contract.rs",
            "support/process_cli.rs",
        ),
        (
            "tests/issue_68_normative_source.rs",
            "support/process_cli.rs",
        ),
    ];
    for (path, harness) in required_harnesses {
        assert!(
            repository_file(path).contains(harness),
            "{path} does not use the hermetic subprocess harness {harness}"
        );
    }

    let cli_tests = repository_file("tests/cli_tests.rs");
    let init = function_block(&cli_tests, "fn test_init_command()");
    assert!(
        init.contains(".arg(\"--root\")") && init.contains("TempDir::new()"),
        "the original init regression can read outside its explicit temporary root"
    );
    let default_root = function_block(
        &cli_tests,
        "fn issue_58_product_current_directory_default_is_covered_explicitly()",
    );
    assert!(
        default_root.contains(".current_dir(root.path())"),
        "the intentional product current-directory default lacks an explicit fixture"
    );

    for entry in WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")) {
        let entry = entry.expect("walk integration tests");
        if !entry.file_type().is_file()
            || entry.path().extension() != Some(OsStr::new("rs"))
            || entry.file_name() == OsStr::new("issue_58_test_hermeticity.rs")
        {
            continue;
        }
        let source = fs::read_to_string(entry.path())
            .unwrap_or_else(|error| panic!("read {}: {error}", entry.path().display()));
        for forbidden in [
            "std::env::set_current_dir",
            "std::env::set_var",
            "std::env::remove_var",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} mutates process-global state with {forbidden}",
                entry.path().display()
            );
        }
    }
}

#[test]
fn issue_58_transient_behavior_and_empty_supported_facade_have_executable_owners() {
    let dumper = repository_file("src/dumper.rs");
    let transient = function_block(
        &dumper,
        "fn issue_42_transient_classification_failure_aborts_collection()",
    );
    for required in [
        "io::ErrorKind::NotFound",
        "a transient classification failure must abort collection",
        "ISSUE42_CLASSIFIER_INTERNAL_SENTINEL",
    ] {
        assert!(
            transient.contains(required),
            "the deterministic transient-entry product test omits {required:?}"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !root.join("src/lib.rs").exists() && !repository_file("Cargo.toml").contains("\n[lib]\n"),
        "the supported Rust facade set is not empty"
    );
    let ci = repository_file(".github/workflows/ci.yml");
    for required in [
        "cargo test --locked --test issue_58_test_hermeticity -- --nocapture",
        "--test issue_54_binary_only_package",
        "issue_62_exact_package_installs_smokes_and_rejects_library_consumers",
        "-- --exact --ignored --nocapture",
    ] {
        assert!(
            ci.contains(required),
            "CI does not execute the binary-only boundary proof {required:?}"
        );
    }
}
