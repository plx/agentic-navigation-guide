//! Agentic Navigation Guide command-line application.
//!
//! The installed `agentic-navigation-guide` executable is the sole supported
//! v0.2 product. This package intentionally exposes no linkable Rust library
//! target or downstream Rust API. The modules below are private implementation
//! details of the binary and may evolve without a Rust SemVer promise.

#![warn(clippy::all)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_const_for_fn,
    clippy::return_self_not_must_use,
    clippy::unused_self,
    clippy::only_used_in_recursion,
    clippy::unnecessary_wraps
)]

use crate::errors::ErrorFormatter;
use clap::Parser;

mod cli;
mod dumper;
mod entry_type;
mod errors;
mod exclusion;
mod guide_input;
mod parser;
mod path_codec;
mod recursive;
mod types;
mod validator;
mod verifier;

#[cfg(test)]
mod containment_guarantee_tests;
#[cfg(test)]
mod exclusion_semantics_tests;
#[cfg(test)]
mod filesystem_identity_snapshot_tests;
#[cfg(test)]
mod parser_robustness_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod v0_2_contract_tests;

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
                // Preserve the command's status even when stderr itself is
                // unavailable; reporting must never introduce a Rust panic.
                let _ = cli::output::stderr_line(&formatted);
            }
            let exit_code = cli::get_exit_code(&config, true);
            std::process::exit(exit_code);
        }
    }
}
