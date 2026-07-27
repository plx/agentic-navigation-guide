#[path = "support/assert_cli.rs"]
mod assert_cli;
#[path = "support/process_cli.rs"]
mod process_cli;
#[path = "support/environment.rs"]
mod test_environment;

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, ExprMethodCall, ExprPath, Item, ItemFn, LitStr, Macro, UseTree};
use walkdir::WalkDir;

use assert_cli::{assert_cli_command, HermeticAssertCommand};
use process_cli::{process_cli_command, HermeticProcessCommand};
use test_environment::GUIDE_ENVIRONMENT_VARIABLES;

fn removed_environment<'a, I>(environment: I) -> BTreeSet<OsString>
where
    I: Iterator<Item = (&'a OsStr, Option<&'a OsStr>)>,
{
    environment
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| name.to_owned())
        .collect()
}

fn assert_isolated_root(root: Option<&Path>, label: &str) -> PathBuf {
    let root = root.unwrap_or_else(|| panic!("{label} inherits the process current directory"));
    assert_ne!(
        root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "{label} still uses the repository checkout"
    );
    assert!(
        root.is_dir(),
        "{label} root does not exist: {}",
        root.display()
    );
    root.to_path_buf()
}

fn assert_environment_removed(removed: &BTreeSet<OsString>, label: &str) {
    for variable in GUIDE_ENVIRONMENT_VARIABLES {
        assert!(
            removed.contains(OsStr::new(variable)),
            "{label} inherits configuration variable {variable}"
        );
    }
}

#[test]
fn issue_58_assert_cli_harness_is_hermetic_and_cleans_its_default_root() {
    let command: HermeticAssertCommand = assert_cli_command();
    let root = assert_isolated_root(command.get_current_dir(), "assert CLI harness");
    let removed = removed_environment(command.get_envs());
    assert_environment_removed(&removed, "assert CLI harness");

    drop(command);
    assert!(
        !root.exists(),
        "assert CLI harness did not clean its default root: {}",
        root.display()
    );
}

#[test]
fn issue_58_process_cli_harness_is_hermetic_and_cleans_its_default_root() {
    let command: HermeticProcessCommand = process_cli_command();
    let root = assert_isolated_root(command.get_current_dir(), "process CLI harness");
    let removed = removed_environment(command.get_envs());
    assert_environment_removed(&removed, "process CLI harness");

    drop(command);
    assert!(
        !root.exists(),
        "process CLI harness did not clean its default root: {}",
        root.display()
    );
}

fn repository_file(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("read {path}: {error}"))
}

fn find_named_function<'a>(items: &'a [Item], name: &str) -> Option<&'a ItemFn> {
    items.iter().find_map(|item| match item {
        Item::Fn(function) if function.sig.ident == name => Some(function),
        Item::Mod(module) => module
            .content
            .as_ref()
            .and_then(|(_, items)| find_named_function(items, name)),
        _ => None,
    })
}

fn named_function<'a>(file: &'a syn::File, name: &str) -> &'a ItemFn {
    find_named_function(&file.items, name).unwrap_or_else(|| panic!("missing function {name}"))
}

#[derive(Default)]
struct FunctionCallInventory {
    string_method_arguments: BTreeSet<(String, String)>,
    associated_calls: BTreeSet<(String, String)>,
    associated_paths: BTreeSet<(String, String)>,
    macro_tokens: BTreeSet<String>,
    string_literals: BTreeSet<String>,
    explicit_root_current_dir: bool,
}

impl<'ast> Visit<'ast> for FunctionCallInventory {
    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        if let Some(Expr::Lit(argument)) = call.args.first() {
            if let syn::Lit::Str(value) = &argument.lit {
                self.string_method_arguments
                    .insert((call.method.to_string(), value.value()));
            }
        }
        if call.method == "current_dir" {
            if let Some(Expr::MethodCall(argument)) = call.args.first() {
                self.explicit_root_current_dir = argument.method == "path"
                    && matches!(
                        argument.receiver.as_ref(),
                        Expr::Path(path)
                            if path.path.segments.last().is_some_and(|segment| segment.ident == "root")
                    );
            }
        }
        visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if let Expr::Path(path) = call.func.as_ref() {
            let mut segments = path.path.segments.iter().rev();
            if let (Some(function), Some(owner)) = (segments.next(), segments.next()) {
                self.associated_calls
                    .insert((owner.ident.to_string(), function.ident.to_string()));
            }
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_path(&mut self, path: &'ast ExprPath) {
        let mut segments = path.path.segments.iter().rev();
        if let Some(last) = segments.next() {
            if let Some(owner) = segments.next() {
                self.associated_paths
                    .insert((owner.ident.to_string(), last.ident.to_string()));
            }
        }
        visit::visit_expr_path(self, path);
    }

    fn visit_macro(&mut self, item: &'ast Macro) {
        self.macro_tokens.insert(item.tokens.to_string());
        visit::visit_macro(self, item);
    }

    fn visit_lit_str(&mut self, literal: &'ast LitStr) {
        self.string_literals.insert(literal.value());
        visit::visit_lit_str(self, literal);
    }
}

fn function_calls(function: &ItemFn) -> FunctionCallInventory {
    let mut inventory = FunctionCallInventory::default();
    inventory.visit_item_fn(function);
    inventory
}

const GLOBAL_MUTATION_FUNCTIONS: &[&str] = &["set_current_dir", "set_var", "remove_var"];

fn collect_environment_aliases(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    call_names: &mut BTreeSet<String>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_environment_aliases(&path.tree, prefix, call_names);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let mut full_path = prefix.clone();
            full_path.push(name.ident.to_string());
            if full_path.len() == 3
                && full_path[0] == "std"
                && full_path[1] == "env"
                && GLOBAL_MUTATION_FUNCTIONS.contains(&full_path[2].as_str())
            {
                call_names.insert(name.ident.to_string());
            }
        }
        UseTree::Rename(rename) => {
            let mut full_path = prefix.clone();
            full_path.push(rename.ident.to_string());
            if full_path.len() == 3
                && full_path[0] == "std"
                && full_path[1] == "env"
                && GLOBAL_MUTATION_FUNCTIONS.contains(&full_path[2].as_str())
            {
                call_names.insert(rename.rename.to_string());
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_environment_aliases(item, prefix, call_names);
            }
        }
        UseTree::Glob(_) => {}
    }
}

struct EnvironmentAliasVisitor<'a> {
    call_names: &'a mut BTreeSet<String>,
}

impl<'ast> Visit<'ast> for EnvironmentAliasVisitor<'_> {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        collect_environment_aliases(&item.tree, &mut Vec::new(), self.call_names);
        visit::visit_item_use(self, item);
    }
}

struct GlobalMutationVisitor<'a> {
    call_names: &'a BTreeSet<String>,
    calls: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for GlobalMutationVisitor<'_> {
    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if let Expr::Path(path) = call.func.as_ref() {
            if let Some(function) = path.path.segments.last() {
                let name = function.ident.to_string();
                if self.call_names.contains(&name) {
                    self.calls.insert(name);
                }
            }
        }
        visit::visit_expr_call(self, call);
    }
}

fn process_global_mutation_calls(source: &str, path: &Path) -> BTreeSet<String> {
    let file =
        syn::parse_file(source).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    let mut call_names: BTreeSet<String> = GLOBAL_MUTATION_FUNCTIONS
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    {
        let mut aliases = EnvironmentAliasVisitor {
            call_names: &mut call_names,
        };
        aliases.visit_file(&file);
    }
    let mut visitor = GlobalMutationVisitor {
        call_names: &call_names,
        calls: BTreeSet::new(),
    };
    visitor.visit_file(&file);
    visitor.calls
}

#[test]
fn issue_58_global_mutation_gate_covers_qualified_imported_and_renamed_calls() {
    let cases = [
        (
            "fn probe() { std::env::set_var(\"NAME\", \"value\"); }",
            "set_var",
        ),
        (
            "use std::env; fn probe() { env::remove_var(\"NAME\"); }",
            "remove_var",
        ),
        (
            "use std::env::set_current_dir; fn probe() { set_current_dir(\"root\"); }",
            "set_current_dir",
        ),
        (
            "use std::{env::set_var as mutate_environment}; \
             fn probe() { mutate_environment(\"NAME\", \"value\"); }",
            "mutate_environment",
        ),
        (
            "mod nested { use std::env::remove_var as clear_environment; \
             fn probe() { clear_environment(\"NAME\"); } }",
            "clear_environment",
        ),
    ];
    for (source, expected) in cases {
        let calls = process_global_mutation_calls(source, Path::new("fixed-mutation-case.rs"));
        assert_eq!(
            calls,
            BTreeSet::from([expected.to_string()]),
            "the syntax gate missed {expected}"
        );
    }

    let child_only =
        "fn probe(command: &mut std::process::Command) { command.env_remove(\"NAME\"); }";
    assert!(
        process_global_mutation_calls(child_only, Path::new("fixed-child-case.rs")).is_empty(),
        "the syntax gate rejected child-only environment control"
    );
}

#[test]
fn issue_58_subprocess_inventory_is_explicit_and_process_global_state_is_untouched() {
    let required_harnesses = [
        ("tests/cli_tests.rs", "support/assert_cli.rs"),
        ("tests/environment_precedence.rs", "support/assert_cli.rs"),
        (
            "tests/issue_101_parent_explicit_guide.rs",
            "support/assert_cli.rs",
        ),
        (
            "tests/issue_47_output_contract.rs",
            "support/process_cli.rs",
        ),
        (
            "tests/issue_68_normative_source.rs",
            "support/process_cli.rs",
        ),
    ];
    for (path, harness) in required_harnesses {
        assert!(
            repository_file(path).contains(harness),
            "{path} does not use the hermetic subprocess harness {harness}"
        );
    }

    let cli_tests = repository_file("tests/cli_tests.rs");
    let cli_file = syn::parse_file(&cli_tests).expect("parse CLI integration tests");
    let init = function_calls(named_function(&cli_file, "test_init_command"));
    assert!(
        init.string_method_arguments
            .contains(&("arg".to_string(), "--root".to_string()))
            && init
                .associated_calls
                .contains(&("TempDir".to_string(), "new".to_string())),
        "the original init regression can read outside its explicit temporary root"
    );
    let default_root = function_calls(named_function(
        &cli_file,
        "issue_58_product_current_directory_default_is_covered_explicitly",
    ));
    assert!(
        default_root.explicit_root_current_dir,
        "the intentional product current-directory default lacks an explicit fixture"
    );

    for entry in WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")) {
        let entry = entry.expect("walk integration tests");
        if !entry.file_type().is_file() || entry.path().extension() != Some(OsStr::new("rs")) {
            continue;
        }
        let source = fs::read_to_string(entry.path())
            .unwrap_or_else(|error| panic!("read {}: {error}", entry.path().display()));
        let mutations = process_global_mutation_calls(&source, entry.path());
        assert!(
            mutations.is_empty(),
            "{} mutates process-global state with {}",
            entry.path().display(),
            mutations.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
}

#[test]
fn issue_58_transient_behavior_and_empty_supported_facade_have_executable_owners() {
    let dumper = repository_file("src/dumper.rs");
    let dumper_file = syn::parse_file(&dumper).expect("parse dumper unit tests");
    let transient = function_calls(named_function(
        &dumper_file,
        "issue_42_transient_classification_failure_aborts_collection",
    ));
    assert!(
        transient
            .associated_paths
            .contains(&("ErrorKind".to_string(), "NotFound".to_string())),
        "the deterministic transient-entry product test omits ErrorKind::NotFound"
    );
    assert!(
        transient
            .macro_tokens
            .iter()
            .any(|tokens| tokens
                .contains("a transient classification failure must abort collection")),
        "the deterministic transient-entry product test omits its fail-closed assertion"
    );
    assert!(
        transient
            .string_literals
            .contains("ISSUE42_CLASSIFIER_INTERNAL_SENTINEL")
            && transient
                .macro_tokens
                .iter()
                .any(|tokens| tokens.contains("ISSUE42_CLASSIFIER_INTERNAL_SENTINEL")),
        "the deterministic transient-entry product test omits injection or redaction of the internal detail"
    );

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !root.join("src/lib.rs").exists() && !repository_file("Cargo.toml").contains("\n[lib]\n"),
        "the supported Rust facade set is not empty"
    );
    let ci = repository_file(".github/workflows/ci.yml");
    for required in [
        "cargo test --locked --test issue_58_test_hermeticity -- --nocapture",
        "--test issue_54_binary_only_package",
        "issue_62_exact_package_installs_smokes_and_rejects_library_consumers",
        "-- --exact --ignored --nocapture",
    ] {
        assert!(
            ci.contains(required),
            "CI does not execute the binary-only boundary proof {required:?}"
        );
    }
}
