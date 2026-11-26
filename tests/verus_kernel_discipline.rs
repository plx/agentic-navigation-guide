//! Static guards for the Verus-verified kernels.
//!
//! These checks run in ordinary `cargo test` without Verus. They keep the
//! proofs meaningful for the shipped binary:
//!
//! - ghost-only gating (`cfg(verus_keep_ghost)`) may wrap only proof syntax,
//!   spec imports, and `verus!` blocks, so Verus checks exactly the
//!   executable code that ordinary builds compile;
//! - no source file uses a proof escape that would make a specification an
//!   unchecked assumption; and
//! - every file that carries a specification is reachable from the Verus
//!   root, so no annotated code silently goes unverified.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const GHOST_CFG: &str = "#[cfg(verus_keep_ghost)]";

/// Statements and items that `#[cfg(verus_keep_ghost)]` may introduce.
const GHOST_ONLY_FORMS: &[&str] = &[
    "proof! {",
    "proof_decl! {",
    "verus! {",
    "use vstd::",
    "use crate::",
    "use super::",
];

/// Attributes that ordinary builds drop and Verus interprets.
const GHOST_ATTRIBUTE_PREFIXES: &[&str] = &[
    "#[cfg_attr(verus_keep_ghost, verus_spec",
    "#[cfg_attr(verus_keep_ghost, verus_verify",
    "#[cfg_attr(verus_keep_ghost, verifier::",
];

/// Item-presence gates: the item is compiled unchanged in Verus and in the
/// listed ordinary configurations.
const PRESENCE_GATES: &[&str] = &[
    "#[cfg(any(windows, test, verus_keep_ghost))]",
    "#[cfg(any(unix, test, verus_keep_ghost))]",
];

/// Constructs that turn a specification into an unchecked assumption or
/// waive a termination obligation.
const PROOF_ESCAPES: &[&str] = &[
    "assume(",
    "admit(",
    "external_body",
    "assume_specification",
    "verifier::external",
    "verus_verify(external",
    "exec_allows_no_decreases_clause",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("source directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

fn product_sources() -> Vec<PathBuf> {
    let mut sources = Vec::new();
    rust_sources(&repository_root().join("src"), &mut sources);
    sources
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repository_root())
        .expect("source below the repository root")
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_comment(line: &str) -> bool {
    line.starts_with("//")
}

#[test]
fn ghost_gating_wraps_only_proof_syntax() {
    let mut failures = Vec::new();
    for path in product_sources() {
        let source = read(&path);
        let lines: Vec<&str> = source.lines().map(str::trim).collect();
        for (index, line) in lines.iter().enumerate() {
            if is_comment(line) || !line.contains("verus_keep_ghost") {
                continue;
            }
            let location = format!("{}:{}", relative(&path), index + 1);
            if *line == GHOST_CFG {
                let next = lines[index + 1..]
                    .iter()
                    .find(|candidate| !candidate.is_empty() && !is_comment(candidate))
                    .copied()
                    .unwrap_or_default();
                if !GHOST_ONLY_FORMS.iter().any(|form| next.starts_with(form)) {
                    failures.push(format!(
                        "{location}: {GHOST_CFG} gates `{next}`, which is not proof syntax, \
                         a spec import, or a verus! block"
                    ));
                }
            } else if GHOST_ATTRIBUTE_PREFIXES
                .iter()
                .any(|prefix| line.starts_with(prefix))
                || PRESENCE_GATES.contains(line)
            {
                continue;
            } else {
                failures.push(format!(
                    "{location}: unrecognized use of verus_keep_ghost: `{line}`"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn verified_sources_use_no_proof_escapes() {
    let mut failures = Vec::new();
    for path in product_sources() {
        let source = read(&path);
        for (index, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if is_comment(trimmed) {
                continue;
            }
            for escape in PROOF_ESCAPES {
                if trimmed.contains(escape) {
                    failures.push(format!(
                        "{}:{}: proof escape `{escape}`",
                        relative(&path),
                        index + 1
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_specified_source_is_reachable_from_the_verus_root() {
    let root = repository_root();
    let verus_root = read(&root.join("proofs/verus_root.rs"));
    let proven_mod = read(&root.join("src/proven/mod.rs"));

    let rooted: BTreeSet<String> = verus_root
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("#[path = \"../")
                .and_then(|rest| rest.strip_suffix("\"]"))
                .map(str::to_string)
        })
        .collect();
    assert!(
        rooted.contains("src/proven/mod.rs"),
        "proofs/verus_root.rs must include src/proven/mod.rs"
    );

    let declared: BTreeSet<String> = proven_mod
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("pub(crate) mod ")
                .and_then(|rest| rest.strip_suffix(';'))
                .map(|name| format!("src/proven/{name}.rs"))
        })
        .collect();

    let mut failures = Vec::new();
    for module in &declared {
        if !root.join(module).is_file() {
            failures.push(format!("src/proven/mod.rs declares missing {module}"));
        }
    }

    for path in product_sources() {
        let name = relative(&path);
        let under_proven = name.starts_with("src/proven/");
        if under_proven && name != "src/proven/mod.rs" && !declared.contains(&name) {
            failures.push(format!("{name} is not declared in src/proven/mod.rs"));
        }
        let specified = read(&path).contains("verus_spec(");
        let reachable = rooted.contains(&name) || declared.contains(&name);
        if specified && !reachable {
            failures.push(format!(
                "{name} carries Verus specifications but proofs/verus_root.rs never checks it"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
