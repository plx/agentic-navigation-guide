//! Check subcommand implementation

use agentic_navigation_guide::errors::{ErrorFormatter, Result};
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
    #[arg(long, conflicts_with = "execution_mode")]
    pub post_tool_use_hook: bool,

    /// Running as pre-commit hook
    #[arg(long, conflicts_with = "execution_mode")]
    pub pre_commit_hook: bool,
}

impl CheckArgs {
    /// Execute the check command
    pub fn execute(self, config: &mut Config) -> Result<()> {
        // Update execution mode based on flags
        if self.post_tool_use_hook {
            config.execution_mode = ExecutionMode::PostToolUse;
        } else if self.pre_commit_hook {
            config.execution_mode = ExecutionMode::PreCommitHook;
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
                let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
                eprintln!("{formatted}");
                return Err(e);
            }
        };

        // Validate syntax
        let validator = Validator::new();
        match validator.validate_syntax(&guide) {
            Ok(()) => {
                if config.log_level != LogLevel::Quiet {
                    println!("✓ Navigation guide syntax is valid");
                }
                Ok(())
            }
            Err(e) => {
                let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
                eprintln!("{formatted}");
                Err(e)
            }
        }
    }
}
