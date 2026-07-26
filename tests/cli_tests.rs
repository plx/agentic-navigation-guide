use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;
use tempfile::TempDir;

fn get_command() -> Command {
    Command::cargo_bin("agentic-navigation-guide").unwrap()
}

const GUIDE_SOURCE_SENTINEL: &str = "ISSUE49_SECRET_7f4a2d909b6c";
const ISSUE39_OPAQUE_BODY_SENTINEL: &str = "ISSUE39_OPAQUE_SECRET_0c6248a7";

fn isolated_command() -> Command {
    let mut command = get_command();
    command.timeout(Duration::from_secs(5));
    for variable in [
        "AGENTIC_NAVIGATION_GUIDE_PATH",
        "AGENTIC_NAVIGATION_GUIDE_ROOT",
        "AGENTIC_NAVIGATION_GUIDE_NAME",
        "AGENTIC_NAVIGATION_GUIDE_LOG_MODE",
        "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE",
    ] {
        command.env_remove(variable);
    }
    command
}

#[cfg(unix)]
fn create_guide_file_link(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn create_guide_file_link(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_file(target, link)
        .expect("Windows file-symlink capability is required for guide-input trust evidence");
}

#[cfg(windows)]
fn create_guide_directory_link(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(target, link)
        .expect("Windows directory-symlink capability is required for guide-input trust evidence");
}

#[derive(Clone, Copy, Debug)]
enum GuideDiagnosticMode {
    Default,
    Quiet,
    Verbose,
    PostToolUse,
    PreCommit,
    GitHubActions,
}

const GUIDE_DIAGNOSTIC_MODES: [GuideDiagnosticMode; 6] = [
    GuideDiagnosticMode::Default,
    GuideDiagnosticMode::Quiet,
    GuideDiagnosticMode::Verbose,
    GuideDiagnosticMode::PostToolUse,
    GuideDiagnosticMode::PreCommit,
    GuideDiagnosticMode::GitHubActions,
];

impl GuideDiagnosticMode {
    fn configure(self, command: &mut Command) {
        match self {
            Self::Default => {}
            Self::Quiet => {
                command.arg("--quiet");
            }
            Self::Verbose => {
                command.arg("--verbose");
            }
            Self::PostToolUse => {
                command.arg("--post-tool-use-hook");
            }
            Self::PreCommit => {
                command.arg("--pre-commit-hook");
            }
            Self::GitHubActions => {
                command.arg("--github-actions-check");
            }
        }
    }

    fn failure_code(self) -> i32 {
        match self {
            Self::PostToolUse => 2,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum GuideExecutionCase {
    Default,
    PostToolUse,
    PreCommit,
    GitHubActions,
}

const GUIDE_EXECUTION_CASES: [GuideExecutionCase; 4] = [
    GuideExecutionCase::Default,
    GuideExecutionCase::PostToolUse,
    GuideExecutionCase::PreCommit,
    GuideExecutionCase::GitHubActions,
];

impl GuideExecutionCase {
    fn configure(self, command: &mut Command) {
        command.arg("--execution-mode").arg(match self {
            Self::Default => "default",
            Self::PostToolUse => "post-tool-use",
            Self::PreCommit => "pre-commit-hook",
            Self::GitHubActions => "github-actions",
        });
    }
}

#[derive(Clone, Copy, Debug)]
enum GuideLogCase {
    Quiet,
    Default,
    Verbose,
}

const GUIDE_LOG_CASES: [GuideLogCase; 3] = [
    GuideLogCase::Quiet,
    GuideLogCase::Default,
    GuideLogCase::Verbose,
];

impl GuideLogCase {
    fn configure(self, command: &mut Command) {
        command.arg("--log-level").arg(match self {
            Self::Quiet => "quiet",
            Self::Default => "default",
            Self::Verbose => "verbose",
        });
    }
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_unsafe_guide_rejection(
    output: &Output,
    mode: GuideDiagnosticMode,
    logical_name: &str,
    forbidden_target: &Path,
) {
    let diagnostics = combined_output(output);
    assert_eq!(
        output.status.code(),
        Some(mode.failure_code()),
        "{mode:?} mode did not reject the unsafe guide:\n{diagnostics}"
    );
    assert!(
        diagnostics.contains("unsafe guide path") && diagnostics.contains(logical_name),
        "{mode:?} mode omitted the typed logical-path diagnostic:\n{diagnostics}"
    );
    assert!(
        !diagnostics.contains(GUIDE_SOURCE_SENTINEL),
        "{mode:?} mode disclosed guide target content:\n{diagnostics}"
    );
    assert!(
        !diagnostics.contains(&forbidden_target.display().to_string()),
        "{mode:?} mode disclosed the resolved external target:\n{diagnostics}"
    );
    if let Some(target_name) = forbidden_target.file_name().and_then(|name| name.to_str()) {
        assert!(
            !diagnostics.contains(target_name),
            "{mode:?} mode disclosed the resolved target basename:\n{diagnostics}"
        );
    }
    assert!(
        !diagnostics.contains("zero navigation guides were verified"),
        "{mode:?} mode converted an unsafe entry into absence:\n{diagnostics}"
    );
}

#[derive(Clone, Copy, Debug)]
enum RecursiveZeroMode {
    Default,
    Quiet,
    PostToolUse,
    PreCommit,
    GitHubActions,
}

const RECURSIVE_ZERO_MODES: [RecursiveZeroMode; 5] = [
    RecursiveZeroMode::Default,
    RecursiveZeroMode::Quiet,
    RecursiveZeroMode::PostToolUse,
    RecursiveZeroMode::PreCommit,
    RecursiveZeroMode::GitHubActions,
];

impl RecursiveZeroMode {
    fn configure(self, command: &mut Command) {
        match self {
            Self::Default => {}
            Self::Quiet => {
                command.arg("--quiet");
            }
            Self::PostToolUse => {
                command.arg("--post-tool-use-hook");
            }
            Self::PreCommit => {
                command.arg("--pre-commit-hook");
            }
            Self::GitHubActions => {
                command.arg("--github-actions-check");
            }
        }
    }

    fn failure_code(self) -> i32 {
        match self {
            Self::PostToolUse => 2,
            _ => 1,
        }
    }

    fn is_quiet(self) -> bool {
        matches!(self, Self::Quiet)
    }
}

#[derive(Clone, Copy, Debug)]
enum Issue39Surface {
    Check,
    SingleVerify,
    RecursiveVerify,
}

const ISSUE39_SURFACES: [Issue39Surface; 3] = [
    Issue39Surface::Check,
    Issue39Surface::SingleVerify,
    Issue39Surface::RecursiveVerify,
];

impl Issue39Surface {
    fn configure(self, command: &mut Command, guide_path: &Path, root: &Path) {
        match self {
            Self::Check => {
                command.arg("check").arg("--guide").arg(guide_path);
            }
            Self::SingleVerify => {
                command
                    .arg("verify")
                    .arg("--guide")
                    .arg(guide_path)
                    .arg("--root")
                    .arg(root);
            }
            Self::RecursiveVerify => {
                command
                    .arg("verify")
                    .arg("--recursive")
                    .arg("--root")
                    .arg(root)
                    .arg("--guide-name")
                    .arg("AGENTIC_NAVIGATION_GUIDE.md");
            }
        }
    }

    fn is_recursive(self) -> bool {
        matches!(self, Self::RecursiveVerify)
    }
}

#[derive(Clone, Copy, Debug)]
enum Issue39Mode {
    Default,
    Quiet,
    QuietGitHubActions,
    PostToolUse,
    PreCommit,
    GitHubActions,
}

const ISSUE39_MODES: [Issue39Mode; 6] = [
    Issue39Mode::Default,
    Issue39Mode::Quiet,
    Issue39Mode::QuietGitHubActions,
    Issue39Mode::PostToolUse,
    Issue39Mode::PreCommit,
    Issue39Mode::GitHubActions,
];

impl Issue39Mode {
    fn configure(self, command: &mut Command) {
        match self {
            Self::Default => {}
            Self::Quiet => {
                command.arg("--quiet");
            }
            Self::QuietGitHubActions => {
                command.arg("--quiet").arg("--github-actions-check");
            }
            Self::PostToolUse => {
                command.arg("--post-tool-use-hook");
            }
            Self::PreCommit => {
                command.arg("--pre-commit-hook");
            }
            Self::GitHubActions => {
                command.arg("--github-actions-check");
            }
        }
    }

    fn failure_code(self) -> i32 {
        match self {
            Self::PostToolUse => 2,
            _ => 1,
        }
    }

    fn is_quiet(self) -> bool {
        matches!(self, Self::Quiet | Self::QuietGitHubActions)
    }
}

#[derive(Clone, Copy, Debug)]
enum Issue39Body {
    Valid,
    InvalidList,
    InvalidIndentation,
    InvalidPath,
    InvalidChoice,
    InvalidPlaceholder,
    MissingFilesystemEntry,
    Empty,
}

const ISSUE39_BODIES: [Issue39Body; 8] = [
    Issue39Body::Valid,
    Issue39Body::InvalidList,
    Issue39Body::InvalidIndentation,
    Issue39Body::InvalidPath,
    Issue39Body::InvalidChoice,
    Issue39Body::InvalidPlaceholder,
    Issue39Body::MissingFilesystemEntry,
    Issue39Body::Empty,
];

impl Issue39Body {
    fn source(self) -> String {
        let (opening, body) = match self {
            Self::Valid => (
                "<agentic-navigation-guide   ignore = \"true\"  >",
                "- present.txt",
            ),
            Self::InvalidList => (
                "<agentic-navigation-guide ignore=true>",
                "this is deliberately not a list",
            ),
            Self::InvalidIndentation => (
                "<agentic-navigation-guide ignore=true>",
                "- directory/\n  - child.txt\n   - crooked.txt",
            ),
            Self::InvalidPath => (
                "<agentic-navigation-guide ignore=true>",
                "- ../outside-root.txt",
            ),
            Self::InvalidChoice => (
                "<agentic-navigation-guide ignore=true>",
                "- ISSUE39_OPAQUE_SECRET_0c6248a7[].txt",
            ),
            Self::InvalidPlaceholder => (
                "<agentic-navigation-guide ignore=true>",
                "- ...\n- ... # adjacent placeholder",
            ),
            Self::MissingFilesystemEntry => (
                "<agentic-navigation-guide ignore=true>",
                "- deliberately-missing.txt",
            ),
            Self::Empty => ("<agentic-navigation-guide ignore=\"true\">", ""),
        };

        if body.is_empty() {
            format!("{opening}\n</agentic-navigation-guide>")
        } else {
            format!("{opening}\n{body}\n</agentic-navigation-guide>")
        }
    }
}

fn run_issue39_ignored_case(
    surface: Issue39Surface,
    mode: Issue39Mode,
    guide_path: &Path,
    root: &Path,
    deny_ignored: bool,
) -> Output {
    let mut command = isolated_command();
    surface.configure(&mut command, guide_path, root);
    mode.configure(&mut command);
    if deny_ignored {
        command.arg("--deny-ignored");
    }
    command.output().unwrap()
}

fn assert_no_issue39_false_success(diagnostics: &str, context: &str) {
    let lowercase = diagnostics.to_ascii_lowercase();
    for false_success in [
        "syntax valid",
        "syntax is valid",
        "navigation guide verified",
        "navigation guide is valid and matches filesystem",
        "all navigation guides verified",
        "all navigation guides are valid and match filesystem",
        ": verified",
    ] {
        assert!(
            !lowercase.contains(false_success),
            "{context} falsely reported ignored work as verified with '{false_success}':\n{diagnostics}"
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum ZeroDiscoveryCase {
    EmptyTree,
    NoMatchingGuide,
    TypoInGuideName,
    WrongRoot,
    AllGuidesExcluded,
    LastGuideDeleted,
}

const ZERO_DISCOVERY_CASES: [ZeroDiscoveryCase; 6] = [
    ZeroDiscoveryCase::EmptyTree,
    ZeroDiscoveryCase::NoMatchingGuide,
    ZeroDiscoveryCase::TypoInGuideName,
    ZeroDiscoveryCase::WrongRoot,
    ZeroDiscoveryCase::AllGuidesExcluded,
    ZeroDiscoveryCase::LastGuideDeleted,
];

struct ZeroDiscoveryFixture {
    _temp: TempDir,
    search_root: PathBuf,
    guide_name: &'static str,
    exclusions: Vec<&'static str>,
}

impl ZeroDiscoveryCase {
    fn fixture(self) -> ZeroDiscoveryFixture {
        const DEFAULT_NAME: &str = "AGENTIC_NAVIGATION_GUIDE.md";
        const TYPO_NAME: &str = "AGENTIC_NAVIGATION_GUDIE.md";
        const VALID_GUIDE: &str =
            "<agentic-navigation-guide>\n- payload.txt\n</agentic-navigation-guide>";

        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let mut search_root = root.to_path_buf();
        let mut guide_name = DEFAULT_NAME;
        let mut exclusions = Vec::new();

        match self {
            Self::EmptyTree => {}
            Self::NoMatchingGuide => {
                fs::write(root.join("README.md"), "not a navigation guide").unwrap();
            }
            Self::TypoInGuideName => {
                fs::write(root.join("payload.txt"), "").unwrap();
                fs::write(root.join(DEFAULT_NAME), VALID_GUIDE).unwrap();
                guide_name = TYPO_NAME;
            }
            Self::WrongRoot => {
                let actual_root = root.join("actual");
                fs::create_dir(&actual_root).unwrap();
                fs::write(actual_root.join("payload.txt"), "").unwrap();
                fs::write(actual_root.join(DEFAULT_NAME), VALID_GUIDE).unwrap();

                search_root = root.join("wrong");
                fs::create_dir(&search_root).unwrap();
            }
            Self::AllGuidesExcluded => {
                let excluded = root.join("excluded");
                fs::create_dir(&excluded).unwrap();
                fs::write(excluded.join("payload.txt"), "").unwrap();
                fs::write(excluded.join(DEFAULT_NAME), VALID_GUIDE).unwrap();
                exclusions.push("excluded");
            }
            Self::LastGuideDeleted => {
                let guide = root.join(DEFAULT_NAME);
                fs::write(root.join("payload.txt"), "").unwrap();
                fs::write(&guide, VALID_GUIDE).unwrap();
                fs::remove_file(guide).unwrap();
            }
        }

        ZeroDiscoveryFixture {
            _temp: temp,
            search_root,
            guide_name,
            exclusions,
        }
    }
}

fn run_recursive_zero_case(
    fixture: &ZeroDiscoveryFixture,
    mode: RecursiveZeroMode,
    allow_empty: bool,
) -> Output {
    let mut command = get_command();
    for variable in [
        "AGENTIC_NAVIGATION_GUIDE_PATH",
        "AGENTIC_NAVIGATION_GUIDE_ROOT",
        "AGENTIC_NAVIGATION_GUIDE_NAME",
        "AGENTIC_NAVIGATION_GUIDE_LOG_MODE",
        "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE",
    ] {
        command.env_remove(variable);
    }
    command
        .arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(&fixture.search_root)
        .arg("--guide-name")
        .arg(fixture.guide_name);

    for exclusion in &fixture.exclusions {
        command.arg("--exclude").arg(exclusion);
    }
    if allow_empty {
        command.arg("--allow-empty");
    }
    mode.configure(&mut command);

    command.output().unwrap()
}

#[derive(Clone, Copy, Debug)]
enum FileOutputCommand {
    Init,
    Dump,
}

const FILE_OUTPUT_COMMANDS: [FileOutputCommand; 2] =
    [FileOutputCommand::Init, FileOutputCommand::Dump];

fn run_file_output(
    command: FileOutputCommand,
    root: &Path,
    output: &Path,
    extra_args: &[&str],
) -> Output {
    let mut invocation = get_command();
    invocation
        .arg(match command {
            FileOutputCommand::Init => "init",
            FileOutputCommand::Dump => "dump",
        })
        .arg("--root")
        .arg(root)
        .arg("--output")
        .arg(output)
        .args(extra_args);
    invocation.output().unwrap()
}

fn write_concatenated_ignore_bypass(
    root: &std::path::Path,
    guide_name: &str,
) -> std::path::PathBuf {
    let guide_path = root.join(guide_name);
    fs::write(
        &guide_path,
        "<agentic-navigation-guideignore=true>\n- definitely-missing.txt\n</agentic-navigation-guide>",
    )
    .unwrap();
    guide_path
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
fn test_check_rejects_concatenated_ignore_marker() {
    let temp_dir = TempDir::new().unwrap();
    let guide_path = write_concatenated_ignore_bypass(temp_dir.path(), "GUIDE.md");

    let mut cmd = get_command();
    cmd.arg("check")
        .arg("--guide")
        .arg(&guide_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("line 1"))
        .stderr(predicate::str::contains("invalid guide document"))
        .stderr(predicate::str::contains("missing opening"))
        .stderr(predicate::str::contains("Skipping").not());
}

#[test]
fn test_verify_rejects_concatenated_ignore_marker_with_missing_path() {
    let temp_dir = TempDir::new().unwrap();
    let guide_path = write_concatenated_ignore_bypass(temp_dir.path(), "GUIDE.md");

    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--guide")
        .arg(&guide_path)
        .arg("--root")
        .arg(temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("line 1"))
        .stderr(predicate::str::contains("invalid guide document"))
        .stderr(predicate::str::contains("missing opening"))
        .stderr(predicate::str::contains("Skipping").not());
}

#[test]
fn test_recursive_verify_rejects_concatenated_ignore_marker_with_missing_path() {
    let temp_dir = TempDir::new().unwrap();
    write_concatenated_ignore_bypass(temp_dir.path(), "AGENTIC_NAVIGATION_GUIDE.md");

    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("line 1"))
        .stderr(predicate::str::contains("invalid guide document"))
        .stderr(predicate::str::contains("missing opening"))
        .stderr(predicate::str::contains("Skipping").not());
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
        .stderr(predicate::str::contains("already exists"))
        .stderr(predicate::str::contains("--force").not());
}

#[cfg(unix)]
#[test]
fn test_init_rejects_dangling_output_symlink_without_creating_target() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    let output_parent = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    fs::write(root.path().join("input.txt"), "input").unwrap();

    let external_target = external.path().join("must-not-be-created.md");
    let output_link = output_parent.path().join("output-link.md");
    symlink(&external_target, &output_link).unwrap();

    let output = get_command()
        .arg("init")
        .arg("--root")
        .arg(root.path())
        .arg("--output")
        .arg(&output_link)
        .output()
        .unwrap();

    assert!(fs::symlink_metadata(&output_link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_link(&output_link).unwrap(), external_target);
    assert!(
        !external_target.exists(),
        "init followed the dangling output symlink and created its external target"
    );
    assert!(!output.status.success());
}

#[cfg(unix)]
#[test]
fn test_dump_rejects_dangling_output_symlink_without_creating_target() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    let output_parent = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    fs::write(root.path().join("input.txt"), "input").unwrap();

    let external_target = external.path().join("must-not-be-created.md");
    let output_link = output_parent.path().join("output-link.md");
    symlink(&external_target, &output_link).unwrap();

    let output = get_command()
        .arg("dump")
        .arg("--root")
        .arg(root.path())
        .arg("--output")
        .arg(&output_link)
        .output()
        .unwrap();

    assert!(fs::symlink_metadata(&output_link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_link(&output_link).unwrap(), external_target);
    assert!(
        !external_target.exists(),
        "dump followed the dangling output symlink and created its external target"
    );
    assert!(!output.status.success());
}

#[test]
fn test_dump_rejects_existing_regular_output_without_modifying_it() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("input.txt"), "input").unwrap();

    let output_path = root.path().join("existing.md");
    let sentinel = b"existing output sentinel";
    fs::write(&output_path, sentinel).unwrap();

    let output = get_command()
        .arg("dump")
        .arg("--root")
        .arg(root.path())
        .arg("--output")
        .arg(&output_path)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(fs::read(&output_path).unwrap(), sentinel);
}

#[cfg(unix)]
#[test]
fn test_init_rejects_link_ancestor_below_root_without_external_creation() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    fs::write(root.path().join("input.txt"), "input").unwrap();
    symlink(external.path(), root.path().join("linked")).unwrap();

    let output_path = root.path().join("linked/output.md");
    let output = get_command()
        .arg("init")
        .arg("--root")
        .arg(root.path())
        .arg("--output")
        .arg(&output_path)
        .arg("--exclude")
        .arg("linked")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!external.path().join("output.md").exists());
}

#[test]
fn output_new_destination_matrix() {
    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();

        let in_root = root.path().join(format!("{command:?}-in-root.md"));
        let result = run_file_output(command, root.path(), &in_root, &[]);
        assert!(
            result.status.success(),
            "{command:?} did not create a new in-root destination: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let content = fs::read_to_string(&in_root).unwrap();
        assert!(content.contains("<agentic-navigation-guide>"));
        assert!(content.contains("- input.txt"));
        assert!(
            !content.contains(&format!("- {command:?}-in-root.md")),
            "{command:?} included its absent destination in generated input"
        );
        assert!(fs::metadata(&in_root).unwrap().is_file());

        let external = TempDir::new().unwrap();
        let external_output = external.path().join(format!("{command:?}-external.md"));
        let result = run_file_output(command, root.path(), &external_output, &[]);
        assert!(
            result.status.success(),
            "{command:?} did not create an explicitly selected external destination: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(fs::metadata(external_output).unwrap().is_file());
    }
}

#[test]
fn output_existing_entry_matrix() {
    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();
        let output_parent = TempDir::new().unwrap();

        let regular = output_parent.path().join(format!("{command:?}-regular.md"));
        fs::write(&regular, b"regular sentinel").unwrap();
        let result = run_file_output(command, root.path(), &regular, &[]);
        assert!(!result.status.success());
        assert_eq!(fs::read(&regular).unwrap(), b"regular sentinel");

        let directory = output_parent.path().join(format!("{command:?}-directory"));
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("child"), b"directory sentinel").unwrap();
        let result = run_file_output(command, root.path(), &directory, &[]);
        assert!(!result.status.success());
        assert!(directory.is_dir());
        assert_eq!(
            fs::read(directory.join("child")).unwrap(),
            b"directory sentinel"
        );
    }
}

#[cfg(unix)]
#[test]
fn output_existing_hard_link_matrix() {
    use std::os::unix::fs::MetadataExt;

    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();
        let output_parent = TempDir::new().unwrap();
        let source = output_parent.path().join(format!("{command:?}-source"));
        let output = output_parent.path().join(format!("{command:?}-hard-link"));
        fs::write(&source, b"hard-link sentinel").unwrap();
        fs::hard_link(&source, &output).unwrap();
        let identity = (
            fs::metadata(&source).unwrap().dev(),
            fs::metadata(&source).unwrap().ino(),
        );

        let result = run_file_output(command, root.path(), &output, &[]);

        assert!(!result.status.success());
        assert_eq!(fs::read(&source).unwrap(), b"hard-link sentinel");
        assert_eq!(fs::read(&output).unwrap(), b"hard-link sentinel");
        assert_eq!(
            (
                fs::metadata(&output).unwrap().dev(),
                fs::metadata(&output).unwrap().ino()
            ),
            identity
        );
    }
}

#[cfg(windows)]
#[test]
fn output_existing_hard_link_matrix() {
    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();
        let output_parent = TempDir::new().unwrap();
        let source = output_parent.path().join(format!("{command:?}-source"));
        let output = output_parent.path().join(format!("{command:?}-hard-link"));
        fs::write(&source, b"hard-link sentinel").unwrap();
        fs::hard_link(&source, &output).unwrap();

        let result = run_file_output(command, root.path(), &output, &[]);

        assert!(!result.status.success());
        assert_eq!(fs::read(&source).unwrap(), b"hard-link sentinel");
        assert_eq!(fs::read(&output).unwrap(), b"hard-link sentinel");
    }
}

#[cfg(unix)]
#[test]
fn output_final_link_matrix() {
    use std::os::unix::fs::symlink;

    for command in FILE_OUTPUT_COMMANDS {
        for relative in [false, true] {
            let root = TempDir::new().unwrap();
            fs::write(root.path().join("input.txt"), "input").unwrap();
            let output_parent = TempDir::new().unwrap();
            let target_parent = TempDir::new().unwrap();
            let target = if relative {
                output_parent
                    .path()
                    .join(format!("{command:?}-{relative}-target"))
            } else {
                target_parent
                    .path()
                    .join(format!("{command:?}-{relative}-target"))
            };
            let link = output_parent
                .path()
                .join(format!("{command:?}-{relative}-link"));
            fs::write(&target, b"linked target sentinel").unwrap();
            if relative {
                symlink(target.file_name().unwrap(), &link).unwrap();
            } else {
                symlink(&target, &link).unwrap();
            }
            let original_target = fs::read_link(&link).unwrap();

            let result = run_file_output(command, root.path(), &link, &[]);

            assert!(!result.status.success());
            assert!(fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink());
            assert_eq!(fs::read_link(&link).unwrap(), original_target);
            assert_eq!(fs::read(&target).unwrap(), b"linked target sentinel");
        }

        let root = TempDir::new().unwrap();
        let output_parent = TempDir::new().unwrap();
        let in_root_target = root.path().join("in-root-target");
        fs::write(&in_root_target, b"in-root target sentinel").unwrap();
        let in_root_link = output_parent
            .path()
            .join(format!("{command:?}-in-root-link"));
        symlink(&in_root_target, &in_root_link).unwrap();
        let result = run_file_output(command, root.path(), &in_root_link, &[]);
        assert!(!result.status.success());
        assert_eq!(
            fs::read(in_root_target).unwrap(),
            b"in-root target sentinel"
        );

        let directory_target = output_parent
            .path()
            .join(format!("{command:?}-target-directory"));
        fs::create_dir(&directory_target).unwrap();
        fs::write(directory_target.join("sentinel"), b"directory").unwrap();
        let directory_link = output_parent
            .path()
            .join(format!("{command:?}-directory-link"));
        symlink(&directory_target, &directory_link).unwrap();
        let result = run_file_output(command, root.path(), &directory_link, &[]);
        assert!(!result.status.success());
        assert_eq!(
            fs::read(directory_target.join("sentinel")).unwrap(),
            b"directory"
        );
    }
}

#[cfg(unix)]
#[test]
fn output_link_chain_and_loop_matrix() {
    use std::os::unix::fs::symlink;

    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();
        let output_parent = TempDir::new().unwrap();
        let terminal_parent = TempDir::new().unwrap();

        let terminal = terminal_parent.path().join("must-not-be-created");
        let second = output_parent.path().join(format!("{command:?}-second"));
        let chain = output_parent.path().join(format!("{command:?}-chain"));
        symlink(&terminal, &second).unwrap();
        symlink(&second, &chain).unwrap();
        let result = run_file_output(command, root.path(), &chain, &[]);
        assert!(!result.status.success());
        assert_eq!(fs::read_link(&chain).unwrap(), second);
        assert!(!terminal.exists());

        let first_loop = output_parent.path().join(format!("{command:?}-loop-a"));
        let second_loop = output_parent.path().join(format!("{command:?}-loop-b"));
        symlink(&second_loop, &first_loop).unwrap();
        symlink(&first_loop, &second_loop).unwrap();
        let result = run_file_output(command, root.path(), &first_loop, &[]);
        assert!(!result.status.success());
        assert_eq!(fs::read_link(&first_loop).unwrap(), second_loop);
        assert_eq!(fs::read_link(&second_loop).unwrap(), first_loop);
    }
}

#[cfg(unix)]
#[test]
fn output_existing_special_entry_matrix() {
    use std::fs::OpenOptions;
    use std::os::unix::fs::FileTypeExt;
    use std::os::unix::net::UnixListener;
    use std::process::Command as ProcessCommand;

    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();
        let output_parent = TempDir::new().unwrap();

        let fifo = output_parent.path().join(format!("{command:?}-fifo"));
        let mkfifo = ProcessCommand::new("mkfifo").arg(&fifo).output().unwrap();
        assert!(
            mkfifo.status.success(),
            "mkfifo unavailable: {}",
            String::from_utf8_lossy(&mkfifo.stderr)
        );
        // Keep both ends open so an implementation that incorrectly opens
        // the FIFO for writing returns and fails the sentinel checks instead
        // of hanging the regression suite.
        let fifo_guard = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&fifo)
            .unwrap();
        let result = run_file_output(command, root.path(), &fifo, &[]);
        assert!(!result.status.success());
        assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
        drop(fifo_guard);

        let socket = output_parent.path().join(format!("{command:?}-socket"));
        let listener = UnixListener::bind(&socket).unwrap();
        let result = run_file_output(command, root.path(), &socket, &[]);
        assert!(!result.status.success());
        assert!(fs::symlink_metadata(&socket)
            .unwrap()
            .file_type()
            .is_socket());
        drop(listener);

        let null_device = Path::new("/dev/null");
        assert!(fs::symlink_metadata(null_device)
            .unwrap()
            .file_type()
            .is_char_device());
        let result = run_file_output(command, root.path(), null_device, &[]);
        assert!(!result.status.success());
    }
}

#[test]
fn output_missing_parent_matrix() {
    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();
        let output_parent = TempDir::new().unwrap();
        let missing_parent = output_parent.path().join("missing/nested");
        let output = missing_parent.join(format!("{command:?}.md"));

        let result = run_file_output(command, root.path(), &output, &[]);

        assert!(!result.status.success());
        assert!(!missing_parent.exists());
        assert!(!output.exists());
    }
}

#[test]
fn output_terminal_dot_component_never_retargets_the_parent_name() {
    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();
        let output_parent = TempDir::new().unwrap();
        let unintended_name = output_parent
            .path()
            .join(format!("{command:?}-must-not-exist"));
        let requested = unintended_name.join(".");

        let result = run_file_output(command, root.path(), &requested, &[]);

        assert!(!result.status.success());
        assert!(
            !unintended_name.exists(),
            "{command:?} retargeted a terminal dot component to its parent name"
        );
    }
}

#[cfg(unix)]
#[test]
fn output_read_only_parent_matrix() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempDir::new().unwrap();
    fs::write(root.path().join("input.txt"), "input").unwrap();
    let output_parent = TempDir::new().unwrap();
    let original_permissions = fs::metadata(output_parent.path()).unwrap().permissions();
    fs::set_permissions(output_parent.path(), fs::Permissions::from_mode(0o555)).unwrap();

    let observations: Vec<_> = FILE_OUTPUT_COMMANDS
        .into_iter()
        .map(|command| {
            let output = output_parent.path().join(format!("{command:?}.md"));
            let result = run_file_output(command, root.path(), &output, &[]);
            (command, result.status.success(), output.exists())
        })
        .collect();

    fs::set_permissions(output_parent.path(), original_permissions).unwrap();
    for (command, succeeded, exists) in observations {
        assert!(!succeeded, "{command:?} accepted a read-only parent");
        assert!(!exists, "{command:?} left output in a read-only parent");
    }
}

#[cfg(unix)]
#[test]
fn output_in_root_link_ancestor_matrix() {
    use std::os::unix::fs::symlink;

    for command in FILE_OUTPUT_COMMANDS {
        for target_kind in ["in-root", "external", "dangling"] {
            let root = TempDir::new().unwrap();
            fs::write(root.path().join("input.txt"), "input").unwrap();
            let external = TempDir::new().unwrap();
            let in_root_target = root.path().join("real");
            fs::create_dir(&in_root_target).unwrap();
            let target = match target_kind {
                "in-root" => in_root_target,
                "external" => external.path().to_path_buf(),
                "dangling" => external.path().join("missing"),
                _ => unreachable!(),
            };
            let linked = root.path().join("linked");
            symlink(&target, &linked).unwrap();
            let output = linked.join(format!("{command:?}.md"));

            let result = run_file_output(command, root.path(), &output, &["--exclude", "linked"]);

            assert!(
                !result.status.success(),
                "{command:?} accepted a {target_kind} link ancestor"
            );
            assert!(!target.join(format!("{command:?}.md")).exists());
        }
    }
}

#[cfg(unix)]
#[test]
fn output_external_link_ancestor_matrix() {
    use std::os::unix::fs::symlink;

    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();
        let link_parent = TempDir::new().unwrap();
        let external = TempDir::new().unwrap();
        let linked = link_parent.path().join("explicit-external-link");
        symlink(external.path(), &linked).unwrap();
        let output = linked.join(format!("{command:?}.md"));

        let result = run_file_output(command, root.path(), &output, &[]);

        assert!(
            result.status.success(),
            "{command:?} rejected an explicitly selected external link ancestor: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(external.path().join(format!("{command:?}.md")).is_file());
    }
}

#[cfg(unix)]
#[test]
fn output_root_alias_matrix() {
    use std::os::unix::fs::symlink;

    for command in FILE_OUTPUT_COMMANDS {
        let real_root = TempDir::new().unwrap();
        fs::write(real_root.path().join("input.txt"), "input").unwrap();
        let alias_parent = TempDir::new().unwrap();
        let root_alias = alias_parent.path().join("selected-root-alias");
        symlink(real_root.path(), &root_alias).unwrap();
        let output = root_alias.join(format!("{command:?}.md"));

        let result = run_file_output(command, &root_alias, &output, &[]);

        assert!(
            result.status.success(),
            "{command:?} rejected a destination beneath the selected root alias: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(real_root.path().join(format!("{command:?}.md")).is_file());
    }
}

#[cfg(unix)]
#[test]
fn output_preserves_selected_root_spelling_with_unresolved_parent_components() {
    for command in FILE_OUTPUT_COMMANDS {
        let base = TempDir::new().unwrap();
        fs::create_dir(base.path().join("anchor")).unwrap();
        fs::create_dir(base.path().join("real-child")).unwrap();
        fs::write(base.path().join("input.txt"), "input").unwrap();
        let root_spelling = base.path().join("anchor/..");
        let output = root_spelling
            .join("real-child")
            .join(format!("{command:?}.md"));

        let result = run_file_output(command, &root_spelling, &output, &[]);

        assert!(
            result.status.success(),
            "{command:?} normalized the selected root spelling before authority classification: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(base
            .path()
            .join("real-child")
            .join(format!("{command:?}.md"))
            .is_file());
    }
}

#[test]
fn explicit_output_with_parent_tail_receives_external_authority() {
    for command in FILE_OUTPUT_COMMANDS {
        let base = TempDir::new().unwrap();
        let root = base.path().join("root");
        let outside = base.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(root.join("input.txt"), "input").unwrap();
        let output = root
            .join("..")
            .join("outside")
            .join(format!("{command:?}.md"));

        let result = run_file_output(command, &root, &output, &[]);

        assert!(
            result.status.success(),
            "{command:?} did not treat a parent-component tail as an explicit external grant: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(outside.join(format!("{command:?}.md")).is_file());
    }
}

#[test]
fn output_preflight_and_generation_order_matrix() {
    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();

        let existing = root.path().join(format!("{command:?}-existing.md"));
        fs::write(&existing, b"preflight sentinel").unwrap();
        let result = run_file_output(command, root.path(), &existing, &["--exclude", "["]);
        let diagnostics = String::from_utf8_lossy(&result.stderr);
        assert!(!result.status.success());
        assert!(
            diagnostics.contains("already exists"),
            "{command:?} generated before rejecting a pre-existing output: {diagnostics}"
        );
        assert!(!diagnostics.contains("glob pattern"));
        assert_eq!(fs::read(existing).unwrap(), b"preflight sentinel");

        let absent = root
            .path()
            .join(format!("{command:?}-generation-failure.md"));
        let result = run_file_output(command, root.path(), &absent, &["--exclude", "["]);
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("glob pattern"),
            "{command:?} did not surface the injected generation failure"
        );
        assert!(
            !absent.exists(),
            "{command:?} created output before generation completed"
        );
    }
}

#[cfg(unix)]
#[test]
fn init_competing_creator_never_gets_overwritten_in_100_races() {
    use std::fs::OpenOptions;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::process::{Command as ProcessCommand, Stdio};

    let root = TempDir::new().unwrap();
    for index in 0..500 {
        fs::write(root.path().join(format!("input-{index:03}.txt")), "input").unwrap();
    }
    let output_parent = TempDir::new().unwrap();
    let competitor_bytes = b"competing creator sentinel";

    for iteration in 0..100 {
        let output = output_parent.path().join(format!("race-{iteration:03}.md"));
        let mut child = ProcessCommand::new(env!("CARGO_BIN_EXE_agentic-navigation-guide"))
            .arg("init")
            .arg("--root")
            .arg(root.path())
            .arg("--output")
            .arg(&output)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stderr = child.stderr.take().unwrap();
        let mut stderr = BufReader::new(stderr);
        let mut diagnostics = String::new();
        loop {
            let mut line = String::new();
            let read = stderr.read_line(&mut line).unwrap();
            diagnostics.push_str(&line);
            if line.contains("Initializing navigation guide") || read == 0 {
                break;
            }
        }
        assert!(
            diagnostics.contains("Initializing navigation guide"),
            "init exited before reaching generation in race {iteration}: {diagnostics}"
        );

        let competitor = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output);
        let competitor_won = match competitor {
            Ok(mut file) => {
                file.write_all(competitor_bytes).unwrap();
                file.flush().unwrap();
                file.sync_data().unwrap();
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(error) => panic!("unexpected competing create error in race {iteration}: {error}"),
        };

        stderr.read_to_string(&mut diagnostics).unwrap();
        let status = child.wait().unwrap();
        assert_ne!(
            status.success(),
            competitor_won,
            "race {iteration} did not produce exactly one successful creator: {diagnostics}"
        );
        let bytes = fs::read(&output).unwrap();
        if competitor_won {
            assert_eq!(
                bytes, competitor_bytes,
                "init overwrote the competing winner in race {iteration}"
            );
        } else {
            assert!(
                bytes.starts_with(b"# Agentic Navigation Guide"),
                "init won race {iteration} but its bytes were not retained"
            );
        }
    }
}

#[cfg(windows)]
#[test]
fn windows_output_reparse_matrix() {
    use std::os::windows::fs::{symlink_dir, symlink_file};

    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();
        let output_parent = TempDir::new().unwrap();
        let external = TempDir::new().unwrap();

        let existing_target = external.path().join(format!("{command:?}-target"));
        fs::write(&existing_target, b"Windows target sentinel").unwrap();
        let file_link = output_parent.path().join(format!("{command:?}-file-link"));
        symlink_file(&existing_target, &file_link)
            .expect("Windows file-symlink capability is required for output trust evidence");
        let result = run_file_output(command, root.path(), &file_link, &[]);
        assert!(!result.status.success());
        assert_eq!(
            fs::read(&existing_target).unwrap(),
            b"Windows target sentinel"
        );

        let dangling_target = external.path().join(format!("{command:?}-missing"));
        let dangling_link = output_parent
            .path()
            .join(format!("{command:?}-dangling-link"));
        symlink_file(&dangling_target, &dangling_link)
            .expect("Windows dangling-symlink capability is required for output trust evidence");
        let result = run_file_output(command, root.path(), &dangling_link, &[]);
        assert!(!result.status.success());
        assert!(!dangling_target.exists());

        let directory_target = external
            .path()
            .join(format!("{command:?}-directory-target"));
        fs::create_dir(&directory_target).unwrap();
        fs::write(directory_target.join("sentinel"), b"directory").unwrap();
        let directory_link = output_parent
            .path()
            .join(format!("{command:?}-directory-link"));
        symlink_dir(&directory_target, &directory_link)
            .expect("Windows directory-symlink capability is required for output trust evidence");
        let result = run_file_output(command, root.path(), &directory_link, &[]);
        assert!(!result.status.success());
        assert_eq!(
            fs::read(directory_target.join("sentinel")).unwrap(),
            b"directory"
        );

        let in_root_link = root.path().join(format!("{command:?}-linked"));
        symlink_dir(external.path(), &in_root_link)
            .expect("Windows ancestor-reparse capability is required for output trust evidence");
        let in_root_output = in_root_link.join("must-not-be-created.md");
        let result = run_file_output(
            command,
            root.path(),
            &in_root_output,
            &["--exclude", &format!("{command:?}-linked")],
        );
        assert!(!result.status.success());
        assert!(!external.path().join("must-not-be-created.md").exists());

        let external_link = output_parent
            .path()
            .join(format!("{command:?}-external-link"));
        symlink_dir(external.path(), &external_link)
            .expect("Windows external-reparse capability is required for output trust evidence");
        let external_output = external_link.join(format!("{command:?}-allowed.md"));
        let result = run_file_output(command, root.path(), &external_output, &[]);
        assert!(
            result.status.success(),
            "explicit external Windows reparse ancestor was rejected: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(external
            .path()
            .join(format!("{command:?}-allowed.md"))
            .is_file());

        let alias_root_target = TempDir::new().unwrap();
        fs::write(alias_root_target.path().join("input.txt"), "input").unwrap();
        let alias_parent = TempDir::new().unwrap();
        let root_alias = alias_parent.path().join(format!("{command:?}-root-alias"));
        symlink_dir(alias_root_target.path(), &root_alias)
            .expect("Windows root-alias capability is required for output trust evidence");
        let alias_output = root_alias.join(format!("{command:?}-alias-output.md"));
        let result = run_file_output(command, &root_alias, &alias_output, &[]);
        assert!(
            result.status.success(),
            "Windows selected-root alias was rejected: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_output_namespaces_and_streams_reject_before_access() {
    use std::path::PathBuf;

    for command in FILE_OUTPUT_COMMANDS {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("input.txt"), "input").unwrap();
        let output_parent = TempDir::new().unwrap();
        let base = output_parent.path().join(format!("{command:?}-base.txt"));
        let ads = PathBuf::from(format!("{}:secret", base.display()));

        for output in [
            ads,
            output_parent.path().join("NUL.txt"),
            output_parent.path().join("CONIN$.txt"),
            output_parent.path().join("CONOUT$"),
            output_parent.path().join("COM¹.log"),
            output_parent.path().join("bad?.md"),
            output_parent.path().join("bad|name.md"),
            PathBuf::from(r"\\.\NUL"),
            PathBuf::from(r"\\.\pipe\agentic-navigation-guide-test"),
            PathBuf::from(r"\\localhost\pipe\agentic-navigation-guide-test"),
            PathBuf::from(r"\\localhost\mailslot\agentic-navigation-guide-test"),
            PathBuf::from(r"\\localhost\IPC$\agentic-navigation-guide-test"),
            PathBuf::from(r"//?/GLOBALROOT/Device/HarddiskVolume1/agentic-navigation-guide-test"),
            PathBuf::from(r"\\?\C:\agentic-navigation-guide-test.md"),
            PathBuf::from(r"\??\C:\agentic-navigation-guide-test.md"),
        ] {
            let result = run_file_output(command, root.path(), &output, &[]);
            assert!(
                !result.status.success(),
                "{command:?} accepted unsafe Windows output {output:?}"
            );
        }
        assert!(!base.exists());
    }
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
        .stderr(predicate::str::contains("- main.rs").not());
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
fn test_github_actions_check_omits_line_content() {
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
        .stderr(predicate::str::contains("- missing_file.txt").not());
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
        .arg("--allow-empty")
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid glob pattern"))
        .stderr(predicate::str::contains("zero navigation guides were verified").not());
}

#[test]
fn test_recursive_verify_with_ignored_guides() {
    const MIXED_SUMMARY: &str =
        "Total: 2, Discovered: 2, Passed: 1, Failed: 0, Ignored: 1, Absent: 0";
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
        .arg("--allow-empty")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains(MIXED_SUMMARY))
        .stdout(predicate::str::contains("All navigation guides").not())
        .stderr(predicate::str::contains("ignore=true"));

    // The explicit deny policy preserves the same categorization and counts
    // while changing only the command outcome.
    let mut denied = get_command();
    denied
        .arg("verify")
        .arg("--recursive")
        .arg("--deny-ignored")
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stdout(predicate::str::contains(MIXED_SUMMARY))
        .stdout(predicate::str::contains("All navigation guides").not())
        .stderr(predicate::str::contains("--deny-ignored"))
        .stderr(predicate::str::contains("ignored"));
}

#[test]
fn test_recursive_verify_preserves_failed_and_ignored_counts() {
    const MIXED_FAILURE_SUMMARY: &str =
        "Total: 2, Discovered: 2, Passed: 0, Failed: 1, Ignored: 1, Absent: 0";

    let temp = TempDir::new().unwrap();
    let ignored_root = temp.path().join("ignored");
    let failed_root = temp.path().join("failed");
    fs::create_dir(&ignored_root).unwrap();
    fs::create_dir(&failed_root).unwrap();
    fs::write(
        ignored_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
        "<agentic-navigation-guide ignore=true>\nopaque body\n</agentic-navigation-guide>",
    )
    .unwrap();
    fs::write(
        failed_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
        "<agentic-navigation-guide>\n- missing.txt\n</agentic-navigation-guide>",
    )
    .unwrap();

    for mode in ISSUE39_MODES {
        for deny_ignored in [false, true] {
            let mut command = isolated_command();
            command
                .arg("verify")
                .arg("--recursive")
                .arg("--root")
                .arg(temp.path());
            mode.configure(&mut command);
            if deny_ignored {
                command.arg("--deny-ignored");
            }
            let output = command.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let diagnostics = format!("{stdout}{stderr}");
            let context = format!("{mode:?}/deny_ignored={deny_ignored}");

            assert_eq!(
                output.status.code(),
                Some(mode.failure_code()),
                "{context} returned the wrong failure code:\n{diagnostics}"
            );
            assert!(
                diagnostics.contains("missing.txt"),
                "{context} lost the genuine verification failure:\n{diagnostics}"
            );

            let expected_summary_count = usize::from(deny_ignored || !mode.is_quiet());
            assert_eq!(
                diagnostics.matches(MIXED_FAILURE_SUMMARY).count(),
                expected_summary_count,
                "{context} reported the mixed aggregate incorrectly:\n{diagnostics}"
            );

            if deny_ignored {
                let combined_reason =
                    "Some guides failed verification, and --deny-ignored rejected the run because \
                     1 ignored navigation guide was discovered";
                assert!(
                    stderr.contains(combined_reason),
                    "{context} did not report both failure reasons:\n{diagnostics}"
                );
                assert_eq!(
                    stderr.lines().filter(|line| !line.is_empty()).next_back(),
                    Some(combined_reason),
                    "{context} did not leave the combined reason as the terminal diagnostic:\n\
                     {diagnostics}"
                );
            } else {
                assert!(
                    !stderr.contains("--deny-ignored rejected"),
                    "{context} enforced an unrequested ignore policy:\n{diagnostics}"
                );
            }

            if mode.is_quiet() {
                assert!(
                    !diagnostics.contains("ignore=true"),
                    "{context} leaked ordinary ignored-guide chatter in quiet mode:\n{diagnostics}"
                );
            } else {
                assert!(
                    diagnostics.contains("ignore=true"),
                    "{context} did not classify the ignored guide:\n{diagnostics}"
                );
            }
        }
    }
}

#[test]
fn test_issue39_ignored_body_and_policy_matrix() {
    const IGNORED_SUMMARY: &str =
        "Total: 1, Discovered: 1, Passed: 0, Failed: 0, Ignored: 1, Absent: 0";

    for body in ISSUE39_BODIES {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let guide_path = root.join("AGENTIC_NAVIGATION_GUIDE.md");
        fs::write(root.join("present.txt"), "").unwrap();
        fs::write(&guide_path, body.source()).unwrap();

        for surface in ISSUE39_SURFACES {
            for mode in ISSUE39_MODES {
                for deny_ignored in [false, true] {
                    let output =
                        run_issue39_ignored_case(surface, mode, &guide_path, root, deny_ignored);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let diagnostics = format!("{stdout}\n{stderr}");
                    let context =
                        format!("{body:?}/{surface:?}/{mode:?}/deny_ignored={deny_ignored}");

                    if deny_ignored {
                        assert_eq!(
                            output.status.code(),
                            Some(mode.failure_code()),
                            "{context} did not enforce --deny-ignored:\n{diagnostics}"
                        );
                        let lowercase = diagnostics.to_ascii_lowercase();
                        assert!(
                            lowercase.contains("ignored")
                                && lowercase.contains("--deny-ignored"),
                            "{context} did not preserve the ignored categorization in its denial:\n{diagnostics}"
                        );
                    } else {
                        assert_eq!(
                            output.status.code(),
                            Some(0),
                            "{context} did not allow the ignored outcome by default:\n{diagnostics}"
                        );
                        if mode.is_quiet() {
                            assert!(
                                stdout.is_empty() && stderr.is_empty(),
                                "{context} emitted ordinary chatter in quiet mode:\n{diagnostics}"
                            );
                        } else {
                            assert!(
                                diagnostics.to_ascii_lowercase().contains("ignore"),
                                "{context} did not make the ignored outcome visible:\n{diagnostics}"
                            );
                        }
                    }

                    assert_no_issue39_false_success(&diagnostics, &context);
                    assert!(
                        !diagnostics.contains(ISSUE39_OPAQUE_BODY_SENTINEL),
                        "{context} parsed or disclosed the opaque body:\n{diagnostics}"
                    );

                    if surface.is_recursive() && (!mode.is_quiet() || deny_ignored) {
                        assert!(
                            diagnostics.contains(IGNORED_SUMMARY),
                            "{context} omitted the exact ignored aggregate:\n{diagnostics}"
                        );
                    }
                    assert!(
                        !diagnostics.contains("zero navigation guides were verified"),
                        "{context} mistook an ignored guide for zero discovery:\n{diagnostics}"
                    );
                }
            }
        }
    }
}

#[test]
fn test_issue39_malformed_marker_never_activates_ignore() {
    const MALFORMED_SOURCE: &str = "<agentic-navigation-guide ignore=false>\n\
                                    ISSUE39_OPAQUE_SECRET_0c6248a7\n\
                                    </agentic-navigation-guide>";

    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let guide_path = root.join("AGENTIC_NAVIGATION_GUIDE.md");
    fs::write(&guide_path, MALFORMED_SOURCE).unwrap();

    for surface in ISSUE39_SURFACES {
        for mode in ISSUE39_MODES {
            for deny_ignored in [false, true] {
                let output =
                    run_issue39_ignored_case(surface, mode, &guide_path, root, deny_ignored);
                let diagnostics = combined_output(&output);
                let lowercase = diagnostics.to_ascii_lowercase();
                let context = format!("{surface:?}/{mode:?}/deny_ignored={deny_ignored}");

                assert_eq!(
                    output.status.code(),
                    Some(mode.failure_code()),
                    "{context} did not reject the malformed marker:\n{diagnostics}"
                );
                assert!(
                    lowercase.contains("missing opening"),
                    "{context} did not report the malformed opening marker:\n{diagnostics}"
                );
                assert!(
                    !lowercase.contains("skipping")
                        && !lowercase.contains("denied by --deny-ignored"),
                    "{context} allowed the malformed marker to activate ignore:\n{diagnostics}"
                );
                assert!(
                    !diagnostics.contains(ISSUE39_OPAQUE_BODY_SENTINEL),
                    "{context} disclosed the body of a rejected document:\n{diagnostics}"
                );
            }
        }
    }
}

#[test]
fn test_recursive_verify_rejects_non_ignore_attribute() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a guide with an unknown attribute that contains "ignore".
    fs::create_dir_all(root.join("docs/examples")).unwrap();

    let non_ignored_guide = r#"<agentic-navigation-guide notignore=true>
- missing.txt
</agentic-navigation-guide>"#;

    fs::write(
        root.join("docs/examples/AGENTIC_NAVIGATION_GUIDE.md"),
        non_ignored_guide,
    )
    .unwrap();

    // Recursive verify must reject the malformed marker rather than skip it.
    let mut cmd = get_command();
    cmd.arg("verify")
        .arg("--recursive")
        .arg("--allow-empty")
        .arg("--root")
        .arg(root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("line 1"))
        .stderr(predicate::str::contains("invalid guide document"))
        .stderr(predicate::str::contains("missing opening"))
        .stderr(predicate::str::contains("Skipping").not())
        .stderr(predicate::str::contains("zero navigation guides were verified").not());
}

#[test]
fn test_recursive_verify_zero_discovery_is_fail_closed_unless_explicitly_allowed() {
    const ZERO_SUMMARY: &str = "Discovered: 0, Passed: 0, Failed: 0, Ignored: 0, Absent: 1";

    for case in ZERO_DISCOVERY_CASES {
        let fixture = case.fixture();

        for mode in RECURSIVE_ZERO_MODES {
            let rejected = run_recursive_zero_case(&fixture, mode, false);
            let rejected_stderr = String::from_utf8_lossy(&rejected.stderr);
            let search_root_name = fixture
                .search_root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap();

            assert_eq!(
                rejected.status.code(),
                Some(mode.failure_code()),
                "{case:?} in {mode:?} mode must fail closed; stderr:\n{rejected_stderr}"
            );
            assert!(
                rejected_stderr.contains("zero navigation guides were verified"),
                "{case:?} in {mode:?} mode omitted the required zero-guide diagnostic:\n{rejected_stderr}"
            );
            assert!(
                rejected_stderr.contains(ZERO_SUMMARY),
                "{case:?} in {mode:?} mode omitted the absent-guide aggregate:\n{rejected_stderr}"
            );
            assert!(
                rejected_stderr.contains(fixture.guide_name)
                    && rejected_stderr.contains(search_root_name)
                    && rejected_stderr.contains("--root")
                    && rejected_stderr.contains("--guide-name")
                    && rejected_stderr.contains("--exclude")
                    && rejected_stderr.contains("--allow-empty"),
                "{case:?} in {mode:?} mode did not explain how to correct or explicitly allow the empty search:\n{rejected_stderr}"
            );

            let allowed = run_recursive_zero_case(&fixture, mode, true);
            let allowed_stdout = String::from_utf8_lossy(&allowed.stdout);
            let allowed_stderr = String::from_utf8_lossy(&allowed.stderr);

            assert_eq!(
                allowed.status.code(),
                Some(0),
                "{case:?} in {mode:?} mode was not allowed by --allow-empty; stderr:\n{allowed_stderr}"
            );
            if mode.is_quiet() {
                assert!(
                    allowed_stdout.is_empty() && allowed_stderr.is_empty(),
                    "{case:?} quiet allow-empty success emitted ordinary chatter"
                );
            } else {
                assert!(
                    allowed_stdout.contains("zero navigation guides were verified")
                        && allowed_stdout.contains(ZERO_SUMMARY),
                    "{case:?} in {mode:?} mode did not report the explicitly allowed zero count:\n{allowed_stdout}"
                );
                assert!(
                    allowed_stderr.is_empty(),
                    "{case:?} in {mode:?} mode reported an allowed empty search as an error:\n{allowed_stderr}"
                );
            }
        }
    }
}

#[test]
fn test_recursive_verify_ignored_guide_is_discovered_not_absent() {
    const IGNORED_SUMMARY: &str = "Discovered: 1, Passed: 0, Failed: 0, Ignored: 1, Absent: 0";
    const IGNORED_GUIDE: &str = "<agentic-navigation-guide ignore=true>\n\
                                - deliberately-missing.txt\n\
                                </agentic-navigation-guide>";

    let temp = TempDir::new().unwrap();
    fs::write(
        temp.path().join("AGENTIC_NAVIGATION_GUIDE.md"),
        IGNORED_GUIDE,
    )
    .unwrap();

    let search_root = temp.path().to_path_buf();
    let fixture = ZeroDiscoveryFixture {
        _temp: temp,
        search_root,
        guide_name: "AGENTIC_NAVIGATION_GUIDE.md",
        exclusions: Vec::new(),
    };

    for allow_empty in [false, true] {
        for mode in RECURSIVE_ZERO_MODES {
            let output = run_recursive_zero_case(&fixture, mode, allow_empty);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let diagnostics = format!("{stdout}\n{stderr}");

            assert_eq!(
                output.status.code(),
                Some(0),
                "ignored guide in {mode:?} mode must remain a discovered success; stderr:\n{stderr}"
            );
            assert!(
                !diagnostics.contains("zero navigation guides were verified"),
                "ignored guide in {mode:?} mode was mistaken for absent"
            );
            if mode.is_quiet() {
                assert!(
                    stdout.is_empty() && stderr.is_empty(),
                    "quiet ignored-guide success emitted ordinary chatter"
                );
            } else {
                assert!(
                    diagnostics.contains(IGNORED_SUMMARY),
                    "ignored guide in {mode:?} mode omitted separate aggregate counts:\n{diagnostics}"
                );
            }
        }
    }
}

#[test]
fn test_allow_empty_requires_recursive() {
    let temp = TempDir::new().unwrap();
    let mut command = get_command();
    command
        .arg("verify")
        .arg("--allow-empty")
        .arg("--root")
        .arg(temp.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--recursive"));
}

#[test]
fn test_check_and_verify_help_document_deny_ignored() {
    for subcommand in ["check", "verify"] {
        let mut command = isolated_command();
        command
            .arg(subcommand)
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("--deny-ignored"))
            .stdout(predicate::str::contains("ignore=true"));
    }
}

#[test]
fn test_recursive_verify_help_documents_allow_empty() {
    let mut command = get_command();
    command
        .arg("verify")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--allow-empty"))
        .stdout(predicate::str::contains(
            "Allow a recursive search to succeed after discovering zero guides",
        ));
}

#[test]
fn test_allow_empty_does_not_convert_traversal_failure_into_empty_success() {
    for mode in RECURSIVE_ZERO_MODES {
        let temp = TempDir::new().unwrap();
        let missing_root = temp.path().join("missing");
        let fixture = ZeroDiscoveryFixture {
            _temp: temp,
            search_root: missing_root,
            guide_name: "AGENTIC_NAVIGATION_GUIDE.md",
            exclusions: Vec::new(),
        };
        let output = run_recursive_zero_case(&fixture, mode, true);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(mode.failure_code()),
            "traversal failure in {mode:?} mode was converted to allow-empty success"
        );
        assert!(
            stderr.contains("filesystem walk error")
                && !stderr.contains("zero navigation guides were verified"),
            "traversal failure in {mode:?} mode was misclassified:\n{stderr}"
        );
    }
}

#[test]
fn test_allow_empty_does_not_convert_non_directory_root_into_empty_success() {
    for allow_empty in [false, true] {
        for mode in RECURSIVE_ZERO_MODES {
            let temp = TempDir::new().unwrap();
            let file_root = temp.path().join("not-a-directory");
            fs::write(&file_root, "").unwrap();
            let fixture = ZeroDiscoveryFixture {
                _temp: temp,
                search_root: file_root,
                guide_name: "AGENTIC_NAVIGATION_GUIDE.md",
                exclusions: Vec::new(),
            };
            let output = run_recursive_zero_case(&fixture, mode, allow_empty);
            let stderr = String::from_utf8_lossy(&output.stderr);

            assert_eq!(
                output.status.code(),
                Some(mode.failure_code()),
                "non-directory root in {mode:?} mode was converted to empty success"
            );
            assert!(
                stderr.contains("is not a directory")
                    && !stderr.contains("zero navigation guides were verified"),
                "non-directory root in {mode:?} mode was misclassified:\n{stderr}"
            );
        }
    }
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

#[cfg(unix)]
#[test]
fn test_rejected_recursive_guide_never_reads_or_discloses_its_target_in_any_mode() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let root = temp.path().join("checkout");
    let outside = temp.path().join("outside-secret.md");
    fs::create_dir(&root).unwrap();
    fs::write(
        &outside,
        format!("{GUIDE_SOURCE_SENTINEL}\nnot a navigation guide"),
    )
    .unwrap();
    symlink(
        "../outside-secret.md",
        root.join("AGENTIC_NAVIGATION_GUIDE.md"),
    )
    .unwrap();

    for mode in GUIDE_DIAGNOSTIC_MODES {
        let mut command = isolated_command();
        command
            .arg("verify")
            .arg("--recursive")
            .arg("--root")
            .arg(&root);
        mode.configure(&mut command);
        let output = command.output().unwrap();

        assert_unsafe_guide_rejection(&output, mode, "AGENTIC_NAVIGATION_GUIDE.md", &outside);
    }
}

#[test]
fn test_regular_guide_source_lines_are_not_echoed_in_any_cli_mode() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let guide = root.join("AGENTIC_NAVIGATION_GUIDE.md");
    fs::write(
        &guide,
        format!("{GUIDE_SOURCE_SENTINEL}\nnot a navigation guide"),
    )
    .unwrap();

    for mode in GUIDE_DIAGNOSTIC_MODES {
        for surface in ["check", "verify", "recursive"] {
            let mut command = isolated_command();
            match surface {
                "check" => {
                    command.arg("check").arg("--guide").arg(&guide);
                }
                "verify" => {
                    command
                        .arg("verify")
                        .arg("--guide")
                        .arg(&guide)
                        .arg("--root")
                        .arg(root);
                }
                "recursive" => {
                    command
                        .arg("verify")
                        .arg("--recursive")
                        .arg("--root")
                        .arg(root);
                }
                _ => unreachable!(),
            }
            mode.configure(&mut command);
            let output = command.output().unwrap();
            let diagnostics = combined_output(&output);

            assert_eq!(
                output.status.code(),
                Some(mode.failure_code()),
                "{surface} in {mode:?} mode did not reject malformed input:\n{diagnostics}"
            );
            assert!(
                diagnostics.contains("line 1") && diagnostics.contains("missing opening"),
                "{surface} in {mode:?} mode omitted bounded source-location context:\n{diagnostics}"
            );
            assert!(
                !diagnostics.contains(GUIDE_SOURCE_SENTINEL),
                "{surface} in {mode:?} mode echoed a raw guide source line:\n{diagnostics}"
            );
        }
    }
}

#[test]
fn test_guide_confidentiality_covers_every_execution_and_log_mode() {
    let temp = TempDir::new().unwrap();
    let linked_root = temp.path().join("linked-checkout");
    let regular_root = temp.path().join("regular-checkout");
    let outside = temp.path().join("outside-secret.md");
    fs::create_dir(&linked_root).unwrap();
    fs::create_dir(&regular_root).unwrap();
    fs::write(
        &outside,
        format!("{GUIDE_SOURCE_SENTINEL}\nnot a navigation guide"),
    )
    .unwrap();
    create_guide_file_link(&outside, &linked_root.join("AGENTIC_NAVIGATION_GUIDE.md"));
    fs::write(
        regular_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
        format!("{GUIDE_SOURCE_SENTINEL}\nnot a navigation guide"),
    )
    .unwrap();

    for execution in GUIDE_EXECUTION_CASES {
        for log in GUIDE_LOG_CASES {
            for surface in [
                "check-implicit",
                "verify-implicit",
                "recursive",
                "check-explicit",
                "verify-explicit",
            ] {
                let link = linked_root.join("AGENTIC_NAVIGATION_GUIDE.md");
                let mut command = isolated_command();
                command.current_dir(&linked_root);
                execution.configure(&mut command);
                log.configure(&mut command);
                match surface {
                    "check-implicit" => {
                        command.arg("check");
                    }
                    "verify-implicit" => {
                        command.arg("verify").arg("--root").arg(&linked_root);
                    }
                    "recursive" => {
                        command
                            .arg("verify")
                            .arg("--recursive")
                            .arg("--allow-empty")
                            .arg("--root")
                            .arg(&linked_root);
                    }
                    "check-explicit" => {
                        command.arg("check").arg("--guide").arg(&link);
                    }
                    "verify-explicit" => {
                        command
                            .arg("verify")
                            .arg("--guide")
                            .arg(&link)
                            .arg("--root")
                            .arg(&linked_root);
                    }
                    _ => unreachable!(),
                }
                let output = command.output().unwrap();
                let diagnostics = combined_output(&output);

                assert!(
                    !output.status.success(),
                    "{surface} in {execution:?}/{log:?} followed a final guide link:\n{diagnostics}"
                );
                assert!(
                    diagnostics.contains("unsafe guide path")
                        && diagnostics.contains("AGENTIC_NAVIGATION_GUIDE.md"),
                    "{surface} in {execution:?}/{log:?} omitted its typed logical-path error:\n{diagnostics}"
                );
                assert!(
                    !diagnostics.contains(GUIDE_SOURCE_SENTINEL)
                        && !diagnostics.contains("outside-secret.md"),
                    "{surface} in {execution:?}/{log:?} disclosed linked target data:\n{diagnostics}"
                );
                assert!(
                    !diagnostics.contains("zero navigation guides were verified"),
                    "{surface} in {execution:?}/{log:?} converted an unsafe guide to absence:\n{diagnostics}"
                );
            }

            for surface in [
                "check-implicit",
                "verify-implicit",
                "recursive",
                "check-explicit",
                "verify-explicit",
            ] {
                let guide = regular_root.join("AGENTIC_NAVIGATION_GUIDE.md");
                let mut command = isolated_command();
                command.current_dir(&regular_root);
                execution.configure(&mut command);
                log.configure(&mut command);
                match surface {
                    "check-implicit" => {
                        command.arg("check");
                    }
                    "verify-implicit" => {
                        command.arg("verify").arg("--root").arg(&regular_root);
                    }
                    "recursive" => {
                        command
                            .arg("verify")
                            .arg("--recursive")
                            .arg("--root")
                            .arg(&regular_root);
                    }
                    "check-explicit" => {
                        command.arg("check").arg("--guide").arg(&guide);
                    }
                    "verify-explicit" => {
                        command
                            .arg("verify")
                            .arg("--guide")
                            .arg(&guide)
                            .arg("--root")
                            .arg(&regular_root);
                    }
                    _ => unreachable!(),
                }
                let output = command.output().unwrap();
                let diagnostics = combined_output(&output);

                assert!(
                    !output.status.success(),
                    "{surface} in {execution:?}/{log:?} accepted malformed input:\n{diagnostics}"
                );
                assert!(
                    diagnostics.contains("line 1") && diagnostics.contains("missing opening"),
                    "{surface} in {execution:?}/{log:?} omitted bounded source context:\n{diagnostics}"
                );
                assert!(
                    !diagnostics.contains(GUIDE_SOURCE_SENTINEL),
                    "{surface} in {execution:?}/{log:?} echoed a raw source line:\n{diagnostics}"
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn test_guide_input_diagnostics_are_control_safe_and_bounded() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let outside = temp.path().join("outside-secret-target.md");
    fs::write(
        &outside,
        "<agentic-navigation-guide>\n- ordinary.txt\n</agentic-navigation-guide>",
    )
    .unwrap();

    let unsafe_name = OsString::from("unsafe\n\u{1b}-guide.md");
    let unsafe_link = temp.path().join(&unsafe_name);
    symlink(&outside, &unsafe_link).unwrap();
    let mut command = isolated_command();
    let output = command
        .current_dir(temp.path())
        .arg("check")
        .arg("--guide")
        .arg(&unsafe_link)
        .output()
        .unwrap();
    let mut raw_output = output.stdout.clone();
    raw_output.extend_from_slice(&output.stderr);
    let diagnostics = String::from_utf8_lossy(&raw_output);

    assert!(!output.status.success(), "{diagnostics}");
    assert!(
        !raw_output
            .windows(b"unsafe\n\x1b-guide.md".len())
            .any(|window| window == b"unsafe\n\x1b-guide.md"),
        "diagnostic emitted raw path controls"
    );
    assert!(
        diagnostics.contains("\\n") && !diagnostics.contains("outside-secret-target.md"),
        "diagnostic did not escape the control-bearing logical path:\n{diagnostics}"
    );

    let invalid_path = temp
        .path()
        .join(OsString::from_vec(b"invalid-\xff-guide.md".to_vec()));
    let mut command = isolated_command();
    let output = command
        .current_dir(temp.path())
        .arg("check")
        .arg("--guide")
        .arg(&invalid_path)
        .output()
        .unwrap();
    let mut raw_output = output.stdout.clone();
    raw_output.extend_from_slice(&output.stderr);
    let diagnostics = String::from_utf8_lossy(&raw_output);
    assert!(!output.status.success(), "{diagnostics}");
    assert!(
        !raw_output
            .windows(b"invalid-\xff-guide.md".len())
            .any(|window| window == b"invalid-\xff-guide.md")
            && diagnostics.contains("\\xFF")
            && !diagnostics.contains('\u{fffd}'),
        "diagnostic did not reversibly escape an undecodable path:\n{diagnostics}"
    );

    let mut deep_parent = temp.path().to_path_buf();
    for index in 0..40 {
        deep_parent.push(format!("long-segment-{index:02}"));
    }
    fs::create_dir_all(&deep_parent).unwrap();
    let long_link = deep_parent.join("AGENTIC_NAVIGATION_GUIDE.md");
    symlink(&outside, &long_link).unwrap();
    let mut command = isolated_command();
    let output = command
        .current_dir(temp.path())
        .arg("check")
        .arg("--guide")
        .arg(&long_link)
        .output()
        .unwrap();
    let diagnostics = combined_output(&output);
    let unsafe_line = diagnostics
        .lines()
        .find(|line| line.contains("unsafe guide path"))
        .expect("bounded unsafe-guide diagnostic");

    assert!(!output.status.success(), "{diagnostics}");
    assert!(
        unsafe_line.contains('…') && unsafe_line.chars().count() < 480,
        "logical path was not bounded:\n{unsafe_line}"
    );
    assert!(!diagnostics.contains("outside-secret-target.md"));
}

#[cfg(unix)]
#[test]
fn test_guide_input_trust_policy_matrix() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;

    const GUIDE: &str = "<agentic-navigation-guide>\n- payload.txt\n</agentic-navigation-guide>";

    let temp = TempDir::new().unwrap();
    let caller = temp.path().join("caller");
    let outside = temp.path().join("outside");
    fs::create_dir(&caller).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(caller.join("payload.txt"), "").unwrap();
    fs::write(outside.join("payload.txt"), "").unwrap();
    fs::write(outside.join("regular-guide.md"), GUIDE).unwrap();
    fs::write(
        outside.join("AGENTIC_NAVIGATION_GUIDE.md"),
        format!("{GUIDE_SOURCE_SENTINEL}\nnot a navigation guide"),
    )
    .unwrap();

    let run_recursive = |root: &Path, extra: &[&str]| {
        let mut command = isolated_command();
        command
            .arg("verify")
            .arg("--recursive")
            .arg("--root")
            .arg(root)
            .args(extra);
        command.output().unwrap()
    };

    for (case_name, target) in [
        ("in-root-link", PathBuf::from("real-guide.md")),
        (
            "relative-external-link",
            PathBuf::from("../../outside/regular-guide.md"),
        ),
        ("absolute-external-link", outside.join("regular-guide.md")),
    ] {
        let case_root = caller.join(case_name);
        fs::create_dir(&case_root).unwrap();
        fs::write(case_root.join("payload.txt"), "").unwrap();
        if case_name == "in-root-link" {
            fs::write(case_root.join("real-guide.md"), GUIDE).unwrap();
        }
        let forbidden_target = target.clone();
        let link = case_root.join("AGENTIC_NAVIGATION_GUIDE.md");
        symlink(&target, &link).unwrap();
        for surface in [
            "check-implicit",
            "verify-implicit",
            "recursive",
            "check-explicit",
            "verify-explicit",
        ] {
            let mut command = isolated_command();
            command.current_dir(&case_root);
            match surface {
                "check-implicit" => {
                    command.arg("check");
                }
                "verify-implicit" => {
                    command.arg("verify").arg("--root").arg(&case_root);
                }
                "recursive" => {
                    command
                        .arg("verify")
                        .arg("--recursive")
                        .arg("--root")
                        .arg(&case_root);
                }
                "check-explicit" => {
                    command.arg("check").arg("--guide").arg(&link);
                }
                "verify-explicit" => {
                    command
                        .arg("verify")
                        .arg("--guide")
                        .arg(&link)
                        .arg("--root")
                        .arg(&case_root);
                }
                _ => unreachable!(),
            }
            let output = command.output().unwrap();
            assert_unsafe_guide_rejection(
                &output,
                GuideDiagnosticMode::Default,
                "AGENTIC_NAVIGATION_GUIDE.md",
                &forbidden_target,
            );
        }
    }

    let dangling_root = caller.join("dangling");
    fs::create_dir(&dangling_root).unwrap();
    symlink(
        "missing-guide.md",
        dangling_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
    )
    .unwrap();

    let absolute_dangling_root = caller.join("absolute-dangling");
    fs::create_dir(&absolute_dangling_root).unwrap();
    let absolute_missing = outside.join("absolute-missing-guide.md");
    symlink(
        &absolute_missing,
        absolute_dangling_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
    )
    .unwrap();

    let chain_root = caller.join("chain");
    fs::create_dir(&chain_root).unwrap();
    fs::write(chain_root.join("payload.txt"), "").unwrap();
    symlink("../../outside/regular-guide.md", chain_root.join("second")).unwrap();
    symlink("second", chain_root.join("AGENTIC_NAVIGATION_GUIDE.md")).unwrap();

    let loop_root = caller.join("loop");
    fs::create_dir(&loop_root).unwrap();
    symlink("second", loop_root.join("AGENTIC_NAVIGATION_GUIDE.md")).unwrap();
    symlink("AGENTIC_NAVIGATION_GUIDE.md", loop_root.join("second")).unwrap();

    let directory_root = caller.join("directory");
    fs::create_dir_all(directory_root.join("AGENTIC_NAVIGATION_GUIDE.md")).unwrap();

    let fifo_root = caller.join("fifo");
    fs::create_dir(&fifo_root).unwrap();
    let fifo = fifo_root.join("AGENTIC_NAVIGATION_GUIDE.md");
    let fifo_name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    // SAFETY: `fifo_name` is a valid NUL-terminated filesystem path.
    assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);

    let socket_root = caller.join("socket");
    fs::create_dir(&socket_root).unwrap();
    let socket =
        std::os::unix::net::UnixListener::bind(socket_root.join("AGENTIC_NAVIGATION_GUIDE.md"))
            .unwrap();

    for (root, forbidden_target) in [
        (&dangling_root, Path::new("missing-guide.md")),
        (&absolute_dangling_root, absolute_missing.as_path()),
        (&chain_root, Path::new("second")),
        (&loop_root, Path::new("second")),
        (&directory_root, Path::new("__no_directory_target__")),
        (&fifo_root, Path::new("__no_fifo_target__")),
        (&socket_root, Path::new("__no_socket_target__")),
    ] {
        let output = run_recursive(root, &[]);
        assert_unsafe_guide_rejection(
            &output,
            GuideDiagnosticMode::Default,
            "AGENTIC_NAVIGATION_GUIDE.md",
            forbidden_target,
        );
    }
    drop(socket);

    for command_name in ["check", "verify"] {
        let mut command = isolated_command();
        command.current_dir(&caller).arg(command_name);
        if command_name == "check" {
            command.arg("--guide").arg("/dev/null");
        } else {
            command
                .arg("--guide")
                .arg("/dev/null")
                .arg("--root")
                .arg(&caller);
        }
        let output = command.output().unwrap();
        let diagnostics = combined_output(&output);
        assert!(
            !output.status.success() && diagnostics.contains("unsafe guide path"),
            "{command_name} opened a device as a guide:\n{diagnostics}"
        );
    }

    let excluded_root = caller.join("excluded-root");
    let excluded = excluded_root.join("excluded");
    fs::create_dir_all(&excluded).unwrap();
    symlink(
        outside.join("regular-guide.md"),
        excluded.join("AGENTIC_NAVIGATION_GUIDE.md"),
    )
    .unwrap();
    let excluded_output =
        run_recursive(&excluded_root, &["--exclude", "excluded", "--allow-empty"]);
    assert!(
        excluded_output.status.success(),
        "an explicitly excluded unsafe match was not pruned:\n{}",
        combined_output(&excluded_output)
    );
    assert!(!combined_output(&excluded_output).contains(GUIDE_SOURCE_SENTINEL));

    let pruned_root = caller.join("pruned-root");
    fs::create_dir(&pruned_root).unwrap();
    symlink(&outside, pruned_root.join("linked-directory")).unwrap();
    let pruned_output = run_recursive(&pruned_root, &["--allow-empty"]);
    assert!(
        pruned_output.status.success(),
        "a nonmatching descendant directory link was traversed or rejected:\n{}",
        combined_output(&pruned_output)
    );
    assert!(!combined_output(&pruned_output).contains(GUIDE_SOURCE_SENTINEL));

    for command_name in ["check", "verify"] {
        let explicit_external = outside.join("regular-guide.md");
        let mut command = isolated_command();
        command.current_dir(&caller).arg(command_name);
        if command_name == "check" {
            command.arg("--guide").arg(&explicit_external);
        } else {
            command
                .arg("--guide")
                .arg(&explicit_external)
                .arg("--root")
                .arg(&caller);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{command_name} rejected explicit external regular-file authority:\n{}",
            combined_output(&output)
        );
    }

    let linked_ancestor = caller.join("linked-ancestor");
    symlink(&outside, &linked_ancestor).unwrap();
    for command_name in ["check", "verify"] {
        let selected = if command_name == "check" {
            PathBuf::from("linked-ancestor/regular-guide.md")
        } else {
            linked_ancestor.join("regular-guide.md")
        };
        let mut command = isolated_command();
        command.current_dir(&caller).arg(command_name);
        if command_name == "check" {
            command.arg("--guide").arg(&selected);
        } else {
            command
                .arg("--guide")
                .arg(&selected)
                .arg("--root")
                .arg(&caller);
        }
        let output = command.output().unwrap();
        assert_unsafe_guide_rejection(
            &output,
            GuideDiagnosticMode::Default,
            "linked-ancestor",
            &outside,
        );
    }

    let real_in_root_ancestor = caller.join("real-in-root-ancestor");
    fs::create_dir(&real_in_root_ancestor).unwrap();
    fs::write(real_in_root_ancestor.join("regular-guide.md"), GUIDE).unwrap();
    let in_root_ancestor = caller.join("in-root-ancestor");
    symlink(&real_in_root_ancestor, &in_root_ancestor).unwrap();
    for command_name in ["check", "verify"] {
        let selected = if command_name == "check" {
            PathBuf::from("in-root-ancestor/regular-guide.md")
        } else {
            in_root_ancestor.join("regular-guide.md")
        };
        let mut command = isolated_command();
        command.current_dir(&caller).arg(command_name);
        if command_name == "check" {
            command.arg("--guide").arg(&selected);
        } else {
            command
                .arg("--guide")
                .arg(&selected)
                .arg("--root")
                .arg(&caller);
        }
        let output = command.output().unwrap();
        assert_unsafe_guide_rejection(
            &output,
            GuideDiagnosticMode::Default,
            "in-root-ancestor",
            &real_in_root_ancestor,
        );
    }

    let external_alias = temp.path().join("external-alias");
    symlink(&outside, &external_alias).unwrap();
    for command_name in ["check", "verify"] {
        let selected = external_alias.join("regular-guide.md");
        let mut command = isolated_command();
        command.current_dir(&caller).arg(command_name);
        if command_name == "check" {
            command.arg("--guide").arg(&selected);
        } else {
            command
                .arg("--guide")
                .arg(&selected)
                .arg("--root")
                .arg(&caller);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{command_name} rejected a stable ancestor in an explicitly external path:\n{}",
            combined_output(&output)
        );
    }

    let real_root = temp.path().join("real-root");
    fs::create_dir(&real_root).unwrap();
    fs::write(real_root.join("payload.txt"), "").unwrap();
    fs::write(real_root.join("AGENTIC_NAVIGATION_GUIDE.md"), GUIDE).unwrap();
    let root_alias = temp.path().join("root-alias");
    symlink(&real_root, &root_alias).unwrap();

    let mut default_verify = isolated_command();
    let default_output = default_verify
        .current_dir(&caller)
        .arg("verify")
        .arg("--root")
        .arg(&root_alias)
        .output()
        .unwrap();
    assert!(
        default_output.status.success(),
        "default verify did not resolve its guide from the effective root alias:\n{}",
        combined_output(&default_output)
    );

    let recursive_alias = run_recursive(&root_alias, &[]);
    assert!(
        recursive_alias.status.success(),
        "recursive verification rejected a caller-selected root alias:\n{}",
        combined_output(&recursive_alias)
    );

    let mut default_check = isolated_command();
    let default_check_output = default_check
        .current_dir(&real_root)
        .arg("check")
        .output()
        .unwrap();
    assert!(
        default_check_output.status.success(),
        "default check rejected a regular in-root guide:\n{}",
        combined_output(&default_check_output)
    );

    let hard_link_root = caller.join("hard-link");
    fs::create_dir(&hard_link_root).unwrap();
    fs::write(hard_link_root.join("payload.txt"), "").unwrap();
    let hard_link_source = hard_link_root.join("source.md");
    fs::write(&hard_link_source, GUIDE).unwrap();
    fs::hard_link(
        &hard_link_source,
        hard_link_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
    )
    .unwrap();
    for surface in ["check", "verify", "recursive"] {
        let mut command = isolated_command();
        command.current_dir(&hard_link_root);
        match surface {
            "check" => {
                command.arg("check");
            }
            "verify" => {
                command.arg("verify").arg("--root").arg(&hard_link_root);
            }
            "recursive" => {
                command
                    .arg("verify")
                    .arg("--recursive")
                    .arg("--root")
                    .arg(&hard_link_root);
            }
            _ => unreachable!(),
        }
        let hard_link_output = command.output().unwrap();
        assert!(
            hard_link_output.status.success(),
            "{surface} did not treat a hard-linked guide as a regular file:\n{}",
            combined_output(&hard_link_output)
        );
    }
}

#[cfg(windows)]
#[test]
fn test_windows_guide_input_trust_policy_matrix() {
    const GUIDE: &str = "<agentic-navigation-guide>\n- payload.txt\n</agentic-navigation-guide>";

    let temp = TempDir::new().unwrap();
    let caller = temp.path().join("caller");
    let outside = temp.path().join("outside");
    fs::create_dir(&caller).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(caller.join("payload.txt"), "").unwrap();
    fs::write(outside.join("payload.txt"), "").unwrap();
    fs::write(outside.join("regular-guide.md"), GUIDE).unwrap();
    fs::write(
        outside.join("AGENTIC_NAVIGATION_GUIDE.md"),
        format!("{GUIDE_SOURCE_SENTINEL}\nnot a navigation guide"),
    )
    .unwrap();

    let run_recursive = |root: &Path, extra: &[&str]| {
        let mut command = isolated_command();
        command
            .arg("verify")
            .arg("--recursive")
            .arg("--root")
            .arg(root)
            .args(extra);
        command.output().unwrap()
    };

    for case_name in [
        "in-root-link",
        "relative-external-link",
        "absolute-external-link",
    ] {
        let case_root = caller.join(case_name);
        fs::create_dir(&case_root).unwrap();
        fs::write(case_root.join("payload.txt"), "").unwrap();
        let target = match case_name {
            "in-root-link" => {
                let target = case_root.join("real-guide.md");
                fs::write(&target, GUIDE).unwrap();
                target
            }
            "relative-external-link" => PathBuf::from("../../outside/regular-guide.md"),
            "absolute-external-link" => outside.join("regular-guide.md"),
            _ => unreachable!(),
        };
        let link = case_root.join("AGENTIC_NAVIGATION_GUIDE.md");
        create_guide_file_link(&target, &link);

        for surface in [
            "check-implicit",
            "verify-implicit",
            "recursive",
            "check-explicit",
            "verify-explicit",
        ] {
            let mut command = isolated_command();
            command.current_dir(&case_root);
            match surface {
                "check-implicit" => {
                    command.arg("check");
                }
                "verify-implicit" => {
                    command.arg("verify").arg("--root").arg(&case_root);
                }
                "recursive" => {
                    command
                        .arg("verify")
                        .arg("--recursive")
                        .arg("--root")
                        .arg(&case_root);
                }
                "check-explicit" => {
                    command.arg("check").arg("--guide").arg(&link);
                }
                "verify-explicit" => {
                    command
                        .arg("verify")
                        .arg("--guide")
                        .arg(&link)
                        .arg("--root")
                        .arg(&case_root);
                }
                _ => unreachable!(),
            }
            let output = command.output().unwrap();
            assert_unsafe_guide_rejection(
                &output,
                GuideDiagnosticMode::Default,
                "AGENTIC_NAVIGATION_GUIDE.md",
                &target,
            );
        }
    }

    let dangling_root = caller.join("dangling");
    fs::create_dir(&dangling_root).unwrap();
    let missing = outside.join("missing-guide.md");
    create_guide_file_link(&missing, &dangling_root.join("AGENTIC_NAVIGATION_GUIDE.md"));

    let chain_root = caller.join("chain");
    fs::create_dir(&chain_root).unwrap();
    create_guide_file_link(
        &outside.join("regular-guide.md"),
        &chain_root.join("second"),
    );
    create_guide_file_link(
        Path::new("second"),
        &chain_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
    );

    let loop_root = caller.join("loop");
    fs::create_dir(&loop_root).unwrap();
    create_guide_file_link(
        Path::new("second"),
        &loop_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
    );
    create_guide_file_link(
        Path::new("AGENTIC_NAVIGATION_GUIDE.md"),
        &loop_root.join("second"),
    );

    let directory_root = caller.join("directory");
    fs::create_dir_all(directory_root.join("AGENTIC_NAVIGATION_GUIDE.md")).unwrap();

    let directory_reparse_root = caller.join("directory-reparse");
    fs::create_dir(&directory_reparse_root).unwrap();
    create_guide_directory_link(
        &outside,
        &directory_reparse_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
    );

    for (root, target) in [
        (&dangling_root, missing.as_path()),
        (&chain_root, Path::new("second")),
        (&loop_root, Path::new("second")),
        (&directory_root, Path::new("__no_directory_target__")),
        (&directory_reparse_root, outside.as_path()),
    ] {
        let output = run_recursive(root, &[]);
        assert_unsafe_guide_rejection(
            &output,
            GuideDiagnosticMode::Default,
            "AGENTIC_NAVIGATION_GUIDE.md",
            target,
        );
    }

    let excluded_root = caller.join("excluded-root");
    fs::create_dir(&excluded_root).unwrap();
    create_guide_file_link(
        &outside.join("regular-guide.md"),
        &excluded_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
    );
    let excluded = run_recursive(
        &excluded_root,
        &["--exclude", "AGENTIC_NAVIGATION_GUIDE.md", "--allow-empty"],
    );
    assert!(
        excluded.status.success(),
        "a Windows unsafe match was classified before exclusion:\n{}",
        combined_output(&excluded)
    );

    let pruned_root = caller.join("pruned-root");
    fs::create_dir(&pruned_root).unwrap();
    create_guide_directory_link(&outside, &pruned_root.join("linked-directory"));
    let pruned = run_recursive(&pruned_root, &["--allow-empty"]);
    assert!(
        pruned.status.success() && !combined_output(&pruned).contains(GUIDE_SOURCE_SENTINEL),
        "a Windows descendant directory reparse point was traversed:\n{}",
        combined_output(&pruned)
    );

    let in_root_ancestor = caller.join("linked-ancestor");
    create_guide_directory_link(&outside, &in_root_ancestor);
    for command_name in ["check", "verify"] {
        let selected = if command_name == "check" {
            PathBuf::from("linked-ancestor/regular-guide.md")
        } else {
            in_root_ancestor.join("regular-guide.md")
        };
        let mut command = isolated_command();
        command.current_dir(&caller).arg(command_name);
        if command_name == "check" {
            command.arg("--guide").arg(&selected);
        } else {
            command
                .arg("--guide")
                .arg(&selected)
                .arg("--root")
                .arg(&caller);
        }
        let output = command.output().unwrap();
        assert_unsafe_guide_rejection(
            &output,
            GuideDiagnosticMode::Default,
            "linked-ancestor",
            &outside,
        );
    }

    let external_alias = temp.path().join("external-alias");
    create_guide_directory_link(&outside, &external_alias);
    for command_name in ["check", "verify"] {
        let selected = external_alias.join("regular-guide.md");
        let mut command = isolated_command();
        command.current_dir(&caller).arg(command_name);
        if command_name == "check" {
            command.arg("--guide").arg(&selected);
        } else {
            command
                .arg("--guide")
                .arg(&selected)
                .arg("--root")
                .arg(&caller);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{command_name} rejected an explicitly external Windows reparse ancestor:\n{}",
            combined_output(&output)
        );
    }

    let real_root = temp.path().join("real-root");
    fs::create_dir(&real_root).unwrap();
    fs::write(real_root.join("payload.txt"), "").unwrap();
    fs::write(real_root.join("AGENTIC_NAVIGATION_GUIDE.md"), GUIDE).unwrap();
    let root_alias = temp.path().join("root-alias");
    create_guide_directory_link(&real_root, &root_alias);
    let mut default_verify = isolated_command();
    let default_output = default_verify
        .current_dir(&caller)
        .arg("verify")
        .arg("--root")
        .arg(&root_alias)
        .output()
        .unwrap();
    assert!(
        default_output.status.success(),
        "default verify rejected a Windows root alias:\n{}",
        combined_output(&default_output)
    );
    let recursive_alias = run_recursive(&root_alias, &[]);
    assert!(
        recursive_alias.status.success(),
        "recursive verify rejected a Windows root alias:\n{}",
        combined_output(&recursive_alias)
    );

    let hard_link_root = caller.join("hard-link");
    fs::create_dir(&hard_link_root).unwrap();
    fs::write(hard_link_root.join("payload.txt"), "").unwrap();
    let hard_link_source = hard_link_root.join("source.md");
    fs::write(&hard_link_source, GUIDE).unwrap();
    fs::hard_link(
        &hard_link_source,
        hard_link_root.join("AGENTIC_NAVIGATION_GUIDE.md"),
    )
    .unwrap();
    let hard_link = run_recursive(&hard_link_root, &[]);
    assert!(
        hard_link.status.success(),
        "Windows hard-linked guide was not treated as regular:\n{}",
        combined_output(&hard_link)
    );

    let case_root = caller.join("case-identity");
    fs::create_dir(&case_root).unwrap();
    fs::write(
        case_root.join("agentic_navigation_guide.md"),
        format!("{GUIDE_SOURCE_SENTINEL}\n{GUIDE}"),
    )
    .unwrap();
    let case_mismatch = run_recursive(
        &case_root,
        &[
            "--guide-name",
            "AGENTIC_NAVIGATION_GUIDE.md",
            "--allow-empty",
        ],
    );
    assert!(
        case_mismatch.status.success()
            && !combined_output(&case_mismatch).contains(GUIDE_SOURCE_SENTINEL),
        "Windows implicit lookup accepted a non-exact enumerated name:\n{}",
        combined_output(&case_mismatch)
    );

    for surface in ["check", "verify"] {
        let mut command = isolated_command();
        command.current_dir(&case_root).arg(surface);
        if surface == "verify" {
            command.arg("--root").arg(&case_root);
        }
        let output = command.output().unwrap();
        let diagnostics = combined_output(&output);
        assert!(
            !output.status.success()
                && diagnostics.contains("exactly match")
                && !diagnostics.contains(GUIDE_SOURCE_SENTINEL),
            "{surface} did not reject a case-aliased implicit guide before reading:\n{diagnostics}"
        );
    }
}

#[cfg(windows)]
#[test]
fn test_windows_guide_namespaces_and_streams_reject_before_access() {
    use agentic_navigation_guide::{verify_guides, GuideLocation};

    let temp = TempDir::new().unwrap();
    let root = temp.path().join("root");
    let missing_root = temp.path().join("missing-root");
    fs::create_dir(&root).unwrap();

    for name in [
        "guide.md:stream",
        "NUL.txt",
        "CONIN$.txt",
        "CONOUT$",
        "COM¹.log",
        "LPT9.md",
    ] {
        let mut command = isolated_command();
        let output = command
            .arg("verify")
            .arg("--recursive")
            .arg("--root")
            .arg(&missing_root)
            .arg("--guide-name")
            .arg(name)
            .arg("--allow-empty")
            .output()
            .unwrap();
        let diagnostics = combined_output(&output);
        assert!(
            !output.status.success()
                && diagnostics.contains("invalid implicit guide name")
                && !diagnostics.contains("filesystem walk error"),
            "unsafe Windows implicit name reached filesystem access: {name:?}\n{diagnostics}"
        );
    }

    let base = root.join("base.txt");
    fs::write(&base, "ordinary base").unwrap();
    let ads = PathBuf::from(format!("{}:secret", base.display()));
    fs::write(&ads, GUIDE_SOURCE_SENTINEL)
        .expect("Windows alternate-data-stream capability is required for guide-input evidence");

    for guide_path in [
        ads.clone(),
        root.join("NUL.txt"),
        root.join("CONIN$.txt"),
        root.join("CONOUT$"),
        root.join("COM¹.log"),
        root.join("LPT9.md"),
        PathBuf::from(r"C:relative-guide.md"),
        PathBuf::from(r"\current-drive-root\guide.md"),
        PathBuf::from(r"\\.\NUL"),
        PathBuf::from(r"\\.\pipe\agentic-navigation-guide-test"),
        PathBuf::from(r"\\localhost\pipe\agentic-navigation-guide-test"),
        PathBuf::from(r"\\localhost\mailslot\agentic-navigation-guide-test"),
        PathBuf::from(r"\\localhost\IPC$\agentic-navigation-guide-test"),
        PathBuf::from(r"//?/GLOBALROOT/Device/HarddiskVolume1/agentic-navigation-guide-test"),
        PathBuf::from(r"\\?\C:\agentic-navigation-guide-test.md"),
        PathBuf::from(r"\??\C:\agentic-navigation-guide-test.md"),
    ] {
        let mut command = isolated_command();
        let output = command
            .current_dir(&root)
            .arg("check")
            .arg("--guide")
            .arg(&guide_path)
            .output()
            .unwrap();
        let diagnostics = combined_output(&output);
        assert!(
            !output.status.success()
                && diagnostics.contains("invalid explicit guide path")
                && !diagnostics.contains(GUIDE_SOURCE_SENTINEL),
            "unsafe Windows explicit path reached guide access: {guide_path:?}\n{diagnostics}"
        );
    }

    let control_path = root.join("unsafe\n\u{1b}-guide.md");
    let mut command = isolated_command();
    let output = command
        .current_dir(&root)
        .arg("check")
        .arg("--guide")
        .arg(&control_path)
        .output()
        .unwrap();
    let mut raw_output = output.stdout.clone();
    raw_output.extend_from_slice(&output.stderr);
    let diagnostics = String::from_utf8_lossy(&raw_output);
    assert!(
        !output.status.success()
            && !raw_output
                .windows(b"unsafe\n\x1b-guide.md".len())
                .any(|window| window == b"unsafe\n\x1b-guide.md")
            && diagnostics.contains("\\n"),
        "Windows guide diagnostic emitted raw controls:\n{diagnostics}"
    );

    let results = verify_guides(
        &[GuideLocation {
            guide_path: ads,
            root_path: missing_root,
        }],
        &agentic_navigation_guide::types::Config::default(),
    )
    .unwrap();
    let error = results[0].error.as_deref().unwrap_or_default();
    assert!(
        !results[0].success
            && error.contains("invalid explicit guide path")
            && !error.contains("trust anchor")
            && !error.contains(GUIDE_SOURCE_SENTINEL),
        "legacy GuideLocation bypassed Windows path validation: {error}"
    );
}

#[test]
fn test_invalid_implicit_guide_names_fail_before_search_or_allow_empty() {
    let temp = TempDir::new().unwrap();
    let missing_root = temp.path().join("missing-root");

    for name in [
        "",
        ".",
        "..",
        "../escape",
        "nested/guide.md",
        "nested\\guide.md",
        "/absolute-guide.md",
        r"C:\absolute-guide.md",
    ] {
        let mut command = isolated_command();
        let output = command
            .arg("verify")
            .arg("--recursive")
            .arg("--root")
            .arg(&missing_root)
            .arg("--guide-name")
            .arg(name)
            .arg("--allow-empty")
            .output()
            .unwrap();
        let diagnostics = combined_output(&output);

        assert_eq!(
            output.status.code(),
            Some(1),
            "invalid implicit name {name:?} did not fail before search:\n{diagnostics}"
        );
        assert!(
            diagnostics.contains("invalid implicit guide name"),
            "invalid implicit name {name:?} did not get a typed configuration error:\n{diagnostics}"
        );
        assert!(
            !diagnostics.contains("filesystem walk error")
                && !diagnostics.contains("zero navigation guides were verified"),
            "invalid implicit name {name:?} reached discovery:\n{diagnostics}"
        );

        let mut command = isolated_command();
        command
            .current_dir(temp.path())
            .env("AGENTIC_NAVIGATION_GUIDE_NAME", name)
            .arg("check");
        let output = command.output().unwrap();
        let diagnostics = combined_output(&output);
        assert!(
            !output.status.success()
                && diagnostics.contains("invalid implicit guide name")
                && !diagnostics.contains("trust anchor"),
            "check did not reject invalid implicit name {name:?} before access:\n{diagnostics}"
        );
    }
}

#[cfg(unix)]
#[test]
fn test_non_utf8_implicit_guide_name_does_not_fall_back_to_default() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = TempDir::new().unwrap();
    fs::write(
        temp.path().join("AGENTIC_NAVIGATION_GUIDE.md"),
        "<agentic-navigation-guide>\n- ordinary.txt\n</agentic-navigation-guide>",
    )
    .unwrap();
    fs::write(temp.path().join("ordinary.txt"), "").unwrap();

    let mut command = isolated_command();
    let output = command
        .current_dir(temp.path())
        .env(
            "AGENTIC_NAVIGATION_GUIDE_NAME",
            OsString::from_vec(vec![b'g', 0xff, b'.', b'm', b'd']),
        )
        .arg("check")
        .output()
        .unwrap();
    let diagnostics = combined_output(&output);

    assert!(
        !output.status.success()
            && diagnostics.contains("invalid implicit guide name")
            && diagnostics.contains("not valid UTF-8"),
        "non-UTF-8 implicit name fell back to the valid default guide:\n{diagnostics}"
    );
}

#[test]
fn test_legacy_recursive_library_path_cannot_bypass_safe_opening() {
    use agentic_navigation_guide::{find_guides, verify_guides, GuideLocation};

    let temp = TempDir::new().unwrap();
    let root = temp.path().join("root");
    let outside = temp.path().join("outside-secret.md");
    fs::create_dir(&root).unwrap();
    fs::write(
        &outside,
        format!("{GUIDE_SOURCE_SENTINEL}\nnot a navigation guide"),
    )
    .unwrap();
    let link = root.join("AGENTIC_NAVIGATION_GUIDE.md");
    create_guide_file_link(&outside, &link);

    let discovery = find_guides(&root, "AGENTIC_NAVIGATION_GUIDE.md", &[])
        .expect_err("legacy discovery accepted an unsafe matching guide");
    let discovery_error = discovery.to_string();
    assert!(
        discovery_error.contains("unsafe guide path"),
        "{discovery_error}"
    );
    assert!(
        !discovery_error.contains(GUIDE_SOURCE_SENTINEL),
        "{discovery_error}"
    );
    assert!(
        !discovery_error.contains(&outside.display().to_string()),
        "{discovery_error}"
    );

    let results = verify_guides(
        &[GuideLocation {
            guide_path: link,
            root_path: root,
        }],
        &agentic_navigation_guide::types::Config::default(),
    )
    .unwrap();
    let result = results.first().expect("one legacy verification result");
    let error = result.error.as_deref().unwrap_or_default();

    assert!(!result.success);
    assert!(error.contains("unsafe guide path"), "{error}");
    assert!(!error.contains(GUIDE_SOURCE_SENTINEL), "{error}");
    assert!(!error.contains(&outside.display().to_string()), "{error}");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuideTrustEvidenceOutcome {
    Conformant,
    #[cfg(not(windows))]
    Unavailable(&'static str),
}

struct GuideTrustEvidenceGroup {
    ids: &'static [&'static str],
    observe: fn() -> GuideTrustEvidenceOutcome,
}

const ISSUE_49_TRUST_EVIDENCE: &[GuideTrustEvidenceGroup] = &[
    GuideTrustEvidenceGroup {
        ids: &[
            "trust-guide-default-regular-in-root",
            "trust-guide-verify-default-from-effective-root",
            "trust-guide-recursive-regular-in-root",
            "trust-guide-explicit-regular-outside-root",
            "trust-guide-hard-link",
            "trust-guide-default-link-in-root",
            "trust-guide-default-link-outside-relative",
            "trust-guide-default-link-outside-absolute",
            "trust-guide-explicit-final-link-in-root",
            "trust-guide-explicit-final-link-outside-root",
            "trust-guide-recursive-final-link-in-root",
            "trust-guide-recursive-final-link-outside-root",
            "trust-guide-dangling-link",
            "trust-guide-link-chain",
            "trust-guide-link-loop",
            "trust-guide-matching-directory-or-special",
            "trust-guide-root-alias",
            "trust-guide-nonmatching-descendant-directory-link",
            "trust-guide-excluded-unsafe-match",
            "trust-guide-link-ancestor-below-root",
            "trust-guide-check-explicit-link-ancestor-below-cwd",
            "trust-guide-explicit-external-ancestor",
        ],
        observe: observe_platform_guide_policy,
    },
    GuideTrustEvidenceGroup {
        ids: &["trust-guide-windows-reparse-link"],
        observe: observe_windows_guide_reparse,
    },
    GuideTrustEvidenceGroup {
        ids: &[
            "trust-guide-windows-device-or-stream-name",
            "trust-guide-windows-device-namespace",
        ],
        observe: observe_windows_guide_spelling,
    },
    GuideTrustEvidenceGroup {
        ids: &[
            "trust-guide-name-parent-escape",
            "trust-guide-name-absolute-escape",
        ],
        observe: observe_invalid_implicit_names,
    },
    GuideTrustEvidenceGroup {
        ids: &["trust-guide-sentinel-all-modes"],
        observe: observe_guide_confidentiality,
    },
    GuideTrustEvidenceGroup {
        ids: &["trust-guide-direct-library-path"],
        observe: observe_legacy_guide_route,
    },
];

#[test]
fn test_guide_trust_evidence_is_an_exact_set_for_issue_49() {
    use std::collections::BTreeSet;

    let expected = issue_49_trust_ids(include_str!("fixtures/v0_2_trust.rs"));
    let mut declared = BTreeSet::new();
    let mut conformant = BTreeSet::new();
    #[cfg(not(windows))]
    let mut unavailable = BTreeSet::new();
    #[cfg(windows)]
    let unavailable = BTreeSet::new();

    for group in ISSUE_49_TRUST_EVIDENCE {
        let outcome = (group.observe)();
        for id in group.ids {
            assert!(
                declared.insert(*id),
                "duplicate guide trust evidence ID {id:?}"
            );
            match outcome {
                GuideTrustEvidenceOutcome::Conformant => {
                    conformant.insert(*id);
                }
                #[cfg(not(windows))]
                GuideTrustEvidenceOutcome::Unavailable(reason) => {
                    assert!(!reason.is_empty());
                    unavailable.insert(*id);
                }
            }
        }
    }

    assert_eq!(declared, expected, "guide evidence declaration drifted");
    assert!(
        conformant.is_disjoint(&unavailable),
        "a guide row cannot be both conformant and unavailable"
    );
    assert_eq!(
        conformant
            .union(&unavailable)
            .copied()
            .collect::<BTreeSet<_>>(),
        declared,
        "every declared guide row must have an explicit observation"
    );

    #[cfg(not(windows))]
    let expected_unavailable = BTreeSet::from([
        "trust-guide-windows-reparse-link",
        "trust-guide-windows-device-or-stream-name",
        "trust-guide-windows-device-namespace",
    ]);
    #[cfg(windows)]
    let expected_unavailable = BTreeSet::new();

    assert_eq!(
        unavailable, expected_unavailable,
        "unexpected unavailable guide evidence on this platform"
    );
    assert_eq!(
        conformant,
        declared
            .difference(&expected_unavailable)
            .copied()
            .collect(),
        "host-applicable guide evidence is not fully conformant"
    );
}

fn observe_platform_guide_policy() -> GuideTrustEvidenceOutcome {
    #[cfg(unix)]
    test_guide_input_trust_policy_matrix();
    #[cfg(windows)]
    test_windows_guide_input_trust_policy_matrix();
    GuideTrustEvidenceOutcome::Conformant
}

#[cfg(windows)]
fn observe_windows_guide_reparse() -> GuideTrustEvidenceOutcome {
    test_windows_guide_input_trust_policy_matrix();
    GuideTrustEvidenceOutcome::Conformant
}

#[cfg(not(windows))]
fn observe_windows_guide_reparse() -> GuideTrustEvidenceOutcome {
    GuideTrustEvidenceOutcome::Unavailable("Windows reparse evidence requires Windows")
}

#[cfg(windows)]
fn observe_windows_guide_spelling() -> GuideTrustEvidenceOutcome {
    test_windows_guide_namespaces_and_streams_reject_before_access();
    GuideTrustEvidenceOutcome::Conformant
}

#[cfg(not(windows))]
fn observe_windows_guide_spelling() -> GuideTrustEvidenceOutcome {
    GuideTrustEvidenceOutcome::Unavailable("Windows path-spelling evidence requires Windows")
}

fn observe_invalid_implicit_names() -> GuideTrustEvidenceOutcome {
    test_invalid_implicit_guide_names_fail_before_search_or_allow_empty();
    GuideTrustEvidenceOutcome::Conformant
}

fn observe_guide_confidentiality() -> GuideTrustEvidenceOutcome {
    test_guide_confidentiality_covers_every_execution_and_log_mode();
    #[cfg(unix)]
    test_guide_input_diagnostics_are_control_safe_and_bounded();
    GuideTrustEvidenceOutcome::Conformant
}

fn observe_legacy_guide_route() -> GuideTrustEvidenceOutcome {
    test_legacy_recursive_library_path_cannot_bypass_safe_opening();
    GuideTrustEvidenceOutcome::Conformant
}

fn issue_49_trust_ids(source: &str) -> std::collections::BTreeSet<&str> {
    source
        .split("TrustCase {")
        .skip(1)
        .filter_map(|block| {
            let block = block.split_once("},").map_or(block, |(case, _)| case);
            if !block.contains("owner_issue: 49") {
                return None;
            }
            block.lines().find_map(|line| {
                line.trim()
                    .strip_prefix("id: \"")
                    .and_then(|value| value.strip_suffix("\","))
            })
        })
        .collect()
}
