use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DECISION_COMMENT: &str =
    "https://github.com/plx/agentic-navigation-guide/issues/71#issuecomment-5090158814";
const EXPECTED_RECORD: &[(&str, &str)] = &[
    ("schema_version", "1"),
    ("decision_date", "2026-07-27"),
    ("exception_expires_on", "2026-10-31"),
    ("mode", "single-maintainer-exception"),
    ("repository_owner", "plx"),
    ("crates_io_owner", "plx"),
    ("github_organization", "none"),
    ("backup_owner", "none"),
    ("github_two_factor_status", "not-verified-api-unavailable"),
    (
        "trusted_publishing_status",
        "not-configured-owned-by-issue-63",
    ),
    (
        "release_environment_status",
        "not-configured-owned-by-issue-65",
    ),
    ("recovery_runbook_status", "not-established-no-backup"),
    ("recovery_drill_status", "not-run-no-backup"),
    (
        "security_report_route_status",
        "not-present-owned-by-issue-69",
    ),
    ("homebrew_tap_status", "not-present"),
    ("publication_after_expiry", "blocked-without-new-decision"),
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn repository_file(path: &str) -> String {
    fs::read_to_string(repository_root().join(path))
        .unwrap_or_else(|error| panic!("read repository file {path}: {error}"))
}

fn parse_flat_string_record(source: &str) -> BTreeMap<String, String> {
    let mut record = BTreeMap::new();
    for (index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, raw_value) = line
            .split_once('=')
            .unwrap_or_else(|| panic!("record line {} has no equals sign", index + 1));
        let key = key.trim();
        let value = raw_value
            .trim()
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or_else(|| panic!("record value {key:?} must be one quoted string"));
        assert!(
            record.insert(key.to_owned(), value.to_owned()).is_none(),
            "duplicate continuity-record key {key:?}"
        );
    }
    record
}

fn assert_no_secret_material(path: &str, source: &str) {
    for forbidden in [
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat_",
        "CARGO_REGISTRY_TOKEN=",
    ] {
        assert!(
            !source.contains(forbidden),
            "{path} contains token-like material beginning with {forbidden:?}"
        );
    }
}

fn normalized_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_date(value: &str) -> (i64, u32, u32) {
    let fields = value.split('-').collect::<Vec<_>>();
    assert_eq!(fields.len(), 3, "date {value:?} must be YYYY-MM-DD");
    assert_eq!(fields[0].len(), 4, "date {value:?} must have a year");
    assert_eq!(fields[1].len(), 2, "date {value:?} must have a month");
    assert_eq!(fields[2].len(), 2, "date {value:?} must have a day");

    let year = fields[0]
        .parse::<i64>()
        .unwrap_or_else(|error| panic!("date {value:?} has an invalid year: {error}"));
    let month = fields[1]
        .parse::<u32>()
        .unwrap_or_else(|error| panic!("date {value:?} has an invalid month: {error}"));
    let day = fields[2]
        .parse::<u32>()
        .unwrap_or_else(|error| panic!("date {value:?} has an invalid day: {error}"));
    assert!((1..=12).contains(&month), "date {value:?} has no month");

    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let maximum_day = match month {
        2 if leap_year => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    assert!(
        (1..=maximum_day).contains(&day),
        "date {value:?} has no day"
    );
    (year, month, day)
}

fn days_since_unix_epoch(value: &str) -> i64 {
    let (mut year, month, day) = parse_date(value);
    if month <= 2 {
        year -= 1;
    }
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let shifted_month = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn exception_is_active(as_of: &str, expires_on: &str) -> bool {
    days_since_unix_epoch(as_of) <= days_since_unix_epoch(expires_on)
}

#[test]
fn issue_71_exception_record_is_explicit_complete_and_time_bounded() {
    let record_source = repository_file("release/maintainer-continuity.toml");
    let record = parse_flat_string_record(&record_source);
    let expected = EXPECTED_RECORD
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        record, expected,
        "the approved exception record must change only through a new maintainer decision"
    );
    assert!(
        record["decision_date"] < record["exception_expires_on"],
        "the sole-maintainer exception must have a future review deadline"
    );
    assert_eq!(record["backup_owner"], "none");
    assert_eq!(record["recovery_drill_status"], "not-run-no-backup");
    assert_no_secret_material("release/maintainer-continuity.toml", &record_source);
}

#[test]
fn issue_71_exception_expiry_is_a_fail_closed_utc_gate() {
    let record = parse_flat_string_record(&repository_file("release/maintainer-continuity.toml"));
    let expires_on = &record["exception_expires_on"];

    assert!(
        exception_is_active("2026-10-31", expires_on),
        "the approved exception includes its final recorded day"
    );
    assert!(
        !exception_is_active("2026-11-01", expires_on),
        "the exception must fail closed on the first day after expiry"
    );

    let current_day = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the current clock must be after the Unix epoch")
        .as_secs()
        / 86_400;
    let expiry_day = days_since_unix_epoch(expires_on);
    assert!(
        i64::try_from(current_day).expect("current UTC day fits i64") <= expiry_day,
        "the issue #71 sole-maintainer exception expired on {expires_on}; \
         publication and ordinary green CI remain blocked until a new explicit \
         maintainer decision updates the continuity record"
    );
}

#[test]
fn issue_71_policy_states_authority_recovery_and_support_without_false_attestation() {
    let policy = repository_file("docs/maintainer-continuity.md");
    let normalized_policy = normalized_whitespace(&policy);

    for required in [
        "<!-- issue-71-single-maintainer-exception -->",
        DECISION_COMMENT,
        "There is no backup maintainer or second administrator.",
        "GitHub 2FA status is not verified by this repository.",
        "No independent recovery drill has been performed.",
        "Publication after 2026-10-31 is blocked",
        "Repository CI fails closed after the expiry",
        "best-effort maintenance",
        "no response-time, availability, or organizational-redundancy guarantee",
        "Issue #63 owns the Trusted Publishing workflow",
        "Issue #65 owns the protected release environment",
        "Issue #69 owns the public security-report route",
        "No Homebrew tap exists.",
        "within 24 hours",
        "within 48 hours",
        "at every minor release and at least every six months",
    ] {
        assert!(
            normalized_policy.contains(required),
            "maintainer-continuity policy is missing {required:?}"
        );
    }

    for false_claim in [
        "Two maintainers currently",
        "2FA is verified",
        "The recovery drill passed",
        "Trusted Publishing is configured",
        "The release environment is configured",
    ] {
        assert!(
            !normalized_policy.contains(false_claim),
            "maintainer-continuity policy makes unsupported claim {false_claim:?}"
        );
    }
    assert_no_secret_material("docs/maintainer-continuity.md", &policy);
}

#[test]
fn issue_71_public_docs_and_acceptance_audit_remain_aligned() {
    let audit_path = "audits/2026-07-27-issue-71-maintainer-continuity.md";
    let audit = repository_file(audit_path);
    let readme = repository_file("README.md");
    let release_policy = repository_file("docs/release-policy.md");
    let changelog = repository_file("CHANGELOG.md");
    let guide = repository_file("AGENTIC_NAVIGATION_GUIDE.md");
    let normalized_audit = normalized_whitespace(&audit);

    for required in [
        DECISION_COMMENT,
        "| A71-001 | Exception — no tested independent recovery path |",
        "| A71-002 | Policy set; external verification deferred |",
        "| A71-003 | Implemented |",
        "| A71-004 | Verified |",
        "| A71-005 | Exception accepted |",
        "No organization, collaborator, crate owner, token, environment, ruleset, or other protected setting was changed.",
        "No tabletop recovery drill was represented as passing.",
        "no fuzzing, mutation testing, randomized generation, or generated hostile input",
    ] {
        assert!(
            normalized_audit.contains(required),
            "issue #71 acceptance audit is missing {required:?}"
        );
    }

    let linked_surfaces = [
        ("README.md", &readme),
        ("docs/release-policy.md", &release_policy),
        ("CHANGELOG.md", &changelog),
    ];
    for (path, source) in linked_surfaces {
        assert!(
            source.contains("docs/maintainer-continuity.md"),
            "{path} must link the maintainer-continuity policy"
        );
    }

    for path in [
        "docs/maintainer-continuity.md",
        "release/maintainer-continuity.toml",
        audit_path,
        "tests/issue_71_maintainer_continuity.rs",
    ] {
        assert!(
            guide.contains(path.rsplit('/').next().expect("path has a filename")),
            "navigation guide must inventory {path}"
        );
    }

    let acceptance_ids = (1..=5)
        .map(|number| format!("A71-{number:03}"))
        .collect::<BTreeSet<_>>();
    for id in &acceptance_ids {
        assert_eq!(
            audit.matches(&format!("| {id} |")).count(),
            1,
            "acceptance criterion {id} must have exactly one disposition"
        );
    }
    assert_no_secret_material(audit_path, &audit);
}
