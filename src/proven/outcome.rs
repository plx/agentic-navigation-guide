//! Verified command-outcome and exit-status policy.
//!
//! These kernels make the two process-level decisions of the v0.2 contract
//! (`docs/v0.2-contract.md`, "Ignored guides" and "Stable streams and exit
//! statuses") that do not depend on any I/O:
//!
//! - [`decide_ignored_policy`]: whether a `check` or `verify` run that
//!   finished its work completed, completed with an allowed ignored result,
//!   or is rejected by `--deny-ignored`;
//! - [`exit_status`]: the process status of a successful run and of a runtime
//!   failure in each execution mode;
//! - [`empty_discovery_decision`]: a recursive search whose discovery
//!   completed with zero guides succeeds only under `--allow-empty`
//!   ("Recursive discovery and empty searches");
//! - [`recursive_completion`]: after recursive verification, whether the run
//!   succeeds, failed, is rejected by `--deny-ignored`, or both.
//!
//! Neither kernel takes a log level: quiet and verbose mode are not inputs to
//! any status decision, by construction of these signatures, so they can
//! change output only, never the outcome or the status.
//!
//! The theorems below state the contract-level consequences:
//!
//! - `theorem_exit_status_table`: success is `0` in every mode, and a runtime
//!   failure is `2` in `post-tool-use` mode and `1` in every other mode, so
//!   a failure is never reported as success;
//! - `theorem_ignored_policy`: a run completes exactly when no guide was
//!   ignored, an ignored result is allowed by default and reported as
//!   ignored (never as completed), and `--deny-ignored` rejects every run
//!   with an ignored guide;
//! - `theorem_denied_ignored_never_succeeds`: under `--deny-ignored`, a run
//!   with an ignored guide exits with a nonzero status in every mode;
//! - `theorem_recursive_run_succeeds`: a recursive run succeeds exactly when
//!   it discovered zero guides under `--allow-empty`, or discovered at least
//!   one guide, none failed, and `--deny-ignored` is off or none was
//!   ignored; `--allow-empty` therefore never excuses a failed or denied
//!   guide, and its status is `0` exactly then;
//! - `theorem_recursive_completion_agrees_with_ignored_policy`: when no guide
//!   failed, the recursive decision is the single-guide ignored-guide policy
//!   applied to the ignored count, and a failure is never accepted.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// The completed outcome of a `check`, `verify`, `dump`, or `init` run.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandOutcome {
    /// Every guide was checked or verified (or the command has no guides).
    Completed,
    /// The run finished, and `count` guides were ignored rather than checked.
    Ignored { count: usize },
}

/// The ignored-guide policy decision for a run that finished its work.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IgnoredPolicyDecision {
    /// The run succeeds with this outcome.
    Accept(CommandOutcome),
    /// `--deny-ignored` rejects the run as a runtime failure.
    Deny,
}

/// The execution mode as the exit-status policy sees it (one variant for
/// each `types::ExecutionMode`).
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StatusMode {
    Default,
    PostToolUse,
    PreCommitHook,
    GitHubActions,
}

/// The decision after a recursive discovery completed with zero guides.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EmptyDiscoveryDecision {
    /// `--allow-empty` accepts the absent search: the run succeeds.
    Accept,
    /// The absent search is a runtime failure.
    Reject,
}

/// The decision after recursive verification of at least one guide.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecursiveCompletion {
    /// The run succeeds with this outcome.
    Accept(CommandOutcome),
    /// Some guide failed; `--deny-ignored` did not reject the run.
    Failed,
    /// `--deny-ignored` rejected the ignored guides; no guide failed.
    Denied,
    /// Some guide failed and `--deny-ignored` rejected the ignored guides.
    FailedAndDenied,
}

#[cfg(verus_keep_ghost)]
verus! {

/// The ignored-guide policy: no ignored guide completes the run; otherwise
/// the run is ignored by default and rejected under `--deny-ignored`.
pub(crate) open spec fn ignored_policy(ignored_count: usize, deny_ignored: bool) -> IgnoredPolicyDecision {
    if ignored_count == 0 {
        IgnoredPolicyDecision::Accept(CommandOutcome::Completed)
    } else if !deny_ignored {
        IgnoredPolicyDecision::Accept(CommandOutcome::Ignored { count: ignored_count })
    } else {
        IgnoredPolicyDecision::Deny
    }
}

/// The runtime-failure status of each execution mode.
pub(crate) open spec fn failure_status(mode: StatusMode) -> i32 {
    match mode {
        StatusMode::Default => 1,
        StatusMode::PostToolUse => 2,
        StatusMode::PreCommitHook => 1,
        StatusMode::GitHubActions => 1,
    }
}

/// The process status of a run: `0` on success and the mode's failure
/// status on a runtime failure.
pub(crate) open spec fn exit_status_spec(mode: StatusMode, is_error: bool) -> i32 {
    if is_error {
        failure_status(mode)
    } else {
        0
    }
}

/// The process status of a run that the ignored-guide policy decided.
pub(crate) open spec fn policy_status(mode: StatusMode, decision: IgnoredPolicyDecision) -> i32 {
    match decision {
        IgnoredPolicyDecision::Accept(_) => exit_status_spec(mode, false),
        IgnoredPolicyDecision::Deny => exit_status_spec(mode, true),
    }
}

/// An absent recursive search succeeds only under `--allow-empty`.
pub(crate) open spec fn empty_discovery_spec(allow_empty: bool) -> EmptyDiscoveryDecision {
    if allow_empty {
        EmptyDiscoveryDecision::Accept
    } else {
        EmptyDiscoveryDecision::Reject
    }
}

/// The recursive decision after verification: `--deny-ignored` with an
/// ignored guide rejects the run (together with any failure); otherwise a
/// failure fails it, and a run without failures is accepted as completed or
/// ignored.
pub(crate) open spec fn recursive_completion_spec(
    all_passed: bool,
    ignored_count: usize,
    deny_ignored: bool,
) -> RecursiveCompletion {
    if deny_ignored && ignored_count != 0 {
        if all_passed {
            RecursiveCompletion::Denied
        } else {
            RecursiveCompletion::FailedAndDenied
        }
    } else if !all_passed {
        RecursiveCompletion::Failed
    } else if ignored_count == 0 {
        RecursiveCompletion::Accept(CommandOutcome::Completed)
    } else {
        RecursiveCompletion::Accept(CommandOutcome::Ignored { count: ignored_count })
    }
}

/// The recursive decision accepts the run.
pub(crate) open spec fn is_accepted(completion: RecursiveCompletion) -> bool {
    match completion {
        RecursiveCompletion::Accept(_) => true,
        _ => false,
    }
}

/// Whether a recursive run succeeds, composed exactly as `verify
/// --recursive` composes the two decisions: an absent search takes the
/// empty-discovery decision, and any other search the completion decision
/// with `all_passed` meaning that no discovered guide failed.
pub(crate) open spec fn recursive_run_succeeds(
    discovered: nat,
    failed: nat,
    ignored_count: usize,
    deny_ignored: bool,
    allow_empty: bool,
) -> bool {
    if discovered == 0 {
        empty_discovery_spec(allow_empty) == EmptyDiscoveryDecision::Accept
    } else {
        is_accepted(recursive_completion_spec(failed == 0, ignored_count, deny_ignored))
    }
}

} // verus!

/// Decide the ignored-guide policy for a run that finished its work with
/// `ignored_count` ignored guides.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == ignored_policy(ignored_count, deny_ignored),
))]
pub(crate) fn decide_ignored_policy(
    ignored_count: usize,
    deny_ignored: bool,
) -> IgnoredPolicyDecision {
    if ignored_count == 0 {
        return IgnoredPolicyDecision::Accept(CommandOutcome::Completed);
    }
    if !deny_ignored {
        return IgnoredPolicyDecision::Accept(CommandOutcome::Ignored {
            count: ignored_count,
        });
    }
    IgnoredPolicyDecision::Deny
}

/// Return the process status: `0` on success, and on a runtime failure `2`
/// in post-tool-use mode and `1` otherwise.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == exit_status_spec(mode, is_error),
))]
pub(crate) fn exit_status(mode: StatusMode, is_error: bool) -> i32 {
    if is_error {
        match mode {
            StatusMode::PostToolUse => 2,
            _ => 1,
        }
    } else {
        0
    }
}

/// Decide an absent recursive search: it succeeds only under
/// `--allow-empty`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == empty_discovery_spec(allow_empty),
))]
pub(crate) fn empty_discovery_decision(allow_empty: bool) -> EmptyDiscoveryDecision {
    if allow_empty {
        EmptyDiscoveryDecision::Accept
    } else {
        EmptyDiscoveryDecision::Reject
    }
}

/// Decide a recursive run after verifying at least one guide, from whether
/// every guide passed, the number of ignored guides, and `--deny-ignored`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == recursive_completion_spec(all_passed, ignored_count, deny_ignored),
))]
pub(crate) fn recursive_completion(
    all_passed: bool,
    ignored_count: usize,
    deny_ignored: bool,
) -> RecursiveCompletion {
    if deny_ignored && ignored_count != 0 {
        if all_passed {
            return RecursiveCompletion::Denied;
        }
        return RecursiveCompletion::FailedAndDenied;
    }
    if !all_passed {
        return RecursiveCompletion::Failed;
    }
    match decide_ignored_policy(ignored_count, deny_ignored) {
        IgnoredPolicyDecision::Accept(accepted) => RecursiveCompletion::Accept(accepted),
        IgnoredPolicyDecision::Deny => RecursiveCompletion::Denied,
    }
}

#[cfg(verus_keep_ghost)]
verus! {

/// Success is `0` in every mode; a runtime failure is `2` exactly in
/// post-tool-use mode and `1` in every other mode, and never `0`.
pub(crate) proof fn theorem_exit_status_table(mode: StatusMode)
    ensures
        exit_status_spec(mode, false) == 0,
        exit_status_spec(mode, true) != 0,
        (exit_status_spec(mode, true) == 2) == (mode == StatusMode::PostToolUse),
        mode != StatusMode::PostToolUse ==> exit_status_spec(mode, true) == 1,
{
}

/// A run completes exactly when no guide was ignored; an ignored result is
/// accepted as ignored (never as completed) unless `--deny-ignored` rejects
/// it; and the count is reported exactly.
pub(crate) proof fn theorem_ignored_policy(ignored_count: usize, deny_ignored: bool)
    ensures
        (ignored_policy(ignored_count, deny_ignored) == IgnoredPolicyDecision::Accept(
            CommandOutcome::Completed,
        )) == (ignored_count == 0),
        ignored_count > 0 && !deny_ignored ==> ignored_policy(ignored_count, deny_ignored)
            == IgnoredPolicyDecision::Accept(CommandOutcome::Ignored { count: ignored_count }),
        ignored_count > 0 && deny_ignored ==> ignored_policy(ignored_count, deny_ignored)
            == IgnoredPolicyDecision::Deny,
        ignored_policy(ignored_count, deny_ignored) == IgnoredPolicyDecision::Deny ==> deny_ignored
            && ignored_count > 0,
{
}

/// Under `--deny-ignored`, a run with an ignored guide never succeeds: its
/// status is nonzero in every execution mode. Without it, the same run
/// succeeds with status `0` and is reported as ignored.
pub(crate) proof fn theorem_denied_ignored_never_succeeds(mode: StatusMode, ignored_count: usize)
    requires
        ignored_count > 0,
    ensures
        policy_status(mode, ignored_policy(ignored_count, true)) != 0,
        policy_status(mode, ignored_policy(ignored_count, true)) == failure_status(mode),
        policy_status(mode, ignored_policy(ignored_count, false)) == 0,
        ignored_policy(ignored_count, false) != IgnoredPolicyDecision::Accept(
            CommandOutcome::Completed,
        ),
{
}

/// A recursive run succeeds exactly when it discovered zero guides under
/// `--allow-empty`, or discovered at least one guide, none failed, and
/// `--deny-ignored` is off or none was ignored. Its status is `0` exactly
/// then, and otherwise the mode's failure status.
pub(crate) proof fn theorem_recursive_run_succeeds(
    mode: StatusMode,
    discovered: nat,
    failed: nat,
    ignored_count: usize,
    deny_ignored: bool,
    allow_empty: bool,
)
    ensures
        recursive_run_succeeds(discovered, failed, ignored_count, deny_ignored, allow_empty) == ((
        discovered == 0 && allow_empty) || (discovered > 0 && failed == 0 && (!deny_ignored
            || ignored_count == 0))),
        (exit_status_spec(
            mode,
            !recursive_run_succeeds(discovered, failed, ignored_count, deny_ignored, allow_empty),
        ) == 0) == recursive_run_succeeds(
            discovered,
            failed,
            ignored_count,
            deny_ignored,
            allow_empty,
        ),
        discovered > 0 && failed > 0 ==> !recursive_run_succeeds(
            discovered,
            failed,
            ignored_count,
            deny_ignored,
            allow_empty,
        ),
        discovered == 0 && !allow_empty ==> !recursive_run_succeeds(
            discovered,
            failed,
            ignored_count,
            deny_ignored,
            allow_empty,
        ),
{
}

/// When no guide failed, the recursive decision is the single-guide
/// ignored-guide policy applied to the ignored count; a failure is never
/// accepted, and it is reported together with a `--deny-ignored` rejection
/// exactly when that policy rejects.
pub(crate) proof fn theorem_recursive_completion_agrees_with_ignored_policy(
    all_passed: bool,
    ignored_count: usize,
    deny_ignored: bool,
)
    ensures
        all_passed ==> match ignored_policy(ignored_count, deny_ignored) {
            IgnoredPolicyDecision::Accept(o) => recursive_completion_spec(
                all_passed,
                ignored_count,
                deny_ignored,
            ) == RecursiveCompletion::Accept(o),
            IgnoredPolicyDecision::Deny => recursive_completion_spec(
                all_passed,
                ignored_count,
                deny_ignored,
            ) == RecursiveCompletion::Denied,
        },
        !all_passed ==> !is_accepted(recursive_completion_spec(all_passed, ignored_count, deny_ignored)),
        !all_passed ==> (recursive_completion_spec(all_passed, ignored_count, deny_ignored)
            == RecursiveCompletion::FailedAndDenied) == (ignored_policy(ignored_count, deny_ignored)
            == IgnoredPolicyDecision::Deny),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_policy_covers_every_branch() {
        assert_eq!(
            decide_ignored_policy(0, false),
            IgnoredPolicyDecision::Accept(CommandOutcome::Completed)
        );
        assert_eq!(
            decide_ignored_policy(0, true),
            IgnoredPolicyDecision::Accept(CommandOutcome::Completed)
        );
        assert_eq!(
            decide_ignored_policy(1, false),
            IgnoredPolicyDecision::Accept(CommandOutcome::Ignored { count: 1 })
        );
        assert_eq!(
            decide_ignored_policy(7, false),
            IgnoredPolicyDecision::Accept(CommandOutcome::Ignored { count: 7 })
        );
        assert_eq!(decide_ignored_policy(1, true), IgnoredPolicyDecision::Deny);
        assert_eq!(
            decide_ignored_policy(usize::MAX, true),
            IgnoredPolicyDecision::Deny
        );
    }

    #[test]
    fn empty_discovery_succeeds_only_under_allow_empty() {
        assert_eq!(
            empty_discovery_decision(true),
            EmptyDiscoveryDecision::Accept
        );
        assert_eq!(
            empty_discovery_decision(false),
            EmptyDiscoveryDecision::Reject
        );
    }

    /// The recursive decision exactly as `verify --recursive` stated it
    /// before it moved into this kernel.
    fn reference_completion(
        all_passed: bool,
        ignored: usize,
        deny_ignored: bool,
    ) -> RecursiveCompletion {
        if deny_ignored && ignored != 0 {
            if !all_passed {
                return RecursiveCompletion::FailedAndDenied;
            }
            return RecursiveCompletion::Denied;
        }
        if !all_passed {
            RecursiveCompletion::Failed
        } else {
            match decide_ignored_policy(ignored, deny_ignored) {
                IgnoredPolicyDecision::Accept(accepted) => RecursiveCompletion::Accept(accepted),
                IgnoredPolicyDecision::Deny => unreachable!("denied runs return above"),
            }
        }
    }

    #[test]
    fn recursive_completion_covers_every_branch() {
        for all_passed in [false, true] {
            for ignored in [0, 1, 2, usize::MAX] {
                for deny_ignored in [false, true] {
                    assert_eq!(
                        recursive_completion(all_passed, ignored, deny_ignored),
                        reference_completion(all_passed, ignored, deny_ignored),
                        "all_passed={all_passed} ignored={ignored} deny={deny_ignored}"
                    );
                }
            }
        }
        assert_eq!(
            recursive_completion(true, 0, true),
            RecursiveCompletion::Accept(CommandOutcome::Completed)
        );
        assert_eq!(
            recursive_completion(true, 3, false),
            RecursiveCompletion::Accept(CommandOutcome::Ignored { count: 3 })
        );
        assert_eq!(
            recursive_completion(false, 0, true),
            RecursiveCompletion::Failed
        );
        assert_eq!(
            recursive_completion(true, 1, true),
            RecursiveCompletion::Denied
        );
        assert_eq!(
            recursive_completion(false, 1, true),
            RecursiveCompletion::FailedAndDenied
        );
    }

    #[test]
    fn exit_status_matches_the_contract_table() {
        for mode in [
            StatusMode::Default,
            StatusMode::PostToolUse,
            StatusMode::PreCommitHook,
            StatusMode::GitHubActions,
        ] {
            assert_eq!(exit_status(mode, false), 0, "{mode:?}");
        }
        assert_eq!(exit_status(StatusMode::Default, true), 1);
        assert_eq!(exit_status(StatusMode::PostToolUse, true), 2);
        assert_eq!(exit_status(StatusMode::PreCommitHook, true), 1);
        assert_eq!(exit_status(StatusMode::GitHubActions, true), 1);
    }
}
