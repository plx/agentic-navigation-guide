//! Verify subcommand implementation

use crate::errors::{AppError, ErrorFormatter, Result};
use crate::guide_input::{self, GuideAnchor, GuideAuthority, GuideInputError};
use crate::parser::Parser;
use crate::recursive::{self, GuideLocation, GuideVerificationResult};
use crate::types::{Config, ExecutionMode, LogLevel};
use crate::validator::Validator;
use crate::verifier::Verifier;
use clap::Args;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RecursiveAggregate {
    discovered: usize,
    passed: usize,
    failed: usize,
    ignored: usize,
    absent: usize,
}

impl RecursiveAggregate {
    const fn absent() -> Self {
        Self {
            discovered: 0,
            passed: 0,
            failed: 0,
            ignored: 0,
            // This is one absent-search outcome, not a count of files that
            // might have been expected to exist.
            absent: 1,
        }
    }

    fn from_results(results: &[GuideVerificationResult]) -> Self {
        let discovered = results.len();
        Self {
            discovered,
            passed: results
                .iter()
                .filter(|result| result.success && !result.ignored)
                .count(),
            failed: results.iter().filter(|result| !result.success).count(),
            ignored: results.iter().filter(|result| result.ignored).count(),
            absent: usize::from(discovered == 0),
        }
    }
}

impl fmt::Display for RecursiveAggregate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Total: {}, Discovered: {}, Passed: {}, Failed: {}, Ignored: {}, Absent: {}",
            self.discovered, self.discovered, self.passed, self.failed, self.ignored, self.absent
        )
    }
}

#[derive(Debug)]
struct NoGuidesFound {
    search_root: PathBuf,
    guide_name: String,
    aggregate: RecursiveAggregate,
}

impl fmt::Display for NoGuidesFound {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "zero navigation guides were verified: no files named {} were discovered under {}. \
             Check --root, --guide-name, and --exclude; pass --allow-empty only when an empty \
             recursive search is intentional.",
            guide_input::render_path(Path::new(&self.guide_name)),
            guide_input::render_path(&self.search_root)
        )
    }
}

impl std::error::Error for NoGuidesFound {}

enum RecursiveDiscovery {
    Found(Vec<GuideLocation>),
    Absent(NoGuidesFound),
}

impl RecursiveDiscovery {
    fn classify(guides: Vec<GuideLocation>, search_root: PathBuf, guide_name: String) -> Self {
        if guides.is_empty() {
            Self::Absent(NoGuidesFound {
                search_root,
                guide_name,
                aggregate: RecursiveAggregate::absent(),
            })
        } else {
            Self::Found(guides)
        }
    }
}

fn finish_empty_discovery(error: NoGuidesFound, allow_empty: bool, config: &Config) -> Result<()> {
    if allow_empty {
        if config.log_level != LogLevel::Quiet {
            println!("--allow-empty accepted: zero navigation guides were verified");
            println!("  {}", error.aggregate);
        }
        Ok(())
    } else {
        eprintln!("{error}");
        eprintln!("  {}", error.aggregate);
        Err(AppError::Other(error.to_string()).reported())
    }
}

/// Arguments for the verify subcommand
#[derive(Args, Debug)]
pub(crate) struct VerifyArgs {
    /// Path to the navigation guide file
    #[arg(short, long, conflicts_with = "recursive")]
    pub(crate) guide: Option<PathBuf>,

    /// Fail when any discovered guide is marked with ignore=true
    #[arg(long)]
    pub(crate) deny_ignored: bool,

    /// Root directory for verification
    #[arg(short, long)]
    pub(crate) root: Option<PathBuf>,

    /// Recursively find and verify all navigation guides
    #[arg(long, conflicts_with = "guide")]
    pub(crate) recursive: bool,

    /// Name of the navigation guide file to search for (only used with --recursive)
    #[arg(long, requires = "recursive")]
    pub(crate) guide_name: Option<String>,

    /// Resolved implicit filename from the environment or built-in default
    #[arg(skip)]
    pub(crate) implicit_guide_name: Option<String>,

    /// Exclusion glob: no `/` matches basenames at every depth; `/` matches the full root-relative path; `**` spans path components (repeatable)
    #[arg(long = "exclude", requires = "recursive")]
    pub(crate) exclude_patterns: Vec<String>,

    /// Allow a recursive search to succeed after discovering zero guides
    #[arg(long, requires = "recursive")]
    pub(crate) allow_empty: bool,

    /// Running as post-tool-use hook
    #[arg(long, conflicts_with_all = ["execution_mode", "pre_commit_hook", "github_actions_check"])]
    pub(crate) post_tool_use_hook: bool,

    /// Running as pre-commit hook
    #[arg(long, conflicts_with_all = ["execution_mode", "post_tool_use_hook", "github_actions_check"])]
    pub(crate) pre_commit_hook: bool,

    /// Running as GitHub Actions check
    #[arg(long, conflicts_with_all = ["execution_mode", "post_tool_use_hook", "pre_commit_hook"])]
    pub(crate) github_actions_check: bool,
}

impl VerifyArgs {
    /// Execute the verify command
    pub(crate) fn execute(self, config: &mut Config) -> Result<super::CommandOutcome> {
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

        // Resolve the effective root first: an implicit single-guide verify
        // finds its default guide beneath this root, not beneath an unrelated
        // current working directory.
        let current_dir = std::env::current_dir()?;
        let root_path = self.root.unwrap_or_else(|| current_dir.clone());
        let (guide_path, logical_path, authority) = match self.guide {
            Some(path) => {
                guide_input::validate_explicit_path(&path).map_err(report_guide_input_error)?;
                (path.clone(), path, GuideAuthority::Explicit)
            }
            None => {
                let name = self
                    .implicit_guide_name
                    .unwrap_or_else(|| super::environment::DEFAULT_GUIDE_NAME.to_string());
                guide_input::validate_implicit_name(&name).map_err(report_guide_input_error)?;
                (
                    root_path.join(&name),
                    PathBuf::from(name),
                    GuideAuthority::Implicit,
                )
            }
        };
        let anchor = GuideAnchor::new(&root_path).map_err(report_guide_input_error)?;

        // Store control-safe caller-facing spellings for hook diagnostics.
        config.original_guide_path = Some(guide_input::render_path(&logical_path));
        config.original_root_path = Some(guide_input::render_path(&root_path));

        log::debug!(
            "Verifying navigation guide: {} against root: {}",
            guide_input::render_path(&logical_path),
            guide_input::render_path(&root_path)
        );

        let content = anchor
            .read(&guide_path, &logical_path, authority)
            .map_err(report_guide_input_error)?;

        // Parse the guide
        let parser = Parser::new();
        let guide = match parser.parse(&content) {
            Ok(guide) => guide,
            Err(e) => {
                if config.execution_mode == ExecutionMode::PostToolUse {
                    let formatted = format_post_tool_use_error(&e, &guide_path, &root_path, config);
                    eprintln!("{formatted}");
                } else if config.execution_mode == ExecutionMode::GitHubActions {
                    let formatted = format_github_actions_error(&e, &guide_path, config);
                    eprintln!("{formatted}");
                } else {
                    let formatted = ErrorFormatter::format_with_context(&e, None);
                    eprintln!("{formatted}");
                }
                return Err(e.reported());
            }
        };

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

            return super::finish_ignored_policy(1, self.deny_ignored);
        }

        // First validate syntax
        let validator = Validator::new();
        if let Err(e) = validator.validate_syntax(&guide) {
            if config.execution_mode == ExecutionMode::PostToolUse {
                let formatted = format_post_tool_use_error(&e, &guide_path, &root_path, config);
                eprintln!("{formatted}");
            } else if config.execution_mode == ExecutionMode::GitHubActions {
                let formatted = format_github_actions_error(&e, &guide_path, config);
                eprintln!("{formatted}");
            } else {
                let formatted = ErrorFormatter::format_with_context(&e, None);
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
                super::finish_ignored_policy(0, self.deny_ignored)
            }
            Err(e) => {
                if config.execution_mode == ExecutionMode::PostToolUse {
                    let formatted = format_post_tool_use_error(&e, &guide_path, &root_path, config);
                    eprintln!("{formatted}");
                } else if config.execution_mode == ExecutionMode::GitHubActions {
                    let formatted = format_github_actions_error(&e, &guide_path, config);
                    eprintln!("{formatted}");
                } else {
                    let formatted = ErrorFormatter::format_with_context(&e, None);
                    eprintln!("{formatted}");
                }
                Err(e.reported())
            }
        }
    }

    /// Execute verification in recursive mode
    fn execute_recursive(self, config: &Config) -> Result<super::CommandOutcome> {
        // Determine the root path for recursive search
        let search_root = match self.root {
            Some(root) => root,
            None => std::env::current_dir()?,
        };

        // Determine the guide name to search for
        let guide_name = self
            .guide_name
            .or(self.implicit_guide_name)
            .unwrap_or_else(|| super::environment::DEFAULT_GUIDE_NAME.to_string());

        log::debug!(
            "Recursively searching for {} guides in {}",
            guide_input::render_path(Path::new(&guide_name)),
            guide_input::render_path(&search_root)
        );

        // Find all guide files
        let guides = recursive::find_guides(&search_root, &guide_name, &self.exclude_patterns)?;
        let guides =
            match RecursiveDiscovery::classify(guides, search_root.clone(), guide_name.clone()) {
                RecursiveDiscovery::Found(guides) => guides,
                RecursiveDiscovery::Absent(error) => {
                    return finish_empty_discovery(error, self.allow_empty, config)
                        .map(|()| super::CommandOutcome::Completed);
                }
            };

        if config.log_level != LogLevel::Quiet {
            match config.execution_mode {
                ExecutionMode::GitHubActions => {
                    println!("Found {} navigation guide(s)", guides.len());
                }
                _ => {
                    println!(
                        "Found {} navigation guide(s) to verify in {}",
                        guides.len(),
                        guide_input::render_path(&search_root)
                    );
                }
            }
        }

        // Verify all guides
        let results = recursive::verify_guides(&guides, config)?;
        let aggregate = RecursiveAggregate::from_results(&results);

        // Display results and determine exit status
        let all_passed = recursive::display_results(&results, config);

        if self.deny_ignored && aggregate.ignored != 0 {
            // Quiet suppresses ordinary success chatter, not the aggregate
            // attached to a policy failure.
            if config.log_level == LogLevel::Quiet {
                eprintln!("  {aggregate}");
            }
            if !all_passed {
                let message = format!(
                    "Some guides failed verification, and {}",
                    super::denied_ignored_message(aggregate.ignored)
                );
                eprintln!("{message}");
                return Err(AppError::Other(message).reported());
            }
            return super::finish_ignored_policy(aggregate.ignored, true);
        }

        if !all_passed {
            Err(AppError::Other("Some guides failed verification".to_string()).reported())
        } else {
            super::finish_ignored_policy(aggregate.ignored, self.deny_ignored)
        }
    }
}

/// Format errors specifically for post-tool-use hook mode
fn format_post_tool_use_error(
    error: &crate::errors::AppError,
    _guide_path: &Path,
    _root_path: &Path,
    config: &Config,
) -> String {
    use crate::errors::AppError;

    // Get display paths - use original if available, otherwise use defaults
    let display_guide_path = config
        .original_guide_path
        .as_deref()
        .unwrap_or("AGENTIC_NAVIGATION_GUIDE.md");

    let display_root_path = config.original_root_path.as_deref().unwrap_or("./");

    match error {
        AppError::Syntax(_) => {
            let error_detail = ErrorFormatter::format_with_context(error, None);

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
        _ => ErrorFormatter::format_with_context(error, None),
    }
}

/// Format errors specifically for GitHub Actions mode
fn format_github_actions_error(
    error: &crate::errors::AppError,
    _guide_path: &Path,
    config: &Config,
) -> String {
    use crate::errors::AppError;

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
    } else {
        output.push_str(&format!("{display_guide_path}: {error}\n"));
    }

    output
}

fn report_guide_input_error(error: GuideInputError) -> AppError {
    eprintln!("{error}");
    AppError::Other(error.to_string()).reported()
}
