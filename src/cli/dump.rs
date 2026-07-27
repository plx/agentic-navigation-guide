//! Dump subcommand implementation

use super::output;
use crate::dumper::Dumper;
use crate::errors::Result;
use crate::types::Config;
use clap::Args;
use std::path::PathBuf;

use super::generation_options::{parse_depth, parse_indent};

/// Arguments for the dump subcommand
#[derive(Args, Debug)]
pub(crate) struct DumpArgs {
    /// New output file path (defaults to stdout); existing entries are never overwritten
    #[arg(short, long)]
    pub(crate) output: Option<PathBuf>,

    /// Maximum logical depth, 0 through 256; 0 selects root children; omission rejects depth above 256
    #[arg(short, long, value_parser = parse_depth)]
    pub(crate) depth: Option<usize>,

    /// Exclusion glob: no `/` matches basenames at every depth; `/` matches the full root-relative path; `**` spans path components (repeatable)
    #[arg(short, long)]
    pub(crate) exclude: Vec<String>,

    /// Number of spaces per level, 1 through 16
    #[arg(short, long, default_value = "2", value_parser = parse_indent)]
    pub(crate) indent: usize,

    /// Omit the XML wrapper tags
    #[arg(long)]
    pub(crate) omit_xml_wrapper: bool,

    /// Readable root directory; empty or fully excluded generation fails
    #[arg(short, long)]
    pub(crate) root: Option<PathBuf>,
}

impl DumpArgs {
    /// Execute the dump command
    pub(crate) fn execute(self, _config: &Config) -> Result<()> {
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
            output::stdout(&generated)?;
        }

        Ok(())
    }
}
