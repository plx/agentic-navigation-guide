# Issue #58 hermetic integration-test evidence

Date: 2026-07-27

Issue: [#58](https://github.com/plx/agentic-navigation-guide/issues/58)

## Result

**PASS.** CLI subprocess tests now start from an owned temporary working
directory with all five guide configuration variables removed. The harness
cleans that directory when the command is dropped, while tests of intentional
current-directory behavior select their fixture explicitly.

The approved #36 handoff on #58 replaces the obsolete positive-library
requirements. The supported Rust facade set is empty. This issue does not
restore a library, add a test-only facade, or create a positive downstream
Rust consumer.

## Tests-first evidence

Commit `b37fc0f` added the harness contract before changing the helper. Against
the component baseline `c85f7bfaf7fa3f952e9ead4f93b73295e5fb6c8e`:

- the explicit product-current-directory test passed; and
- the default helper test failed because it inherited the process current
  directory and all five guide configuration variables.

The focused result was 1 passed and 1 failed. After the implementation, the
same contract passes and also proves that dropping each shared helper removes
its temporary working directory.

## Hermetic subprocess harness

`tests/support/assert_cli.rs` owns the `assert_cmd` helper used by the main CLI,
environment-precedence, and parent-containing-guide suites.
`tests/support/process_cli.rs` owns the raw-process helper used where tests need
direct spawning or process output.

Both helpers:

1. create an independent `TempDir`;
2. set it as the child command's default current directory;
3. remove `AGENTIC_NAVIGATION_GUIDE_PATH`,
   `AGENTIC_NAVIGATION_GUIDE_ROOT`, `AGENTIC_NAVIGATION_GUIDE_NAME`,
   `AGENTIC_NAVIGATION_GUIDE_LOG_MODE`, and
   `AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE`; and
4. retain ownership until the command wrapper is dropped.

The assert-style helper also applies the existing five-second subprocess
timeout. Tests may override the child current directory or set a configuration
variable only when that behavior is the subject of the test.

## Integration-test inventory

| Suite | Filesystem/process policy |
| --- | --- |
| `tests/cli_tests.rs` | Shared assert-style helper; generation and verification tests pass explicit temporary roots and guide/output paths. |
| `tests/environment_precedence.rs` | Shared assert-style helper; every current-directory or environment input is an explicit case fixture. |
| `tests/issue_101_parent_explicit_guide.rs` | Shared assert-style helper; explicit temporary anchors and guide spellings. |
| `tests/issue_47_output_contract.rs` | Shared raw-process helper; explicit roots, guide paths, and output paths. |
| `tests/issue_68_normative_source.rs` | Shared raw-process helper; its one parser subprocess selects the temporary historical-example directory explicitly. |
| `tests/issue_66_readme_examples.rs` | Its product-command constructor requires an explicit current directory and removes all five guide variables. |
| `tests/issue_62_package_boundary.rs` | Installed-binary commands require an explicit current directory and remove all five guide variables. Cargo packaging reads the checkout intentionally as the artifact under test. |
| `tests/issue_54_binary_only_package.rs` and `tests/issue_64_release_identity.rs` | Cargo metadata/package/install tests intentionally read the candidate checkout; product smoke commands use isolated package/install directories. |
| Documentation and policy tests | Read named repository files as their explicit subject; they do not run filesystem discovery against the checkout. |

The issue #58 contract walks all integration-test Rust sources and rejects
process-global `set_current_dir`, `set_var`, or `remove_var` calls. Child-only
`Command::current_dir`, `env`, and `env_remove` remain safe under parallel test
execution.

The original `test_init_command` now has both an explicit `TempDir` and
`--root`. The separate
`issue_58_product_current_directory_default_is_covered_explicitly` test changes
the child directory to a temporary root before testing the product's intended
omitted-`--root` behavior. No ordinary command inherits the checkout.

## Transient entry behavior

Issue #42 already supplied the required deterministic product result in
`issue_42_transient_classification_failure_aborts_collection`. It injects one
fixed `NotFound` classification result for `vanished.txt` and proves:

- collection aborts instead of silently omitting the entry;
- the logical path and typed `NotFound` class remain actionable;
- the injected internal detail is redacted; and
- the physical temporary root is not disclosed.

This is a direct internal-engine unit test, separate from test-fixture
isolation. Issue #58 retains that owner instead of attempting to reproduce an
uncontrolled filesystem race.

## Empty supported Rust facade

The approved issue comment requires direct proof of the binary-only boundary,
not positive library coverage:

- #54's workspace and packaged metadata test rejects every linkable target
  kind, `src/lib.rs`, and externally public Rust visibility;
- #62's exact packaged-artifact test installs and smokes the CLI, then builds
  a downstream Rust consumer and requires Cargo/rustc to reject it because the
  dependency has no library target; and
- CI runs both gates, including #62's intentionally ignored expensive package
  acceptance test.

`tests/issue_58_test_hermeticity.rs` binds those owners to this acceptance
record and fails if a library target appears or either CI command disappears.
No supported facade entry point is skipped: the complete set is empty.

## Continuous and local validation

The three-OS build matrix runs:

```text
cargo test --locked --test issue_58_test_hermeticity -- --nocapture
```

The complete local gate is:

```text
cargo test --locked --all-targets --all-features
cargo test --locked --all-targets --all-features -- --test-threads=1
cargo test --locked --all-targets --all-features --release
cargo test --locked --test issue_62_package_boundary \
  issue_62_exact_package_installs_smokes_and_rejects_library_consumers \
  -- --exact --ignored --nocapture
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo run --locked -- check --guide AGENTIC_NAVIGATION_GUIDE.md
cargo run --locked -- verify --guide AGENTIC_NAVIGATION_GUIDE.md --root .
just --fmt --check
git diff --check
```

One bounded parallel validation runs the integration suite while Cargo builds
into an independent temporary target directory. The test fixtures remain
independent because product subprocesses start from owned temporary roots.

No randomized input generation, property generation, mutation testing, or
fuzzing is added or required by this issue.
