use std::ops::{Deref, DerefMut};
use std::process::Command;
use tempfile::TempDir;

const GUIDE_ENVIRONMENT_VARIABLES: &[&str] = &[
    "AGENTIC_NAVIGATION_GUIDE_PATH",
    "AGENTIC_NAVIGATION_GUIDE_ROOT",
    "AGENTIC_NAVIGATION_GUIDE_NAME",
    "AGENTIC_NAVIGATION_GUIDE_LOG_MODE",
    "AGENTIC_NAVIGATION_GUIDE_EXECUTION_MODE",
];

pub struct HermeticProcessCommand {
    command: Command,
    _default_root: TempDir,
}

impl Deref for HermeticProcessCommand {
    type Target = Command;

    fn deref(&self) -> &Self::Target {
        &self.command
    }
}

impl DerefMut for HermeticProcessCommand {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.command
    }
}

pub fn process_cli_command() -> HermeticProcessCommand {
    let default_root = TempDir::new().expect("isolated CLI test default root");
    let mut command = Command::new(env!("CARGO_BIN_EXE_agentic-navigation-guide"));
    command.current_dir(default_root.path());
    for variable in GUIDE_ENVIRONMENT_VARIABLES {
        command.env_remove(variable);
    }
    HermeticProcessCommand {
        command,
        _default_root: default_root,
    }
}
