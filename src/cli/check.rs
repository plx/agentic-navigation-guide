//! Check subcommand implementation

use super::error_format::{report_guide_error, GuideCommand};
use agentic_navigation_guide::errors::{AppError, Result};
use agentic_navigation_guide::parser::Parser;
use agentic_navigation_guide::types::{Config, ExecutionMode, LogLevel};
use agentic_navigation_guide::validator::Validator;
use clap::Args;
use std::path::PathBuf;

/// Arguments for the check subcommand
#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Path to the navigation guide file
    #[arg(short, long, env = "AGENTIC_NAVIGATION_GUIDE_PATH")]
    pub guide: Option<PathBuf>,

    /// Running as post-tool-use hook
    #[arg(long, conflicts_with_all = ["execution_mode", "pre_commit_hook", "github_actions_check"])]
    pub post_tool_use_hook: bool,

    /// Running as pre-commit hook
    #[arg(long, conflicts_with_all = ["execution_mode", "post_tool_use_hook", "github_actions_check"])]
    pub pre_commit_hook: bool,

    /// Running as GitHub Actions check
    #[arg(long, conflicts_with_all = ["execution_mode", "post_tool_use_hook", "pre_commit_hook"])]
    pub github_actions_check: bool,
}

impl CheckArgs {
    /// Execute the check command
    pub fn execute(self, config: &mut Config) -> Result<()> {
        // Update execution mode based on flags
        if self.post_tool_use_hook {
            config.execution_mode = ExecutionMode::PostToolUse;
        } else if self.pre_commit_hook {
            config.execution_mode = ExecutionMode::PreCommitHook;
        } else if self.github_actions_check {
            config.execution_mode = ExecutionMode::GitHubActions;
        }

        // Determine guide path
        let current_dir = std::env::current_dir()?;
        let guide_path = match self.guide {
            Some(path) => path,
            None => match std::env::var("AGENTIC_NAVIGATION_GUIDE_NAME") {
                Ok(name) => current_dir.join(name),
                Err(_) => current_dir.join("AGENTIC_NAVIGATION_GUIDE.md"),
            },
        };
        // `check` has historically reported the resolved guide path in
        // GitHub Actions diagnostics, including when the filename comes from
        // AGENTIC_NAVIGATION_GUIDE_NAME.
        config.original_guide_path = Some(guide_path.display().to_string());

        log::debug!("Checking navigation guide: {}", guide_path.display());

        // Read the file
        let content = match std::fs::read_to_string(&guide_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file {}: {}", guide_path.display(), e);
                let err: AppError = e.into();
                return Err(err.reported());
            }
        };

        // Parse the guide
        let parser = Parser::new();
        let guide = match parser.parse(&content) {
            Ok(guide) => guide,
            Err(e) => {
                report_guide_error(&e, config, Some(&content), GuideCommand::Check);
                return Err(e.reported());
            }
        };

        // Check if the guide should be ignored
        if guide.ignore {
            let display_path = guide_path.display();

            // Emit warning based on execution mode
            if config.log_level != LogLevel::Quiet {
                match config.execution_mode {
                    ExecutionMode::GitHubActions => {
                        eprintln!(
                            "⚠️  Skipping syntax check: guide at {display_path} has ignore=true"
                        );
                    }
                    _ => {
                        eprintln!("Warning: Skipping syntax check of {display_path} (marked with ignore=true)");
                    }
                }

                // Extra warning if this is a standalone guide file
                if guide_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == "AGENTIC_NAVIGATION_GUIDE.md")
                    .unwrap_or(false)
                {
                    eprintln!(
                        "Note: Standalone guide file is marked with ignore=true. This may be intentional for examples."
                    );
                }
            }

            return Ok(());
        }

        // Validate syntax
        let validator = Validator::new();
        match validator.validate_syntax(&guide) {
            Ok(()) => {
                if config.log_level != LogLevel::Quiet {
                    match config.execution_mode {
                        ExecutionMode::GitHubActions => {
                            println!("✓ Syntax valid");
                        }
                        _ => {
                            println!("✓ Navigation guide syntax is valid");
                        }
                    }
                }
                Ok(())
            }
            Err(e) => {
                report_guide_error(&e, config, Some(&content), GuideCommand::Check);
                Err(e.reported())
            }
        }
    }
}
