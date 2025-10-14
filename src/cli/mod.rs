//! CLI module for the agentic navigation guide

pub mod check;
pub mod dump;
pub mod init;
pub mod verify;

use agentic_navigation_guide::types::{Config, ExecutionMode, LogLevel};
use clap::{Parser, Subcommand};

/// CLI arguments structure
#[derive(Parser, Debug)]
#[command(
    name = "agentic-navigation-guide",
    about = "A tool for verifying hand-written navigation guides against filesystem structure",
    version,
    author
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
        env = "AGENTIC_NAVIGATION_GUIDE_LOG_MODE",
        value_parser = ["quiet", "default", "verbose"],
        hide = true,
        conflicts_with_all = ["verbose", "quiet"]
    )]
    pub log_level: Option<String>,

    /// Set execution mode directly
    #[arg(
        long,
        global = true,
        env = "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE",
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
    /// Parse CLI arguments and environment variables into a Config
    pub fn build_config(&self) -> Config {
        // First resolve log level from the direct parameter
        let log_level = self
            .log_level
            .as_ref()
            .and_then(|level| match level.as_str() {
                "quiet" => Some(LogLevel::Quiet),
                "verbose" => Some(LogLevel::Verbose),
                "default" => Some(LogLevel::Default),
                _ => None,
            })
            .unwrap_or({
                // Then check convenience flags as overrides
                if self.verbose {
                    LogLevel::Verbose
                } else if self.quiet {
                    LogLevel::Quiet
                } else {
                    LogLevel::Default
                }
            });

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
