//! CLI module for the agentic navigation guide

pub(crate) mod check;
pub(crate) mod dump;
mod environment;
mod generation_options;
pub(crate) mod init;
pub(crate) mod output;
pub(crate) mod verify;

use crate::errors::{AppError, Result};
use crate::proven::outcome::{self, IgnoredPolicyDecision, StatusMode};
use crate::proven::settings::{self, CliSelections, CommandKind, LogChoice, ModeChoice};
use crate::types::{Config, ExecutionMode, LogLevel};
use clap::{error::ErrorKind, CommandFactory, Parser, Subcommand};

pub(crate) use crate::proven::outcome::CommandOutcome;

const ENVIRONMENT_HELP: &str = "\
Environment defaults (precedence: CLI > environment > built-in):
  AGENTIC_NAVIGATION_GUIDE_PATH
      Explicit guide path for check and non-recursive verify; no path default.
  AGENTIC_NAVIGATION_GUIDE_ROOT
      Root for dump, init, and verify; built-in default: current directory.
  AGENTIC_NAVIGATION_GUIDE_NAME
      Implicit guide filename for check and verify; built-in default:
      AGENTIC_NAVIGATION_GUIDE.md.
  AGENTIC_NAVIGATION_GUIDE_LOG_MODE
      Global quiet|default|verbose mode; built-in default: default.
  AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE
      Global default|post-tool-use|pre-commit-hook|github-actions mode;
      built-in default: default.

Only a relevant, unshadowed environment value is parsed and applied. Empty
path/root defaults and invalid name/mode defaults fail without printing their
contents.";

pub(crate) fn denied_ignored_message(ignored_count: usize) -> String {
    let noun = if ignored_count == 1 {
        "navigation guide was"
    } else {
        "navigation guides were"
    };
    format!("--deny-ignored rejected the run because {ignored_count} ignored {noun} discovered")
}

/// Apply the ignored-guide policy (verified in `proven::outcome`) to a run
/// that finished its work, reporting a `--deny-ignored` rejection.
pub(crate) fn finish_ignored_policy(
    ignored_count: usize,
    deny_ignored: bool,
) -> Result<CommandOutcome> {
    match outcome::decide_ignored_policy(ignored_count, deny_ignored) {
        IgnoredPolicyDecision::Accept(accepted) => Ok(accepted),
        IgnoredPolicyDecision::Deny => {
            let message = denied_ignored_message(ignored_count);
            output::stderr_line(&message)?;
            Err(AppError::Other(message).reported())
        }
    }
}

/// CLI arguments structure
#[derive(Parser, Debug)]
#[command(
    name = "agentic-navigation-guide",
    about = "A tool for verifying hand-written navigation guides against filesystem structure",
    version,
    author,
    after_help = ENVIRONMENT_HELP
)]
pub(crate) struct Cli {
    /// Include verbose informational output; diagnostics remain source-free
    #[arg(short, long, global = true, conflicts_with_all = ["quiet", "log_level"])]
    pub(crate) verbose: bool,

    /// Suppress ordinary chatter; primary dump output and errors remain visible
    #[arg(short, long, global = true, conflicts_with_all = ["verbose", "log_level"])]
    pub(crate) quiet: bool,

    /// Set log level directly
    #[arg(
        long,
        global = true,
        value_parser = ["quiet", "default", "verbose"],
        hide = true,
        conflicts_with_all = ["verbose", "quiet"]
    )]
    pub(crate) log_level: Option<String>,

    /// Set execution mode directly
    #[arg(
        long,
        global = true,
        value_parser = ["default", "post-tool-use", "pre-commit-hook", "github-actions"],
        hide = true
    )]
    pub(crate) execution_mode: Option<String>,

    /// Subcommand to execute
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// Available subcommands
#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Dump the current directory contents in navigation guide format
    Dump(dump::DumpArgs),

    /// Initialize a new navigation guide file
    Init(init::InitArgs),

    /// Check navigation guide syntax
    Check(check::CheckArgs),

    /// Verify navigation guide against filesystem
    Verify(verify::VerifyArgs),
}

impl Cli {
    /// Apply relevant environment defaults after Clap validates explicit CLI intent.
    pub(crate) fn apply_environment_defaults(&mut self) -> std::result::Result<(), clap::Error> {
        let defaults = environment::EnvironmentDefaults::capture();
        self.resolve_environment(&defaults)
            .map_err(|error| Self::command().error(ErrorKind::InvalidValue, error.to_string()))
    }

    /// Apply the environment defaults that the verified
    /// `proven::settings::environment_plan` selects: exactly the relevant
    /// variables that no higher-precedence selection shadows, parsed in the
    /// fixed order log mode, execution mode, root, path, name, so the first
    /// invalid selected value is reported.
    fn resolve_environment(
        &mut self,
        defaults: &environment::EnvironmentDefaults,
    ) -> std::result::Result<(), environment::EnvironmentError> {
        let plan = settings::environment_plan(self.cli_selections(), defaults.has_guide_path());

        if plan.log_mode {
            self.log_level = defaults.log_mode()?;
        }

        if plan.execution_mode {
            self.execution_mode = defaults.execution_mode()?;
        }

        match &mut self.command {
            Command::Dump(dump::DumpArgs { root, .. })
            | Command::Init(init::InitArgs { root, .. }) => {
                if plan.guide_root {
                    *root = defaults.guide_root()?;
                }
            }
            Command::Check(args) => {
                if plan.guide_path {
                    args.guide = defaults.guide_path()?;
                }
                if plan.guide_name {
                    args.implicit_guide_name = Some(defaults.guide_name()?);
                }
            }
            Command::Verify(args) => {
                if plan.guide_root {
                    args.root = defaults.guide_root()?;
                }
                if plan.guide_path {
                    args.guide = defaults.guide_path()?;
                }
                if plan.guide_name {
                    args.implicit_guide_name = Some(defaults.guide_name()?);
                }
            }
        }

        Ok(())
    }

    /// The higher-precedence CLI selections clap parsed, as the verified
    /// precedence kernel sees them.
    fn cli_selections(&self) -> CliSelections {
        let (command, root_option, guide_option, guide_name_option) = match &self.command {
            Command::Dump(args) => (CommandKind::Dump, args.root.is_some(), false, false),
            Command::Init(args) => (CommandKind::Init, args.root.is_some(), false, false),
            Command::Check(args) => (CommandKind::Check, false, args.guide.is_some(), false),
            Command::Verify(args) => (
                if args.recursive {
                    CommandKind::RecursiveVerify
                } else {
                    CommandKind::Verify
                },
                args.root.is_some(),
                args.guide.is_some(),
                args.guide_name.is_some(),
            ),
        };
        CliSelections {
            command,
            log_option: self.verbose || self.quiet || self.log_level.is_some(),
            mode_option: self.execution_mode.is_some() || self.command.has_execution_mode_flag(),
            root_option,
            guide_option,
            guide_name_option,
        }
    }

    /// Resolve already-parsed CLI arguments into a Config. The verified
    /// `proven::settings` kernels accept exactly the listed spellings and
    /// rank `--verbose`, then `--quiet`, then the selected value, then the
    /// built-in default.
    pub(crate) fn build_config(&self) -> Config {
        let selected_level = self
            .log_level
            .as_deref()
            .and_then(|level| settings::log_choice(level.as_bytes()));
        let log_level = match settings::resolve_log_level(self.verbose, self.quiet, selected_level)
        {
            LogChoice::Quiet => LogLevel::Quiet,
            LogChoice::Default => LogLevel::Default,
            LogChoice::Verbose => LogLevel::Verbose,
        };

        let selected_mode = self
            .execution_mode
            .as_deref()
            .and_then(|mode| settings::mode_choice(mode.as_bytes()));
        let execution_mode = match settings::resolve_execution_mode(selected_mode) {
            ModeChoice::Default => ExecutionMode::Default,
            ModeChoice::PostToolUse => ExecutionMode::PostToolUse,
            ModeChoice::PreCommitHook => ExecutionMode::PreCommitHook,
            ModeChoice::GitHubActions => ExecutionMode::GitHubActions,
        };

        Config {
            execution_mode,
            log_level,
            root_path: None,
            guide_path: None,
            original_guide_path: None,
            original_root_path: None,
        }
    }
}

impl Command {
    fn has_execution_mode_flag(&self) -> bool {
        match self {
            Self::Check(args) => {
                args.post_tool_use_hook || args.pre_commit_hook || args.github_actions_check
            }
            Self::Verify(args) => {
                args.post_tool_use_hook || args.pre_commit_hook || args.github_actions_check
            }
            Self::Dump(_) | Self::Init(_) => false,
        }
    }
}

/// Initialize logging based on config
pub(crate) fn init_logging(config: &Config) {
    use env_logger::{Builder, Target};
    use log::LevelFilter;

    let level = match config.log_level {
        LogLevel::Quiet => LevelFilter::Error,
        LogLevel::Default => LevelFilter::Info,
        LogLevel::Verbose => LevelFilter::Debug,
    };

    Builder::new()
        .target(Target::Stderr)
        .filter_level(level)
        .init();
}

/// Get the appropriate exit code based on execution mode.
///
/// `main` routes every outcome, success included, through this function, and
/// the status is decided by the verified `proven::outcome::exit_status` from
/// the execution mode alone; the log level is not an input.
pub(crate) fn get_exit_code(config: &Config, is_error: bool) -> i32 {
    outcome::exit_status(status_mode(config.execution_mode), is_error)
}

/// The exit-status policy's view of an execution mode.
fn status_mode(mode: ExecutionMode) -> StatusMode {
    match mode {
        ExecutionMode::Default => StatusMode::Default,
        ExecutionMode::PostToolUse => StatusMode::PostToolUse,
        ExecutionMode::PreCommitHook => StatusMode::PreCommitHook,
        ExecutionMode::GitHubActions => StatusMode::GitHubActions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(execution_mode: ExecutionMode, log_level: LogLevel) -> Config {
        Config {
            execution_mode,
            log_level,
            root_path: None,
            guide_path: None,
            original_guide_path: None,
            original_root_path: None,
        }
    }

    #[test]
    fn exit_codes_depend_only_on_execution_mode() {
        let modes = [
            (ExecutionMode::Default, 1),
            (ExecutionMode::PostToolUse, 2),
            (ExecutionMode::PreCommitHook, 1),
            (ExecutionMode::GitHubActions, 1),
        ];
        for (mode, failure) in modes {
            for level in [LogLevel::Quiet, LogLevel::Default, LogLevel::Verbose] {
                let config = config(mode, level);
                assert_eq!(get_exit_code(&config, false), 0, "{mode:?} {level:?}");
                assert_eq!(get_exit_code(&config, true), failure, "{mode:?} {level:?}");
            }
        }
    }

    #[test]
    fn ignored_policy_completes_allows_or_rejects() {
        assert_eq!(
            finish_ignored_policy(0, false).expect("no ignored guide"),
            CommandOutcome::Completed
        );
        assert_eq!(
            finish_ignored_policy(0, true).expect("no ignored guide"),
            CommandOutcome::Completed
        );
        assert_eq!(
            finish_ignored_policy(2, false).expect("allowed ignored guides"),
            CommandOutcome::Ignored { count: 2 }
        );
        let error = finish_ignored_policy(1, true).expect_err("denied ignored guide");
        assert!(error.is_reported());
        assert_eq!(
            error.root_cause().to_string(),
            AppError::Other(denied_ignored_message(1)).to_string()
        );
    }
}
