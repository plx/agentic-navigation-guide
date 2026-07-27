# Issue #59 coverage, mutation, and performance evidence

Date: 2026-07-27

Issue: [#59](https://github.com/plx/agentic-navigation-guide/issues/59)

## Decision and scope

The approved #36 handoff makes the packaged product CLI-only. Coverage therefore
measures the private parser, dumper, validator, verifier, recursive discovery,
and CLI routes through binary-unit and subprocess tests. It does not restore a
library target or count the deliberate absence of a public Rust facade as
untested behavior.

This gate uses deterministic tests and fixed sequential fixtures. It performs no
random-input generation or fuzzing.

## Branch-aware coverage

The `branch-coverage` CI job starts with `cargo llvm-cov clean --workspace` and
uses pinned `cargo-llvm-cov 0.8.7`, pinned nightly `2025-11-04`, every target,
every feature, and LLVM branch instrumentation. It uploads JSON, LCOV, and HTML
reports. `scripts/check_coverage.py` fails closed if branch instrumentation or a
critical module is absent, overall line coverage is below 85%, overall branch
coverage is below 80%, or a critical module is below 85% line coverage.

The implementation-time clean measurement was:

| Scope | Lines | Branches |
| --- | ---: | ---: |
| Overall | 88.41% (6775/7663) | 81.57% (770/944) |
| Parser | 91.51% (1520/1661) | 87.97% (234/266) |
| Dumper | 86.80% (572/659) | 87.50% (42/48) |
| Validator | 93.25% (428/459) | 85.29% (58/68) |
| Verifier | 91.51% (1304/1425) | 63.73% (65/102) |
| Recursive discovery | 87.68% (434/495) | 84.62% (66/78) |
| CLI aggregate | 88.04% (1568/1781) | 81.73% (170/208) |

The verifier's branch percentage is reported rather than hidden. Its line
coverage clears the approved critical-module floor; the lower branch result is
largely platform/capability and defensive error classification. Overall branch
coverage remains a hard gate. The checker uses count-weighted CLI aggregation,
not an average of file percentages.

`guide_input.rs` is part of the overall gate and has exact trust-boundary
regressions in the table below, but is not mislabeled as an 85%-covered module:
the hosted Linux report measures it at 71.05% (373/525), with mutually
exclusive Windows implementation paths and defensive I/O-race errors making up
most of the unexecuted lines. The approved critical-module floor remains scoped
to parser, dumper, validator, verifier, recursive discovery, and the CLI
aggregate.

## Original P0/P1 traceability

Every original release blocker and high-priority audit defect has an exact
executable owner:

| Original audit defect | Exact regression owner |
| --- | --- |
| Wrong-parent hierarchy attachment | `parser::tests::test_rejects_audited_child_under_intervening_file`, `test_rejects_direct_child_beneath_file`, and `test_rejects_stale_parent_after_dedent_to_file` |
| Dump/init output does not round-trip | `v0_2_contract_tests::issue_41_supported_filesystem_names_round_trip_canonically`, `issue_41_generation_is_all_or_nothing_and_diagnostics_are_control_safe`, `issue_42_directory_links_never_generate_non_round_trippable_guides`, and `dumper::tests::issue_43_invalid_roots_and_empty_generation_reject` |
| Malformed opening marker activates ignore | `parser::tests::test_rejects_malformed_opening_marker_candidates` and `cli_tests::test_issue39_malformed_marker_never_activates_ignore` |
| Recursive zero discovery succeeds | `cli_tests::test_recursive_verify_zero_discovery_is_fail_closed_unless_explicitly_allowed` and `recursive::tests::empty_result_slice_is_an_absent_failure_not_vacuous_success` |
| Guide-file link crosses the read boundary | `cli_tests::test_rejected_recursive_guide_never_reads_or_discloses_its_target_in_any_mode`, `test_guide_input_trust_policy_matrix`, and `containment_guarantee_tests::issue_51_cli_and_internal_route_share_containment` |
| Init follows a dangling output link or loses a creator race | `cli_tests::test_init_rejects_dangling_output_symlink_without_creating_target`, `init_competing_creator_never_gets_overwritten_in_100_races`, and `cli::output::tests::exclusive_create_has_exactly_one_winner_for_100_races` |
| No publishable release identity | `issue_64_release_identity::issue_64_published_api_baseline_is_complete_and_changeloged`, `issue_64_exact_package_has_the_prepared_identity_and_installs`, and `tests/test_check_release_identity.py` |
| Inconsistent `ignore=true` semantics | `cli_tests::test_issue39_ignored_body_and_policy_matrix`, `v0_2_contract_tests::library_ignored_gate_requires_non_vacuous_absence_of_supported_facades`, and `parser::tests::test_rejects_malformed_marker_candidate_inside_ignored_envelope` |
| Repeated trailing separators normalized before validation | `v0_2_contract_tests::issue_40_path_normalization_boundaries_are_executable` |
| Quoted choice whitespace not preserved | `v0_2_contract_tests::issue_40_choice_token_preservation_is_executable` |
| Placeholder identity false positives | `filesystem_identity_snapshot_tests::issue_50_case_identity_is_exact_or_capability_is_explicit`, `issue_50_unicode_identity_is_exact_or_capability_is_explicit`, and `issue_50_placeholder_matrix_preserves_partial_guide_semantics` |
| Nested basename exclusions do not match | `exclusion_semantics_tests::issue_44_basename_and_root_relative_patterns_are_distinct`, `issue_44_init_vcs_defaults_apply_at_root_and_nested_depths`, and `recursive::tests::issue_44_excluded_directories_do_not_reach_the_enumerator` |
| Environment values conflict instead of defaulting | all eight cases in `tests/environment_precedence.rs` |
| Unfinished public symlink model | `v0_2_contract_tests::issue_53_removed_symlink_model_is_absent_but_its_ledger_rows_remain` |
| Incorrect public full-path helper | `v0_2_contract_tests::issue_52_removed_full_path_method_is_absent_but_its_ledger_row_remains` |
| Numeric inputs panic or flatten output | `dumper::tests::issue_43_numeric_bounds_are_enforced_without_panics`, `issue_43_valid_numeric_boundaries_generate_checkable_guides`, and `cli_tests::issue_43_cli_numeric_bounds_reject_before_generation` |
| Broken pipe panics | `issue_47_output_contract::issue_47_dump_closed_stdout_is_normal_unix_termination` |
| Quiet/GitHub Actions output drifts | `issue_47_output_contract::issue_47_quiet_init_creates_without_ordinary_output`, `issue_47_recursive_github_error_has_discovery_path_and_line`, and `issue_47_command_log_and_execution_mode_matrix_is_stable` |
| Platform-sensitive behavior lacks behavioral ownership | the three-OS `build` matrix runs README, hermeticity, entry-type, generation, exclusion, environment, identity, containment, binary-boundary, contract, logical-backslash, and syntax-sensitive-name regressions |
| Hierarchy and placeholder paths are quadratic | `parser::tests::test_hierarchy_work_is_linear_and_stack_is_bounded`, `filesystem_identity_snapshot_tests::issue_50_repeated_placeholders_enumerate_the_parent_once`, and the fixed #59 performance job |
| Stable-tree containment limitation was unstated | `containment_guarantee_tests::issue_51_hostile_replacement_is_characterized_as_unsupported` and the #51 observed-change regressions |
| Integration test uses the checkout as its fixture | `issue_58_test_hermeticity::issue_58_assert_cli_harness_is_hermetic_and_cleans_its_default_root`, `issue_58_process_cli_harness_is_hermetic_and_cleans_its_default_root`, and `cli_tests::issue_58_product_current_directory_default_is_covered_explicitly` |
| Public surface is broader than support intent | `v0_2_contract_tests::api_ledger_matches_the_realized_binary_only_cargo_target` and `issue_54_binary_only_package::issue_54_workspace_and_packaged_metadata_are_binary_only` |
| Workflow/MSRV/package gates were absent | `tests/issue_61_workflow_security_policy.rs`, `tests/issue_60_msrv_dependency_policy.rs`, and `tests/issue_62_package_boundary.rs` |
| README/specification ambiguity | `tests/issue_66_readme_examples.rs` and `tests/issue_68_normative_source.rs` |

This table is the binding map for #59. Broader release-artifact and final-audit
work remains owned by its later issues.

## Reviewed mutation sentinels

The mutation campaign is deliberately bounded to 15 deterministic,
original-blocker sentinels in parser marker classification and hierarchy
construction, dump generation, syntax validation, verification/snapshot use,
and recursive discovery/zero-result aggregation. CI pins `cargo-mutants
27.1.0`, requires the successful unmodified baseline, and uploads its complete
report. `scripts/check_mutation_report.py` rejects a missing/wrong tool, a
missing module or mutant, any survivor, any timeout, or a failed baseline.

Implementation-time disposition:

| Disposition | Count | Explanation |
| --- | ---: | --- |
| Caught by tests | 10 | The relevant original-blocker regressions failed |
| Unviable | 5 | The proposed replacement required `Default` for `MarkerLine`, `NavigationGuideLine`, `VerificationAggregate`, `GuideLocation`, or `DirectorySnapshot`; those types intentionally do not implement it |
| Survived | 0 | Hard-gate count |
| Timed out | 0 | Hard-gate count |

The unviable diffs and compiler logs remain in the uploaded artifact; they are
not silently counted as caught. Package-boundary, release-identity, and README
package-lifecycle tests are skipped only inside the copied mutation checkout
because their dedicated CI jobs are authoritative and they do not exercise the
five mutated modules.

## Fixed performance and resource baseline

`scripts/run_performance_baseline.py` builds no generated or random inputs. Its
fixture seed is the literal `fixed-sequential-v1`; names and structure are
sequential and reproducible. Each case receives one warmup and five measured
release-mode subprocesses. The JSON report records median, nearest-rank p95,
maximum child RSS, binary SHA-256, fixture seed, filesystem, OS, and Rust
toolchain. Every measured subprocess is reaped independently with `wait4`; its
per-child resource record is used directly, so a large earlier fixture cannot
contaminate a later fixture's RSS value.

The versioned implementation baseline is
`benchmarks/issue-59-baseline.json`:

| Case | Median | p95 | Maximum RSS |
| --- | ---: | ---: | ---: |
| Flat 10k | 0.0108 s | 0.0109 s | 19.42 MiB |
| Flat 20k | 0.0208 s | 0.0211 s | 19.42 MiB |
| Flat 40k | 0.0397 s | 0.0401 s | 19.42 MiB |
| Flat 100k | 0.0943 s | 0.0945 s | 32.86 MiB |
| Deep valid, depth 256 | 0.0031 s | 0.0031 s | 19.42 MiB |
| Deep invalid, depth 257 | 0.0020 s | 0.0022 s | 19.42 MiB |
| 500 placeholder entries | 0.0094 s | 0.0094 s | 19.42 MiB |
| 1,000 placeholder entries | 0.0174 s | 0.0176 s | 19.42 MiB |
| 2,000 placeholder entries | 0.0339 s | 0.0345 s | 19.54 MiB |
| 200 recursive roots | 0.0318 s | 0.0322 s | 19.54 MiB |
| Repository self-verification | 0.0054 s | 0.0055 s | 19.54 MiB |

This reference was captured by the first hosted Ubuntu run on ext4 with Rust
1.97.1; the artifact records the exact kernel, binary hash, and raw values.
Flat exact doublings scale by 1.92x and 1.91x. Placeholder exact doublings
scale by 1.86x and 1.94x. Both remain below 2.5x and provide direct evidence
that #37 and #50 removed the audited quadratic trends. The 100k and self-check
absolute thresholds have wide margins.

CI compares every current median and RSS result with the versioned reference.
A value may rise by at most 20%, with a 10 ms timing and 8 MiB RSS measurement
resolution allowance for very small cross-run values. Any larger change needs
an explicit reviewed baseline update and analysis. The hard 2.5x, five-second,
256 MiB, and one-second limits apply independently.

## Fail-closed evidence

- `tests/test_check_coverage.py` proves absent branch instrumentation, missing
  modules, and under-floor measurements fail.
- `tests/test_check_mutation_report.py` proves missing/incomplete runs,
  survivors, timeouts, and baseline failures fail.
- `tests/test_performance_baseline.py` proves missing metadata/cases, wrong
  outcomes, scaling/resource overruns, reference regressions, and accidental
  replacement of per-child RSS with a process-lifetime high-water mark fail.
- Each CI job uses a pinned toolchain/tool version, explicit timeout and
  read-only permissions, and `if-no-files-found: error` artifact publication.

No public facade, fuzz target, random fixture generator, or low-value
assertion-free test was added.
