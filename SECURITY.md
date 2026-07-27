# Security Policy

## Supported versions

There is no currently published release with a security-fix commitment.
Support follows the same CLI compatibility line and release identity as the
[v0.2 contract](docs/v0.2-contract.md#supported-product-versions).

| Version | Security support |
| --- | --- |
| `0.2.0` (prepared, not yet published) | Candidate fixes only; it is not a supported registry release |
| newest non-yanked `0.2.x` release after publication | Best-effort fixes |
| `0.1.4` and every other `0.1.x` release | Unsupported |

After the first v0.2 release, only the newest non-yanked `0.2.x` patch is
supported unless an advisory or release note states a narrower affected and
fixed range. There is no long-term-support line or guaranteed support overlap.
This table is updated in the same reviewed change that alters
[`release/identity.toml`](https://github.com/plx/agentic-navigation-guide/blob/main/release/identity.toml),
publishes the first v0.2 release, or changes the support policy.

## Report a vulnerability privately

Use
[GitHub private vulnerability reporting](https://github.com/plx/agentic-navigation-guide/security/advisories/new).
That route creates a private report visible to the repository maintainer. Do
not open a public GitHub issue, discussion, or pull request for a suspected
vulnerability. Do not send exploit details, secrets, personal data, private
repository contents, or embargoed findings through a public channel.

The repository has one maintainer, `plx`, and no backup owner. If the
maintainer is unavailable or repository authority is uncertain, there is no
independent project contact who can receive or release a fix. Publication
stops until authority is recovered.

## What to include

Provide the smallest safe report that lets the maintainer reproduce and assess
the issue:

- the installed version, source commit, and installation method;
- operating system, filesystem type when relevant, and Rust version for source
  builds;
- the exact command shape and relevant option names, with tokens and private
  values removed;
- expected behavior, observed behavior, security impact, and prerequisites;
- a minimal reproducer or test fixture that contains no real secret or personal
  data;
- whether exploitation is known or only theoretical;
- any suggested affected or fixed version range; and
- your preferred credit and coordinated-disclosure constraints.

Replace sensitive absolute paths, usernames, guide text, repository names, and
filesystem contents with stable labels. Attach sensitive evidence only inside
the private advisory. If a path spelling itself is essential, provide the
minimum synthetic equivalent rather than a real private path.

Dependency, GitHub Actions, release-pipeline, artifact-provenance, reporting
route, parser, filesystem, and CLI findings are all in scope when they affect
this repository or its distributed artifacts. Ordinary product bugs without a
security impact belong in a public issue after removing sensitive data.

## Response and coordinated disclosure

The following are best-effort targets, not service-level guarantees:

- human acknowledgement within seven calendar days;
- an initial scope and severity update within fourteen calendar days; and
- an update at least every fourteen calendar days while active investigation
  continues.

Complexity, maintainer availability, platform access, and release authority
may make those targets impossible. The maintainer will say so in the private
advisory when able. Acknowledgement does not confirm a vulnerability.

Please keep the report private while impact and a feasible fix are assessed.
The maintainer and reporter should agree on a disclosure date that accounts
for a patched release and downstream notification. No indefinite embargo is
requested: if coordination stalls, the reporter retains control of their
eventual disclosure and is asked to give reasonable private notice first.

When applicable, the project publishes a GitHub Security Advisory, requests a
CVE, credits reporters who consent, identifies affected and fixed versions,
adds a `CHANGELOG.md` entry, and ships a release through the ordinary
fail-closed release process. See the
[vulnerability response runbook](docs/security-response-runbook.md).

## Security model and limitations

The supported threat model is an untrusted repository on a trusted developer
or CI host. Guide text, filesystem names, links, link targets, and repository
contents may be hostile. The selected executable, operating system, kernel,
process credentials, command-line arguments, and configuration environment
are trusted. Callers must validate arguments or environment values derived
from untrusted content.

On a supported stable filesystem tree, the CLI:

- refuses final guide links and link or reparse components below a canonical
  trust anchor;
- contains implicit guide reads and item verification to their documented
  anchors while allowing caller-selected explicit external regular paths;
- observes identity changes and fails instead of treating them as success;
- creates filesystem outputs exclusively without following or overwriting an
  existing destination; and
- emits control-safe diagnostics that omit raw guide source lines and resolved
  external targets.

Diagnostics may still reveal a bounded source location, a control-safe logical
path relative to its anchor, or an explicit guide path exactly as configured
by the caller. Do not pass private path values if even that caller-visible
spelling must remain secret.

The CLI is not a sandbox, access-control boundary, malware scanner, or safe way
to execute an untrusted process; in particular, it is not a filesystem
sandbox. It does not guarantee containment against hostile concurrent
replacement, every kernel or filesystem defect, network or userspace
filesystems, atomic content publication, or crash durability.

Guide and generation depth are bounded at 256, and indentation is bounded at
16 spaces. There is no fixed input-byte, line-count, tree-width, or discovered
guide quota. Work and memory scale with accepted input and included filesystem
state. For adversarially large inputs, apply operating-system time, memory, and
filesystem-access limits appropriate to the host.

The complete normative boundary is
[Security and vulnerability reporting](docs/v0.2-contract.md#security-and-vulnerability-reporting).
