use agentic_navigation_guide::{Dumper, FilesystemItem, Parser, Validator, Verifier};
use quote::ToTokens;
use std::collections::{BTreeMap, BTreeSet};
use std::env::VarError;
use std::fs;
use std::process::Command;
use syn::{Fields, FnArg, Item, ReturnType, UseTree, Visibility};
use tempfile::TempDir;

const ALLOWED_PENDING_OWNERS: &[u32] = &[40, 41, 42, 43, 44, 50];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConformanceRequest {
    Current,
    All,
    Owner(u32),
}

impl ConformanceRequest {
    fn requires_normative(self, pending_issue: Option<u32>) -> bool {
        match self {
            Self::Current => false,
            Self::All => true,
            Self::Owner(issue) => pending_issue == Some(issue),
        }
    }
}

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
    CliIgnoredDenied,
    NoSupportedLibraryFacade,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TrustSurface {
    GuideInput,
    Generation,
    Containment,
    Output,
    Diagnostics,
    Concurrency,
}

impl TrustSurface {
    fn contract_text(self) -> &'static str {
        match self {
            Self::GuideInput => "Guide input",
            Self::Generation => "Generation root",
            Self::Containment => "Containment",
            Self::Output => "File output",
            Self::Diagnostics => "Diagnostics",
            Self::Concurrency => "Concurrency",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrustOutcome {
    Allow,
    AllowExplicit,
    AllowAsRegular,
    AllowAsAnchor,
    CreateNew,
    CreateExplicitExternal,
    CreateRegularFileOnly,
    ExactlyOneCreator,
    EnforceSharedPolicy,
    PruneWithoutTraversal,
    PreserveRootSpelling,
    RejectBeforeRead,
    RejectBeforeCreate,
    RejectBeforeTraversal,
    RejectAndReportResidualArtifact,
    RejectObservedMutation,
    RejectUsage,
    RejectWithoutDisclosure,
    RejectWithoutMutation,
    RejectWithoutTraversal,
    ResolveFromAnchor,
    DocumentInProgressVisibility,
    DocumentStableOnly,
}

impl TrustOutcome {
    fn contract_text(self) -> &'static str {
        match self {
            Self::Allow => "Allow",
            Self::AllowExplicit => "Allow only as explicit authority",
            Self::AllowAsRegular => "Allow as a regular file",
            Self::AllowAsAnchor => "Allow as the caller-selected canonical anchor",
            Self::CreateNew => "Create only when the final name is absent",
            Self::CreateExplicitExternal => {
                "Create only as explicit external authority when the final name is absent"
            }
            Self::CreateRegularFileOnly => "Create only a verified regular filesystem entry",
            Self::ExactlyOneCreator => {
                "Exactly one creator may succeed; the loser never overwrites"
            }
            Self::EnforceSharedPolicy => "Enforce the same safe-opening policy; no bypass",
            Self::PruneWithoutTraversal => "Prune without traversal",
            Self::PreserveRootSpelling => {
                "Preserve unresolved parent components in lexical anchor comparison"
            }
            Self::RejectBeforeRead => "Reject before reading",
            Self::RejectBeforeCreate => "Reject before destination creation",
            Self::RejectBeforeTraversal => "Reject before input traversal",
            Self::RejectAndReportResidualArtifact => {
                "Reject, attempt identity-safe cleanup, and report any residual artifact"
            }
            Self::RejectObservedMutation => "Reject every observed identity or type change",
            Self::RejectUsage => "Reject as invalid configuration before filesystem access",
            Self::RejectWithoutDisclosure => {
                "Reject without source content or resolved-target disclosure"
            }
            Self::RejectWithoutMutation => "Reject without creating, replacing, or modifying",
            Self::RejectWithoutTraversal => "Reject without following or traversal",
            Self::ResolveFromAnchor => "Resolve the implicit path from the effective root",
            Self::DocumentInProgressVisibility => {
                "Document that create-new is not atomic content publication"
            }
            Self::DocumentStableOnly => {
                "Document as unsupported beyond the stable-filesystem guarantee"
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TrustCase {
    id: &'static str,
    surface: TrustSurface,
    normative: TrustOutcome,
    owner_issue: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ApiKind {
    PackageTarget,
    Module,
    ReExport,
    TypeAlias,
    Struct,
    Enum,
    Variant,
    Field,
    Function,
    Method,
}

impl ApiKind {
    fn contract_text(self) -> &'static str {
        match self {
            Self::PackageTarget => "Package target",
            Self::Module => "Module",
            Self::ReExport => "Root re-export",
            Self::TypeAlias => "Type alias",
            Self::Struct => "Struct",
            Self::Enum => "Enum",
            Self::Variant => "Enum variant",
            Self::Field => "Public field",
            Self::Function => "Free function",
            Self::Method => "Inherent method",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApiDisposition {
    RemoveLibraryTarget,
    MakeImplementationOnly,
    RemoveIncorrectMethod,
    RemoveUnsupportedLinkModel,
}

impl ApiDisposition {
    fn contract_text(self) -> &'static str {
        match self {
            Self::RemoveLibraryTarget => "Remove the linkable library target",
            Self::MakeImplementationOnly => "Make implementation-only",
            Self::RemoveIncorrectMethod => "Remove the incorrect method",
            Self::RemoveUnsupportedLinkModel => "Remove the unsupported link model",
        }
    }

    fn is_supported_v0_2_facade(self) -> bool {
        match self {
            Self::RemoveLibraryTarget
            | Self::MakeImplementationOnly
            | Self::RemoveIncorrectMethod
            | Self::RemoveUnsupportedLinkModel => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ApiCase {
    id: &'static str,
    kind: ApiKind,
    symbol: &'static str,
    disposition: ApiDisposition,
    owner_issue: u32,
}

#[derive(Clone, Copy, Debug)]
struct ApiTraitCase {
    id: &'static str,
    group: &'static str,
    current_commitments: &'static str,
    disposition: &'static str,
}

mod fixtures {
    use super::{ContractCase, ExpectedItem, ExpectedResult, ItemKind};

    include!("fixtures/v0_2_contract.rs");
}

mod operation_fixtures {
    use super::{ExpectedItem, ExpectedOperationResult, ItemKind, OperationCase, OperationKind};

    include!("fixtures/v0_2_operations.rs");
}

mod trust_fixtures {
    use super::{TrustCase, TrustOutcome, TrustSurface};

    include!("fixtures/v0_2_trust.rs");
}

mod api_fixtures {
    use super::{ApiCase, ApiDisposition, ApiKind, ApiTraitCase};

    include!("fixtures/v0_2_api.rs");
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
    NoSupportedLibraryFacade,
    CapabilityUnavailable,
    Identity { host_aliases: bool, verified: bool },
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

fn conformance_request() -> ConformanceRequest {
    match std::env::var("GUIDE_FORMAT_REQUIRE_CONFORMANCE") {
        Ok(value) => parse_conformance_request(&value),
        Err(VarError::NotPresent) => ConformanceRequest::Current,
        Err(VarError::NotUnicode(_)) => {
            panic!("GUIDE_FORMAT_REQUIRE_CONFORMANCE must contain valid Unicode")
        }
    }
}

fn parse_conformance_request(value: &str) -> ConformanceRequest {
    match value {
        "" => ConformanceRequest::Current,
        "all" => ConformanceRequest::All,
        owner => {
            let issue = owner
                .parse::<u32>()
                .unwrap_or_else(|_| panic!("invalid conformance owner '{owner}'"));
            assert!(
                ALLOWED_PENDING_OWNERS.contains(&issue),
                "conformance owner #{issue} is not in the contract handoff"
            );
            assert!(
                fixtures::CASES
                    .iter()
                    .any(|case| case.pending_issue == Some(issue))
                    || operation_fixtures::CASES
                        .iter()
                        .any(|case| case.pending_issue == Some(issue)),
                "conformance owner #{issue} has no pending rows"
            );
            ConformanceRequest::Owner(issue)
        }
    }
}

#[test]
fn contract_cases() {
    let request = conformance_request();

    let mut pending_rows = 0;

    for case in fixtures::CASES {
        if case.pending_issue.is_some() {
            pending_rows += 1;
        }

        let require_normative = request.requires_normative(case.pending_issue);

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

    if request == ConformanceRequest::All {
        assert_eq!(
            pending_rows, 0,
            "all-mode requires every pending contract row to be activated"
        );
    }
}

#[test]
fn operation_cases() {
    let request = conformance_request();
    let mut pending_rows = 0;

    for case in operation_fixtures::CASES {
        if case.pending_issue.is_some() {
            pending_rows += 1;
        }

        let require_normative = request.requires_normative(case.pending_issue);
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

    if request == ConformanceRequest::All {
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

    let trust_ids: BTreeSet<_> = trust_fixtures::CASES.iter().map(|case| case.id).collect();
    assert_eq!(
        trust_ids.len(),
        trust_fixtures::CASES.len(),
        "trust fixture IDs must be unique"
    );

    let document = include_str!("../docs/v0.2-contract.md");
    let documented_trust_rows = document
        .lines()
        .filter(|line| line.starts_with("| `trust-"))
        .count();
    assert_eq!(
        documented_trust_rows,
        trust_fixtures::CASES.len(),
        "the normative document and trust fixture must contain the same number of rows"
    );

    for case in trust_fixtures::CASES {
        let documented_row = format!(
            "| `{}` | {} | {} | #{} |",
            case.id,
            case.surface.contract_text(),
            case.normative.contract_text(),
            case.owner_issue
        );
        assert_eq!(
            document
                .lines()
                .filter(|line| *line == documented_row)
                .count(),
            1,
            "trust fixture '{}' must have one exact documented row",
            case.id
        );
    }

    let api_ids: BTreeSet<_> = api_fixtures::CASES.iter().map(|case| case.id).collect();
    assert_eq!(
        api_ids.len(),
        api_fixtures::CASES.len(),
        "API fixture IDs must be unique"
    );

    let documented_api_rows = document
        .lines()
        .filter(|line| line.starts_with("| `api-"))
        .count();
    assert_eq!(
        documented_api_rows,
        api_fixtures::CASES.len(),
        "the normative document and API fixture must contain the same number of rows"
    );

    for case in api_fixtures::CASES {
        let documented_row = format!(
            "| `{}` | {} | `{}` | {} | #{} |",
            case.id,
            case.kind.contract_text(),
            case.symbol,
            case.disposition.contract_text(),
            case.owner_issue
        );
        assert_eq!(
            document
                .lines()
                .filter(|line| *line == documented_row)
                .count(),
            1,
            "API fixture '{}' must have one exact documented row",
            case.id
        );
    }

    let trait_ids: BTreeSet<_> = api_fixtures::TRAIT_CASES
        .iter()
        .map(|case| case.id)
        .collect();
    assert_eq!(
        trait_ids.len(),
        api_fixtures::TRAIT_CASES.len(),
        "trait commitment fixture IDs must be unique"
    );
    assert_eq!(
        document
            .lines()
            .filter(|line| line.starts_with("| `trait-commitment-"))
            .count(),
        api_fixtures::TRAIT_CASES.len(),
        "the normative document and trait commitment fixture must contain the same rows"
    );
    for case in api_fixtures::TRAIT_CASES {
        let documented_row = format!(
            "| `{}` | {} | {} | {} |",
            case.id, case.group, case.current_commitments, case.disposition
        );
        assert_eq!(
            document
                .lines()
                .filter(|line| *line == documented_row)
                .count(),
            1,
            "trait commitment fixture '{}' must have one exact documented row",
            case.id
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
fn trust_rows_have_one_focused_owner_and_cover_every_surface() {
    const ALLOWED_TRUST_OWNERS: &[u32] = &[43, 45, 49, 51];

    let mut surfaces = BTreeSet::new();
    let mut owners = BTreeSet::new();

    for case in trust_fixtures::CASES {
        assert!(
            case.id.starts_with("trust-"),
            "trust fixture '{}' must use the trust- prefix",
            case.id
        );
        assert!(
            ALLOWED_TRUST_OWNERS.contains(&case.owner_issue),
            "trust fixture '{}' has unexpected owner #{}",
            case.id,
            case.owner_issue
        );
        surfaces.insert(case.surface);
        owners.insert(case.owner_issue);
    }

    assert_eq!(
        surfaces,
        BTreeSet::from([
            TrustSurface::GuideInput,
            TrustSurface::Generation,
            TrustSurface::Containment,
            TrustSurface::Output,
            TrustSurface::Diagnostics,
            TrustSurface::Concurrency,
        ]),
        "the trust ledger must cover every policy surface"
    );
    assert_eq!(
        owners,
        BTreeSet::from([43, 45, 49, 51]),
        "each focused implementation owner must receive at least one trust row"
    );
}

#[test]
fn api_rows_inventory_the_complete_current_source_surface_and_one_fate_per_export() {
    const EXPECTED_KIND_COUNTS: &[(ApiKind, usize)] = &[
        (ApiKind::PackageTarget, 1),
        (ApiKind::Module, 7),
        (ApiKind::ReExport, 17),
        (ApiKind::TypeAlias, 1),
        (ApiKind::Struct, 10),
        (ApiKind::Enum, 6),
        (ApiKind::Variant, 39),
        (ApiKind::Field, 19),
        (ApiKind::Function, 7),
        (ApiKind::Method, 25),
    ];

    assert_eq!(
        api_fixtures::CASES.len(),
        132,
        "the audited current-source surface changed; update the explicit decision ledger"
    );

    let mut kind_counts = BTreeMap::new();
    let mut owner_counts = BTreeMap::new();
    let mut symbols = BTreeSet::new();

    for case in api_fixtures::CASES {
        assert!(
            case.id.starts_with("api-"),
            "API fixture '{}' must use the api- prefix",
            case.id
        );
        assert!(
            matches!(case.owner_issue, 52..=54),
            "API fixture '{}' has unexpected owner #{}",
            case.id,
            case.owner_issue
        );
        let expected_owner = match case.disposition {
            ApiDisposition::RemoveIncorrectMethod => 52,
            ApiDisposition::RemoveUnsupportedLinkModel => 53,
            ApiDisposition::RemoveLibraryTarget | ApiDisposition::MakeImplementationOnly => 54,
        };
        assert_eq!(
            case.owner_issue, expected_owner,
            "API fixture '{}' assigns {:?} to the wrong focused owner",
            case.id, case.disposition
        );
        assert!(
            symbols.insert((case.kind, case.symbol)),
            "API fixture '{}' duplicates {:?} '{}'",
            case.id,
            case.kind,
            case.symbol
        );
        *kind_counts.entry(case.kind).or_insert(0) += 1;
        *owner_counts.entry(case.owner_issue).or_insert(0) += 1;
    }

    assert_eq!(
        kind_counts,
        EXPECTED_KIND_COUNTS.iter().copied().collect(),
        "the current export inventory changed without an explicit #36 disposition"
    );
    assert_eq!(
        owner_counts,
        BTreeMap::from([(52, 1), (53, 2), (54, 129)]),
        "every current export must have exactly one focused implementation owner"
    );
    assert_eq!(
        api_fixtures::CASES
            .iter()
            .filter(|case| case.disposition == ApiDisposition::RemoveLibraryTarget)
            .map(|case| case.id)
            .collect::<Vec<_>>(),
        vec!["api-target-library"],
        "the binary-only decision must remove exactly the library package target"
    );
    assert_eq!(
        api_fixtures::CASES
            .iter()
            .filter(|case| case.disposition == ApiDisposition::RemoveIncorrectMethod)
            .map(|case| case.id)
            .collect::<Vec<_>>(),
        vec!["api-method-navigation-guide-get-full-path"],
        "the known incorrect helper must remain assigned to #52"
    );
    assert_eq!(
        api_fixtures::CASES
            .iter()
            .filter(|case| case.disposition == ApiDisposition::RemoveUnsupportedLinkModel)
            .map(|case| case.id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "api-variant-filesystem-item-symlink",
            "api-variant-semantic-error-symlink-target-mismatch",
        ]),
        "the unsupported textual-link model must remain assigned to #53"
    );
}

#[test]
fn api_ledger_matches_current_rust_source_and_cargo_target() {
    let expected: BTreeSet<_> = api_fixtures::CASES
        .iter()
        .filter(|case| case.kind != ApiKind::PackageTarget)
        .map(|case| (case.kind, case.symbol.to_string()))
        .collect();
    let observed = collect_current_source_api();

    assert_eq!(
        observed, expected,
        "the current-source Rust API changed without an exact #36 ledger update"
    );

    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo metadata for the current package target snapshot");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata = String::from_utf8(output.stdout).expect("UTF-8 cargo metadata");
    assert_eq!(
        metadata.matches("\"kind\":[\"lib\"]").count(),
        1,
        "the current-source decision ledger expects exactly one legacy library target"
    );
    assert!(
        metadata.contains(
            "\"kind\":[\"lib\"],\"crate_types\":[\"lib\"],\"name\":\"agentic_navigation_guide\""
        ),
        "the legacy library target name drifted from the decision ledger"
    );
    assert_eq!(
        api_fixtures::CASES
            .iter()
            .filter(|case| case.kind == ApiKind::PackageTarget)
            .map(|case| case.symbol)
            .collect::<Vec<_>>(),
        vec!["agentic_navigation_guide (lib)"],
        "the Cargo target and package-target ledger row must stay aligned"
    );

    let mut expected_variant_order: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for case in api_fixtures::CASES
        .iter()
        .filter(|case| case.kind == ApiKind::Variant)
    {
        let (type_name, remainder) = case
            .symbol
            .split_once("::")
            .expect("qualified variant ledger symbol");
        let variant_name = remainder
            .split(['(', ' ', '{', '='])
            .next()
            .expect("variant name");
        let variant_identity = if let Some((_, discriminant)) = remainder.rsplit_once(" = ") {
            format!(
                "{variant_name}={}",
                discriminant
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .collect::<String>()
            )
        } else {
            variant_name.to_string()
        };
        expected_variant_order
            .entry(type_name.to_string())
            .or_default()
            .push(variant_identity);
    }
    assert_eq!(
        collect_current_variant_order(),
        expected_variant_order,
        "enum variant order or explicit discriminants changed without a #36 ledger update"
    );
}

#[test]
fn current_concrete_public_types_preserve_the_audited_auto_traits() {
    fn assert_core_auto_traits<T: Send + Sync + Unpin>() {}

    fn assert_unwind_auto_traits<
        T: Send + Sync + Unpin + std::panic::UnwindSafe + std::panic::RefUnwindSafe,
    >() {
    }

    assert_unwind_auto_traits::<agentic_navigation_guide::Dumper>();
    assert_unwind_auto_traits::<agentic_navigation_guide::errors::ErrorFormatter>();
    assert_unwind_auto_traits::<agentic_navigation_guide::Parser>();
    assert_unwind_auto_traits::<agentic_navigation_guide::GuideLocation>();
    assert_unwind_auto_traits::<agentic_navigation_guide::GuideVerificationResult>();
    assert_unwind_auto_traits::<agentic_navigation_guide::NavigationGuideLine>();
    assert_unwind_auto_traits::<agentic_navigation_guide::NavigationGuide>();
    assert_unwind_auto_traits::<agentic_navigation_guide::types::Config>();
    assert_unwind_auto_traits::<agentic_navigation_guide::Validator>();
    assert_unwind_auto_traits::<agentic_navigation_guide::Verifier>();
    assert_core_auto_traits::<agentic_navigation_guide::AppError>();
    assert_unwind_auto_traits::<agentic_navigation_guide::SyntaxError>();
    assert_unwind_auto_traits::<agentic_navigation_guide::SemanticError>();
    assert_unwind_auto_traits::<agentic_navigation_guide::FilesystemItem>();
    assert_unwind_auto_traits::<agentic_navigation_guide::ExecutionMode>();
    assert_unwind_auto_traits::<agentic_navigation_guide::LogLevel>();
}

fn collect_current_source_api() -> BTreeSet<(ApiKind, String)> {
    const SOURCES: &[(&str, &str)] = &[
        ("crate", include_str!("../src/lib.rs")),
        ("dumper", include_str!("../src/dumper.rs")),
        ("errors", include_str!("../src/errors.rs")),
        ("parser", include_str!("../src/parser.rs")),
        ("recursive", include_str!("../src/recursive.rs")),
        ("types", include_str!("../src/types.rs")),
        ("validator", include_str!("../src/validator.rs")),
        ("verifier", include_str!("../src/verifier.rs")),
    ];

    let mut surface = BTreeSet::new();

    for (module, source) in SOURCES {
        let file = syn::parse_file(source).unwrap_or_else(|error| {
            panic!("parse current Rust source for API inventory ({module}): {error}")
        });

        for item in file.items {
            match item {
                Item::Mod(item) if is_public(&item.vis) => {
                    assert_eq!(*module, "crate", "unexpected nested public module");
                    surface.insert((ApiKind::Module, item.ident.to_string()));
                }
                Item::Use(item) if is_public(&item.vis) => {
                    assert_eq!(*module, "crate", "unexpected non-root public re-export");
                    let mut exported = Vec::new();
                    flatten_public_use(&item.tree, &mut exported);
                    for name in exported {
                        surface.insert((ApiKind::ReExport, format!("crate::{name}")));
                    }
                }
                Item::Type(item) if is_public(&item.vis) => {
                    let generics = render_generics(&item.generics);
                    let target = render_type(&item.ty);
                    surface.insert((
                        ApiKind::TypeAlias,
                        format!("{module}::{}{generics} = {target}", item.ident),
                    ));
                }
                Item::Struct(item) if is_public(&item.vis) => {
                    let type_name = item.ident.to_string();
                    surface.insert((
                        ApiKind::Struct,
                        format!("{module}::{type_name}{}", render_generics(&item.generics)),
                    ));
                    collect_public_struct_fields(&mut surface, &type_name, &item.fields);
                }
                Item::Enum(item) if is_public(&item.vis) => {
                    let type_name = item.ident.to_string();
                    surface.insert((
                        ApiKind::Enum,
                        format!("{module}::{type_name}{}", render_generics(&item.generics)),
                    ));
                    for variant in item.variants {
                        surface.insert((ApiKind::Variant, render_variant(&type_name, &variant)));
                    }
                }
                Item::Fn(item) if is_public(&item.vis) => {
                    surface.insert((ApiKind::Function, render_function(module, &item.sig)));
                }
                Item::Impl(item) if item.trait_.is_none() => {
                    let type_name = inherent_impl_type_name(&item.self_ty);
                    for impl_item in item.items {
                        if let syn::ImplItem::Fn(function) = impl_item {
                            if is_public(&function.vis) {
                                surface.insert((
                                    ApiKind::Method,
                                    render_function(&type_name, &function.sig),
                                ));
                            }
                        }
                    }
                }
                Item::Const(item) if is_public(&item.vis) => {
                    panic!("uninventoried public const '{}::{:?}'", module, item.ident)
                }
                Item::Static(item) if is_public(&item.vis) => {
                    panic!("uninventoried public static '{}::{:?}'", module, item.ident)
                }
                Item::Trait(item) if is_public(&item.vis) => {
                    panic!("uninventoried public trait '{}::{:?}'", module, item.ident)
                }
                Item::TraitAlias(item) if is_public(&item.vis) => {
                    panic!(
                        "uninventoried public trait alias '{}::{:?}'",
                        module, item.ident
                    )
                }
                Item::Union(item) if is_public(&item.vis) => {
                    panic!("uninventoried public union '{}::{:?}'", module, item.ident)
                }
                _ => {}
            }
        }
    }

    surface
}

fn collect_current_variant_order() -> BTreeMap<String, Vec<String>> {
    const ENUM_SOURCES: &[&str] = &[
        include_str!("../src/errors.rs"),
        include_str!("../src/types.rs"),
    ];

    let mut order = BTreeMap::new();
    for source in ENUM_SOURCES {
        let file = syn::parse_file(source).expect("parse enum source for order snapshot");
        for item in file.items {
            let Item::Enum(item) = item else {
                continue;
            };
            if !is_public(&item.vis) {
                continue;
            }
            let type_name = item.ident.to_string();
            let variants = item
                .variants
                .iter()
                .map(|variant| {
                    let mut identity = variant.ident.to_string();
                    if let Some((_, discriminant)) = &variant.discriminant {
                        identity.push('=');
                        identity.push_str(&compact_tokens(discriminant));
                    }
                    identity
                })
                .collect();
            order.insert(type_name, variants);
        }
    }
    order
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn flatten_public_use(tree: &UseTree, exported: &mut Vec<String>) {
    match tree {
        UseTree::Path(path) => flatten_public_use(&path.tree, exported),
        UseTree::Name(name) => exported.push(name.ident.to_string()),
        UseTree::Rename(rename) => exported.push(rename.rename.to_string()),
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_public_use(item, exported);
            }
        }
        UseTree::Glob(_) => panic!("glob re-exports are not allowed in the exact API ledger"),
    }
}

fn render_generics(generics: &syn::Generics) -> String {
    let parameters = if generics.params.is_empty() {
        String::new()
    } else {
        let parameters = generics
            .params
            .iter()
            .map(compact_tokens)
            .collect::<Vec<_>>()
            .join(",");
        format!("<{parameters}>")
    };
    format!("{parameters}{}", render_where_clause(generics))
}

fn render_generic_parameters(generics: &syn::Generics) -> String {
    if generics.params.is_empty() {
        String::new()
    } else {
        let parameters = generics
            .params
            .iter()
            .map(compact_tokens)
            .collect::<Vec<_>>()
            .join(",");
        format!("<{parameters}>")
    }
}

fn render_where_clause(generics: &syn::Generics) -> String {
    generics
        .where_clause
        .as_ref()
        .map(|clause| {
            let clause = compact_tokens(clause);
            let predicates = clause
                .strip_prefix("where")
                .expect("syn where-clause tokens start with where");
            format!(" where {predicates}")
        })
        .unwrap_or_default()
}

fn render_type(ty: &syn::Type) -> String {
    compact_tokens(ty)
}

fn compact_tokens(tokens: &impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn collect_public_struct_fields(
    surface: &mut BTreeSet<(ApiKind, String)>,
    type_name: &str,
    fields: &Fields,
) {
    for (index, field) in fields.iter().enumerate() {
        if !is_public(&field.vis) {
            continue;
        }
        let field_name = field
            .ident
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| index.to_string());
        surface.insert((
            ApiKind::Field,
            format!("{type_name}::{field_name}: {}", render_type(&field.ty)),
        ));
    }
}

fn render_variant(type_name: &str, variant: &syn::Variant) -> String {
    let prefix = format!("{type_name}::{}", variant.ident);
    let fields = match &variant.fields {
        Fields::Unit => prefix,
        Fields::Unnamed(fields) => {
            let fields = fields
                .unnamed
                .iter()
                .map(|field| render_type(&field.ty))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{prefix}({fields})")
        }
        Fields::Named(fields) => {
            let fields = fields
                .named
                .iter()
                .map(|field| {
                    format!(
                        "{}: {}",
                        field.ident.as_ref().expect("named variant field"),
                        render_type(&field.ty)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{prefix} {{ {fields} }}")
        }
    };
    if let Some((_, discriminant)) = &variant.discriminant {
        format!("{fields} = {}", compact_tokens(discriminant))
    } else {
        fields
    }
}

fn render_function(namespace: &str, signature: &syn::Signature) -> String {
    let mut arguments = signature
        .inputs
        .iter()
        .map(|argument| match argument {
            FnArg::Receiver(receiver) => {
                if receiver.reference.is_some() {
                    if receiver.mutability.is_some() {
                        "&mut self".to_string()
                    } else {
                        "&self".to_string()
                    }
                } else {
                    "self".to_string()
                }
            }
            FnArg::Typed(argument) => format!(
                "{}: {}",
                compact_tokens(&argument.pat),
                render_type(&argument.ty)
            ),
        })
        .collect::<Vec<_>>();
    if signature.variadic.is_some() {
        arguments.push("...".to_string());
    }
    let arguments = arguments.join(", ");
    let output = match &signature.output {
        ReturnType::Default => String::new(),
        ReturnType::Type(_, ty) => format!(" -> {}", render_type(ty)),
    };
    let mut modifiers = String::new();
    if signature.constness.is_some() {
        modifiers.push_str("const ");
    }
    if signature.asyncness.is_some() {
        modifiers.push_str("async ");
    }
    if signature.unsafety.is_some() {
        modifiers.push_str("unsafe ");
    }
    if let Some(abi) = &signature.abi {
        modifiers.push_str(&format!("{} ", compact_tokens(abi)));
    }
    let generics = render_generic_parameters(&signature.generics);
    let where_clause = render_where_clause(&signature.generics);

    format!(
        "{namespace}::{modifiers}{}{generics}({arguments}){output}{where_clause}",
        signature.ident
    )
}

fn inherent_impl_type_name(ty: &syn::Type) -> String {
    let syn::Type::Path(path) = ty else {
        panic!("unsupported inherent impl target in API inventory")
    };
    path.path
        .segments
        .last()
        .expect("inherent impl type path")
        .ident
        .to_string()
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
    assert!(
        matches!(observe(&over_limit), ObservedResult::Reject),
        "logical depth above {MAX_LOGICAL_DEPTH} must be rejected"
    );
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
fn conformance_request_rejects_unknown_owners() {
    assert_eq!(
        parse_conformance_request("40"),
        ConformanceRequest::Owner(40)
    );
    assert_eq!(parse_conformance_request("all"), ConformanceRequest::All);

    for invalid in ["ALL", "owner", "36", "37", "38", "39", "99"] {
        assert!(
            std::panic::catch_unwind(|| parse_conformance_request(invalid)).is_err(),
            "invalid conformance request '{invalid}' was accepted"
        );
    }
}

#[test]
fn library_ignored_gate_requires_non_vacuous_absence_of_supported_facades() {
    assert_eq!(
        api_fixtures::CASES.len(),
        132,
        "the supported-facade assertion must inspect the complete pinned API inventory"
    );
    assert_eq!(
        supported_v0_2_facade_ids(),
        Vec::<&str>::new(),
        "the approved #36 decision leaves no supported Rust facade"
    );
    assert_eq!(
        observe_ignored_library_matrix(),
        ObservedOperationResult::NoSupportedLibraryFacade
    );
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
        | (ObservedOperationResult::CliIgnoredDenied, ExpectedOperationResult::CliIgnoredDenied)
        | (
            ObservedOperationResult::NoSupportedLibraryFacade,
            ExpectedOperationResult::NoSupportedLibraryFacade,
        )
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
        OperationKind::LibraryIgnored => observe_ignored_library_matrix(),
        OperationKind::DumpZeroIndent => observe_dump_cli_number("--indent", "0", true),
        OperationKind::DumpExcessiveDepth => observe_dump_cli_number("--depth", "257", false),
    }
}

fn observe_ignored_library_matrix() -> ObservedOperationResult {
    assert_eq!(
        api_fixtures::CASES.len(),
        132,
        "the ignored-library operation must inspect the complete API inventory"
    );
    if supported_v0_2_facade_ids().is_empty() {
        ObservedOperationResult::NoSupportedLibraryFacade
    } else {
        ObservedOperationResult::Rejected
    }
}

fn supported_v0_2_facade_ids() -> Vec<&'static str> {
    api_fixtures::CASES
        .iter()
        .filter(|case| case.disposition.is_supported_v0_2_facade())
        .map(|case| case.id)
        .collect()
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
    conformance_request().requires_normative(Some(issue))
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
