use std::ops::{Deref, DerefMut};
use std::time::Duration;
use tempfile::TempDir;

use crate::test_environment::GUIDE_ENVIRONMENT_VARIABLES;

pub struct HermeticAssertCommand {
    command: assert_cmd::Command,
    _default_root: TempDir,
}

impl Deref for HermeticAssertCommand {
    type Target = assert_cmd::Command;

    fn deref(&self) -> &Self::Target {
        &self.command
    }
}

impl DerefMut for HermeticAssertCommand {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.command
    }
}

pub fn assert_cli_command() -> HermeticAssertCommand {
    let default_root = TempDir::new().expect("isolated CLI test default root");
    let mut command =
        assert_cmd::Command::cargo_bin("agentic-navigation-guide").expect("test binary");
    command
        .current_dir(default_root.path())
        .timeout(Duration::from_secs(5));
    for variable in GUIDE_ENVIRONMENT_VARIABLES {
        command.env_remove(variable);
    }
    HermeticAssertCommand {
        command,
        _default_root: default_root,
    }
}
