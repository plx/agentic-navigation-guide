# Issue #68 original-specification disposition

## Decision

The repository owner approved both decision groups on 2026-07-27:

1. preserve the complete original specification and its Git history by moving
   it from the repository root to
   [`docs/history/Specification.md`](../docs/history/Specification.md), with an
   unmistakable dated non-normative banner; and
2. supersede rather than merge
   [draft PR #21](https://github.com/plx/agentic-navigation-guide/pull/21),
   recording the disposition of every proposal it carried.

The durable approval is
[issue comment 5088836259](https://github.com/plx/agentic-navigation-guide/issues/68#issuecomment-5088836259).
This implements the preferred outcome allowed by issue #68 without changing
the behavior approved by #34 or the binary-only boundary approved by #36.

`docs/v0.2-contract.md` is the sole normative v0.2 specification.
`README.md` is the concise released-behavior entry point, implementation plus
tests describe realized unreleased behavior, and the relocated original
specification is recoverable historical evidence only.

## Preserved provenance

Before the move, `Specification.md` had Git blob
`694a1752aec9f3f29836fc9d006ea16f7cd7915b`. Its history begins at commit
`e6848333db81269bc8e311818c7f5e08058bed0f` on 2025-07-29. Commit
`324498f7fbbcd8b4431cb920e3396c01e4d5e199` added the interim historical
banner after establishing the normative v0.2 contract. The issue #68 move
retains the body, strengthens the banner, and uses a Git rename so
`git log --follow -- docs/history/Specification.md` preserves that lineage.

The background problem and solution sections remain useful rationale: a
partial but accurate guide helps agents navigate a changing tree, while an
inaccurate guide can be worse than none. Those paragraphs state motivation,
not format or product rules, so they are retained verbatim outside the
substantive-rule ledger below.

## Complete substantive-rule classification

The table groups adjacent sentences only when they express one inseparable
rule. Every prescriptive statement in the historical format, placeholder,
command, execution-mode, and Rust sections appears exactly once. The four
issue-mandated dispositions have these meanings:

- **Implemented/normative:** the rule is retained by the current normative
  contract and executable evidence.
- **Deliberately changed:** v0.2 makes a different explicit choice and the row
  identifies the replacement.
- **Obsolete:** the statement is implementation planning, not a supported
  product rule, or assumes a removed product surface.
- **Unresolved:** issue #67 owns the complete CLI option/output contract; this
  issue does not silently settle it.

<!-- issue-68-classification:start -->

| ID | Historical rule | Disposition | Current authority or replacement | Executable fixture or evidence | Rationale and traceability |
| --- | --- | --- | --- | --- | --- |
| H68-001 | A guide is a Markdown document with one guide block and optional prologue or epilogue. | Implemented/normative | `docs/v0.2-contract.md`, Document and marker grammar. | `body-prologue-epilogue`, `marker-bare` | The current grammar makes the UTF-8 Markdown envelope and surrounding text explicit. |
| H68-002 | The default guide is `AGENTIC_NAVIGATION_GUIDE.md` at the project root, while name and location are configurable. | Unresolved | [#67](https://github.com/plx/agentic-navigation-guide/issues/67) owns the complete CLI defaults and configuration contract. | Current CLI integration tests; final public wording remains #67-owned. | The grammar decision does not silently freeze every command-line and environment default. |
| H68-003 | Exact opening and closing sentinels are required and only one block may occur. | Implemented/normative | `docs/v0.2-contract.md`, Document and marker grammar. | `marker-bare`, `marker-two-blocks`, `marker-closing-attribute` | #34 and its implementation owners replaced permissive marker recognition with one exact envelope. |
| H68-004 | An opening marker may use quoted or unquoted `ignore=true`; verification skips it and warns. | Deliberately changed | `docs/v0.2-contract.md`, Ignored guides. | `marker-ignore-unquoted`, `marker-ignore-quoted`, `ignore-opaque-body`, `operation-cli-ignored-default` | v0.2 validates the envelope, treats the body as opaque, returns a distinct ignored result, and adds `--deny-ignored`; it is not a verified success. |
| H68-005 | An active body is one unordered list with no blank lines or other content. | Implemented/normative | `docs/v0.2-contract.md`, Active body grammar. | `body-blank-line`, `body-non-list`, `body-empty` | The current grammar states the nonempty single-list body directly. |
| H68-006 | Each body line is a filesystem item introduced by the list delimiter and nesting is expressed by indentation. | Implemented/normative | `docs/v0.2-contract.md`, Active body grammar and Indentation and parentage. | `body-extra-list-space`, `body-tab-after-dash`, `indent-two-spaces` | v0.2 fixes the delimiter to one ASCII space and defines the hierarchy independently of Markdown rendering. |
| H68-007 | Indentation indicates nesting without a specified unit or depth bound. | Deliberately changed | `docs/v0.2-contract.md`, Indentation and parentage. | `indent-two-spaces`, `indent-sixteen-spaces`, `indent-seventeen-spaces`, `indent-skipped-depth` | v0.2 uses a 1–16-space unit, forward parentage, checked depth 256, and rejects skipped depths. |
| H68-008 | Every item is a path relative to its textual parent. | Deliberately changed | `docs/v0.2-contract.md`, Path expressions and filesystem trust. | `path-posix-absolute`, `path-windows-prefix`, `path-nested-drive-looking-component` | Relative parentage remains, but v0.2 explicitly permits multi-component entries and rejects absolute, drive, UNC, device, and parent-escape forms before access. |
| H68-009 | Directories end in `/`; entries without it are files. | Implemented/normative | `docs/v0.2-contract.md`, Path expressions. | `path-quoted-directory`, `path-quoted-trailing-separator`, `choice-directory-result` | The directory marker remains textual and exact; malformed repeated or in-quote trailing separators are rejected. |
| H68-010 | `.` and `..` pseudo-directories are invalid items. | Implemented/normative | `docs/v0.2-contract.md`, Path expressions. | `path-dot-component`, `path-parent-component` | The prohibition now applies to every decoded path component. |
| H68-011 | A symlink may be represented as a file or annotated with its target. | Deliberately changed | `docs/v0.2-contract.md`, Filesystem entry types and trust boundary. | `operation-dump-file-symlink`, `operation-dump-directory-symlink`, `operation-verify-file-symlink` | #36, #42, #53, and #54 establish no supported link syntax or API model; links and reparse entries are rejected without following. |
| H68-012 | A first `#` starts a comment, later `#` characters are comment text, and arbitrary whitespace may separate path, comment, and line end. | Deliberately changed | `docs/v0.2-contract.md`, Active body grammar and Path expressions. | `path-comment-escaped-hash`, `path-quoted-sensitive`, `operation-parse-tab-name` | The first unescaped outer `#` still starts a comment, but layout is ASCII space only; literal hashes use quoting or escaping and HTAB in a name is rejected. |
| H68-013 | List ordering is not validated. | Implemented/normative | `docs/v0.2-contract.md`, Indentation and parentage. | `indent-two-spaces`, `choice-simple` | Verification is order-insensitive while parsing and generation preserve source or expansion order. |
| H68-014 | A guide may omit filesystem entries and is partial by default. | Implemented/normative | `docs/v0.2-contract.md`, Placeholders and listing completeness. | `placeholder-forms`, `operation-verify-placeholder-first-component` | v0.2 explicitly has no implicit exhaustive mode. |
| H68-015 | A line may contain at most one bracketed choice list. | Deliberately changed | `docs/v0.2-contract.md`, Choice lists. | `choice-multiple-lists`, `choice-single-alternative`, `choice-simple` | One list remains the maximum, but a valid choice now requires 2–256 alternatives and must produce sibling regular files. |
| H68-016 | Whitespace inside choices is ignored except inside quotes. | Implemented/normative | `docs/v0.2-contract.md`, Choice lists. | `choice-quoted-whitespace`, `choice-simple` | Spaces and tabs around unquoted alternatives are layout; quoting or escaping preserves edge spaces. |
| H68-017 | Empty choice alternatives are allowed. | Deliberately changed | `docs/v0.2-contract.md`, Choice lists. | `choice-empty-alternative`, `choice-all-empty` | Empty alternatives remain valid only when at least one decoded alternative is nonempty. |
| H68-018 | A backslash escapes individual comma, space, bracket, and similar characters in a choice. | Deliberately changed | `docs/v0.2-contract.md`, Choice lists and Path expressions. | `choice-escaped-comma`, `choice-escaped-hash-comment`, `path-unknown-escape` | v0.2 defines a closed escape alphabet, separates quoted-choice escapes, and rejects unknown or dangling escapes. |
| H68-019 | Double quotes preserve complex choice alternatives and `\"` represents a quote. | Implemented/normative | `docs/v0.2-contract.md`, Choice lists. | `choice-quoted-whitespace`, `choice-escaped-hash-comment` | The normative grammar specifies quote termination, preserved syntax characters, and the exact quoted escape set. |
| H68-020 | Verification requires listed entries to exist, match file or directory type, and have the declared parent-child relationship. | Implemented/normative | `docs/v0.2-contract.md`, Core invariant, Filesystem identity, and Textual-item containment. | `operation-dump-regular-file`, `operation-dump-directory`, `indent-child-under-file` | The current contract strengthens this to exact textual identity on a supported stable filesystem. |
| H68-021 | A commented `...` placeholder may describe omitted or future entries, including in an empty directory. | Implemented/normative | `docs/v0.2-contract.md`, Placeholders and listing completeness. | `placeholder-forms`, `placeholder-child` | Meaningful comments retain the non-asserting future or abstraction use while placeholders cannot own children. |
| H68-022 | An uncommented `...` requires at least one actual unlisted immediate child and is invalid in an empty directory. | Implemented/normative | `docs/v0.2-contract.md`, Placeholders and listing completeness. | `placeholder-forms`, `operation-verify-placeholder-first-component` | v0.2 defines exact-identity and first-component accounting for the asserted unlisted child. |
| H68-023 | Multiple placeholders at one level inspect the same explicitly mentioned set independently. | Deliberately changed | `docs/v0.2-contract.md`, Placeholders and listing completeness. | `placeholder-forms`, `placeholder-adjacent` | Multiple nonadjacent placeholders share one snapshot; adjacent placeholders and placeholder children are rejected. |
| H68-024 | Global `--verbose` and `--quiet` options and a three-way log-mode environment setting control output. | Unresolved | [#67](https://github.com/plx/agentic-navigation-guide/issues/67) owns complete global-option, environment, precedence, and output guarantees. | Current CLI tests are realized behavior, not a #68 decision. | #68 preserves the proposal but cannot convert it into a public contract ahead of #67. |
| H68-025 | The product exposes `dump`, `init`, `check`, and `verify` with the summarized historical roles. | Unresolved | [#67](https://github.com/plx/agentic-navigation-guide/issues/67) owns the complete command surface and help contract. | Current command help and CLI integration tests remain implementation evidence. | The normative grammar binds named operations only where they expose already approved language or filesystem decisions. |
| H68-026 | `dump` has historical output, depth, exclusion, indentation, wrapper, and root arguments with listed defaults. | Unresolved | [#67](https://github.com/plx/agentic-navigation-guide/issues/67) owns the complete argument/default contract. | Existing operation fixtures cover approved behavior subsets, but not the entire historical option prose. | #43, #44, and #45 settled focused safety behavior without authorizing #68 to freeze all CLI details. |
| H68-027 | `dump` root precedence is environment root, then current directory. | Unresolved | [#67](https://github.com/plx/agentic-navigation-guide/issues/67) owns final CLI and environment precedence wording. | Current configuration tests remain realized behavior pending #67. | The trust-anchor decision does not by itself approve every historical environment-variable promise. |
| H68-028 | `init` mirrors `dump`, requires output, and always includes wrapper markers. | Unresolved | [#67](https://github.com/plx/agentic-navigation-guide/issues/67) owns final command/option relationships. | `operation-output-init-new-in-root` and generation tests cover focused safety behavior outside this ledger. | Create-only destinations and generation grammar are normative, but the complete command comparison remains #67 work. |
| H68-029 | `check` accepts three execution flags and a guide path, with environment/name/default precedence and a historical positional-path proposal. | Unresolved | [#67](https://github.com/plx/agentic-navigation-guide/issues/67) owns flags, aliases, conflicts, positional syntax, and precedence. | Current CLI integration tests expose the realized surface. | The historical text itself names both a `--guide` option and a positional argument, so #68 must not silently choose between them. |
| H68-030 | `check` performs syntax checks only, is silent on success, and emits one line-numbered diagnostic per error. | Unresolved | [#67](https://github.com/plx/agentic-navigation-guide/issues/67) owns the complete diagnostic, aggregation, and output contract. | Parser fixtures establish accept/reject results; current CLI tests establish realized formatting. | Syntax-only scope is retained in implementation, but exact public diagnostic guarantees await #67. |
| H68-031 | `verify` runs syntax before filesystem semantics, shares `check` arguments plus root, is silent on success, and reports line-numbered errors. | Unresolved | [#67](https://github.com/plx/agentic-navigation-guide/issues/67) owns the complete verify command and diagnostic contract. | Current CLI and trust tests remain realized behavior. | Fail-before-semantic parsing and trust rules are approved, but #68 does not freeze every output detail. |
| H68-032 | Default, post-tool-use, pre-commit, and GitHub Actions modes have the listed exits and presentation. | Unresolved | [#67](https://github.com/plx/agentic-navigation-guide/issues/67) owns mode names, conflicts, exact outputs, and exit-code tables. | Current CLI mode tests remain realized behavior. | The historical emoji and message examples are preserved as prior art, not silently promoted. |
| H68-033 | Internal design should use filesystem-item, line, guide, and typed syntax/semantic-error data types. | Obsolete | Internal architecture is implementation-private under the binary-only v0.2 product. | Source review and ordinary unit tests; there is no supported Rust type contract. | These are maintainability suggestions, not compatibility promises. |
| H68-034 | The crate should expose both a library and binary, with a well-defined public library API. | Deliberately changed | `docs/v0.2-contract.md`, Supported product and Rust API. | `operation-library-ignored`, `trust-guide-direct-library-path` | #36, #52, and #54 deliberately select one installed binary and zero supported Rust facade. |
| H68-035 | A public library should have unit tests, while rustfmt, Clippy, tests, and documentation should run through editor or hook automation. | Obsolete | Repository CI and contributor policy own quality automation; no public library exists in v0.2. | Current CI jobs and the full repository suite provide evidence without creating a product guarantee. | Tool invocation locations are contributor workflow, not guide-language or compatibility rules. |
| H68-036 | The binary should be installable with Cargo. | Implemented/normative | `README.md`, `docs/release-policy.md`, and the package/release identity checks. | `operation-dump-regular-file`, `operation-cli-ignored-default` plus issue #62 packaged-install smoke evidence | The source crate is binary-only, package-allowlisted, MSRV-installed, and prepared for exact locked publication. |

<!-- issue-68-classification:end -->

## Contradictory historical example

H68-012 is the selected executable contradiction. The historical text allows
“arbitrary whitespace” between a path and comment. This fixed input therefore
looks allowed under the old prose:

```text
<agentic-navigation-guide>
- Cargo.toml	# historical text allowed arbitrary whitespace
</agentic-navigation-guide>
```

The v0.2 grammar permits ASCII spaces as line padding and rejects HTAB in a
path. `operation-parse-tab-name` already records that normative rejection.
`tests/issue_68_normative_source.rs` invokes the real CLI against the fixed
example and requires the line-2 invalid-path diagnostic. Before reconciliation
the new issue test also failed because the contradictory artifact remained at
the repository root and had no classification record; after reconciliation,
the example remains visibly historical and cannot override executable
behavior.

## Draft PR #21 disposition

PR #21 is non-authoritative prior art based on commit
`15a6df3b4d78273ed3beeb82a412e7c1dc259c46`. It is not merged or rebased.
After this replacement lands it is closed as superseded with a link to the
issue #68 PR.

| ID | PR #21 proposal | Disposition | Evidence and reason |
| --- | --- | --- | --- |
| P21-01 | Delete root `Specification.md`. | Superseded | The approved history-preserving move removes root ambiguity without discarding in-tree rationale. |
| P21-02 | Delete completed remediation records. | Rejected | Those records remain traceable production-readiness evidence and are outside #68's approved historical-specification move. |
| P21-03 | Treat README plus implementation as the only maintained authority. | Superseded | #34 established `docs/v0.2-contract.md` as the sole normative target, with README concise and implementation/tests describing realized source behavior. |
| P21-04 | Remove README roadmap and divergence material. | Superseded | Later merged remediation rewrote these sections around explicit v0.2 contract and migration evidence; no stale PR #21 prose is applied. |
| P21-05 | Tighten opening/closing marker lookalike parsing and tests. | Incorporated independently | #34's ledger and merged PR #88 require exact marker grammar, including malformed lookalike rejection, without taking PR #21's branch. |
| P21-06 | Remove `NavigationGuide::get_full_path`. | Incorporated independently | #36 selected the binary-only boundary and #52 with PR #104 removed the incorrect method using frozen API-disposition evidence. |
| P21-07 | Consolidate CLI error formatting and alter guide diagnostic path handling. | Rejected | The proposal targets a stale pre-remediation CLI architecture and is outside #68; current command behavior remains subject to focused owners including #67. |
| P21-08 | Update the navigation guide for PR #21's deleted and added files. | Superseded | The issue #68 guide update catalogs the approved history directory and current evidence rather than the stale branch tree. |

## Deterministic repository checks

`tests/issue_68_normative_source.rs` provides three bounded checks:

1. recursively inspect repository Markdown while excluding build, dependency,
   VCS, and private context directories; require exactly one normative claimant
   at `docs/v0.2-contract.md` and exactly one historical marker at the moved
   specification;
2. require exact H68-001 through H68-036 and P21-01 through P21-08 ledgers,
   validate allowed dispositions, and resolve every cited executable fixture
   ID against the three v0.2 fixture sources; and
3. execute the single fixed contradictory tab example and require rejection.

The same test balances Markdown fences and resolves local Markdown links in
the normative contract, historical record/index, and this audit. README,
agent-memory, and navigation-guide references must point to the moved file.
This is deterministic contract validation; no fuzzing, mutation testing, or
generated input exploration is used.

## Red-before-fix and acceptance

On the exact issue base `1076e081280426fc42fc9299322657fefd8a3b27`,
the focused test exited 101 with all three tests failing:

- the contradictory `Specification.md` still existed at repository root;
- the required disposition audit did not exist; and
- the fixed tab example was rejected, but the initial assertion had not yet
  accepted the parser's precise `invalid path format` wording.

That last assertion was narrowed to require the exact line-2 invalid-path
diagnostic plus the escaped tab context. It does not relax the parser outcome.
The post-change acceptance run must pass this focused test, the full repository
suite, formatting, Clippy, guide check/verification, and the existing
documentation/contract bijection.

## Local acceptance results

The completed change passed:

- `cargo test --locked --test issue_68_normative_source`: 3 passed;
- the same focused test under Rust `1.85.0`: 3 passed;
- `cargo fmt -- --check`;
- `cargo check --locked --all-targets --all-features`;
- `cargo clippy --locked --all-targets --all-features -- -D warnings`;
- `cargo test --locked --all-targets --all-features`: 348 passed and 3
  intentional ignores;
- the exact issue #62 package-manifest regression;
- guide `check` and `verify` against the repository root;
- 61 production-readiness selector regressions;
- 14 release-identity checker regressions and the prepared identity check;
- clean-tree `cargo package --list --locked --offline`: the reviewed package
  remains exactly 33 paths;
- clean-tree `cargo package --locked`: 661.1 KiB source and 151.6 KiB
  compressed; and
- clean-tree `cargo publish --dry-run --locked`: package verification passed
  and upload was aborted as required by dry-run mode.

The first parallel full-suite attempt encountered the known macOS
`Bad file descriptor` condition in the pre-existing
`output_trust_evidence_is_an_exact_set_for_issue_45` concurrent filesystem
test after 219 unit passes. Its exact isolated rerun passed, and the complete
suite then passed without recurrence. No #68 source or documentation logic is
on that filesystem-output path; the existing hermetic-test work remains the
appropriate owner.

All issue #68 inputs are fixed strings, exact ledgers, or bounded repository
document scans. No fuzzing, mutation testing, randomized generation, or
headless third-party-agent delegation was used.
