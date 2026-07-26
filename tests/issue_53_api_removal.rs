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

fn write_source_and_check(consumer: &Path, target: &Path, source: &str) -> Output {
    fs::write(consumer.join("src/main.rs"), source).expect("consumer source");
    run_consumer_check(consumer, target)
}

fn assert_missing_variant(output: &Output, variant: &str, enum_name: &str) {
    assert!(
        !output.status.success(),
        "the packaged crate still exposes {enum_name}::{variant}"
    );
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("error[E0599]")
            && diagnostic.contains(&format!("no variant named `{variant}`"))
            && diagnostic.contains(enum_name),
        "the {enum_name}::{variant} consumer failed for an unrelated reason:\n{diagnostic}"
    );
}

#[test]
fn issue_53_packaged_downstream_consumers_cannot_name_removed_variants() {
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
             name = \"issue-53-downstream-consumer\"\n\
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

    let positive_source = "use agentic_navigation_guide::{FilesystemItem, SemanticError};\n\
         fn selected_item_variant(item: FilesystemItem) -> bool {\n\
             matches!(item, FilesystemItem::File { .. })\n\
         }\n\
         fn selected_error_variant(error: SemanticError) -> bool {\n\
             matches!(error, SemanticError::TypeMismatch { .. })\n\
         }\n\
         fn main() {\n\
             let _item_matcher: fn(FilesystemItem) -> bool = selected_item_variant;\n\
             let _error_matcher: fn(SemanticError) -> bool = selected_error_variant;\n\
         }\n";
    let consumer_target = temp.path().join("consumer-target");
    let positive = write_source_and_check(&consumer, &consumer_target, positive_source);
    assert!(
        positive.status.success(),
        "the packaged crate did not compile for the same downstream consumer using surviving \
         variants\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&positive.stdout),
        String::from_utf8_lossy(&positive.stderr)
    );

    let item_negative_source =
        positive_source.replacen("FilesystemItem::File", "FilesystemItem::Symlink", 1);
    let item_negative = write_source_and_check(&consumer, &consumer_target, &item_negative_source);

    let error_negative_source = positive_source.replacen(
        "SemanticError::TypeMismatch",
        "SemanticError::SymlinkTargetMismatch",
        1,
    );
    let error_negative =
        write_source_and_check(&consumer, &consumer_target, &error_negative_source);

    assert!(
        !item_negative.status.success() && !error_negative.status.success(),
        "the packaged crate still exposes at least one selected variant\n\
         FilesystemItem::Symlink status: {}\n\
         SemanticError::SymlinkTargetMismatch status: {}",
        item_negative.status,
        error_negative.status
    );
    assert_missing_variant(&item_negative, "Symlink", "FilesystemItem");
    assert_missing_variant(&error_negative, "SymlinkTargetMismatch", "SemanticError");
}
