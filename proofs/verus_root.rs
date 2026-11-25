//! Verus verification root for the formally verified kernels.
//!
//! This file is not part of the Cargo package. It re-includes the product
//! modules at their real source paths so that Verus checks exactly the code
//! that `cargo build` compiles. Run `proofs/run.sh` (or `just verify-proofs`).

#![feature(proc_macro_hygiene)]
#![allow(dead_code, unused_imports)]

#[path = "../src/entry_type.rs"]
mod entry_type;
#[path = "../src/path_codec.rs"]
mod path_codec;
#[path = "../src/proven/mod.rs"]
mod proven;

// Proof-only theorems composed from the kernels' specifications; this file
// is not part of the Cargo package.
#[path = "round_trip.rs"]
mod round_trip;
