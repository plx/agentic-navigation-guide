//! Verify subcommand implementation

use agentic_navigation_guide::errors::{ErrorFormatter, Result};
use agentic_navigation_guide::parser::Parser;
use agentic_navigation_guide::types::{Config, ExecutionMode, LogLevel};
use agentic_navigation_guide::validator::Validator;
use agentic_navigation_guide::verifier::Verifier;
use clap::Args;
use std::path::{Path, PathBuf};

/// Arguments for the verify subcommand
#[derive(Args, Debug)]
pub struct VerifyArgs {
    /// Path to the navigation guide file
    #[arg(short, long, env = "AGENTIC_NAVIGATION_GUIDE_PATH")]
    pub guide: Option<PathBuf>,

    /// Root directory for verification
    #[arg(short, long, env = "AGENTIC_NAVIGATION_GUIDE_ROOT")]
    pub root: Option<PathBuf>,

    /// Running as post-tool-use hook
    #[arg(long, conflicts_with_all = ["execution_mode", "pre_commit_hook"])]
    pub post_tool_use_hook: bool,

    /// Running as pre-commit hook
    #[arg(long, conflicts_with_all = ["execution_mode", "post_tool_use_hook"])]
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

        // Store original paths for error messages
        config.original_guide_path = self
            .guide
            .as_ref()
            .map(|p| p.display().to_string())
            .or_else(|| std::env::var("AGENTIC_NAVIGATION_GUIDE_NAME").ok());

        config.original_root_path = self.root.as_ref().map(|p| p.display().to_string());

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

        // Determine root path
        let root_path = self
            .root
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
                if config.execution_mode == ExecutionMode::PostToolUse {
                    let formatted = format_post_tool_use_error(
                        &e,
                        &guide_path,
                        &root_path,
                        config,
                        Some(&content),
                    );
                    eprintln!("{formatted}");
                } else {
                    let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
                    eprintln!("{formatted}");
                }
                return Err(e);
            }
        };

        // First validate syntax
        let validator = Validator::new();
        if let Err(e) = validator.validate_syntax(&guide) {
            if config.execution_mode == ExecutionMode::PostToolUse {
                let formatted =
                    format_post_tool_use_error(&e, &guide_path, &root_path, config, Some(&content));
                eprintln!("{formatted}");
            } else {
                let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
                eprintln!("{formatted}");
            }
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
                if config.execution_mode == ExecutionMode::PostToolUse {
                    let formatted = format_post_tool_use_error(
                        &e,
                        &guide_path,
                        &root_path,
                        config,
                        Some(&content),
                    );
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

/// Format errors specifically for post-tool-use hook mode
fn format_post_tool_use_error(
    error: &agentic_navigation_guide::errors::AppError,
    _guide_path: &Path,
    _root_path: &Path,
    config: &Config,
    file_content: Option<&str>,
) -> String {
    use agentic_navigation_guide::errors::AppError;

    // Get display paths - use original if available, otherwise use defaults
    let display_guide_path = config
        .original_guide_path
        .as_deref()
        .unwrap_or("AGENTIC_NAVIGATION_GUIDE.md");

    let display_root_path = config.original_root_path.as_deref().unwrap_or("./");

    match error {
        AppError::Syntax(_) => {
            // Get the basic error message with context
            let error_detail = ErrorFormatter::format_with_context(error, file_content);

            format!(
                "The agentic navigation guide at {display_guide_path} has a syntax error:\n\n{error_detail}"
            )
        }
        AppError::Semantic(semantic_error) => {
            // For semantic errors, just use the error message without line context
            let error_detail = semantic_error.to_string();

            format!(
                "The agentic navigation guide has become out-of-date vis-a-vis the current state of the file system.\n\n\
                - guide: {display_guide_path}\n\
                - root: {display_root_path}\n\
                - details:\n  - {error_detail}"
            )
        }
        _ => {
            // For other errors, use standard formatting
            ErrorFormatter::format_with_context(error, file_content)
        }
    }
}
