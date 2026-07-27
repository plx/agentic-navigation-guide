# Vulnerability response runbook

This is the public, non-secret maintainer procedure for
`plx/agentic-navigation-guide`. The project is a personal repository with a
single maintainer and no backup owner under the
[dated continuity exception](https://github.com/plx/agentic-navigation-guide/blob/main/docs/maintainer-continuity.md).

## Private intake and record ownership

1. Keep the report in its GitHub Security Advisory. Do not copy sensitive
   evidence into an issue, ordinary pull request, workflow log, commit message,
   changelog draft, or public planning document.
2. `plx` owns the private advisory, attachments, reporter communication, and
   any related private incident notes. The stable public label for that record
   is the GHSA identifier; private contents are not mirrored into git.
3. Confirm receipt, remove accidentally supplied credentials from working
   copies, ask the reporter to rotate any exposed secret, and identify the
   minimum safe reproducer.
4. Record the affected product, source commit or version, platforms,
   prerequisites, suspected boundary, reporter credit preference, and proposed
   disclosure constraints.
5. If GitHub or maintainer authority is uncertain, stop. The single maintainer
   cannot perform independent recovery, and publication stops until authority
   is restored.

## Severity and scope decision

Classify the finding against the documented threat model before choosing
severity:

- confirm whether the input is inside the supported untrusted-repository model
  or relies on an explicitly excluded hostile-host condition;
- distinguish confidentiality, integrity, availability, containment,
  provenance, dependency, workflow, and reporting-route impact;
- identify required privileges, user interaction, platform capabilities, and
  whether exploitation crosses a documented trust anchor;
- reproduce against the exact candidate or published artifact without using
  real secrets; and
- use CVSS when it clarifies a real vulnerability, while recording material
  assumptions that the score does not express.

Low, moderate, high, or critical labels guide urgency but do not replace the
affected-version analysis. A product bug outside the security boundary may be
closed privately and moved to a sanitized public issue.

## Embargoed fix

1. Preserve the private reproducer and write a deterministic regression that
   fails for the claimed boundary. Do not add fuzzing or generated hostile
   input merely because the report is security-related.
2. Use the advisory's temporary private fork when collaboration or hosted
   review is necessary. Otherwise use a private local branch. Never put an
   embargoed patch in the public repository before the disclosure decision.
3. Keep credentials out of fixtures, commits, patches, command output, and
   workflow logs. Synthetic paths and data must replace reporter material.
4. Validate the smallest correction against the full relevant platform,
   package, workflow, and release gates. A v0.2 security correction may restore
   the documented boundary but must not silently redefine it.
5. Reassess severity and affected versions after the fix. Invite the reporter
   to verify privately when practical.

## Release and advisory coordination

1. Prepare the advisory with summary, impact, affected and fixed versions,
   mitigations, CWE and CVSS data when useful, and consented credit.
2. Request a CVE through GitHub only when a real publishable vulnerability
   warrants one. The GHSA remains the primary private coordination record.
3. Add a concise `CHANGELOG.md` entry that does not expose exploit details
   before the embargo ends.
4. Run the ordinary fail-closed release pipeline. Do not move or reuse a tag,
   bypass the protected environment, replace immutable artifacts, or introduce
   a long-lived publication token.
5. Create the patched release and verify its checksums, SBOM, provenance,
   attestations, immutable GitHub Release, and registry artifact before
   announcing availability.
6. If Trusted Publishing, protected release authority, or the sole-maintainer
   exception is unavailable, publication stops. Security urgency does not
   authorize weakening those gates.

## Disclosure and follow-up

After patched artifacts are independently visible:

1. publish the GitHub Security Advisory on the coordinated date;
2. notify the reporter and accept credit decisions;
3. link the GHSA or CVE from the release notes and identify exact fixed
   versions;
4. notify materially affected downstream users or maintainers through an
   appropriate non-secret channel;
5. close private fix branches or the temporary private fork only after
   retaining necessary advisory evidence;
6. review whether dependencies, workflows, tests, documentation, or the
   reporting route need follow-up; and
7. record a sanitized retrospective, including missed response targets and
   changes to this runbook.

Yanking a crate is a separate ecosystem decision. It does not replace a fixed
release or advisory and must not be represented as deleting an immutable
artifact.
