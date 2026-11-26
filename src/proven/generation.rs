//! Verified guide-generation decisions and line formatting.
//!
//! These kernels make the pure decisions of `dump` and `init`
//! (`docs/v0.2-contract.md`, "Empty guides and generated output",
//! "Filesystem-name representation", and "Complete argument ledger"):
//!
//! - [`parse_unsigned_decimal`] and [`within_bounds`]: the `--depth` and
//!   `--indent` option parser accepts exactly the text `str::parse::<usize>`
//!   accepts (an optional `+` and one or more ASCII digits whose value fits
//!   in `usize`), and then the inclusive range;
//! - [`generation_configuration`]: an indent size outside `1..=16` is
//!   rejected first, then an explicit maximum depth above 256; an explicit
//!   depth `d` limits entries to entry depth `d + 1` (logical depth `d`) and
//!   truncates below it, while no depth allows entry depth 257 (logical depth
//!   256) and fails closed beyond it;
//! - [`child_depth`] and [`descends_into`]: the depth of an included child
//!   and whether a directory is descended;
//! - [`host_name_verdict`]: a directory-entry name is rejected when it
//!   contains a forbidden control, and at the generation root when it starts
//!   with `\` or a drive prefix (shared with the verifier's snapshots);
//! - [`format_entry_line`]: one generated guide line, exactly
//!   `entry_line_spec`: `depth * indent` spaces, `- `, the serialized name,
//!   `/` for a directory, and a line feed, or `None` when the indentation
//!   width overflows.
//!
//! The theorems below state the contract-level consequences:
//!
//! - `theorem_configuration_table`: which configurations are rejected, in
//!   which precedence, and the exact limit of every accepted one;
//! - `theorem_explicit_depth_is_exact`: with an explicit depth `d`, no
//!   enumerated entry is ever beyond the limit, every emitted entry has
//!   logical depth at most `d`, every included directory above logical depth
//!   `d` is descended, and none at logical depth `d` is;
//! - `theorem_default_depth_fails_closed`: without an explicit depth, every
//!   included directory is descended and an entry beyond logical depth 256
//!   is rejected, so generation never silently truncates;
//! - `theorem_entry_line_layout`: a generated line is its indentation, `- `,
//!   the serialized name, the optional `/`, and exactly one final line feed;
//! - `theorem_representable_names_are_valid_paths`: a representable entry name
//!   that is nonempty, not `.` or `..`, and free of `/` (as every
//!   directory-entry name is) is accepted by the validator's structural path
//!   rules at the same level;
//! - `theorem_parse_is_decimal`: an accepted option value is the decimal
//!   value of its digits, and leading zeros do not change it.

use crate::path_codec::{contains_forbidden_control, has_windows_drive_prefix};
#[cfg(verus_keep_ghost)]
use crate::path_codec::{has_drive_prefix, has_forbidden_control};
#[cfg(verus_keep_ghost)]
use crate::proven::path_rules::{
    component_verdict, components_verdict, is_dot_component, path_verdict, split_components,
    PathVerdict,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

/// Smallest supported indent size.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const MIN_INDENT_SIZE: usize = 1;
/// Largest supported indent size.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const MAX_INDENT_SIZE: usize = 16;
/// Smallest supported `--depth`.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const MIN_LOGICAL_DEPTH: usize = 0;
/// Largest supported logical depth.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const MAX_LOGICAL_DEPTH: usize = 256;

/// The entry-depth limit of one generation traversal.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TraversalLimit {
    /// The largest entry depth (number of path components) that is included.
    pub(crate) maximum_entry_depth: usize,
    /// Whether an included entry beyond the limit is an error (no explicit
    /// depth) rather than never enumerated (explicit depth).
    pub(crate) reject_beyond_limit: bool,
}

/// The decision about a generation configuration.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GenerationConfiguration {
    /// The indent size is outside `1..=16`.
    IndentOutOfRange,
    /// The explicit maximum depth is above 256.
    DepthOutOfRange { depth: usize },
    /// The configuration is valid with this traversal limit.
    Limit(TraversalLimit),
}

/// The depth decision for an included child entry.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChildDepth {
    /// The child depth overflows `usize`.
    Unrepresentable,
    /// The child is beyond the maximum entry depth.
    BeyondLimit,
    /// The child is included at this entry depth.
    Included(usize),
}

/// The decision about one directory-entry name.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostNameVerdict {
    /// The name can be represented in a guide.
    Representable,
    /// The name contains a forbidden control.
    ControlBearing,
    /// A root-level name starts with `\` or a drive prefix.
    RootedOrDrivePrefixed,
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Option parsing
// ---------------------------------------------------------------------------

/// ASCII `0`-`9`.
pub(crate) open spec fn is_decimal_digit(c: char) -> bool {
    '0' <= c && c <= '9'
}

/// The value of a decimal digit (zero for any other scalar).
pub(crate) open spec fn digit_value(c: char) -> nat {
    if is_decimal_digit(c) {
        (c as int - '0' as int) as nat
    } else {
        0
    }
}

/// The value of a sequence of decimal digits, most significant first.
pub(crate) open spec fn decimal_value(s: Seq<char>) -> nat
    decreases s.len(),
{
    if s.len() == 0 {
        0
    } else {
        decimal_value(s.drop_last()) * 10 + digit_value(s.last())
    }
}

/// Every scalar is a decimal digit.
pub(crate) open spec fn all_decimal_digits(s: Seq<char>) -> bool {
    forall|i: int| 0 <= i < s.len() ==> #[trigger] is_decimal_digit(s[i])
}

/// The text after an optional leading `+`.
pub(crate) open spec fn unsigned_digits(s: Seq<char>) -> Seq<char> {
    if s.len() > 0 && s[0] == '+' {
        s.drop_first()
    } else {
        s
    }
}

/// `str::parse::<usize>`: an optional `+` and one or more ASCII digits whose
/// value fits in `usize`.
pub(crate) open spec fn parse_usize_spec(s: Seq<char>) -> Option<usize> {
    let digits = unsigned_digits(s);
    if digits.len() > 0 && all_decimal_digits(digits) && decimal_value(digits) <= usize::MAX {
        Some(decimal_value(digits) as usize)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Configuration and depth limits
// ---------------------------------------------------------------------------

/// The configuration decision: indent first, then the explicit depth.
pub(crate) open spec fn configuration_spec(
    indent_size: usize,
    max_depth: Option<usize>,
) -> GenerationConfiguration {
    if indent_size < 1 || indent_size > 16 {
        GenerationConfiguration::IndentOutOfRange
    } else {
        match max_depth {
            Some(d) => if d <= 256 {
                GenerationConfiguration::Limit(
                    TraversalLimit { maximum_entry_depth: (d + 1) as usize, reject_beyond_limit: false },
                )
            } else {
                GenerationConfiguration::DepthOutOfRange { depth: d }
            },
            None => GenerationConfiguration::Limit(
                TraversalLimit { maximum_entry_depth: 257, reject_beyond_limit: true },
            ),
        }
    }
}

/// The configuration is accepted.
pub(crate) open spec fn is_limit(c: GenerationConfiguration) -> bool {
    match c {
        GenerationConfiguration::Limit(_) => true,
        _ => false,
    }
}

/// The explicit depth is rejected.
pub(crate) open spec fn is_depth_out_of_range(c: GenerationConfiguration) -> bool {
    match c {
        GenerationConfiguration::DepthOutOfRange { .. } => true,
        _ => false,
    }
}

/// The traversal limit of an accepted configuration.
pub(crate) open spec fn limit_of(c: GenerationConfiguration) -> TraversalLimit {
    match c {
        GenerationConfiguration::Limit(limit) => limit,
        _ => TraversalLimit { maximum_entry_depth: 0, reject_beyond_limit: false },
    }
}

/// The entry depth of an included child (zero otherwise).
pub(crate) open spec fn included_depth(c: ChildDepth) -> int {
    match c {
        ChildDepth::Included(depth) => depth as int,
        _ => 0,
    }
}

/// The depth decision for a child of a directory at entry depth `parent`.
pub(crate) open spec fn child_depth_spec(parent: usize, limit: TraversalLimit) -> ChildDepth {
    if parent + 1 > usize::MAX {
        ChildDepth::Unrepresentable
    } else if parent + 1 > limit.maximum_entry_depth {
        ChildDepth::BeyondLimit
    } else {
        ChildDepth::Included((parent + 1) as usize)
    }
}

/// An included entry at entry depth `depth` is descended.
pub(crate) open spec fn descends_spec(depth: usize, is_directory: bool, limit: TraversalLimit) -> bool {
    is_directory && (depth < limit.maximum_entry_depth || limit.reject_beyond_limit)
}

/// The traversal enumerates the children of a directory at entry depth
/// `depth`: the root (depth 0), or an included directory whose parent the
/// traversal enumerates and which it descends.
pub(crate) open spec fn enumerated(depth: int, limit: TraversalLimit) -> bool
    decreases depth,
{
    if depth <= 0 {
        depth == 0
    } else {
        &&& depth <= usize::MAX
        &&& enumerated(depth - 1, limit)
        &&& child_depth_spec((depth - 1) as usize, limit) == ChildDepth::Included(depth as usize)
        &&& descends_spec(depth as usize, true, limit)
    }
}

// ---------------------------------------------------------------------------
// Names and lines
// ---------------------------------------------------------------------------

/// The name decision: a forbidden control first, then (at the root) a
/// leading `\` or a drive prefix.
pub(crate) open spec fn host_name_verdict_spec(s: Seq<char>, at_root: bool) -> HostNameVerdict {
    if has_forbidden_control(s) {
        HostNameVerdict::ControlBearing
    } else if at_root && ((s.len() > 0 && s[0] == '\\') || has_drive_prefix(s)) {
        HostNameVerdict::RootedOrDrivePrefixed
    } else {
        HostNameVerdict::Representable
    }
}

/// `n` spaces.
pub(crate) open spec fn spaces(n: nat) -> Seq<char> {
    Seq::new(n, |_i: int| ' ')
}

/// The optional directory marker.
pub(crate) open spec fn directory_marker(is_dir: bool) -> Seq<char> {
    if is_dir {
        seq!['/']
    } else {
        Seq::<char>::empty()
    }
}

/// One generated guide line: `depth * indent` spaces, `- `, the serialized
/// name, `/` for a directory, and a line feed.
pub(crate) open spec fn entry_line_spec(depth: nat, indent: nat, name: Seq<char>, is_dir: bool) -> Seq<char> {
    spaces(depth * indent) + seq!['-', ' '] + name + directory_marker(is_dir) + seq!['\n']
}

/// The scalar view of an optional generated line.
pub(crate) open spec fn line_view(r: Option<String>) -> Option<Seq<char>> {
    match r {
        Some(s) => Some(s@),
        None => None,
    }
}

} // verus!

/// Parse an option value exactly as `str::parse::<usize>` does: an optional
/// `+` followed by one or more ASCII digits whose value fits in `usize`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == parse_usize_spec(value@),
))]
pub(crate) fn parse_unsigned_decimal(value: &str) -> Option<usize> {
    let mut chars = value.chars();
    let mut at_start = true;
    let mut seen_digit = false;
    let mut parsed: usize = 0;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
        let ghost mut signed: bool = false;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= value@.len(),
            chars.remaining() == value@.skip(index),
            at_start == (index == 0),
            signed ==> index >= 1 && value@[0] == '+',
            !signed && index >= 1 ==> value@[0] != '+',
            seen_digit == (index > (if signed { 1int } else { 0int })),
            all_decimal_digits(value@.subrange(if signed { 1int } else { 0int }, index)),
            parsed as nat == decimal_value(value@.subrange(if signed { 1int } else { 0int }, index)),
        decreases value@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                // Only a first `+` is a sign; every other scalar must be a
                // digit.
                let leading_sign = at_start && ch == '+';
                if !leading_sign {
                    if !ch.is_ascii_digit() {
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            lemma_unsigned_digits(value@, signed);
                            let start = if signed { 1int } else { 0int };
                            assert(unsigned_digits(value@)[index - start] == ch);
                            assert(!is_decimal_digit(unsigned_digits(value@)[index - start]));
                        }
                        return None;
                    }
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        let start = if signed { 1int } else { 0int };
                        let next = value@.subrange(start, index + 1);
                        assert(next.drop_last() =~= value@.subrange(start, index));
                        assert(next.last() == ch);
                        assert(all_decimal_digits(next));
                        if all_decimal_digits(unsigned_digits(value@)) {
                            lemma_unsigned_digits(value@, signed);
                            lemma_decimal_monotone(unsigned_digits(value@), index + 1 - start);
                            assert(unsigned_digits(value@).take(index + 1 - start) =~= next);
                        }
                    }
                    let digit = (ch as u32 - '0' as u32) as usize;
                    parsed = parsed.checked_mul(10)?.checked_add(digit)?;
                    seen_digit = true;
                }
                at_start = false;
                #[cfg(verus_keep_ghost)]
                proof! {
                    if leading_sign {
                        signed = true;
                    }
                    index = index + 1;
                    let start = if signed { 1int } else { 0int };
                    if signed && index == 1 {
                        assert(value@.subrange(start, index) =~= Seq::<char>::empty());
                    }
                }
            }
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_unsigned_digits(value@, signed);
                    let start = if signed { 1int } else { 0int };
                    assert(value@.subrange(start, index) =~= unsigned_digits(value@));
                }
                if seen_digit {
                    return Some(parsed);
                }
                return None;
            }
        }
    }
}

/// Return whether `minimum <= value <= maximum`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == (minimum <= value && value <= maximum),
))]
pub(crate) fn within_bounds(value: usize, minimum: usize, maximum: usize) -> bool {
    minimum <= value && value <= maximum
}

/// Decide a generation configuration: an indent size outside `1..=16` first,
/// then an explicit maximum depth above 256, and otherwise the traversal
/// limit.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == configuration_spec(indent_size, max_depth),
))]
pub(crate) fn generation_configuration(
    indent_size: usize,
    max_depth: Option<usize>,
) -> GenerationConfiguration {
    if !(MIN_INDENT_SIZE..=MAX_INDENT_SIZE).contains(&indent_size) {
        return GenerationConfiguration::IndentOutOfRange;
    }
    match max_depth {
        Some(depth) if depth <= MAX_LOGICAL_DEPTH => {
            GenerationConfiguration::Limit(TraversalLimit {
                maximum_entry_depth: depth + 1,
                reject_beyond_limit: false,
            })
        }
        Some(depth) => GenerationConfiguration::DepthOutOfRange { depth },
        None => GenerationConfiguration::Limit(TraversalLimit {
            maximum_entry_depth: MAX_LOGICAL_DEPTH + 1,
            reject_beyond_limit: true,
        }),
    }
}

/// Decide the entry depth of an included child of a directory at entry
/// depth `parent_depth`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == child_depth_spec(parent_depth, limit),
))]
pub(crate) fn child_depth(parent_depth: usize, limit: TraversalLimit) -> ChildDepth {
    let depth = match parent_depth.checked_add(1) {
        Some(depth) => depth,
        None => return ChildDepth::Unrepresentable,
    };
    if depth > limit.maximum_entry_depth {
        return ChildDepth::BeyondLimit;
    }
    ChildDepth::Included(depth)
}

/// Return whether an included entry at entry depth `depth` is descended.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == descends_spec(depth, is_directory, limit),
))]
pub(crate) fn descends_into(depth: usize, is_directory: bool, limit: TraversalLimit) -> bool {
    is_directory && (depth < limit.maximum_entry_depth || limit.reject_beyond_limit)
}

/// Return whether the first scalar of `name` is `\`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == (name@.len() > 0 && name@[0] == '\\'),
))]
fn starts_with_backslash(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first == '\\',
        None => false,
    }
}

/// Decide whether a directory-entry name is representable: a forbidden
/// control first, then (at the root) a leading `\` or a drive prefix.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == host_name_verdict_spec(name@, at_root),
))]
pub(crate) fn host_name_verdict(name: &str, at_root: bool) -> HostNameVerdict {
    if contains_forbidden_control(name) {
        return HostNameVerdict::ControlBearing;
    }
    if at_root && (starts_with_backslash(name) || has_windows_drive_prefix(name)) {
        return HostNameVerdict::RootedOrDrivePrefixed;
    }
    HostNameVerdict::Representable
}

/// Format one generated guide line: `depth * indent_size` spaces, `- `, the
/// serialized name, `/` for a directory, and a line feed; `None` when the
/// indentation width overflows `usize`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        (r is None) == (depth * indent_size > usize::MAX),
        r is Some ==> line_view(r) == Some(
            entry_line_spec(depth as nat, indent_size as nat, serialized_name@, is_dir),
        ),
))]
pub(crate) fn format_entry_line(
    depth: usize,
    indent_size: usize,
    serialized_name: &str,
    is_dir: bool,
) -> Option<String> {
    let width = depth.checked_mul(indent_size)?;
    let mut line = String::new();
    let mut count: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            count <= width,
            width == depth * indent_size,
            line@ == spaces(count as nat),
        decreases width - count
    ))]
    while count < width {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(spaces(count as nat).push(' ') =~= spaces((count + 1) as nat));
        }
        line.push(' ');
        count += 1;
    }
    line.push('-');
    line.push(' ');
    line.push_str(serialized_name);
    if is_dir {
        line.push('/');
    }
    line.push('\n');
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(line@ =~= entry_line_spec(depth as nat, indent_size as nat, serialized_name@, is_dir));
    }
    Some(line)
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Supporting lemmas
// ---------------------------------------------------------------------------

/// `unsigned_digits` drops exactly the leading `+` the parser recorded.
proof fn lemma_unsigned_digits(s: Seq<char>, signed: bool)
    requires
        signed ==> s.len() >= 1 && s[0] == '+',
        !signed && s.len() >= 1 ==> s[0] != '+',
    ensures
        unsigned_digits(s) == s.subrange(if signed { 1int } else { 0int }, s.len() as int),
{
    if signed {
        assert(s.drop_first() =~= s.subrange(1, s.len() as int));
    } else {
        assert(s =~= s.subrange(0, s.len() as int));
    }
}

/// Appending digits never decreases a decimal value.
proof fn lemma_decimal_monotone(s: Seq<char>, k: int)
    requires
        0 <= k <= s.len(),
    ensures
        decimal_value(s.take(k)) <= decimal_value(s),
    decreases s.len(),
{
    if k < s.len() {
        lemma_decimal_monotone(s.drop_last(), k);
        assert(s.drop_last().take(k) =~= s.take(k));
    } else {
        assert(s.take(k) =~= s);
    }
}

/// A leading zero does not change a decimal value.
proof fn lemma_leading_zero(s: Seq<char>)
    ensures
        decimal_value(seq!['0'] + s) == decimal_value(s),
    decreases s.len(),
{
    if s.len() == 0 {
        assert((seq!['0'] + s).drop_last() =~= Seq::<char>::empty());
        assert((seq!['0'] + s).last() == '0');
        assert(decimal_value(Seq::<char>::empty()) == 0);
        assert(digit_value('0') == 0);
    } else {
        lemma_leading_zero(s.drop_last());
        assert((seq!['0'] + s).drop_last() =~= seq!['0'] + s.drop_last());
        assert((seq!['0'] + s).last() == s.last());
    }
}

/// A `/`-free sequence splits into exactly itself.
proof fn lemma_split_single(s: Seq<char>)
    requires
        forall|i: int| 0 <= i < s.len() ==> s[i] != '/',
    ensures
        split_components(s) == seq![s],
{
    crate::proven::placeholder::lemma_split_free(s);
}

// ---------------------------------------------------------------------------
// Theorems
// ---------------------------------------------------------------------------

/// Which configurations are rejected, in which precedence, and the exact
/// limit of every accepted one.
pub(crate) proof fn theorem_configuration_table(indent_size: usize, max_depth: Option<usize>)
    ensures
        (configuration_spec(indent_size, max_depth) == GenerationConfiguration::IndentOutOfRange)
            == !(1 <= indent_size <= 16),
        1 <= indent_size <= 16 && max_depth is Some && max_depth->0 > 256 ==> configuration_spec(
            indent_size,
            max_depth,
        ) == (GenerationConfiguration::DepthOutOfRange { depth: max_depth->0 }),
        is_depth_out_of_range(configuration_spec(indent_size, max_depth)) == (1 <= indent_size <= 16
            && max_depth is Some && max_depth->0 > 256),
        1 <= indent_size <= 16 && max_depth is Some && max_depth->0 <= 256 ==> configuration_spec(
            indent_size,
            max_depth,
        ) == GenerationConfiguration::Limit(
            TraversalLimit {
                maximum_entry_depth: (max_depth->0 + 1) as usize,
                reject_beyond_limit: false,
            },
        ),
        1 <= indent_size <= 16 && max_depth is None ==> configuration_spec(indent_size, max_depth)
            == GenerationConfiguration::Limit(
            TraversalLimit { maximum_entry_depth: 257, reject_beyond_limit: true },
        ),
        is_limit(configuration_spec(indent_size, max_depth)) ==> 1 <= limit_of(configuration_spec(
            indent_size,
            max_depth,
        )).maximum_entry_depth <= 257,
{
}

/// The entry depth of an enumerated directory is at most its limit.
proof fn lemma_enumerated_bound(depth: int, limit: TraversalLimit)
    requires
        enumerated(depth, limit),
    ensures
        0 <= depth <= limit.maximum_entry_depth || depth == 0,
        depth <= usize::MAX,
    decreases depth,
{
    if depth > 0 {
        lemma_enumerated_bound(depth - 1, limit);
    }
}

/// With an explicit depth `d`, no enumerated entry is ever beyond the limit
/// (so no entry is an error), every emitted entry has logical depth (entry
/// depth minus one) at most `d`, every included directory above logical
/// depth `d` is descended, and none at logical depth `d` is.
pub(crate) proof fn theorem_explicit_depth_is_exact(
    indent_size: usize,
    d: usize,
    parent: usize,
    entry_depth: usize,
)
    requires
        1 <= indent_size <= 16,
        d <= 256,
        enumerated(parent as int, limit_of(configuration_spec(indent_size, Some(d)))),
    ensures
        is_limit(configuration_spec(indent_size, Some(d))),
        child_depth_spec(parent, limit_of(configuration_spec(indent_size, Some(d))))
            == ChildDepth::Included((parent + 1) as usize),
        parent <= d,
        1 <= entry_depth <= d ==> descends_spec(
            entry_depth,
            true,
            limit_of(configuration_spec(indent_size, Some(d))),
        ),
        !descends_spec(
            (d + 1) as usize,
            true,
            limit_of(configuration_spec(indent_size, Some(d))),
        ),
{
    let limit = limit_of(configuration_spec(indent_size, Some(d)));
    assert(limit == TraversalLimit { maximum_entry_depth: (d + 1) as usize, reject_beyond_limit: false });
    lemma_enumerated_bound(parent as int, limit);
}

/// Without an explicit depth, every included directory is descended, every
/// enumerated entry up to logical depth 256 is included, and an entry beyond
/// logical depth 256 is rejected rather than silently dropped.
pub(crate) proof fn theorem_default_depth_fails_closed(
    indent_size: usize,
    parent: usize,
    entry_depth: usize,
)
    requires
        1 <= indent_size <= 16,
    ensures
        is_limit(configuration_spec(indent_size, None)),
        descends_spec(entry_depth, true, limit_of(configuration_spec(indent_size, None))),
        parent < 257 ==> child_depth_spec(parent, limit_of(configuration_spec(indent_size, None)))
            == ChildDepth::Included((parent + 1) as usize),
        257 <= parent < usize::MAX ==> child_depth_spec(
            parent,
            limit_of(configuration_spec(indent_size, None)),
        ) == ChildDepth::BeyondLimit,
        included_depth(child_depth_spec(parent, limit_of(configuration_spec(indent_size, None))))
            - 1 <= 256,
{
}

/// A generated line is its indentation, `- `, the serialized name, the
/// optional `/`, and exactly one final line feed (when the name has none).
pub(crate) proof fn theorem_entry_line_layout(depth: nat, indent: nat, name: Seq<char>, is_dir: bool)
    ensures
        ({
            let line = entry_line_spec(depth, indent, name, is_dir);
            let w = (depth * indent) as int;
            &&& line.len() == w + 2 + name.len() + (if is_dir { 1int } else { 0int }) + 1
            &&& forall|i: int| 0 <= i < w ==> line[i] == ' '
            &&& line[w as int] == '-'
            &&& line[w + 1] == ' '
            &&& line.subrange(w + 2, w + 2 + name.len()) == name
            &&& (is_dir ==> line[w + 2 + name.len()] == '/')
            &&& line.last() == '\n'
            &&& ((forall|i: int| 0 <= i < name.len() ==> name[i] != '\n') ==> forall|i: int|
                0 <= i < line.len() - 1 ==> line[i] != '\n')
        }),
{
    let line = entry_line_spec(depth, indent, name, is_dir);
    let w = (depth * indent) as int;
    assert(line.subrange(w + 2, w + 2 + name.len()) =~= name);
    if forall|i: int| 0 <= i < name.len() ==> name[i] != '\n' {
        assert forall|i: int| 0 <= i < line.len() - 1 implies line[i] != '\n' by {
            if w + 2 <= i < w + 2 + name.len() {
                assert(line[i] == name[i - w - 2]);
            }
        }
    }
}

/// A representable entry name that is nonempty, not `.` or `..`, and free of
/// `/` (as every directory-entry name is) is accepted by the validator's
/// structural path rules at the same level.
pub(crate) proof fn theorem_representable_names_are_valid_paths(s: Seq<char>, at_root: bool)
    requires
        host_name_verdict_spec(s, at_root) == HostNameVerdict::Representable,
        s.len() > 0,
        !is_dot_component(s),
        forall|i: int| 0 <= i < s.len() ==> s[i] != '/',
    ensures
        component_verdict(s, at_root) == PathVerdict::Valid,
        path_verdict(s, at_root) == PathVerdict::Valid,
{
    lemma_split_single(s);
    let comps = split_components(s);
    assert(comps[0] == s);
    assert(component_verdict(comps[0], at_root && 0 == 0) == PathVerdict::Valid);
    assert(components_verdict(comps, at_root, 1) == PathVerdict::Valid);
}

/// An accepted option value is the decimal value of its digits after an
/// optional `+`, and a leading zero never changes it.
pub(crate) proof fn theorem_parse_is_decimal(s: Seq<char>)
    ensures
        parse_usize_spec(s) is Some ==> unsigned_digits(s).len() > 0 && all_decimal_digits(
            unsigned_digits(s),
        ) && parse_usize_spec(s)->0 as nat == decimal_value(unsigned_digits(s)),
        decimal_value(seq!['0'] + unsigned_digits(s)) == decimal_value(unsigned_digits(s)),
        s.len() == 0 ==> parse_usize_spec(s) is None,
        s == seq!['+'] ==> parse_usize_spec(s) is None,
{
    lemma_leading_zero(unsigned_digits(s));
    if s == seq!['+'] {
        assert(unsigned_digits(s).len() == 0);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_matches_std_on_every_sample() {
        let max = usize::MAX.to_string();
        let over = format!("{}0", usize::MAX);
        let just_over = (u128::from(usize::MAX as u64) + 1).to_string();
        let padded_max = format!("000{max}");
        let mut samples: Vec<String> = [
            "", "+", "-", "+-1", "-0", "0", "+0", "00", "007", "+16", "16", "17", "256", "257",
            "1_0", " 1", "1 ", "1.0", "0x10", "++1", "+ 1", "١", "１", "²", "9a", "a9", "1+",
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        samples.extend([max, over, just_over, padded_max]);
        for sample in &samples {
            assert_eq!(
                parse_unsigned_decimal(sample),
                sample.parse::<usize>().ok(),
                "{sample:?}"
            );
        }
        for value in 0..=300usize {
            assert_eq!(parse_unsigned_decimal(&value.to_string()), Some(value));
            assert_eq!(parse_unsigned_decimal(&format!("+{value}")), Some(value));
        }
    }

    #[test]
    fn bounds_and_configuration_cover_every_branch() {
        assert!(within_bounds(1, 1, 16));
        assert!(within_bounds(16, 1, 16));
        assert!(!within_bounds(0, 1, 16));
        assert!(!within_bounds(17, 1, 16));

        assert_eq!(
            generation_configuration(0, None),
            GenerationConfiguration::IndentOutOfRange
        );
        assert_eq!(
            generation_configuration(17, Some(999)),
            GenerationConfiguration::IndentOutOfRange
        );
        assert_eq!(
            generation_configuration(2, Some(257)),
            GenerationConfiguration::DepthOutOfRange { depth: 257 }
        );
        assert_eq!(
            generation_configuration(2, Some(usize::MAX)),
            GenerationConfiguration::DepthOutOfRange { depth: usize::MAX }
        );
        assert_eq!(
            generation_configuration(1, Some(0)),
            GenerationConfiguration::Limit(TraversalLimit {
                maximum_entry_depth: 1,
                reject_beyond_limit: false
            })
        );
        assert_eq!(
            generation_configuration(16, Some(256)),
            GenerationConfiguration::Limit(TraversalLimit {
                maximum_entry_depth: 257,
                reject_beyond_limit: false
            })
        );
        assert_eq!(
            generation_configuration(2, None),
            GenerationConfiguration::Limit(TraversalLimit {
                maximum_entry_depth: 257,
                reject_beyond_limit: true
            })
        );
    }

    #[test]
    fn depth_decisions_cover_every_branch() {
        let explicit = TraversalLimit {
            maximum_entry_depth: 2,
            reject_beyond_limit: false,
        };
        let implicit = TraversalLimit {
            maximum_entry_depth: 257,
            reject_beyond_limit: true,
        };
        assert_eq!(child_depth(0, explicit), ChildDepth::Included(1));
        assert_eq!(child_depth(1, explicit), ChildDepth::Included(2));
        assert_eq!(child_depth(2, explicit), ChildDepth::BeyondLimit);
        assert_eq!(
            child_depth(usize::MAX, implicit),
            ChildDepth::Unrepresentable
        );
        assert!(descends_into(1, true, explicit));
        assert!(!descends_into(2, true, explicit));
        assert!(!descends_into(1, false, explicit));
        assert!(descends_into(257, true, implicit));
        assert!(!descends_into(3, false, implicit));
    }

    #[test]
    fn host_name_verdict_covers_every_branch() {
        assert_eq!(host_name_verdict("a", true), HostNameVerdict::Representable);
        assert_eq!(host_name_verdict("", true), HostNameVerdict::Representable);
        assert_eq!(
            host_name_verdict("a\nb", false),
            HostNameVerdict::ControlBearing
        );
        assert_eq!(
            host_name_verdict("\\\u{7f}", true),
            HostNameVerdict::ControlBearing
        );
        assert_eq!(
            host_name_verdict("\\name", true),
            HostNameVerdict::RootedOrDrivePrefixed
        );
        assert_eq!(
            host_name_verdict("\\name", false),
            HostNameVerdict::Representable
        );
        assert_eq!(
            host_name_verdict("C:name", true),
            HostNameVerdict::RootedOrDrivePrefixed
        );
        assert_eq!(
            host_name_verdict("C:name", false),
            HostNameVerdict::Representable
        );
        assert_eq!(
            host_name_verdict("1:name", true),
            HostNameVerdict::Representable
        );
        assert!(starts_with_backslash("\\"));
        assert!(!starts_with_backslash(""));
        assert!(!starts_with_backslash("a\\"));
    }

    #[test]
    fn entry_lines_match_the_previous_formatter() {
        for (depth, indent, name, is_dir) in [
            (0, 2, "src", true),
            (1, 2, "main.rs", false),
            (3, 4, "\"a b\"", true),
            (2, 1, "é", false),
            (0, 16, "x", false),
        ] {
            let indent_text = " ".repeat(depth * indent);
            let rendered = if is_dir {
                format!("{name}/")
            } else {
                name.to_string()
            };
            assert_eq!(
                format_entry_line(depth, indent, name, is_dir),
                Some(format!("{indent_text}- {rendered}\n"))
            );
        }
        assert_eq!(format_entry_line(usize::MAX, 2, "x", false), None);
        assert_eq!(
            format_entry_line(usize::MAX, 0, "x", true),
            Some("- x/\n".to_string())
        );
    }
}
