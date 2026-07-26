//! Init subcommand implementation

use super::output;
use agentic_navigation_guide::dumper::Dumper;
use agentic_navigation_guide::errors::Result;
use agentic_navigation_guide::types::Config;
use clap::Args;
use std::path::PathBuf;

use super::generation_options::{parse_depth, parse_indent};

/// Common version control system directories that should be excluded by default
const DEFAULT_VCS_EXCLUDES: &[&str] = &[".git", ".svn", ".hg", ".bzr", "CVS", "_darcs"];

/// Arguments for the init subcommand
#[derive(Args, Debug)]
pub struct InitArgs {
    /// New output file path; existing entries are never overwritten
    #[arg(short, long)]
    pub output: PathBuf,

    /// Maximum logical depth, 0 through 256; 0 selects root children; omission rejects depth above 256
    #[arg(short, long, value_parser = parse_depth)]
    pub depth: Option<usize>,

    /// Glob patterns to exclude (can be repeated)
    #[arg(short, long)]
    pub exclude: Vec<String>,

    /// Number of spaces per level, 1 through 16
    #[arg(short, long, default_value = "2", value_parser = parse_indent)]
    pub indent: usize,

    /// Readable root directory; empty or fully excluded generation fails
    #[arg(short, long, env = "AGENTIC_NAVIGATION_GUIDE_ROOT")]
    pub root: Option<PathBuf>,

    /// Include version control system directories (e.g., .git, .svn, .hg)
    /// By default, common VCS directories are excluded
    #[arg(long)]
    pub include_vcs_directories: bool,
}

impl InitArgs {
    /// Execute the init command
    pub fn execute(self, _config: &Config) -> Result<()> {
        let Self {
            output: output_path,
            depth,
            exclude,
            indent,
            root,
            include_vcs_directories,
        } = self;

        // Determine root path
        let root_path = match root {
            Some(root) => root,
            None => std::env::current_dir()?,
        };

        // Build exclude patterns: VCS directories (unless --include-vcs-directories) + user patterns
        let mut exclude_patterns = Vec::new();
        if !include_vcs_directories {
            exclude_patterns.extend(DEFAULT_VCS_EXCLUDES.iter().map(|s| s.to_string()));
        }
        exclude_patterns.extend(exclude);

        output::generate_to_file(&root_path, &output_path, || {
            log::info!("Initializing navigation guide for: {}", root_path.display());

            let dumper = Dumper::new(&root_path)
                .with_max_depth(depth)
                .with_exclude_patterns(&exclude_patterns)?
                .with_indent_size(indent);
            let output = dumper.dump_with_wrapper()?;

            let full_output = format!(
                r#"# Agentic Navigation Guide

This navigation guide helps AI coding assistants understand the structure of this project.
The listing below may be incomplete and highlights key files and directories.

{output}

Note: This guide was automatically generated and may need manual adjustments.
"#
            );

            log::info!("Writing navigation guide to: {}", output_path.display());
            Ok(full_output.into_bytes())
        })?;

        println!("Navigation guide created at: {}", output_path.display());
        Ok(())
    }
}
