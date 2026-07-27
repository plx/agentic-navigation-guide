//! Recursive navigation guide discovery and verification

use crate::cli::output::{self, GuideCommand, GuideDiagnostic};
use crate::errors::{AppError, Result};
use crate::exclusion::ExclusionMatcher;
use crate::guide_input::{self, GuideAnchor, GuideAuthority, GuideInputError};
use crate::parser::Parser;
use crate::types::{Config, ExecutionMode, LogLevel};
use crate::validator::Validator;
use crate::verifier::Verifier;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Represents a single guide file to be verified
#[derive(Debug, Clone)]
pub(crate) struct GuideLocation {
    /// Path to the guide file
    pub(crate) guide_path: PathBuf,
    /// Root directory for verification (parent of guide file)
    pub(crate) root_path: PathBuf,
    /// Search-root-relative path retained for safe, stable diagnostics.
    pub(crate) logical_path: PathBuf,
}

/// Result of verifying a single guide
#[derive(Debug)]
pub(crate) struct GuideVerificationResult {
    /// The guide that was verified
    pub(crate) location: GuideLocation,
    /// Whether processing completed without a failure.
    ///
    /// An ignored result also sets this shared transport field, but is excluded
    /// from verified-success counts by `ignored`.
    pub(crate) success: bool,
    /// Source-free structured diagnostic if verification failed.
    pub(crate) error: Option<GuideDiagnostic>,
    /// Whether the guide produced the distinct ignored outcome.
    pub(crate) ignored: bool,
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

type ChildNames = Box<dyn Iterator<Item = io::Result<OsString>>>;

/// Recursively find all navigation guide files
pub(crate) fn find_guides(
    root: &Path,
    guide_name: &str,
    exclude_patterns: &[String],
) -> Result<Vec<GuideLocation>> {
    guide_input::validate_implicit_name(guide_name).map_err(map_guide_input_error)?;

    // Validate every pattern before touching the selected root.
    let exclude_matcher = ExclusionMatcher::compile(exclude_patterns)?;
    let mut enumerate = read_child_names;
    find_guides_with(root, guide_name, &exclude_matcher, &mut enumerate)
}

fn find_guides_with<E>(
    root: &Path,
    guide_name: &str,
    exclude_matcher: &ExclusionMatcher,
    enumerate: &mut E,
) -> Result<Vec<GuideLocation>>
where
    E: FnMut(&Path) -> io::Result<ChildNames>,
{
    // Read the selected root before anchor construction to preserve traversal
    // failures as traversal failures. A caller-selected root link is followed
    // by this one explicit read and accepted as the anchor below.
    let root_metadata = fs::metadata(root).map_err(|error| {
        AppError::Other(format!(
            "filesystem walk error: could not inspect the selected recursive root {} ({:?})",
            guide_input::render_path(root),
            error.kind()
        ))
    })?;
    if !root_metadata.is_dir() {
        return Err(AppError::Other(format!(
            "filesystem walk error: the selected recursive root {} is not a directory",
            guide_input::render_path(root)
        )));
    }
    let root_children = enumerate(root)
        .map_err(|error| discovery_enumeration_error(root, Path::new(""), error.kind()))?;
    let anchor = GuideAnchor::new(root).map_err(map_guide_input_error)?;
    let mut guides = Vec::new();
    let mut root_children = Some(root_children);
    let mut pending_directories = vec![PathBuf::new()];

    // Scan and close each directory before visiting any included child
    // directory. This keeps one live traversal-enumeration handle at a time
    // without buffering ordinary file entries from very wide directories.
    while let Some(relative_directory) = pending_directories.pop() {
        let directory = if relative_directory.as_os_str().is_empty() {
            root.to_path_buf()
        } else {
            root.join(&relative_directory)
        };
        let children = match root_children.take() {
            Some(children) => children,
            None => enumerate(&directory).map_err(|error| {
                discovery_enumeration_error(root, &relative_directory, error.kind())
            })?,
        };

        for name in children {
            let name = name.map_err(|error| {
                discovery_enumeration_error(root, &relative_directory, error.kind())
            })?;
            let path = directory.join(&name);
            let relative_path = relative_directory.join(&name);

            // Match the child name before metadata access or opening the child
            // as a directory. Excluded directories therefore never reach
            // `read_dir`.
            if exclude_matcher.is_match(&relative_path)? {
                continue;
            }

            let matches_guide_name = name == OsStr::new(guide_name);
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| discovery_entry_error(&relative_path, error.kind()))?;
            if guide_input::is_link_like(&metadata) {
                if matches_guide_name {
                    anchor
                        .validate_implicit(&path, &relative_path)
                        .map_err(map_guide_input_error)?;
                }
                continue;
            }

            if matches_guide_name {
                anchor
                    .validate_implicit(&path, &relative_path)
                    .map_err(map_guide_input_error)?;

                // Preserve the caller-facing spelling for diagnostics and
                // root-alias behavior. GuideAnchor and Verifier canonicalize
                // this already validated real containing directory internally.
                let root_path = path.parent().unwrap_or(root).to_path_buf();
                guides.push(GuideLocation {
                    guide_path: path,
                    root_path,
                    logical_path: relative_path,
                });
                continue;
            }

            if metadata.is_dir() {
                pending_directories.push(relative_path);
            }
        }
    }

    Ok(guides)
}

fn read_child_names(directory: &Path) -> io::Result<ChildNames> {
    Ok(Box::new(
        fs::read_dir(directory)?.map(|entry| entry.map(|entry| entry.file_name())),
    ))
}

fn discovery_enumeration_error(
    root: &Path,
    relative_directory: &Path,
    kind: io::ErrorKind,
) -> AppError {
    if relative_directory.as_os_str().is_empty() {
        return AppError::Other(format!(
            "filesystem walk error: could not enumerate the selected recursive root {} ({kind:?})",
            guide_input::render_path(root)
        ));
    }
    AppError::Other(format!(
        "filesystem walk error: could not enumerate included directory {} ({kind:?})",
        guide_input::render_path(relative_directory)
    ))
}

fn discovery_entry_error(relative_path: &Path, kind: io::ErrorKind) -> AppError {
    AppError::Other(format!(
        "filesystem walk error: could not inspect included entry {} ({kind:?})",
        guide_input::render_path(relative_path)
    ))
}

/// Verify multiple guides and collect results
pub(crate) fn verify_guides(
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
    // Revalidate and open without following the final entry. This protects
    // both normal discovery and manually constructed internal GuideLocations.
    let content = match anchor.read(
        &location.guide_path,
        &location.logical_path,
        GuideAuthority::Implicit,
    ) {
        Ok(content) => content,
        Err(error) => return failed_guide_input(location, error),
    };

    // Parse the guide
    let parser = Parser::new();
    let guide = match parser.parse(&content) {
        Ok(guide) => guide,
        Err(e) => {
            return GuideVerificationResult {
                location: location.clone(),
                success: false,
                error: Some(GuideDiagnostic::from_error(&e)),
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
        return GuideVerificationResult {
            location: location.clone(),
            success: false,
            error: Some(GuideDiagnostic::from_error(&e)),
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
        Err(e) => GuideVerificationResult {
            location: location.clone(),
            success: false,
            error: Some(GuideDiagnostic::from_error(&e)),
            ignored: false,
        },
    }
}

fn failed_guide_input(location: &GuideLocation, error: GuideInputError) -> GuideVerificationResult {
    GuideVerificationResult {
        location: location.clone(),
        success: false,
        error: Some(GuideDiagnostic::from_message(error.to_string())),
        ignored: false,
    }
}

fn map_guide_input_error(error: GuideInputError) -> AppError {
    AppError::Other(error.to_string())
}

/// Format and display verification results
pub(crate) fn display_results(
    results: &[GuideVerificationResult],
    config: &Config,
) -> Result<bool> {
    let aggregate = VerificationAggregate::from_results(results);

    // Keep the shared internal renderer from treating an empty slice as
    // vacuous success. The CLI normally handles this richer outcome first so
    // it can include the selected root, guide name, and explicit remedy.
    if aggregate.absent != 0 {
        if config.log_level != LogLevel::Quiet {
            output::stderr_line("zero navigation guides were verified")?;
            output::stderr_line(&format!("  {aggregate}"))?;
        }
        return Ok(false);
    }

    // Display individual results based on execution mode
    match config.execution_mode {
        ExecutionMode::GitHubActions => {
            display_github_actions_results(results, config)?;
        }
        ExecutionMode::PostToolUse => {
            display_post_tool_use_results(results, config)?;
        }
        _ => {
            display_default_results(results, config)?;
        }
    }

    // Display summary (unless in quiet mode)
    if config.log_level != LogLevel::Quiet {
        match config.execution_mode {
            ExecutionMode::GitHubActions => {
                if aggregate.failed == 0 {
                    if aggregate.ignored == 0 {
                        output::stdout_line(&format!(
                            "✓ All navigation guides verified ({aggregate})"
                        ))?;
                    } else {
                        output::stdout_line(&format!(
                            "Navigation guide verification complete ({aggregate})"
                        ))?;
                    }
                } else {
                    output::stderr_line(&format!(
                        "❌ Navigation guide verification failed: {aggregate}"
                    ))?;
                }
            }
            _ => {
                if aggregate.failed == 0 {
                    if aggregate.ignored == 0 {
                        output::stdout_line(
                            "✓ All navigation guides are valid and match filesystem",
                        )?;
                    } else if aggregate.passed == 0 {
                        output::stdout_line(
                            "No navigation guides were verified; ignored guides were discovered",
                        )?;
                    } else {
                        output::stdout_line(
                            "Navigation guide verification complete; active guides passed and ignored guides were skipped",
                        )?;
                    }
                    output::stdout_line(&format!("  {aggregate}"))?;
                } else {
                    output::stderr_line("✗ Some navigation guides failed verification")?;
                    output::stderr_line(&format!("  {aggregate}"))?;
                }
            }
        }
    }

    Ok(aggregate.failed == 0)
}

/// Display results for GitHub Actions mode
fn display_github_actions_results(
    results: &[GuideVerificationResult],
    config: &Config,
) -> Result<()> {
    for result in results {
        let guide_path = render_location(&result.location);
        if result.ignored {
            if config.log_level != LogLevel::Quiet {
                output::stderr_line(&format!(
                    "⚠️  Skipping verification: guide at {guide_path} has ignore=true"
                ))?;
            }
        } else if result.success {
            if config.log_level != LogLevel::Quiet {
                output::stdout_line(&format!("✓ {guide_path}: verified"))?;
            }
        } else if let Some(error) = &result.error {
            let root_path = render_root(&result.location);
            output::stderr_line(&error.render(
                GuideCommand::Verify,
                config,
                &guide_path,
                Some(&root_path),
            ))?;
        }
    }
    Ok(())
}

/// Display results for post-tool-use mode
fn display_post_tool_use_results(
    results: &[GuideVerificationResult],
    config: &Config,
) -> Result<()> {
    for result in results {
        let guide_path = render_location(&result.location);
        if result.ignored {
            if config.log_level != LogLevel::Quiet {
                output::stderr_line(&format!(
                    "Warning: Skipping verification of {guide_path} (marked with ignore=true)"
                ))?;
            }
        } else if !result.success {
            if let Some(error) = &result.error {
                let root_path = render_root(&result.location);
                output::stderr_line(&error.render(
                    GuideCommand::Verify,
                    config,
                    &guide_path,
                    Some(&root_path),
                ))?;
            }
        }
    }
    Ok(())
}

/// Display results for default mode
fn display_default_results(results: &[GuideVerificationResult], config: &Config) -> Result<()> {
    for result in results {
        let guide_path = render_location(&result.location);
        if result.ignored {
            if config.log_level != LogLevel::Quiet {
                output::stderr_line(&format!(
                    "Warning: Skipping verification of {guide_path} (marked with ignore=true)"
                ))?;
            }
        } else if result.success {
            if config.log_level == LogLevel::Verbose {
                output::stdout_line(&format!("✓ {guide_path}: valid"))?;
            }
        } else if let Some(error) = &result.error {
            output::stderr_line(&format!("✗ {guide_path}:"))?;
            output::stderr_line(error.reason())?;
            output::stderr_line("")?;
        }
    }
    Ok(())
}

fn render_location(location: &GuideLocation) -> String {
    guide_input::render_path(&location.logical_path)
}

fn render_root(location: &GuideLocation) -> String {
    match location.logical_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => guide_input::render_path(parent),
        _ => "./".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const GUIDE_SOURCE_SENTINEL: &str = "ISSUE49_SECRET_7f4a2d909b6c";

    #[cfg(unix)]
    fn create_guide_file_link(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create guide symlink");
    }

    #[cfg(windows)]
    fn create_guide_file_link(target: &Path, link: &Path) {
        std::os::windows::fs::symlink_file(target, link)
            .expect("Windows file-symlink capability is required for guide-input trust evidence");
    }

    struct CountingChildNames {
        inner: ChildNames,
        live: std::rc::Rc<std::cell::Cell<usize>>,
    }

    impl Iterator for CountingChildNames {
        type Item = io::Result<OsString>;

        fn next(&mut self) -> Option<Self::Item> {
            self.inner.next()
        }
    }

    impl Drop for CountingChildNames {
        fn drop(&mut self) {
            let live = self.live.get();
            assert!(live > 0, "live enumerator count underflow");
            self.live.set(live - 1);
        }
    }

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
        assert!(!display_results(&[], &config).expect("render empty result"));
    }

    #[test]
    fn issue_44_excluded_directories_do_not_reach_the_enumerator() {
        let root = TempDir::new().expect("temporary root");
        fs::create_dir_all(root.path().join("project/target/deep")).expect("excluded subtree");
        fs::write(root.path().join("project/keep.txt"), "").expect("included sibling");

        let matcher = ExclusionMatcher::compile(&["target".to_string()]).expect("valid exclusion");
        let mut enumerated = Vec::new();
        let mut enumerate = |directory: &Path| {
            let relative = directory
                .strip_prefix(root.path())
                .expect("enumerated path beneath fixture root");
            if relative == Path::new("project/target") {
                return Err(io::Error::other(
                    "excluded directory reached the enumeration seam",
                ));
            }
            enumerated.push(
                relative
                    .components()
                    .map(|component| {
                        component
                            .as_os_str()
                            .to_str()
                            .expect("UTF-8 fixture component")
                    })
                    .collect::<Vec<_>>()
                    .join("/"),
            );
            read_child_names(directory)
        };

        let guides = find_guides_with(root.path(), "GUIDE.md", &matcher, &mut enumerate)
            .expect("excluded subtree must not be enumerated");
        assert!(guides.is_empty());
        assert_eq!(enumerated, ["", "project"]);
    }

    #[test]
    fn issue_44_recursive_discovery_keeps_one_live_enumerator() {
        let root = TempDir::new().expect("temporary root");
        let mut deepest = root.path().to_path_buf();
        for _ in 0..64 {
            deepest.push("d");
            fs::create_dir(&deepest).expect("deep fixture directory");
        }
        let deepest_guide = deepest.join("GUIDE.md");
        fs::write(&deepest_guide, "").expect("deep fixture guide");

        let matcher = ExclusionMatcher::compile(&[]).expect("empty exclusion set");
        let live = std::rc::Rc::new(std::cell::Cell::new(0));
        let maximum = std::rc::Rc::new(std::cell::Cell::new(0));
        let openings = std::rc::Rc::new(std::cell::Cell::new(0));
        let closure_live = std::rc::Rc::clone(&live);
        let closure_maximum = std::rc::Rc::clone(&maximum);
        let closure_openings = std::rc::Rc::clone(&openings);
        let mut enumerate = move |directory: &Path| {
            assert_eq!(
                closure_live.get(),
                0,
                "a parent enumerator remained live while opening a child"
            );
            let inner = read_child_names(directory)?;
            closure_openings.set(closure_openings.get() + 1);
            closure_live.set(1);
            closure_maximum.set(closure_maximum.get().max(1));
            Ok(Box::new(CountingChildNames {
                inner,
                live: std::rc::Rc::clone(&closure_live),
            }) as ChildNames)
        };

        let guides = find_guides_with(root.path(), "GUIDE.md", &matcher, &mut enumerate)
            .expect("deep discovery must keep its enumeration handles bounded");
        assert_eq!(guides.len(), 1);
        assert_eq!(guides[0].guide_path, deepest_guide);
        assert_eq!(openings.get(), 65);
        assert_eq!(maximum.get(), 1);
        assert_eq!(live.get(), 0);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn issue_54_internal_recursive_paths_cannot_bypass_safe_opening() {
        let internal_rows = include_str!("../tests/fixtures/v0_2_trust.rs")
            .split("TrustCase {")
            .skip(1)
            .filter_map(|block| {
                let block = block.split_once("},").map_or(block, |(case, _)| case);
                if !block.contains("owner_issue: 49")
                    || !block.contains("normative: TrustOutcome::EnforceSharedPolicy")
                {
                    return None;
                }
                block.lines().find_map(|line| {
                    line.trim()
                        .strip_prefix("id: \"")
                        .and_then(|value| value.strip_suffix("\","))
                })
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            internal_rows,
            std::collections::BTreeSet::from(["trust-guide-direct-library-path"]),
            "the frozen trust ledger must retain exactly one internal direct-route row"
        );

        let temp = TempDir::new().expect("temporary recursive fixture");
        let root = temp.path().join("root");
        let outside = temp.path().join("outside-secret.md");
        fs::create_dir(&root).expect("recursive root");
        fs::write(
            &outside,
            format!("{GUIDE_SOURCE_SENTINEL}\nnot a navigation guide"),
        )
        .expect("outside sentinel");
        let link = root.join("AGENTIC_NAVIGATION_GUIDE.md");
        create_guide_file_link(&outside, &link);

        let discovery = find_guides(&root, "AGENTIC_NAVIGATION_GUIDE.md", &[])
            .expect_err("internal discovery accepted an unsafe matching guide");
        let discovery_error = discovery.to_string();
        assert!(
            discovery_error.contains("unsafe guide path"),
            "{discovery_error}"
        );
        assert!(
            !discovery_error.contains(GUIDE_SOURCE_SENTINEL),
            "{discovery_error}"
        );
        assert!(
            !discovery_error.contains(&outside.display().to_string()),
            "{discovery_error}"
        );

        let results = verify_guides(
            &[GuideLocation {
                guide_path: link,
                root_path: root,
                logical_path: PathBuf::from("AGENTIC_NAVIGATION_GUIDE.md"),
            }],
            &Config::default(),
        )
        .expect("internal verification result");
        let result = results.first().expect("one internal verification result");
        let error = result.error.as_ref().map_or("", GuideDiagnostic::reason);

        assert!(!result.success);
        assert!(error.contains("unsafe guide path"), "{error}");
        assert!(!error.contains(GUIDE_SOURCE_SENTINEL), "{error}");
        assert!(!error.contains(&outside.display().to_string()), "{error}");
    }

    #[cfg(windows)]
    #[test]
    fn issue_54_internal_recursive_path_rejects_windows_stream_before_anchor() {
        let temp = TempDir::new().expect("temporary Windows stream fixture");
        let root = temp.path().join("root");
        let missing_root = temp.path().join("missing-root");
        fs::create_dir(&root).expect("stream fixture root");
        let base = root.join("base.txt");
        fs::write(&base, "ordinary base").expect("stream base");
        let stream = PathBuf::from(format!("{}:secret", base.display()));
        fs::write(&stream, GUIDE_SOURCE_SENTINEL).expect(
            "Windows alternate-data-stream capability is required for guide-input evidence",
        );

        let results = verify_guides(
            &[GuideLocation {
                guide_path: stream,
                root_path: missing_root,
                logical_path: PathBuf::from("base.txt:secret"),
            }],
            &Config::default(),
        )
        .expect("internal Windows verification result");
        let error = results[0]
            .error
            .as_ref()
            .map_or("", GuideDiagnostic::reason);
        assert!(
            !results[0].success
                && error.contains("invalid explicit guide path")
                && !error.contains("trust anchor")
                && !error.contains(GUIDE_SOURCE_SENTINEL),
            "internal GuideLocation bypassed Windows path validation: {error}"
        );
    }
}
