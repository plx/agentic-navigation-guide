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
//!   failure in each execution mode.
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
//!   with an ignored guide exits with a nonzero status in every mode.

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
