use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub(crate) fn cli_binary() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();

    BINARY
        .get_or_init(|| {
            if let Some(path) = std::env::var_os("CARGO_BIN_EXE_agentic-navigation-guide") {
                return PathBuf::from(path);
            }

            let mut binary = std::env::current_exe().expect("current binary test executable");
            binary.pop();
            if binary.ends_with("deps") {
                binary.pop();
            }
            binary.push(format!(
                "agentic-navigation-guide{}",
                std::env::consts::EXE_SUFFIX
            ));

            if !binary.is_file() {
                let profile_directory = binary
                    .parent()
                    .and_then(Path::file_name)
                    .and_then(|name| name.to_str())
                    .expect("UTF-8 Cargo target profile directory");
                let cargo_profile = if profile_directory == "debug" {
                    "dev"
                } else {
                    profile_directory
                };
                let target_dir = binary
                    .parent()
                    .and_then(Path::parent)
                    .expect("binary below Cargo target profile directory");
                let output = Command::new(env!("CARGO"))
                    .args([
                        "build",
                        "--locked",
                        "--profile",
                        cargo_profile,
                        "--bin",
                        "agentic-navigation-guide",
                    ])
                    .current_dir(env!("CARGO_MANIFEST_DIR"))
                    .env("CARGO_TARGET_DIR", target_dir)
                    .output()
                    .expect("build CLI for binary-owned subprocess tests");
                assert!(
                    output.status.success(),
                    "building the CLI for binary-owned tests failed:\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            assert!(
                binary.is_file(),
                "Cargo-built CLI is unavailable at {}",
                binary.display()
            );
            binary
        })
        .as_path()
}
