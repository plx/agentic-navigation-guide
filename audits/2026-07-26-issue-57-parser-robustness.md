# Issue #57 Deterministic Parser Robustness Evidence

Date: 2026-07-26

## Scope and Outcome

Issue #57 validates the navigation-guide parser against a bounded,
deterministic, reviewable input matrix using the ordinary stable Rust test
toolchain. The repository owner explicitly authorized replacing the original
mutation-fuzzer proposal with this parser-only validation:

- no `cargo-fuzz`, libFuzzer, nightly-only workspace, sanitizer campaign, or
  long-running mutation workflow is present;
- no headless external model was invoked to implement or generate these tests;
  the repository's pre-existing automated PR reviewer remains unchanged;
- verifier and filesystem-containment behavior remain outside this issue; and
- the installed binary and its production parser implementation are unchanged.

The live issue title and acceptance text were updated before the implementation
was published so this evidence does not claim a campaign that was deliberately
not run.

Implementation commit:

```text
01adb9576869b8a7f09a3ad76a2c41f1244bc9ff
```

Base revision:

```text
213b9bfcd88ad1ad620275ddf533e2b645f511fb
```

## Tests-First Evidence

The replacement test module was registered in `src/main.rs` before its source
file existed. The focused command:

```sh
cargo test --locked issue_57_parser_robustness -- --nocapture
```

exited `101` with Rust error `E0583`, identifying the missing
`src/parser_robustness_tests.rs`. This proves the ordinary deterministic test
surface was absent before implementation.

An earlier structural test for the original mutation-fuzzer proposal also
failed because the fuzz workspace, corpus manifest, and runner did not exist.
After the owner-approved scope change, that unpushed commit was rewritten and
the abandoned test was removed. Neither the obsolete test nor any fuzz
workspace appears in branch history or the final tree.

The first implementation run exposed one incorrect test expectation:
`file[a,,b]` is valid because the grammar permits an empty alternative within
a choice of two through 256 alternatives. The test was corrected to require
the exact expansions `filea`, `file`, and `fileb`; no product code changed.

## Persistent Deterministic Matrix

`src/parser_robustness_tests.rs` contains five normal binary-unit tests.
`src/main.rs` includes the module only under `cfg(test)`. Like the existing
binary-unit test modules, its source remains in published packages so
`cargo test` on an unpacked package can resolve every `cfg(test)` module.
`Cargo.toml` excludes only `.context/**`, preserving local operator notes while
preventing Cargo from including ignored workspace state in an archive.

### Full UTF-8 documents

- 2,048 inputs are generated from a fixed, source-controlled sequence and a
  reviewed 30-fragment alphabet.
- Cases alternate among raw documents, active guide envelopes, valid blocks
  after generated prologues, and ignored envelopes.
- Explicit fixtures cover empty and blank guides, LF, CRLF, lone carriage
  returns, duplicate blocks, BOM-prefixed markers, and a 65,000-character
  path line.
- Every input is at most 65,536 bytes.

The same source is parsed and syntax-validated twice. Both observations,
including accepted trees and rejected diagnostics, must compare exactly.

### Exact markers and attributes

The positive matrix covers:

- bare markers with four outer horizontal-whitespace forms; and
- 648 exact `ignore=true` combinations across SP/HTAB name separation,
  whitespace around `=`, quoted/unquoted values, and whitespace before `>`.

The negative matrix directly covers concatenated and suffixed markers, missing
separation, typos, plural names, unknown/duplicate/wrong-case attributes,
false and malformed values, invalid quoting, missing terminators, and
BOM-prefixed marker candidates. A malformed candidate must reject and can
never activate `ignore`.

### Choices, escapes, and identity

The tests require exact paths for quoted commas, escaped commas, literal
brackets, comments, and escapes. They prove:

- 256 alternatives accept and 257 reject;
- empty alternatives retain their documented identities;
- empty blocks, whitespace-only blocks, multiple blocks, unmatched delimiters,
  unterminated quotes, and trailing escapes reject; and
- two source spellings that decode to `report#draft` reject as a duplicate
  validated filesystem identity.

### Indentation and hierarchy

Every supported indentation unit from one through 16 spaces is exercised at
depths 1, 2, 8, and 16. A one-space tree exercises the exact logical depth 256
boundary, while depth 257 rejects. Direct negative cases cover children beneath
files, placeholders, and choice-expanded lines; an indented first item; mixed
SP/HTAB indentation; skipped depth; and repeated path separators.

A 1,024-item wide guide must retain first/last order. Accepted trees are walked
iteratively to require:

- stored indentation equals actual tree depth;
- depth does not exceed 256;
- validated full filesystem identities remain unique;
- item-count arithmetic does not overflow; and
- accepted expansion count is no greater than physical source lines times 256.

### Serialized component round-trips

One active guide contains distinct serialized components for ordinary,
hash-bearing, bracket-bearing, comma-bearing, quote-bearing, backslash-bearing,
leading/trailing-space, literal `...`, composed/decomposed Unicode, and emoji
names. Parsing must recover the exact ordered scalar sequences without
trimming, placeholder reinterpretation, normalization, or identity collapse.

## Validation Evidence

The focused suite passed:

```text
5 passed; 0 failed; 217 filtered out
```

The complete locked suite passed:

```text
binary unit tests:       220 passed; 2 intentionally ignored
CLI integration tests:  106 passed
environment tests:        8 passed
package-shape tests:       1 passed
release-identity tests:    2 passed
total:                   337 passed; 2 intentionally ignored
```

The following gates also passed:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
just --fmt --check
cargo test --locked --test issue_54_binary_only_package -- --nocapture
cargo package --locked --allow-dirty --list
cargo package --locked --allow-dirty
cargo test --locked \
  --manifest-path target/package/agentic-navigation-guide-0.2.0/Cargo.toml \
  issue_57_parser_robustness -- --nocapture
cargo run --locked -- check --guide AGENTIC_NAVIGATION_GUIDE.md
cargo run --locked -- verify --guide AGENTIC_NAVIGATION_GUIDE.md --root .
```

The package list contains neither `.context/**` nor any abandoned fuzz
material. It intentionally retains `src/parser_robustness_tests.rs`, consistent
with the existing packaged binary-unit tests. The binary-only proof still finds
exactly one product binary and zero Rust-linkable library targets. The focused
five-test suite also passes from Cargo's verified unpacked package.

## Acceptance-Criteria Mapping

- The five required deterministic suites are permanent normal Rust tests.
- The input matrix directly covers every required marker, choice, escape,
  hierarchy, line-ending, resource-bound, and component-name category.
- Identical-input determinism, accepted-tree bounds, and exact identity
  preservation are executable assertions.
- The full suite, formatting, lint, packaging, binary-only surface, and
  repository guide gates are green.
- This audit records the exact no-fuzzer decision and does not characterize an
  unrun mutation or sanitizer campaign as passing evidence.
