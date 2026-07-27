# Issue #69 security-policy evidence

Date: 2026-07-27

Repository: `plx/agentic-navigation-guide`

## Scope and sole-maintainer constraint

This work establishes a real private report route and a feasible public
security policy without creating a GitHub organization or claiming an
independent responder. `plx` remains the only repository administrator,
private-advisory owner, and release authority. If that account is unavailable
or compromised, intake and publication stop.

The policy is documentation and hosted-control work. It changes no parser,
filesystem, or CLI runtime behavior. No fuzzing, randomized generation,
mutation campaign, or fabricated live vulnerability was used.

## Hosted private-route validation

The repository-level private vulnerability reporting setting was disabled at
the start of issue #69. It was enabled through GitHub's repository API and a
subsequent status read returned `enabled: true`.

An owner account cannot submit an external-style private vulnerability report
to its own repository. GitHub rejected that probe with HTTP 403 before
creating a record, as expected from its documented owner workflow. The owner
path was then exercised with draft advisory `GHSA-5qph-7jv3-m93c`:

- summary clearly began `TEST ONLY`;
- description stated that it was not a vulnerability;
- affected-products list was empty;
- no sensitive data or credential was supplied;
- no temporary private fork was created;
- the record was never published; and
- it was immediately closed at `2026-07-27T18:33:23Z`.

This verifies private owner intake and close handling without claiming an
external reporter identity or a real affected package.

## Deterministic red-before-policy evidence

On exact issue base `9af7ee4ce29802283de823971aecd4232e180d33`,
`cargo test --locked --test issue_69_security_policy -- --nocapture` exited
101. Its three tests failed because `SECURITY.md`, the response runbook, the
hosted-control record, and this audit did not exist. The test reads fixed
repository files only.

## Claim-to-evidence review

| Policy claim | Existing implementation evidence | Disposition |
| --- | --- | --- |
| Issue #35 untrusted-repository/trusted-host boundary | `docs/v0.2-contract.md` and `tests/fixtures/v0_2_trust.rs` | Published without expansion |
| Issue #49 safe guide opening, link refusal, and target redaction | `trust-guide-default-link-outside-relative`, `trust-guide-explicit-final-link-outside-root`, CLI guide-input tests, and the three-platform issue #55 suite | Published as stable-tree behavior |
| Issue #51 containment, observed-change failure, and hostile-replacement limit | `trust-containment-target-redaction`, `trust-containment-observed-identity-change`, `trust-containment-hostile-replacement`, and binary containment tests | No sandbox or hostile-replacement claim |
| Issue #61 workflow boundary | `workflow_lint_is_fail_closed_and_checksum_pins_every_tool`, explicit permissions/timeouts, immutable action SHAs, and least-privilege release jobs | Dependency, workflow, and release reports are in scope |

The security policy additionally preserves the exact resource limits from the
normative contract: depth 256, indentation 16, and no fixed byte, line, width,
or discovered-guide quota. It recommends host time, memory, and filesystem
limits instead of misrepresenting performance tests as denial-of-service
protection.

## Maintainer tabletop

The following tabletop exercised the complete response sequence without a live
vulnerability:

1. **Intake:** receive a private GitHub report, acknowledge on a best-effort
   target, keep sensitive details in the advisory, and assign `plx` as owner.
2. **Severity:** reproduce with synthetic data, compare the report with the
   documented trust boundary, record prerequisites and affected versions, and
   use CVSS only when useful.
3. **Private patch:** create a deterministic regression and smallest fix on a
   local private branch or advisory temporary private fork; do not expose it
   in a public pull request.
4. **Advisory:** prepare GHSA impact, ranges, mitigation, credit, CWE/CVSS, and
   a CVE request only when applicable.
5. **Release:** pass the ordinary immutable-tag, package, artifact,
   provenance, protected-environment, and Trusted Publishing gates. Security
   urgency does not bypass issue #63 or the issue #71 expiry.
6. **Disclosure:** verify patched artifacts, publish the advisory on the
   coordinated date, notify the reporter and affected downstream users, and
   record a sanitized retrospective.

## Recorded gaps

- One person owns intake, remediation, approval, and disclosure; there is no
  independent recovery or response path.
- Response dates are best-effort targets rather than an SLA.
- The reporter-facing submission path cannot be tested by the owner as an
  external identity; enabled status plus the owner draft/close workflow is the
  available harmless validation.
- Real embargoed collaboration and a temporary private fork were not created
  because there is no vulnerability or second collaborator.
- Issue #63's exact crates.io Trusted Publisher remains pending, so any real
  patched release remains blocked until that separate release criterion is
  satisfied.

## Local validation

The final issue branch passed:

- `GUIDE_FORMAT_REQUIRE_CONFORMANCE=all cargo test --workspace --all-targets
  --all-features --locked -- --nocapture`, including all three issue #69
  policy tests;
- the explicit packaged-artifact acceptance test and `cargo package --locked
  --allow-dirty`;
- `cargo fmt -- --check`, locked all-target/all-feature `cargo check`, and
  Clippy with warnings denied;
- the exact CI `rumdl` and `lychee` commands, with 86 links checked, 56 unique,
  zero errors, and two redirects;
- `actionlint`, pedantic `zizmor`, and navigation-guide verification; and
- a repository diff whitespace check.

The first post-policy full-suite attempt exposed an existing macOS concurrency
test with two `Bad file descriptor` results in one output-creation iteration.
That test passed immediately in isolation, and the complete unchanged suite
then passed. No runtime source or unrelated test was modified in response.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Tested private route and clear version policy | Hosted setting enabled; closed unpublished test advisory; `SECURITY.md` binds support to prepared `0.2.0`, future newest `0.2.x`, and unsupported `0.1.x` |
| Accurate trust, containment, link, diagnostic, and resource model | Claim-to-evidence table maps the policy to issues #35, #49, #51, and their platform regressions |
| Feasible triage and coordinated release | Public runbook plus tabletop covers intake through follow-up and records sole-maintainer stops |
| GitHub recognizes policy and current identity is covered | Root `SECURITY.md` is the recognized policy location; deterministic test requires the exact `release/identity.toml` version; the live default-branch surface must be checked immediately after merge |
| No placeholder, impossible SLA, or unverified claim | Real GitHub route, best-effort targets, explicit gaps, and deterministic negative assertions |

GitHub cannot surface a pull-request-only `SECURITY.md` as the repository
policy before it reaches the default branch. After merge, issue closure
requires a read of GitHub's community-profile/security-policy surface and a
public issue note recording the result.
