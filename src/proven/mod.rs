//! Formally verified kernels.
//!
//! Every function in these modules that carries a
//! `#[cfg_attr(verus_keep_ghost, verus_spec(...))]` attribute is checked by
//! [Verus](https://github.com/verus-lang/verus) against the specification in
//! that attribute: functional correctness against the stated contract,
//! absence of panics, arithmetic overflow, and out-of-bounds indexing, and
//! termination. Specifications and proofs exist only when the code is
//! compiled by Verus (`cfg(verus_keep_ghost)`); ordinary `rustc` builds see
//! plain Rust and the product gains no dependency. See `proofs/README.md` for
//! the trusted base and how to re-run the proofs.

pub(crate) mod anchor;
pub(crate) mod authority;
pub(crate) mod choice;
pub(crate) mod discovery;
pub(crate) mod document;
pub(crate) mod entry;
pub(crate) mod generation;
pub(crate) mod glob;
pub(crate) mod hierarchy;
pub(crate) mod layout;
pub(crate) mod lines;
pub(crate) mod marker;
pub(crate) mod outcome;
pub(crate) mod output_path;
pub(crate) mod path_rules;
pub(crate) mod placeholder;
pub(crate) mod settings;
pub(crate) mod tree_rules;
pub(crate) mod windows_names;
