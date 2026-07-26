use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;
use tempfile::TempDir;

const ENVIRONMENT_VARIABLES: &[&str] = &[
    "AGENTIC_NAVIGATION_GUIDE_PATH",
    "AGENTIC_NAVIGATION_GUIDE_ROOT",
    "AGENTIC_NAVIGATION_GUIDE_NAME",
    "AGENTIC_NAVIGATION_GUIDE_LOG_MODE",
    "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE",
];
const DEFAULT_GUIDE_NAME: &str = "AGENTIC_NAVIGATION_GUIDE.md";
const VALID_GUIDE: &str = "<agentic-navigation-guide>\n- present.txt\n</agentic-navigation-guide>";

fn isolated_command() -> Command {
    let mut command = Command::cargo_bin("agentic-navigation-guide").expect("test binary");
    command.timeout(Duration::from_secs(5));
    for variable in ENVIRONMENT_VARIABLES {
        command.env_remove(variable);
    }
    command
}

fn write_guide(directory: &Path, name: &str) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, VALID_GUIDE).expect("fixture guide");
    path
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed:\n{}",
        combined_output(output)
    );
}

fn precedence_fixture() -> (TempDir, PathBuf, PathBuf) {
    let temp = TempDir::new().expect("temporary fixture");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir(&root).expect("fixture root");
    fs::create_dir(&outside).expect("outside fixture directory");
    fs::write(root.join("present.txt"), "").expect("fixture item");
    write_guide(&root, DEFAULT_GUIDE_NAME);
    write_guide(&root, "ENV_GUIDE.md");
    write_guide(&root, "CLI_GUIDE.md");
    let external_guide = write_guide(&outside, "external-guide.md");
    (temp, root, external_guide)
}

#[test]
fn issue_46_guide_path_and_name_precedence_matrix() {
    let (temp, root, external_guide) = precedence_fixture();
    let missing_guide = temp.path().join("missing-guide.md");

    for command_name in ["check", "verify"] {
        let mut command = isolated_command();
        command
            .current_dir(&root)
            .env("AGENTIC_NAVIGATION_GUIDE_PATH", &external_guide)
            .arg(command_name);
        if command_name == "verify" {
            command.arg("--root").arg(&root);
        }
        let output = command.output().expect("environment path command");
        assert_success(
            &format!("{command_name} did not use the environment path"),
            &output,
        );

        let mut command = isolated_command();
        command
            .current_dir(&root)
            .env("AGENTIC_NAVIGATION_GUIDE_PATH", &external_guide)
            .arg(command_name)
            .arg("--guide")
            .arg(&external_guide);
        if command_name == "verify" {
            command.arg("--root").arg(&root);
        }
        let output = command.output().expect("equivalent CLI path command");
        assert_success(
            &format!("{command_name} rejected an equivalent CLI path"),
            &output,
        );

        let mut command = isolated_command();
        command
            .current_dir(&root)
            .env("AGENTIC_NAVIGATION_GUIDE_PATH", &missing_guide)
            .env("AGENTIC_NAVIGATION_GUIDE_NAME", "nested/invalid.md")
            .arg(command_name)
            .arg("--guide")
            .arg(&external_guide);
        if command_name == "verify" {
            command.arg("--root").arg(&root);
        }
        let output = command.output().expect("overriding CLI path command");
        assert_success(
            &format!("{command_name} did not let --guide override path/name defaults"),
            &output,
        );

        let mut command = isolated_command();
        command
            .current_dir(&root)
            .env("AGENTIC_NAVIGATION_GUIDE_PATH", &external_guide)
            .env("AGENTIC_NAVIGATION_GUIDE_NAME", "nested/invalid.md")
            .arg(command_name);
        if command_name == "verify" {
            command.arg("--root").arg(&root);
        }
        let output = command.output().expect("path versus name command");
        assert_success(
            &format!("{command_name} did not give environment PATH precedence over NAME"),
            &output,
        );
    }

    for command_name in ["check", "verify"] {
        let mut command = isolated_command();
        command
            .current_dir(&root)
            .env("AGENTIC_NAVIGATION_GUIDE_NAME", "ENV_GUIDE.md")
            .arg(command_name);
        if command_name == "verify" {
            command.arg("--root").arg(&root);
        }
        let output = command.output().expect("environment name command");
        assert_success(
            &format!("{command_name} did not use the implicit environment name"),
            &output,
        );
    }

    let output = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_PATH", &missing_guide)
        .env("AGENTIC_NAVIGATION_GUIDE_NAME", "ENV_GUIDE.md")
        .arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(&root)
        .output()
        .expect("recursive environment command");
    assert_success(
        "recursive verify did not ignore environment PATH and use NAME",
        &output,
    );

    let output = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_NAME", "ENV_GUIDE.md")
        .arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(&root)
        .arg("--guide-name")
        .arg("ENV_GUIDE.md")
        .output()
        .expect("equivalent recursive CLI name command");
    assert_success(
        "recursive verify rejected an equivalent explicit --guide-name",
        &output,
    );

    let output = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_NAME", "nested/invalid.md")
        .arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(&root)
        .arg("--guide-name")
        .arg("CLI_GUIDE.md")
        .output()
        .expect("recursive CLI name command");
    assert_success(
        "recursive --guide-name did not override the environment name",
        &output,
    );

    for command_name in ["check", "verify"] {
        let mut command = isolated_command();
        command.current_dir(&root).arg(command_name);
        if command_name == "verify" {
            command.arg("--root").arg(&root);
        }
        let output = command.output().expect("built-in guide command");
        assert_success(
            &format!("{command_name} did not use the built-in guide name"),
            &output,
        );
    }

    let output = isolated_command()
        .arg("verify")
        .arg("--recursive")
        .arg("--root")
        .arg(&root)
        .output()
        .expect("built-in recursive guide command");
    assert_success(
        "recursive verify did not use the built-in guide name",
        &output,
    );
}

fn run_root_surface(
    surface: &str,
    root: Option<&Path>,
    environment_root: Option<&Path>,
    output_path: &Path,
) -> Output {
    let mut command = isolated_command();
    if let Some(environment_root) = environment_root {
        command.env("AGENTIC_NAVIGATION_GUIDE_ROOT", environment_root);
    }

    match surface {
        "dump" => {
            command.arg("dump");
        }
        "init" => {
            command.arg("init").arg("--output").arg(output_path);
        }
        "verify" => {
            command.arg("verify");
        }
        "verify-recursive" => {
            command.arg("verify").arg("--recursive");
        }
        _ => panic!("unknown root surface"),
    }

    if let Some(root) = root {
        command.arg("--root").arg(root);
    }
    command.output().expect("root surface command")
}

#[test]
fn issue_46_root_precedence_and_command_scope_matrix() {
    let (temp, root, external_guide) = precedence_fixture();
    fs::write(root.join("root-only.txt"), "").expect("root marker");
    let missing_root = temp.path().join("missing-root");

    for (index, surface) in ["dump", "init", "verify", "verify-recursive"]
        .into_iter()
        .enumerate()
    {
        let environment_output = temp.path().join(format!("env-{index}.md"));
        let output = run_root_surface(surface, None, Some(&root), environment_output.as_path());
        assert_success(
            &format!("{surface} did not use AGENTIC_NAVIGATION_GUIDE_ROOT"),
            &output,
        );
        if surface == "dump" {
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("root-only.txt"),
                "dump environment root did not select the fixture root"
            );
        }
        if surface == "init" {
            assert!(
                environment_output.exists(),
                "init did not create its output"
            );
        }

        let equivalent_output = temp.path().join(format!("equivalent-{index}.md"));
        let output = run_root_surface(
            surface,
            Some(&root),
            Some(&root),
            equivalent_output.as_path(),
        );
        assert_success(
            &format!("{surface} rejected an equivalent explicit --root"),
            &output,
        );
        if surface == "init" {
            assert!(
                equivalent_output.exists(),
                "init with an equivalent explicit root did not create its output"
            );
        }

        let cli_output = temp.path().join(format!("cli-{index}.md"));
        let output = run_root_surface(
            surface,
            Some(&root),
            Some(&missing_root),
            cli_output.as_path(),
        );
        assert_success(
            &format!("{surface} did not let --root override the environment root"),
            &output,
        );
        if surface == "init" {
            assert!(cli_output.exists(), "overriding init did not create output");
        }
    }

    let output = isolated_command()
        .current_dir(&root)
        .env("AGENTIC_NAVIGATION_GUIDE_ROOT", "")
        .arg("check")
        .arg("--guide")
        .arg(&external_guide)
        .output()
        .expect("check with irrelevant root");
    assert_success("check consulted an irrelevant environment root", &output);

    let built_in_output = temp.path().join("built-in.md");
    let mut command = isolated_command();
    command.current_dir(&root).arg("dump");
    let output = command.output().expect("built-in dump root");
    assert_success(
        "dump did not fall back to its current-directory root",
        &output,
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("root-only.txt"));

    let mut command = isolated_command();
    command
        .current_dir(&root)
        .arg("init")
        .arg("--output")
        .arg(&built_in_output);
    let output = command.output().expect("built-in init root");
    assert_success(
        "init did not fall back to its current-directory root",
        &output,
    );
    assert!(built_in_output.exists());
}

#[test]
fn issue_46_log_mode_precedence_and_true_cli_conflicts() {
    let (temp, root, external_guide) = precedence_fixture();

    let quiet = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_LOG_MODE", "quiet")
        .arg("check")
        .arg("--guide")
        .arg(&external_guide)
        .output()
        .expect("environment quiet command");
    assert_success("environment quiet mode failed", &quiet);
    assert!(
        quiet.stdout.is_empty(),
        "environment quiet mode emitted ordinary output"
    );

    let equivalent = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_LOG_MODE", "quiet")
        .arg("check")
        .arg("--guide")
        .arg(&external_guide)
        .arg("--quiet")
        .output()
        .expect("equivalent log command");
    assert_success("equivalent CLI log mode failed", &equivalent);
    assert!(equivalent.stdout.is_empty());

    let verbose_override = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_LOG_MODE", "quiet")
        .arg("check")
        .arg("--guide")
        .arg(&external_guide)
        .arg("--verbose")
        .output()
        .expect("verbose override command");
    assert_success(
        "--verbose did not override environment quiet",
        &verbose_override,
    );
    assert!(
        !verbose_override.stdout.is_empty(),
        "--verbose left environment quiet mode active"
    );

    let quiet_override = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_LOG_MODE", "verbose")
        .arg("check")
        .arg("--guide")
        .arg(&external_guide)
        .arg("--quiet")
        .output()
        .expect("quiet override command");
    assert_success(
        "--quiet did not override environment verbose",
        &quiet_override,
    );
    assert!(quiet_override.stdout.is_empty());

    let poison = "ISSUE46_LOG_SECRET\nforged-line";
    let poisoned_override = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_LOG_MODE", poison)
        .arg("--quiet")
        .arg("check")
        .arg("--guide")
        .arg(&external_guide)
        .output()
        .expect("poisoned log override command");
    assert_success(
        "explicit quiet did not shadow an invalid environment log mode",
        &poisoned_override,
    );
    assert!(!combined_output(&poisoned_override).contains("ISSUE46_LOG_SECRET"));

    let default_output = isolated_command()
        .arg("check")
        .arg("--guide")
        .arg(&external_guide)
        .output()
        .expect("built-in log mode command");
    assert_success("built-in default log mode failed", &default_output);
    assert!(!default_output.stdout.is_empty());

    for arguments in [
        vec!["--quiet", "--verbose"],
        vec!["--quiet", "--log-level", "quiet"],
    ] {
        let output = isolated_command()
            .args(arguments)
            .arg("check")
            .arg("--guide")
            .arg(&external_guide)
            .output()
            .expect("genuine log conflict");
        assert_eq!(
            output.status.code(),
            Some(2),
            "genuine CLI log conflict was not a usage error:\n{}",
            combined_output(&output)
        );
        assert!(combined_output(&output).contains("cannot be used with"));
    }

    drop(temp);
    drop(root);
}

#[test]
fn issue_46_execution_mode_precedence_and_true_cli_conflicts() {
    let (temp, root, external_guide) = precedence_fixture();
    fs::remove_file(root.join("present.txt")).expect("make verification fail");

    let run_failure = |environment: Option<&str>, cli_arguments: &[&str]| {
        let mut command = isolated_command();
        if let Some(environment) = environment {
            command.env("AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE", environment);
        }
        command
            .arg("verify")
            .arg("--guide")
            .arg(&external_guide)
            .arg("--root")
            .arg(&root)
            .args(cli_arguments)
            .output()
            .expect("execution-mode command")
    };

    let environment_only = run_failure(Some("post-tool-use"), &[]);
    assert_eq!(environment_only.status.code(), Some(2));

    let equivalent = run_failure(Some("post-tool-use"), &["--post-tool-use-hook"]);
    assert_eq!(equivalent.status.code(), Some(2));

    let direct_override = run_failure(Some("post-tool-use"), &["--execution-mode", "default"]);
    assert_eq!(
        direct_override.status.code(),
        Some(1),
        "explicit direct execution mode did not override the environment"
    );

    let hook_override = run_failure(Some("post-tool-use"), &["--github-actions-check"]);
    assert_eq!(
        hook_override.status.code(),
        Some(1),
        "explicit hook mode did not override the environment:\n{}",
        combined_output(&hook_override)
    );
    assert!(combined_output(&hook_override).contains("❌"));

    let poison = "ISSUE46_EXECUTION_SECRET\nforged-line";
    let poisoned_override = run_failure(Some(poison), &["--pre-commit-hook"]);
    assert_eq!(
        poisoned_override.status.code(),
        Some(1),
        "explicit hook did not shadow an invalid environment execution mode"
    );
    assert!(!combined_output(&poisoned_override).contains("ISSUE46_EXECUTION_SECRET"));

    let built_in = run_failure(None, &[]);
    assert_eq!(built_in.status.code(), Some(1));

    for cli_arguments in [
        vec!["--post-tool-use-hook", "--pre-commit-hook"],
        vec!["--execution-mode", "default", "--github-actions-check"],
    ] {
        let output = run_failure(None, &cli_arguments);
        assert_eq!(
            output.status.code(),
            Some(2),
            "genuine CLI execution conflict was not a usage error:\n{}",
            combined_output(&output)
        );
        assert!(combined_output(&output).contains("cannot be used with"));
    }

    drop(temp);
}

#[test]
fn issue_46_irrelevant_environment_values_do_not_poison_commands() {
    let (temp, root, external_guide) = precedence_fixture();
    let poison = "ISSUE46_IRRELEVANT_SECRET\nforged-line";

    let dump = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_PATH", poison)
        .env("AGENTIC_NAVIGATION_GUIDE_NAME", poison)
        .arg("dump")
        .arg("--root")
        .arg(&root)
        .output()
        .expect("dump with irrelevant guide variables");
    assert_success("dump consulted irrelevant guide variables", &dump);
    assert!(!combined_output(&dump).contains("ISSUE46_IRRELEVANT_SECRET"));

    let init_output = temp.path().join("irrelevant-init.md");
    let init = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_PATH", poison)
        .env("AGENTIC_NAVIGATION_GUIDE_NAME", poison)
        .arg("init")
        .arg("--root")
        .arg(&root)
        .arg("--output")
        .arg(&init_output)
        .output()
        .expect("init with irrelevant guide variables");
    assert_success("init consulted irrelevant guide variables", &init);
    assert!(!combined_output(&init).contains("ISSUE46_IRRELEVANT_SECRET"));
    assert!(init_output.exists());

    let check = isolated_command()
        .env("AGENTIC_NAVIGATION_GUIDE_ROOT", poison)
        .arg("check")
        .arg("--guide")
        .arg(&external_guide)
        .output()
        .expect("check with irrelevant root");
    assert_success("check consulted an irrelevant root", &check);
    assert!(!combined_output(&check).contains("ISSUE46_IRRELEVANT_SECRET"));
}

#[test]
fn issue_46_selected_invalid_environment_values_are_safe_and_actionable() {
    let (temp, root, external_guide) = precedence_fixture();
    let cases = [
        (
            "AGENTIC_NAVIGATION_GUIDE_LOG_MODE",
            "ISSUE46_INVALID_LOG\nforged-line",
            vec!["check", "--guide"],
            Some(external_guide.as_path()),
        ),
        (
            "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE",
            "ISSUE46_INVALID_EXECUTION\nforged-line",
            vec!["check", "--guide"],
            Some(external_guide.as_path()),
        ),
        ("AGENTIC_NAVIGATION_GUIDE_PATH", "", vec!["check"], None),
        ("AGENTIC_NAVIGATION_GUIDE_ROOT", "", vec!["dump"], None),
        ("AGENTIC_NAVIGATION_GUIDE_NAME", "", vec!["check"], None),
    ];

    for (variable, value, arguments, trailing_path) in cases {
        let mut command = isolated_command();
        command
            .current_dir(&root)
            .env(variable, value)
            .args(arguments);
        if let Some(trailing_path) = trailing_path {
            command.arg(trailing_path);
        }
        let output = command.output().expect("invalid environment command");
        let diagnostics = combined_output(&output);
        assert!(
            !output.status.success(),
            "{variable} accepted its selected invalid value"
        );
        assert!(
            diagnostics.contains(variable),
            "{variable} diagnostic did not identify the variable:\n{diagnostics}"
        );
        assert!(
            !diagnostics.contains("ISSUE46_INVALID_") && !diagnostics.contains("forged-line"),
            "{variable} diagnostic echoed untrusted environment content:\n{diagnostics}"
        );
        assert!(
            output.stdout.is_empty(),
            "{variable} delivered command output before configuration rejection"
        );
    }

    drop(temp);
}

#[test]
fn issue_46_help_and_readme_document_names_without_values() {
    let secret = "ISSUE46_HELP_SECRET\nforged-line";

    for arguments in [
        vec!["--help"],
        vec!["dump", "--help"],
        vec!["init", "--help"],
        vec!["check", "--help"],
        vec!["verify", "--help"],
        vec!["--version"],
    ] {
        let mut command = isolated_command();
        for variable in ENVIRONMENT_VARIABLES {
            command.env(variable, secret);
        }
        let output = command
            .args(arguments.clone())
            .output()
            .expect("help command");
        assert_success(&format!("{arguments:?}"), &output);
        assert!(
            !combined_output(&output).contains("ISSUE46_HELP_SECRET")
                && !combined_output(&output).contains("forged-line"),
            "{arguments:?} leaked an environment value:\n{}",
            combined_output(&output)
        );
    }

    let mut command = isolated_command();
    for variable in ENVIRONMENT_VARIABLES {
        command.env(variable, secret);
    }
    let help = command.arg("--help").output().expect("top-level help");
    let help = combined_output(&help);
    for variable in ENVIRONMENT_VARIABLES {
        assert!(
            help.contains(variable),
            "top-level help omitted {variable}:\n{help}"
        );
    }
    assert!(
        help.contains("CLI") && help.contains("environment") && help.contains("built-in"),
        "top-level help omitted the precedence order:\n{help}"
    );

    let readme = include_str!("../README.md");
    for variable in ENVIRONMENT_VARIABLES {
        assert!(readme.contains(variable), "README omitted {variable}");
    }
    assert!(
        readme.contains("CLI") && readme.contains("environment") && readme.contains("built-in"),
        "README omitted the precedence order"
    );
}

#[test]
fn issue_46_genuine_guide_cli_relations_remain_usage_errors() {
    let (_temp, root, external_guide) = precedence_fixture();

    let conflicts = isolated_command()
        .arg("verify")
        .arg("--guide")
        .arg(&external_guide)
        .arg("--recursive")
        .arg("--root")
        .arg(&root)
        .output()
        .expect("guide conflict");
    assert_eq!(conflicts.status.code(), Some(2));
    assert!(combined_output(&conflicts).contains("cannot be used with"));

    for option in ["--guide-name", "--exclude"] {
        let output = isolated_command()
            .arg("verify")
            .arg(option)
            .arg("value")
            .arg("--root")
            .arg(&root)
            .output()
            .expect("recursive requirement");
        assert_eq!(output.status.code(), Some(2));
        assert!(combined_output(&output).contains("--recursive"));
    }

    let allow_empty = isolated_command()
        .arg("verify")
        .arg("--allow-empty")
        .arg("--root")
        .arg(&root)
        .output()
        .expect("allow-empty requirement");
    assert_eq!(allow_empty.status.code(), Some(2));
    assert!(combined_output(&allow_empty).contains("--recursive"));
}
