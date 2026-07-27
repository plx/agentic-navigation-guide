#[path = "support/process_cli.rs"]
mod test_cli;

use std::fs;
use std::path::Path;
#[cfg(unix)]
use std::process::Stdio;
use std::process::{Command, Output};
use tempfile::TempDir;
use test_cli::{process_cli_command, HermeticProcessCommand};

fn isolated_command() -> HermeticProcessCommand {
    process_cli_command()
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn write_valid_guide(path: &Path, item: &str) {
    fs::write(
        path,
        format!("<agentic-navigation-guide>\n- \"{item}\"\n</agentic-navigation-guide>"),
    )
    .expect("write valid guide");
}

#[cfg(unix)]
#[test]
fn issue_47_dump_closed_stdout_is_normal_unix_termination() {
    let temp = TempDir::new().expect("temporary broken-pipe fixture");
    let root = temp.path().join("large tree");
    fs::create_dir(&root).expect("create generation root");

    // This is a fixed, bounded fixture. Its generation work ensures the parent
    // closes the pipe reader before delivery, and its output exceeds ordinary
    // pipe capacity so the pre-fix `print!` path observes BrokenPipe.
    for index in 0..4_096 {
        let name = format!("entry-{index:04}-{}", "bounded-output-segment-".repeat(6));
        fs::write(root.join(name), "").expect("write bounded entry");
    }

    let mut child = isolated_command()
        .arg("dump")
        .arg("--root")
        .arg(&root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dump");
    drop(child.stdout.take().expect("piped stdout"));
    let output = child.wait_with_output().expect("wait for dump");
    let diagnostics = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "closed stdout was not a normal termination:\n{diagnostics}"
    );
    assert!(
        !diagnostics.to_ascii_lowercase().contains("panicked")
            && !diagnostics.contains("Broken pipe")
            && !diagnostics.contains("exit code: 101"),
        "closed stdout emitted Rust panic diagnostics:\n{diagnostics}"
    );
}

#[test]
fn issue_47_quiet_init_creates_without_ordinary_output() {
    let temp = TempDir::new().expect("temporary quiet-init fixture");
    let root = temp.path().join("workspace space Ω");
    let output_path = temp.path().join("created guide ü.md");
    fs::create_dir(&root).expect("create generation root");
    fs::write(root.join("present file Ω.txt"), "").expect("write fixture item");

    let output = isolated_command()
        .arg("--quiet")
        .arg("init")
        .arg("--root")
        .arg(&root)
        .arg("--output")
        .arg(&output_path)
        .output()
        .expect("run quiet init");

    assert!(
        output.status.success(),
        "quiet init failed:\n{}",
        combined_output(&output)
    );
    assert!(output_path.is_file(), "quiet init did not create its guide");
    assert!(
        output.stdout.is_empty() && output.stderr.is_empty(),
        "quiet init emitted ordinary chatter:\n{}",
        combined_output(&output)
    );

    let dump = isolated_command()
        .arg("--quiet")
        .arg("dump")
        .arg("--root")
        .arg(&root)
        .output()
        .expect("run quiet primary dump");
    assert!(
        dump.status.success()
            && String::from_utf8_lossy(&dump.stdout).contains("present file Ω.txt")
            && dump.stderr.is_empty(),
        "quiet mode suppressed primary dump data or emitted chatter:\n{}",
        combined_output(&dump)
    );
}

#[test]
fn issue_47_recursive_github_error_has_discovery_path_and_line() {
    let temp = TempDir::new().expect("temporary recursive-diagnostic fixture");
    let root = temp.path().join("workspace space ü");
    let module = root.join("module space Ω");
    let guide_name = "GUIDE Ω.md";
    fs::create_dir_all(&module).expect("create recursive fixture");
    write_valid_guide(&module.join(guide_name), "missing file ü.txt");

    let output = isolated_command()
        .arg("verify")
        .arg("--recursive")
        .arg("--github-actions-check")
        .arg("--guide-name")
        .arg(guide_name)
        .arg("--root")
        .arg(&root)
        .output()
        .expect("run recursive GitHub verification");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected_location = format!("module space Ω/{guide_name}:2:");

    assert_eq!(
        output.status.code(),
        Some(1),
        "recursive GitHub failure used the wrong status:\n{stderr}"
    );
    assert!(
        stderr.contains(&expected_location),
        "recursive GitHub diagnostic did not contain {expected_location:?}:\n{stderr}"
    );
    assert!(
        stderr.contains("missing file ü.txt") && stderr.contains("not found"),
        "recursive GitHub diagnostic omitted its typed reason:\n{stderr}"
    );
}

#[derive(Clone, Copy, Debug)]
enum MatrixCommand {
    Dump,
    Init,
    Check,
    Verify,
    Recursive,
}

impl MatrixCommand {
    const ALL: [Self; 5] = [
        Self::Dump,
        Self::Init,
        Self::Check,
        Self::Verify,
        Self::Recursive,
    ];
}

fn configure_mode(command: &mut Command, log: &str, execution: &str) {
    command
        .arg("--log-level")
        .arg(log)
        .arg("--execution-mode")
        .arg(execution);
}

#[test]
fn issue_47_command_log_and_execution_mode_matrix_is_stable() {
    let temp = TempDir::new().expect("temporary output-mode fixture");
    let root = temp.path().join("workspace space ü");
    let recursive_root = temp.path().join("recursive workspace Ω");
    let recursive_module = recursive_root.join("module space ü");
    let output_root = temp.path().join("outputs");
    let valid_guide = root.join("GUIDE Ω.md");
    let invalid_guide = root.join("BROKEN ü.md");
    let recursive_guide_name = "GUIDE recursive Ω.md";

    fs::create_dir(&root).expect("create single-guide root");
    fs::create_dir_all(&recursive_module).expect("create recursive root");
    fs::create_dir(&output_root).expect("create output root");
    fs::write(root.join("present file Ω.txt"), "").expect("write single-guide item");
    fs::write(recursive_module.join("present recursive ü.txt"), "").expect("write recursive item");
    write_valid_guide(&valid_guide, "present file Ω.txt");
    write_valid_guide(
        &recursive_module.join(recursive_guide_name),
        "present recursive ü.txt",
    );
    fs::write(&invalid_guide, "not a navigation guide").expect("write invalid guide");

    for (log_index, log) in ["quiet", "default", "verbose"].into_iter().enumerate() {
        for (execution_index, execution) in [
            "default",
            "post-tool-use",
            "pre-commit-hook",
            "github-actions",
        ]
        .into_iter()
        .enumerate()
        {
            let case_index = log_index * 4 + execution_index;

            for command_kind in MatrixCommand::ALL {
                let mut command = isolated_command();
                configure_mode(&mut command, log, execution);
                match command_kind {
                    MatrixCommand::Dump => {
                        command
                            .arg("dump")
                            .arg("--root")
                            .arg(&root)
                            .arg("--output")
                            .arg(output_root.join(format!("dump-{case_index}.md")));
                    }
                    MatrixCommand::Init => {
                        command
                            .arg("init")
                            .arg("--root")
                            .arg(&root)
                            .arg("--output")
                            .arg(output_root.join(format!("init-{case_index}.md")));
                    }
                    MatrixCommand::Check => {
                        command.arg("check").arg("--guide").arg(&valid_guide);
                    }
                    MatrixCommand::Verify => {
                        command
                            .arg("verify")
                            .arg("--guide")
                            .arg(&valid_guide)
                            .arg("--root")
                            .arg(&root);
                    }
                    MatrixCommand::Recursive => {
                        command
                            .arg("verify")
                            .arg("--recursive")
                            .arg("--guide-name")
                            .arg(recursive_guide_name)
                            .arg("--root")
                            .arg(&recursive_root);
                    }
                }
                let output = command.output().expect("run successful matrix case");
                let combined = combined_output(&output);
                assert!(
                    output.status.success(),
                    "{command_kind:?} failed in {log}/{execution}:\n{combined}"
                );
                if log == "quiet" {
                    assert!(
                        output.stdout.is_empty() && output.stderr.is_empty(),
                        "{command_kind:?} emitted quiet success chatter in {execution}:\n{combined}"
                    );
                } else {
                    assert!(
                        !combined.is_empty(),
                        "{command_kind:?} omitted {log} success output in {execution}"
                    );
                }
            }

            for command_kind in MatrixCommand::ALL {
                let mut command = isolated_command();
                configure_mode(&mut command, log, execution);
                match command_kind {
                    MatrixCommand::Dump => {
                        command
                            .arg("dump")
                            .arg("--root")
                            .arg(temp.path().join("missing root Ω"));
                    }
                    MatrixCommand::Init => {
                        command
                            .arg("init")
                            .arg("--root")
                            .arg(temp.path().join("missing root Ω"))
                            .arg("--output")
                            .arg(output_root.join(format!("failed-init-{case_index}.md")));
                    }
                    MatrixCommand::Check => {
                        command.arg("check").arg("--guide").arg(&invalid_guide);
                    }
                    MatrixCommand::Verify => {
                        command
                            .arg("verify")
                            .arg("--guide")
                            .arg(&invalid_guide)
                            .arg("--root")
                            .arg(&root);
                    }
                    MatrixCommand::Recursive => {
                        command
                            .arg("verify")
                            .arg("--recursive")
                            .arg("--guide-name")
                            .arg("missing guide Ω.md")
                            .arg("--root")
                            .arg(&recursive_root);
                    }
                }
                let output = command.output().expect("run failing matrix case");
                let stderr = String::from_utf8_lossy(&output.stderr);
                let expected_code = if execution == "post-tool-use" { 2 } else { 1 };
                assert_eq!(
                    output.status.code(),
                    Some(expected_code),
                    "{command_kind:?} used the wrong failure status in {log}/{execution}:\n{stderr}"
                );
                assert!(
                    !stderr.trim().is_empty(),
                    "{command_kind:?} suppressed its required error in {log}/{execution}"
                );
            }
        }
    }
}
