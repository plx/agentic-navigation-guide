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
fn test_verify_command_rejects_parent_path_component() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().join("project");
    fs::create_dir(&workspace_root).unwrap();
    fs::write(temp_dir.path().join("outside.txt"), "").unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- ../outside.txt
</agentic-navigation-guide>"#;

    let guide_path = workspace_root.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(&workspace_root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid special directory"))
        .stderr(predicate::str::contains("../outside.txt"));
}

#[cfg(unix)]
#[test]
fn test_verify_command_rejects_symlink_directory_escape() {
    use std::os::unix::fs::symlink;

    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().join("project");
    let outside_dir = temp_dir.path().join("outside");
    fs::create_dir(&workspace_root).unwrap();
    fs::create_dir(&outside_dir).unwrap();
    fs::write(outside_dir.join("secret.txt"), "").unwrap();
    symlink(&outside_dir, workspace_root.join("linked")).unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- linked/
  - secret.txt
</agentic-navigation-guide>"#;

    let guide_path = workspace_root.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(&workspace_root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("outside root boundary"))
        .stderr(predicate::str::contains("linked"));
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
fn test_verify_rejects_child_under_intervening_file() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::create_dir(root.join("a")).unwrap();
    fs::write(root.join("a/c"), "").unwrap();
    fs::write(root.join("b"), "").unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- a/
- b
  - c
</agentic-navigation-guide>"#;
    let guide_path = root.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut check = get_command();
    check
        .arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("line 4"))
        .stderr(predicate::str::contains(
            "indent only immediately after a directory",
        ));

    let mut github_actions = get_command();
    github_actions
        .arg("check")
        .arg("--github-actions-check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("GUIDE.md:4:"))
        .stderr(predicate::str::contains(
            "indent only immediately after a directory",
        ));

    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("line 4"))
        .stderr(predicate::str::contains(
            "indent only immediately after a directory",
        ));

    let mut post_tool_use = get_command();
    post_tool_use
        .env("AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE", "post-tool-use")
        .arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(root)
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "indent only immediately after a directory",
        ));
}

#[test]
fn test_check_command_accepts_path_without_trailing_slash() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    let guide_content = r#"<agentic-navigation-guide>
- src
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Navigation guide syntax is valid"));
}

#[test]
fn test_verify_command_path_without_trailing_slash_type_mismatch() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    fs::create_dir(dir_path.join("src")).unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- src
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "expected file but found directory",
        ))
        .stderr(predicate::str::contains("src"));
}

#[test]
fn test_check_command_unhashed_suffix_is_path_text() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    let guide_content = r#"<agentic-navigation-guide>
- src/ source code
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Navigation guide syntax is valid"));
}

#[test]
fn test_verify_command_unhashed_suffix_is_not_comment() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    fs::create_dir(dir_path.join("src")).unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- src/ source code
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("src/ source code"));
}

#[test]
fn test_check_command_accepts_escaped_hash_in_path() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    let guide_content = r#"<agentic-navigation-guide>
- docs/issue\#123.md
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Navigation guide syntax is valid"));
}

#[test]
fn test_verify_command_accepts_escaped_hash_in_path() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    fs::create_dir_all(dir_path.join("docs")).unwrap();
    fs::write(dir_path.join("docs/issue#123.md"), "").unwrap();

    let guide_content = r#"<agentic-navigation-guide>
- docs/issue\#123.md # tracked issue notes
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

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
fn test_check_command_multiple_guide_blocks_error() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    let guide_content = r#"<agentic-navigation-guide>
- src/
</agentic-navigation-guide>

<agentic-navigation-guide>
- docs/
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "line 5: multiple <agentic-navigation-guide> blocks found",
        ));
}

#[test]
fn test_check_command_stray_closing_marker_error() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    let guide_content = r#"<agentic-navigation-guide>
- src/
</agentic-navigation-guide>
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "line 4: multiple <agentic-navigation-guide> blocks found",
        ));
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
fn test_init_command_existing_output_file_reports_error() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();
    let output_path = dir_path.join("EXISTING_GUIDE.md");
    fs::write(&output_path, "already here").unwrap();

    let mut cmd = get_command();
    cmd.arg("init")
        .arg("--output")
        .arg(&output_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("File already exists"))
        .stderr(predicate::str::contains("--force").not());
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
fn test_check_accepts_utf8_and_symbolic_paths() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    let guide_content = r#"<agentic-navigation-guide>
- src/
  - naïve-文件.rs
- docs/Guía rápida.md
- data|set@v2(β).json
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .success();
}

#[test]
fn test_invalid_path_structure() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    let guide_content = r#"<agentic-navigation-guide>
- src/./invalid.rs
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid special directory"));
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
fn test_dump_with_invalid_glob_pattern_reports_error() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    fs::write(dir_path.join("README.md"), "# test").unwrap();

    let mut cmd = get_command();
    cmd.arg("dump")
        .arg("--root")
        .arg(dir_path)
        .arg("--exclude")
        .arg("[")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid glob pattern"));
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
fn test_verify_placeholder_whitespace_only_comment_fails() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

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
    - ... #    
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

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
fn test_verify_placeholder_non_empty_comment_remains_relaxed() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

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
    - ... # future phases
</agentic-navigation-guide>"#;

    let guide_path = dir_path.join("GUIDE.md");
    fs::write(&guide_path, guide_content).unwrap();

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

#[test]
fn test_recursive_verify_basic() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a monorepo structure with multiple guides
    fs::create_dir_all(root.join("backend/src")).unwrap();
    fs::write(root.join("backend/src/main.rs"), "").unwrap();
    fs::write(root.join("backend/README.md"), "").unwrap();

    fs::create_dir_all(root.join("frontend/src")).unwrap();
    fs::write(root.join("frontend/src/index.js"), "").unwrap();
    fs::write(root.join("frontend/package.json"), "").unwrap();

    // Create guide files
    let backend_guide = r#"<agentic-navigation-guide>
- src/
  - main.rs
- README.md
</agentic-navigation-guide>"#;

    let frontend_guide = r#"<agentic-navigation-guide>
- src/
  - index.js
- package.json
</agentic-navigation-guide>"#;

    fs::write(
        root.join("backend/AGENTIC_NAVIGATION_GUIDE.md"),
        backend_guide,
    )
    .unwrap();
    fs::write(
        root.join("frontend/AGENTIC_NAVIGATION_GUIDE.md"),
        frontend_guide,
    )
    .unwrap();

    // Run recursive verify
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 2 navigation guide(s)"));
}

#[test]
fn test_recursive_verify_with_custom_name() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create structure with custom guide name
    fs::create_dir_all(root.join("module-a")).unwrap();
    fs::write(root.join("module-a/file.txt"), "").unwrap();

    fs::create_dir_all(root.join("module-b")).unwrap();
    fs::write(root.join("module-b/data.json"), "").unwrap();

    let guide_a = r#"<agentic-navigation-guide>
- file.txt
</agentic-navigation-guide>"#;

    let guide_b = r#"<agentic-navigation-guide>
- data.json
</agentic-navigation-guide>"#;

    fs::write(root.join("module-a/GUIDE.md"), guide_a).unwrap();
    fs::write(root.join("module-b/GUIDE.md"), guide_b).unwrap();

    // Run recursive verify with custom guide name
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--guide-name")
        .arg("GUIDE.md")
        .arg("--root")
        .arg(root)
        .assert()
        .success();
}

#[test]
fn test_recursive_verify_with_failures() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create structure with one valid and one invalid guide
    fs::create_dir_all(root.join("valid")).unwrap();
    fs::write(root.join("valid/file.txt"), "").unwrap();

    fs::create_dir_all(root.join("invalid")).unwrap();
    // Note: missing.txt is NOT created

    let valid_guide = r#"<agentic-navigation-guide>
- file.txt
</agentic-navigation-guide>"#;

    let invalid_guide = r#"<agentic-navigation-guide>
- missing.txt
</agentic-navigation-guide>"#;

    fs::write(root.join("valid/AGENTIC_NAVIGATION_GUIDE.md"), valid_guide).unwrap();
    fs::write(
        root.join("invalid/AGENTIC_NAVIGATION_GUIDE.md"),
        invalid_guide,
    )
    .unwrap();

    // Run recursive verify - should fail
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing.txt"));
}

#[test]
fn test_recursive_verify_with_exclusions() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create structure with guides in excluded directories
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "").unwrap();

    fs::create_dir_all(root.join("target/debug")).unwrap();
    fs::write(root.join("target/debug/binary"), "").unwrap();

    fs::create_dir_all(root.join("node_modules/package")).unwrap();
    fs::write(root.join("node_modules/package/index.js"), "").unwrap();

    let src_guide = r#"<agentic-navigation-guide>
- main.rs
</agentic-navigation-guide>"#;

    // These guides should be excluded
    let target_guide = r#"<agentic-navigation-guide>
- debug/
</agentic-navigation-guide>"#;

    let node_guide = r#"<agentic-navigation-guide>
- package/
</agentic-navigation-guide>"#;

    fs::write(root.join("src/AGENTIC_NAVIGATION_GUIDE.md"), src_guide).unwrap();
    fs::write(
        root.join("target/AGENTIC_NAVIGATION_GUIDE.md"),
        target_guide,
    )
    .unwrap();
    fs::write(
        root.join("node_modules/AGENTIC_NAVIGATION_GUIDE.md"),
        node_guide,
    )
    .unwrap();

    // Run recursive verify with exclusions
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--exclude")
        .arg("target")
        .arg("--exclude")
        .arg("node_modules")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 1 navigation guide(s)")); // Only src/
}

#[test]
fn test_recursive_verify_with_invalid_glob_pattern_reports_error() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--exclude")
        .arg("[")
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid glob pattern"));
}

#[test]
fn test_recursive_verify_with_ignored_guides() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create structure with ignored guide
    fs::create_dir_all(root.join("docs/examples")).unwrap();
    fs::write(root.join("docs/examples/demo.txt"), "").unwrap();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "").unwrap();

    let ignored_guide = r#"<agentic-navigation-guide ignore=true>
- nonexistent.txt
</agentic-navigation-guide>"#;

    let valid_guide = r#"<agentic-navigation-guide>
- main.rs
</agentic-navigation-guide>"#;

    fs::write(
        root.join("docs/examples/AGENTIC_NAVIGATION_GUIDE.md"),
        ignored_guide,
    )
    .unwrap();
    fs::write(root.join("src/AGENTIC_NAVIGATION_GUIDE.md"), valid_guide).unwrap();

    // Run recursive verify - should succeed and report ignored guide
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stderr(predicate::str::contains("ignore=true"));
}

#[test]
fn test_recursive_verify_with_non_ignore_attribute_is_not_skipped() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create structure with a guide that should NOT be treated as ignored.
    fs::create_dir_all(root.join("docs/examples")).unwrap();

    let non_ignored_guide = r#"<agentic-navigation-guide notignore=true>
- missing.txt
</agentic-navigation-guide>"#;

    fs::write(
        root.join("docs/examples/AGENTIC_NAVIGATION_GUIDE.md"),
        non_ignored_guide,
    )
    .unwrap();

    // Run recursive verify - should fail because the guide is not ignored.
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing.txt"))
        .stderr(predicate::str::contains("ignore=true").not());
}

#[test]
fn test_recursive_verify_no_guides_found() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create structure with no guide files
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "").unwrap();

    // Run recursive verify
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stderr(predicate::str::contains("No navigation guide files"));
}

#[test]
fn test_recursive_verify_deeply_nested() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create deeply nested structure
    fs::create_dir_all(root.join("a/b/c/d")).unwrap();
    fs::write(root.join("a/b/c/d/deep.txt"), "").unwrap();

    let guide = r#"<agentic-navigation-guide>
- deep.txt
</agentic-navigation-guide>"#;

    fs::write(root.join("a/b/c/d/AGENTIC_NAVIGATION_GUIDE.md"), guide).unwrap();

    // Run recursive verify
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(root)
        .assert()
        .success();
}

#[test]
fn test_recursive_verify_github_actions_mode() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create structure with one valid and one invalid guide
    fs::create_dir_all(root.join("valid")).unwrap();
    fs::write(root.join("valid/file.txt"), "").unwrap();

    fs::create_dir_all(root.join("invalid")).unwrap();

    let valid_guide = r#"<agentic-navigation-guide>
- file.txt
</agentic-navigation-guide>"#;

    let invalid_guide = r#"<agentic-navigation-guide>
- missing.txt
</agentic-navigation-guide>"#;

    fs::write(root.join("valid/AGENTIC_NAVIGATION_GUIDE.md"), valid_guide).unwrap();
    fs::write(
        root.join("invalid/AGENTIC_NAVIGATION_GUIDE.md"),
        invalid_guide,
    )
    .unwrap();

    // Run recursive verify in GitHub Actions mode
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--github-actions-check")
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("❌"));
}

#[test]
fn test_recursive_verify_quiet_mode() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create valid structure
    fs::create_dir_all(root.join("module")).unwrap();
    fs::write(root.join("module/file.txt"), "").unwrap();

    let guide = r#"<agentic-navigation-guide>
- file.txt
</agentic-navigation-guide>"#;

    fs::write(root.join("module/AGENTIC_NAVIGATION_GUIDE.md"), guide).unwrap();

    // Run recursive verify in quiet mode
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--quiet")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_guide_name_requires_recursive() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Try to use --guide-name without --recursive
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide-name")
        .arg("GUIDE.md")
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "the following required arguments were not provided",
        ))
        .stderr(predicate::str::contains("--recursive"));
}

#[test]
fn test_exclude_requires_recursive() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Try to use --exclude without --recursive
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--exclude")
        .arg("target")
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "the following required arguments were not provided",
        ))
        .stderr(predicate::str::contains("--recursive"));
}

#[test]
fn test_guide_conflicts_with_recursive() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a guide file
    fs::write(
        root.join("GUIDE.md"),
        "<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide>",
    )
    .unwrap();

    // Try to use --guide with --recursive
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(root.join("GUIDE.md"))
        .arg("--recursive")
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"))
        .stderr(predicate::str::contains("--recursive"));
}

#[test]
fn test_recursive_conflicts_with_guide() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a guide file
    fs::write(
        root.join("GUIDE.md"),
        "<agentic-navigation-guide>\n- file.txt\n</agentic-navigation-guide>",
    )
    .unwrap();

    // Try to use --recursive with --guide (reversed order from previous test)
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--guide")
        .arg(root.join("GUIDE.md"))
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_guide_name_and_exclude_work_with_recursive() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create valid structure with custom guide name
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "").unwrap();
    fs::write(
        root.join("src/GUIDE.md"),
        "<agentic-navigation-guide>\n- main.rs\n</agentic-navigation-guide>",
    )
    .unwrap();

    fs::create_dir_all(root.join("target")).unwrap();
    fs::write(
        root.join("target/GUIDE.md"),
        "<agentic-navigation-guide>\n- binary\n</agentic-navigation-guide>",
    )
    .unwrap();

    // Verify that --guide-name and --exclude work correctly WITH --recursive
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--guide-name")
        .arg("GUIDE.md")
        .arg("--exclude")
        .arg("target")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 1 navigation guide(s)"));
}

#[test]
fn test_init_excludes_vcs_directories_by_default() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();
    let output_path = dir_path.join("GUIDE.md");

    // Create structure with VCS directories and nested files
    fs::create_dir_all(dir_path.join(".git/objects/pack")).unwrap();
    fs::write(dir_path.join(".git/config"), "[core]\n").unwrap();
    fs::write(dir_path.join(".git/objects/pack/file.pack"), "").unwrap();

    fs::create_dir_all(dir_path.join(".svn/pristine")).unwrap();
    fs::write(dir_path.join(".svn/pristine/data"), "").unwrap();

    fs::create_dir_all(dir_path.join(".hg")).unwrap();
    fs::write(dir_path.join(".hg/store"), "").unwrap();

    fs::create_dir(dir_path.join(".bzr")).unwrap();
    fs::create_dir(dir_path.join("CVS")).unwrap();
    fs::create_dir(dir_path.join("_darcs")).unwrap();

    // Create normal directories
    fs::create_dir(dir_path.join("src")).unwrap();
    fs::write(dir_path.join("src/main.rs"), "").unwrap();
    fs::write(dir_path.join("README.md"), "").unwrap();

    // Run init without any exclusions
    let mut cmd = get_command();
    cmd.arg("init")
        .arg("--output")
        .arg(&output_path)
        .arg("--root")
        .arg(dir_path)
        .assert()
        .success();

    // Verify VCS directories are excluded by default
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("- src/"), "Should contain src/");
    assert!(content.contains("- README.md"), "Should contain README.md");
    assert!(!content.contains(".git"), "Should NOT contain .git");
    assert!(!content.contains(".svn"), "Should NOT contain .svn");
    assert!(!content.contains(".hg"), "Should NOT contain .hg");
    assert!(!content.contains(".bzr"), "Should NOT contain .bzr");
    assert!(!content.contains("CVS"), "Should NOT contain CVS");
    assert!(!content.contains("_darcs"), "Should NOT contain _darcs");
}

#[test]
fn test_init_with_include_vcs_directories_flag() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();
    let output_path = dir_path.join("GUIDE.md");

    // Create structure with VCS directories
    fs::create_dir_all(dir_path.join(".git/objects")).unwrap();
    fs::write(dir_path.join(".git/config"), "[core]\n").unwrap();
    fs::write(dir_path.join(".git/objects/abc123"), "").unwrap();

    fs::create_dir(dir_path.join("src")).unwrap();
    fs::write(dir_path.join("src/main.rs"), "").unwrap();

    // Run init WITH --include-vcs-directories flag
    let mut cmd = get_command();
    cmd.arg("init")
        .arg("--output")
        .arg(&output_path)
        .arg("--root")
        .arg(dir_path)
        .arg("--include-vcs-directories")
        .assert()
        .success();

    // Verify VCS directories ARE included when flag is set
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("- src/"), "Should contain src/");
    assert!(
        content.contains("- .git/"),
        "Should contain .git/ when flag is set"
    );
    assert!(
        content.contains("  - config"),
        "Should contain nested .git files"
    );
}

#[test]
fn test_init_vcs_exclusions_with_user_exclusions() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();
    let output_path = dir_path.join("GUIDE.md");

    // Create structure with VCS directories and other directories
    fs::create_dir(dir_path.join(".git")).unwrap();
    fs::create_dir(dir_path.join("target")).unwrap();
    fs::create_dir(dir_path.join("node_modules")).unwrap();
    fs::create_dir(dir_path.join("src")).unwrap();
    fs::write(dir_path.join("src/main.rs"), "").unwrap();

    // Run init with user-specified exclusions (target, node_modules)
    // VCS exclusions should be automatic
    let mut cmd = get_command();
    cmd.arg("init")
        .arg("--output")
        .arg(&output_path)
        .arg("--root")
        .arg(dir_path)
        .arg("--exclude")
        .arg("target")
        .arg("--exclude")
        .arg("node_modules")
        .assert()
        .success();

    // Verify both VCS and user exclusions work together
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("- src/"), "Should contain src/");
    assert!(
        !content.contains(".git"),
        "Should NOT contain .git (auto-excluded)"
    );
    assert!(
        !content.contains("target"),
        "Should NOT contain target (user-excluded)"
    );
    assert!(
        !content.contains("node_modules"),
        "Should NOT contain node_modules (user-excluded)"
    );
}
