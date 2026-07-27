//! Check subcommand implementation

use super::output::{self, GuideCommand};
use crate::errors::{AppError, Result};
use crate::guide_input::{self, GuideAnchor, GuideAuthority};
use crate::parser::Parser;
use crate::types::{Config, ExecutionMode, LogLevel};
use crate::validator::Validator;
use clap::Args;
use std::path::PathBuf;

/// Arguments for the check subcommand
#[derive(Args, Debug)]
pub(crate) struct CheckArgs {
    /// Path to the navigation guide file
    #[arg(short, long)]
    pub(crate) guide: Option<PathBuf>,

    /// Resolved implicit filename from the environment or built-in default
    #[arg(skip)]
    pub(crate) implicit_guide_name: Option<String>,

    /// Fail when the guide is marked with ignore=true
    #[arg(long)]
    pub(crate) deny_ignored: bool,

    /// Running as post-tool-use hook
    #[arg(long, conflicts_with_all = ["execution_mode", "pre_commit_hook", "github_actions_check"])]
    pub(crate) post_tool_use_hook: bool,

    /// Running as pre-commit hook
    #[arg(long, conflicts_with_all = ["execution_mode", "post_tool_use_hook", "github_actions_check"])]
    pub(crate) pre_commit_hook: bool,

    /// Emit GitHub diagnostics on stderr as path:line: typed reason
    #[arg(long, conflicts_with_all = ["execution_mode", "post_tool_use_hook", "pre_commit_hook"])]
    pub(crate) github_actions_check: bool,
}

impl CheckArgs {
    /// Execute the check command
    pub(crate) fn execute(self, config: &mut Config) -> Result<super::CommandOutcome> {
        // Update execution mode based on flags
        if self.post_tool_use_hook {
            config.execution_mode = ExecutionMode::PostToolUse;
        } else if self.pre_commit_hook {
            config.execution_mode = ExecutionMode::PreCommitHook;
        } else if self.github_actions_check {
            config.execution_mode = ExecutionMode::GitHubActions;
        }

        // Resolve path provenance before opening. `--guide` and its
        // environment equivalent are explicit authority; the default/name
        // selector remains implicit beneath the canonical current directory.
        let current_dir = std::env::current_dir()?;
        let (guide_path, logical_path, authority) = match self.guide {
            Some(path) => {
                guide_input::validate_explicit_path(&path)
                    .map_err(|error| AppError::Other(error.to_string()))?;
                (path.clone(), path, GuideAuthority::Explicit)
            }
            None => {
                let name = self
                    .implicit_guide_name
                    .unwrap_or_else(|| super::environment::DEFAULT_GUIDE_NAME.to_string());
                guide_input::validate_implicit_name(&name)
                    .map_err(|error| AppError::Other(error.to_string()))?;
                (
                    current_dir.join(&name),
                    PathBuf::from(name),
                    GuideAuthority::Implicit,
                )
            }
        };
        let anchor =
            GuideAnchor::new(&current_dir).map_err(|error| AppError::Other(error.to_string()))?;
        let display_path = guide_input::render_path(&logical_path);

        log::debug!("Checking navigation guide: {display_path}");

        let content = anchor
            .read(&guide_path, &logical_path, authority)
            .map_err(|error| AppError::Other(error.to_string()))?;

        // Parse the guide
        let parser = Parser::new();
        let guide = match parser.parse(&content) {
            Ok(guide) => guide,
            Err(e) => {
                output::report_guide_error(&e, GuideCommand::Check, config, &display_path, None)?;
                return Err(e.reported());
            }
        };

        // Check if the guide should be ignored
        if guide.ignore {
            let display_path = guide_input::render_path(&logical_path);

            // Emit warning based on execution mode
            if config.log_level != LogLevel::Quiet {
                match config.execution_mode {
                    ExecutionMode::GitHubActions => {
                        output::stderr_line(&format!(
                            "⚠️  Skipping syntax check: guide at {display_path} has ignore=true"
                        ))?;
                    }
                    _ => {
                        output::stderr_line(&format!(
                            "Warning: Skipping syntax check of {display_path} (marked with ignore=true)"
                        ))?;
                    }
                }

                // Extra warning if this is a standalone guide file
                if guide_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == "AGENTIC_NAVIGATION_GUIDE.md")
                    .unwrap_or(false)
                {
                    output::stderr_line(
                        "Note: Standalone guide file is marked with ignore=true. This may be intentional for examples.",
                    )?;
                }
            }

            return super::finish_ignored_policy(1, self.deny_ignored);
        }

        // Validate syntax
        let validator = Validator::new();
        match validator.validate_syntax(&guide) {
            Ok(()) => {
                if config.log_level != LogLevel::Quiet {
                    match config.execution_mode {
                        ExecutionMode::GitHubActions => {
                            output::stdout_line("✓ Syntax valid")?;
                        }
                        _ => {
                            output::stdout_line("✓ Navigation guide syntax is valid")?;
                        }
                    }
                }
                super::finish_ignored_policy(0, self.deny_ignored)
            }
            Err(e) => {
                output::report_guide_error(&e, GuideCommand::Check, config, &display_path, None)?;
                Err(e.reported())
            }
        }
    }
}
