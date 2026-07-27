use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn job_block(workflow: &str, job_name: &str) -> String {
    let start_marker = format!("  {job_name}:");
    let mut lines = workflow
        .lines()
        .skip_while(|line| *line != start_marker)
        .peekable();
    lines
        .peek()
        .unwrap_or_else(|| panic!("missing CI job {job_name:?}"));
    let mut block = String::new();
    for line in lines {
        if line != start_marker
            && leading_spaces(line) == 2
            && !line.trim().is_empty()
            && !line.trim_start().starts_with('#')
        {
            break;
        }
        block.push_str(line);
        block.push('\n');
    }
    block
}

fn portable_relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .expect("source beneath repository root")
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn rust_sources_below(path: &Path, sources: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
    {
        let entry = entry.expect("read source directory entry");
        let path = entry.path();
        if path.is_dir() {
            rust_sources_below(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

fn ignored_tests() -> BTreeSet<String> {
    let root = repository_root();
    let mut sources = Vec::new();
    rust_sources_below(&root.join("src"), &mut sources);
    rust_sources_below(&root.join("tests"), &mut sources);
    sources.sort();

    let mut ignored = BTreeSet::new();
    for path in sources {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let lines = source.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            if !line.trim_start().starts_with("#[ignore") {
                continue;
            }
            let function = lines
                .iter()
                .skip(index + 1)
                .find_map(|candidate| {
                    candidate
                        .trim()
                        .strip_prefix("fn ")
                        .and_then(|rest| rest.split_once('('))
                        .map(|(name, _)| name)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "{}:{} ignored attribute has no following test function",
                        path.display(),
                        index + 1
                    )
                });
            let relative = portable_relative_path(&path, &root);
            ignored.insert(format!("{relative}::{function}::{}", line.trim()));
        }
    }
    ignored
}

#[test]
fn issue_55_ci_runs_complete_locked_suites_on_the_exact_supported_matrix() {
    let ci = fs::read_to_string(repository_root().join(".github/workflows/ci.yml"))
        .expect("read CI workflow");
    let build = job_block(&ci, "build");
    let lf_ci = ci.replace("\r\n", "\n");
    let crlf_ci = lf_ci.replace('\n', "\r\n");
    assert_eq!(
        build,
        job_block(&crlf_ci, "build"),
        "workflow policy extraction must be independent of checkout line endings"
    );

    assert!(
        build.contains("os: [ubuntu-latest, windows-latest, macos-latest]"),
        "the behavioral matrix must retain exactly Linux, Windows, and macOS"
    );
    assert!(
        build.contains(
            "cargo test --workspace --all-targets --all-features --locked -- --nocapture"
        ),
        "every matrix host must run the complete locked debug suite with auditable output"
    );
    assert!(
        build.contains(
            "cargo test --workspace --all-targets --all-features --release --locked -- --nocapture"
        ),
        "every matrix host must run the complete locked release suite with auditable output"
    );
    assert!(
        build.contains("GUIDE_FORMAT_REQUIRE_CONFORMANCE: all"),
        "the platform suites must require every operation-ledger owner to conform"
    );
    assert!(
        build.contains(
            "cargo test --workspace --all-targets --all-features --locked\n        trust_evidence -- --nocapture"
        ),
        "every matrix host must visibly rerun the exact guide, output, and containment trust oracles"
    );
}

#[test]
fn issue_55_release_preparation_cannot_bypass_platform_conformance() {
    let ci = fs::read_to_string(repository_root().join(".github/workflows/ci.yml"))
        .expect("read CI workflow");
    let release = job_block(&ci, "release-identity");

    assert!(
        release.lines().any(|line| line == "    needs: build"),
        "prepared release validation must wait for the complete platform matrix"
    );
}

#[test]
fn issue_55_intentional_ignore_allowlist_is_exact_and_documented() {
    let expected = BTreeSet::from([
        "src/filesystem_identity_snapshot_tests.rs::issue_50_release_placeholder_scaling_benchmark::#[ignore = \"manual release benchmark; run with --release --ignored --nocapture --test-threads=1\"]".to_string(),
        "src/parser.rs::benchmark_flat_hierarchy_scaling::#[ignore = \"manual release-mode hierarchy scaling evidence\"]".to_string(),
        "tests/issue_62_package_boundary.rs::issue_62_exact_package_installs_smokes_and_rejects_library_consumers::#[ignore = \"explicit packaged-artifact acceptance test; CI runs it once\"]".to_string(),
    ]);
    assert_eq!(
        ignored_tests(),
        expected,
        "a skipped Rust test must be added to the reviewed issue #55 allowlist with a nonempty reason"
    );

    let contract =
        fs::read_to_string(repository_root().join("docs/v0.2-contract.md")).expect("read contract");
    for name in [
        "benchmark_flat_hierarchy_scaling",
        "issue_50_release_placeholder_scaling_benchmark",
        "issue_62_exact_package_installs_smokes_and_rejects_library_consumers",
    ] {
        assert!(
            contract.contains(name),
            "the supported-platform contract must classify intentional ignore {name:?}"
        );
    }
}

#[test]
fn issue_55_supported_platform_documentation_matches_the_enforced_matrix() {
    let root = repository_root();
    let readme = fs::read_to_string(root.join("README.md")).expect("read README");
    let contract = fs::read_to_string(root.join("docs/v0.2-contract.md")).expect("read contract");

    assert!(
        !readme.contains("gated on the issue #55 filesystem conformance matrix"),
        "README must not retain the pre-conformance support disclaimer"
    );
    assert!(
        readme.contains("complete locked debug and release suites"),
        "README must summarize the realized three-platform support gate"
    );
    assert!(
        contract.contains("### Supported platform and capability matrix"),
        "the normative contract must define the tested platform matrix"
    );
    for required in [
        "`ubuntu-latest`",
        "`macos-latest`",
        "`windows-latest`",
        "case-sensitive or case-insensitive",
        "precomposed and decomposed Unicode",
        "drive-relative",
        "UNC",
        "dangling",
        "permission",
        "transient",
        "capability-unavailable",
    ] {
        assert!(
            contract.contains(required),
            "the supported-platform contract omitted {required:?}"
        );
    }
}

#[test]
fn issue_55_capability_results_are_visible_in_hosted_logs() {
    let root = repository_root();
    let identity = fs::read_to_string(root.join("src/filesystem_identity_snapshot_tests.rs"))
        .expect("read identity tests");
    let output = fs::read_to_string(root.join("src/cli/output.rs")).expect("read output tests");
    let guide = fs::read_to_string(root.join("tests/cli_tests.rs")).expect("read CLI tests");

    for required in [
        "dimension=case identity_mode=",
        "dimension=unicode-normalization identity_mode=",
    ] {
        assert!(
            identity.contains(required),
            "identity capability log omitted {required:?}"
        );
    }
    assert!(
        output.contains("surface=output-trust conformant="),
        "output trust capability result is not visible"
    );
    assert!(
        guide.contains("surface=guide-trust conformant="),
        "guide trust capability result is not visible"
    );
}
