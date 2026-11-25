# Formal verification with Verus

This directory holds the [Verus](https://github.com/verus-lang/verus) harness
for the formally verified parts of `agentic-navigation-guide`. It is not part
of the Cargo package and adds nothing to the product's dependency graph.

## How it works

The verified code is ordinary production code. Functions in `src/proven/`
(and the few verified in place, listed below) carry their specification in a
`#[cfg_attr(verus_keep_ghost, verus_spec(...))]` attribute, and their spec
functions and lemmas live in `#[cfg(verus_keep_ghost)] verus! { ... }` blocks.
Verus compiles with `--cfg verus_keep_ghost`; ordinary `cargo build` never
sets that flag, so the product sees plain Rust with no Verus dependency.

[`verus_root.rs`](verus_root.rs) is the Verus crate root. It includes the
product source files at their real paths, under the same module names that
`src/main.rs` uses, so Verus checks exactly the code that Cargo compiles.

For every annotated function, Verus proves:

- functional correctness against its `ensures` specification;
- absence of panics, arithmetic overflow, and out-of-bounds indexing;
- termination of every loop and recursive function.

Unannotated functions in the same files are outside the proof, and Verus
refuses to let verified code call them.

## Running the proofs

```bash
proofs/run.sh /path/to/verus        # or: just verify-proofs /path/to/verus
```

A successful run ends with `verification results:: N verified, 0 errors`.
The [`Verus Proofs`](../.github/workflows/verus-proofs.yml) workflow builds
the pinned Verus and runs the same command.

### Pinned toolchain

| Component | Pin |
|---|---|
| Verus | commit `48ed2e8a36716eecc22057111681d8d29993b701` |
| Rust (for Verus only) | `1.100.0`, from Verus's `rust-toolchain.toml` |
| Z3 | `4.16.0`, the `z3` binary from the PyPI wheel `z3_solver-4.16.0.0-py3-none-manylinux_2_27_x86_64.whl` (SHA-256 `afae2551f795670f0522cfce82132d129c408a2694adff71eb01ba0f2ece44f9`) |

The product itself still builds with Rust 1.85 or newer; only the proof
check needs the Verus toolchain.

### Building Verus locally

```bash
git clone https://github.com/verus-lang/verus.git
cd verus/source
git checkout 48ed2e8a36716eecc22057111681d8d29993b701
# Place a Z3 4.16.0 binary at ./z3 (see the pin above), then:
rustup toolchain install
cargo build --release -p rust_verify -p verus -p cargo-verus
./target-verus/release/cargo-verus build --release --manifest-path vstd/Cargo.toml
export VERUS_Z3_PATH="$PWD/z3"
```

Invoke `target-verus/release/verus` by that path (or through a symlink):
Verus locates its verified standard library next to the binary.

## Trusted base

A proof is only as strong as what it assumes. These kernels trust:

- the Verus verifier, the Rust 1.100 front end it embeds, and Z3;
- `vstd`'s specifications of the standard-library operations the kernels
  call: `str` as a sequence of Unicode scalar values with its UTF-8
  encoding (`as_bytes`, `len`, `is_empty`, `chars`, `split_at` at a char
  boundary, range slicing, `to_owned`); `char::is_whitespace` as the
  Unicode `White_Space` property, `char::len_utf8`, and
  `char::is_ascii_digit`; `String::new`, `push`, and `push_str`; `Vec::new`,
  `with_capacity`, `push`, `pop`, `as_slice`, `vec!`, indexing, and length;
  `usize::checked_add`, `checked_mul`, and `saturating_add`;
  `RangeInclusive::contains`; `Option::is_some`, `Result::is_ok`, and the
  `?` operator; and derived `==` on kernel enums;
- inputs that glue computes with `std` and hands to a kernel:
  `Path::components`, `Path::file_name`, and `Path::strip_prefix` results,
  `OsStr` conversions and UTF-16 encoding, `str::to_uppercase` for Windows
  device names, file metadata (`is_dir`, `is_file`, link checks, permission
  modes), and `std::env` values;
- that the stable `rustc` used for the product (1.85 or newer) gives the
  same meaning to the verified Rust as Verus's front end does;
- the unverified glue that calls the kernels: reading the guide file,
  `str::lines`, mapping kernel verdicts to `SyntaxError` values and messages,
  and all filesystem access.

No kernel uses `assume`, `admit`, `external_body`, or
`assume_specification`.

## What is verified

Each row names the production decision, where it lives, and the main
theorems. Every kernel's executable code is proven equal to its
specification; the theorems then state the contract-level properties of
that specification. Theorem names are searchable in the source.

### Guide language (`check` and `verify`)

| Decision | Code | What Verus proves |
|---|---|---|
| Line endings | `src/proven/document.rs` | `first_lone_cr_line` returns no line exactly when every CR is followed by LF, and otherwise reports the first lone CR's line. |
| Marker lines | `src/proven/marker.rs` | `marker_class` restates the contract's marker productions as existential decompositions of the line; the scanner is proven equal to it on every byte sequence, including the outer-whitespace, `ignore=true`, and BOM rules. |
| Guide envelope | `src/proven/document.rs` | `theorem_envelope_accepts_exactly`: a document is accepted exactly when it has one opening marker, one later closing marker, and otherwise only ordinary lines; `theorem_envelope_reports_first_offense` and the missing-marker theorems fix which line is reported. |
| List items and indentation | `src/proven/layout.rs` | The list-item splitter matches `^(\s*)- (.+)$` with Unicode `White_Space` and exact byte offsets. `theorem_layout_sound`, `theorem_layout_complete`, and `theorem_layout_first_error`: the line fold accepts exactly the bodies that satisfy the indentation rules (unit 1–16, multiples of the unit, depth at most 256, first line at depth 0, one level deeper only below a line that can own children) and rejects at the first failing line. |
| Entry text | `src/proven/entry.rs`, `src/proven/choice.rs` | `parse_entry` equals `entry_spec`. `theorem_placeholder_iff_bare_ellipsis`; `theorem_bare_paths_are_contained` (every bare or choice path the parser accepts is one the validator accepts); `theorem_choice_list_shape` (2–256 pairwise-distinct sibling expansions); `theorem_quoted_path_decoder`; `theorem_comment_is_normalized`. |
| Path validation | `src/proven/path_rules.rs` | `check_path_structure` carries the validator's exact precedence. `theorem_valid_iff_contained`: a path is valid exactly when it is a non-empty `/`-join of non-empty, non-dot, control-free components (with the root-level backslash and drive-prefix rules); `theorem_valid_path_stays_below`: no prefix of a valid path names its root or an ancestor of it. |
| Duplicate paths and placeholder siblings | `src/proven/tree_rules.rs` | `theorem_duplicate_check_decides_decoded_identity`: a guide is rejected exactly when two entries decode to the same full path (through any hierarchy spelling), at the first repeat in preorder; the check is a verified linear hash index whose keys affect only running time. `theorem_placeholder_rules`: sibling lists are accepted exactly when no two placeholders are adjacent and no placeholder has children. |
| Name codec and diagnostics | `src/path_codec.rs` (in place) | `serialize_component` meets the exact quoting rules; `theorem_serialize_injective`, `theorem_quoted_component_decodes`. The diagnostic renderers are proven delimited, decodable, injective, and free of C0 controls and DEL (`theorem_rendered_*`). |
| Dump → parse round trip | `proofs/round_trip.rs` | See [Round trip](#round-trip). |

### Generation (`dump` and `init`)

| Decision | Code | What Verus proves |
|---|---|---|
| Configuration and depth limits | `src/proven/generation.rs` | `theorem_configuration_table`; `theorem_explicit_depth_is_exact` (an explicit `--depth` never errors and bounds the logical depth); `theorem_default_depth_fails_closed`. The CLI's `--indent` and `--depth` parsing uses the same verified bounds; clap's argument order decides which error wins when both are out of range. |
| Guide lines | `src/proven/generation.rs` | `format_entry_line` emits exactly indentation, `- `, the name, an optional `/`, and a newline (`entry_line_spec`). |
| Name admission and integers | `src/proven/generation.rs` | `host_name_verdict` is the shared representability rule; `parse_unsigned_decimal` equals `str::parse::<usize>`. |
| Output destinations | `src/proven/output_path.rs` | `theorem_output_spelling_rule`, `theorem_accepted_unix_output_names_a_file`, `theorem_accepted_windows_output_names_a_file` (an accepted `--output` spelling ends in a non-empty, separator-free final segment that is not `.` or `..`); `theorem_plain_descent_walks_to_its_final_entry`; `theorem_mode_rule`. |

### Filesystem decisions (`verify`, recursive discovery)

| Decision | Code | What Verus proves |
|---|---|---|
| Entry types | `src/entry_type.rs` (in place) | `theorem_fail_closed`: a supported kind is returned exactly when there is no reparse, link, FIFO, socket, or device evidence and exactly one of the file and directory flags is set. |
| Placeholders and resolution | `src/proven/placeholder.rs` | `theorem_placeholder_acceptance`; `theorem_resolution_descends_only_through_directories`. A comment satisfies a placeholder when it has a non-`White_Space` scalar, which is wider than the parser's SP/HTAB comment trimming. |
| Recursive discovery | `src/proven/discovery.rs` | The per-entry decision: an excluded entry gets no action, a link is never descended or accepted as a guide, a guide-named entry is always validated and never descended, and only non-link, non-excluded directories are descended. That the traversal consults the exclusion verdict before metadata access and pushes exactly the descended entries is a property of the unverified loop in `src/recursive.rs`, which `pushed_entries` models. |
| Guide authority and entry checks | `src/proven/authority.rs` | `theorem_implicit_authority_stays_beneath` (implicit authority never takes the external or parent-reduction route), `theorem_explicit_authority_routes_by_place`, `theorem_walk_step_rejects_links_and_non_directory_ancestors`, `theorem_walk_accepts_only_real_directory_chains`, `theorem_link_classification_precedes_type`. |
| Guide-path reduction | `src/proven/anchor.rs` | The reduction is an ordered plan that the glue executes. `theorem_plan_follows_components`, `theorem_every_erased_component_is_inspected`, `theorem_reduction_tracks_lexical_depth`, `theorem_left_path_is_never_in_anchor`. |
| Windows spellings | `src/proven/windows_names.rs` | Namespace prefixes and the component verdict in its exact order; `theorem_accepted_component_is_ordinary`. Unicode uppercasing stays a trusted `std` input. |
| Exclusion patterns | `src/proven/glob.rs` | `compile_pattern` returns exactly the grammar-defined `compiled_from(text)`, including which diagnostic it reports, and the matcher equals a declarative glob semantics. Over pattern text: `**` excludes exactly the non-empty paths; `*`, `?`, escapes, ranges, and plain names behave as the contract states; `a/**` and `a/**/b` match as expected; and, given components that are non-empty and contain no `/` (which `Path::components` guarantees), wildcards never cross a separator. |
| Configuration precedence | `src/proven/settings.rs` | `theorem_consulted_iff_relevant_and_unshadowed` (an environment variable is read exactly when it applies to the command and nothing on the command line shadows it), `theorem_explicit_path_shadows_implicit_name`, `theorem_scope_table`, `theorem_choices_are_exact`, `theorem_log_precedence`, `theorem_execution_mode_precedence`. |
| Outcomes and exit status | `src/proven/outcome.rs` | `theorem_exit_status_table`, `theorem_denied_ignored_never_succeeds`, `theorem_recursive_run_succeeds`. Once arguments and environment defaults are resolved, `main` routes every exit, success included, through the verified table; clap's own exits (usage errors, `--help`, `--version`) do not pass through it. No decision takes the log level, although a stdout write that only non-quiet mode performs can still fail in the unverified output glue. |

### Round trip

[`round_trip.rs`](round_trip.rs) proves the textual half of the contract's
Core invariant by composing the kernels' specifications:

- `theorem_entry_line_round_trip`: for every name that `dump` accepts at a
  level (representable, non-empty, not `.` or `..`, no `/`), every depth, and
  every indentation, the emitted line is not a marker candidate, is a list
  item of exactly that indentation, parses back to exactly that name and
  kind with no comment, and passes path validation.
- `theorem_body_round_trip` and `theorem_parse_recovers_emission`: a
  preorder emission (first depth 0, at most 256, one level deeper only
  below a directory) is accepted by the layout fold with exactly those
  depths, and every line yields its original name and kind.
- `theorem_tree_round_trip` and `theorem_tree_emission_injective`: a tree's
  preorder emission satisfies those conditions, and distinct trees never
  produce the same emission, so the parsed items determine the tree.
- `theorem_dump_round_trip` and `theorem_init_round_trip`: the complete
  `dump` and `init` documents contain no carriage return, split into exactly
  the expected lines, and the envelope accepts them with the body between
  the markers.

Some kernel mutations, such as emitting `#` unquoted or capping the
indentation unit at 8, still verify on their own and are caught only by
these theorems.

The theorems model, rather than verify, this glue: the dumper's traversal,
sorting, and recursion in `format_tree`; `str::lines` and `join`; the
parser's per-line loop, item assembly, and `build_hierarchy`; the
validator's nesting and indentation checks; file I/O and
UTF-8 decoding; and the filesystem half of `verify`. The module
documentation in `round_trip.rs` states each correspondence.

### Measuring coverage

About a quarter of the product's executable lines (excluding tests,
comments, and ghost code) sit inside verified functions. Those lines are
the pure decisions above; most of the remainder performs filesystem I/O,
argument parsing, and output.

### Keeping the proofs honest

[`tests/verus_kernel_discipline.rs`](../tests/verus_kernel_discipline.rs)
runs in ordinary `cargo test`, without Verus, and fails if:

- `cfg(verus_keep_ghost)` gates anything other than proof syntax, so the
  verified and the shipped executable code can never differ;
- any source file uses a proof escape (`assume`, `admit`,
  `external_body`, `assume_specification`, `external`, or an exemption from
  termination checking); or
- a file carries specifications but is unreachable from `verus_root.rs`.

## What is not verified, and why

- **Filesystem access and trust checks.** Opening guides without following
  links, directory enumeration, entry classification syscalls, and identity
  checks depend on operating-system state that `vstd` does not model. The
  contract also excludes hostile concurrent mutation, which has no
  sequential specification.
- **Host path semantics.** `std::path` component parsing, Windows prefixes,
  `OsStr` conversions, and Unicode case mapping have no Verus model; kernels
  consume their already-split outputs instead.
- **Command-line parsing, environment reads, logging, and output streams.**
  These are `clap`, `std::env`, `env_logger`, and formatting plumbing; the
  precedence and value-acceptance decisions over them are verified.
- **Diagnostic assembly.** Messages are built with `format!`, which Verus
  cannot reason about; the name and path renderers they embed are verified.
- **Resource exhaustion.** Allocation failure and stack depth are not
  modeled.

These areas remain covered by the test suite described in
[CONTRIBUTING.md](../CONTRIBUTING.md).
