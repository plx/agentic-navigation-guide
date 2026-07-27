use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;
use tempfile::TempDir;

const GUIDE: &str = "<agentic-navigation-guide>\n- probe.txt\n</agentic-navigation-guide>";
const GUIDE_SENTINEL: &str = "ISSUE101_GUIDE_BYTES_MUST_NOT_BE_READ_5dc315da";

fn isolated_command() -> Command {
    let mut command = Command::cargo_bin("agentic-navigation-guide").expect("test binary");
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
    let absolute = root.join("padding/../linked/guide.md");
    for surface in ["check", "verify"] {
        for (kind, spelling) in [
            ("relative", relative.as_path()),
            ("absolute", absolute.as_path()),
        ] {
            let output = run_explicit(surface, &root, &root, spelling);
            assert_parent_path_rejected(
                &format!("{surface} --guide {kind}"),
                &output,
                &["padding", "..", "linked", "guide.md"],
                &outside,
            );
        }

        let output = run_environment(surface, &root, &root, &relative);
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
        let output = run_explicit(surface, &root, &root, &spelling);
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
        let output = run_explicit(surface, &root, &root, &spelling);
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
        let real_parent =
            run_explicit(surface, &root, &root, Path::new("padding/../real/guide.md"));
        assert_success(&format!("{surface} real parent reduction"), &real_parent);

        let relative_external =
            run_explicit(surface, &root, &root, Path::new("../outside/guide.md"));
        assert_success(
            &format!("{surface} relative explicit external guide"),
            &relative_external,
        );
    }

    let external_alias = temp.path().join("external-alias");
    create_directory_link(&outside, &external_alias);
    for surface in ["check", "verify"] {
        let stable_external = run_explicit(surface, &root, &root, &external_alias.join("guide.md"));
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
