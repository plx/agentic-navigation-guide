const MIN_INDENT_SIZE: usize = 1;
const MAX_INDENT_SIZE: usize = 16;
const MIN_LOGICAL_DEPTH: usize = 0;
const MAX_LOGICAL_DEPTH: usize = 256;

pub(super) fn parse_indent(value: &str) -> Result<usize, String> {
    parse_bounded(value, MIN_INDENT_SIZE, MAX_INDENT_SIZE, "indent size")
}

pub(super) fn parse_depth(value: &str) -> Result<usize, String> {
    parse_bounded(value, MIN_LOGICAL_DEPTH, MAX_LOGICAL_DEPTH, "maximum depth")
}

fn parse_bounded(
    value: &str,
    minimum: usize,
    maximum: usize,
    label: &str,
) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{label} must be an integer from {minimum} through {maximum}"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!("{label} must be from {minimum} through {maximum}"));
    }

    Ok(parsed)
}
