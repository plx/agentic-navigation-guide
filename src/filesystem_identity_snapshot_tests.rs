use crate::errors::{AppError, SemanticError};
use crate::parser::Parser;
use crate::types::{FilesystemItem, NavigationGuide, NavigationGuideLine};
use crate::verifier::Verifier;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;

const PRECOMPOSED: &str = "\u{e9}.txt";
const DECOMPOSED: &str = "e\u{301}.txt";

fn parse_guide(lines: &str) -> NavigationGuide {
    Parser::new()
        .parse(&format!(
            "<agentic-navigation-guide>\n{lines}</agentic-navigation-guide>"
        ))
        .expect("issue #50 guide must parse")
}

fn verify_lines(root: &Path, lines: &str) -> Result<(), AppError> {
    Verifier::new(root).verify(&parse_guide(lines))
}

fn enumerated_names(root: &Path) -> Vec<String> {
    let mut names = fs::read_dir(root)
        .expect("enumerate fixture root")
        .map(|entry| {
            entry
                .expect("read fixture entry")
                .file_name()
                .into_string()
                .expect("UTF-8 fixture name")
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn assert_exact_identity_mismatch(error: &AppError, line: usize, requested: &str) {
    let diagnostic = error.to_string();
    assert!(
        matches!(error, AppError::Other(_)),
        "identity mismatch must use the private error surface without changing the API ledger: {error:?}"
    );
    assert!(
        diagnostic.contains(&format!("line {line}:")),
        "identity mismatch must identify the source line: {diagnostic}"
    );
    assert!(
        diagnostic.contains("exact filesystem name"),
        "identity mismatch must explain the exact-name requirement: {diagnostic}"
    );
    assert!(
        diagnostic.contains(requested),
        "identity mismatch must identify the requested spelling: {diagnostic}"
    );
}

#[test]
fn issue_50_case_identity_is_exact_or_capability_is_explicit() {
    let temp = TempDir::new().expect("temporary case-identity root");
    fs::write(temp.path().join("Readme.md"), "").expect("case identity fixture");
    let actual = enumerated_names(temp.path())
        .into_iter()
        .next()
        .expect("enumerated case fixture");
    let alias = if actual == "README.md" {
        "Readme.md"
    } else {
        "README.md"
    };
    let host_aliases = temp.path().join(alias).is_file();
    eprintln!(
        "issue55_capability os={} dimension=case identity_mode={}",
        std::env::consts::OS,
        if host_aliases {
            "filesystem-alias"
        } else {
            "distinct-names"
        }
    );

    if host_aliases {
        let error = verify_lines(temp.path(), &format!("- {alias}\n- ...\n"))
            .expect_err("a host alias must not satisfy exact filesystem identity");
        assert_exact_identity_mismatch(&error, 2, alias);
    } else {
        fs::write(temp.path().join(alias), "").expect("case-distinct control fixture");
        assert_eq!(enumerated_names(temp.path()).len(), 2);
        verify_lines(temp.path(), &format!("- {actual}\n- ...\n"))
            .expect("the case-distinct spelling is a real unlisted entry");
        let error = verify_lines(temp.path(), &format!("- {actual}\n- {alias}\n- ...\n"))
            .expect_err("listing both exact case-distinct names leaves no placeholder entry");
        assert!(matches!(
            error,
            AppError::Semantic(SemanticError::PlaceholderNoUnmentionedItems { line: 4, .. })
        ));
    }
}

#[test]
fn issue_50_case_identity_is_exact_in_later_components() {
    let temp = TempDir::new().expect("temporary nested case-identity root");
    fs::create_dir(temp.path().join("src")).expect("nested case fixture directory");
    fs::write(temp.path().join("src/Main.rs"), "").expect("nested case fixture file");

    if temp.path().join("src/main.rs").is_file() {
        let error = verify_lines(temp.path(), "- src/main.rs\n")
            .expect_err("a later host-alias component must not verify");
        assert_exact_identity_mismatch(&error, 2, "main.rs");
    } else {
        fs::write(temp.path().join("src/main.rs"), "").expect("nested case-distinct control");
        verify_lines(temp.path(), "- src/Main.rs\n- src/main.rs\n")
            .expect("case-distinct later components must remain distinct");
    }
}

#[test]
fn issue_50_case_identity_is_exact_in_first_directory_components() {
    let temp = TempDir::new().expect("temporary directory case-identity root");
    fs::create_dir(temp.path().join("src")).expect("directory case fixture");
    fs::write(temp.path().join("src/main.rs"), "").expect("directory case fixture file");

    if temp.path().join("SRC").is_dir() {
        let error = verify_lines(temp.path(), "- SRC/main.rs\n")
            .expect_err("a first-component directory alias must not verify");
        assert_exact_identity_mismatch(&error, 2, "SRC");
    } else {
        fs::create_dir(temp.path().join("SRC")).expect("case-distinct directory control");
        fs::write(temp.path().join("SRC/main.rs"), "")
            .expect("case-distinct directory control file");
        verify_lines(temp.path(), "- src/main.rs\n- SRC/main.rs\n")
            .expect("case-distinct first directory components must remain distinct");
    }
}

#[cfg(unix)]
#[test]
fn issue_50_intermediate_alias_to_external_symlink_rejects_identity_first() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temporary symlink-alias root");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir(&root).expect("verification root");
    fs::create_dir(&outside).expect("external directory");
    fs::write(outside.join("secret.txt"), "").expect("external file");
    symlink(&outside, root.join("Src")).expect("external directory symlink");

    if fs::symlink_metadata(root.join("src")).is_err() {
        return;
    }

    let alias_error = verify_lines(&root, "- src/secret.txt\n")
        .expect_err("an aliased intermediate symlink must fail exact identity first");
    assert_exact_identity_mismatch(&alias_error, 2, "src");

    assert!(matches!(
        verify_lines(&root, "- Src/secret.txt\n"),
        // #51 deliberately supersedes the older containment-first
        // PathEscapesRoot precedence: an exact intermediate link is now
        // rejected without resolving its target.
        Err(AppError::Semantic(SemanticError::TypeMismatch {
            line: 2,
            ref expected,
            ref found,
            ref path,
        })) if expected == "directory"
            && found == "symbolic link"
            && path == "Src"
    ));
}

#[cfg(unix)]
#[test]
fn issue_50_intermediate_alias_to_dangling_symlink_rejects_identity_first() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temporary dangling-symlink-alias root");
    symlink("missing-target", temp.path().join("Src")).expect("dangling directory symlink");

    if fs::symlink_metadata(temp.path().join("src")).is_err() {
        return;
    }

    let alias_error = verify_lines(temp.path(), "- src/file.txt\n")
        .expect_err("an aliased dangling intermediate symlink must fail exact identity first");
    assert_exact_identity_mismatch(&alias_error, 2, "src");
    assert!(matches!(
        verify_lines(temp.path(), "- Src/file.txt\n"),
        // #51 rejects dangling intermediate links by their non-following
        // type observation instead of attempting target resolution.
        Err(AppError::Semantic(SemanticError::TypeMismatch {
            line: 2,
            ref expected,
            ref found,
            ref path,
        })) if expected == "directory"
            && found == "symbolic link"
            && path == "Src"
    ));
}

#[cfg(unix)]
#[test]
fn issue_50_exact_intermediate_symlink_within_root_is_not_a_directory() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temporary in-root symlink root");
    let actual = temp.path().join("actual");
    fs::create_dir(&actual).expect("in-root target directory");
    fs::write(actual.join("inside.txt"), "").expect("in-root target file");
    symlink("actual", temp.path().join("alias")).expect("in-root directory symlink");

    let error = verify_lines(temp.path(), "- alias/inside.txt\n")
        .expect_err("a flat path must not traverse an exact intermediate symlink");
    assert!(matches!(
        error,
        AppError::Semantic(SemanticError::TypeMismatch {
            line: 2,
            ref expected,
            ref found,
            ref path,
        }) if expected == "directory" && found == "symbolic link" && path == "alias"
    ));
}

#[test]
fn issue_50_unicode_identity_is_exact_or_capability_is_explicit() {
    let temp = TempDir::new().expect("temporary Unicode-identity root");
    fs::write(temp.path().join(PRECOMPOSED), "").expect("Unicode identity fixture");
    let actual = enumerated_names(temp.path())
        .into_iter()
        .next()
        .expect("enumerated Unicode fixture");
    let alias = if actual == PRECOMPOSED {
        DECOMPOSED
    } else {
        PRECOMPOSED
    };
    let host_aliases = temp.path().join(alias).is_file();
    eprintln!(
        "issue55_capability os={} dimension=unicode-normalization identity_mode={}",
        std::env::consts::OS,
        if host_aliases {
            "filesystem-alias"
        } else {
            "distinct-names"
        }
    );

    if host_aliases {
        let error = verify_lines(temp.path(), &format!("- {alias}\n- ...\n"))
            .expect_err("a normalization alias must not satisfy exact filesystem identity");
        assert_exact_identity_mismatch(&error, 2, alias);
    } else {
        fs::write(temp.path().join(alias), "").expect("Unicode-distinct control fixture");
        assert_eq!(enumerated_names(temp.path()).len(), 2);
        verify_lines(temp.path(), &format!("- {actual}\n- ...\n"))
            .expect("the Unicode-distinct spelling is a real unlisted entry");
        let error = verify_lines(temp.path(), &format!("- {actual}\n- {alias}\n- ...\n"))
            .expect_err("listing both exact Unicode names leaves no placeholder entry");
        assert!(matches!(
            error,
            AppError::Semantic(SemanticError::PlaceholderNoUnmentionedItems { line: 4, .. })
        ));
    }
}

#[test]
fn issue_50_flat_path_first_component_is_not_unmentioned() {
    let temp = TempDir::new().expect("temporary first-component root");
    fs::create_dir(temp.path().join("src")).expect("src fixture");
    fs::write(temp.path().join("src/main.rs"), "").expect("source fixture");

    let error = verify_lines(temp.path(), "- src/main.rs\n- ...\n")
        .expect_err("the only root child is mentioned by the flat guide path");
    assert!(matches!(
        error,
        AppError::Semantic(SemanticError::PlaceholderNoUnmentionedItems { line: 3, .. })
    ));
}

#[test]
fn issue_50_placeholder_matrix_preserves_partial_guide_semantics() {
    let empty = TempDir::new().expect("temporary empty root");
    assert!(matches!(
        verify_lines(empty.path(), "- ...\n"),
        Err(AppError::Semantic(
            SemanticError::PlaceholderNoUnmentionedItems { line: 2, .. }
        ))
    ));
    verify_lines(empty.path(), "- ... # future\n")
        .expect("a meaningful placeholder comment is annotation-only");

    let full = TempDir::new().expect("temporary fully-listed root");
    fs::write(full.path().join("only.txt"), "").expect("fully-listed fixture");
    assert!(matches!(
        verify_lines(full.path(), "- only.txt\n- ...\n"),
        Err(AppError::Semantic(
            SemanticError::PlaceholderNoUnmentionedItems { line: 3, .. }
        ))
    ));
    verify_lines(full.path(), "- only.txt\n- ... # future\n")
        .expect("commented placeholder may annotate a fully listed root");

    let partial = TempDir::new().expect("temporary partially-listed root");
    fs::write(partial.path().join("listed.txt"), "").expect("listed fixture");
    fs::write(partial.path().join("other.txt"), "").expect("unlisted fixture");
    verify_lines(partial.path(), "- ...\n- listed.txt\n- ...\n")
        .expect("repeated nonadjacent placeholders share one unmentioned entry");
    verify_lines(
        partial.path(),
        "- ... # before\n- listed.txt\n- ... # after\n",
    )
    .expect("placeholder ordering must not change sibling accounting");

    let nested = TempDir::new().expect("temporary nested placeholder root");
    fs::create_dir(nested.path().join("src")).expect("nested fixture directory");
    fs::write(nested.path().join("src/main.rs"), "").expect("nested listed fixture");
    assert!(matches!(
        verify_lines(nested.path(), "- src/\n  - main.rs\n  - ...\n"),
        Err(AppError::Semantic(
            SemanticError::PlaceholderNoUnmentionedItems { line: 4, .. }
        ))
    ));
    fs::write(nested.path().join("src/lib.rs"), "").expect("nested unlisted fixture");
    verify_lines(nested.path(), "- src/\n  - ...\n  - main.rs\n  - ...\n")
        .expect("nested repeated placeholders share the same unmentioned child");
}

#[cfg(unix)]
#[test]
fn issue_50_unlisted_non_utf8_name_rejects_without_a_placeholder() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let temp = TempDir::new().expect("temporary non-UTF-8 root");
    fs::write(temp.path().join("listed.txt"), "").expect("listed fixture");
    let invalid = OsStr::from_bytes(b"bad-\xFF-name");
    if fs::write(temp.path().join(invalid), "").is_err() {
        return;
    }

    let error = verify_lines(temp.path(), "- listed.txt\n")
        .expect_err("every visited parent snapshot must reject an undecodable sibling");
    assert!(matches!(
        error,
        AppError::Semantic(SemanticError::NonUtf8Path { line: 2, .. })
    ));
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("\"\\x62\\x61\\x64\\x2D\\xFF\\x2D\\x6E\\x61\\x6D\\x65\""));
    assert!(!diagnostic.contains('\u{fffd}'));
}

#[test]
fn issue_50_snapshot_cache_lives_for_one_verification_only() {
    let temp = TempDir::new().expect("temporary cache-lifetime root");
    fs::write(temp.path().join("before.txt"), "").expect("initial fixture");
    let verifier = Verifier::new(temp.path());

    verifier
        .verify(&parse_guide("- before.txt\n"))
        .expect("first verification");
    fs::rename(
        temp.path().join("before.txt"),
        temp.path().join("after.txt"),
    )
    .expect("rename between verification runs");
    verifier
        .verify(&parse_guide("- after.txt\n"))
        .expect("a later verification must construct a fresh snapshot");
    assert!(matches!(
        verifier.verify(&parse_guide("- before.txt\n")),
        Err(AppError::Semantic(SemanticError::ItemNotFound {
            line: 2,
            ..
        }))
    ));

    fs::remove_file(temp.path().join("after.txt")).expect("remove file between runs");
    fs::create_dir(temp.path().join("after.txt")).expect("replace file with directory");
    verifier
        .verify(&parse_guide("- after.txt/\n"))
        .expect("a later verification must refresh snapshotted type");
}

#[test]
fn issue_50_hard_links_keep_distinct_textual_names() {
    let temp = TempDir::new().expect("temporary hard-link identity root");
    fs::write(temp.path().join("first.txt"), "").expect("hard-link source");
    fs::hard_link(
        temp.path().join("first.txt"),
        temp.path().join("second.txt"),
    )
    .expect("hard-link alias");

    verify_lines(temp.path(), "- first.txt\n- ...\n")
        .expect("the second hard-link name remains an unlisted textual identity");
    assert!(matches!(
        verify_lines(temp.path(), "- first.txt\n- second.txt\n- ...\n"),
        Err(AppError::Semantic(
            SemanticError::PlaceholderNoUnmentionedItems { line: 4, .. }
        ))
    ));
}

#[cfg(unix)]
#[test]
fn issue_50_unlisted_control_name_rejects_without_a_placeholder() {
    let temp = TempDir::new().expect("temporary control-name root");
    fs::write(temp.path().join("listed.txt"), "").expect("listed fixture");
    let control_name = "bad\nname.txt";
    if fs::write(temp.path().join(control_name), "").is_err() {
        return;
    }

    let error = verify_lines(temp.path(), "- listed.txt\n")
        .expect_err("every visited parent snapshot must reject a control-bearing sibling");
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("line 2:"));
    assert!(diagnostic.contains("\"bad\\nname.txt\""));
    assert!(!diagnostic.contains(control_name));
    assert!(!diagnostic.contains('\u{fffd}'));
}

#[test]
#[ignore = "manual release benchmark; run with --release --ignored --nocapture --test-threads=1"]
fn issue_50_release_placeholder_scaling_benchmark() {
    if cfg!(debug_assertions) {
        panic!("the issue #50 benchmark must run with --release");
    }

    const SIZES: [usize; 3] = [500, 1_000, 2_000];
    const WARMUPS: usize = 3;
    const SAMPLES: usize = 10;
    let mut results = Vec::new();

    println!(
        "issue50_environment os={} arch={} rust={} git_sha={}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        command_output("rustc", &["--version"]),
        command_output("git", &["rev-parse", "HEAD"])
    );

    for size in SIZES {
        let temp = TempDir::new().expect("temporary benchmark root");
        for index in 0..size {
            fs::write(temp.path().join(format!("file-{index:04}.txt")), "")
                .expect("benchmark file");
        }
        let case_aliases = {
            fs::write(temp.path().join("CaseProbe"), "").expect("case capability probe");
            temp.path().join("caseprobe").exists()
        };
        let unicode_aliases = {
            fs::write(temp.path().join(PRECOMPOSED), "").expect("Unicode capability probe");
            temp.path().join(DECOMPOSED).exists()
        };
        println!(
            "issue50_capabilities size={size} case_aliases={case_aliases} unicode_aliases={unicode_aliases}"
        );

        let plain = benchmark_guide(size, false);
        let alternating = benchmark_guide(size, true);
        let verifier = Verifier::new(temp.path());

        let plain_samples = benchmark_samples(&verifier, &plain, WARMUPS, SAMPLES, "plain", size);
        let alternating_samples = benchmark_samples(
            &verifier,
            &alternating,
            WARMUPS,
            SAMPLES,
            "alternating",
            size,
        );
        let plain_median = percentile(&plain_samples, 0.50);
        let plain_p95 = percentile(&plain_samples, 0.95);
        let alternating_median = percentile(&alternating_samples, 0.50);
        let alternating_p95 = percentile(&alternating_samples, 0.95);
        println!(
            "issue50_benchmark size={size} plain_median_ms={:.3} plain_p95_ms={:.3} alternating_median_ms={:.3} alternating_p95_ms={:.3} ratio={:.3}",
            duration_ms(plain_median),
            duration_ms(plain_p95),
            duration_ms(alternating_median),
            duration_ms(alternating_p95),
            alternating_median.as_secs_f64() / plain_median.as_secs_f64()
        );
        results.push((size, plain_median, alternating_median));
    }

    for pair in results.windows(2) {
        let ratio = pair[1].2.as_secs_f64() / pair[0].2.as_secs_f64();
        println!(
            "issue50_scaling from={} to={} alternating_median_ratio={ratio:.3}",
            pair[0].0, pair[1].0
        );
        assert!(
            ratio <= 2.5,
            "alternating-placeholder median grew {ratio:.3}x; threshold is 2.5x"
        );
    }
    for (size, plain, alternating) in results {
        let ratio = alternating.as_secs_f64() / plain.as_secs_f64();
        assert!(
            ratio <= 4.0,
            "{size}-entry alternating/plain median ratio was {ratio:.3}x; threshold is 4.0x"
        );
    }
}

fn benchmark_guide(size: usize, alternating_placeholders: bool) -> NavigationGuide {
    let mut items = Vec::with_capacity(if alternating_placeholders {
        size * 2
    } else {
        size
    });
    for index in 0..size {
        items.push(NavigationGuideLine {
            line_number: index * 2 + 2,
            indent_level: 0,
            item: FilesystemItem::File {
                path: format!("file-{index:04}.txt"),
                comment: None,
            },
        });
        if alternating_placeholders {
            items.push(NavigationGuideLine {
                line_number: index * 2 + 3,
                indent_level: 0,
                item: FilesystemItem::Placeholder {
                    comment: Some("benchmark annotation".to_string()),
                },
            });
        }
    }
    NavigationGuide {
        items,
        prologue: None,
        epilogue: None,
        ignore: false,
    }
}

fn benchmark_samples(
    verifier: &Verifier,
    guide: &NavigationGuide,
    warmups: usize,
    samples: usize,
    label: &str,
    size: usize,
) -> Vec<Duration> {
    for _ in 0..warmups {
        verifier.verify(guide).expect("benchmark warmup");
    }

    let mut durations = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        verifier.verify(guide).expect("benchmark sample");
        durations.push(started.elapsed());
    }
    assert_eq!(durations.len(), samples, "{label} {size}");
    durations
}

fn percentile(samples: &[Duration], quantile: f64) -> Duration {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() as f64 * quantile).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[index]
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn command_output(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|output| output.trim().to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}
