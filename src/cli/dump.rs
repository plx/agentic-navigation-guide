//! Dump subcommand implementation

use super::output;
use agentic_navigation_guide::dumper::Dumper;
use agentic_navigation_guide::errors::Result;
use agentic_navigation_guide::types::Config;
use clap::Args;
use std::io::Write;
use std::path::PathBuf;

use super::generation_options::{parse_depth, parse_indent};

/// Arguments for the dump subcommand
#[derive(Args, Debug)]
pub struct DumpArgs {
    /// New output file path (defaults to stdout); existing entries are never overwritten
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Maximum logical depth, 0 through 256; 0 selects root children; omitted depth rejects deeper input
    #[arg(short, long, value_parser = parse_depth)]
    pub depth: Option<usize>,

    /// Glob patterns to exclude (can be repeated)
    #[arg(short, long)]
    pub exclude: Vec<String>,

    /// Number of spaces per level, 1 through 16 (default: 2)
    #[arg(short, long, default_value = "2", value_parser = parse_indent)]
    pub indent: usize,

    /// Omit the XML wrapper tags
    #[arg(long)]
    pub omit_xml_wrapper: bool,

    /// Readable root directory; empty or fully excluded generation fails
    #[arg(short, long, env = "AGENTIC_NAVIGATION_GUIDE_ROOT")]
    pub root: Option<PathBuf>,
}

impl DumpArgs {
    /// Execute the dump command
    pub fn execute(self, _config: &Config) -> Result<()> {
        let Self {
            output: output_path,
            depth,
            exclude,
            indent,
            omit_xml_wrapper,
            root,
        } = self;

        // Determine root path
        let root_path = match root {
            Some(root) => root,
            None => std::env::current_dir()?,
        };

        let generate = || -> Result<String> {
            log::debug!("Dumping directory: {}", root_path.display());
            let dumper = Dumper::new(&root_path)
                .with_max_depth(depth)
                .with_exclude_patterns(&exclude)?
                .with_indent_size(indent);
            if omit_xml_wrapper {
                dumper.dump()
            } else {
                dumper.dump_with_wrapper()
            }
        };

        if let Some(output_path) = output_path {
            output::generate_to_file(&root_path, &output_path, || {
                let generated = generate()?;
                log::info!("Writing output to: {}", output_path.display());
                Ok(generated.into_bytes())
            })?;
        } else {
            let generated = generate()?;
            print!("{generated}");
            std::io::stdout().flush()?;
        }

        Ok(())
    }
}
