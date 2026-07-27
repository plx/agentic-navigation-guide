use std::fs;
use std::path::PathBuf;

const MSRV: &str = "1.85";
const MSRV_TOOLCHAIN: &str = "1.85.0";
const STABLE_MINUS_ONE: &str = "1.96.1";
const CURRENT_STABLE: &str = "1.97.1";

fn repository_file(path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn normalized_whitespace(contents: &str) -> String {
    contents.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn issue_60_declares_one_enforced_rust_floor() {
    let manifest = repository_file("Cargo.toml");
    let clippy = repository_file(".clippy.toml");
    let cargo_config = repository_file(".cargo/config.toml");
    let ci = repository_file(".github/workflows/ci.yml");

    assert!(
        manifest.contains(&format!("rust-version = \"{MSRV}\"")),
        "Cargo.toml must declare the supported Rust floor"
    );
    assert!(
        clippy.contains(&format!("msrv = \"{MSRV_TOOLCHAIN}\"")),
        ".clippy.toml must use the same Rust floor"
    );
    assert!(
        cargo_config.contains("[resolver]")
            && cargo_config.contains("incompatible-rust-versions = \"fallback\""),
        "Cargo resolution must prefer dependencies compatible with rust-version"
    );

    for required in [
        format!("rust: \"{MSRV_TOOLCHAIN}\""),
        "cargo check --locked --all-targets --all-features".to_owned(),
        "cargo test --locked --all-targets --all-features".to_owned(),
        "cargo clippy --locked --all-targets --all-features -- -D warnings".to_owned(),
        "cargo package --locked".to_owned(),
        "cargo install --path . --locked".to_owned(),
    ] {
        assert!(
            ci.contains(&required),
            "CI must enforce the complete MSRV contract: missing `{required}`"
        );
    }
}

#[test]
fn issue_60_tests_supported_stable_lines_and_informational_beta() {
    let ci = repository_file(".github/workflows/ci.yml");
    let policy = normalized_whitespace(&repository_file("docs/release-policy.md"));

    for toolchain in [STABLE_MINUS_ONE, CURRENT_STABLE] {
        assert!(
            ci.contains(&format!("rust: \"{toolchain}\"")),
            "CI must test supported Rust {toolchain}"
        );
        assert!(
            policy.contains(&format!("Rust `{toolchain}`")),
            "release policy must document supported Rust {toolchain}"
        );
    }

    assert!(
        ci.contains("toolchain: beta") && ci.contains("continue-on-error: true"),
        "beta must be an explicitly informational CI signal"
    );
    assert!(
        policy.contains("Beta is informational"),
        "release policy must document beta as informational"
    );
}

#[test]
fn issue_60_documents_locked_exact_install_and_review_only_updates() {
    let readme = repository_file("README.md");
    let policy = normalized_whitespace(&repository_file("docs/release-policy.md"));
    let dependabot = repository_file(".github/dependabot.yml");

    assert!(
        readme.contains("cargo install agentic-navigation-guide --version 0.2.0 --locked"),
        "README must show a reproducible exact-version install"
    );
    assert!(
        policy.contains("Rust `1.85.0` is the minimum supported toolchain")
            && policy.contains("`--locked`")
            && policy.contains("exact `--version`"),
        "release policy must state the full MSRV and install policy"
    );

    for ecosystem in ["cargo", "github-actions"] {
        assert!(
            dependabot.contains(&format!("package-ecosystem: \"{ecosystem}\"")),
            "Dependabot must cover {ecosystem}"
        );
    }
    assert!(
        dependabot.matches("interval: \"weekly\"").count() == 2,
        "both dependency ecosystems must use a weekly review cadence"
    );
    assert!(
        policy.contains("Dependabot opens review-only pull requests")
            && policy.contains("cannot publish"),
        "automation authority must be explicit"
    );
}
