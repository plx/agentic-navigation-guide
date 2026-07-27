#[path = "support/assert_cli.rs"]
mod test_cli;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use tempfile::TempDir;
use test_cli::{assert_cli_command, HermeticAssertCommand};

const GUIDE: &str = "<agentic-navigation-guide>\n- probe.txt\n</agentic-navigation-guide>";
const GUIDE_SENTINEL: &str = "ISSUE101_GUIDE_BYTES_MUST_NOT_BE_READ_5dc315da";

fn isolated_command() -> HermeticAssertCommand {
    assert_cli_command()
}

fn write_guide(path: &Path) {
    fs::create_dir_all(path.parent().expect("guide parent")).expect("create guide parent");
    fs::write(path, format!("{GUIDE}\n{GUIDE_SENTINEL}\n")).expect("write guide");
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_parent_path_rejected(
    label: &str,
    output: &Output,
    configured_parts: &[&str],
    resolved_target: &Path,
) {
    let diagnostics = combined_output(output);
    assert!(
        !output.status.success(),
        "{label} accepted a parent-containing guide path:\n{diagnostics}"
    );
    assert!(
        diagnostics.contains("unsafe guide path") && diagnostics.contains("link or reparse point"),
        "{label} lacked a typed unsafe-ancestor reason:\n{diagnostics}"
    );
    for part in configured_parts {
        assert!(
            diagnostics.contains(part),
            "{label} omitted configured spelling component {part:?}:\n{diagnostics}"
        );
    }
    assert!(
        !diagnostics.contains(GUIDE_SENTINEL)
            && !diagnostics.contains(&resolved_target.display().to_string()),
        "{label} disclosed guide bytes or the resolved target:\n{diagnostics}"
    );
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed:\n{}",
        combined_output(output)
    );
}

#[cfg(not(windows))]
fn absolute_anchor_spelling(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical fixture root")
}

#[cfg(windows)]
fn absolute_anchor_spelling(path: &Path) -> PathBuf {
    // `std::fs::canonicalize` returns a verbatim namespace spelling on
    // Windows, and explicit verbatim paths are intentionally rejected.
    path.to_path_buf()
}

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
        "real Windows junction capability is required for issue #101:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_explicit(surface: &str, current_dir: &Path, root: &Path, guide: &Path) -> Output {
    let mut command = isolated_command();
    command.current_dir(current_dir).arg(surface);
    if surface == "verify" {
        command.arg("--root").arg(root);
    }
    command.arg("--guide").arg(guide).output().expect("run CLI")
}

fn run_environment(surface: &str, current_dir: &Path, root: &Path, guide: &Path) -> Output {
    let mut command = isolated_command();
    command
        .current_dir(current_dir)
        .env("AGENTIC_NAVIGATION_GUIDE_PATH", guide)
        .arg(surface);
    if surface == "verify" {
        command.arg("--root").arg(root);
    }
    command.output().expect("run environment-configured CLI")
}

#[test]
fn issue_101_parent_path_rejects_in_anchor_links_on_every_explicit_surface() {
    let temp = TempDir::new().expect("temporary fixture");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside-target");
    fs::create_dir_all(root.join("padding")).expect("padding directory");
    fs::create_dir(&outside).expect("outside directory");
    fs::write(root.join("probe.txt"), "").expect("verification probe");
    write_guide(&outside.join("guide.md"));
    create_directory_link(&outside, &root.join("linked"));

    let relative = PathBuf::from("padding/../linked/guide.md");
    let absolute_anchor = absolute_anchor_spelling(&root);
    let absolute = absolute_anchor.join("padding/../linked/guide.md");
    for surface in ["check", "verify"] {
        for (kind, anchor, spelling) in [
            ("relative", Path::new("."), relative.as_path()),
            ("absolute", absolute_anchor.as_path(), absolute.as_path()),
        ] {
            let output = run_explicit(surface, &root, anchor, spelling);
            assert_parent_path_rejected(
                &format!("{surface} --guide {kind}"),
                &output,
                &["padding", "..", "linked", "guide.md"],
                &outside,
            );
        }

        let output = run_environment(surface, &root, Path::new("."), &relative);
        assert_parent_path_rejected(
            &format!("{surface} environment path"),
            &output,
            &["padding", "..", "linked", "guide.md"],
            &outside,
        );
    }
}

#[test]
fn issue_101_parent_path_rejects_links_that_resolve_back_inside_the_anchor() {
    let temp = TempDir::new().expect("temporary fixture");
    let root = temp.path().join("root");
    let real = root.join("real-target");
    fs::create_dir_all(root.join("padding")).expect("padding directory");
    fs::create_dir_all(&real).expect("real in-anchor target");
    fs::write(root.join("probe.txt"), "").expect("verification probe");
    write_guide(&real.join("guide.md"));
    create_directory_link(&real, &root.join("linked"));

    let spelling = PathBuf::from("padding/../linked/guide.md");
    for surface in ["check", "verify"] {
        let output = run_explicit(surface, &root, Path::new("."), &spelling);
        assert_parent_path_rejected(
            &format!("{surface} in-anchor target"),
            &output,
            &["padding", "..", "linked", "guide.md"],
            &real,
        );
    }
}

#[test]
fn issue_101_link_before_parent_is_not_erased_lexically() {
    let temp = TempDir::new().expect("temporary fixture");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside-target");
    let linked_target = outside.join("linked-target");
    fs::create_dir_all(&linked_target).expect("linked directory");
    fs::create_dir_all(root.join("real")).expect("Windows lexical-parent decoy");
    fs::create_dir_all(outside.join("real")).expect("Unix resolved-parent target");
    fs::write(root.join("probe.txt"), "").expect("verification probe");
    write_guide(&root.join("real/guide.md"));
    write_guide(&outside.join("real/guide.md"));
    create_directory_link(&linked_target, &root.join("linked-padding"));

    let spelling = PathBuf::from("linked-padding/../real/guide.md");
    for surface in ["check", "verify"] {
        let output = run_explicit(surface, &root, Path::new("."), &spelling);
        assert_parent_path_rejected(
            &format!("{surface} link-before-parent"),
            &output,
            &["linked-padding", "..", "real", "guide.md"],
            &outside,
        );
    }
}

#[test]
fn issue_101_real_parent_reduction_and_true_external_authority_remain_supported() {
    let temp = TempDir::new().expect("temporary fixture");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir_all(root.join("padding")).expect("padding directory");
    fs::create_dir_all(root.join("real")).expect("real directory");
    fs::create_dir(&outside).expect("outside directory");
    fs::write(root.join("probe.txt"), "").expect("verification probe");
    write_guide(&root.join("real/guide.md"));
    write_guide(&outside.join("guide.md"));

    for surface in ["check", "verify"] {
        let real_parent = run_explicit(
            surface,
            &root,
            Path::new("."),
            Path::new("padding/../real/guide.md"),
        );
        assert_success(&format!("{surface} real parent reduction"), &real_parent);

        let relative_external = run_explicit(
            surface,
            &root,
            Path::new("."),
            Path::new("../outside/guide.md"),
        );
        assert_success(
            &format!("{surface} relative explicit external guide"),
            &relative_external,
        );
    }

    let external_alias = temp.path().join("external-alias");
    create_directory_link(&outside, &external_alias);
    for surface in ["check", "verify"] {
        let stable_external = run_explicit(
            surface,
            &root,
            Path::new("."),
            &external_alias.join("guide.md"),
        );
        assert_success(
            &format!("{surface} stable explicit external ancestor"),
            &stable_external,
        );
    }
}

#[test]
fn issue_101_root_alias_and_unresolved_root_spelling_keep_parent_checks() {
    let temp = TempDir::new().expect("temporary fixture");
    let outside = temp.path().join("outside-target");
    fs::create_dir(&outside).expect("outside directory");
    write_guide(&outside.join("guide.md"));

    let real_root = temp.path().join("real-root");
    fs::create_dir_all(real_root.join("padding")).expect("aliased padding");
    fs::write(real_root.join("probe.txt"), "").expect("aliased probe");
    create_directory_link(&outside, &real_root.join("linked"));
    let root_alias = temp.path().join("root-alias");
    create_directory_link(&real_root, &root_alias);
    let alias_spelling = root_alias.join("padding/../linked/guide.md");
    let alias_output = run_explicit("verify", temp.path(), &root_alias, &alias_spelling);
    assert_parent_path_rejected(
        "verify root alias",
        &alias_output,
        &["root-alias", "padding", "..", "linked", "guide.md"],
        &outside,
    );

    let real_parent = temp.path().join("real-parent");
    let aliased_child = real_parent.join("child");
    fs::create_dir_all(&aliased_child).expect("aliased child");
    fs::create_dir(real_parent.join("padding")).expect("parent-spelling padding");
    fs::write(real_parent.join("probe.txt"), "").expect("parent-spelling probe");
    create_directory_link(&outside, &real_parent.join("linked"));
    let child_alias = temp.path().join("child-alias");
    create_directory_link(&aliased_child, &child_alias);
    let root_spelling = child_alias.join("..");
    let guide_spelling = root_spelling.join("padding/../linked/guide.md");
    let parent_output = run_explicit("verify", temp.path(), &root_spelling, &guide_spelling);
    assert_parent_path_rejected(
        "verify unresolved root spelling",
        &parent_output,
        &["child-alias", "..", "padding", "linked", "guide.md"],
        &outside,
    );
}

#[test]
fn issue_101_classifier_and_documentation_remain_aligned() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(root.join("src/guide_input.rs")).expect("guide-input source");
    for required in [
        "enum CandidateClass",
        "ProvenInAnchor",
        "ProvenExternal",
        "ParentContaining",
        "validate_component_before_parent",
    ] {
        assert!(
            source.contains(required),
            "guide-input classifier omitted {required:?}"
        );
    }
    assert!(
        !source.contains("fn safe_tail"),
        "the ambiguous safe-tail fallback was restored"
    );

    let contract =
        fs::read_to_string(root.join("docs/v0.2-contract.md")).expect("normative contract");
    let readme = fs::read_to_string(root.join("README.md")).expect("README");
    let changelog = fs::read_to_string(root.join("CHANGELOG.md")).expect("changelog");
    let normalized_contract = contract.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized_readme = readme.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized_changelog = changelog.split_whitespace().collect::<Vec<_>>().join(" ");
    let guide =
        fs::read_to_string(root.join("AGENTIC_NAVIGATION_GUIDE.md")).expect("navigation guide");
    let audit =
        fs::read_to_string(root.join("audits/2026-07-27-issue-101-parent-explicit-guide.md"))
            .expect("issue audit");
    let normalized_audit = audit.split_whitespace().collect::<Vec<_>>().join(" ");

    for required in [
        "An unresolved parent in a tail that begins beneath",
        "each component erased by `..` MUST first be inspected",
        "Only a parent-containing spelling proven to remain outside",
        "| #101 | Classify parent-containing explicit guide paths",
    ] {
        assert!(
            normalized_contract.contains(required),
            "normative contract omitted {required:?}"
        );
    }
    assert!(normalized_readme.contains(
        "An unresolved `..` in a path that starts beneath the anchor does not automatically grant external authority."
    ));
    assert!(normalized_changelog
        .contains("Parent-containing explicit guide paths no longer gain external authority"));
    assert!(guide.contains("issue_101_parent_explicit_guide.rs"));
    assert!(guide.contains("2026-07-27-issue-101-parent-explicit-guide.md"));

    for id in 1..=8 {
        let id = format!("A101-{id:03}");
        assert_eq!(
            audit.matches(&id).count(),
            1,
            "acceptance evidence ID {id} must occur exactly once"
        );
    }
    assert!(normalized_audit.contains(
        "No fuzzing, mutation testing, randomized generation, or generated hostile input was used."
    ));
}
