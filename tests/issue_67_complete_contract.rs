#[path = "support/process_cli.rs"]
mod test_cli;
#[path = "support/environment.rs"]
mod test_environment;

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use test_cli::process_cli_command;

const CONTRACT_PATH: &str = "docs/v0.2-contract.md";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repository_file(path: &str) -> String {
    let path = repository_root().join(path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn issue_67_contract_is_the_complete_versioned_support_reference() {
    let contract = repository_file(CONTRACT_PATH);
    let normalized_contract = contract.split_whitespace().collect::<Vec<_>>().join(" ");

    for required in [
        "# Agentic Navigation Guide v0.2 Contract",
        "<!-- normative-v0.2-specification -->",
        "## Complete CLI reference",
        "### Commands and built-in help",
        "### Complete argument ledger",
        "### Stable streams and exit statuses",
        "## Compatibility, versioning, and support",
        "### Guide-format and CLI versioning",
        "### Rust toolchain and dependency support",
        "### Supported product versions",
        "## Security and vulnerability reporting",
        "Rust `1.85.0` is the minimum supported toolchain",
        "There are zero supported Rust symbols in v0.2",
        "not a sandbox",
        "hostile concurrent replacement",
        "ordinary public GitHub issues are not a private vulnerability-report channel",
    ] {
        let normalized_required = required.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            normalized_contract.contains(&normalized_required),
            "the complete normative contract is missing `{required}`"
        );
    }

    for stale in [
        "Later production-readiness work will extend this same document",
        "Issue #67 owns the complete command and option contract",
        "complete CLI, security, and platform contracts while",
    ] {
        let normalized_stale = stale.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            !normalized_contract.contains(&normalized_stale),
            "the completed contract still delegates its own scope: `{stale}`"
        );
    }

    let manifest = repository_file("Cargo.toml");
    assert!(
        manifest.contains("rust-version = \"1.85\""),
        "the documented MSRV must remain tied to the package declaration"
    );

    for path in [
        "README.md",
        "docs/history/README.md",
        "docs/history/Specification.md",
    ] {
        let source = repository_file(path);
        assert!(
            source.contains("v0.2-contract.md"),
            "{path} must point readers to the normative v0.2 contract"
        );
    }
}

#[test]
fn issue_67_help_usage_runtime_status_and_stream_classes_are_executable() {
    let secret = "ISSUE67_HELP_ENV_SECRET_6c41b102";
    let help = process_cli_command()
        .arg("--help")
        .env("AGENTIC_NAVIGATION_GUIDE_PATH", secret)
        .env("AGENTIC_NAVIGATION_GUIDE_ROOT", secret)
        .env("AGENTIC_NAVIGATION_GUIDE_NAME", secret)
        .env("AGENTIC_NAVIGATION_GUIDE_LOG_MODE", secret)
        .env("AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE", secret)
        .output()
        .expect("render root help");
    let help_stdout = String::from_utf8_lossy(&help.stdout);
    assert_eq!(help.status.code(), Some(0));
    assert!(help.stderr.is_empty(), "help diagnostics must be empty");
    assert!(
        !help_stdout.contains(secret),
        "help interpolated live environment"
    );
    for required in [
        "dump",
        "init",
        "check",
        "verify",
        "--verbose",
        "--quiet",
        "--help",
        "--version",
        "AGENTIC_NAVIGATION_GUIDE_PATH",
        "AGENTIC_NAVIGATION_GUIDE_ROOT",
        "AGENTIC_NAVIGATION_GUIDE_NAME",
        "AGENTIC_NAVIGATION_GUIDE_LOG_MODE",
        "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE",
    ] {
        assert!(
            help_stdout.contains(required),
            "root help omitted `{required}`"
        );
    }

    let version = process_cli_command()
        .arg("--version")
        .output()
        .expect("render version");
    assert_eq!(version.status.code(), Some(0));
    assert!(version.stderr.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "agentic-navigation-guide 0.2.0"
    );

    let usage = process_cli_command()
        .arg("init")
        .output()
        .expect("reject missing required output");
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());
    assert!(String::from_utf8_lossy(&usage.stderr).contains("--output <OUTPUT>"));

    let temp = TempDir::new().expect("temporary CLI contract root");
    let invalid = temp.path().join("invalid-guide.md");
    fs::write(&invalid, "not a navigation guide").expect("write invalid guide");

    let default_failure = process_cli_command()
        .arg("check")
        .arg("--guide")
        .arg(&invalid)
        .output()
        .expect("run default syntax failure");
    assert_eq!(default_failure.status.code(), Some(1));
    assert!(default_failure.stdout.is_empty());
    assert!(!default_failure.stderr.is_empty());

    let hook_failure = process_cli_command()
        .arg("check")
        .arg("--post-tool-use-hook")
        .arg("--guide")
        .arg(&invalid)
        .output()
        .expect("run hook syntax failure");
    assert_eq!(hook_failure.status.code(), Some(2));
    assert!(hook_failure.stdout.is_empty());
    assert!(!hook_failure.stderr.is_empty());

    let valid = temp.path().join("valid-guide.md");
    fs::write(
        &valid,
        "<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide>\n",
    )
    .expect("write valid guide");
    let quiet_success = process_cli_command()
        .arg("--quiet")
        .arg("check")
        .arg("--guide")
        .arg(&valid)
        .output()
        .expect("run quiet syntax success");
    assert_eq!(quiet_success.status.code(), Some(0));
    assert!(quiet_success.stdout.is_empty() && quiet_success.stderr.is_empty());

    fs::write(temp.path().join("file.txt"), "").expect("write dump fixture");
    let quiet_dump = process_cli_command()
        .arg("--quiet")
        .arg("dump")
        .arg("--root")
        .arg(temp.path())
        .arg("--exclude")
        .arg("invalid-guide.md")
        .arg("--exclude")
        .arg("valid-guide.md")
        .output()
        .expect("run quiet primary-output command");
    assert_eq!(quiet_dump.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&quiet_dump.stdout).contains("file.txt"),
        "quiet mode suppressed primary dump data"
    );
    assert!(quiet_dump.stderr.is_empty());
}
