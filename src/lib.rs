//! # Agentic Navigation Guide
//!
//! A library for verifying hand-written navigation guides against filesystem structure.
//!
//! This library provides functionality to:
//! - Parse navigation guides from markdown files
//! - Validate syntax of navigation guides
//! - Verify guides against actual filesystem state
//! - Generate navigation guides from directory structures

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

pub mod dumper;
pub mod errors;
pub mod parser;
pub mod types;
pub mod validator;
pub mod verifier;

pub use dumper::Dumper;
pub use errors::{AppError, Result, SemanticError, SyntaxError};
pub use parser::Parser;
pub use types::{ExecutionMode, FilesystemItem, LogLevel, NavigationGuide, NavigationGuideLine};
pub use validator::Validator;
pub use verifier::Verifier;

/// Parse a navigation guide from markdown content
pub fn parse_navigation_guide(content: &str) -> Result<NavigationGuide> {
    Parser::new().parse(content)
}

/// Check if a navigation guide has valid syntax
pub fn check_syntax(guide: &NavigationGuide) -> Result<()> {
    Validator::new().validate_syntax(guide)
}

/// Verify a navigation guide against the filesystem
pub fn verify_guide(guide: &NavigationGuide, root_path: &std::path::Path) -> Result<()> {
    Verifier::new(root_path).verify(guide)
}

/// Dump a directory structure as a navigation guide
pub fn dump_directory(
    root_path: &std::path::Path,
    max_depth: Option<usize>,
    exclude_patterns: &[String],
    indent_size: usize,
) -> Result<String> {
    Dumper::new(root_path)
        .with_max_depth(max_depth)
        .with_exclude_patterns(exclude_patterns)?
        .with_indent_size(indent_size)
        .dump()
}
