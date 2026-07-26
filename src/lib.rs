//! # Agentic Navigation Guide
//!
//! Legacy Rust surface for verifying hand-written navigation guides.
//!
//! The installed CLI is the sole supported v0.2 product. This linkable
//! `0.1.4` library remains temporarily so the audited legacy exports can be
//! removed or made private by focused migration issues; it is not a supported
//! v0.2 integration facade. New Rust consumers should not depend on it.
//!
//! The legacy surface currently provides functionality to:
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
mod guide_input;
pub mod parser;
mod path_codec;
pub mod recursive;
pub mod types;
pub mod validator;
pub mod verifier;

pub use dumper::Dumper;
pub use errors::{AppError, Result, SemanticError, SyntaxError};
pub use parser::Parser;
pub use recursive::{find_guides, verify_guides, GuideLocation, GuideVerificationResult};
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
