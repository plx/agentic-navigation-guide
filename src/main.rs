//! Main entry point for the agentic navigation guide CLI

use agentic_navigation_guide::errors::ErrorFormatter;
use clap::Parser;

mod cli;
#[allow(dead_code)]
mod guide_input;
use cli::{Cli, Command};

fn main() {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Build config
    let mut config = cli.build_config();

    // Initialize logging
    cli::init_logging(&config);

    // Execute the command
    let result = match cli.command {
        Command::Dump(args) => args.execute(&config),
        Command::Init(args) => args.execute(&config),
        Command::Check(args) => args.execute(&mut config),
        Command::Verify(args) => args.execute(&mut config),
    };

    // Handle the result and exit with appropriate code
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            if !e.is_reported() {
                let formatted = ErrorFormatter::format_with_context(e.root_cause(), None);
                eprintln!("{formatted}");
            }
            let exit_code = cli::get_exit_code(&config, true);
            std::process::exit(exit_code);
        }
    }
}
