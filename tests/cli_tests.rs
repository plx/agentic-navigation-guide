use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn get_command() -> Command {
    Command::cargo_bin("agentic-navigation-guide").unwrap()
}

#[test]
fn test_dump_command_basic() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create test structure
    fs::create_dir(dir_path.join("src")).unwrap();
    fs::write(dir_path.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(dir_path.join("README.md"), "# Test Project").unwrap();

    // Run dump command
    let mut cmd = get_command();
    cmd.arg("dump")
        .arg("--root")
        .arg(dir_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("<agentic-navigation-guide>"))
        .stdout(predicate::str::contains("- README.md"))
        .stdout(predicate::str::contains("- src/"))
        .stdout(predicate::str::contains("  - main.rs"));
}

#[test]
fn test_dump_command_with_depth() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create nested structure
    fs::create_dir_all(dir_path.join("src/nested/deep")).unwrap();
    fs::write(dir_path.join("src/nested/deep/file.rs"), "").unwrap();
    fs::write(dir_path.join("src/main.rs"), "").unwrap();

    // Run dump with depth=2
    let mut cmd = get_command();
    cmd.arg("dump")
        .arg("--root")
        .arg(dir_path)
        .arg("--depth")
        .arg("2")
        .assert()
        .success()
        .stdout(predicate::str::contains("- src/"))
        .stdout(predicate::str::contains("  - main.rs"))
        .stdout(predicate::str::contains("  - nested/"))
        .stdout(predicate::str::contains("    - deep/"))
        .stdout(predicate::str::contains("file.rs").not()); // The file inside deep/ should not be shown
}

#[test]
fn test_dump_command_with_exclusions() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create structure with excludable directories
    fs::create_dir(dir_path.join("src")).unwrap();
    fs::create_dir(dir_path.join("target")).unwrap();
    fs::create_dir(dir_path.join(".git")).unwrap();
    fs::write(dir_path.join("src/main.rs"), "").unwrap();
    fs::write(dir_path.join("target/debug"), "").unwrap();

    // Run dump with exclusions
    let mut cmd = get_command();
    cmd.arg("dump")
        .arg("--root")
        .arg(dir_path)
        .arg("--exclude")
        .arg("target")
        .arg("--exclude")
        .arg(".git")
        .assert()
        .success()
        .stdout(predicate::str::contains("- src/"))
        .stdout(predicate::str::contains("target").not())
        .stdout(predicate::str::contains(".git").not());
}

#[test]
fn test_verify_command_success() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create matching guide and filesystem
    fs::create_dir(dir_path.join("src")).unwrap();
    fs::write(dir_path.join("src/main.rs"), "").unwrap();
    fs::write(dir_path.join("README.md"), "").unwrap();

    let guide_content = r#"# Test Project

<agentic-navigation-guide>
- src/
  - main.rs
- README.md
</agentic-navigation-guide>
"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run verify command
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Navigation guide is valid"));
}

#[test]
fn test_verify_command_missing_file() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create guide with missing file
    let guide_content = r#"<agentic-navigation-guide>
- src/
  - main.rs
- missing.txt
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Create only partial structure
    fs::create_dir(dir_path.join("src")).unwrap();
    fs::write(dir_path.join("src/main.rs"), "").unwrap();

    // Run verify command - should fail
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing.txt"));
}

#[test]
fn test_check_command_valid_syntax() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    let guide_content = r#"<agentic-navigation-guide>
- src/
  - main.rs # Main entry point
  - lib.rs
- Cargo.toml
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run check command
    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Navigation guide syntax is valid"));
}

#[test]
fn test_check_command_invalid_syntax() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Guide with invalid syntax (bad indentation)
    let guide_content = r#"<agentic-navigation-guide>
- src/
   - main.rs
  - lib.rs
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run check command - should fail
    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("indentation"));
}

#[test]
fn test_init_command() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();
    let output_path = dir_path.join("NEW_GUIDE.md");

    // Run init command
    let mut cmd = get_command();
    cmd.arg("init")
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success();

    // Verify file was created
    assert!(output_path.exists());
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("<agentic-navigation-guide>"));
    assert!(content.contains("</agentic-navigation-guide>"));
}

#[test]
fn test_post_tool_use_mode() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create guide with error
    let guide_content = r#"<agentic-navigation-guide>
- missing_file.txt
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run verify with post-tool-use mode
    let mut cmd = get_command();
    cmd.env("AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE", "post-tool-use")
        .arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .code(2); // Should exit with code 2
}

#[test]
fn test_quiet_log_mode() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create valid structure
    fs::write(dir_path.join("README.md"), "").unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- README.md
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run verify with quiet mode
    let mut cmd = get_command();
    cmd.env("AGENTIC_NAVIGATION_GUIDE_LOG_MODE", "quiet")
        .arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .success()
        .stdout(predicate::str::is_empty()); // No output in quiet mode
}

#[test]
fn test_empty_guide() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create guide with empty content
    let guide_content = r#"<agentic-navigation-guide>
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Check should fail on empty guide
    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty"));
}

#[test]
fn test_type_mismatch_file_vs_directory() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create a file
    fs::write(dir_path.join("src"), "This is a file, not a directory").unwrap();

    // Guide expects src to be a directory
    let guide_content = r#"<agentic-navigation-guide>
- src/
  - main.rs
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Verify should fail with type mismatch
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "expected directory but found file",
        ));
}

#[test]
fn test_invalid_path_characters() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Guide with invalid characters in path
    let guide_content = r#"<agentic-navigation-guide>
- src|invalid/
- file//double_slash.txt
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Check should fail
    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid path format"));
}

#[test]
fn test_nested_directories_with_comments() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create nested structure
    fs::create_dir_all(dir_path.join("src/modules/auth")).unwrap();
    fs::write(dir_path.join("src/modules/auth/login.rs"), "").unwrap();
    fs::write(dir_path.join("src/main.rs"), "").unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- src/
  - main.rs # Application entry point
  - modules/
    - auth/ # Authentication module
      - login.rs # Login functionality
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Should pass verification
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .success();
}

#[test]
fn test_dump_with_glob_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create structure with various files to exclude
    fs::create_dir(dir_path.join("src")).unwrap();
    fs::write(dir_path.join("src/main.rs"), "").unwrap();
    fs::write(dir_path.join("config.toml"), "").unwrap();
    fs::write(dir_path.join("secret.toml"), "").unwrap();
    fs::write(dir_path.join(".env"), "").unwrap();
    fs::write(dir_path.join("README.md"), "").unwrap();

    // Exclude all .toml files and dotfiles
    let mut cmd = get_command();
    cmd.arg("dump")
        .arg("--root")
        .arg(dir_path)
        .arg("--exclude")
        .arg("*.toml")
        .arg("--exclude")
        .arg(".*")
        .assert()
        .success()
        .stdout(predicate::str::contains("README.md"))
        .stdout(predicate::str::contains("src/"))
        .stdout(predicate::str::contains(".toml").not())
        .stdout(predicate::str::contains(".env").not());
}

#[test]
fn test_pre_commit_hook_mode() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create invalid guide
    let guide_content = r#"<agentic-navigation-guide>
- missing_file.txt
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run verify with pre-commit-hook mode
    let mut cmd = get_command();
    cmd.env("AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE", "pre-commit-hook")
        .arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .failure()
        .code(1); // Standard failure exit code for git hooks
}
