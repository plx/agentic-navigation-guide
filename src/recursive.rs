//! Recursive navigation guide discovery and verification

use crate::errors::{AppError, ErrorFormatter, Result};
use crate::guide_input::{self, GuideAnchor, GuideAuthority, GuideInputError};
use crate::parser::Parser;
use crate::types::{Config, ExecutionMode, LogLevel};
use crate::validator::Validator;
use crate::verifier::Verifier;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::ffi::OsStr;
use std::fmt;
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
    /// Whether processing completed without a failure.
    ///
    /// An ignored result also sets this transitional transport field, but is
    /// excluded from verified-success counts by `ignored`.
    pub success: bool,
    /// Error message if verification failed
    pub error: Option<String>,
    /// Whether the guide produced the distinct ignored outcome.
    pub ignored: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VerificationAggregate {
    discovered: usize,
    passed: usize,
    failed: usize,
    ignored: usize,
    absent: usize,
}

impl VerificationAggregate {
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
            // Absence is one aggregate search outcome, not a count of files.
            absent: usize::from(discovered == 0),
        }
    }
}

impl fmt::Display for VerificationAggregate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Total: {}, Discovered: {}, Passed: {}, Failed: {}, Ignored: {}, Absent: {}",
            self.discovered, self.discovered, self.passed, self.failed, self.ignored, self.absent
        )
    }
}

/// Recursively find all navigation guide files
pub fn find_guides(
    root: &Path,
    guide_name: &str,
    exclude_patterns: &[String],
) -> Result<Vec<GuideLocation>> {
    let mut guides = Vec::new();

    guide_input::validate_implicit_name(guide_name).map_err(map_guide_input_error)?;

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

    // Walk the selected root without following descendant links. WalkDir
    // follows a root link by default, which permits a caller-selected root
    // alias while the checks below reject link/reparse entries beneath it.
    let mut walker = WalkDir::new(root).follow_links(false).into_iter();
    // Preserve traversal failures as traversal failures (rather than empty
    // discovery) while still validating names and globs before root access.
    if let Some(root_entry) = walker.next() {
        root_entry?;
    }
    let anchor = GuideAnchor::new(root).map_err(map_guide_input_error)?;

    while let Some(entry) = walker.next() {
        let entry = entry?;
        let path = entry.path();

        // Explicit exclusions win before unsafe matching-entry
        // classification, and excluded directories are pruned pre-descent.
        if !should_include_entry(&entry, root, &exclude_globs) {
            if entry.file_type().is_dir() {
                walker.skip_current_dir();
            }
            continue;
        }
        if entry.depth() == 0 {
            continue;
        }

        let matches_guide_name = entry.file_name() == OsStr::new(guide_name);
        let metadata = std::fs::symlink_metadata(path)?;
        if guide_input::is_link_like(&metadata) {
            if guide_input::is_directory_like(&metadata) {
                walker.skip_current_dir();
            }
            if matches_guide_name {
                let logical_path = path.strip_prefix(root).unwrap_or(path);
                anchor
                    .validate_implicit(path, logical_path)
                    .map_err(map_guide_input_error)?;
            }
            continue;
        }
        if !matches_guide_name {
            continue;
        }

        let logical_path = path.strip_prefix(root).unwrap_or(path);
        anchor
            .validate_implicit(path, logical_path)
            .map_err(map_guide_input_error)?;

        // Preserve the caller-facing spelling for diagnostics and root-alias
        // behavior. GuideAnchor and Verifier canonicalize this already
        // validated real containing directory internally.
        let root_path = path.parent().unwrap_or(root).to_path_buf();
        guides.push(GuideLocation {
            guide_path: path.to_path_buf(),
            root_path,
        });
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
    if let Err(error) = guide_input::validate_explicit_path(&location.guide_path) {
        return failed_guide_input(location, error);
    }
    let anchor = match GuideAnchor::new(&location.root_path) {
        Ok(anchor) => anchor,
        Err(error) => return failed_guide_input(location, error),
    };
    let logical_path = location
        .guide_path
        .strip_prefix(&location.root_path)
        .unwrap_or(&location.guide_path);

    // Revalidate and open without following the final entry. This protects
    // both normal discovery and manually constructed legacy GuideLocations.
    let content = match anchor.read(&location.guide_path, logical_path, GuideAuthority::Implicit) {
        Ok(content) => content,
        Err(error) => return failed_guide_input(location, error),
    };

    // Parse the guide
    let parser = Parser::new();
    let guide = match parser.parse(&content) {
        Ok(guide) => guide,
        Err(e) => {
            let formatted = ErrorFormatter::format_with_context(&e, None);
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
        let formatted = ErrorFormatter::format_with_context(&e, None);
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
            let formatted = ErrorFormatter::format_with_context(&e, None);
            GuideVerificationResult {
                location: location.clone(),
                success: false,
                error: Some(formatted),
                ignored: false,
            }
        }
    }
}

fn failed_guide_input(location: &GuideLocation, error: GuideInputError) -> GuideVerificationResult {
    GuideVerificationResult {
        location: location.clone(),
        success: false,
        error: Some(error.to_string()),
        ignored: false,
    }
}

fn map_guide_input_error(error: GuideInputError) -> AppError {
    AppError::Other(error.to_string())
}

/// Format and display verification results
pub fn display_results(results: &[GuideVerificationResult], config: &Config) -> bool {
    let aggregate = VerificationAggregate::from_results(results);

    // Keep the legacy public function from treating an empty slice as
    // vacuous success. The CLI normally handles this richer outcome first so
    // it can include the selected root, guide name, and explicit remedy.
    if aggregate.absent != 0 {
        if config.log_level != LogLevel::Quiet {
            eprintln!("zero navigation guides were verified");
            eprintln!("  {aggregate}");
        }
        return false;
    }

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
                if aggregate.failed == 0 {
                    if aggregate.ignored == 0 {
                        println!("✓ All navigation guides verified ({aggregate})");
                    } else {
                        println!("Navigation guide verification complete ({aggregate})");
                    }
                } else {
                    eprintln!("❌ Navigation guide verification failed: {aggregate}");
                }
            }
            _ => {
                if aggregate.failed == 0 {
                    if aggregate.ignored == 0 {
                        println!("✓ All navigation guides are valid and match filesystem");
                    } else if aggregate.passed == 0 {
                        println!(
                            "No navigation guides were verified; ignored guides were discovered"
                        );
                    } else {
                        println!(
                            "Navigation guide verification complete; active guides passed and ignored guides were skipped"
                        );
                    }
                    println!("  {aggregate}");
                } else {
                    eprintln!("✗ Some navigation guides failed verification");
                    eprintln!("  {aggregate}");
                }
            }
        }
    }

    aggregate.failed == 0
}

/// Display results for GitHub Actions mode
fn display_github_actions_results(results: &[GuideVerificationResult], config: &Config) {
    for result in results {
        let guide_path = render_location(&result.location);
        if result.ignored {
            if config.log_level != LogLevel::Quiet {
                eprintln!("⚠️  Skipping verification: guide at {guide_path} has ignore=true");
            }
        } else if result.success {
            if config.log_level != LogLevel::Quiet {
                println!("✓ {guide_path}: verified");
            }
        } else if let Some(error) = &result.error {
            eprintln!("❌ {guide_path}:");
            eprintln!("{error}");
        }
    }
}

/// Display results for post-tool-use mode
fn display_post_tool_use_results(results: &[GuideVerificationResult], config: &Config) {
    for result in results {
        let guide_path = render_location(&result.location);
        if result.ignored {
            if config.log_level != LogLevel::Quiet {
                eprintln!(
                    "Warning: Skipping verification of {guide_path} (marked with ignore=true)"
                );
            }
        } else if !result.success {
            if let Some(error) = &result.error {
                eprintln!("The agentic navigation guide at {guide_path} has errors:\n\n{error}");
            }
        }
    }
}

/// Display results for default mode
fn display_default_results(results: &[GuideVerificationResult], config: &Config) {
    for result in results {
        let guide_path = render_location(&result.location);
        if result.ignored {
            if config.log_level != LogLevel::Quiet {
                eprintln!(
                    "Warning: Skipping verification of {guide_path} (marked with ignore=true)"
                );
            }
        } else if result.success {
            if config.log_level == LogLevel::Verbose {
                println!("✓ {guide_path}: valid");
            }
        } else if let Some(error) = &result.error {
            eprintln!("✗ {guide_path}:");
            eprintln!("{error}");
            eprintln!();
        }
    }
}

fn render_location(location: &GuideLocation) -> String {
    let logical_path = location
        .guide_path
        .strip_prefix(&location.root_path)
        .unwrap_or(&location.guide_path);
    guide_input::render_path(logical_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_result_slice_is_an_absent_failure_not_vacuous_success() {
        let config = Config {
            log_level: LogLevel::Quiet,
            ..Config::default()
        };

        let aggregate = VerificationAggregate::from_results(&[]);
        assert_eq!(
            aggregate,
            VerificationAggregate {
                discovered: 0,
                passed: 0,
                failed: 0,
                ignored: 0,
                absent: 1,
            }
        );
        assert!(!display_results(&[], &config));
    }
}
