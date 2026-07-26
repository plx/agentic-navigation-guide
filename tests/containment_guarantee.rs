use agentic_navigation_guide::{AppError, Parser, SemanticError, Verifier};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const EXTERNAL_TARGET_SENTINEL: &str = "ISSUE51_EXTERNAL_TARGET_SENTINEL";

fn verify_lines(root: &Path, lines: &str) -> Result<(), AppError> {
    let guide = Parser::new()
        .parse(&format!(
            "<agentic-navigation-guide>\n{lines}</agentic-navigation-guide>"
        ))
        .expect("issue #51 fixture guide must parse");
    Verifier::new(root).verify(&guide)
}

#[cfg(unix)]
fn create_directory_link(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create issue #51 directory link");
}

#[cfg(windows)]
fn create_directory_link(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(target, link)
        .expect("Windows directory-link capability is required for issue #51");
}

fn is_link_like_diagnostic(found: &str) -> bool {
    found.contains("link") || found.contains("reparse point")
}

#[test]
fn issue_51_parent_and_absolute_textual_paths_reject_before_access() {
    let temp = TempDir::new().expect("temporary lexical-containment root");

    for path in ["../outside.txt", "/outside.txt", "C:/outside.txt"] {
        let source = format!("<agentic-navigation-guide>\n- {path}\n</agentic-navigation-guide>");
        assert!(
            Parser::new().parse(&source).is_err(),
            "unsafe textual path unexpectedly parsed: {path}"
        );
    }

    fs::write(temp.path().join("inside.txt"), "").expect("positive containment fixture");
    verify_lines(temp.path(), "- inside.txt\n").expect("ordinary in-root entry must verify");
}

#[test]
fn issue_51_out_of_root_link_ancestor_is_rejected_without_target_disclosure() {
    let temp = TempDir::new().expect("temporary escaping-link root");
    let root = temp.path().join("root");
    let outside = temp.path().join(EXTERNAL_TARGET_SENTINEL);
    fs::create_dir(&root).expect("verification root");
    fs::create_dir(&outside).expect("external target directory");
    fs::write(outside.join("secret.txt"), "external secret").expect("external target file");
    create_directory_link(&outside, &root.join("linked"));

    let error = verify_lines(&root, "- linked/secret.txt\n")
        .expect_err("an external item must not satisfy an in-root guide entry");
    assert!(matches!(
        error,
        AppError::Semantic(SemanticError::TypeMismatch {
            line: 2,
            ref expected,
            ref found,
            ref path,
        }) if expected == "directory"
            && is_link_like_diagnostic(found)
            && path == "linked"
    ));
    let diagnostic = error.to_string();
    assert!(
        !diagnostic.contains(EXTERNAL_TARGET_SENTINEL),
        "containment diagnostics disclosed the resolved target: {diagnostic}"
    );
    assert!(
        !diagnostic.contains("external secret"),
        "containment diagnostics disclosed external content: {diagnostic}"
    );
}

#[test]
fn issue_51_in_root_link_ancestor_is_rejected_without_traversal() {
    let temp = TempDir::new().expect("temporary in-root-link root");
    let actual = temp.path().join("actual");
    fs::create_dir(&actual).expect("in-root target directory");
    fs::write(actual.join("inside.txt"), "").expect("in-root target file");
    create_directory_link(&actual, &temp.path().join("alias"));

    let error = verify_lines(temp.path(), "- alias/inside.txt\n")
        .expect_err("an in-root link ancestor must not be traversed");
    assert!(matches!(
        error,
        AppError::Semantic(SemanticError::TypeMismatch {
            line: 2,
            ref expected,
            ref found,
            ref path,
        }) if expected == "directory"
            && is_link_like_diagnostic(found)
            && path == "alias"
    ));
}

#[test]
fn issue_51_link_ancestor_with_nonexistent_final_is_rejected_without_resolution() {
    let temp = TempDir::new().expect("temporary missing-final link root");
    let actual = temp.path().join("actual");
    fs::create_dir(&actual).expect("link target directory");
    create_directory_link(&actual, &temp.path().join("alias"));

    let error = verify_lines(temp.path(), "- alias/does-not-exist.txt\n")
        .expect_err("a link ancestor must reject before looking up a missing final entry");
    assert!(matches!(
        error,
        AppError::Semantic(SemanticError::TypeMismatch {
            line: 2,
            ref expected,
            ref found,
            ref path,
        }) if expected == "directory"
            && is_link_like_diagnostic(found)
            && path == "alias"
    ));
}

#[test]
fn issue_51_dangling_link_ancestor_is_rejected_without_resolution() {
    let temp = TempDir::new().expect("temporary dangling-link root");
    create_directory_link(
        &temp.path().join("missing-target"),
        &temp.path().join("dangling"),
    );

    let error = verify_lines(temp.path(), "- dangling/missing.txt\n")
        .expect_err("a dangling ancestor must be rejected without resolution");
    assert!(matches!(
        error,
        AppError::Semantic(SemanticError::TypeMismatch {
            line: 2,
            ref expected,
            ref found,
            ref path,
        }) if expected == "directory"
            && is_link_like_diagnostic(found)
            && path == "dangling"
    ));
}

#[test]
fn issue_51_link_chain_and_loop_ancestors_are_rejected_without_resolution() {
    let temp = TempDir::new().expect("temporary chained-link root");
    let actual = temp.path().join("actual");
    fs::create_dir(&actual).expect("chain target directory");
    fs::write(actual.join("inside.txt"), "").expect("chain target file");
    create_directory_link(&actual, &temp.path().join("second"));
    create_directory_link(&temp.path().join("second"), &temp.path().join("chain"));
    create_directory_link(&temp.path().join("loop-b"), &temp.path().join("loop-a"));
    create_directory_link(&temp.path().join("loop-a"), &temp.path().join("loop-b"));

    for ancestor in ["chain", "loop-a"] {
        let error = verify_lines(temp.path(), &format!("- {ancestor}/inside.txt\n"))
            .expect_err("a chained or looping ancestor must be rejected without resolution");
        assert!(
            matches!(
                error,
                AppError::Semantic(SemanticError::TypeMismatch {
                    line: 2,
                    ref expected,
                    ref found,
                    ref path,
                }) if expected == "directory"
                    && is_link_like_diagnostic(found)
                    && path == ancestor
            ),
            "unexpected {ancestor} rejection: {error:?}"
        );
    }
}

#[test]
fn issue_51_caller_selected_root_alias_is_the_canonical_anchor() {
    let temp = TempDir::new().expect("temporary root-alias fixture");
    let real = temp.path().join("real");
    fs::create_dir(&real).expect("real root");
    fs::write(real.join("inside.txt"), "").expect("root-alias fixture file");
    let alias = temp.path().join("alias");
    create_directory_link(&real, &alias);

    verify_lines(&alias, "- inside.txt\n")
        .expect("the caller-selected root alias must be accepted as the anchor");
}

#[test]
fn issue_51_root_parent_spelling_does_not_broaden_authority() {
    let temp = TempDir::new().expect("temporary root-parent-spelling fixture");
    let real = temp.path().join("real");
    let child = real.join("child");
    fs::create_dir_all(&child).expect("aliased child");
    fs::create_dir(real.join("sub")).expect("safe root-relative directory");
    fs::write(real.join("sub/inside.txt"), "").expect("safe root-relative file");
    fs::write(temp.path().join("lexical-parent-decoy.txt"), "")
        .expect("decoy beneath the alias's lexical parent");
    let alias = temp.path().join("alias");
    create_directory_link(&child, &alias);
    let selected_root = alias.join("..");

    verify_lines(&selected_root, "- sub/inside.txt\n")
        .expect("unresolved parent components in the selected root must retain their spelling");
    let error = verify_lines(&selected_root, "- lexical-parent-decoy.txt\n")
        .expect_err("the alias's lexical parent must not broaden the canonical anchor");
    assert!(matches!(
        error,
        AppError::Semantic(SemanticError::ItemNotFound {
            line: 2,
            ref path,
            ..
        }) if path == "lexical-parent-decoy.txt"
    ));
}

#[test]
fn issue_51_path_escape_error_redacts_both_canonical_paths() {
    let error = AppError::Semantic(SemanticError::PathEscapesRoot {
        line: 17,
        path: "safe/logical/path".to_string(),
        root: EXTERNAL_TARGET_SENTINEL.into(),
        resolved: format!("{EXTERNAL_TARGET_SENTINEL}/resolved").into(),
    });
    let diagnostic = error.to_string();

    assert!(diagnostic.contains("safe/logical/path"), "{diagnostic}");
    assert!(diagnostic.contains("root boundary"), "{diagnostic}");
    assert!(
        !diagnostic.contains(EXTERNAL_TARGET_SENTINEL),
        "{diagnostic}"
    );
}

#[test]
fn issue_51_cli_and_transitional_public_routes_share_containment() {
    let temp = TempDir::new().expect("temporary shared-route root");
    let root = temp.path().join("root");
    let outside = temp.path().join(EXTERNAL_TARGET_SENTINEL);
    fs::create_dir(&root).expect("verification root");
    fs::create_dir(&outside).expect("external target");
    fs::write(outside.join("secret.txt"), "").expect("external target file");
    create_directory_link(&outside, &root.join("linked"));
    let guide_source =
        "<agentic-navigation-guide>\n- linked/secret.txt\n</agentic-navigation-guide>";
    let guide = Parser::new()
        .parse(guide_source)
        .expect("shared-route guide must parse");

    for result in [
        Verifier::new(&root).verify(&guide),
        agentic_navigation_guide::verify_guide(&guide, &root),
    ] {
        assert!(matches!(
            result,
            Err(AppError::Semantic(SemanticError::TypeMismatch {
                line: 2,
                ref expected,
                ref found,
                ref path,
            })) if expected == "directory"
                && is_link_like_diagnostic(found)
                && path == "linked"
        ));
    }

    let guide_path = root.join("guide.md");
    fs::write(&guide_path, guide_source).expect("CLI guide file");
    let output = Command::new(env!("CARGO_BIN_EXE_agentic-navigation-guide"))
        .args(["verify", "--guide"])
        .arg(&guide_path)
        .args(["--root"])
        .arg(&root)
        .output()
        .expect("run issue #51 CLI fixture");
    assert!(!output.status.success(), "CLI unexpectedly followed link");
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("expected directory")
            && (diagnostic.contains("link") || diagnostic.contains("reparse point")),
        "unexpected CLI containment diagnostic: {diagnostic}"
    );
    assert!(
        !diagnostic.contains(EXTERNAL_TARGET_SENTINEL),
        "CLI containment diagnostic disclosed the target: {diagnostic}"
    );
}

fn assert_hostile_replacement_boundary_is_documented() {
    let readme = include_str!("../README.md");
    let contract = include_str!("../docs/v0.2-contract.md");
    for (name, document) in [("README", readme), ("v0.2 contract", contract)] {
        let normalized = document.to_ascii_lowercase();
        assert!(
            normalized.contains("stable-filesystem") || normalized.contains("stable filesystem"),
            "{name} omitted the stable-filesystem boundary"
        );
        assert!(
            normalized.contains("hostile")
                && normalized.contains("replacement")
                && (normalized.contains("not a filesystem sandbox")
                    || normalized.contains("not a sandbox")),
            "{name} omitted the hostile-replacement/no-sandbox limitation"
        );
    }

    let security_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("SECURITY.md");
    if let Ok(security) = fs::read_to_string(security_path) {
        let normalized = security.to_ascii_lowercase();
        assert!(
            normalized.contains("stable")
                && normalized.contains("hostile")
                && normalized.contains("not a sandbox"),
            "the downstream SECURITY policy weakened the #51 boundary"
        );
    }
}

#[test]
fn issue_51_hostile_replacement_is_characterized_as_unsupported() {
    assert_hostile_replacement_boundary_is_documented();

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        use std::sync::{Arc, Barrier};
        use std::thread;

        const ATTEMPTS: usize = 128;

        let temp = TempDir::new().expect("temporary hostile-race root");
        let stable = temp.path().join("stable");
        let external = temp.path().join(EXTERNAL_TARGET_SENTINEL);
        fs::create_dir(&stable).expect("stable alias target");
        fs::create_dir(&external).expect("external alias target");
        fs::write(stable.join("inside.txt"), "").expect("stable listed file");
        fs::write(external.join("inside.txt"), "").expect("external listed file");
        let alias = temp.path().join("active-root");
        symlink(&stable, &alias).expect("initial caller-selected root alias");

        let barrier = Arc::new(Barrier::new(2));
        let mutator_barrier = Arc::clone(&barrier);
        let mutator_alias = alias.clone();
        let mutator_stable = stable.clone();
        let mutator_external = external.clone();
        let mutator = thread::spawn(move || -> std::io::Result<()> {
            mutator_barrier.wait();
            for index in 0..ATTEMPTS {
                let target = if index % 2 == 0 {
                    &mutator_external
                } else {
                    &mutator_stable
                };
                let staged = mutator_alias.with_extension(format!("issue51-{index}"));
                symlink(target, &staged)?;
                fs::rename(staged, &mutator_alias)?;
                thread::yield_now();
            }
            Ok(())
        });

        barrier.wait();
        let mut successes = 0;
        let mut rejections = 0;
        for _ in 0..ATTEMPTS {
            match verify_lines(&alias, "- inside.txt\n") {
                Ok(()) => successes += 1,
                Err(error) => {
                    rejections += 1;
                    assert!(
                        !error.to_string().contains(EXTERNAL_TARGET_SENTINEL),
                        "race characterization disclosed a resolved target: {error}"
                    );
                }
            }
            thread::yield_now();
        }
        mutator
            .join()
            .expect("hostile-race mutator must not panic")
            .expect("hostile-race mutator must finish its bounded swaps");
        assert_eq!(successes + rejections, ATTEMPTS);
        eprintln!(
            "issue51_hostile_root_alias_race attempts={ATTEMPTS} successes={successes} rejections={rejections} expected=unsupported"
        );
    }

    #[cfg(not(unix))]
    eprintln!(
        "issue51_hostile_root_alias_race unavailable_on={} expected=unsupported; deterministic observed-change tests remain binding",
        std::env::consts::OS
    );
}

struct TrustEvidenceGroup {
    ids: &'static [&'static str],
    tests: &'static [&'static str],
}

const ISSUE_51_TRUST_EVIDENCE: &[TrustEvidenceGroup] = &[
    TrustEvidenceGroup {
        ids: &[
            "trust-containment-root-alias",
            "trust-containment-root-parent-spelling",
        ],
        tests: &[
            "issue_51_caller_selected_root_alias_is_the_canonical_anchor",
            "issue_51_root_parent_spelling_does_not_broaden_authority",
        ],
    },
    TrustEvidenceGroup {
        ids: &[
            "trust-containment-existing-link-escape",
            "trust-containment-existing-link-in-root",
            "trust-containment-link-ancestor-missing-final",
            "trust-containment-dangling-ancestor",
            "trust-containment-link-chain-or-loop",
            "trust-containment-target-redaction",
        ],
        tests: &[
            "issue_51_out_of_root_link_ancestor_is_rejected_without_target_disclosure",
            "issue_51_in_root_link_ancestor_is_rejected_without_traversal",
            "issue_51_link_ancestor_with_nonexistent_final_is_rejected_without_resolution",
            "issue_51_dangling_link_ancestor_is_rejected_without_resolution",
            "issue_51_link_chain_and_loop_ancestors_are_rejected_without_resolution",
            "issue_51_path_escape_error_redacts_both_canonical_paths",
            "issue_51_path_escape_errors_do_not_retain_resolved_targets",
        ],
    },
    TrustEvidenceGroup {
        ids: &["trust-containment-observed-identity-change"],
        tests: &[
            "issue_51_observed_item_identity_and_type_changes_fail_closed",
            "issue_51_observed_parent_identity_change_during_enumeration_fails_closed",
            "issue_51_observed_ancestor_replacement_cannot_satisfy_an_in_root_item",
        ],
    },
    TrustEvidenceGroup {
        ids: &["trust-containment-hostile-replacement"],
        tests: &["issue_51_hostile_replacement_is_characterized_as_unsupported"],
    },
];

#[test]
fn issue_51_trust_evidence_is_an_exact_set() {
    let expected = issue_51_trust_ids(include_str!("fixtures/v0_2_trust.rs"));
    let mut declared = BTreeSet::new();
    let mut test_names = BTreeSet::new();
    for group in ISSUE_51_TRUST_EVIDENCE {
        for id in group.ids {
            assert!(
                declared.insert(*id),
                "duplicate containment trust evidence ID {id:?}"
            );
        }
        for test in group.tests {
            assert!(
                test_names.insert(*test),
                "duplicate containment evidence test {test:?}"
            );
        }
    }

    assert_eq!(expected.len(), 10, "issue #51 must retain exactly ten rows");
    assert_eq!(
        declared, expected,
        "containment evidence declaration drifted"
    );

    let executable_sources = format!(
        "{}\n{}",
        include_str!("containment_guarantee.rs"),
        include_str!("../src/verifier.rs")
    );
    for test in test_names {
        assert!(
            executable_sources.contains(&format!("fn {test}")),
            "declared containment evidence test {test:?} is not executable"
        );
    }
}

fn issue_51_trust_ids(source: &str) -> BTreeSet<&str> {
    source
        .split("TrustCase {")
        .skip(1)
        .filter_map(|block| {
            let block = block.split_once("},").map_or(block, |(case, _)| case);
            if !block.contains("owner_issue: 51") {
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

#[cfg(windows)]
#[test]
fn issue_51_windows_junction_ancestor_is_rejected_without_traversal() {
    let temp = TempDir::new().expect("temporary Windows junction root");
    let target = temp.path().join("target");
    let junction = temp.path().join("junction");
    fs::create_dir(&target).expect("junction target");
    fs::write(target.join("inside.txt"), "").expect("junction target file");
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .expect("execute mklink for issue #51 junction");
    assert!(
        output.status.success(),
        "Windows junction capability is required for issue #51: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let error = verify_lines(temp.path(), "- junction/inside.txt\n")
        .expect_err("an intermediate junction must not be traversed");
    assert!(matches!(
        error,
        AppError::Semantic(SemanticError::TypeMismatch {
            line: 2,
            ref expected,
            ref found,
            ref path,
        }) if expected == "directory"
            && found.contains("reparse point")
            && path == "junction"
    ));
}
