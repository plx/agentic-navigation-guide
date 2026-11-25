//! CLI regressions for `dump --output` and `init --output` destinations whose
//! spelling starts beneath the generation root and contains `..`.
//!
//! Such a tail receives the same ordered parent reduction as an explicit
//! guide path: every component a `..` erases is inspected without following
//! it, a spelling that leaves and returns fails closed, and only a reduction
//! proven to remain outside the root receives explicit external authority.

#[path = "support/assert_cli.rs"]
mod test_cli;
#[path = "support/environment.rs"]
mod test_environment;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use tempfile::TempDir;
use test_cli::assert_cli_command;

const COMMANDS: [&str; 2] = ["dump", "init"];

#[cfg(unix)]
fn create_directory_link(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create directory symlink");
}

#[cfg(windows)]
fn create_directory_link(target: &Path, link: &Path) {
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .expect("execute mklink /J");
    assert!(
        output.status.success(),
        "real Windows junction capability is required for parent-containing output paths:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Run `command --root <root> --output <output>` from `current_dir`,
/// excluding the in-root links so input traversal itself succeeds.
fn run(command: &str, current_dir: &Path, root: &Path, output: &Path) -> Output {
    let mut invocation = assert_cli_command();
    invocation
        .current_dir(current_dir)
        .arg(command)
        .arg("--root")
        .arg(root)
        .arg("--output")
        .arg(output)
        .args(["--exclude", "linked*"]);
    invocation.output().expect("run CLI")
}

fn files_below(path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("read fixture directory") {
            let entry = entry.expect("fixture entry");
            let file_type = entry.file_type().expect("fixture entry type");
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    files
}

fn assert_rejected(
    label: &str,
    output: &Output,
    reason: &str,
    configured_parts: &[&str],
    resolved_target: &Path,
) {
    let diagnostics = combined_output(output);
    assert!(
        !output.status.success(),
        "{label} accepted a parent-containing output path:\n{diagnostics}"
    );
    assert!(
        diagnostics.contains("unsafe output destination") && diagnostics.contains(reason),
        "{label} lacked the typed reason {reason:?}:\n{diagnostics}"
    );
    for part in configured_parts {
        assert!(
            diagnostics.contains(part),
            "{label} omitted configured spelling component {part:?}:\n{diagnostics}"
        );
    }
    assert!(
        !diagnostics.contains(&resolved_target.display().to_string()),
        "{label} disclosed the resolved target:\n{diagnostics}"
    );
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed:\n{}",
        combined_output(output)
    );
}

/// A generation root with real directories `padding` and `real`, an input
/// file, and an in-root directory link `linked` to a sibling of the root.
struct Fixture {
    temp: TempDir,
    root: PathBuf,
    outside: PathBuf,
}

fn fixture() -> Fixture {
    let temp = TempDir::new().expect("temporary fixture");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside-target");
    fs::create_dir_all(root.join("padding")).expect("padding directory");
    fs::create_dir_all(root.join("real")).expect("real directory");
    fs::write(root.join("input.txt"), "input").expect("input file");
    fs::create_dir(&outside).expect("outside target");
    create_directory_link(&outside, &root.join("linked"));
    // Unix resolves `linked/..` to the link target's parent (the fixture
    // directory) and Windows erases `linked\..` lexically (the root). Give
    // both readings a writable `real` directory so neither can mask a write.
    fs::create_dir(temp.path().join("real")).expect("resolved-parent decoy");
    Fixture {
        temp,
        root,
        outside,
    }
}

#[test]
fn in_root_link_before_parent_is_rejected_on_every_output_command() {
    let fixture = fixture();
    let before = files_below(fixture.temp.path());
    for command in COMMANDS {
        for (kind, current_dir, root, spelling) in [
            (
                "relative from the root",
                fixture.root.clone(),
                PathBuf::from("."),
                PathBuf::from("linked/../escaped.md"),
            ),
            (
                "relative from the parent",
                fixture.temp.path().to_path_buf(),
                PathBuf::from("root"),
                PathBuf::from("root/linked/../real/escaped.md"),
            ),
            (
                "absolute",
                fixture.temp.path().to_path_buf(),
                fixture.root.clone(),
                fixture.root.join("linked/../escaped.md"),
            ),
        ] {
            let output = run(command, &current_dir, &root, &spelling);
            assert_rejected(
                &format!("{command} --output {kind}"),
                &output,
                "an output-path component before '..' is a link or reparse point",
                &["linked", "..", "escaped.md"],
                &fixture.outside,
            );
        }
    }
    assert_eq!(files_below(fixture.temp.path()), before);
}

#[test]
fn link_remaining_after_reduction_and_leave_and_return_are_rejected() {
    let fixture = fixture();
    let before = files_below(fixture.temp.path());
    for command in COMMANDS {
        let linked = run(
            command,
            &fixture.root,
            Path::new("."),
            Path::new("padding/../linked/out.md"),
        );
        assert_rejected(
            &format!("{command} link after reduction"),
            &linked,
            "an ancestor below the selected generation root is a link or reparse point",
            &["padding", "..", "linked", "out.md"],
            &fixture.outside,
        );

        let returning = run(
            command,
            &fixture.root,
            Path::new("."),
            Path::new("../root/out.md"),
        );
        assert_rejected(
            &format!("{command} leave and return"),
            &returning,
            "leaves and then returns to the selected generation root",
            &["..", "root", "out.md"],
            &fixture.outside,
        );
    }
    assert_eq!(files_below(fixture.temp.path()), before);
}

#[test]
fn real_parent_reduction_and_proven_external_authority_remain_supported() {
    let fixture = fixture();
    let external = fixture.temp.path().join("external");
    fs::create_dir(&external).expect("external directory");
    for command in COMMANDS {
        let in_root = format!("{command}-in-root.md");
        let reduced = run(
            command,
            &fixture.root,
            Path::new("."),
            &Path::new("padding/..").join(&in_root),
        );
        assert_success(&format!("{command} real parent reduction"), &reduced);
        assert!(fixture.root.join(&in_root).is_file());

        let external_name = format!("{command}-external.md");
        let proven_external = run(
            command,
            &fixture.root,
            Path::new("."),
            &Path::new("padding/../../external").join(&external_name),
        );
        assert_success(
            &format!("{command} proven external reduction"),
            &proven_external,
        );
        assert!(external.join(&external_name).is_file());
    }
    assert!(files_below(&fixture.outside).is_empty());
}

#[test]
fn parent_output_contract_and_documentation_remain_aligned() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let normalized = |path: &str| {
        fs::read_to_string(root.join(path))
            .unwrap_or_else(|error| panic!("read {path}: {error}"))
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let contract = normalized("docs/v0.2-contract.md");
    for required in [
        "For an explicit guide or an output destination, each component erased by `..`",
        "Only a parent-containing spelling proven to remain outside",
        "A destination whose spelling begins with the complete generation-root spelling and contains `..` receives the ordered parent reduction",
        "its external authority applies to the reduced path",
    ] {
        assert!(
            contract.contains(required),
            "normative contract omitted {required:?}"
        );
    }
    assert!(normalized("README.md").contains(
        "Parent components are classified in order for explicit guide and `--output` paths"
    ));
    assert!(normalized("CHANGELOG.md")
        .contains("Parent-containing `--output` destinations no longer gain external authority"));
    assert!(normalized("AGENTIC_NAVIGATION_GUIDE.md").contains("parent_output_destination.rs"));

    let source = fs::read_to_string(root.join("src/cli/output.rs")).expect("output source");
    for required in [
        "output_route(",
        "parent_reduction_plan(",
        "parent_reduction_outcome(",
        "inspect_component_before_parent(",
    ] {
        assert!(
            source.contains(required),
            "output glue omitted {required:?}"
        );
    }
}
