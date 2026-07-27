use std::ops::{Deref, DerefMut};
use std::process::Command;
use tempfile::TempDir;

use crate::test_environment::GUIDE_ENVIRONMENT_VARIABLES;

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
