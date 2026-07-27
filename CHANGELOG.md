# Changelog

All notable changes to `agentic-navigation-guide` are recorded here. The
project follows Cargo's compatibility conventions for pre-1.0 releases.

## [0.2.0] - Unreleased

This identity is prepared but has not been published. There is no `v0.2.0`
tag, crates.io release, or GitHub Release yet.

This entry is the cumulative approved contract and migration record for the
eventual `0.2.0` release. Except where a bullet explicitly describes the
prepared identity or package now, present-tense language states the required
release behavior; it does not claim that every focused implementation issue
has landed in the current source. Publication remains blocked until the
[implementation handoff](docs/v0.2-contract.md#implementation-handoff) is
complete and the final candidate is revalidated.

### Release identity and supported product

- The prepared crate, lockfile, built executable, expected tag input, and this
  heading now agree on `0.2.0`. Release tags use `v{version}`, so this
  candidate's expected tag is `v0.2.0`.
- The installed `agentic-navigation-guide` CLI is the sole supported v0.2
  product. The package is binary-only: it has no Rust-linkable library target,
  supported Rust facade, hidden compatibility feature, or in-process shim.
- The complete documented CLI, guide-format, machine-output, exit-status,
  platform, and trust-boundary contract is the compatibility surface for the
  `0.2.x` line. A supported library or another breaking CLI-contract change
  requires `0.3.0`. A narrow security correction may restore, but may not
  redefine, an existing documented boundary.
- The exact release inputs and future baseline rule are machine-readable in
  [`release/identity.toml`](release/identity.toml) and explained in the
  [release identity and compatibility policy](docs/release-policy.md).
- The source crate now uses a root-anchored 33-path package allowlist. It ships
  only normalized Cargo metadata and lockfile, production Rust sources,
  maintained user/contract documentation, and intentional license, notice,
  and attribution files; repository workflows, site sources, agent material,
  audits, remediations, and internal test fixtures are excluded.

### CLI and automation changes from `0.1.4`

- `check` and both single and recursive `verify` now treat a valid
  `ignore=true` guide as a distinct ignored outcome. Ignored guides remain
  allowed by default and are never reported as checked or verified success;
  `--deny-ignored` makes any ignored outcome fail.
- `verify --recursive` now fails after a successful search that discovers
  zero guides. `--allow-empty` is the only opt-out and applies only to a
  genuinely empty successful search; bad roots, invalid exclusions, unsafe
  entries, traversal failures, and discovered-guide failures remain errors.
- Recursive summaries distinguish discovered, passed, failed, ignored, and
  absent outcomes. Quiet mode may suppress ordinary success chatter but not a
  policy or empty-discovery failure.
- Command-line values now take precedence over environment defaults, followed
  by built-in defaults. An environment-backed path, root, guide name, log
  mode, or execution mode no longer creates a false conflict with an explicit
  CLI value. Invalid selected environment values fail without echoing their
  contents.
- Environment resolution is scope-aware: `AGENTIC_NAVIGATION_GUIDE_PATH`
  selects an explicit guide for `check` and nonrecursive `verify`;
  `AGENTIC_NAVIGATION_GUIDE_ROOT` applies to generation and verification;
  `AGENTIC_NAVIGATION_GUIDE_NAME` selects the implicit filename or recursive
  name; and the log/execution variables remain global. Irrelevant or shadowed
  invalid variables are not parsed. Selected invalid environment defaults and
  ordinary Clap usage errors exit `2` before command execution.
- `AGENTIC_NAVIGATION_GUIDE_NAME` and `--guide-name` are restricted to one
  nonempty filename component. Explicit `--guide` and
  `AGENTIC_NAVIGATION_GUIDE_PATH` remain path authorities. A default
  single-guide `verify` resolves its guide beneath the effective `--root`;
  explicit regular guide files may still be selected outside that root.
- `--indent` accepts only `1` through `16`; `--depth` accepts `0` through
  `256`, where zero includes root children but no grandchildren. Invalid
  values are usage errors rather than wrapping, flattening, panicking, or
  triggering pathological allocation. An explicit depth is a deliberate
  cutoff and does not inspect deeper entries; an omitted depth rejects a tree
  that would require logical depth 257.
- `init` and `dump --output` share a create-new destination policy. They never
  overwrite an existing file and have no force mode. Generation failures are
  completed before destination creation; concurrent creators have exactly
  one winner, and cleanup never removes a replaced entry.
- Output parents must already exist as effectively writable directories; no
  command creates them. In-root parent components must be stable real
  directories, while an explicit external output grants only stable
  external-parent authority. Every existing leaf type rejects. The complete
  buffer is exclusively created, written, flushed, data-synced, and checked
  for final type, identity, and length; failed cleanup preserves replacements
  and reports a residual artifact. Name ownership is atomic, but readers may
  observe an in-progress prefix, parent-directory sync and crash durability
  are not promised, and stdout remains fallible and cannot be rolled back.
- No tested nonzero command path exits silently. Default, hook, and GitHub
  Actions modes emit deterministic diagnostics while retaining their
  documented exit-code behavior.
- Ordinary and allowed-ignored success exits `0`. A genuine zero discovery
  accepted by `--allow-empty` also exits `0`, prints its zero aggregate unless
  quiet, and is silent when quiet. Runtime and policy failures exit `2` in
  post-tool-use mode and `1` in default, pre-commit, and GitHub Actions modes;
  quiet never changes status.

### Guide-language and format changes from `0.1.4`

- A document must contain exactly one complete guide block. Opening and
  closing marker names are case-sensitive, occupy complete lines, and accept
  only surrounding horizontal whitespace. Malformed marker prefixes,
  duplicate/nested blocks, stray closing markers, marker attributes on the
  closing line, a UTF-8 BOM on the opening line, and lone carriage returns are
  rejected.
- The only opening-marker attribute is one exact `ignore=true` assignment,
  quoted or unquoted, with optional horizontal whitespace around `=`.
  Unknown, duplicate, false, valueless, concatenated, single-quoted, or
  malformed attributes are rejected. A valid ignored body is opaque and may
  be empty, but its envelope is still validated.
- An active body is nonempty and contains only entry lines. The list delimiter
  is exactly `- `; a tab or a second unescaped space after the dash is
  invalid. Blank lines and other Markdown inside the active body are invalid.
- Indentation is space-only. The first entry is at depth zero; the first
  indented entry establishes a 1–16-space unit; later indentation is an exact
  multiple of that unit; depth may increase by one only beneath the
  immediately preceding directory; and logical depth is capped at 256.
  Parsing no longer searches backward for a convenient earlier parent, so
  siblings and nested entries retain their textual hierarchy.
- Exactly one trailing `/` marks a directory. Logical paths use `/`, are
  relative, and reject absolute or drive-prefixed forms, empty components,
  `.` or `..`, repeated separators, repeated trailing separators, and
  duplicate decoded paths. A line without an unescaped `#` treats everything
  after the exact list delimiter as its path.
- Outside a whole quoted path, the first unescaped `#` starts the comment.
  Bare paths support exactly `\#`, `\\`, `\[`, `\]`, `\,`, `\"`, and `\ `;
  dangling or unknown escapes fail. Whole quoted paths preserve syntax
  characters and edge spaces and support only `\"` and `\\`.
- Bare `...` remains the placeholder; quoted `"..."` is the literal filename.
  Empty or whitespace-only comments are normalized to no comment.
  Placeholder accounting uses exact first-component filesystem identity, so
  already-mentioned descendants cannot make their parent look unmentioned.
  An uncommented placeholder requires at least one unlisted immediate child;
  a meaningfully commented placeholder may describe a fully listed or empty
  directory. Placeholder matching remains type-agnostic and does not make a
  link or special sibling listable.
- Choice expressions permit 2–256 alternatives in one bare regular-file path,
  preserve quoted alternative whitespace, parent, order, and comments, and
  reject malformed or second lists, all-empty or duplicate alternatives,
  directory- or placeholder-producing alternatives, different parents,
  children, and ambiguous expansions. Expanded paths still pass every
  ordinary path and duplicate check.
- UTF-8 names, including non-ASCII punctuation and symbols, are supported
  without the old character allowlist. NUL, CR, LF, HTAB, other C0 controls,
  DEL, and non-UTF-8 filesystem names are rejected. Diagnostics render
  rejected names reversibly instead of using lossy conversion.
- `dump` and `init` emit one canonical spelling for every supported name,
  preserve hierarchy, and sort siblings by ascending UTF-8 bytes. Generated
  active output is nonempty and must round-trip through `check`.

### Filesystem, generation, and verification changes from `0.1.4`

- Only regular files, directories, and hard-linked regular files are
  representable. Included symbolic links, Windows reparse entries, FIFOs,
  sockets, devices, unknown entry types, and transient classification
  failures make generation fail before any guide bytes are delivered.
- Generation requires an existing readable directory and at least one
  included representable entry. Missing, regular-file, unreadable, empty, and
  fully excluded roots fail. A caller-selected root link or reparse alias may
  establish the generation anchor, but included descendants are never
  followed through links.
- `dump`, `init`, and recursive discovery use one case-sensitive,
  platform-independent exclusion dialect. A pattern without `/` matches a
  basename at every depth; a slash pattern matches the complete
  root-relative path; `*` and `?` remain inside a component; a complete `**`
  spans components; documented classes, ranges, escapes, and leading class
  negation are supported; multiple patterns form a union without reinclusion;
  and directories are pruned before descent. `init`'s default VCS-directory
  exclusions now apply at every depth, `--include-vcs-directories` disables
  them, and `.gitignore` is not interpreted.
- Guide files are classified and opened without following a final link or
  reparse entry. This applies to implicit, recursive, and explicitly selected
  guides. Exclusions are applied before unsafe matching entries are
  classified, and nonmatching descendant links are not traversed.
- On Windows, guide reads reject alternate streams, named-pipe/device
  namespaces, reserved DOS aliases, and unsupported verbatim forms. Output
  destinations reject the corresponding stream/device forms and validate a
  newly created regular non-reparse disk handle.
- Verification uses one enumerated snapshot per visited parent and requires
  exact case and Unicode spelling. A host-filesystem case alias or
  normalization alias no longer satisfies a differently spelled guide entry.
- Verification anchors once to the caller-selected root, rejects every link
  or reparse ancestor below that anchor without resolving its target, and
  fails if an observed entry changes identity or type. This is a
  stable-filesystem consistency guarantee, not a sandbox or a promise against
  hostile concurrent replacement.
- Diagnostics no longer reproduce raw guide source lines, raw untrusted
  environment values, or resolved external targets. Logical paths and
  filesystem names use bounded, control-safe rendering in default, hook, and
  GitHub Actions output.

### Packaging, documentation, and licensing

- Current source and the prepared package declare `MIT OR Apache-2.0` and
  carry `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, and generated third-party
  attribution. This prospective package metadata does not alter any immutable
  `0.1.x` archive.
- Cargo publication is explicitly restricted to crates.io. Issue #64 does not
  publish there or settle the final package-file allowlist, which remains a
  separate release blocker.
- The contradictory licensing information in every published `0.1.0` through
  `0.1.4` archive, plus the explicit decision to leave those releases
  unyanked, is recorded factually in
  [`LICENSING.md`](LICENSING.md#historical-01x-licensing-metadata-clarification).
- The normative v0.2 guide-language, filesystem, trust, and supported-product
  contract is now maintained in
  [`docs/v0.2-contract.md`](docs/v0.2-contract.md). A generated static
  documentation site and CI-gated third-party attribution were also added.

### Rust source compatibility: complete removal of published `0.1.4`

This release deliberately removes the complete Rust library exposed by the
immutable published `0.1.4` crate. The inventory below contains all 128
published API entries under the project's ledger counting convention: one
library target, seven public modules, 17 root re-exports, one alias, ten
structs, six enums, 38 variants, 19 public fields, seven free functions, and
22 inherent methods. None has an in-process v0.2 replacement.

The exact baseline is the crates.io `0.1.4` archive with SHA-256
`d08fefac88faf8d737eea273f86bfbc80aaac1eb80ff3a57bde5add824fe5da0`,
VCS revision `560ce399e1e28e8e0d6b87988956893796d2dfab`, normalized-manifest
SHA-256
`1dc83730531459a1fcae387cc5e5f625a3ff498659915d58fa875dd14c9fab3b`,
and `src/lib.rs` SHA-256
`c2107c1948025e592e4af33a39b8f80ce7f02b8160d48c12acf6a4c67963d656`.
It is distinct from the later last-linkable development revision
`e34399c14683878064cad18e9506186cd7e4fef1`.

<!-- published-v0.1.4-api:start -->
#### PackageTarget (1)

```text
agentic_navigation_guide (lib)
```

#### Module (7)

```text
dumper
errors
parser
recursive
types
validator
verifier
```

#### ReExport (17)

```text
crate::Dumper
crate::AppError
crate::Result
crate::SemanticError
crate::SyntaxError
crate::Parser
crate::find_guides
crate::verify_guides
crate::GuideLocation
crate::GuideVerificationResult
crate::ExecutionMode
crate::FilesystemItem
crate::LogLevel
crate::NavigationGuide
crate::NavigationGuideLine
crate::Validator
crate::Verifier
```

#### TypeAlias (1)

```text
errors::Result<T> = std::result::Result<T,AppError>
```

#### Struct (10)

```text
dumper::Dumper
errors::ErrorFormatter
parser::Parser
recursive::GuideLocation
recursive::GuideVerificationResult
types::NavigationGuideLine
types::NavigationGuide
types::Config
validator::Validator
verifier::Verifier
```

#### Enum (6)

```text
errors::AppError
errors::SyntaxError
errors::SemanticError
types::FilesystemItem
types::ExecutionMode
types::LogLevel
```

#### Variant (38)

```text
AppError::Syntax(SyntaxError)
AppError::Semantic(SemanticError)
AppError::Io(std::io::Error)
AppError::GlobPattern(globset::Error)
AppError::WalkDir(walkdir::Error)
AppError::Other(String)
SyntaxError::MissingOpeningMarker { line: usize }
SyntaxError::MissingClosingMarker { line: usize }
SyntaxError::MultipleGuideBlocks { line: usize }
SyntaxError::EmptyGuideBlock
SyntaxError::InvalidListFormat { line: usize }
SyntaxError::DirectoryMissingSlash { line: usize, path: String }
SyntaxError::InvalidSpecialDirectory { line: usize, path: String }
SyntaxError::InconsistentIndentation { line: usize, expected: usize, found: usize }
SyntaxError::InvalidIndentationLevel { line: usize }
SyntaxError::BlankLineInGuide { line: usize }
SyntaxError::InvalidPathFormat { line: usize, path: String }
SyntaxError::InvalidWildcardSyntax { line: usize, path: String, message: String }
SyntaxError::InvalidCommentFormat { line: usize }
SyntaxError::AdjacentPlaceholders { line: usize }
SyntaxError::PlaceholderWithChildren { line: usize }
SemanticError::ItemNotFound { line: usize, item_type: String, path: String, full_path: PathBuf }
SemanticError::TypeMismatch { line: usize, expected: String, found: String, path: String }
SemanticError::InvalidNesting { line: usize, child: String, parent: String }
SemanticError::SymlinkTargetMismatch { line: usize, path: String, expected: String, actual: String }
SemanticError::PermissionDenied { line: usize, path: String }
SemanticError::PlaceholderNoUnmentionedItems { line: usize, parent: String }
FilesystemItem::File { path: String, comment: Option<String> }
FilesystemItem::Directory { path: String, comment: Option<String>, children: Vec<NavigationGuideLine> }
FilesystemItem::Symlink { path: String, comment: Option<String>, target: Option<String> }
FilesystemItem::Placeholder { comment: Option<String> }
ExecutionMode::Default
ExecutionMode::PostToolUse
ExecutionMode::PreCommitHook
ExecutionMode::GitHubActions
LogLevel::Quiet
LogLevel::Default
LogLevel::Verbose
```

#### Field (19)

```text
GuideLocation::guide_path: PathBuf
GuideLocation::root_path: PathBuf
GuideVerificationResult::location: GuideLocation
GuideVerificationResult::success: bool
GuideVerificationResult::error: Option<String>
GuideVerificationResult::ignored: bool
NavigationGuideLine::line_number: usize
NavigationGuideLine::indent_level: usize
NavigationGuideLine::item: FilesystemItem
NavigationGuide::items: Vec<NavigationGuideLine>
NavigationGuide::prologue: Option<String>
NavigationGuide::epilogue: Option<String>
NavigationGuide::ignore: bool
Config::execution_mode: ExecutionMode
Config::log_level: LogLevel
Config::root_path: Option<PathBuf>
Config::guide_path: Option<PathBuf>
Config::original_guide_path: Option<String>
Config::original_root_path: Option<String>
```

#### Function (7)

```text
crate::parse_navigation_guide(content: &str) -> Result<NavigationGuide>
crate::check_syntax(guide: &NavigationGuide) -> Result<()>
crate::verify_guide(guide: &NavigationGuide, root_path: &std::path::Path) -> Result<()>
crate::dump_directory(root_path: &std::path::Path, max_depth: Option<usize>, exclude_patterns: &[String], indent_size: usize) -> Result<String>
recursive::find_guides(root: &Path, guide_name: &str, exclude_patterns: &[String]) -> Result<Vec<GuideLocation>>
recursive::verify_guides(guides: &[GuideLocation], config: &Config) -> Result<Vec<GuideVerificationResult>>
recursive::display_results(results: &[GuideVerificationResult], config: &Config) -> bool
```

#### Method (22)

```text
Dumper::new(root_path: &Path) -> Self
Dumper::with_max_depth(self, max_depth: Option<usize>) -> Self
Dumper::with_exclude_patterns(self, patterns: &[String]) -> Result<Self>
Dumper::with_indent_size(self, indent_size: usize) -> Self
Dumper::dump(&self) -> Result<String>
Dumper::dump_with_wrapper(&self) -> Result<String>
SyntaxError::line_number(&self) -> Option<usize>
SemanticError::line_number(&self) -> usize
ErrorFormatter::format_with_context(error: &AppError, file_content: Option<&str>) -> String
Parser::new() -> Self
Parser::parse(&self, content: &str) -> Result<NavigationGuide>
NavigationGuideLine::path(&self) -> &str
NavigationGuideLine::comment(&self) -> Option<&str>
NavigationGuideLine::is_directory(&self) -> bool
NavigationGuideLine::is_placeholder(&self) -> bool
NavigationGuideLine::children(&self) -> Option<&[NavigationGuideLine]>
NavigationGuide::new() -> Self
NavigationGuide::get_full_path(&self, item: &NavigationGuideLine) -> PathBuf
Validator::new() -> Self
Validator::validate_syntax(&self, guide: &NavigationGuide) -> Result<()>
Verifier::new(root_path: &Path) -> Self
Verifier::verify(&self, guide: &NavigationGuide) -> Result<()>
```

<!-- published-v0.1.4-api:end -->

Published traits and generated implementations disappear with those types:
the model types' Debug, Clone, PartialEq, Eq, Serialize and Deserialize
commitments; Copy and Default on modes; Config's Default and serde
commitments; Debug and Clone on recursive results; Default on Parser and
Validator; Display and Error, equality, and conversion implementations on the
error types; observed Send, Sync, and Unpin auto traits; and observed
UnwindSafe and RefUnwindSafe behavior (except where `AppError` already lacked
the unwind traits).

### Migration from `0.1.4`

- Rust callers must replace in-process calls with an invocation of the
  installed CLI through its documented process or machine contract. There is
  no source-compatible shim or supported partial facade.
- `NavigationGuide::get_full_path` has no replacement;
  `NavigationGuideLine::path()` is not equivalent. `FilesystemItem::Symlink`
  and `SemanticError::SymlinkTargetMismatch` likewise have no replacement,
  and v0.2 provides no link-inventory or target-matching operation.
- Callers that cannot migrate may remain pinned to the immutable, unsupported
  `0.1.4` artifact at their own risk. That pin receives no v0.2 compatibility
  or maintenance promise.
- Guide authors should run `agentic-navigation-guide check` under v0.2 and
  address every reported grammar error. There is no lossy automatic
  conversion. `dump` or `init` can regenerate a guide after unsupported
  entries are excluded or removed; hand-written comments must then be
  reapplied deliberately.
- Required automation should add `--deny-ignored`, must omit
  `--allow-empty`, and should account for create-new output behavior and
  exact case/Unicode identity.

### SemVer evidence and future baseline

Pinned `cargo-semver-checks 0.49.0` compared the immutable published `0.1.4`
crate with the exact last-linkable source revision and exited `100`. It
reported four major lint classes:
`enum_no_repr_variant_discriminant_changed`, `enum_variant_added`,
`enum_variant_missing`, and `inherent_method_missing`. Those findings plus the
total library removal above make `0.2.0` the deliberate breaking boundary.

That report is migration evidence, not a passing gate for the final
binary-only candidate: there is no library target for the tool to select.
Future compatible `0.2.x` candidates select the most recent preceding
non-yanked release in the same compatibility line and compare the exact binary
target shape plus the complete documented CLI contract. Every incompatibility
fails a `0.2.x` candidate; acknowledging it in release notes cannot authorize a
same-line break. An accepted break requires a new breaking line, whose first
candidate compares with the latest non-yanked published predecessor across
lines and records every approved break. If that line restores a supported
Rust library, it must establish a new published Rust baseline and resume
pinned library SemVer analysis.
