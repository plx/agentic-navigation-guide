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

#[test]
fn test_verify_placeholder_future_items() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create scenario matching user's example: plans/phases/ with one file
    fs::create_dir_all(dir_path.join("plans/phases")).unwrap();
    fs::write(
        dir_path.join("plans/phases/phase-01-project-scaffolding.md"),
        "# Phase 01",
    )
    .unwrap();

    // Guide with placeholder that has a comment about future phases
    let guide_content = r#"<agentic-navigation-guide>
- plans/
  - phases/
    - phase-01-project-scaffolding.md # Plan for "Phase 01" - COMPLETED
    - ... # Plans for future phases will appear here
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Should succeed - placeholder has a comment indicating future items
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
fn test_verify_placeholder_no_comment_fails() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Same scenario but placeholder without comment
    fs::create_dir_all(dir_path.join("plans/phases")).unwrap();
    fs::write(
        dir_path.join("plans/phases/phase-01-project-scaffolding.md"),
        "# Phase 01",
    )
    .unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- plans/
  - phases/
    - phase-01-project-scaffolding.md
    - ...
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Should fail - placeholder without comment and no unmentioned items
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("placeholder"));
}

#[test]
fn test_verify_placeholder_mixed_scenarios() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create complex nested structure
    fs::create_dir_all(dir_path.join("src/modules")).unwrap();
    fs::create_dir_all(dir_path.join("tests")).unwrap();
    fs::write(dir_path.join("src/main.rs"), "").unwrap();
    fs::write(dir_path.join("src/lib.rs"), "").unwrap();
    fs::write(dir_path.join("src/utils.rs"), "").unwrap();
    fs::write(dir_path.join("tests/integration.rs"), "").unwrap();
    fs::write(dir_path.join("README.md"), "").unwrap();

    // Guide with various placeholder configurations
    let guide_content = r#"<agentic-navigation-guide>
- src/
  - main.rs # Entry point
  - ... # Other source files
  - modules/
    - ... # Future modules will be added here
- tests/
  - integration.rs
- README.md
- ... # Additional project files coming soon
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Should succeed - mixed placeholders with various scenarios:
    // - src/ has placeholder with comment AND unmentioned items (lib.rs, utils.rs)
    // - modules/ has placeholder with comment but directory is empty
    // - root has placeholder with comment for future items
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
fn test_github_actions_mode_success() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create valid structure
    fs::write(dir_path.join("README.md"), "").unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- README.md
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run verify with GitHub Actions mode
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--github-actions-check")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ Navigation guide verified"));
}

#[test]
fn test_github_actions_mode_error_format() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create guide with missing file
    let guide_content = r#"<agentic-navigation-guide>
- missing.txt
- also_missing.txt
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run verify with GitHub Actions mode
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--github-actions-check")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("❌"))
        .stderr(predicate::str::contains("GUIDE.md:2:"))
        .stderr(predicate::str::contains("missing.txt"));
}

#[test]
fn test_github_actions_mode_env_var() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create guide with error
    let guide_content = r#"<agentic-navigation-guide>
- missing_file.txt
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run verify with GitHub Actions mode via env var
    let mut cmd = get_command();
    cmd.env("AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE", "github-actions")
        .arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .failure()
        .code(1);
}

#[test]
fn test_github_actions_mode_syntax_error() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Guide with syntax error (indentation)
    let guide_content = r#"<agentic-navigation-guide>
- src
  - main.rs
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run check with GitHub Actions mode
    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--github-actions-check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("❌"))
        .stderr(predicate::str::contains("GUIDE.md:3:"))
        .stderr(predicate::str::contains("- main.rs"));
}

#[test]
fn test_github_actions_mode_quiet() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create valid structure
    fs::write(dir_path.join("README.md"), "").unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- README.md
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    // Run verify with GitHub Actions mode and quiet
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--github-actions-check")
        .arg("--quiet")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_github_actions_check_shows_line_content() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create guide with error on a specific line
    let guide_content = r#"<agentic-navigation-guide>
- README.md
- missing_file.txt
- another.txt
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();
    fs::write(dir_path.join("README.md"), "").unwrap();
    fs::write(dir_path.join("another.txt"), "").unwrap();

    // Run verify with GitHub Actions mode
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--github-actions-check")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("GUIDE.md:3:"))
        .stderr(predicate::str::contains("- missing_file.txt")); // Line content shown
}
