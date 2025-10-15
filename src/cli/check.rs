//! Check subcommand implementation

use agentic_navigation_guide::errors::{ErrorFormatter, Result};
use agentic_navigation_guide::parser::Parser;
use agentic_navigation_guide::types::{Config, ExecutionMode, LogLevel};
use agentic_navigation_guide::validator::Validator;
use clap::Args;
use std::path::{Path, PathBuf};

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
        let guide_path = self
            .guide
            .or_else(|| {
                std::env::var("AGENTIC_NAVIGATION_GUIDE_NAME")
                    .ok()
                    .map(|name| std::env::current_dir().unwrap().join(name))
            })
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap()
                    .join("AGENTIC_NAVIGATION_GUIDE.md")
            });

        log::debug!("Checking navigation guide: {}", guide_path.display());

        // Read the file
        let content = match std::fs::read_to_string(&guide_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file {}: {}", guide_path.display(), e);
                return Err(e.into());
            }
        };

        // Parse the guide
        let parser = Parser::new();
        let guide = match parser.parse(&content) {
            Ok(guide) => guide,
            Err(e) => {
                if config.execution_mode == ExecutionMode::GitHubActions {
                    let formatted = format_github_actions_error(&e, &guide_path, Some(&content));
                    eprintln!("{formatted}");
                } else {
                    let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
                    eprintln!("{formatted}");
                }
                return Err(e);
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
                if config.execution_mode == ExecutionMode::GitHubActions {
                    let formatted = format_github_actions_error(&e, &guide_path, Some(&content));
                    eprintln!("{formatted}");
                } else {
                    let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
                    eprintln!("{formatted}");
                }
                Err(e)
            }
        }
    }
}

/// Format errors specifically for GitHub Actions mode
fn format_github_actions_error(
    error: &agentic_navigation_guide::errors::AppError,
    guide_path: &Path,
    file_content: Option<&str>,
) -> String {
    use agentic_navigation_guide::errors::AppError;

    let mut output = String::new();

    // Error header with emoji
    output.push_str("❌ Navigation guide syntax check failed\n\n");

    // Get line number from error
    let line_num = match error {
        AppError::Syntax(e) => e.line_number(),
        AppError::Semantic(e) => Some(e.line_number()),
        _ => None,
    };

    // Format error with file:line if available
    if let Some(line_num) = line_num {
        let guide_path_str = guide_path.display();
        output.push_str(&format!("{guide_path_str}:{line_num}: "));
        output.push_str(&error.to_string());
        output.push('\n');

        // Show the actual line content if available
        if let Some(content) = file_content {
            if let Some(line) = content.lines().nth(line_num.saturating_sub(1)) {
                let trimmed_line = line.trim_end();
                output.push_str(&format!("  {trimmed_line}\n"));
            }
        }
    } else {
        let guide_path_str = guide_path.display();
        output.push_str(&format!("{guide_path_str}: {error}\n"));
    }

    output
}
