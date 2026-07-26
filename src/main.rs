//! Main entry point for the agentic navigation guide CLI

use agentic_navigation_guide::errors::ErrorFormatter;
use clap::Parser;

mod cli;
#[allow(dead_code)]
mod guide_input;
use cli::{Cli, Command, CommandOutcome};

fn main() {
    // Parse explicit CLI arguments before consulting lower-precedence
    // environment defaults.
    let mut cli = Cli::parse();
    if let Err(error) = cli.apply_environment_defaults() {
        error.exit();
    }

    // Build config
    let mut config = cli.build_config();

    // Initialize logging
    cli::init_logging(&config);

    // Execute the command
    let result = match cli.command {
        Command::Dump(args) => args.execute(&config).map(|()| CommandOutcome::Completed),
        Command::Init(args) => args.execute(&config).map(|()| CommandOutcome::Completed),
        Command::Check(args) => args.execute(&mut config),
        Command::Verify(args) => args.execute(&mut config),
    };

    // Handle the result and exit with appropriate code
    match result {
        Ok(CommandOutcome::Completed) => std::process::exit(0),
        Ok(CommandOutcome::Ignored { count }) => {
            debug_assert_ne!(count, 0);
            std::process::exit(0);
        }
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
