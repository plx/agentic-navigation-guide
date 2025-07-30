//! Verify subcommand implementation

use agentic_navigation_guide::errors::{ErrorFormatter, Result};
use agentic_navigation_guide::parser::Parser;
use agentic_navigation_guide::types::{Config, ExecutionMode, LogLevel};
use agentic_navigation_guide::validator::Validator;
use agentic_navigation_guide::verifier::Verifier;
use clap::Args;
use std::path::PathBuf;

/// Arguments for the verify subcommand
#[derive(Args, Debug)]
pub struct VerifyArgs {
    /// Path to the navigation guide file
    #[arg(short, long)]
    pub guide: Option<PathBuf>,

    /// Root directory for verification
    #[arg(short, long)]
    pub root: Option<PathBuf>,

    /// Running as post-tool-use hook
    #[arg(long)]
    pub post_tool_use_hook: bool,

    /// Running as pre-commit hook
    #[arg(long)]
    pub pre_commit_hook: bool,
}

impl VerifyArgs {
    /// Execute the verify command
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
                std::env::var("AGENTIC_NAVIGATION_GUIDE_PATH")
                    .ok()
                    .map(PathBuf::from)
            })
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

        // Determine root path
        let root_path = self
            .root
            .or_else(|| {
                std::env::var("AGENTIC_NAVIGATION_GUIDE_ROOT")
                    .ok()
                    .map(PathBuf::from)
            })
            .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

        log::debug!(
            "Verifying navigation guide: {} against root: {}",
            guide_path.display(),
            root_path.display()
        );

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

        // First validate syntax
        let validator = Validator::new();
        if let Err(e) = validator.validate_syntax(&guide) {
            let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
            eprintln!("{formatted}");
            return Err(e);
        }

        // Then verify against filesystem
        let verifier = Verifier::new(&root_path);
        match verifier.verify(&guide) {
            Ok(()) => {
                if config.log_level != LogLevel::Quiet {
                    println!("✓ Navigation guide is valid and matches filesystem");
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
