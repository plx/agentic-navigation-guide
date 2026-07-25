# Post-Remediation Production-Readiness Audit Playbook

## Purpose

This playbook defines the independent audit that must be performed after the
release-readiness remediation backlog is complete and before
`agentic-navigation-guide` is represented as suitable for production use
outside toy, hobby, or experimental projects.

The audit is not a checklist confirming that issues were closed. It must
independently test the release candidate, reproduce the original failure modes,
look for new failure modes, and evaluate the crate, CLI, public API,
distribution artifacts, and release process as one product.

The static documentation site and landing page are out of scope. Their source,
design, build, and deployment must not affect the verdict. Package-boundary
checks remain in scope: site files must not leak into the crate or binary
release artifacts unless explicitly justified.

Normative terms such as **MUST**, **MUST NOT**, **SHOULD**, and **MAY** describe
audit requirements.

## Production-readiness standard

The central safety property is:

> The tool must not report successful verification when a guide was
> interpreted differently from what was written, when the represented
> filesystem item is absent or of the wrong type, when no required guide was
> checked, or when validation was unintentionally bypassed.

A production-ready result additionally requires:

- documented behavior that agrees with implementation;
- deterministic, fail-closed behavior on malformed and adversarial input;
- safe handling of filesystem boundaries, symlinks, output files, and errors;
- tested operation on every supported platform and Rust version;
- a deliberate and supportable public API;
- bounded performance for documented workloads;
- accurate licensing and package metadata;
- reproducible, reviewable release procedures; and
- no unresolved critical or high-severity findings.

Passing the existing unit tests alone is not sufficient.

## Audit outcomes

The final verdict must be exactly one of:

- **PASS — production-ready:** every hard gate passes on the exact release
  candidate, with no unresolved critical or high finding.
- **CONDITIONAL PASS — releaseable but not production-ready:** ordinary release
  may be reasonable, but specified residual risks prevent recommending it as a
  work-enforced or load-bearing gate.
- **FAIL — not release-ready:** one or more correctness, security,
  compatibility, packaging, or release hard gates fail.

A failed gate may not be waived merely because the affected case is uncommon.
Any exception must identify affected users, impact, mitigation, owner, and
expiration date. Correctness false positives, unintended validation bypasses,
external-file disclosure, and destructive output behavior are not waivable for
a production-ready verdict.

## 1. Freeze and identify the candidate

Audit one immutable commit. Do not audit a moving branch.

Record:

- candidate commit SHA;
- proposed version;
- proposed tag;
- upstream repository URL;
- previous published version and tag;
- audit date and auditor;
- whether the auditor authored any remediations;
- working-tree status;
- submodule status, if applicable;
- operating systems, architectures, filesystem types, and case sensitivity
  used;
- Rust, Cargo, and audit-tool versions; and
- all deviations from this playbook.

Run from a fresh clone:

```bash
git status --short
git rev-parse HEAD
git rev-parse --verify HEAD^{commit}
git fsck --full
git log -1 --show-signature
rustc -Vv
cargo -V
cargo metadata --locked --format-version 1
```

Hard gates:

- The working tree is clean.
- The candidate commit is the commit that will be tagged.
- `Cargo.toml`, the proposed tag, CLI `--version`, changelog, and release notes
  use the same version.
- The lockfile is committed and accepted by every declared supported Cargo
  version.
- Any candidate change after testing invalidates affected evidence and requires
  those gates to be rerun.

## 2. Create an evidence record

Store logs outside the checkout or as CI artifacts so evidence collection
cannot make the candidate dirty. A suggested layout is:

```text
production-audit/
  manifest.md
  environments/
  build/
  tests/
  adversarial/
  coverage/
  mutation/
  fuzz/
  performance/
  security/
  package/
  release/
  final-report.md
  SHA256SUMS
```

For every command, capture:

- exact command;
- current commit;
- directory;
- environment variables that affect behavior;
- start and end time;
- exit status;
- complete stdout and stderr;
- tool version;
- platform and filesystem; and
- any manual interpretation.

On POSIX systems, enable `set -o pipefail` before piping commands through
`tee`; otherwise a failed command may appear successful. On Windows, record
`$LASTEXITCODE` after native commands.

At the end, hash the evidence files:

```bash
find production-audit -type f -print0 \
  | sort -z \
  | xargs -0 shasum -a 256 \
  > production-audit/SHA256SUMS
```

Use `sha256sum` instead of `shasum -a 256` where appropriate.

## 3. Verify remediation traceability

Before testing the result, review the top-level remediation epic and every
linked issue.

For every individually addressed finding, confirm:

- the issue states the prior behavior, intended behavior, scope, and acceptance
  criteria;
- dependencies were resolved in the declared order or consciously revised;
- the implementing PR links the issue;
- behavior and documentation changed together when user-visible;
- a regression test covers the exact defect;
- the issue was not closed solely by documentation unless the behavior was
  intentionally retained and safe; and
- no temporary bypass, ignored test, broad lint allowance, or platform skip
  hides the finding.

For each original critical or high finding, preserve evidence that its
regression test would fail without the fix. Prefer one of:

1. the test was added before or with the fix, and CI evidence shows it failing
   on the pre-fix implementation;
2. the test patch can be applied to the pre-fix parent in a temporary worktree
   and demonstrably fails there; or
3. if refactoring makes that impossible, the issue contains an independent
   reproducer captured before remediation and the auditor reruns an equivalent
   black-box reproducer against both revisions.

The release candidate must pass the same test. A test that passes both before
and after the purported fix is not adequate evidence.

Produce a traceability table:

| Issue | Sev. | PR | Regression | Red | Green | Docs | Audit |
| --- | --- | --- | --- | --- | --- | --- | --- |

## 4. Baseline build and quality gates

Run all commands with the committed lockfile:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" \
  cargo doc --workspace --all-features --no-deps --locked
cargo audit --file Cargo.lock
cargo tree --workspace --all-features --locked
cargo tree --workspace --all-features --locked --duplicates
```

If the repository intentionally has no features or workspace members, retain
the flags so future additions remain covered.

Hard gates:

- Every command exits zero.
- There are no ignored or conditionally skipped correctness tests without
  documented justification.
- There are no known unmitigated RustSec vulnerabilities.
- No unreviewed Git, path, prerelease, or yanked dependency appears.
- Duplicate dependency versions have been reviewed for security and binary-size
  impact.
- Warnings are not suppressed globally to obtain a green result.

Also build and test in both debug and release modes:

```bash
cargo build --workspace --all-targets --all-features --locked
cargo build --workspace --all-targets --all-features --locked --release
cargo test --workspace --all-targets --all-features --locked --release
```

Release-mode tests are necessary because overflow and allocation behavior can
differ from debug builds.

## 5. Rust-version and platform matrix

`Cargo.toml` must declare `rust-version`. Treat it as a tested contract, not
metadata decoration.

Test at least:

- declared MSRV;
- current stable Rust;
- current stable minus one, if supported; and
- current beta as an informational forward-compatibility signal.

For the MSRV and current stable:

```bash
cargo +<toolchain> check --workspace --all-targets --all-features --locked
cargo +<toolchain> test --workspace --all-targets --all-features --locked
cargo +<toolchain> clippy \
  --workspace --all-targets --all-features --locked -- -D warnings
cargo +<toolchain> install --path . --locked --root <temporary-install-root>
```

Run the complete behavioral suite on actual:

- Linux, on a case-sensitive filesystem;
- macOS, on the default case-insensitive filesystem and, if supported, a
  case-sensitive APFS volume; and
- Windows, on NTFS.

Compilation-only jobs are not sufficient for this filesystem-oriented tool.

The matrix must include symlink-capable environments. On Windows, enable the
required developer-mode or privilege configuration. A skipped Windows symlink
test is inconclusive and must not be reported as a pass.

Record:

- architecture;
- filesystem;
- case behavior;
- path-length configuration;
- locale;
- timezone;
- shell; and
- symlink availability.

Hard gates:

- All supported-platform behavioral tests pass.
- Platform-specific differences are documented and intentional.
- MSRV commands resolve strictly from the committed lockfile.
- The package does not require a newer Rust edition or compiler through an
  unlocked transitive dependency than the published MSRV permits.

## 6. Coverage and test effectiveness

Generate coverage from a clean state:

```bash
cargo llvm-cov clean --workspace
cargo llvm-cov \
  --workspace \
  --all-targets \
  --all-features \
  --locked \
  --html \
  --output-dir production-audit/coverage/html

cargo llvm-cov \
  --workspace \
  --all-targets \
  --all-features \
  --locked \
  --lcov \
  --output-path production-audit/coverage/lcov.info
```

Collect branch coverage where the installed toolchain supports it.

Minimum expectations:

- overall line coverage is at least 85%;
- parser, dumper, validator, verifier, and recursive-discovery modules are each
  at least 85%;
- critical branches introduced or changed by remediation are covered;
- every original critical/high finding has an exact regression test;
- public library convenience functions are not left at zero coverage;
- error and cleanup paths receive deliberate tests; and
- coverage has not fallen below the accepted post-remediation baseline.

Coverage percentages are supporting evidence, not proof of correctness.

Run targeted mutation testing against correctness-critical modules:

```bash
cargo mutants \
  --file src/parser.rs \
  --file src/dumper.rs \
  --file src/validator.rs \
  --file src/verifier.rs \
  --file src/recursive.rs
```

Review every surviving mutant. A mutant that can remove or invert a regression
check for an original blocker without failing a test is a hard failure. Other
surviving mutants must be classified and either covered or explicitly
justified.

## 7. Original-defect recurrence suite

Independently rerun every original defect, even if an automated test exists.

### 7.1 Hierarchy reparenting

Create:

```text
root/
  a/
    c
  b
  GUIDE.md
```

with:

```markdown
<agentic-navigation-guide>
- a/
- b
  - c
</agentic-navigation-guide>
```

Both `check` and `verify` must reject the impossible hierarchy according to the
documented phase responsible for it. `verify` must not attach `c` to `a`, and
must not exit zero.

Also test:

- a child directly under a file;
- a child after one or more intervening files;
- indentation that skips a level;
- dedenting and re-indenting among siblings;
- tabs and mixed indentation;
- zero indentation width; and
- very deep nesting.

### 7.2 Dumper/parser round-trip

Construct a temporary filesystem corpus containing, where the platform permits:

- ordinary files and nested directories;
- `report#draft`;
- `Foo[bar].txt`;
- a file named exactly `...`;
- commas, quotes, and backslashes;
- leading and trailing spaces;
- embedded newline characters;
- precomposed and decomposed Unicode names;
- emoji and non-Latin names;
- file symlinks, directory symlinks, and dangling symlinks;
- FIFO or another special entry on Unix; and
- empty directories.

For every name and entry type, the documented contract must select exactly one
safe behavior:

1. `dump` emits syntax that `check` accepts and `verify` resolves to the
   original filesystem item without loss; or
2. `dump` rejects the unsupported item with a specific nonzero error and emits
   no misleading partial guide.

Lossy normalization, comment interpretation, choice expansion, type conversion,
silent omission, and successful verification of a different path are failures.

For supported corpora, test this invariant:

```text
filesystem → dump → check → verify = success against the same filesystem
```

Run it for both `dump` and `init`, with output placed outside the enumerated
root. Then mutate the filesystem by removing, renaming, and changing the type
of represented entries; verification must fail.

### 7.3 Marker and attribute parsing

Test exact valid markers and at least:

```text
<agentic-navigation-guideignore=true>
<agentic-navigation-guideTYPO>
<agentic-navigation-guide ignore=truthy>
<agentic-navigation-guide unknown=true>
<agentic-navigation-guide
```

Also test missing closing markers, duplicate blocks, nested blocks, text
resembling a marker, extra closing markers, unusual whitespace, CRLF input, and
attributes in every documented quoting form.

Malformed markers must never activate `ignore`, create an empty guide, or
result in successful verification. Unknown attributes must follow the
documented policy consistently.

### 7.4 Empty recursive discovery

Run recursive verification against a directory containing no matching guide.

The default must be nonzero and clearly state that zero guides were verified.
If an explicit `--allow-empty` or equivalent option exists, verify that only
the explicit option permits success and that output still reports a zero count.

Repeat with:

- a misspelled guide name;
- the wrong root;
- all guides excluded;
- the last guide removed;
- quiet mode; and
- every execution mode.

### 7.5 Ignored-guide semantics

Test a deliberately ignored, structurally valid guide containing otherwise
invalid list content.

The result must match one documented contract across:

- `check`;
- single-guide `verify`;
- recursive `verify`;
- public library entry points; and
- default, quiet, hook, and GitHub Actions modes.

A deliberately ignored guide must be visible in non-quiet summaries.
CI-oriented usage must have a documented way to reject ignored guides if
ignored files are unacceptable. Malformed syntax in the opening marker itself
must never be ignored.

### 7.6 Trailing separators and quoted choices

Test:

- `foo/`, `foo//`, and `foo///`;
- empty components in the middle of paths;
- quoted choices containing leading/trailing spaces;
- escaped comma, space, `#`, `[`, `]`, `"`, and `\`;
- empty choice alternatives;
- malformed/unclosed choices; and
- more than one choice list.

The parser must preserve every character the documented syntax says is
significant and reject forbidden empty components before normalization hides
them.

### 7.7 Placeholder and case behavior

Test commented and uncommented placeholders in:

- empty directories;
- fully listed directories;
- directories with exactly one unlisted entry;
- nested directories;
- adjacent placeholder positions; and
- directories containing case-only name differences.

On case-insensitive systems, create `Readme.md` and list `README.md`; confirm
placeholder accounting uses the same identity rules as filesystem verification.
It must not produce a false positive by treating one name as both listed and
unlisted.

Document behavior on case-sensitive systems and for Unicode normalization
differences. Do not claim portable equivalence the implementation does not
provide.

## 8. Parser and verifier adversarial testing

Build a table-driven corpus covering:

### Structure

- empty block;
- only whitespace;
- blank line inside block;
- very long line;
- many flat siblings;
- extreme nesting;
- inconsistent indentation;
- children of files, symlinks, and placeholders;
- duplicate entries and duplicate expansions; and
- comments with all escape combinations.

### Paths

- empty path;
- `.`, `..`, and repeated separators;
- absolute POSIX paths;
- Windows drive-absolute, drive-relative, rooted, and UNC paths;
- separator mixtures;
- parent traversal hidden by normalization;
- NUL in API-level strings;
- non-UTF-8 filesystem entries where the platform permits; and
- long paths near platform limits.

### Filesystem type matching

- guide file vs actual directory;
- guide directory vs actual file;
- symlink-to-file and symlink-to-directory;
- dangling symlink;
- symlink loop;
- symlink whose target changes during verification;
- special device, FIFO, or socket entries;
- unreadable entries; and
- entries removed during traversal.

For each case, record expected parse result, expected semantic result, exit
status, stdout, and stderr. No case may panic, hang, read outside the documented
root, or succeed after reinterpreting the input.

## 9. Filesystem-boundary and security audit

Begin by writing down the supported threat model. At minimum, evaluate an
untrusted repository on a trusted developer or CI machine. Do not assume
repository symlinks or guide content are benign.

### 9.1 External guide symlink disclosure

Create a readable file outside the root whose first line contains a unique
sentinel, such as:

```text
AUDIT_SECRET_MUST_NOT_APPEAR_9B4B...
```

Inside the root, create a guide-name symlink to that file. Exercise recursive
discovery, default discovery, and any explicit-guide path allowed by the
documented policy.

Required outcome:

- implicit/recursive discovery does not follow an out-of-root guide symlink;
- the sentinel never appears in stdout, stderr, logs, annotations, or
  summaries;
- the command returns a specific nonzero error unless the documented
  explicit-path policy intentionally permits the input; and
- even an allowed explicit path must not disclose unrelated source lines in
  error output.

Search all evidence for the sentinel.

### 9.2 Root containment

Test guide entries that reach outside the root through:

- `..`;
- absolute paths;
- existing symlink ancestors;
- dangling symlink ancestors;
- a symlink followed by a nonexistent final component;
- symlink chains and loops; and
- race-time replacement of a checked directory with a symlink, where
  practical.

No external item may satisfy an in-root guide entry. Errors must identify the
unsafe guide path without exposing external file content.

Document whether containment assumes a stable filesystem. If concurrent hostile
mutation remains out of scope, state that explicitly in user-facing security
documentation; do not describe the implementation as a sandbox.

### 9.3 Output-file safety

For `init` and any `dump --output` path, test:

- existing regular file;
- existing directory;
- existing symlink;
- dangling symlink to an absent external target;
- symlink to an existing external target;
- read-only parent; and
- output path created concurrently after the command begins.

Required outcome:

- `init` never overwrites or follows an existing or dangling output symlink;
- an external symlink target is neither created nor modified;
- create-new behavior is one atomic filesystem operation;
- failures leave no partial file;
- any overwrite behavior offered by `dump` is explicit, documented, and
  separately tested; and
- race stress does not overwrite a competing creator's file.

Repeat the create race enough times to exercise timing—at least 100 iterations
on each supported Unix platform—and retain the test harness.

### 9.4 Error confidentiality

Review all formatted errors and logs. They must not expose:

- external file contents;
- environment secrets;
- authentication tokens;
- arbitrary first lines from files discovered through symlinks; or
- absolute paths unnecessarily in privacy-sensitive modes.

Intentionally place sentinel values in external files and relevant environment
variables, trigger every error mode, and scan captured output.

### 9.5 Resource exhaustion

Test:

- a guide with at least 100,000 flat entries;
- deeply nested guides up to and beyond the documented limit;
- a very long single line;
- a large choice expansion;
- many placeholders in one directory;
- symlink cycles;
- very large directories; and
- malformed escape sequences designed to force rescans.

The tool must remain within documented time and memory limits, terminate
without stack overflow or panic, and reject any configured limit with a clear
error.

## 10. Recursive discovery audit

Create a monorepo fixture with guides at multiple depths and excluded
directories at both root and nested levels.

Verify:

- each guide is resolved relative to its own parent;
- the result order is deterministic;
- custom guide names work through both CLI and documented environment
  configuration;
- exclusion patterns have exactly the documented matching semantics at every
  depth;
- default VCS exclusions work at every depth;
- directory symlink loops are not traversed;
- file symlinks are handled by the security policy;
- permission and traversal errors are not silently swallowed;
- one bad guide makes the aggregate command fail;
- all failures remain visible rather than stopping after the first unless
  fail-fast is documented;
- summary counts distinguish passed, failed, ignored, and absent guides; and
- quiet mode changes chatter, not correctness or exit status.

Test a tree that changes during enumeration. The result may report a race, but
it must not panic or claim complete success after silently losing required
work.

## 11. CLI contract audit

Capture:

```bash
agentic-navigation-guide --help
agentic-navigation-guide --version
agentic-navigation-guide dump --help
agentic-navigation-guide init --help
agentic-navigation-guide check --help
agentic-navigation-guide verify --help
```

Compare help, README, examples, environment variables, and actual behavior.

Build a matrix for:

- explicit arguments;
- environment defaults;
- explicit arguments overriding environment values;
- conflicting convenience flags;
- omitted optional arguments;
- malformed and extreme numeric arguments;
- Unicode argument values;
- every execution mode; and
- quiet, default, and verbose logging.

Verify at least these exit-code classes:

| Scenario | Expected |
| --- | ---: |
| Successful check/verify | 0 |
| Syntax error | documented nonzero |
| Filesystem mismatch | documented nonzero |
| I/O error | documented nonzero |
| Zero recursively discovered guides | documented nonzero by default |
| Explicit allow-empty | 0 |
| Post-tool-use failure | documented hook-specific status |
| GitHub Actions failure | documented CI status |
| Invalid CLI usage | Clap/documented usage status |

Also verify:

- machine-consumable output stays on the documented stream;
- errors stay on stderr;
- quiet success emits no ordinary chatter;
- quiet mode does not suppress required errors;
- verbose mode does not disclose sensitive content;
- GitHub Actions diagnostics use valid `path:line` or workflow-command syntax;
- paths containing spaces and Unicode are rendered unambiguously; and
- piped output handles a closed consumer without a Rust panic or exit code 101.

On Unix, exercise:

```bash
set -o pipefail
agentic-navigation-guide dump --root <large-tree> | head -c 1 >/dev/null
```

A normal broken-pipe status or documented signal status is acceptable; a panic
message or backtrace is not.

## 12. Public Rust API audit

First decide whether the Rust library is a supported product surface. The
decision must be stated in README/API documentation and release policy.

If supported:

- build a separate temporary consumer crate against the packaged candidate;
- compile every documented example;
- test parsing, validation, verification, ignore behavior, nested full-path
  calculation, error handling, and configuration through public APIs;
- confirm public functions behave consistently with the CLI;
- inspect public exhaustive enums and structs for accidental SemVer
  commitments;
- use `#[non_exhaustive]`, constructors, or private fields where evolution is
  intended;
- ensure every public item has accurate rustdoc; and
- ensure inaccessible or unconstructible concepts, such as symlink variants not
  supported by the grammar, are removed or coherently implemented.

Run SemVer analysis against the most recent non-yanked published baseline:

```bash
cargo semver-checks check-release --baseline-version <previous-version>
```

Classify every reported change. The proposed version must legally encode all
supported API changes.

If the library is not supported, implementation modules and unfinished types
must not remain publicly exposed by accident. Confirm a downstream crate cannot
depend on internals that the project claims are private.

## 13. Fuzzing and property testing

The post-remediation repository should contain persistent property tests for
the core invariant and fuzz targets for untrusted text parsing.

At minimum, require targets for:

- parser: arbitrary UTF-8 input must not panic or hang;
- marker/attribute parser;
- choice and escape parser;
- parse/serialize or dump/parse round-trip where defined;
- verifier path handling; and
- filesystem-name corpus generation.

Run each fuzz target on the release candidate for at least 30 minutes on Linux
with sanitizers:

```bash
cargo fuzz list
cargo +nightly fuzz run <target> -- \
  -max_total_time=1800 \
  -timeout=10 \
  -rss_limit_mb=2048
```

Retain corpus, crash, timeout, and exact seed information. Rerun any historical
regression corpus.

Hard gates:

- no crash, panic, sanitizer finding, timeout, or uncontrolled memory growth;
- every discovered failure becomes a minimized permanent regression test;
- corpus files do not contain secrets; and
- fuzz coverage reaches marker, choice, hierarchy, placeholder, and escape
  branches.

Where feasible, run:

```bash
cargo +nightly miri test --lib
```

Any Miri failure in project code must be resolved. An unsupported dependency
must be recorded rather than described as a pass.

## 14. Performance audit

Use the optimized binary, fixed fixtures, an otherwise idle machine, and a
benchmarking tool such as `hyperfine`. Record CPU, RAM, OS, filesystem, Rust
version, and binary hash.

Benchmark at least:

1. flat guides with 10k, 20k, 40k, and 100k entries;
2. deeply nested valid guides;
3. directories with 500, 1k, and 2k entries plus alternating placeholders;
4. recursive discovery across many small repositories;
5. dumping a large tree; and
6. ordinary self-verification of this repository.

Use warmup runs and at least ten measured runs. Capture median, p95, maximum
RSS, and output size.

Default hard thresholds, unless a stricter documented service level exists:

- doubling a flat guide must not increase median time by more than 2.5 times
  over adjacent sizes;
- doubling placeholder workload must not increase median time by more than 2.5
  times;
- 100,000 flat entries must verify in under five seconds and under 256 MiB RSS
  on a current standard hosted CI runner;
- this repository's normal guide must verify in under one second;
- no benchmark may panic or exhibit unbounded recursion; and
- median time and RSS must not regress by more than 20% from the accepted
  post-remediation baseline without explicit analysis.

Performance failures do not become acceptable merely because ordinary guides
are small if the CLI documents support for generated or large trees.

## 15. Dependency, workflow, and supply-chain audit

Inspect all workflow files, reusable actions, scripts, and release automation.

Run:

```bash
actionlint
zizmor --pedantic .github/workflows/
cargo audit --file Cargo.lock
cargo about generate about.hbs --output-file <temporary-attribution-file>
git diff --no-index THIRD_PARTY_LICENSES.md <temporary-attribution-file>
```

If secret scanning is part of the project's release policy, run it over both
the candidate and relevant history, for example:

```bash
gitleaks detect --redact --no-banner
```

Manually verify:

- every third-party GitHub Action is pinned to a full reviewed commit SHA;
- action tags appear only as review comments, not executable references;
- checkout credentials are not persisted unless required;
- permissions are explicitly minimal per job;
- token-bearing jobs cannot execute untrusted pull-request code;
- `id-token: write` exists only where OIDC publishing requires it;
- jobs have reasonable timeouts and concurrency controls;
- release publishing uses a protected environment and, preferably, crates.io
  Trusted Publishing;
- no long-lived publishing token is printed or exposed;
- dependency update automation cannot publish;
- release tags cannot be silently moved; and
- maintainers have a 2FA and ownership-recovery plan.

Any high-confidence workflow finding involving token exposure, mutable
execution, or untrusted code is a hard failure.

## 16. License and provenance audit

Verify consistency among:

- `Cargo.toml` SPDX expression;
- `README`;
- `LICENSE-MIT`;
- `LICENSE-APACHE`;
- `NOTICE`;
- generated third-party attribution;
- crate package contents;
- release notes; and
- historical licensing clarification for published `0.1.x` artifacts.

Confirm each dependency license is compatible with distribution. Generated
attributions must match the locked dependency graph exactly.

The prior `0.1.4` metadata/license contradiction must be explicitly
acknowledged in durable release documentation. The audit does not provide legal
advice; any unresolved ownership or licensing doubt must be escalated before
workplace recommendation.

If SBOM, provenance, or signatures are promised, generate and verify them. A
missing artifact that was never promised is a release-quality observation; a
missing or unverifiable promised artifact is a failure.

## 17. Package-boundary and installation audit

Extract the version from Cargo metadata and build the exact crate package:

```bash
cargo package --locked
cargo package --list --locked
cargo publish --dry-run --locked
```

Review every packaged path. A recommended package allowlist is limited to:

- normalized Cargo metadata and lockfile;
- `src/`;
- README and supported user documentation;
- license and notice files; and
- required attribution files.

The package must not contain:

- `site/` or landing-page source;
- `.github/`;
- agent memory/context files;
- remediation notes;
- local audit evidence;
- editor files;
- credentials or secrets;
- unrelated generated assets; or
- tests or fixtures not intentionally shipped.

Install from the unpacked package, not from the working tree:

```bash
cargo install \
  --path target/package/agentic-navigation-guide-<version> \
  --locked \
  --root <temporary-install-root>
```

Using only that installed binary, run:

- `--version`;
- every `--help`;
- a successful check and verify;
- a failing verification;
- dump/check/verify round-trip;
- recursive discovery; and
- expected exit-code modes.

Then install from each proposed binary archive in a clean environment and run
the same smoke suite.

Verify:

- archive filenames identify version, target, and architecture;
- checksums match;
- archive contents contain only intended files;
- binaries execute without build-tree dependencies;
- release binary version matches the tag;
- source archive and crates.io package correspond to the audited commit; and
- installation instructions work verbatim.

Rebuild artifacts twice in clean environments. If reproducible binaries are
claimed, hashes must match. If reproducibility is not claimed, record expected
sources of nondeterminism and still verify source/tag provenance.

## 18. Documentation audit

Treat current implementation plus accepted contract decisions as authoritative,
then verify every user-facing claim.

Audit:

- format grammar;
- escaping and choice rules;
- placeholder behavior;
- ignored-guide behavior;
- path and symlink policy;
- case and Unicode scope;
- exclusions;
- recursive zero-guide behavior;
- supported operating systems;
- MSRV;
- CLI flags, defaults, environment precedence, and exit codes;
- stdout/stderr behavior;
- security boundary and stable-filesystem assumptions;
- installation, upgrade, and uninstall;
- hook and CI examples;
- versioning/stability policy;
- public-library support;
- changelog;
- SECURITY reporting instructions;
- contribution instructions; and
- license statement.

Execute every shell command and example from a clean environment. Installation
examples must pin an intended version and use `--locked` where appropriate.
GitHub Actions examples must pin actions and install a known tool version.

Render rustdoc with warnings denied and compile code examples. Search for stale
version numbers, early-preview caveats that contradict the intended release
posture, obsolete CLI syntax, and mutable dependency examples.

The static site remains out of scope; do not treat site content or deployment
status as evidence for or against production readiness.

## 19. Release-process rehearsal

Perform a dry run without publishing or creating irreversible external state.

The release workflow must prove:

1. version and tag agree;
2. changelog contains the version and date;
3. all required gates ran on the tagged commit;
4. tests ran on every supported OS and MSRV;
5. license attribution is current;
6. SemVer analysis passed or the version correctly signals breaking changes;
7. package dry run passed;
8. source and binary artifacts came from the candidate commit;
9. checksums were generated;
10. artifacts can be installed and smoke-tested;
11. publishing requires protected maintainer approval; and
12. crates.io and GitHub releases cannot diverge silently.

For the first hardened release, verify that the intended version is at least
`0.2.0` unless the public API was restored to published `0.1.4` compatibility.
Never attempt to reuse an already published version.

If a third-party Homebrew tap is part of the release:

- the formula source URL must point to an immutable tag/archive;
- SHA-256 must match;
- dependencies must be locked;
- `brew audit --strict <formula>` must pass;
- `brew install --build-from-source <formula>` must work on supported macOS and
  Linux environments; and
- the formula test must execute meaningful success and failure behavior.

Homebrew/core acceptance or popularity thresholds are not a
production-readiness gate unless core submission is explicitly part of the
release objective.

## 20. Exploratory review

After scripted gates pass, reserve time for unscripted examination. At minimum:

- read all `src/` code without relying on issue descriptions;
- trace every success return in `check`, `verify`, recursive verification,
  `dump`, and `init`;
- identify every swallowed error and every `unwrap`, `expect`, panic, unchecked
  arithmetic operation, lossy conversion, and filesystem metadata call;
- inspect parser normalization order for information loss;
- compare all parser-produced variants with all verifier branches;
- compare every CLI path with equivalent public-library behavior;
- inspect check-then-use filesystem operations;
- search for unexpectedly quadratic loops or repeated directory scans;
- inspect all platform-conditional code;
- inspect all public API commitments; and
- examine test helpers for non-hermetic access to the repository or current
  directory.

Useful searches include:

```bash
rg -n 'unwrap\(|expect\(|panic!|unreachable!|todo!|unimplemented!' src tests
rg -n 'let _ =|\.ok\(\)|unwrap_or|unwrap_or_default' src
rg -n 'exists\(|canonicalize\(|symlink_metadata\(|metadata\(' src
rg -n 'read_to_string|read_dir' src
rg -n 'unsafe' .
rg -n 'starts_with|trim|replace|normalize|insert\(0' src/parser.rs src/dumper.rs
```

Every new finding must receive a severity, reproducer, impact statement, and
disposition before the verdict.

## 21. Final pass/fail criteria

A **PASS — production-ready** requires all of the following:

- exact original false-positive and bypass reproductions now fail safely;
- no required validation path succeeds after checking zero guides;
- dump/init either round-trip exactly or reject unsupported names and types;
- no implicit guide discovery reads outside its trust boundary;
- output creation cannot follow dangling symlinks or overwrite a racing creator;
- all supported-platform behavioral suites pass;
- MSRV and stable tests pass with the committed lockfile;
- no critical/high security or correctness finding remains;
- parser/verifier fuzzing finds no crash, timeout, or uncontrolled growth;
- performance meets the stated thresholds;
- CLI, library, README, and rustdoc agree;
- the public API and version increment are deliberate;
- package contents are minimal and correct;
- license metadata and historical clarification are coherent;
- workflow supply-chain checks pass;
- release artifacts are traceable to the audited commit; and
- all evidence is attached and reproducible.

The absence of Homebrew/core eligibility, a static website, signed binaries, or
a large user community does not by itself prevent a production-ready result
unless the project claims those properties.

## Severity rubric

- **Critical:** unintended verification success, arbitrary external read/write,
  secret disclosure, destructive overwrite, or release compromise with
  practical impact.
- **High:** common correctness failure, fail-open automation,
  supported-platform breakage, SemVer/license defect blocking safe
  distribution, or denial of service on documented workloads.
- **Medium:** bounded edge-case incorrectness, misleading diagnostics,
  meaningful performance regression, incomplete portability, or
  release-process weakness with mitigation.
- **Low:** polish, maintainability, or documentation issue with no plausible
  incorrect-success or safety impact.
- **Informational:** observation or future improvement without current release
  impact.

## Final report template

```markdown
# Production-readiness audit: agentic-navigation-guide <version>

## Verdict

PASS — production-ready
<!-- or CONDITIONAL PASS / FAIL -->

Audited commit: `<sha>`
Proposed tag: `<tag>`
Audit dates: `<start>` through `<end>`
Auditor(s): `<names>`
Prior remediation epic: `<link>`
Baseline due-diligence report: `<path or link>`

## Executive summary

<What was audited, the decisive evidence, and why the verdict follows.>

## Scope

Included:
- Rust CLI and library
- grammar/parser/validator/verifier
- dumper/init/recursive discovery
- security and filesystem boundaries
- tests, CI, packaging, licensing, and release process

Excluded:
- static documentation site and landing page, except package-boundary checks

## Candidate identity

| Item | Value |
|---|---|
| Commit | |
| Version | |
| Tag | |
| Previous release | |
| MSRV | |
| Stable Rust | |
| Lockfile hash | |
| Source package hash | |
| Binary hashes | |

## Test environments

| OS | Architecture | Filesystem | Case-sensitive | Rust | Result |
|---|---|---|---|---|---|

## Gate results

| Gate | Result | Evidence | Notes |
|---|---|---|---|
| Clean candidate | | | |
| Formatting | | | |
| Tests | | | |
| Clippy | | | |
| Rustdoc | | | |
| MSRV | | | |
| Cross-platform behavior | | | |
| Original-defect recurrence | | | |
| Round-trip properties | | | |
| Filesystem security | | | |
| Fuzzing | | | |
| Mutation testing | | | |
| Coverage | | | |
| Performance | | | |
| Dependency audit | | | |
| Workflow security | | | |
| SemVer | | | |
| Licensing | | | |
| Package contents | | | |
| Install smoke tests | | | |
| Release rehearsal | | | |
| Documentation | | | |

## Original-defect recurrence

| Original finding | Reproduction | Expected | Actual | Evidence |
|---|---|---|---|---|
| Hierarchy reparenting | | | | |
| Dump/parser mismatch | | | | |
| Malformed ignore marker | | | | |
| Zero-guide recursive success | | | | |
| External guide symlink read | | | | |
| Dangling output symlink | | | | |
| Ignore inconsistency | | | | |
| Trailing separator normalization | | | | |
| Quoted-choice whitespace | | | | |
| Placeholder case identity | | | | |
| Numeric extremes | | | | |
| Broken pipe | | | | |

## Coverage and test effectiveness

<Line and branch coverage, critical module coverage, mutation results, and
evidence that blocker regressions fail before their fixes.>

## Security assessment

<Threat model, containment, symlink policy, output safety, confidentiality,
resource exhaustion, workflow security, and residual limitations.>

## Performance

| Scenario | Size | Median | p95 | Max RSS | Scaling ratio | Result |
|---|---:|---:|---:|---:|---:|---|

## Packaging and release assessment

<Package manifest, install tests, version/tag agreement, artifact provenance,
checksums, release rehearsal, and distribution readiness.>

## New findings

### `<severity>` — `<title>`

- Reproducer:
- Expected:
- Actual:
- Impact:
- Affected platforms:
- Recommendation:
- Tracking issue:
- Release disposition:

## Residual risks and explicit limitations

<List only risks that remain after remediation. Include owner and expiration
for any temporary exception.>

## Evidence index

| Evidence | Hash | Description |
|---|---|---|

## Recommendation

<State whether the exact candidate may be published, whether it may be
recommended for workplace use, and any required follow-up.>
```

The final report must distinguish “the test suite is green” from “the product
is production-ready.” Its verdict must follow from captured evidence on the
exact candidate rather than from issue status, intent, or confidence alone.
