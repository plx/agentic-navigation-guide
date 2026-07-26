//! CLI module for the agentic navigation guide

pub mod check;
pub mod dump;
mod environment;
mod generation_options;
pub mod init;
mod output;
pub mod verify;

use agentic_navigation_guide::errors::{AppError, Result};
use agentic_navigation_guide::types::{Config, ExecutionMode, LogLevel};
use clap::{error::ErrorKind, CommandFactory, Parser, Subcommand};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandOutcome {
    Completed,
    Ignored { count: usize },
}

pub(crate) fn denied_ignored_message(ignored_count: usize) -> String {
    let noun = if ignored_count == 1 {
        "navigation guide was"
    } else {
        "navigation guides were"
    };
    format!("--deny-ignored rejected the run because {ignored_count} ignored {noun} discovered")
}

pub(crate) fn finish_ignored_policy(
    ignored_count: usize,
    deny_ignored: bool,
) -> Result<CommandOutcome> {
    if ignored_count == 0 {
        return Ok(CommandOutcome::Completed);
    }

    let outcome = CommandOutcome::Ignored {
        count: ignored_count,
    };
    if !deny_ignored {
        return Ok(outcome);
    }

    let message = denied_ignored_message(ignored_count);
    eprintln!("{message}");
    Err(AppError::Other(message).reported())
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
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true, conflicts_with_all = ["quiet", "log_level"])]
    pub verbose: bool,

    /// Enable quiet output (minimal messages)
    #[arg(short, long, global = true, conflicts_with_all = ["verbose", "log_level"])]
    pub quiet: bool,

    /// Set log level directly
    #[arg(
        long,
        global = true,
        value_parser = ["quiet", "default", "verbose"],
        hide = true,
        conflicts_with_all = ["verbose", "quiet"]
    )]
    pub log_level: Option<String>,

    /// Set execution mode directly
    #[arg(
        long,
        global = true,
        value_parser = ["default", "post-tool-use", "pre-commit-hook", "github-actions"],
        hide = true
    )]
    pub execution_mode: Option<String>,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Command,
}

/// Available subcommands
#[derive(Subcommand, Debug)]
pub enum Command {
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
    pub fn apply_environment_defaults(&mut self) -> std::result::Result<(), clap::Error> {
        let defaults = environment::EnvironmentDefaults::capture();
        self.resolve_environment(&defaults)
            .map_err(|error| Self::command().error(ErrorKind::InvalidValue, error.to_string()))
    }

    fn resolve_environment(
        &mut self,
        defaults: &environment::EnvironmentDefaults,
    ) -> std::result::Result<(), environment::EnvironmentError> {
        if !self.verbose && !self.quiet && self.log_level.is_none() {
            self.log_level = defaults.log_mode()?;
        }

        if self.execution_mode.is_none() && !self.command.has_execution_mode_flag() {
            self.execution_mode = defaults.execution_mode()?;
        }

        match &mut self.command {
            Command::Dump(args) => {
                if args.root.is_none() {
                    args.root = defaults.guide_root()?;
                }
            }
            Command::Init(args) => {
                if args.root.is_none() {
                    args.root = defaults.guide_root()?;
                }
            }
            Command::Check(args) => {
                if args.guide.is_none() {
                    args.guide = defaults.guide_path()?;
                }
                if args.guide.is_none() {
                    args.implicit_guide_name = Some(defaults.guide_name()?);
                }
            }
            Command::Verify(args) => {
                if args.root.is_none() {
                    args.root = defaults.guide_root()?;
                }
                if args.recursive {
                    if args.guide_name.is_none() {
                        args.implicit_guide_name = Some(defaults.guide_name()?);
                    }
                } else {
                    if args.guide.is_none() {
                        args.guide = defaults.guide_path()?;
                    }
                    if args.guide.is_none() {
                        args.implicit_guide_name = Some(defaults.guide_name()?);
                    }
                }
            }
        }

        Ok(())
    }

    /// Resolve already-parsed CLI arguments into a Config.
    pub fn build_config(&self) -> Config {
        let log_level = if self.verbose {
            LogLevel::Verbose
        } else if self.quiet {
            LogLevel::Quiet
        } else {
            self.log_level
                .as_ref()
                .and_then(|level| match level.as_str() {
                    "quiet" => Some(LogLevel::Quiet),
                    "verbose" => Some(LogLevel::Verbose),
                    "default" => Some(LogLevel::Default),
                    _ => None,
                })
                .unwrap_or(LogLevel::Default)
        };

        // Resolve execution mode from the direct parameter
        let execution_mode = self
            .execution_mode
            .as_ref()
            .and_then(|mode| match mode.as_str() {
                "post-tool-use" => Some(ExecutionMode::PostToolUse),
                "pre-commit-hook" => Some(ExecutionMode::PreCommitHook),
                "github-actions" => Some(ExecutionMode::GitHubActions),
                "default" => Some(ExecutionMode::Default),
                _ => None,
            })
            .unwrap_or_default();

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
pub fn init_logging(config: &Config) {
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

/// Get the appropriate exit code based on execution mode
pub fn get_exit_code(config: &Config, is_error: bool) -> i32 {
    if is_error {
        match config.execution_mode {
            ExecutionMode::PostToolUse => 2,
            _ => 1,
        }
    } else {
        0
    }
}
