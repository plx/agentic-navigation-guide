//! Dump subcommand implementation

use agentic_navigation_guide::dumper::Dumper;
use agentic_navigation_guide::errors::Result;
use agentic_navigation_guide::types::Config;
use clap::Args;
use std::io::Write;
use std::path::PathBuf;

/// Arguments for the dump subcommand
#[derive(Args, Debug)]
pub struct DumpArgs {
    /// Output file path (defaults to stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Maximum depth to traverse
    #[arg(short, long)]
    pub depth: Option<usize>,

    /// Glob patterns to exclude (can be repeated)
    #[arg(short, long)]
    pub exclude: Vec<String>,

    /// Number of spaces for indentation
    #[arg(short, long, default_value = "2")]
    pub indent: usize,

    /// Omit the XML wrapper tags
    #[arg(long)]
    pub omit_xml_wrapper: bool,

    /// Root directory to dump
    #[arg(short, long, env = "AGENTIC_NAVIGATION_GUIDE_ROOT")]
    pub root: Option<PathBuf>,
}

impl DumpArgs {
    /// Execute the dump command
    pub fn execute(self, _config: &Config) -> Result<()> {
        // Determine root path
        let root_path = self
            .root
            .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

        log::debug!("Dumping directory: {}", root_path.display());

        // Create dumper
        let dumper = Dumper::new(&root_path)
            .with_max_depth(self.depth)
            .with_exclude_patterns(&self.exclude)?
            .with_indent_size(self.indent);

        // Generate output
        let output = if self.omit_xml_wrapper {
            dumper.dump()?
        } else {
            dumper.dump_with_wrapper()?
        };

        // Write output
        if let Some(output_path) = self.output {
            log::info!("Writing output to: {}", output_path.display());
            std::fs::write(&output_path, output)?;
        } else {
            print!("{output}");
            std::io::stdout().flush()?;
        }

        Ok(())
    }
}
