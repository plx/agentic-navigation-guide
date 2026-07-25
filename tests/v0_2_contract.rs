use agentic_navigation_guide::{Dumper, FilesystemItem, Parser, Validator, Verifier};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

const ALLOWED_PENDING_OWNERS: &[u32] = &[37, 38, 39, 40, 41, 42, 43, 44, 50];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ItemKind {
    File,
    Directory,
    Placeholder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExpectedItem {
    kind: ItemKind,
    path: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpectedResult {
    Reject,
    Accept {
        ignore: bool,
        items: Option<&'static [ExpectedItem]>,
    },
}

#[derive(Clone, Copy, Debug)]
struct ContractCase {
    id: &'static str,
    source: &'static str,
    normative: ExpectedResult,
    current: ExpectedResult,
    pending_issue: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
enum OperationKind {
    ParseLoneCarriageReturn,
    ParseTabName,
    DumpHashName,
    DumpRegularFile,
    DumpDirectory,
    DumpHardLink,
    DumpEmptyRoot,
    DumpFullyExcludedRoot,
    DumpFileRoot,
    DumpNestedBasenameExclusion,
    DumpInvalidExclusion,
    DumpFileSymlink,
    DumpDirectorySymlink,
    DumpDanglingSymlink,
    DumpSymlinkChain,
    DumpSymlinkLoop,
    VerifyFileSymlink,
    VerifyDirectorySymlink,
    DumpFifo,
    DumpUnixSocket,
    DumpCharacterDevice,
    DumpBlockDevice,
    DumpWindowsJunction,
    VerifyWindowsJunction,
    DumpUnreadableDirectory,
    DumpUnknownEntryType,
    DumpNonUtf8Name,
    VerifyCaseAlias,
    VerifyUnicodeAlias,
    VerifyPlaceholderFirstComponent,
    CliIgnoredDefaultMatrix,
    CliIgnoredDeniedMatrix,
    LibraryIgnored,
    DumpZeroIndent,
    DumpExcessiveDepth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpectedOperationResult {
    Rejected,
    GeneratedInvalid,
    GeneratedPaths(&'static [&'static str]),
    GeneratedItems(&'static [ExpectedItem]),
    Verified,
    CliIgnoredAllowed,
    CliIgnoredAllowedWithRecursiveFalseSuccess,
    CliIgnoredDenied,
    CliOptionUnknown,
    LibraryIgnored,
    CapabilityRejected,
    CapabilityGeneratedPaths(&'static [&'static str]),
    CapabilityGeneratedItems(&'static [ExpectedItem]),
    CapabilityVerified,
    CapabilityUnavailable,
    CapabilityExactIdentityRejected,
    CapabilityLegacyHostIdentity,
}

#[derive(Clone, Copy, Debug)]
struct OperationCase {
    id: &'static str,
    kind: OperationKind,
    normative: ExpectedOperationResult,
    current: ExpectedOperationResult,
    pending_issue: Option<u32>,
}

mod fixtures {
    use super::{ContractCase, ExpectedItem, ExpectedResult, ItemKind};

    include!("fixtures/v0_2_contract.rs");
}

mod operation_fixtures {
    use super::{ExpectedItem, ExpectedOperationResult, ItemKind, OperationCase, OperationKind};

    include!("fixtures/v0_2_operations.rs");
}

#[derive(Debug, PartialEq, Eq)]
struct ObservedItem {
    kind: ItemKind,
    path: String,
}

#[derive(Debug, PartialEq, Eq)]
enum ObservedResult {
    Reject,
    Accept {
        ignore: bool,
        items: Vec<ObservedItem>,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum ObservedOperationResult {
    Rejected,
    GeneratedInvalid,
    GeneratedPaths(Vec<String>),
    GeneratedItems(Vec<ObservedItem>),
    Verified,
    CliIgnoredAllowed,
    CliIgnoredAllowedWithRecursiveFalseSuccess,
    CliIgnoredDenied,
    CliOptionUnknown,
    // Constructed when #36/#39 expose an observable ignored library result.
    #[allow(dead_code)]
    LibraryIgnored,
    LibrarySuccess,
    CapabilityUnavailable,
    Identity {
        host_aliases: bool,
        verified: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IgnoredCliMode {
    Check,
    Verify,
    Recursive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IgnoredCliObservation {
    Allowed,
    Denied,
    OptionUnknown,
    RecursiveFalseSuccess,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnixSymlinkFixture {
    File,
    Directory,
    Dangling,
    Chain,
    Loop,
}

fn observe(source: &str) -> ObservedResult {
    let guide = match Parser::new().parse(source) {
        Ok(guide) => guide,
        Err(_) => return ObservedResult::Reject,
    };

    if !guide.ignore && Validator::new().validate_syntax(&guide).is_err() {
        return ObservedResult::Reject;
    }

    let mut items = Vec::new();
    flatten_items(&guide.items, "", &mut items);

    ObservedResult::Accept {
        ignore: guide.ignore,
        items,
    }
}

fn flatten_items(
    source: &[agentic_navigation_guide::NavigationGuideLine],
    parent: &str,
    output: &mut Vec<ObservedItem>,
) {
    for line in source {
        match &line.item {
            FilesystemItem::File { path, .. } => output.push(ObservedItem {
                kind: ItemKind::File,
                path: join_path(parent, path),
            }),
            FilesystemItem::Directory { path, children, .. } => {
                let full_path = join_path(parent, path);
                output.push(ObservedItem {
                    kind: ItemKind::Directory,
                    path: full_path.clone(),
                });
                flatten_items(children, &full_path, output);
            }
            FilesystemItem::Placeholder { .. } => output.push(ObservedItem {
                kind: ItemKind::Placeholder,
                path: join_path(parent, "..."),
            }),
            FilesystemItem::Symlink { path, .. } => {
                panic!("the v0.2 parser fixture must not construct a symlink for '{path}'")
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

fn matches_expected(observed: &ObservedResult, expected: ExpectedResult) -> bool {
    match (observed, expected) {
        (ObservedResult::Reject, ExpectedResult::Reject) => true,
        (
            ObservedResult::Accept { ignore, .. },
            ExpectedResult::Accept {
                ignore: expected_ignore,
                items: None,
            },
        ) => *ignore == expected_ignore,
        (
            ObservedResult::Accept { ignore, items },
            ExpectedResult::Accept {
                ignore: expected_ignore,
                items: Some(expected_items),
            },
        ) => {
            *ignore == expected_ignore
                && items.len() == expected_items.len()
                && items.iter().zip(expected_items).all(|(actual, expected)| {
                    actual.kind == expected.kind && actual.path == expected.path
                })
        }
        _ => false,
    }
}

fn requested_conformance_owner() -> Option<String> {
    std::env::var("GUIDE_FORMAT_REQUIRE_CONFORMANCE")
        .ok()
        .filter(|value| !value.is_empty())
}

#[test]
fn contract_cases() {
    let requested = requested_conformance_owner();

    if let Some(owner) = requested.as_deref().filter(|owner| *owner != "all") {
        let issue = owner
            .parse::<u32>()
            .unwrap_or_else(|_| panic!("invalid conformance owner '{owner}'"));
        assert!(
            ALLOWED_PENDING_OWNERS.contains(&issue),
            "conformance owner #{issue} is not in the contract handoff"
        );
    }

    let mut pending_rows = 0;

    for case in fixtures::CASES {
        if case.pending_issue.is_some() {
            pending_rows += 1;
        }

        let require_normative = match requested.as_deref() {
            None => false,
            Some("all") => true,
            Some(owner) => case
                .pending_issue
                .is_some_and(|issue| issue.to_string() == owner),
        };

        let expected = if require_normative {
            case.normative
        } else {
            case.current
        };
        let observed = observe(case.source);

        assert!(
            matches_expected(&observed, expected),
            "contract case '{}' differed\nexpected: {:?}\nobserved: {:?}",
            case.id,
            expected,
            observed
        );

        match case.pending_issue {
            Some(issue) => {
                assert_ne!(
                    case.normative, case.current,
                    "pending case '{}' for #{} does not record a real divergence",
                    case.id, issue
                );
            }
            None => assert_eq!(
                case.normative, case.current,
                "conforming case '{}' has inconsistent ledger outcomes",
                case.id
            ),
        }
    }

    if requested.as_deref() == Some("all") {
        assert_eq!(
            pending_rows, 0,
            "all-mode requires every pending contract row to be activated"
        );
    }
}

#[test]
fn operation_cases() {
    let requested = requested_conformance_owner();
    let mut pending_rows = 0;

    for case in operation_fixtures::CASES {
        if case.pending_issue.is_some() {
            pending_rows += 1;
        }

        let require_normative = match requested.as_deref() {
            None => false,
            Some("all") => true,
            Some(owner) => case
                .pending_issue
                .is_some_and(|issue| issue.to_string() == owner),
        };
        let expected = if require_normative {
            case.normative
        } else {
            case.current
        };
        let observed = run_operation(case.kind);

        assert!(
            matches_expected_operation(&observed, expected),
            "operation case '{}' differed\nexpected: {:?}\nobserved: {:?}",
            case.id,
            expected,
            observed
        );

        match case.pending_issue {
            Some(issue) => assert_ne!(
                case.normative, case.current,
                "pending operation '{}' for #{} does not record a divergence",
                case.id, issue
            ),
            None => assert_eq!(
                case.normative, case.current,
                "conforming operation '{}' has inconsistent outcomes",
                case.id
            ),
        }
    }

    if requested.as_deref() == Some("all") {
        assert_eq!(
            pending_rows, 0,
            "all-mode requires every pending operation row to be activated"
        );
    }
}

#[test]
fn documentation_and_fixture_are_a_bijection() {
    let documented = extract_documented_cases(include_str!("../docs/v0.2-contract.md"));
    let fixture_ids: BTreeSet<_> = fixtures::CASES.iter().map(|case| case.id).collect();
    let documented_ids: BTreeSet<_> = documented.keys().map(String::as_str).collect();

    assert_eq!(
        fixture_ids.len(),
        fixtures::CASES.len(),
        "fixture case IDs must be unique"
    );
    assert_eq!(
        fixture_ids, documented_ids,
        "the normative document and fixture must contain the same case IDs"
    );

    for case in fixtures::CASES {
        assert_eq!(
            documented.get(case.id).map(String::as_str),
            Some(case.source),
            "documented source for '{}' drifted from its fixture",
            case.id
        );
    }

    let operation_ids: BTreeSet<_> = operation_fixtures::CASES
        .iter()
        .map(|case| case.id)
        .collect();
    assert_eq!(
        operation_ids.len(),
        operation_fixtures::CASES.len(),
        "operation fixture IDs must be unique"
    );
    for id in operation_ids {
        let documented_id = format!("`{id}`");
        assert_eq!(
            include_str!("../docs/v0.2-contract.md")
                .matches(&documented_id)
                .count(),
            1,
            "operation fixture '{id}' must be documented exactly once"
        );
    }
}

#[test]
fn pending_rows_have_one_allowed_owner_and_no_tbd_policy() {
    for case in fixtures::CASES {
        for (label, result) in [("normative", case.normative), ("current", case.current)] {
            if let ExpectedResult::Accept { items, .. } = result {
                assert!(
                    items.is_some(),
                    "{label} result for '{}' must assert an exact item list",
                    case.id
                );
            }
        }

        if let Some(owner) = case.pending_issue {
            assert!(
                ALLOWED_PENDING_OWNERS.contains(&owner),
                "case '{}' has unexpected owner #{}",
                case.id,
                owner
            );
            assert!(
                include_str!("../docs/v0.2-contract.md").contains(&format!("#{}", owner)),
                "the contract does not include handoff text for #{}",
                owner
            );
        }
    }

    for case in operation_fixtures::CASES {
        if let Some(owner) = case.pending_issue {
            assert!(
                ALLOWED_PENDING_OWNERS.contains(&owner),
                "operation '{}' has unexpected owner #{}",
                case.id,
                owner
            );
            assert!(
                include_str!("../docs/v0.2-contract.md").contains(&format!("#{}", owner)),
                "the contract does not include handoff text for #{}",
                owner
            );
        }
    }

    assert!(
        !include_str!("../docs/v0.2-contract.md")
            .to_ascii_lowercase()
            .contains("tbd"),
        "the normative contract must not retain TBD policy"
    );
}

#[test]
fn comments_and_choice_comment_inheritance_are_executable() {
    let path_case = fixture("path-comment-escaped-hash");
    assert_eq!(
        comment_snapshot(path_case.source),
        vec![(
            "docs/issue#123.md".to_string(),
            Some("ticket #123".to_string())
        )]
    );

    let choice_case = fixture("choice-escaped-hash-comment");
    assert_eq!(
        comment_snapshot(choice_case.source),
        vec![
            ("xa#by".to_string(), Some("inherited".to_string())),
            ("xcy".to_string(), Some("inherited".to_string())),
        ]
    );

    let placeholder_case = fixture("placeholder-forms");
    assert_eq!(
        comment_snapshot(placeholder_case.source),
        vec![
            ("src".to_string(), None),
            ("src/...".to_string(), None),
            ("src/main.rs".to_string(), None),
            ("src/...".to_string(), Some("future modules".to_string())),
        ]
    );
}

#[test]
fn generated_depth_boundary_is_executable() {
    const MAX_LOGICAL_DEPTH: usize = 256;

    let at_limit = nested_directory_source(MAX_LOGICAL_DEPTH);
    assert_exact_nested_tree(&observe(&at_limit), MAX_LOGICAL_DEPTH);

    let over_limit = nested_directory_source(MAX_LOGICAL_DEPTH + 1);
    let observed = observe(&over_limit);
    if requires_normative_owner(37) {
        assert!(
            matches!(observed, ObservedResult::Reject),
            "logical depth above {MAX_LOGICAL_DEPTH} must be rejected"
        );
    } else {
        assert_exact_nested_tree(&observed, MAX_LOGICAL_DEPTH + 1);
    }
}

#[test]
fn generated_choice_count_boundary_is_executable() {
    const MAX_CHOICE_ALTERNATIVES: usize = 256;

    let at_limit = choice_count_source(MAX_CHOICE_ALTERNATIVES);
    assert_exact_choice_expansion(&observe(&at_limit), MAX_CHOICE_ALTERNATIVES);

    let over_limit = choice_count_source(MAX_CHOICE_ALTERNATIVES + 1);
    let observed = observe(&over_limit);
    if requires_normative_owner(40) {
        assert!(
            matches!(observed, ObservedResult::Reject),
            "more than {MAX_CHOICE_ALTERNATIVES} alternatives must be rejected"
        );
    } else {
        assert_exact_choice_expansion(&observed, MAX_CHOICE_ALTERNATIVES + 1);
    }
}

#[test]
fn marker_line_endings_are_platform_independent() {
    let source = fixture("marker-bare").source;
    let crlf = source.replace('\n', "\r\n");

    assert_eq!(observe(source), observe(&crlf));
}

#[test]
fn marker_outer_horizontal_whitespace_is_insignificant() {
    let source = "\t<agentic-navigation-guide>\t\n- Cargo.toml\n\t</agentic-navigation-guide>\t";

    assert!(matches!(observe(source), ObservedResult::Accept { .. }));
}

#[test]
fn byte_order_mark_is_not_silently_discarded() {
    let source = "\u{feff}<agentic-navigation-guide>\n- Cargo.toml\n</agentic-navigation-guide>";

    assert_eq!(observe(source), ObservedResult::Reject);
}

fn matches_expected_operation(
    observed: &ObservedOperationResult,
    expected: ExpectedOperationResult,
) -> bool {
    match (observed, expected) {
        (ObservedOperationResult::Rejected, ExpectedOperationResult::Rejected)
        | (ObservedOperationResult::GeneratedInvalid, ExpectedOperationResult::GeneratedInvalid)
        | (ObservedOperationResult::Verified, ExpectedOperationResult::Verified)
        | (
            ObservedOperationResult::CliIgnoredAllowed,
            ExpectedOperationResult::CliIgnoredAllowed,
        )
        | (
            ObservedOperationResult::CliIgnoredAllowedWithRecursiveFalseSuccess,
            ExpectedOperationResult::CliIgnoredAllowedWithRecursiveFalseSuccess,
        )
        | (ObservedOperationResult::CliIgnoredDenied, ExpectedOperationResult::CliIgnoredDenied)
        | (ObservedOperationResult::CliOptionUnknown, ExpectedOperationResult::CliOptionUnknown)
        | (ObservedOperationResult::LibraryIgnored, ExpectedOperationResult::LibraryIgnored)
        | (ObservedOperationResult::Rejected, ExpectedOperationResult::CapabilityRejected)
        | (ObservedOperationResult::Verified, ExpectedOperationResult::CapabilityVerified)
        | (
            ObservedOperationResult::CapabilityUnavailable,
            ExpectedOperationResult::CapabilityUnavailable,
        )
        | (
            ObservedOperationResult::CapabilityUnavailable,
            ExpectedOperationResult::CapabilityRejected
            | ExpectedOperationResult::CapabilityGeneratedPaths(_)
            | ExpectedOperationResult::CapabilityGeneratedItems(_)
            | ExpectedOperationResult::CapabilityVerified
            | ExpectedOperationResult::CapabilityExactIdentityRejected
            | ExpectedOperationResult::CapabilityLegacyHostIdentity,
        )
        | (
            ObservedOperationResult::Identity {
                host_aliases: true,
                verified: false,
            },
            ExpectedOperationResult::CapabilityExactIdentityRejected,
        ) => true,
        (
            ObservedOperationResult::GeneratedPaths(actual),
            ExpectedOperationResult::GeneratedPaths(expected),
        ) => actual
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied()),
        (
            ObservedOperationResult::GeneratedItems(actual),
            ExpectedOperationResult::GeneratedPaths(expected),
        ) => actual
            .iter()
            .map(|item| item.path.as_str())
            .eq(expected.iter().copied()),
        (
            ObservedOperationResult::GeneratedItems(actual),
            ExpectedOperationResult::GeneratedItems(expected),
        ) => exact_items_match(actual, expected),
        (
            ObservedOperationResult::GeneratedPaths(actual),
            ExpectedOperationResult::CapabilityGeneratedPaths(expected),
        ) => actual
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied()),
        (
            ObservedOperationResult::GeneratedItems(actual),
            ExpectedOperationResult::CapabilityGeneratedPaths(expected),
        ) => actual
            .iter()
            .map(|item| item.path.as_str())
            .eq(expected.iter().copied()),
        (
            ObservedOperationResult::GeneratedItems(actual),
            ExpectedOperationResult::CapabilityGeneratedItems(expected),
        ) => exact_items_match(actual, expected),
        (
            ObservedOperationResult::Identity {
                host_aliases: true,
                verified: true,
            },
            ExpectedOperationResult::CapabilityLegacyHostIdentity,
        ) => true,
        _ => false,
    }
}

fn exact_items_match(actual: &[ObservedItem], expected: &[ExpectedItem]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.kind == expected.kind && actual.path == expected.path)
}

fn run_operation(kind: OperationKind) -> ObservedOperationResult {
    match kind {
        OperationKind::ParseLoneCarriageReturn => observe_source_operation(
            "<agentic-navigation-guide>\n- file.txt # before\rafter\n</agentic-navigation-guide>",
        ),
        OperationKind::ParseTabName => observe_source_operation(
            "<agentic-navigation-guide>\n- tab\tname.txt\n</agentic-navigation-guide>",
        ),
        OperationKind::DumpHashName => {
            let temp = TempDir::new().expect("temporary dump root");
            fs::write(temp.path().join("report"), "").expect("report fixture");
            fs::write(temp.path().join("report#draft"), "").expect("hash fixture");
            observe_generated(&Dumper::new(temp.path()))
        }
        OperationKind::DumpRegularFile => {
            let temp = TempDir::new().expect("temporary regular-file root");
            fs::write(temp.path().join("file.txt"), "").expect("regular-file fixture");
            observe_generated(&Dumper::new(temp.path()))
        }
        OperationKind::DumpDirectory => {
            let temp = TempDir::new().expect("temporary directory root");
            fs::create_dir(temp.path().join("directory")).expect("directory fixture");
            observe_generated(&Dumper::new(temp.path()))
        }
        OperationKind::DumpHardLink => {
            let temp = TempDir::new().expect("temporary hard-link root");
            fs::write(temp.path().join("first.txt"), "").expect("hard-link source");
            fs::hard_link(
                temp.path().join("first.txt"),
                temp.path().join("second.txt"),
            )
            .expect("hard-link fixture");
            observe_generated(&Dumper::new(temp.path()))
        }
        OperationKind::DumpEmptyRoot => {
            let temp = TempDir::new().expect("temporary empty root");
            observe_generated(&Dumper::new(temp.path()))
        }
        OperationKind::DumpFullyExcludedRoot => {
            let temp = TempDir::new().expect("temporary excluded root");
            fs::write(temp.path().join("only.txt"), "").expect("excluded fixture");
            let patterns = vec!["only.txt".to_string()];
            let dumper = Dumper::new(temp.path())
                .with_exclude_patterns(&patterns)
                .expect("valid exclusion");
            observe_generated(&dumper)
        }
        OperationKind::DumpFileRoot => {
            let temp = TempDir::new().expect("temporary file root parent");
            let root = temp.path().join("root.txt");
            fs::write(&root, "").expect("file root");
            observe_generated(&Dumper::new(&root))
        }
        OperationKind::DumpNestedBasenameExclusion => {
            let temp = TempDir::new().expect("temporary nested exclusion root");
            fs::create_dir_all(temp.path().join("project/target"))
                .expect("nested target directory");
            fs::write(temp.path().join("project/keep.txt"), "").expect("kept fixture");
            fs::write(temp.path().join("project/target/generated.txt"), "")
                .expect("excluded nested fixture");
            let patterns = vec!["target".to_string()];
            let dumper = Dumper::new(temp.path())
                .with_exclude_patterns(&patterns)
                .expect("valid basename exclusion");
            observe_generated(&dumper)
        }
        OperationKind::DumpInvalidExclusion => {
            let temp = TempDir::new().expect("temporary invalid exclusion root");
            fs::write(temp.path().join("keep.txt"), "").expect("kept fixture");
            let patterns = vec!["a/**b".to_string()];
            match Dumper::new(temp.path()).with_exclude_patterns(&patterns) {
                Ok(dumper) => observe_generated(&dumper),
                Err(_) => ObservedOperationResult::Rejected,
            }
        }
        OperationKind::DumpFileSymlink => observe_unix_symlink_dump(UnixSymlinkFixture::File),
        OperationKind::DumpDirectorySymlink => {
            observe_unix_symlink_dump(UnixSymlinkFixture::Directory)
        }
        OperationKind::DumpDanglingSymlink => {
            observe_unix_symlink_dump(UnixSymlinkFixture::Dangling)
        }
        OperationKind::DumpSymlinkChain => observe_unix_symlink_dump(UnixSymlinkFixture::Chain),
        OperationKind::DumpSymlinkLoop => observe_unix_symlink_dump(UnixSymlinkFixture::Loop),
        OperationKind::VerifyFileSymlink => {
            observe_unix_symlink_verification(UnixSymlinkFixture::File)
        }
        OperationKind::VerifyDirectorySymlink => {
            observe_unix_symlink_verification(UnixSymlinkFixture::Directory)
        }
        OperationKind::DumpFifo => observe_fifo_dump(),
        OperationKind::DumpUnixSocket => observe_unix_socket_dump(),
        OperationKind::DumpCharacterDevice => observe_unix_device_dump("c", "character-device"),
        OperationKind::DumpBlockDevice => observe_unix_device_dump("b", "block-device"),
        OperationKind::DumpWindowsJunction => observe_windows_junction(false),
        OperationKind::VerifyWindowsJunction => observe_windows_junction(true),
        OperationKind::DumpUnreadableDirectory => observe_unreadable_directory_dump(),
        OperationKind::DumpUnknownEntryType => observe_unknown_entry_type(),
        OperationKind::DumpNonUtf8Name => observe_non_utf8_name_dump(),
        OperationKind::VerifyCaseAlias => observe_case_alias_verification(),
        OperationKind::VerifyUnicodeAlias => observe_unicode_alias_verification(),
        OperationKind::VerifyPlaceholderFirstComponent => {
            let temp = TempDir::new().expect("temporary placeholder root");
            fs::create_dir(temp.path().join("src")).expect("src directory");
            fs::write(temp.path().join("src/main.rs"), "").expect("source fixture");
            let source =
                "<agentic-navigation-guide>\n- src/main.rs\n- ...\n</agentic-navigation-guide>";
            let guide = Parser::new().parse(source).expect("placeholder guide");
            match Verifier::new(temp.path()).verify(&guide) {
                Ok(()) => ObservedOperationResult::Verified,
                Err(_) => ObservedOperationResult::Rejected,
            }
        }
        OperationKind::CliIgnoredDefaultMatrix => observe_ignored_cli_matrix(false),
        OperationKind::CliIgnoredDeniedMatrix => observe_ignored_cli_matrix(true),
        OperationKind::LibraryIgnored => {
            let temp = TempDir::new().expect("temporary ignored library root");
            let source = "<agentic-navigation-guide ignore=true>\n- missing.txt\n</agentic-navigation-guide>";
            let guide = Parser::new().parse(source).expect("ignored library guide");
            match agentic_navigation_guide::verify_guide(&guide, temp.path()) {
                Ok(()) => ObservedOperationResult::LibrarySuccess,
                Err(_) => ObservedOperationResult::Rejected,
            }
        }
        OperationKind::DumpZeroIndent => observe_dump_cli_number("--indent", "0", true),
        OperationKind::DumpExcessiveDepth => observe_dump_cli_number("--depth", "257", false),
    }
}

fn observe_source_operation(source: &str) -> ObservedOperationResult {
    match observe(source) {
        ObservedResult::Reject => ObservedOperationResult::Rejected,
        ObservedResult::Accept {
            ignore: false,
            items,
        } => ObservedOperationResult::GeneratedItems(items),
        ObservedResult::Accept { ignore: true, .. } => ObservedOperationResult::GeneratedInvalid,
    }
}

fn observe_generated(dumper: &Dumper) -> ObservedOperationResult {
    let source = match dumper.dump_with_wrapper() {
        Ok(source) => source,
        Err(_) => return ObservedOperationResult::Rejected,
    };

    match observe_source_operation(&source) {
        ObservedOperationResult::Rejected => ObservedOperationResult::GeneratedInvalid,
        result => result,
    }
}

#[cfg(unix)]
fn observe_unix_symlink_dump(fixture: UnixSymlinkFixture) -> ObservedOperationResult {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temporary symlink fixture");
    let root = temp.path().join("root");
    fs::create_dir(&root).expect("symlink dump root");

    match fixture {
        UnixSymlinkFixture::File => {
            fs::write(temp.path().join("target"), "").expect("file symlink target");
            symlink("../target", root.join("link")).expect("file symlink");
        }
        UnixSymlinkFixture::Directory => {
            fs::create_dir(temp.path().join("target")).expect("directory symlink target");
            symlink("../target", root.join("link")).expect("directory symlink");
        }
        UnixSymlinkFixture::Dangling => {
            symlink("missing-target", root.join("link")).expect("dangling symlink");
        }
        UnixSymlinkFixture::Chain => {
            fs::write(temp.path().join("target"), "").expect("chain target");
            symlink("second", root.join("first")).expect("first chain link");
            symlink("../target", root.join("second")).expect("second chain link");
        }
        UnixSymlinkFixture::Loop => {
            symlink("second", root.join("first")).expect("first loop link");
            symlink("first", root.join("second")).expect("second loop link");
        }
    }

    observe_generated(&Dumper::new(&root))
}

#[cfg(not(unix))]
fn observe_unix_symlink_dump(_fixture: UnixSymlinkFixture) -> ObservedOperationResult {
    ObservedOperationResult::CapabilityUnavailable
}

#[cfg(unix)]
fn observe_unix_symlink_verification(fixture: UnixSymlinkFixture) -> ObservedOperationResult {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temporary symlink verification root");
    let (source, target) = match fixture {
        UnixSymlinkFixture::File => {
            fs::write(temp.path().join("target"), "").expect("file symlink target");
            (
                "<agentic-navigation-guide>\n- link\n</agentic-navigation-guide>",
                "target",
            )
        }
        UnixSymlinkFixture::Directory => {
            fs::create_dir(temp.path().join("target")).expect("directory symlink target");
            (
                "<agentic-navigation-guide>\n- link/\n</agentic-navigation-guide>",
                "target",
            )
        }
        _ => panic!("unsupported verification symlink fixture: {fixture:?}"),
    };
    symlink(target, temp.path().join("link")).expect("verification symlink");

    let guide = Parser::new()
        .parse(source)
        .expect("symlink verification guide");
    match Verifier::new(temp.path()).verify(&guide) {
        Ok(()) => ObservedOperationResult::Verified,
        Err(_) => ObservedOperationResult::Rejected,
    }
}

#[cfg(not(unix))]
fn observe_unix_symlink_verification(_fixture: UnixSymlinkFixture) -> ObservedOperationResult {
    ObservedOperationResult::CapabilityUnavailable
}

#[cfg(unix)]
fn observe_fifo_dump() -> ObservedOperationResult {
    let temp = TempDir::new().expect("temporary FIFO root");
    let output = Command::new("mkfifo")
        .arg(temp.path().join("pipe"))
        .output()
        .expect("execute mkfifo");
    if !output.status.success() {
        return ObservedOperationResult::CapabilityUnavailable;
    }
    observe_generated(&Dumper::new(temp.path()))
}

#[cfg(not(unix))]
fn observe_fifo_dump() -> ObservedOperationResult {
    ObservedOperationResult::CapabilityUnavailable
}

#[cfg(unix)]
fn observe_unix_socket_dump() -> ObservedOperationResult {
    use std::os::unix::net::UnixListener;

    let temp = TempDir::new().expect("temporary Unix-socket root");
    let _listener =
        UnixListener::bind(temp.path().join("socket")).expect("Unix-domain socket fixture");
    observe_generated(&Dumper::new(temp.path()))
}

#[cfg(not(unix))]
fn observe_unix_socket_dump() -> ObservedOperationResult {
    ObservedOperationResult::CapabilityUnavailable
}

#[cfg(unix)]
fn observe_unix_device_dump(device_kind: &str, name: &str) -> ObservedOperationResult {
    let temp = TempDir::new().expect("temporary device-node root");
    let output = Command::new("mknod")
        .arg(temp.path().join(name))
        .arg(device_kind)
        .arg("1")
        .arg("3")
        .output()
        .expect("execute mknod");
    if !output.status.success() {
        return ObservedOperationResult::CapabilityUnavailable;
    }
    observe_generated(&Dumper::new(temp.path()))
}

#[cfg(not(unix))]
fn observe_unix_device_dump(_device_kind: &str, _name: &str) -> ObservedOperationResult {
    ObservedOperationResult::CapabilityUnavailable
}

#[cfg(windows)]
fn observe_windows_junction(verify: bool) -> ObservedOperationResult {
    let temp = TempDir::new().expect("temporary Windows-junction fixture");
    let root = temp.path().join("root");
    let junction = root.join("junction");
    fs::create_dir(&root).expect("junction root");
    let target = if verify {
        root.join("target")
    } else {
        temp.path().join("target")
    };
    fs::create_dir(&target).expect("junction target");

    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .expect("execute mklink");
    if !output.status.success() {
        return ObservedOperationResult::CapabilityUnavailable;
    }

    if verify {
        let source = "<agentic-navigation-guide>\n- junction/\n</agentic-navigation-guide>";
        let guide = Parser::new()
            .parse(source)
            .expect("junction verification guide");
        match Verifier::new(&root).verify(&guide) {
            Ok(()) => ObservedOperationResult::Verified,
            Err(_) => ObservedOperationResult::Rejected,
        }
    } else {
        observe_generated(&Dumper::new(&root))
    }
}

#[cfg(not(windows))]
fn observe_windows_junction(_verify: bool) -> ObservedOperationResult {
    ObservedOperationResult::CapabilityUnavailable
}

#[cfg(unix)]
fn observe_unreadable_directory_dump() -> ObservedOperationResult {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().expect("temporary unreadable-directory root");
    let directory = temp.path().join("unreadable");
    fs::create_dir(&directory).expect("unreadable directory fixture");
    fs::write(directory.join("file.txt"), "").expect("unreadable child fixture");

    let original_permissions = fs::metadata(&directory)
        .expect("unreadable directory metadata")
        .permissions();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o0))
        .expect("remove directory permissions");

    if fs::read_dir(&directory).is_ok() {
        fs::set_permissions(&directory, original_permissions)
            .expect("restore readable directory permissions");
        return ObservedOperationResult::CapabilityUnavailable;
    }

    let observed = observe_generated(&Dumper::new(temp.path()));
    fs::set_permissions(&directory, original_permissions)
        .expect("restore readable directory permissions");
    observed
}

#[cfg(not(unix))]
fn observe_unreadable_directory_dump() -> ObservedOperationResult {
    ObservedOperationResult::CapabilityUnavailable
}

fn observe_unknown_entry_type() -> ObservedOperationResult {
    // Host filesystem APIs do not provide a portable way to construct an
    // entry whose classification is unknown. Issue #42 must replace this
    // sentinel with an injected classifier observation before its owner gate
    // can pass.
    ObservedOperationResult::CapabilityUnavailable
}

#[cfg(unix)]
fn observe_non_utf8_name_dump() -> ObservedOperationResult {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let temp = TempDir::new().expect("temporary non-UTF-8 root");
    let name = OsStr::from_bytes(b"bad-\xFF-name");
    if fs::write(temp.path().join(name), "").is_err() {
        return ObservedOperationResult::CapabilityUnavailable;
    }
    observe_generated(&Dumper::new(temp.path()))
}

#[cfg(not(unix))]
fn observe_non_utf8_name_dump() -> ObservedOperationResult {
    ObservedOperationResult::CapabilityUnavailable
}

fn observe_case_alias_verification() -> ObservedOperationResult {
    observe_identity_alias("Readme.md", "README.md")
}

fn observe_unicode_alias_verification() -> ObservedOperationResult {
    const PRECOMPOSED: &str = "\u{e9}.txt";
    const DECOMPOSED: &str = "e\u{301}.txt";

    let temp = TempDir::new().expect("temporary Unicode-identity root");
    fs::write(temp.path().join(PRECOMPOSED), "").expect("Unicode identity fixture");
    let enumerated = fs::read_dir(temp.path())
        .expect("enumerate Unicode identity root")
        .next()
        .expect("Unicode identity entry")
        .expect("read Unicode identity entry")
        .file_name()
        .into_string()
        .expect("UTF-8 Unicode identity entry");
    let alias = if enumerated == PRECOMPOSED {
        DECOMPOSED
    } else {
        PRECOMPOSED
    };
    observe_identity_alias_in(temp, alias)
}

fn observe_identity_alias(actual: &str, alias: &str) -> ObservedOperationResult {
    let temp = TempDir::new().expect("temporary identity root");
    fs::write(temp.path().join(actual), "").expect("identity fixture");
    observe_identity_alias_in(temp, alias)
}

fn observe_identity_alias_in(temp: TempDir, alias: &str) -> ObservedOperationResult {
    let host_aliases = temp.path().join(alias).is_file();
    if !host_aliases {
        return ObservedOperationResult::CapabilityUnavailable;
    }

    let source = format!("<agentic-navigation-guide>\n- {alias}\n</agentic-navigation-guide>");
    let guide = Parser::new().parse(&source).expect("identity guide");
    let verified = Verifier::new(temp.path()).verify(&guide).is_ok();
    ObservedOperationResult::Identity {
        host_aliases,
        verified,
    }
}

fn observe_ignored_cli_matrix(deny_ignored: bool) -> ObservedOperationResult {
    let temp = TempDir::new().expect("temporary ignored guide root");
    let guide_path = temp.path().join("AGENTIC_NAVIGATION_GUIDE.md");
    fs::write(
        &guide_path,
        "<agentic-navigation-guide ignore=true>\n- example.txt\n</agentic-navigation-guide>",
    )
    .expect("ignored guide");

    let mut observations = Vec::new();
    for (mode, arguments) in [
        (
            IgnoredCliMode::Check,
            vec![
                "check".to_string(),
                "--guide".to_string(),
                guide_path.display().to_string(),
            ],
        ),
        (
            IgnoredCliMode::Verify,
            vec![
                "verify".to_string(),
                "--guide".to_string(),
                guide_path.display().to_string(),
                "--root".to_string(),
                temp.path().display().to_string(),
            ],
        ),
        (
            IgnoredCliMode::Recursive,
            vec![
                "verify".to_string(),
                "--recursive".to_string(),
                "--root".to_string(),
                temp.path().display().to_string(),
                "--guide-name".to_string(),
                "AGENTIC_NAVIGATION_GUIDE.md".to_string(),
            ],
        ),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agentic-navigation-guide"));
        command.args(arguments);
        if deny_ignored {
            command.arg("--deny-ignored");
        }
        let output = command.output().expect("run ignored command");
        observations.push(classify_ignored_cli_output(&output, deny_ignored, mode));
    }

    match observations.as_slice() {
        [IgnoredCliObservation::Allowed, IgnoredCliObservation::Allowed, IgnoredCliObservation::Allowed] => {
            ObservedOperationResult::CliIgnoredAllowed
        }
        [IgnoredCliObservation::Allowed, IgnoredCliObservation::Allowed, IgnoredCliObservation::RecursiveFalseSuccess] => {
            ObservedOperationResult::CliIgnoredAllowedWithRecursiveFalseSuccess
        }
        [IgnoredCliObservation::Denied, IgnoredCliObservation::Denied, IgnoredCliObservation::Denied] => {
            ObservedOperationResult::CliIgnoredDenied
        }
        [IgnoredCliObservation::OptionUnknown, IgnoredCliObservation::OptionUnknown, IgnoredCliObservation::OptionUnknown] => {
            ObservedOperationResult::CliOptionUnknown
        }
        _ => ObservedOperationResult::Rejected,
    }
}

fn classify_ignored_cli_output(
    output: &std::process::Output,
    deny_ignored: bool,
    mode: IgnoredCliMode,
) -> IgnoredCliObservation {
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lowercase = diagnostics.to_ascii_lowercase();
    let mentions_ignored = lowercase.contains("ignore");
    let recursive_counts_are_exact = lowercase.contains("total: 1")
        && lowercase.contains("passed: 0")
        && lowercase.contains("ignored: 1");
    let claims_positive_success = lowercase.contains("syntax is valid")
        || lowercase.contains(": verified")
        || lowercase.contains("navigation guide verified")
        || lowercase.contains("navigation guide is valid and matches filesystem")
        || lowercase.contains("all navigation guides verified")
        || lowercase.contains("all navigation guides are valid and match filesystem");

    if output.status.success() {
        if !mentions_ignored {
            return IgnoredCliObservation::Rejected;
        }
        if mode == IgnoredCliMode::Recursive {
            if claims_positive_success {
                return IgnoredCliObservation::RecursiveFalseSuccess;
            }
            if !recursive_counts_are_exact {
                return IgnoredCliObservation::Rejected;
            }
        } else if claims_positive_success {
            return IgnoredCliObservation::Rejected;
        }
        IgnoredCliObservation::Allowed
    } else if deny_ignored
        && lowercase.contains("unexpected argument")
        && lowercase.contains("deny-ignored")
    {
        IgnoredCliObservation::OptionUnknown
    } else if deny_ignored && mentions_ignored {
        if mode == IgnoredCliMode::Recursive && !lowercase.contains("ignored: 1") {
            return IgnoredCliObservation::Rejected;
        }
        IgnoredCliObservation::Denied
    } else {
        IgnoredCliObservation::Rejected
    }
}

fn observe_dump_cli_number(option: &str, value: &str, nested: bool) -> ObservedOperationResult {
    let temp = TempDir::new().expect("temporary numeric CLI root");
    if nested {
        fs::create_dir(temp.path().join("nested")).expect("nested fixture directory");
        fs::write(temp.path().join("nested/file.txt"), "").expect("nested fixture file");
    } else {
        fs::write(temp.path().join("file.txt"), "").expect("fixture file");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_agentic-navigation-guide"))
        .arg("dump")
        .arg("--root")
        .arg(temp.path())
        .arg(option)
        .arg(value)
        .output()
        .expect("run dump command");

    if !output.status.success() {
        return ObservedOperationResult::Rejected;
    }

    let source = match String::from_utf8(output.stdout) {
        Ok(source) => source,
        Err(_) => return ObservedOperationResult::GeneratedInvalid,
    };
    match observe(&source) {
        ObservedResult::Reject => ObservedOperationResult::GeneratedInvalid,
        ObservedResult::Accept {
            ignore: false,
            items,
        } => ObservedOperationResult::GeneratedPaths(
            items.into_iter().map(|item| item.path).collect(),
        ),
        ObservedResult::Accept { ignore: true, .. } => ObservedOperationResult::GeneratedInvalid,
    }
}

fn fixture(id: &str) -> &'static ContractCase {
    fixtures::CASES
        .iter()
        .find(|case| case.id == id)
        .unwrap_or_else(|| panic!("missing contract fixture '{id}'"))
}

fn requires_normative_owner(issue: u32) -> bool {
    match requested_conformance_owner().as_deref() {
        Some("all") => true,
        Some(owner) => owner == issue.to_string(),
        None => false,
    }
}

fn assert_exact_nested_tree(observed: &ObservedResult, deepest_depth: usize) {
    let ObservedResult::Accept {
        ignore: false,
        items,
    } = observed
    else {
        panic!("logical depth {deepest_depth} did not produce an active guide: {observed:?}");
    };

    assert_eq!(
        items.len(),
        deepest_depth + 1,
        "logical depth {deepest_depth} was truncated or duplicated"
    );
    let mut expected_path = String::new();
    for (depth, item) in items.iter().enumerate() {
        if depth > 0 {
            expected_path.push('/');
        }
        expected_path.push_str("directory");
        assert_eq!(
            item,
            &ObservedItem {
                kind: ItemKind::Directory,
                path: expected_path.clone(),
            },
            "logical depth {deepest_depth} changed at entry depth {depth}"
        );
    }
}

fn assert_exact_choice_expansion(observed: &ObservedResult, alternatives: usize) {
    let ObservedResult::Accept {
        ignore: false,
        items,
    } = observed
    else {
        panic!("{alternatives} alternatives did not produce an active guide: {observed:?}");
    };

    assert_eq!(
        items.len(),
        alternatives,
        "{alternatives} alternatives were truncated or duplicated"
    );
    for (index, item) in items.iter().enumerate() {
        assert_eq!(
            item,
            &ObservedItem {
                kind: ItemKind::File,
                path: format!("filechoice{index}.txt"),
            },
            "{alternatives}-alternative expansion changed at index {index}"
        );
    }
}

fn nested_directory_source(deepest_depth: usize) -> String {
    let mut source = String::from("<agentic-navigation-guide>\n");
    for depth in 0..=deepest_depth {
        source.push_str(&" ".repeat(depth));
        source.push_str("- directory/\n");
    }
    source.push_str("</agentic-navigation-guide>");
    source
}

fn choice_count_source(alternatives: usize) -> String {
    let choices = (0..alternatives)
        .map(|index| format!("choice{index}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("<agentic-navigation-guide>\n- file[{choices}].txt\n</agentic-navigation-guide>")
}

fn comment_snapshot(source: &str) -> Vec<(String, Option<String>)> {
    let guide = Parser::new()
        .parse(source)
        .unwrap_or_else(|error| panic!("comment fixture did not parse: {error}"));
    Validator::new()
        .validate_syntax(&guide)
        .unwrap_or_else(|error| panic!("comment fixture did not validate: {error}"));

    let mut comments = Vec::new();
    flatten_comments(&guide.items, "", &mut comments);
    comments
}

fn flatten_comments(
    source: &[agentic_navigation_guide::NavigationGuideLine],
    parent: &str,
    output: &mut Vec<(String, Option<String>)>,
) {
    for line in source {
        let path = join_path(parent, line.path());
        output.push((path.clone(), line.comment().map(str::to_string)));
        if let Some(children) = line.children() {
            flatten_comments(children, &path, output);
        }
    }
}

fn extract_documented_cases(document: &str) -> BTreeMap<String, String> {
    const PREFIX: &str = "<!-- contract-case: ";
    const SUFFIX: &str = " -->";

    let lines: Vec<_> = document.lines().collect();
    let mut cases = BTreeMap::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let Some(id) = line
            .strip_prefix(PREFIX)
            .and_then(|value| value.strip_suffix(SUFFIX))
        else {
            index += 1;
            continue;
        };

        assert!(!id.is_empty(), "contract case ID must not be empty");
        assert_eq!(
            lines.get(index + 1),
            Some(&"```text"),
            "contract case '{id}' must be followed by a text fence"
        );

        index += 2;
        let mut source_lines = Vec::new();
        while lines.get(index).is_some_and(|line| *line != "```") {
            source_lines.push(lines[index]);
            index += 1;
        }
        assert_eq!(
            lines.get(index),
            Some(&"```"),
            "contract case '{id}' has no closing fence"
        );

        let previous = cases.insert(id.to_string(), source_lines.join("\n"));
        assert!(previous.is_none(), "duplicate documented case ID '{id}'");
        index += 1;
    }

    cases
}
