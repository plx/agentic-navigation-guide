//! Recursive navigation guide discovery and verification

use crate::errors::{ErrorFormatter, Result};
use crate::parser::Parser;
use crate::types::{Config, ExecutionMode, LogLevel};
use crate::validator::Validator;
use crate::verifier::Verifier;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Represents a single guide file to be verified
#[derive(Debug, Clone)]
pub struct GuideLocation {
    /// Path to the guide file
    pub guide_path: PathBuf,
    /// Root directory for verification (parent of guide file)
    pub root_path: PathBuf,
}

/// Result of verifying a single guide
#[derive(Debug)]
pub struct GuideVerificationResult {
    /// The guide that was verified
    pub location: GuideLocation,
    /// Whether verification succeeded
    pub success: bool,
    /// Error message if verification failed
    pub error: Option<String>,
    /// Whether the guide was ignored (has ignore=true)
    pub ignored: bool,
}

/// Recursively find all navigation guide files
pub fn find_guides(
    root: &Path,
    guide_name: &str,
    exclude_patterns: &[String],
) -> Result<Vec<GuideLocation>> {
    let mut guides = Vec::new();

    // Build exclusion glob set
    let exclude_globs = if exclude_patterns.is_empty() {
        None
    } else {
        let mut builder = GlobSetBuilder::new();
        for pattern in exclude_patterns {
            builder.add(Glob::new(pattern)?);
        }
        Some(builder.build()?)
    };

    // Walk directory tree
    let walker = WalkDir::new(root).follow_links(false).into_iter();

    for entry in walker.filter_entry(|e| should_include_entry(e, root, &exclude_globs)) {
        let entry = entry?;
        let path = entry.path();

        // Check if this is a guide file
        if path.is_file() {
            if let Some(file_name) = path.file_name() {
                if file_name == guide_name {
                    // The root for this guide is its parent directory
                    let root_path = path.parent().unwrap_or(root).to_path_buf();

                    guides.push(GuideLocation {
                        guide_path: path.to_path_buf(),
                        root_path,
                    });
                }
            }
        }
    }

    Ok(guides)
}

/// Check if a directory entry should be included in the walk
fn should_include_entry(
    entry: &walkdir::DirEntry,
    root: &Path,
    exclude_globs: &Option<GlobSet>,
) -> bool {
    if let Some(globs) = exclude_globs {
        let path = entry.path();
        if let Ok(relative_path) = path.strip_prefix(root) {
            // Check the full relative path
            if globs.is_match(relative_path) {
                return false;
            }

            // For directories, check if any parent component matches
            let mut current_path = PathBuf::new();
            for component in relative_path.components() {
                current_path.push(component);
                if globs.is_match(&current_path) {
                    return false;
                }
            }
        }
    }
    true
}

/// Verify multiple guides and collect results
pub fn verify_guides(
    guides: &[GuideLocation],
    config: &Config,
) -> Result<Vec<GuideVerificationResult>> {
    let mut results = Vec::new();

    for location in guides {
        let result = verify_single_guide(location, config);
        results.push(result);
    }

    Ok(results)
}

/// Verify a single guide and return the result
fn verify_single_guide(location: &GuideLocation, _config: &Config) -> GuideVerificationResult {
    // Read the file
    let content = match std::fs::read_to_string(&location.guide_path) {
        Ok(content) => content,
        Err(e) => {
            return GuideVerificationResult {
                location: location.clone(),
                success: false,
                error: Some(format!("Error reading file: {e}")),
                ignored: false,
            };
        }
    };

    // Parse the guide
    let parser = Parser::new();
    let guide = match parser.parse(&content) {
        Ok(guide) => guide,
        Err(e) => {
            let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
            return GuideVerificationResult {
                location: location.clone(),
                success: false,
                error: Some(formatted),
                ignored: false,
            };
        }
    };

    // Check if the guide should be ignored
    if guide.ignore {
        return GuideVerificationResult {
            location: location.clone(),
            success: true,
            error: None,
            ignored: true,
        };
    }

    // Validate syntax
    let validator = Validator::new();
    if let Err(e) = validator.validate_syntax(&guide) {
        let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
        return GuideVerificationResult {
            location: location.clone(),
            success: false,
            error: Some(formatted),
            ignored: false,
        };
    }

    // Verify against filesystem
    let verifier = Verifier::new(&location.root_path);
    match verifier.verify(&guide) {
        Ok(()) => GuideVerificationResult {
            location: location.clone(),
            success: true,
            error: None,
            ignored: false,
        },
        Err(e) => {
            let formatted = ErrorFormatter::format_with_context(&e, Some(&content));
            GuideVerificationResult {
                location: location.clone(),
                success: false,
                error: Some(formatted),
                ignored: false,
            }
        }
    }
}

/// Format and display verification results
pub fn display_results(results: &[GuideVerificationResult], config: &Config) -> bool {
    let total = results.len();
    let passed = results.iter().filter(|r| r.success && !r.ignored).count();
    let ignored = results.iter().filter(|r| r.ignored).count();
    let failed = results.iter().filter(|r| !r.success).count();

    // Display individual results based on execution mode
    match config.execution_mode {
        ExecutionMode::GitHubActions => {
            display_github_actions_results(results, config);
        }
        ExecutionMode::PostToolUse => {
            display_post_tool_use_results(results, config);
        }
        _ => {
            display_default_results(results, config);
        }
    }

    // Display summary (unless in quiet mode)
    if config.log_level != LogLevel::Quiet {
        match config.execution_mode {
            ExecutionMode::GitHubActions => {
                if failed == 0 {
                    println!("✓ All navigation guides verified ({total} total)");
                } else {
                    eprintln!("❌ Navigation guide verification failed: {passed} passed, {failed} failed, {ignored} ignored");
                }
            }
            _ => {
                if failed == 0 {
                    println!("✓ All navigation guides are valid and match filesystem");
                    println!("  Total: {total}, Passed: {passed}, Ignored: {ignored}");
                } else {
                    eprintln!("✗ Some navigation guides failed verification");
                    eprintln!(
                        "  Total: {total}, Passed: {passed}, Failed: {failed}, Ignored: {ignored}"
                    );
                }
            }
        }
    }

    failed == 0
}

/// Display results for GitHub Actions mode
fn display_github_actions_results(results: &[GuideVerificationResult], config: &Config) {
    for result in results {
        if result.ignored {
            if config.log_level != LogLevel::Quiet {
                eprintln!(
                    "⚠️  Skipping verification: guide at {} has ignore=true",
                    result.location.guide_path.display()
                );
            }
        } else if result.success {
            if config.log_level != LogLevel::Quiet {
                println!("✓ {}: verified", result.location.guide_path.display());
            }
        } else if let Some(error) = &result.error {
            eprintln!("❌ {}:", result.location.guide_path.display());
            eprintln!("{error}");
        }
    }
}

/// Display results for post-tool-use mode
fn display_post_tool_use_results(results: &[GuideVerificationResult], config: &Config) {
    for result in results {
        if result.ignored {
            if config.log_level != LogLevel::Quiet {
                eprintln!(
                    "Warning: Skipping verification of {} (marked with ignore=true)",
                    result.location.guide_path.display()
                );
            }
        } else if !result.success {
            if let Some(error) = &result.error {
                eprintln!(
                    "The agentic navigation guide at {} has errors:\n\n{}",
                    result.location.guide_path.display(),
                    error
                );
            }
        }
    }
}

/// Display results for default mode
fn display_default_results(results: &[GuideVerificationResult], config: &Config) {
    for result in results {
        if result.ignored {
            if config.log_level != LogLevel::Quiet {
                eprintln!(
                    "Warning: Skipping verification of {} (marked with ignore=true)",
                    result.location.guide_path.display()
                );
            }
        } else if result.success {
            if config.log_level == LogLevel::Verbose {
                println!("✓ {}: valid", result.location.guide_path.display());
            }
        } else if let Some(error) = &result.error {
            eprintln!("✗ {}:", result.location.guide_path.display());
            eprintln!("{error}");
            eprintln!();
        }
    }
}
