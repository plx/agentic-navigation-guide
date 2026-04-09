//! Verify subcommand implementation

use agentic_navigation_guide::errors::{AppError, ErrorFormatter, Result};
use agentic_navigation_guide::parser::Parser;
use agentic_navigation_guide::types::{
    Config, ExecutionMode, FilesystemItem, LogLevel, NavigationGuideLine,
};
use agentic_navigation_guide::validator::Validator;
use agentic_navigation_guide::verifier::Verifier;
use clap::{Args, ValueEnum};
use std::path::{Path, PathBuf};

/// Output format for the verify command
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Default human-readable output
    Default,
    /// Print parsed paths one per line (directories end with /)
    Paths,
}

/// Arguments for the verify subcommand
#[derive(Args, Debug)]
pub struct VerifyArgs {
    /// Path to the navigation guide file
    #[arg(
        short,
        long,
        env = "AGENTIC_NAVIGATION_GUIDE_PATH",
        conflicts_with = "recursive"
    )]
    pub guide: Option<PathBuf>,

    /// Root directory for verification
    #[arg(short, long, env = "AGENTIC_NAVIGATION_GUIDE_ROOT")]
    pub root: Option<PathBuf>,

    /// Output format (default: human-readable, paths: one parsed path per line)
    #[arg(long, value_enum, default_value = "default")]
    pub format: OutputFormat,

    /// Recursively find and verify all navigation guides
    #[arg(long, conflicts_with_all = ["guide", "format"])]
    pub recursive: bool,

    /// Name of the navigation guide file to search for (only used with --recursive)
    #[arg(long, requires = "recursive", env = "AGENTIC_NAVIGATION_GUIDE_NAME")]
    pub guide_name: Option<String>,

    /// Glob patterns to exclude from recursive search (can be specified multiple times)
    #[arg(long = "exclude", requires = "recursive")]
    pub exclude_patterns: Vec<String>,

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

impl VerifyArgs {
    /// Execute the verify command
    pub fn execute(self, config: &mut Config) -> Result<()> {
        // Update execution mode based on flags
        if self.post_tool_use_hook {
            config.execution_mode = ExecutionMode::PostToolUse;
        } else if self.pre_commit_hook {
            config.execution_mode = ExecutionMode::PreCommitHook;
        } else if self.github_actions_check {
            config.execution_mode = ExecutionMode::GitHubActions;
        }

        // Handle recursive mode
        if self.recursive {
            return self.execute_recursive(config);
        }

        // Store original paths for error messages
        config.original_guide_path = self
            .guide
            .as_ref()
            .map(|p| p.display().to_string())
            .or_else(|| std::env::var("AGENTIC_NAVIGATION_GUIDE_NAME").ok());

        config.original_root_path = self.root.as_ref().map(|p| p.display().to_string());

        // Determine guide/root paths
        let current_dir = std::env::current_dir()?;
        let guide_path = match self.guide {
            Some(path) => path,
            None => match std::env::var("AGENTIC_NAVIGATION_GUIDE_NAME") {
                Ok(name) => current_dir.join(name),
                Err(_) => current_dir.join("AGENTIC_NAVIGATION_GUIDE.md"),
            },
        };

        let root_path = self.root.unwrap_or_else(|| current_dir.clone());

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
                let err: AppError = e.into();
                return Err(err.reported());
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
                } else if config.execution_mode == ExecutionMode::GitHubActions {
                    let formatted =
                        format_github_actions_error(&e, &guide_path, Some(&content), config);
                    eprintln!("{formatted}");
                } else {
                    let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
                    eprintln!("{formatted}");
                }
                return Err(e.reported());
            }
        };

        // If --format paths was requested, emit parsed paths and exit early
        if self.format == OutputFormat::Paths {
            let paths = collect_paths(&guide.items, "");
            for path in paths {
                println!("{path}");
            }
            return Ok(());
        }

        // Check if the guide should be ignored
        if guide.ignore {
            let display_path = config
                .original_guide_path
                .as_deref()
                .unwrap_or("AGENTIC_NAVIGATION_GUIDE.md");

            // Emit warning based on execution mode
            if config.log_level != LogLevel::Quiet {
                match config.execution_mode {
                    ExecutionMode::GitHubActions => {
                        eprintln!(
                            "⚠️  Skipping verification: guide at {display_path} has ignore=true"
                        );
                    }
                    _ => {
                        eprintln!("Warning: Skipping verification of {display_path} (marked with ignore=true)");
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

        // First validate syntax
        let validator = Validator::new();
        if let Err(e) = validator.validate_syntax(&guide) {
            if config.execution_mode == ExecutionMode::PostToolUse {
                let formatted =
                    format_post_tool_use_error(&e, &guide_path, &root_path, config, Some(&content));
                eprintln!("{formatted}");
            } else if config.execution_mode == ExecutionMode::GitHubActions {
                let formatted =
                    format_github_actions_error(&e, &guide_path, Some(&content), config);
                eprintln!("{formatted}");
            } else {
                let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
                eprintln!("{formatted}");
            }
            return Err(e.reported());
        }

        // Then verify against filesystem
        let verifier = Verifier::new(&root_path);
        match verifier.verify(&guide) {
            Ok(()) => {
                if config.log_level != LogLevel::Quiet {
                    match config.execution_mode {
                        ExecutionMode::GitHubActions => {
                            println!("✓ Navigation guide verified");
                        }
                        _ => {
                            println!("✓ Navigation guide is valid and matches filesystem");
                        }
                    }
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
                } else if config.execution_mode == ExecutionMode::GitHubActions {
                    let formatted =
                        format_github_actions_error(&e, &guide_path, Some(&content), config);
                    eprintln!("{formatted}");
                } else {
                    let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
                    eprintln!("{formatted}");
                }
                Err(e.reported())
            }
        }
    }

    /// Execute verification in recursive mode
    fn execute_recursive(self, config: &Config) -> Result<()> {
        use agentic_navigation_guide::recursive;

        // Determine the root path for recursive search
        let search_root = match self.root {
            Some(root) => root,
            None => std::env::current_dir()?,
        };

        // Determine the guide name to search for
        let guide_name = self
            .guide_name
            .unwrap_or_else(|| "AGENTIC_NAVIGATION_GUIDE.md".to_string());

        log::debug!(
            "Recursively searching for '{}' guides in {}",
            guide_name,
            search_root.display()
        );

        // Find all guide files
        let guides = recursive::find_guides(&search_root, &guide_name, &self.exclude_patterns)?;

        if guides.is_empty() {
            if config.log_level != LogLevel::Quiet {
                eprintln!(
                    "No navigation guide files named '{}' found in {}",
                    guide_name,
                    search_root.display()
                );
            }
            return Ok(());
        }

        if config.log_level != LogLevel::Quiet {
            match config.execution_mode {
                ExecutionMode::GitHubActions => {
                    println!("Found {} navigation guide(s)", guides.len());
                }
                _ => {
                    println!(
                        "Found {} navigation guide(s) to verify in {}",
                        guides.len(),
                        search_root.display()
                    );
                }
            }
        }

        // Verify all guides
        let results = recursive::verify_guides(&guides, config)?;

        // Display results and determine exit status
        let all_passed = recursive::display_results(&results, config);

        if all_passed {
            Ok(())
        } else {
            Err(AppError::Other("Some guides failed verification".to_string()).reported())
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

/// Format errors specifically for GitHub Actions mode
fn format_github_actions_error(
    error: &agentic_navigation_guide::errors::AppError,
    _guide_path: &Path,
    file_content: Option<&str>,
    config: &Config,
) -> String {
    use agentic_navigation_guide::errors::AppError;

    let display_guide_path = config
        .original_guide_path
        .as_deref()
        .unwrap_or("AGENTIC_NAVIGATION_GUIDE.md");

    let mut output = String::new();

    // Error header with emoji
    output.push_str("❌ Navigation guide verification failed\n\n");

    // Get line number from error
    let line_num = match error {
        AppError::Syntax(e) => e.line_number(),
        AppError::Semantic(e) => Some(e.line_number()),
        _ => None,
    };

    // Format error with file:line if available
    if let Some(line_num) = line_num {
        output.push_str(&format!("{display_guide_path}:{line_num}: "));
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
        output.push_str(&format!("{display_guide_path}: {error}\n"));
    }

    output
}

/// Recursively collect full paths from parsed guide items.
///
/// Directories are emitted with a trailing `/`. Placeholders are skipped.
fn collect_paths(items: &[NavigationGuideLine], prefix: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for item in items {
        match &item.item {
            FilesystemItem::Directory { path, children, .. } => {
                let full = format!("{prefix}{path}/");
                paths.push(full.clone());
                paths.extend(collect_paths(children, &full));
            }
            FilesystemItem::File { path, .. } => {
                paths.push(format!("{prefix}{path}"));
            }
            FilesystemItem::Symlink { path, .. } => {
                paths.push(format!("{prefix}{path}"));
            }
            FilesystemItem::Placeholder { .. } => {}
        }
    }
    paths
}
