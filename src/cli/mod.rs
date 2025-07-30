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
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Enable quiet output (minimal messages)
    #[arg(short, long, global = true)]
    pub quiet: bool,

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
        let log_level = if self.verbose {
            LogLevel::Verbose
        } else if self.quiet {
            LogLevel::Quiet
        } else {
            LogLevel::Default
        };

        // Check environment variable for log level override
        let log_level = std::env::var("AGENTIC_NAVIGATION_GUIDE_LOG_MODE")
            .ok()
            .and_then(|mode| match mode.as_str() {
                "quiet" => Some(LogLevel::Quiet),
                "verbose" => Some(LogLevel::Verbose),
                "default" => Some(LogLevel::Default),
                _ => None,
            })
            .unwrap_or(log_level);

        // Check execution mode from environment
        let execution_mode = std::env::var("AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE")
            .ok()
            .and_then(|mode| match mode.as_str() {
                "post-tool-use" => Some(ExecutionMode::PostToolUse),
                "pre-commit-hook" => Some(ExecutionMode::PreCommitHook),
                "default" => Some(ExecutionMode::Default),
                _ => None,
            })
            .unwrap_or_default();

        Config {
            execution_mode,
            log_level,
            root_path: None,
            guide_path: None,
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