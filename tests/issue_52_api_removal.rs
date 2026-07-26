use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn run_consumer_check(consumer: &Path, target: &Path) -> Output {
    Command::new(env!("CARGO"))
        .args(["check", "--offline"])
        .current_dir(consumer)
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_TARGET_DIR", target)
        .output()
        .expect("run downstream cargo check")
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[test]
fn issue_52_packaged_downstream_consumer_cannot_call_removed_method() {
    let temp = TempDir::new().expect("temporary packaged-consumer workspace");
    let package_target = temp.path().join("package-target");
    let package = Command::new(env!("CARGO"))
        .args(["package", "--locked", "--allow-dirty", "--offline"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("CARGO_TARGET_DIR", &package_target)
        .output()
        .expect("package the current candidate");
    assert!(
        package.status.success(),
        "cargo package failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&package.stdout),
        String::from_utf8_lossy(&package.stderr)
    );

    let packaged_crate = package_target.join("package").join(format!(
        "{}-{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    ));
    assert!(
        packaged_crate.join("Cargo.toml").is_file(),
        "cargo package did not leave its verified unpacked artifact at {}",
        packaged_crate.display()
    );

    let consumer = temp.path().join("consumer");
    fs::create_dir(&consumer).expect("consumer directory");
    fs::create_dir(consumer.join("src")).expect("consumer source directory");
    fs::write(
        consumer.join("Cargo.toml"),
        format!(
            "[package]\n\
             name = \"issue-52-downstream-consumer\"\n\
             version = \"0.0.0\"\n\
             edition = \"2021\"\n\
             publish = false\n\
             \n\
             [workspace]\n\
             \n\
             [dependencies]\n\
             agentic-navigation-guide = {{ path = \"{}\" }}\n",
            toml_path(&packaged_crate)
        ),
    )
    .expect("consumer manifest");

    let consumer_source_prefix =
        "use agentic_navigation_guide::{FilesystemItem, NavigationGuide, NavigationGuideLine};\n\
         fn main() {\n\
             let guide = NavigationGuide::new();\n\
             let item = NavigationGuideLine {\n\
                 line_number: 1,\n\
                 indent_level: 0,\n\
                 item: FilesystemItem::File { path: \"child.txt\".into(), comment: None },\n\
             };\n";
    let consumer_target = temp.path().join("consumer-target");
    fs::write(
        consumer.join("src/main.rs"),
        format!("{consumer_source_prefix}    let _path = (&guide, &item);\n}}\n"),
    )
    .expect("positive consumer source");
    let positive = run_consumer_check(&consumer, &consumer_target);
    assert!(
        positive.status.success(),
        "the packaged crate did not compile for the same downstream consumer without the removed \
         method call\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&positive.stdout),
        String::from_utf8_lossy(&positive.stderr)
    );

    fs::write(
        consumer.join("src/main.rs"),
        format!("{consumer_source_prefix}    let _path = guide.get_full_path(&item);\n}}\n"),
    )
    .expect("negative consumer source");
    let negative = run_consumer_check(&consumer, &consumer_target);
    assert!(
        !negative.status.success(),
        "the packaged crate still lets a downstream consumer call get_full_path"
    );
    let diagnostic = String::from_utf8_lossy(&negative.stderr);
    assert!(
        diagnostic.contains("error[E0599]")
            && diagnostic.contains("no method named `get_full_path`")
            && diagnostic.contains("NavigationGuide"),
        "the negative consumer failed for an unrelated reason:\n{diagnostic}"
    );
}
