use crate::dumper::Dumper;
use crate::parser::Parser;
use crate::recursive::find_guides;
use crate::types::{FilesystemItem, NavigationGuideLine};
use assert_cmd::Command;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

const GUIDE_NAME: &str = "AGENTIC_NAVIGATION_GUIDE.md";
const VALID_GUIDE: &str = "<agentic-navigation-guide>\n- keep.txt\n</agentic-navigation-guide>\n";

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn expected_paths(values: &[&str]) -> BTreeSet<String> {
    strings(values).into_iter().collect()
}

fn write_file(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
    fs::write(path, "").expect("write fixture");
}

fn flatten_paths(lines: &[NavigationGuideLine], parent: &str, paths: &mut BTreeSet<String>) {
    for line in lines {
        match &line.item {
            FilesystemItem::File { path, .. } => {
                let full_path = join_path(parent, path);
                paths.insert(full_path);
            }
            FilesystemItem::Directory { path, children, .. } => {
                let full_path = join_path(parent, path);
                paths.insert(full_path.clone());
                flatten_paths(children, &full_path, paths);
            }
            FilesystemItem::Placeholder { .. } => {
                paths.insert(join_path(parent, "..."));
            }
        }
    }
}

fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else {
        format!("{parent}/{child}")
    }
}

fn parsed_paths(source: &str) -> BTreeSet<String> {
    let guide = Parser::new().parse(source).expect("parse generated guide");
    let mut paths = BTreeSet::new();
    flatten_paths(&guide.items, "", &mut paths);
    paths
}

fn dump_paths(root: &Path, patterns: &[&str]) -> BTreeSet<String> {
    let source = Dumper::new(root)
        .with_exclude_patterns(&strings(patterns))
        .expect("valid exclusion patterns")
        .dump_with_wrapper()
        .expect("generate fixture");
    parsed_paths(&source)
}

fn discovered_paths(root: &Path, patterns: &[&str]) -> BTreeSet<String> {
    find_guides(root, GUIDE_NAME, &strings(patterns))
        .expect("discover guides")
        .into_iter()
        .map(|guide| {
            guide
                .guide_path
                .strip_prefix(root)
                .expect("guide below root")
                .components()
                .map(|component| component.as_os_str().to_str().expect("UTF-8 fixture"))
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect()
}

fn command() -> Command {
    Command::new(crate::test_support::cli_binary())
}

fn write_valid_guide(root: &Path, relative_directory: &str) {
    let directory = root.join(relative_directory);
    fs::create_dir_all(&directory).expect("create guide directory");
    fs::write(directory.join("keep.txt"), "").expect("kept guide entry");
    fs::write(directory.join(GUIDE_NAME), VALID_GUIDE).expect("valid guide");
}

#[test]
fn issue_44_basename_and_root_relative_patterns_are_distinct() {
    let root = TempDir::new().expect("temporary root");
    for path in [
        "target/root.txt",
        "project/target/nested.txt",
        "project/keep.txt",
        "targets/keep.txt",
        "other/project/target/keep.txt",
    ] {
        write_file(root.path(), path);
    }

    assert_eq!(
        dump_paths(root.path(), &["target"]),
        expected_paths(&[
            "other",
            "other/project",
            "project",
            "project/keep.txt",
            "targets",
            "targets/keep.txt",
        ]),
        "a no-slash pattern must match basenames at every depth"
    );

    assert_eq!(
        dump_paths(root.path(), &["project/target"]),
        expected_paths(&[
            "other",
            "other/project",
            "other/project/target",
            "other/project/target/keep.txt",
            "project",
            "project/keep.txt",
            "target",
            "target/root.txt",
            "targets",
            "targets/keep.txt",
        ]),
        "a slash pattern must match only the complete root-relative path"
    );
}

#[test]
fn issue_44_globstar_spans_complete_components_and_star_does_not() {
    let root = TempDir::new().expect("temporary root");
    for path in [
        "projects/target/zero.txt",
        "projects/a/target/one.txt",
        "projects/a/b/target/many.txt",
        "projects/a/b/not-target/keep.txt",
    ] {
        write_file(root.path(), path);
    }

    assert_eq!(
        dump_paths(root.path(), &["projects/*/target"]),
        expected_paths(&[
            "projects",
            "projects/a",
            "projects/a/b",
            "projects/a/b/not-target",
            "projects/a/b/not-target/keep.txt",
            "projects/a/b/target",
            "projects/a/b/target/many.txt",
            "projects/target",
            "projects/target/zero.txt",
        ]),
        "`*` must consume exactly one path component in this slash pattern"
    );

    assert_eq!(
        dump_paths(root.path(), &["projects/**/target"]),
        expected_paths(&[
            "projects",
            "projects/a",
            "projects/a/b",
            "projects/a/b/not-target",
            "projects/a/b/not-target/keep.txt",
        ]),
        "a complete `**` component must consume zero or many path components"
    );
}

#[test]
fn issue_44_tokens_are_unicode_scalar_based_and_patterns_form_a_union() {
    let root = TempDir::new().expect("temporary root");
    for path in [
        "nested/é.tmp",
        "nested/ab.tmp",
        "nested/b.log",
        "nested/d.log",
        "nested/cache.bin",
        "nested/keep.txt",
    ] {
        write_file(root.path(), path);
    }

    assert_eq!(
        dump_paths(root.path(), &["?.tmp", "[a-c].log", "cache.*"]),
        expected_paths(&["nested", "nested/ab.tmp", "nested/d.log", "nested/keep.txt",]),
        "`?`, classes, and every member of a pattern union must use the shared dialect"
    );
}

#[test]
fn issue_44_contract_literals_are_not_gitignore_or_globset_extensions() {
    let root = TempDir::new().expect("temporary root");
    for name in ["{one,two}", "one", "two", "!target", "target"] {
        write_file(root.path(), name);
    }

    assert_eq!(
        dump_paths(root.path(), &["{one,two}", "!target"]),
        expected_paths(&["one", "target", "two"]),
        "braces and a leading `!` are literals, not alternation or re-inclusion"
    );
}

#[test]
fn issue_44_gitignore_files_have_no_exclusion_authority() {
    let root = TempDir::new().expect("temporary root");
    fs::write(root.path().join(".gitignore"), "target\n").expect("gitignore fixture");
    write_file(root.path(), "target/kept.txt");

    assert_eq!(
        dump_paths(root.path(), &[]),
        expected_paths(&[".gitignore", "target", "target/kept.txt"])
    );
}

#[test]
fn issue_44_invalid_patterns_reject_before_root_traversal() {
    let invalid_patterns = [
        "", "/a", "a/", "a//b", ".", "..", "a/./b", "a/../b", "***", "a/**b", "a/b**", "a/***/b",
        "[]", "[!]", "[", "[z-a]", "\\", "\\q",
    ];

    for pattern in invalid_patterns {
        let error = match Dumper::new(Path::new("missing-root"))
            .with_exclude_patterns(&[pattern.to_string()])
        {
            Ok(_) => panic!("invalid exclusion pattern {pattern:?} was accepted"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("invalid glob pattern"),
            "{pattern:?} produced a non-actionable diagnostic: {error}"
        );
    }

    let missing = Path::new("issue-44-missing-recursive-root");
    let error = find_guides(missing, GUIDE_NAME, &["a/**b".to_string()])
        .expect_err("invalid recursive pattern must win over missing-root traversal");
    assert!(
        error.to_string().contains("invalid glob pattern"),
        "recursive discovery did not report the matcher error first: {error}"
    );

    let root = TempDir::new().expect("temporary root");
    write_file(root.path(), "keep.txt");
    assert!(
        Dumper::new(root.path())
            .with_exclude_patterns(&["keep.txt".to_string(), "a/**b".to_string()])
            .is_err(),
        "one invalid member must reject the complete union"
    );
}

#[test]
fn issue_44_recursive_discovery_uses_the_same_nested_basename_matcher() {
    let root = TempDir::new().expect("temporary root");
    for directory in ["target", "project/target", "project/src"] {
        fs::create_dir_all(root.path().join(directory)).expect("create guide directory");
    }
    fs::write(root.path().join("target").join(GUIDE_NAME), "invalid").expect("root target guide");
    fs::write(
        root.path().join("project/target").join(GUIDE_NAME),
        "invalid",
    )
    .expect("nested target guide");
    fs::write(
        root.path().join("project/src").join(GUIDE_NAME),
        VALID_GUIDE,
    )
    .expect("kept guide");
    fs::write(root.path().join("project/src/keep.txt"), "").expect("kept guide entry");

    assert_eq!(
        discovered_paths(root.path(), &["target"]),
        expected_paths(&["project/src/AGENTIC_NAVIGATION_GUIDE.md"])
    );

    command()
        .args(["verify", "--recursive", "--exclude", "target", "--root"])
        .arg(root.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Found 1 navigation guide(s)"));
}

#[test]
fn issue_44_recursive_verify_covers_root_relative_globstar_and_union_patterns() {
    let root = TempDir::new().expect("temporary root");
    for directory in [
        "target",
        "project/target",
        "other/project/target",
        "project/src",
        "projects/target",
        "projects/a/b/target",
    ] {
        write_valid_guide(root.path(), directory);
    }

    let cases: &[(&[&str], &[&str])] = &[
        (
            &["project/target"],
            &[
                "other/project/target/AGENTIC_NAVIGATION_GUIDE.md",
                "project/src/AGENTIC_NAVIGATION_GUIDE.md",
                "projects/a/b/target/AGENTIC_NAVIGATION_GUIDE.md",
                "projects/target/AGENTIC_NAVIGATION_GUIDE.md",
                "target/AGENTIC_NAVIGATION_GUIDE.md",
            ],
        ),
        (
            &["projects/**/target"],
            &[
                "other/project/target/AGENTIC_NAVIGATION_GUIDE.md",
                "project/src/AGENTIC_NAVIGATION_GUIDE.md",
                "project/target/AGENTIC_NAVIGATION_GUIDE.md",
                "target/AGENTIC_NAVIGATION_GUIDE.md",
            ],
        ),
        (&["target", "project/src"], &[]),
    ];

    for (patterns, expected) in cases {
        assert_eq!(
            discovered_paths(root.path(), patterns),
            expected_paths(expected),
            "recursive discovery differed for {patterns:?}"
        );

        let mut verify = command();
        verify
            .args(["verify", "--recursive", "--root"])
            .arg(root.path());
        for pattern in *patterns {
            verify.args(["--exclude", pattern]);
        }
        if expected.is_empty() {
            verify.arg("--allow-empty");
        }
        let assertion = verify.assert().success();
        if !expected.is_empty() {
            assertion.stdout(predicates::str::contains(format!(
                "Found {} navigation guide(s)",
                expected.len()
            )));
        }
    }
}

#[test]
fn issue_44_init_user_patterns_share_root_relative_globstar_and_union_semantics() {
    let root = TempDir::new().expect("temporary root");
    for path in [
        "other/keep.txt",
        "project/keep.txt",
        "project/target/hidden.txt",
        "projects/target/hidden.txt",
        "projects/a/b/keep.txt",
        "projects/a/b/target/hidden.txt",
    ] {
        write_file(root.path(), path);
    }

    let output_parent = TempDir::new().expect("output parent");
    let output = output_parent.path().join("guide.md");
    command()
        .args(["init", "--root"])
        .arg(root.path())
        .arg("--output")
        .arg(&output)
        .args([
            "--exclude",
            "project/target",
            "--exclude",
            "projects/**/target",
        ])
        .assert()
        .success();

    let source = fs::read_to_string(output).expect("read generated init guide");
    assert_eq!(
        parsed_paths(&source),
        expected_paths(&[
            "other",
            "other/keep.txt",
            "project",
            "project/keep.txt",
            "projects",
            "projects/a",
            "projects/a/b",
            "projects/a/b/keep.txt",
        ])
    );
}

#[test]
fn issue_44_init_vcs_defaults_apply_at_root_and_nested_depths() {
    let root = TempDir::new().expect("temporary root");
    const VCS_NAMES: [&str; 6] = [".git", ".svn", ".hg", ".bzr", "CVS", "_darcs"];
    for name in VCS_NAMES {
        write_file(root.path(), &format!("{name}/root-data"));
        write_file(root.path(), &format!("project/{name}/nested-data"));
    }
    write_file(root.path(), "project/keep.txt");

    let output_parent = TempDir::new().expect("output parent");
    let excluded_output = output_parent.path().join("excluded.md");
    command()
        .args(["init", "--root"])
        .arg(root.path())
        .arg("--output")
        .arg(&excluded_output)
        .assert()
        .success();
    let excluded = fs::read_to_string(&excluded_output).expect("read generated guide");
    for name in VCS_NAMES {
        assert!(
            !excluded.contains(name),
            "default init output retained nested VCS basename {name}"
        );
    }

    let included_output = output_parent.path().join("included.md");
    command()
        .args(["init", "--root"])
        .arg(root.path())
        .arg("--output")
        .arg(&included_output)
        .arg("--include-vcs-directories")
        .assert()
        .success();
    let included = fs::read_to_string(&included_output).expect("read included guide");
    for name in VCS_NAMES {
        assert!(
            included.contains(name),
            "--include-vcs-directories did not restore {name}"
        );
    }

    let explicit_output = output_parent.path().join("explicit.md");
    command()
        .args(["init", "--root"])
        .arg(root.path())
        .arg("--output")
        .arg(&explicit_output)
        .args(["--include-vcs-directories", "--exclude", ".git"])
        .assert()
        .success();
    let explicit = fs::read_to_string(&explicit_output).expect("read explicit exclusion guide");
    assert!(
        !explicit.contains(".git"),
        "an explicit user exclusion must still win when VCS defaults are disabled"
    );
    assert!(
        explicit.contains(".svn"),
        "disabling VCS defaults must not retain unrelated implicit exclusions"
    );
}

#[test]
fn issue_44_help_describes_the_shared_pattern_distinction() {
    for subcommand in ["dump", "init", "verify"] {
        let output = command()
            .args([subcommand, "--help"])
            .output()
            .expect("read help");
        assert!(output.status.success(), "{subcommand} help failed");
        let normalized = String::from_utf8(output.stdout)
            .expect("UTF-8 help")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        for required in [
            "basenames at every depth",
            "full root-relative path",
            "`**` spans path components",
        ] {
            assert!(
                normalized.contains(required),
                "{subcommand} help omitted {required:?}:\n{normalized}"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn issue_44_non_utf8_entries_fail_before_matching_but_pruned_children_are_unseen() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let direct = TempDir::new().expect("direct non-UTF-8 root");
    let non_utf8 = OsStr::from_bytes(b"bad-\xFF-name");
    if fs::write(direct.path().join(non_utf8), "").is_err() {
        return;
    }
    let diagnostic = Dumper::new(direct.path())
        .with_exclude_patterns(&["*".to_string()])
        .expect("valid wildcard")
        .dump()
        .expect_err("an encountered non-UTF-8 name must fail before wildcard pruning")
        .to_string();
    assert!(
        diagnostic.contains("UTF-8") && diagnostic.contains("\\xFF"),
        "non-UTF-8 matcher diagnostic was not actionable: {diagnostic}"
    );

    let pruned = TempDir::new().expect("pruned non-UTF-8 root");
    fs::create_dir_all(pruned.path().join("project/target")).expect("excluded directory");
    if fs::write(pruned.path().join("project/target").join(non_utf8), "").is_err() {
        return;
    }
    write_file(pruned.path(), "project/keep.txt");
    assert_eq!(
        dump_paths(pruned.path(), &["target"]),
        expected_paths(&["project", "project/keep.txt"]),
        "children below a matched UTF-8 directory must not be encountered"
    );
}
