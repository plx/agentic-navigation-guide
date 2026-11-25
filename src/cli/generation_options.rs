//! `--indent` and `--depth` value parsing.
//!
//! The bounds are the verified generation kernel's own constants, so the CLI
//! and `Dumper` configuration accept the same ranges. When both options are
//! out of range, clap's argument order decides which error is reported; the
//! kernel's indent-before-depth precedence (`theorem_configuration_table`)
//! governs programmatic `Dumper` configuration.

use crate::proven::generation::{
    parse_unsigned_decimal, within_bounds, MAX_INDENT_SIZE, MAX_LOGICAL_DEPTH, MIN_INDENT_SIZE,
    MIN_LOGICAL_DEPTH,
};

pub(super) fn parse_indent(value: &str) -> Result<usize, String> {
    parse_bounded(value, MIN_INDENT_SIZE, MAX_INDENT_SIZE, "indent size")
}

pub(super) fn parse_depth(value: &str) -> Result<usize, String> {
    parse_bounded(value, MIN_LOGICAL_DEPTH, MAX_LOGICAL_DEPTH, "maximum depth")
}

/// Parse a bounded option value. The verified kernels accept exactly the
/// text `str::parse::<usize>` accepts and then decide the inclusive range;
/// this glue maps each rejection to its diagnostic.
fn parse_bounded(
    value: &str,
    minimum: usize,
    maximum: usize,
    label: &str,
) -> Result<usize, String> {
    let Some(parsed) = parse_unsigned_decimal(value) else {
        return Err(format!(
            "{label} must be an integer from {minimum} through {maximum}"
        ));
    };
    if !within_bounds(parsed, minimum, maximum) {
        return Err(format!("{label} must be from {minimum} through {maximum}"));
    }

    Ok(parsed)
}
