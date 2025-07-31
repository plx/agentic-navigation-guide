//! Init subcommand implementation

use agentic_navigation_guide::dumper::Dumper;
use agentic_navigation_guide::errors::{AppError, Result};
use agentic_navigation_guide::types::Config;
use clap::Args;
use std::path::PathBuf;

/// Arguments for the init subcommand
#[derive(Args, Debug)]
pub struct InitArgs {
    /// Output file path (required)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Maximum depth to traverse
    #[arg(short, long)]
    pub depth: Option<usize>,

    /// Glob patterns to exclude (can be repeated)
    #[arg(short, long)]
    pub exclude: Vec<String>,

    /// Number of spaces for indentation
    #[arg(short, long, default_value = "2")]
    pub indent: usize,

    /// Root directory to dump
    #[arg(short, long, env = "AGENTIC_NAVIGATION_GUIDE_ROOT")]
    pub root: Option<PathBuf>,
}

impl InitArgs {
    /// Execute the init command
    pub fn execute(self, _config: &Config) -> Result<()> {
        // Check if output file already exists
        if self.output.exists() {
            return Err(AppError::Other(format!(
                "File already exists: {}. Use --force to overwrite.",
                self.output.display()
            )));
        }

        // Determine root path
        let root_path = self
            .root
            .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

        log::info!("Initializing navigation guide for: {}", root_path.display());

        // Create dumper
        let dumper = Dumper::new(&root_path)
            .with_max_depth(self.depth)
            .with_exclude_patterns(&self.exclude)?
            .with_indent_size(self.indent);

        // Generate output with wrapper (always include for init)
        let output = dumper.dump_with_wrapper()?;

        // Add header and footer
        let full_output = format!(
            r#"# Agentic Navigation Guide

This navigation guide helps AI coding assistants understand the structure of this project.
The listing below may be incomplete and highlights key files and directories.

{output}

Note: This guide was automatically generated and may need manual adjustments.
"#
        );

        // Write output
        log::info!("Writing navigation guide to: {}", self.output.display());
        std::fs::write(&self.output, full_output)?;

        println!("Navigation guide created at: {}", self.output.display());

        Ok(())
    }
}
