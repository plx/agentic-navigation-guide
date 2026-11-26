//! Verified configuration precedence: which environment defaults are
//! parsed, which values they accept, and how the log level and execution
//! mode resolve.
//!
//! These kernels make the pure decisions of `cli/mod.rs` and
//! `cli/environment.rs` for the `docs/v0.2-contract.md` rules in
//! "Command-line and environment configuration": every setting resolves in
//! the order CLI > environment > built-in default, and an implementation
//! decides relevance and higher-precedence selection before parsing a
//! lower-precedence environment value. Glue reads every variable with
//! `std::env::var_os` (trusted), records which CLI options clap parsed, and
//! performs exactly the parses the plan names, in the fixed order log mode,
//! execution mode, root, path, name, so the first invalid selected value is
//! the one reported:
//!
//! - [`environment_plan`]: an environment variable is parsed exactly when it
//!   is relevant to the command and no higher-precedence selection shadows
//!   it. The CLI option of the same setting shadows it; for the implicit
//!   guide name of `check` and non-recursive `verify`, an explicit guide
//!   path from the CLI or from `AGENTIC_NAVIGATION_GUIDE_PATH` also shadows
//!   it;
//! - [`path_default`]: a selected path or root value is rejected exactly when
//!   it is empty;
//! - [`log_choice`] and [`mode_choice`]: a log or execution-mode value is
//!   accepted exactly when its bytes are one of the listed case-sensitive
//!   spellings;
//! - [`resolve_log_level`] and [`resolve_execution_mode`]: `--verbose` and
//!   `--quiet` outrank the selected log value, and an absent value falls back
//!   to the built-in `default`.
//!
//! The theorems below state the contract-level consequences:
//!
//! - `theorem_consulted_iff_relevant_and_unshadowed`: for every setting, the
//!   plan parses its variable exactly when the setting is in the command's
//!   scope (the contract's scope table) and neither a CLI selection nor, for
//!   the implicit name, an explicit environment path selects it first;
//! - `theorem_cli_shadows_environment` and
//!   `theorem_explicit_path_shadows_implicit_name`: a CLI selection always
//!   prevents parsing the variable, and an effective explicit guide path
//!   (CLI or environment) always prevents resolving the implicit name;
//! - `theorem_scope_table`: `dump` and `init` never parse the path or name,
//!   `check` never parses the root, and recursive `verify` never parses the
//!   path;
//! - `theorem_choices_are_exact`: a log or execution-mode value is accepted
//!   exactly when it is one of the listed spellings, and each spelling
//!   selects its own choice;
//! - `theorem_selected_path_is_nonempty`: a selected path value is never
//!   empty, and an absent one selects nothing;
//! - `theorem_log_precedence` and `theorem_execution_mode_precedence`:
//!   composing the plan with the resolution, the effective log level and
//!   execution mode are the CLI selection when there is one, else the valid
//!   environment value when there is one, else the built-in `default`.

// Verus reasons about array literals element by element but treats
// byte-string literals as opaque, so the spellings below stay spelled out.
#![allow(clippy::byte_char_slices)]

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// The command as environment relevance sees it.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandKind {
    Dump,
    Init,
    Check,
    /// `verify` without `--recursive`.
    Verify,
    /// `verify --recursive`.
    RecursiveVerify,
}

/// Which higher-precedence CLI selections clap parsed.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CliSelections {
    pub(crate) command: CommandKind,
    /// `--verbose`, `--quiet`, or `--log-level`.
    pub(crate) log_option: bool,
    /// `--execution-mode` or an execution convenience flag.
    pub(crate) mode_option: bool,
    /// `--root`.
    pub(crate) root_option: bool,
    /// `--guide`.
    pub(crate) guide_option: bool,
    /// `--guide-name`.
    pub(crate) guide_name_option: bool,
}

/// Which environment defaults to parse, in the fixed order of the fields.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EnvironmentPlan {
    /// `AGENTIC_NAVIGATION_GUIDE_LOG_MODE`.
    pub(crate) log_mode: bool,
    /// `AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE`.
    pub(crate) execution_mode: bool,
    /// `AGENTIC_NAVIGATION_GUIDE_ROOT`.
    pub(crate) guide_root: bool,
    /// `AGENTIC_NAVIGATION_GUIDE_PATH`.
    pub(crate) guide_path: bool,
    /// Resolve the implicit name from `AGENTIC_NAVIGATION_GUIDE_NAME`, or
    /// from the built-in name when the variable is absent.
    pub(crate) guide_name: bool,
}

/// The decision on a path or root environment value.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathDefault {
    /// The variable is absent: no default.
    Unset,
    /// The variable is empty: reject it.
    Empty,
    /// The variable selects its nonempty value.
    Selected,
}

/// A log mode spelling: `quiet`, `default`, or `verbose`.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogChoice {
    Quiet,
    Default,
    Verbose,
}

/// An execution mode spelling: `default`, `post-tool-use`,
/// `pre-commit-hook`, or `github-actions`.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModeChoice {
    Default,
    PostToolUse,
    PreCommitHook,
    GitHubActions,
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Specifications
// ---------------------------------------------------------------------------

/// The configuration settings that have an environment default.
pub(crate) enum Setting {
    LogMode,
    ExecutionMode,
    GuideRoot,
    GuidePath,
    GuideName,
}

/// The contract's scope table: log and execution mode are global; the root
/// applies to `dump`, `init`, and every `verify`; the explicit path to
/// `check` and non-recursive `verify`; the implicit name to `check` and
/// every `verify`.
pub(crate) open spec fn relevant(command: CommandKind, setting: Setting) -> bool {
    match setting {
        Setting::LogMode => true,
        Setting::ExecutionMode => true,
        Setting::GuideRoot => command != CommandKind::Check,
        Setting::GuidePath => command == CommandKind::Check || command == CommandKind::Verify,
        Setting::GuideName => command == CommandKind::Check || command == CommandKind::Verify
            || command == CommandKind::RecursiveVerify,
    }
}

/// The higher-precedence CLI selection of each setting: the implicit name is
/// selected by `--guide-name` in recursive `verify` and replaced by an
/// explicit `--guide` otherwise.
pub(crate) open spec fn cli_selects(cli: CliSelections, setting: Setting) -> bool {
    match setting {
        Setting::LogMode => cli.log_option,
        Setting::ExecutionMode => cli.mode_option,
        Setting::GuideRoot => cli.root_option,
        Setting::GuidePath => cli.guide_option,
        Setting::GuideName => if cli.command == CommandKind::RecursiveVerify {
            cli.guide_name_option
        } else {
            cli.guide_option
        },
    }
}

/// A higher-precedence environment selection: for `check` and non-recursive
/// `verify`, an effective explicit path from the environment wins over the
/// name selector.
pub(crate) open spec fn environment_selects(
    cli: CliSelections,
    path_variable_set: bool,
    setting: Setting,
) -> bool {
    match setting {
        Setting::GuideName => cli.command != CommandKind::RecursiveVerify && path_variable_set,
        _ => false,
    }
}

/// The variable of `setting` is parsed exactly when the setting is relevant
/// and nothing of higher precedence selects it.
pub(crate) open spec fn consulted(cli: CliSelections, path_variable_set: bool, setting: Setting) -> bool {
    relevant(cli.command, setting) && !cli_selects(cli, setting) && !environment_selects(
        cli,
        path_variable_set,
        setting,
    )
}

/// The plan's entry for `setting`.
pub(crate) open spec fn plan_entry(plan: EnvironmentPlan, setting: Setting) -> bool {
    match setting {
        Setting::LogMode => plan.log_mode,
        Setting::ExecutionMode => plan.execution_mode,
        Setting::GuideRoot => plan.guide_root,
        Setting::GuidePath => plan.guide_path,
        Setting::GuideName => plan.guide_name,
    }
}

/// The plan parses exactly the consulted settings.
pub(crate) open spec fn is_environment_plan(
    cli: CliSelections,
    path_variable_set: bool,
    plan: EnvironmentPlan,
) -> bool {
    forall|setting: Setting|
        (#[trigger] plan_entry(plan, setting)) == consulted(cli, path_variable_set, setting)
}

/// An absent value selects nothing, an empty one is rejected, and any other
/// one is selected.
pub(crate) open spec fn path_default_spec(empty: Option<bool>) -> PathDefault {
    match empty {
        None => PathDefault::Unset,
        Some(true) => PathDefault::Empty,
        Some(false) => PathDefault::Selected,
    }
}

/// `quiet`, `default`, `verbose`.
pub(crate) open spec fn quiet_bytes() -> Seq<u8> {
    seq![b'q', b'u', b'i', b'e', b't']
}

pub(crate) open spec fn default_bytes() -> Seq<u8> {
    seq![b'd', b'e', b'f', b'a', b'u', b'l', b't']
}

pub(crate) open spec fn verbose_bytes() -> Seq<u8> {
    seq![b'v', b'e', b'r', b'b', b'o', b's', b'e']
}

/// `post-tool-use`, `pre-commit-hook`, `github-actions`.
pub(crate) open spec fn post_tool_use_bytes() -> Seq<u8> {
    seq![b'p', b'o', b's', b't', b'-', b't', b'o', b'o', b'l', b'-', b'u', b's', b'e']
}

pub(crate) open spec fn pre_commit_hook_bytes() -> Seq<u8> {
    seq![b'p', b'r', b'e', b'-', b'c', b'o', b'm', b'm', b'i', b't', b'-', b'h', b'o', b'o', b'k']
}

pub(crate) open spec fn github_actions_bytes() -> Seq<u8> {
    seq![b'g', b'i', b't', b'h', b'u', b'b', b'-', b'a', b'c', b't', b'i', b'o', b'n', b's']
}

/// The log choice of a spelling, if it is one of the listed spellings.
pub(crate) open spec fn log_choice_spec(value: Seq<u8>) -> Option<LogChoice> {
    if value == quiet_bytes() {
        Some(LogChoice::Quiet)
    } else if value == default_bytes() {
        Some(LogChoice::Default)
    } else if value == verbose_bytes() {
        Some(LogChoice::Verbose)
    } else {
        None
    }
}

/// The execution-mode choice of a spelling, if it is one of the listed
/// spellings.
pub(crate) open spec fn mode_choice_spec(value: Seq<u8>) -> Option<ModeChoice> {
    if value == default_bytes() {
        Some(ModeChoice::Default)
    } else if value == post_tool_use_bytes() {
        Some(ModeChoice::PostToolUse)
    } else if value == pre_commit_hook_bytes() {
        Some(ModeChoice::PreCommitHook)
    } else if value == github_actions_bytes() {
        Some(ModeChoice::GitHubActions)
    } else {
        None
    }
}

/// `--verbose`, then `--quiet`, then the selected value, then `default`.
pub(crate) open spec fn resolve_log_level_spec(verbose: bool, quiet: bool, level: Option<LogChoice>) -> LogChoice {
    if verbose {
        LogChoice::Verbose
    } else if quiet {
        LogChoice::Quiet
    } else {
        match level {
            Some(choice) => choice,
            None => LogChoice::Default,
        }
    }
}

/// The selected value, then `default`.
pub(crate) open spec fn resolve_execution_mode_spec(mode: Option<ModeChoice>) -> ModeChoice {
    match mode {
        Some(choice) => choice,
        None => ModeChoice::Default,
    }
}

} // verus!

// ---------------------------------------------------------------------------
// Executable kernels
// ---------------------------------------------------------------------------

/// Decide which environment defaults to parse.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        is_environment_plan(cli, path_variable_set, r),
))]
pub(crate) fn environment_plan(cli: CliSelections, path_variable_set: bool) -> EnvironmentPlan {
    let guide_root = !matches!(cli.command, CommandKind::Check) && !cli.root_option;
    let guide_path =
        matches!(cli.command, CommandKind::Check | CommandKind::Verify) && !cli.guide_option;
    let guide_name = match cli.command {
        CommandKind::Dump | CommandKind::Init => false,
        CommandKind::Check | CommandKind::Verify => !cli.guide_option && !path_variable_set,
        CommandKind::RecursiveVerify => !cli.guide_name_option,
    };
    let plan = EnvironmentPlan {
        log_mode: !cli.log_option,
        execution_mode: !cli.mode_option,
        guide_root,
        guide_path,
        guide_name,
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        assert forall|setting: Setting|
            (#[trigger] plan_entry(plan, setting)) == consulted(cli, path_variable_set, setting) by {
            match setting {
                Setting::GuideName => {},
                _ => {},
            }
        }
    }
    plan
}

/// Decide a path or root environment value from whether it is present and,
/// if so, whether it is empty.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == path_default_spec(empty),
))]
pub(crate) fn path_default(empty: Option<bool>) -> PathDefault {
    match empty {
        None => PathDefault::Unset,
        Some(true) => PathDefault::Empty,
        Some(false) => PathDefault::Selected,
    }
}

/// Return whether `value` is exactly `expected`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == (value@ == expected@),
))]
fn bytes_equal(value: &[u8], expected: &[u8]) -> bool {
    if value.len() != expected.len() {
        return false;
    }
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= value.len(),
            value.len() == expected.len(),
            forall|k: int| 0 <= k < index ==> value@[k] == expected@[k],
        decreases value.len() - index
    ))]
    while index < value.len() {
        if value[index] != expected[index] {
            return false;
        }
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(value@ =~= expected@);
    }
    true
}

/// The log choice that `value` spells exactly, if any.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == log_choice_spec(value@),
))]
pub(crate) fn log_choice(value: &[u8]) -> Option<LogChoice> {
    let quiet: [u8; 5] = [b'q', b'u', b'i', b'e', b't'];
    let default: [u8; 7] = [b'd', b'e', b'f', b'a', b'u', b'l', b't'];
    let verbose: [u8; 7] = [b'v', b'e', b'r', b'b', b'o', b's', b'e'];
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(quiet@ =~= quiet_bytes());
        assert(default@ =~= default_bytes());
        assert(verbose@ =~= verbose_bytes());
    }
    if bytes_equal(value, quiet.as_slice()) {
        Some(LogChoice::Quiet)
    } else if bytes_equal(value, default.as_slice()) {
        Some(LogChoice::Default)
    } else if bytes_equal(value, verbose.as_slice()) {
        Some(LogChoice::Verbose)
    } else {
        None
    }
}

/// The execution-mode choice that `value` spells exactly, if any.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == mode_choice_spec(value@),
))]
pub(crate) fn mode_choice(value: &[u8]) -> Option<ModeChoice> {
    let default: [u8; 7] = [b'd', b'e', b'f', b'a', b'u', b'l', b't'];
    let post_tool_use: [u8; 13] = [
        b'p', b'o', b's', b't', b'-', b't', b'o', b'o', b'l', b'-', b'u', b's', b'e',
    ];
    let pre_commit_hook: [u8; 15] = [
        b'p', b'r', b'e', b'-', b'c', b'o', b'm', b'm', b'i', b't', b'-', b'h', b'o', b'o', b'k',
    ];
    let github_actions: [u8; 14] = [
        b'g', b'i', b't', b'h', b'u', b'b', b'-', b'a', b'c', b't', b'i', b'o', b'n', b's',
    ];
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(default@ =~= default_bytes());
        assert(post_tool_use@ =~= post_tool_use_bytes());
        assert(pre_commit_hook@ =~= pre_commit_hook_bytes());
        assert(github_actions@ =~= github_actions_bytes());
    }
    if bytes_equal(value, default.as_slice()) {
        Some(ModeChoice::Default)
    } else if bytes_equal(value, post_tool_use.as_slice()) {
        Some(ModeChoice::PostToolUse)
    } else if bytes_equal(value, pre_commit_hook.as_slice()) {
        Some(ModeChoice::PreCommitHook)
    } else if bytes_equal(value, github_actions.as_slice()) {
        Some(ModeChoice::GitHubActions)
    } else {
        None
    }
}

/// Resolve the effective log level.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == resolve_log_level_spec(verbose, quiet, level),
))]
pub(crate) fn resolve_log_level(verbose: bool, quiet: bool, level: Option<LogChoice>) -> LogChoice {
    if verbose {
        LogChoice::Verbose
    } else if quiet {
        LogChoice::Quiet
    } else {
        match level {
            Some(choice) => choice,
            None => LogChoice::Default,
        }
    }
}

/// Resolve the effective execution mode.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == resolve_execution_mode_spec(mode),
))]
pub(crate) fn resolve_execution_mode(mode: Option<ModeChoice>) -> ModeChoice {
    match mode {
        Some(choice) => choice,
        None => ModeChoice::Default,
    }
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Contract theorems
// ---------------------------------------------------------------------------

/// For every setting, the plan parses its variable exactly when the setting
/// is in the command's scope and neither a CLI selection nor, for the
/// implicit name, an explicit environment path selects it first.
pub(crate) proof fn theorem_consulted_iff_relevant_and_unshadowed(
    cli: CliSelections,
    path_variable_set: bool,
    plan: EnvironmentPlan,
    setting: Setting,
)
    requires
        is_environment_plan(cli, path_variable_set, plan),
    ensures
        plan_entry(plan, setting) == (relevant(cli.command, setting) && !cli_selects(cli, setting)
            && !environment_selects(cli, path_variable_set, setting)),
{
    assert(plan_entry(plan, setting) == consulted(cli, path_variable_set, setting));
}

/// A CLI selection always prevents parsing the variable of its setting.
pub(crate) proof fn theorem_cli_shadows_environment(
    cli: CliSelections,
    path_variable_set: bool,
    plan: EnvironmentPlan,
)
    requires
        is_environment_plan(cli, path_variable_set, plan),
    ensures
        cli.log_option ==> !plan.log_mode,
        cli.mode_option ==> !plan.execution_mode,
        cli.root_option ==> !plan.guide_root,
        cli.guide_option ==> !plan.guide_path,
        cli.command == CommandKind::RecursiveVerify && cli.guide_name_option ==> !plan.guide_name,
{
    assert(plan_entry(plan, Setting::LogMode) == consulted(cli, path_variable_set, Setting::LogMode));
    assert(plan_entry(plan, Setting::ExecutionMode) == consulted(
        cli,
        path_variable_set,
        Setting::ExecutionMode,
    ));
    assert(plan_entry(plan, Setting::GuideRoot) == consulted(cli, path_variable_set, Setting::GuideRoot));
    assert(plan_entry(plan, Setting::GuidePath) == consulted(cli, path_variable_set, Setting::GuidePath));
    assert(plan_entry(plan, Setting::GuideName) == consulted(cli, path_variable_set, Setting::GuideName));
}

/// For `check` and non-recursive `verify`, an effective explicit guide path
/// from the CLI or from `AGENTIC_NAVIGATION_GUIDE_PATH` always prevents
/// resolving the implicit name, and the implicit name is resolved exactly
/// when neither is present.
pub(crate) proof fn theorem_explicit_path_shadows_implicit_name(
    cli: CliSelections,
    path_variable_set: bool,
    plan: EnvironmentPlan,
)
    requires
        is_environment_plan(cli, path_variable_set, plan),
        cli.command == CommandKind::Check || cli.command == CommandKind::Verify,
    ensures
        plan.guide_name == (!cli.guide_option && !path_variable_set),
        plan.guide_path == !cli.guide_option,
{
    assert(plan_entry(plan, Setting::GuidePath) == consulted(cli, path_variable_set, Setting::GuidePath));
    assert(plan_entry(plan, Setting::GuideName) == consulted(cli, path_variable_set, Setting::GuideName));
}

/// The scope table: `dump` and `init` never parse the explicit path or the
/// implicit name, `check` never parses the root, recursive `verify` never
/// parses the explicit path and resolves the name exactly without
/// `--guide-name`, and the global modes are parsed exactly without their
/// CLI selection.
pub(crate) proof fn theorem_scope_table(cli: CliSelections, path_variable_set: bool, plan: EnvironmentPlan)
    requires
        is_environment_plan(cli, path_variable_set, plan),
    ensures
        cli.command == CommandKind::Dump || cli.command == CommandKind::Init ==> !plan.guide_path
            && !plan.guide_name && plan.guide_root == !cli.root_option,
        cli.command == CommandKind::Check ==> !plan.guide_root,
        cli.command == CommandKind::RecursiveVerify ==> !plan.guide_path && plan.guide_name
            == !cli.guide_name_option && plan.guide_root == !cli.root_option,
        cli.command == CommandKind::Verify ==> plan.guide_root == !cli.root_option,
        plan.log_mode == !cli.log_option,
        plan.execution_mode == !cli.mode_option,
{
    assert(plan_entry(plan, Setting::LogMode) == consulted(cli, path_variable_set, Setting::LogMode));
    assert(plan_entry(plan, Setting::ExecutionMode) == consulted(
        cli,
        path_variable_set,
        Setting::ExecutionMode,
    ));
    assert(plan_entry(plan, Setting::GuideRoot) == consulted(cli, path_variable_set, Setting::GuideRoot));
    assert(plan_entry(plan, Setting::GuidePath) == consulted(cli, path_variable_set, Setting::GuidePath));
    assert(plan_entry(plan, Setting::GuideName) == consulted(cli, path_variable_set, Setting::GuideName));
}

/// A log or execution-mode value is accepted exactly when it is one of the
/// listed case-sensitive spellings, and each spelling selects its own
/// choice.
pub(crate) proof fn theorem_choices_are_exact(value: Seq<u8>)
    ensures
        (log_choice_spec(value) is Some) == (value == quiet_bytes() || value == default_bytes()
            || value == verbose_bytes()),
        (mode_choice_spec(value) is Some) == (value == default_bytes() || value
            == post_tool_use_bytes() || value == pre_commit_hook_bytes() || value
            == github_actions_bytes()),
        log_choice_spec(quiet_bytes()) == Some(LogChoice::Quiet),
        log_choice_spec(default_bytes()) == Some(LogChoice::Default),
        log_choice_spec(verbose_bytes()) == Some(LogChoice::Verbose),
        mode_choice_spec(default_bytes()) == Some(ModeChoice::Default),
        mode_choice_spec(post_tool_use_bytes()) == Some(ModeChoice::PostToolUse),
        mode_choice_spec(pre_commit_hook_bytes()) == Some(ModeChoice::PreCommitHook),
        mode_choice_spec(github_actions_bytes()) == Some(ModeChoice::GitHubActions),
{
    // The spellings are pairwise distinct (their lengths or first bytes
    // differ), so the first matching branch is the spelling's own.
    assert(quiet_bytes() != default_bytes()) by {
        assert(quiet_bytes().len() != default_bytes().len());
    }
    assert(quiet_bytes() != verbose_bytes()) by {
        assert(quiet_bytes().len() != verbose_bytes().len());
    }
    assert(default_bytes() != verbose_bytes()) by {
        assert(default_bytes()[0] != verbose_bytes()[0]);
    }
    assert(default_bytes() != post_tool_use_bytes()) by {
        assert(default_bytes().len() != post_tool_use_bytes().len());
    }
    assert(default_bytes() != pre_commit_hook_bytes()) by {
        assert(default_bytes().len() != pre_commit_hook_bytes().len());
    }
    assert(default_bytes() != github_actions_bytes()) by {
        assert(default_bytes().len() != github_actions_bytes().len());
    }
    assert(post_tool_use_bytes() != pre_commit_hook_bytes()) by {
        assert(post_tool_use_bytes().len() != pre_commit_hook_bytes().len());
    }
    assert(post_tool_use_bytes() != github_actions_bytes()) by {
        assert(post_tool_use_bytes().len() != github_actions_bytes().len());
    }
    assert(pre_commit_hook_bytes() != github_actions_bytes()) by {
        assert(pre_commit_hook_bytes().len() != github_actions_bytes().len());
    }
}

/// A selected path value is never empty, an absent one selects nothing, and
/// an empty one is rejected.
pub(crate) proof fn theorem_selected_path_is_nonempty(empty: Option<bool>)
    ensures
        path_default_spec(empty) == PathDefault::Selected ==> empty == Some(false),
        (path_default_spec(empty) == PathDefault::Unset) == (empty is None),
        (path_default_spec(empty) == PathDefault::Empty) == (empty == Some(true)),
{
}

/// Composing the plan with the resolution, the effective log level is the
/// CLI selection when there is one (`--verbose`, then `--quiet`, then
/// `--log-level`), else the valid environment value when there is one, else
/// the built-in `default`.
pub(crate) proof fn theorem_log_precedence(
    cli: CliSelections,
    path_variable_set: bool,
    plan: EnvironmentPlan,
    verbose: bool,
    quiet: bool,
    cli_level: Option<LogChoice>,
    environment_level: Option<LogChoice>,
)
    requires
        is_environment_plan(cli, path_variable_set, plan),
        cli.log_option == (verbose || quiet || cli_level is Some),
    ensures
        ({
            let level = if plan.log_mode {
                environment_level
            } else {
                cli_level
            };
            resolve_log_level_spec(verbose, quiet, level) == if verbose {
                LogChoice::Verbose
            } else if quiet {
                LogChoice::Quiet
            } else {
                match (cli_level, environment_level) {
                    (Some(choice), _) => choice,
                    (None, Some(choice)) => choice,
                    (None, None) => LogChoice::Default,
                }
            }
        }),
{
    assert(plan_entry(plan, Setting::LogMode) == consulted(cli, path_variable_set, Setting::LogMode));
}

/// Composing the plan with the resolution, the effective execution mode is
/// the CLI selection when there is one, else the valid environment value
/// when there is one, else the built-in `default`. (An execution
/// convenience flag is a CLI selection that the command applies after this
/// resolution.)
pub(crate) proof fn theorem_execution_mode_precedence(
    cli: CliSelections,
    path_variable_set: bool,
    plan: EnvironmentPlan,
    cli_mode: Option<ModeChoice>,
    environment_mode: Option<ModeChoice>,
)
    requires
        is_environment_plan(cli, path_variable_set, plan),
        cli_mode is Some ==> cli.mode_option,
    ensures
        ({
            let mode = if plan.execution_mode {
                environment_mode
            } else {
                cli_mode
            };
            &&& match cli_mode {
                Some(choice) => resolve_execution_mode_spec(mode) == choice,
                None => true,
            }
            &&& !cli.mode_option ==> resolve_execution_mode_spec(mode) == match environment_mode {
                Some(choice) => choice,
                None => ModeChoice::Default,
            }
        }),
{
    assert(plan_entry(plan, Setting::ExecutionMode) == consulted(
        cli,
        path_variable_set,
        Setting::ExecutionMode,
    ));
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    const COMMANDS: [CommandKind; 5] = [
        CommandKind::Dump,
        CommandKind::Init,
        CommandKind::Check,
        CommandKind::Verify,
        CommandKind::RecursiveVerify,
    ];

    /// The precedence decisions exactly as `Cli::resolve_environment` stated
    /// them before they moved into this kernel, with the explicit path
    /// variable either absent or present and nonempty.
    fn reference_plan(cli: CliSelections, path_variable_set: bool) -> EnvironmentPlan {
        let mut plan = EnvironmentPlan {
            log_mode: !cli.log_option,
            execution_mode: !cli.mode_option,
            guide_root: false,
            guide_path: false,
            guide_name: false,
        };
        match cli.command {
            CommandKind::Dump | CommandKind::Init => plan.guide_root = !cli.root_option,
            CommandKind::Check => {
                let mut guide = cli.guide_option;
                if !guide {
                    plan.guide_path = true;
                    guide = path_variable_set;
                }
                plan.guide_name = !guide;
            }
            CommandKind::Verify => {
                plan.guide_root = !cli.root_option;
                let mut guide = cli.guide_option;
                if !guide {
                    plan.guide_path = true;
                    guide = path_variable_set;
                }
                plan.guide_name = !guide;
            }
            CommandKind::RecursiveVerify => {
                plan.guide_root = !cli.root_option;
                plan.guide_name = !cli.guide_name_option;
            }
        }
        plan
    }

    #[test]
    fn plan_matches_the_previous_precedence_on_every_input() {
        for command in COMMANDS {
            for bits in 0u8..64 {
                let bit = |index: u8| bits & (1 << index) != 0;
                let cli = CliSelections {
                    command,
                    log_option: bit(0),
                    mode_option: bit(1),
                    root_option: bit(2),
                    guide_option: bit(3),
                    guide_name_option: bit(4),
                };
                let path_variable_set = bit(5);
                assert_eq!(
                    environment_plan(cli, path_variable_set),
                    reference_plan(cli, path_variable_set),
                    "{cli:?} {path_variable_set}"
                );
            }
        }
    }

    #[test]
    fn path_defaults_reject_only_empty_values() {
        assert_eq!(path_default(None), PathDefault::Unset);
        assert_eq!(path_default(Some(true)), PathDefault::Empty);
        assert_eq!(path_default(Some(false)), PathDefault::Selected);
    }

    #[test]
    fn choices_match_the_previous_lists() {
        let log = ["quiet", "default", "verbose"];
        let mode = [
            "default",
            "post-tool-use",
            "pre-commit-hook",
            "github-actions",
        ];
        let mut values: Vec<String> = log.iter().chain(&mode).map(|v| v.to_string()).collect();
        for value in log.iter().chain(&mode) {
            values.push(value.to_uppercase());
            values.push(format!(" {value}"));
            values.push(format!("{value} "));
            values.push(value[..value.len() - 1].to_string());
            values.push(format!("{value}x"));
        }
        values.extend(["", "unknown", "quiet\nforged"].map(str::to_string));
        for value in &values {
            assert_eq!(
                log_choice(value.as_bytes()).is_some(),
                log.contains(&value.as_str()),
                "{value:?}"
            );
            assert_eq!(
                mode_choice(value.as_bytes()).is_some(),
                mode.contains(&value.as_str()),
                "{value:?}"
            );
        }
        assert_eq!(log_choice(b"quiet"), Some(LogChoice::Quiet));
        assert_eq!(log_choice(b"default"), Some(LogChoice::Default));
        assert_eq!(log_choice(b"verbose"), Some(LogChoice::Verbose));
        assert_eq!(mode_choice(b"default"), Some(ModeChoice::Default));
        assert_eq!(mode_choice(b"post-tool-use"), Some(ModeChoice::PostToolUse));
        assert_eq!(
            mode_choice(b"pre-commit-hook"),
            Some(ModeChoice::PreCommitHook)
        );
        assert_eq!(
            mode_choice(b"github-actions"),
            Some(ModeChoice::GitHubActions)
        );
    }

    #[test]
    fn resolution_follows_cli_then_selected_value_then_default() {
        for level in [
            None,
            Some(LogChoice::Quiet),
            Some(LogChoice::Default),
            Some(LogChoice::Verbose),
        ] {
            assert_eq!(resolve_log_level(true, false, level), LogChoice::Verbose);
            assert_eq!(resolve_log_level(false, true, level), LogChoice::Quiet);
            assert_eq!(
                resolve_log_level(false, false, level),
                level.unwrap_or(LogChoice::Default)
            );
        }
        assert_eq!(resolve_execution_mode(None), ModeChoice::Default);
        assert_eq!(
            resolve_execution_mode(Some(ModeChoice::GitHubActions)),
            ModeChoice::GitHubActions
        );
    }
}
