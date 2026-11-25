//! Verified active-body line layout: blank lines, the list-item shape, and
//! indentation depth.
//!
//! - [`is_blank`] decides whether text consists only of Unicode `White_Space`
//!   scalars, which is exactly when `str::trim` leaves it empty.
//! - [`split_list_item`] is the anchored list-item pattern `^(\s*)- (.+)$`
//!   with the `regex` crate's default Unicode semantics: `\s` is
//!   `White_Space` (`char::is_whitespace`) and `.` is any scalar except LF.
//!   It returns the two capture groups as sub-slices of the line.
//! - [`layout_entry_line`] applies the per-line layout rules of
//!   `docs/v0.2-contract.md` ("Active body grammar" and "Indentation and
//!   parentage") in the parser's diagnostic order, and [`advance`] records
//!   the accepted line once its path expression has been parsed.
//!
//! The per-line specification mirrors the parser's check order, so the
//! contract meaning is proven over whole bodies (these proofs exist only in
//! the Verus build):
//! `theorem_layout_sound` shows that every accepted body has a first entry
//! at depth 0, depths of at most 256 that grow by at most one and only below
//! a line that can own children, and indentation widths that are all
//! multiples of a single unit between 1 and 16 spaces;
//! `theorem_layout_complete` shows that every body with those properties is
//! accepted with exactly those depths; and `theorem_layout_first_error` shows
//! that a rejected body is rejected at the first line that cannot extend a
//! valid layout of the lines before it.
//!
//! Trusted glue outside this file: `str::lines` splits the body, the parser
//! calls [`layout_entry_line`] and [`advance`] once per line in order, and it
//! decides from the parsed path expression whether a line can own children.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::char::is_white_space;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;
#[cfg(verus_keep_ghost)]
use vstd::utf8::*;

/// Largest indentation unit, in spaces.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const MAX_INDENT_UNIT: usize = 16;
/// Largest logical depth; root entries are at depth 0.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const MAX_LOGICAL_DEPTH: usize = 256;

/// Why one active-body line is not a valid entry line.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LayoutError {
    /// The line is empty or contains only whitespace.
    BlankLine,
    /// The line is not `indent "- " entry`, or its entry starts with a space
    /// or tab.
    InvalidListFormat,
    /// The indentation contains a non-space character or violates the
    /// indentation-unit, depth, or parentage rules.
    InvalidIndentationLevel,
}

/// The indentation context established by the preceding body lines.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IndentState {
    /// The indentation unit, set by the first indented line.
    pub(crate) unit: Option<usize>,
    /// The depth of the previous line, if any.
    pub(crate) previous_depth: Option<usize>,
    /// Whether the previous line denotes exactly one directory.
    pub(crate) previous_can_own_children: bool,
}

/// One body line that satisfies every layout rule.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EntryLine<'a> {
    /// The logical depth of the line.
    pub(crate) depth: usize,
    /// The entry text after `indent "- "`.
    pub(crate) content: &'a str,
    /// The indentation unit after this line.
    pub(crate) unit: Option<usize>,
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Line shape.
// ---------------------------------------------------------------------------

/// Every scalar of `s` is Unicode `White_Space`.
pub(crate) open spec fn all_white_space(s: Seq<char>) -> bool {
    forall|k: int| 0 <= k < s.len() ==> is_white_space(#[trigger] s[k])
}

/// `^(\s*)- (.+)$` matches `s` with the first capture group `s[0..w]`.
pub(crate) open spec fn list_item_at(s: Seq<char>, w: int) -> bool {
    &&& 0 <= w
    &&& w + 2 < s.len()
    &&& forall|k: int| 0 <= k < w ==> is_white_space(#[trigger] s[k])
    &&& s[w] == '-'
    &&& s[w + 1] == ' '
    &&& forall|k: int| w + 2 <= k < s.len() ==> #[trigger] s[k] != '\n'
}

/// `s[0..n]` is the maximal leading run of `White_Space` scalars.
pub(crate) open spec fn leading_white_space_run(s: Seq<char>, n: int) -> bool {
    &&& 0 <= n <= s.len()
    &&& forall|k: int| 0 <= k < n ==> is_white_space(#[trigger] s[k])
    &&& n == s.len() || !is_white_space(s[n])
}

/// `^(\s*)- (.+)$` matches `s`.
pub(crate) open spec fn is_list_item(s: Seq<char>) -> bool {
    exists|w: int| list_item_at(s, w)
}

/// The width of the list-item indentation (meaningful when
/// [`is_list_item`] holds; the decomposition is unique).
pub(crate) open spec fn item_width(s: Seq<char>) -> int {
    choose|w: int| list_item_at(s, w)
}

/// The first `n` scalars of `s` are U+0020 SPACE.
pub(crate) open spec fn spaces_only(s: Seq<char>, n: int) -> bool {
    forall|k: int| 0 <= k < n ==> #[trigger] s[k] == ' '
}

/// The shape checks of one entry line, in the parser's order. On success the
/// result is the indentation width in spaces.
#[verifier::opaque]
pub(crate) open spec fn entry_shape(s: Seq<char>) -> Result<int, LayoutError> {
    if all_white_space(s) {
        Err(LayoutError::BlankLine)
    } else if !is_list_item(s) {
        Err(LayoutError::InvalidListFormat)
    } else if !spaces_only(s, item_width(s)) {
        Err(LayoutError::InvalidIndentationLevel)
    } else if s[item_width(s) + 2] == ' ' || s[item_width(s) + 2] == '\t' {
        Err(LayoutError::InvalidListFormat)
    } else {
        Ok(item_width(s))
    }
}

// ---------------------------------------------------------------------------
// Indentation.
// ---------------------------------------------------------------------------

pub(crate) open spec fn initial_indent_state() -> IndentState {
    IndentState { unit: None, previous_depth: None, previous_can_own_children: false }
}

/// Depth bound and parentage for a line at `depth`.
pub(crate) open spec fn parent_rule(
    state: IndentState,
    depth: int,
    unit: Option<usize>,
) -> Result<(usize, Option<usize>), LayoutError> {
    if depth > 256 {
        Err(LayoutError::InvalidIndentationLevel)
    } else {
        match state.previous_depth {
            None => if depth != 0 {
                Err(LayoutError::InvalidIndentationLevel)
            } else {
                Ok((depth as usize, unit))
            },
            Some(p) => if depth > p && (depth != p + 1 || !state.previous_can_own_children) {
                Err(LayoutError::InvalidIndentationLevel)
            } else {
                Ok((depth as usize, unit))
            },
        }
    }
}

/// The indentation rules for a line of width `w`: the first indented line
/// sets the unit (at most 16), every width is a multiple of the unit, and
/// [`parent_rule`] holds. On success the result is `(depth, unit)`.
pub(crate) open spec fn indentation_rule(state: IndentState, w: int) -> Result<
    (usize, Option<usize>),
    LayoutError,
> {
    if w == 0 {
        parent_rule(state, 0, state.unit)
    } else {
        match state.unit {
            Some(u) => if u == 0 || w % (u as int) != 0 {
                Err(LayoutError::InvalidIndentationLevel)
            } else {
                parent_rule(state, w / (u as int), state.unit)
            },
            None => if w > 16 {
                Err(LayoutError::InvalidIndentationLevel)
            } else {
                parent_rule(state, 1, Some(w as usize))
            },
        }
    }
}

/// The complete layout decision for one body line.
pub(crate) open spec fn layout_line(state: IndentState, s: Seq<char>) -> Result<
    (usize, Option<usize>),
    LayoutError,
> {
    match entry_shape(s) {
        Err(e) => Err(e),
        Ok(w) => indentation_rule(state, w),
    }
}

/// The state after an accepted line.
pub(crate) open spec fn advance_state(depth: usize, unit: Option<usize>, can_own: bool) -> IndentState {
    IndentState { unit, previous_depth: Some(depth), previous_can_own_children: can_own }
}

/// The layout of a whole body: each line in order, where `owns[i]` records
/// whether line `i` can own children. On success the result is the final
/// state and the depths; on failure, the index of the first rejected line
/// and its error.
pub(crate) open spec fn layout_lines(lines: Seq<Seq<char>>, owns: Seq<bool>) -> Result<
    (IndentState, Seq<int>),
    (int, LayoutError),
>
    decreases lines.len(),
{
    if lines.len() == 0 {
        Ok((initial_indent_state(), Seq::empty()))
    } else {
        let n = lines.len() - 1;
        match layout_lines(lines.drop_last(), owns) {
            Err(e) => Err(e),
            Ok((state, depths)) => match layout_line(state, lines[n]) {
                Err(e) => Err((n, e)),
                Ok((depth, unit)) => Ok(
                    (advance_state(depth, unit, owns[n]), depths.push(depth as int)),
                ),
            },
        }
    }
}

/// The contract's depth rules: the first entry is at depth 0, every depth is
/// within 0..=256, and depth grows by at most one per line and only directly
/// below a line that can own children (`owns[i]` for line `i`).
pub(crate) open spec fn depth_rules(depths: Seq<int>, owns: Seq<bool>) -> bool {
    &&& forall|i: int| 0 <= i < depths.len() ==> 0 <= #[trigger] depths[i] <= 256
    &&& depths.len() > 0 ==> depths[0] == 0
    &&& forall|i: int| 0 < i < depths.len() ==> #[trigger] depths[i] <= depths[i - 1] + 1
    &&& forall|i: int|
        0 < i < depths.len() && #[trigger] depths[i] == depths[i - 1] + 1 ==> owns[i - 1]
}

/// Every line is a well-shaped entry whose indentation is `depths[i]` units
/// of `unit` spaces.
pub(crate) open spec fn width_rule(lines: Seq<Seq<char>>, depths: Seq<int>, unit: int) -> bool {
    forall|i: int|
        0 <= i < lines.len() ==> #[trigger] entry_shape(lines[i]) == Ok::<int, LayoutError>(
            depths[i] * unit,
        )
}

/// The contract's indentation and parentage rules for a body, stated
/// without the parser: every line is a well-shaped entry whose width is its
/// depth times one unit of 1 to 16 spaces, and the depths obey
/// [`depth_rules`].
pub(crate) open spec fn valid_layout(
    lines: Seq<Seq<char>>,
    owns: Seq<bool>,
    depths: Seq<int>,
    unit: int,
) -> bool {
    &&& depths.len() == lines.len()
    &&& 1 <= unit <= 16
    &&& depth_rules(depths, owns)
    &&& width_rule(lines, depths, unit)
}

// ---------------------------------------------------------------------------
// UTF-8 lemmas.
// ---------------------------------------------------------------------------

proof fn lemma_encode_split(s: Seq<char>, n: int)
    requires
        0 <= n <= s.len(),
    ensures
        encode_utf8(s) == encode_utf8(s.take(n)) + encode_utf8(s.skip(n)),
{
    assert(s =~= s.take(n) + s.skip(n));
    encode_utf8_concat(s.take(n), s.skip(n));
}

proof fn lemma_encode_take_push(s: Seq<char>, n: int)
    requires
        0 <= n < s.len(),
    ensures
        encode_utf8(s.take(n + 1)) == encode_utf8(s.take(n)) + encode_scalar(s[n] as u32),
{
    assert(s.take(n + 1) =~= s.take(n).push(s[n]));
    encode_utf8_push(s.take(n), s[n]);
}

/// The encoding of a prefix ends on a character boundary.
proof fn lemma_prefix_char_boundary(s: Seq<char>, n: int)
    requires
        0 <= n <= s.len(),
    ensures
        is_char_boundary(encode_utf8(s), encode_utf8(s.take(n)).len() as int),
    decreases n,
{
    broadcast use encode_utf8_valid_utf8;

    let bytes = encode_utf8(s);
    if n == 0 {
        assert(s.take(0) =~= Seq::<char>::empty());
        assert(encode_utf8(s.take(0)).len() == 0);
    } else {
        let rest = s.drop_first();
        let prefix = s.take(n);
        encode_utf8_first_scalar(s);
        let first_len = encode_scalar(s[0] as u32).len();
        assert(bytes == encode_scalar(s[0] as u32) + encode_utf8(rest));
        assert(pop_first_scalar(bytes) =~= encode_utf8(rest));
        assert(prefix[0] == s[0]);
        assert(prefix.drop_first() =~= rest.take(n - 1));
        assert(encode_utf8(prefix) == encode_scalar(s[0] as u32) + encode_utf8(rest.take(n - 1)));
        lemma_prefix_char_boundary(rest, n - 1);
        assert(first_len > 0);
    }
}

proof fn lemma_encode_injective(a: Seq<char>, b: Seq<char>)
    requires
        encode_utf8(a) == encode_utf8(b),
    ensures
        a == b,
{
    encode_utf8_decode_utf8(a);
    encode_utf8_decode_utf8(b);
}

/// The bytes before the encoding of the first `n` scalars decode to those
/// scalars.
proof fn lemma_prefix_view(s: Seq<char>, n: int, left: Seq<char>)
    requires
        0 <= n <= s.len(),
        encode_utf8(left) == encode_utf8(s).subrange(0, encode_utf8(s.take(n)).len() as int),
    ensures
        left == s.take(n),
{
    lemma_encode_split(s, n);
    let k = encode_utf8(s.take(n)).len() as int;
    assert(encode_utf8(s).subrange(0, k) =~= encode_utf8(s.take(n)));
    lemma_encode_injective(left, s.take(n));
}

/// The bytes after the encoding of the first `n` scalars decode to the
/// remaining scalars.
proof fn lemma_suffix_view(s: Seq<char>, n: int, right: Seq<char>)
    requires
        0 <= n <= s.len(),
        encode_utf8(right) == encode_utf8(s).subrange(
            encode_utf8(s.take(n)).len() as int,
            encode_utf8(s).len() as int,
        ),
    ensures
        right == s.skip(n),
{
    lemma_encode_split(s, n);
    let k = encode_utf8(s.take(n)).len() as int;
    assert(encode_utf8(s).subrange(k, encode_utf8(s).len() as int) =~= encode_utf8(s.skip(n)));
    lemma_encode_injective(right, s.skip(n));
}

/// `"- "` is two bytes.
proof fn lemma_dash_space_bytes(s: Seq<char>)
    requires
        s.len() >= 2,
        s[0] == '-',
        s[1] == ' ',
    ensures
        encode_utf8(s.take(2)).len() == 2,
{
    assert(s.take(0) =~= Seq::<char>::empty());
    lemma_encode_take_push(s, 0);
    lemma_encode_take_push(s, 1);
}

/// A list-item decomposition is unique and its indentation is the maximal
/// leading whitespace run.
proof fn lemma_list_item_unique(s: Seq<char>, w1: int, w2: int)
    requires
        list_item_at(s, w1),
        list_item_at(s, w2),
    ensures
        w1 == w2,
{
    if w1 < w2 {
        assert(is_white_space(s[w1]));
    }
    if w2 < w1 {
        assert(is_white_space(s[w2]));
    }
}

/// When `s[0..n]` is the maximal leading whitespace run, `s` is a list item
/// exactly when its decomposition starts at `n`.
proof fn lemma_list_item_at_run(s: Seq<char>, n: int)
    requires
        leading_white_space_run(s, n),
    ensures
        is_list_item(s) <==> list_item_at(s, n),
{
    if is_list_item(s) {
        let w = choose|w: int| list_item_at(s, w);
        if w < n {
            assert(is_white_space(s[w]));
        }
        if w > n {
            assert(is_white_space(s[n]));
        }
    }
}

proof fn lemma_item_width(s: Seq<char>, w: int)
    requires
        list_item_at(s, w),
    ensures
        is_list_item(s),
        item_width(s) == w,
{
    assert(is_list_item(s));
    lemma_list_item_unique(s, w, item_width(s));
}


// ---------------------------------------------------------------------------
// Contract-level theorems over whole bodies.
// ---------------------------------------------------------------------------

/// Some depth is positive, so the indentation unit has been established.
pub(crate) open spec fn any_indented(depths: Seq<int>) -> bool {
    exists|i: int| 0 <= i < depths.len() && #[trigger] depths[i] > 0
}

proof fn lemma_entry_shape_width(s: Seq<char>)
    requires
        entry_shape(s) is Ok,
    ensures
        entry_shape(s)->Ok_0 >= 0,
{
    reveal(entry_shape);
}

/// What one accepted line implies about its depth and the unit.
proof fn lemma_indentation_rule_ok(state: IndentState, w: int)
    requires
        w >= 0,
        indentation_rule(state, w) is Ok,
    ensures
        ({
            let (depth, unit) = indentation_rule(state, w)->Ok_0;
            &&& depth <= 256
            &&& state.previous_depth is None ==> depth == 0
            &&& state.previous_depth matches Some(p) ==> depth <= p + 1 && (depth == p + 1
                ==> state.previous_can_own_children)
            &&& w == 0 ==> depth == 0 && unit == state.unit
            &&& w > 0 && state.unit is Some ==> unit == state.unit && state.unit->Some_0 > 0
                && w == depth * (state.unit->Some_0 as int)
            &&& w > 0 && state.unit is None ==> depth == 1 && unit == Some(w as usize) && w <= 16
        }),
{
    if w > 0 {
        if let Some(u) = state.unit {
            let depth = w / (u as int);
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(w, u as int);
            assert(w == depth * (u as int)) by (nonlinear_arith)
                requires
                    w == (u as int) * (w / (u as int)) + w % (u as int),
                    w % (u as int) == 0,
                    depth == w / (u as int),
            ;
        }
    }
}

proof fn lemma_depth_rules_push(depths: Seq<int>, owns: Seq<bool>, d: int)
    requires
        depth_rules(depths, owns),
        0 <= d <= 256,
        depths.len() == 0 ==> d == 0,
        depths.len() > 0 ==> d <= depths.last() + 1 && (d == depths.last() + 1
            ==> owns[depths.len() - 1]),
    ensures
        depth_rules(depths.push(d), owns),
{
    let next = depths.push(d);
    assert forall|i: int| 0 <= i < depths.len() implies #[trigger] next[i] == depths[i] by {}
    assert forall|i: int| 0 < i < next.len() implies #[trigger] next[i] <= next[i - 1] + 1 by {
        if i < depths.len() {
            assert(depths[i] <= depths[i - 1] + 1);
        }
    }
    assert forall|i: int| 0 < i < next.len() && #[trigger] next[i] == next[i - 1] + 1 implies owns[i
        - 1] by {
        if i < depths.len() {
            assert(depths[i] == depths[i - 1] + 1);
        }
    }
}

proof fn lemma_depth_rules_prefix(depths: Seq<int>, owns: Seq<bool>)
    requires
        depth_rules(depths, owns),
        depths.len() > 0,
    ensures
        depth_rules(depths.drop_last(), owns),
{
    let prefix = depths.drop_last();
    assert forall|i: int| 0 <= i < prefix.len() implies #[trigger] prefix[i] == depths[i] by {}
    assert forall|i: int| 0 < i < prefix.len() implies #[trigger] prefix[i] <= prefix[i - 1] + 1 by {
        assert(depths[i] <= depths[i - 1] + 1);
    }
    assert forall|i: int|
        0 < i < prefix.len() && #[trigger] prefix[i] == prefix[i - 1] + 1 implies owns[i - 1] by {
        assert(depths[i] == depths[i - 1] + 1);
    }
}

proof fn lemma_width_rule_push(
    lines: Seq<Seq<char>>,
    depths: Seq<int>,
    unit: int,
    line: Seq<char>,
    d: int,
)
    requires
        lines.len() == depths.len(),
        width_rule(lines, depths, unit),
        entry_shape(line) == Ok::<int, LayoutError>(d * unit),
    ensures
        width_rule(lines.push(line), depths.push(d), unit),
{
    assert forall|i: int| 0 <= i < lines.len() + 1 implies #[trigger] entry_shape(lines.push(line)[i])
        == Ok::<int, LayoutError>(depths.push(d)[i] * unit) by {
        if i < lines.len() {
            assert(lines.push(line)[i] == lines[i]);
            assert(depths.push(d)[i] == depths[i]);
        }
    }
}

proof fn lemma_width_rule_prefix(lines: Seq<Seq<char>>, depths: Seq<int>, unit: int)
    requires
        lines.len() == depths.len(),
        lines.len() > 0,
        width_rule(lines, depths, unit),
    ensures
        width_rule(lines.drop_last(), depths.drop_last(), unit),
        entry_shape(lines.last()) == Ok::<int, LayoutError>(depths.last() * unit),
{
    assert forall|i: int| 0 <= i < lines.len() - 1 implies #[trigger] entry_shape(
        lines.drop_last()[i],
    ) == Ok::<int, LayoutError>(depths.drop_last()[i] * unit) by {
        assert(lines.drop_last()[i] == lines[i]);
        assert(depths.drop_last()[i] == depths[i]);
    }
}

/// With every depth zero, the width rule does not depend on the unit.
proof fn lemma_width_rule_flat(lines: Seq<Seq<char>>, depths: Seq<int>, unit: int, other: int)
    requires
        width_rule(lines, depths, unit),
        !any_indented(depths),
        forall|i: int| 0 <= i < depths.len() ==> #[trigger] depths[i] >= 0,
        lines.len() == depths.len(),
    ensures
        width_rule(lines, depths, other),
{
    assert forall|i: int| 0 <= i < lines.len() implies #[trigger] entry_shape(lines[i])
        == Ok::<int, LayoutError>(depths[i] * other) by {
        assert(depths[i] == 0);
        assert(depths[i] * unit == 0);
        assert(depths[i] * other == 0);
    }
}

proof fn lemma_any_indented_push(depths: Seq<int>, d: int)
    ensures
        any_indented(depths.push(d)) == (any_indented(depths) || d > 0),
{
    let next = depths.push(d);
    if any_indented(depths) {
        let i = choose|i: int| 0 <= i < depths.len() && #[trigger] depths[i] > 0;
        assert(next[i] == depths[i]);
    }
    if d > 0 {
        assert(next[depths.len() as int] == d);
    }
    if any_indented(next) {
        let i = choose|i: int| 0 <= i < next.len() && #[trigger] next[i] > 0;
        if i < depths.len() {
            assert(next[i] == depths[i]);
        }
    }
}

/// What is known after laying out `lines` successfully.
spec fn layout_summary(
    lines: Seq<Seq<char>>,
    owns: Seq<bool>,
    state: IndentState,
    depths: Seq<int>,
) -> bool {
    &&& depths.len() == lines.len()
    &&& depth_rules(depths, owns)
    &&& match state.unit {
        None => !any_indented(depths) && width_rule(lines, depths, 1),
        Some(u) => 1 <= u <= 16 && width_rule(lines, depths, u as int),
    }
    &&& lines.len() == 0 ==> state == initial_indent_state()
    &&& lines.len() > 0 ==> state.previous_depth == Some(depths.last() as usize)
        && state.previous_can_own_children == owns[lines.len() - 1]
}

proof fn lemma_layout_summary(lines: Seq<Seq<char>>, owns: Seq<bool>)
    requires
        layout_lines(lines, owns) is Ok,
    ensures
        layout_summary(lines, owns, layout_lines(lines, owns)->Ok_0.0, layout_lines(lines, owns)->Ok_0.1),
    decreases lines.len(),
{
    if lines.len() > 0 {
        let n = lines.len() - 1;
        let prefix = lines.drop_last();
        lemma_layout_summary(prefix, owns);
        let (state0, depths0) = layout_lines(prefix, owns)->Ok_0;
        let w = entry_shape(lines[n])->Ok_0;
        lemma_entry_shape_width(lines[n]);
        lemma_indentation_rule_ok(state0, w);
        let (depth, unit) = indentation_rule(state0, w)->Ok_0;
        let d = depth as int;
        let depths = depths0.push(d);
        assert(prefix.push(lines[n]) =~= lines);
        lemma_depth_rules_push(depths0, owns, d);
        lemma_any_indented_push(depths0, d);
        match state0.unit {
            None => {
                if w == 0 {
                    lemma_width_rule_push(prefix, depths0, 1, lines[n], d);
                } else {
                    lemma_width_rule_flat(prefix, depths0, 1, w);
                    lemma_width_rule_push(prefix, depths0, w, lines[n], d);
                }
            },
            Some(u) => {
                lemma_width_rule_push(prefix, depths0, u as int, lines[n], d);
            },
        }
    }
}

/// Soundness: every body the layout rules accept satisfies the contract's
/// indentation and parentage rules ([`valid_layout`]) with the depths it
/// reports.
pub(crate) proof fn theorem_layout_sound(lines: Seq<Seq<char>>, owns: Seq<bool>)
    requires
        layout_lines(lines, owns) is Ok,
    ensures
        exists|unit: int| valid_layout(lines, owns, layout_lines(lines, owns)->Ok_0.1, unit),
{
    lemma_layout_summary(lines, owns);
    let (state, depths) = layout_lines(lines, owns)->Ok_0;
    match state.unit {
        Some(u) => {
            assert(valid_layout(lines, owns, depths, u as int));
        },
        None => {
            assert(valid_layout(lines, owns, depths, 1));
        },
    }
}

proof fn lemma_layout_complete(lines: Seq<Seq<char>>, owns: Seq<bool>, depths: Seq<int>, unit: int)
    requires
        valid_layout(lines, owns, depths, unit),
    ensures
        layout_lines(lines, owns) is Ok,
        layout_lines(lines, owns)->Ok_0.1 == depths,
        layout_lines(lines, owns)->Ok_0.0.unit == if any_indented(depths) {
            Some(unit as usize)
        } else {
            None::<usize>
        },
        lines.len() == 0 ==> layout_lines(lines, owns)->Ok_0.0 == initial_indent_state(),
        lines.len() > 0 ==> layout_lines(lines, owns)->Ok_0.0.previous_depth == Some(
            depths.last() as usize,
        ) && layout_lines(lines, owns)->Ok_0.0.previous_can_own_children == owns[lines.len() - 1],
    decreases lines.len(),
{
    if lines.len() == 0 {
        assert(depths =~= Seq::<int>::empty());
    } else {
        let n = lines.len() - 1;
        let prefix = lines.drop_last();
        let d0 = depths.drop_last();
        lemma_depth_rules_prefix(depths, owns);
        lemma_width_rule_prefix(lines, depths, unit);
        lemma_layout_complete(prefix, owns, d0, unit);
        let state0 = layout_lines(prefix, owns)->Ok_0.0;
        let d = depths[n];
        let w = d * unit;
        assert(lines[n] == lines.last());
        assert(d0.push(d) =~= depths);
        lemma_any_indented_push(d0, d);
        if n > 0 {
            assert(d0.last() == depths[n - 1]);
            assert(d <= depths[n - 1] + 1);
            if d == depths[n - 1] + 1 {
                assert(owns[n - 1]);
            }
            if !any_indented(d0) {
                assert(depths[n - 1] <= 0) by {
                    if depths[n - 1] > 0 {
                        assert(d0[n - 1] > 0);
                    }
                }
            }
        }
        lemma_indentation_rule_complete(state0, d, unit, any_indented(d0));
    }
}

/// One line of depth `d` and width `d * unit` that obeys the depth rules is
/// accepted by the indentation rules.
proof fn lemma_indentation_rule_complete(state: IndentState, d: int, unit: int, indented_before: bool)
    requires
        1 <= unit <= 16,
        0 <= d <= 256,
        state.unit == if indented_before {
            Some(unit as usize)
        } else {
            None::<usize>
        },
        state.previous_depth is None ==> d == 0,
        state.previous_depth matches Some(p) ==> d <= p + 1 && (d == p + 1
            ==> state.previous_can_own_children),
        !indented_before && state.previous_depth is Some ==> state.previous_depth->Some_0 == 0,
    ensures
        indentation_rule(state, d * unit) == Ok::<(usize, Option<usize>), LayoutError>(
            (d as usize, if d > 0 { Some(unit as usize) } else { state.unit }),
        ),
{
    let w = d * unit;
    if d == 0 {
        assert(w == 0);
    } else {
        assert(w > 0) by (nonlinear_arith)
            requires
                d > 0,
                unit > 0,
                w == d * unit,
        ;
        if indented_before {
            vstd::arithmetic::div_mod::lemma_mod_multiples_basic(d, unit);
            vstd::arithmetic::div_mod::lemma_div_by_multiple(d, unit);
        } else {
            assert(d == 1);
            assert(w == unit);
        }
    }
}

/// Completeness: every body that satisfies the contract's indentation and
/// parentage rules ([`valid_layout`]) is accepted, with exactly the given
/// depths.
pub(crate) proof fn theorem_layout_complete(
    lines: Seq<Seq<char>>,
    owns: Seq<bool>,
    depths: Seq<int>,
    unit: int,
)
    requires
        valid_layout(lines, owns, depths, unit),
    ensures
        layout_lines(lines, owns) matches Ok((state, ds)) && ds == depths,
{
    lemma_layout_complete(lines, owns, depths, unit);
}

/// A rejected body is rejected at its first line that cannot extend a valid
/// layout of the lines before it.
pub(crate) proof fn theorem_layout_first_error(lines: Seq<Seq<char>>, owns: Seq<bool>)
    requires
        layout_lines(lines, owns) is Err,
    ensures
        ({
            let (i, e) = layout_lines(lines, owns)->Err_0;
            &&& 0 <= i < lines.len()
            &&& layout_lines(lines.take(i), owns) is Ok
            &&& exists|unit: int|
                valid_layout(lines.take(i), owns, layout_lines(lines.take(i), owns)->Ok_0.1, unit)
            &&& layout_line(layout_lines(lines.take(i), owns)->Ok_0.0, lines[i]) == Err::<
                (usize, Option<usize>),
                LayoutError,
            >(e)
        }),
    decreases lines.len(),
{
    let n = lines.len() - 1;
    let prefix = lines.drop_last();
    if layout_lines(prefix, owns) is Err {
        theorem_layout_first_error(prefix, owns);
        let (i, e) = layout_lines(prefix, owns)->Err_0;
        assert(prefix.take(i) =~= lines.take(i));
    } else {
        assert(lines.take(n) =~= prefix);
        theorem_layout_sound(prefix, owns);
    }
}

} // verus!

/// Return whether `text` consists only of Unicode `White_Space` scalars
/// (equivalently, `text.trim().is_empty()`).
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == all_white_space(text@),
))]
pub(crate) fn is_blank(text: &str) -> bool {
    let mut chars = text.chars();
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost mut gi: int = 0int; }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= gi <= text@.len(),
            chars.remaining() == text@.skip(gi),
            forall|k: int| 0 <= k < gi ==> is_white_space(#[trigger] text@[k]),
        decreases text@.len() - gi
    ))]
    loop {
        match chars.next() {
            None => {
                return true;
            }
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(text@.skip(gi)[0] == text@[gi]);
                }
                if !ch.is_whitespace() {
                    return false;
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(text@.skip(gi).drop_first() =~= text@.skip(gi + 1));
                    gi = gi + 1;
                }
            }
        }
    }
}

/// Return the byte length of the maximal leading run of Unicode
/// `White_Space` scalars in `line`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        exists|n: int| leading_white_space_run(line@, n) && r == encode_utf8(line@.take(n)).len(),
))]
fn leading_white_space(line: &str) -> usize {
    // Verification-only binding: the slice length bounds the encoded length
    // by `usize::MAX`, which rules out overflow below.
    #[cfg(verus_keep_ghost)]
    let total_len = line.as_bytes().len();
    let mut chars = line.chars();
    let mut byte_len: usize = 0;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost mut count: int = 0int; }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= count <= line@.len(),
            chars.remaining() == line@.skip(count),
            forall|k: int| 0 <= k < count ==> is_white_space(#[trigger] line@[k]),
            byte_len == encode_utf8(line@.take(count)).len(),
            total_len == encode_utf8(line@).len(),
        decreases line@.len() - count
    ))]
    loop {
        match chars.next() {
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(leading_white_space_run(line@, count));
                }
                return byte_len;
            }
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(line@.skip(count)[0] == line@[count]);
                    assert(line@.skip(count).drop_first() =~= line@.skip(count + 1));
                }
                if !ch.is_whitespace() {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(leading_white_space_run(line@, count));
                    }
                    return byte_len;
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_encode_take_push(line@, count);
                    lemma_encode_split(line@, count + 1);
                }
                byte_len += ch.len_utf8();
                #[cfg(verus_keep_ghost)]
                proof! {
                    count = count + 1;
                }
            }
        }
    }
}

/// Return whether `text` has at least one scalar and no U+000A LINE FEED
/// (the `(.+)$` part of the list-item pattern).
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == (text@.len() > 0 && forall|k: int| 0 <= k < text@.len() ==> #[trigger] text@[k] != '\n'),
))]
fn nonempty_without_line_feed(text: &str) -> bool {
    let mut chars = text.chars();
    let mut nonempty = false;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost mut gi: int = 0int; }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= gi <= text@.len(),
            chars.remaining() == text@.skip(gi),
            forall|k: int| 0 <= k < gi ==> #[trigger] text@[k] != '\n',
            nonempty == (gi > 0),
        decreases text@.len() - gi
    ))]
    loop {
        match chars.next() {
            None => {
                return nonempty;
            }
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(text@.skip(gi)[0] == text@[gi]);
                    assert(text@.skip(gi).drop_first() =~= text@.skip(gi + 1));
                }
                if ch == '\n' {
                    return false;
                }
                nonempty = true;
                #[cfg(verus_keep_ghost)]
                proof! {
                    gi = gi + 1;
                }
            }
        }
    }
}

/// Match the anchored list-item pattern `^(\s*)- (.+)$` and return its two
/// capture groups: the leading whitespace and the entry text after `"- "`.
///
/// `\s*` is greedy, and `-` is not whitespace, so the only possible first
/// group is the maximal leading whitespace run.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r is None <==> !is_list_item(line@),
        r matches Some(parts) ==> {
            let w = parts.0@.len() as int;
            &&& list_item_at(line@, w)
            &&& parts.0@ == line@.take(w)
            &&& parts.1@ == line@.skip(w + 2)
        },
))]
pub(crate) fn split_list_item(line: &str) -> Option<(&str, &str)> {
    let indent_len = leading_white_space(line);
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost indent_chars: int = choose|n: int|
            leading_white_space_run(line@, n) && indent_len == encode_utf8(line@.take(n)).len();
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_prefix_char_boundary(line@, indent_chars);
    }
    let (indent, rest) = line.split_at(indent_len);
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_prefix_view(line@, indent_chars, indent@);
        lemma_suffix_view(line@, indent_chars, rest@);
        lemma_list_item_at_run(line@, indent_chars);
    }
    let mut delimiter = rest.chars();
    if delimiter.next() != Some('-') || delimiter.next() != Some(' ') {
        return None;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(rest@[0] == '-' && rest@[1] == ' ');
        lemma_dash_space_bytes(rest@);
        lemma_prefix_char_boundary(rest@, 2);
    }
    let (_, content) = rest.split_at(2);
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_suffix_view(rest@, 2, content@);
        assert(rest@.skip(2) =~= line@.skip(indent_chars + 2));
        assert forall|k: int| 0 <= k < content@.len() implies #[trigger] content@[k]
            == line@[indent_chars + 2 + k] by {}
    }
    if !nonempty_without_line_feed(content) {
        #[cfg(verus_keep_ghost)]
        proof! {
            if list_item_at(line@, indent_chars) {
                if content@.len() > 0 {
                    let k = choose|k: int| 0 <= k < content@.len() && #[trigger] content@[k] == '\n';
                    assert(line@[indent_chars + 2 + k] == '\n');
                }
            }
        }
        return None;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert forall|k: int| indent_chars + 2 <= k < line@.len() implies #[trigger] line@[k] != '\n' by {
            assert(content@[k - indent_chars - 2] == line@[k]);
        }
        assert(list_item_at(line@, indent_chars));
    }
    Some((indent, content))
}

/// Return the number of scalars in `indent` when all of them are U+0020
/// SPACE.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r is Some <==> spaces_only(indent@, indent@.len() as int),
        r matches Some(n) ==> n == indent@.len(),
))]
fn space_indent_width(indent: &str) -> Option<usize> {
    let mut chars = indent.chars();
    let mut width: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            width <= indent@.len(),
            indent@.len() <= indent.len(),
            chars.remaining() == indent@.skip(width as int),
            spaces_only(indent@, width as int),
        decreases indent@.len() - width
    ))]
    loop {
        match chars.next() {
            None => {
                return Some(width);
            }
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(indent@.skip(width as int)[0] == indent@[width as int]);
                    assert(indent@.skip(width as int).drop_first() =~= indent@.skip(width + 1));
                }
                if ch != ' ' {
                    return None;
                }
                width += 1;
            }
        }
    }
}

/// Return whether `content` starts with U+0020 SPACE or U+0009 TAB.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == (content@.len() > 0 && (content@[0] == ' ' || content@[0] == '\t')),
))]
fn starts_with_horizontal_space(content: &str) -> bool {
    let mut chars = content.chars();
    match chars.next() {
        Some(first) => first == ' ' || first == '\t',
        None => false,
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl IndentState {
    /// The state before the first body line.
    #[cfg_attr(verus_keep_ghost, verus_spec(r =>
        ensures
            r == initial_indent_state(),
    ))]
    pub(crate) fn new() -> Self {
        IndentState {
            unit: None,
            previous_depth: None,
            previous_can_own_children: false,
        }
    }
}

/// Apply the layout rules to one body line, in the parser's diagnostic
/// order: blank line, list shape, space-only indentation, the single
/// delimiter space, the indentation unit, depth, and parentage.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        match r {
            Ok(entry) => {
                &&& layout_line(state, line@) == Ok::<(usize, Option<usize>), LayoutError>(
                    (entry.depth, entry.unit),
                )
                &&& entry.content@ == line@.skip(item_width(line@) + 2)
            },
            Err(e) => layout_line(state, line@) == Err::<(usize, Option<usize>), LayoutError>(e),
        },
))]
pub(crate) fn layout_entry_line(
    state: IndentState,
    line: &str,
) -> Result<EntryLine<'_>, LayoutError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(entry_shape);
    }
    if is_blank(line) {
        return Err(LayoutError::BlankLine);
    }
    let (indent, content) = match split_list_item(line) {
        Some(parts) => parts,
        None => return Err(LayoutError::InvalidListFormat),
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_item_width(line@, indent@.len() as int);
        assert forall|k: int| 0 <= k < indent@.len() implies #[trigger] indent@[k] == line@[k] by {}
    }
    let width = match space_indent_width(indent) {
        Some(width) => width,
        None => {
            #[cfg(verus_keep_ghost)]
            proof! {
                if spaces_only(line@, item_width(line@)) {
                    assert forall|k: int| 0 <= k < indent@.len() implies #[trigger] indent@[k] == ' ' by {
                        assert(line@[k] == ' ');
                    }
                }
            }
            return Err(LayoutError::InvalidIndentationLevel);
        }
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        assert forall|k: int| 0 <= k < item_width(line@) implies #[trigger] line@[k] == ' ' by {
            assert(indent@[k] == ' ');
        }
        assert(content@[0] == line@[item_width(line@) + 2]);
    }
    if starts_with_horizontal_space(content) {
        return Err(LayoutError::InvalidListFormat);
    }

    let (unit, depth) = if width == 0 {
        (state.unit, 0)
    } else {
        let unit = match state.unit {
            Some(unit) => unit,
            None => {
                if width > MAX_INDENT_UNIT {
                    return Err(LayoutError::InvalidIndentationLevel);
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(width % width == 0 && width / width == 1) by (nonlinear_arith)
                        requires width > 0;
                }
                width
            }
        };
        if unit == 0 || width % unit != 0 {
            return Err(LayoutError::InvalidIndentationLevel);
        }
        (Some(unit), width / unit)
    };

    if depth > MAX_LOGICAL_DEPTH {
        return Err(LayoutError::InvalidIndentationLevel);
    }
    match state.previous_depth {
        None => {
            if depth != 0 {
                return Err(LayoutError::InvalidIndentationLevel);
            }
        }
        Some(previous) => {
            if depth > previous && (depth - previous != 1 || !state.previous_can_own_children) {
                return Err(LayoutError::InvalidIndentationLevel);
            }
        }
    }
    Ok(EntryLine {
        depth,
        content,
        unit,
    })
}

/// The state after `entry` has been accepted, given whether its path
/// expression denotes exactly one directory.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == advance_state(entry.depth, entry.unit, can_own_children),
))]
pub(crate) fn advance(entry: &EntryLine<'_>, can_own_children: bool) -> IndentState {
    IndentState {
        unit: entry.unit,
        previous_depth: Some(entry.depth),
        previous_can_own_children: can_own_children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    /// Every Unicode `White_Space` scalar.
    const WHITE_SPACE: [char; 25] = [
        '\u{9}', '\u{A}', '\u{B}', '\u{C}', '\u{D}', '\u{20}', '\u{85}', '\u{A0}', '\u{1680}',
        '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}',
        '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}', '\u{2028}', '\u{2029}', '\u{202F}',
        '\u{205F}', '\u{3000}',
    ];

    fn list_item_regex() -> Regex {
        Regex::new(r"^(\s*)- (.+)$").expect("the previous parser's list-item pattern")
    }

    /// The body-line layout rules as the parser implemented them before the
    /// kernel, over a whole body whose line `i` can own children exactly
    /// when `owns[i]`. On success the result is each line's depth and entry
    /// text; on failure, the index of the rejected line and its error.
    fn reference_layout(
        regex: &Regex,
        lines: &[&str],
        owns: &[bool],
    ) -> Result<Vec<(usize, String)>, (usize, LayoutError)> {
        let mut accepted = Vec::new();
        let mut indent_size = None;
        let mut previous_depth: Option<usize> = None;
        let mut previous_line_can_own_children = false;
        for (idx, line) in lines.iter().enumerate() {
            let fail = |error| Err((idx, error));
            if line.trim().is_empty() {
                return fail(LayoutError::BlankLine);
            }
            let Some(captures) = regex.captures(line) else {
                return fail(LayoutError::InvalidListFormat);
            };
            let indent_text = captures.get(1).expect("group 1").as_str();
            if !indent_text.bytes().all(|byte| byte == b' ') {
                return fail(LayoutError::InvalidIndentationLevel);
            }
            let indent = indent_text.len();
            let content = captures.get(2).expect("group 2").as_str();
            if content.starts_with(' ') || content.starts_with('\t') {
                return fail(LayoutError::InvalidListFormat);
            }
            if indent > 0 && indent_size.is_none() {
                if indent > 16 {
                    return fail(LayoutError::InvalidIndentationLevel);
                }
                indent_size = Some(indent);
            }
            let indent_level = if indent == 0 {
                0
            } else {
                let size = indent_size.expect("a positive indentation sets the unit");
                let depth = indent / size;
                if depth * size != indent {
                    return fail(LayoutError::InvalidIndentationLevel);
                }
                depth
            };
            if indent_level > 256 {
                return fail(LayoutError::InvalidIndentationLevel);
            }
            match previous_depth {
                None if indent_level != 0 => return fail(LayoutError::InvalidIndentationLevel),
                Some(depth)
                    if indent_level > depth
                        && (depth + 1 != indent_level || !previous_line_can_own_children) =>
                {
                    return fail(LayoutError::InvalidIndentationLevel);
                }
                _ => {}
            }
            accepted.push((indent_level, content.to_string()));
            previous_depth = Some(indent_level);
            previous_line_can_own_children = owns[idx];
        }
        Ok(accepted)
    }

    /// The kernel driven exactly as `Parser::parse_guide_content` drives it.
    fn kernel_layout(
        lines: &[&str],
        owns: &[bool],
    ) -> Result<Vec<(usize, String)>, (usize, LayoutError)> {
        let mut state = IndentState::new();
        let mut accepted = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            let entry = layout_entry_line(state, line).map_err(|error| (idx, error))?;
            accepted.push((entry.depth, entry.content.to_string()));
            state = advance(&entry, owns[idx]);
        }
        Ok(accepted)
    }

    fn depths(lines: &[&str], owns: &[bool]) -> Result<Vec<usize>, (usize, LayoutError)> {
        kernel_layout(lines, owns).map(|accepted| accepted.into_iter().map(|(d, _)| d).collect())
    }

    fn all_own(lines: &[&str]) -> Vec<bool> {
        vec![true; lines.len()]
    }

    /// A small deterministic generator (xorshift64*), so the differential
    /// corpus is reproducible without another dependency.
    struct Corpus(u64);

    impl Corpus {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn below(&mut self, bound: usize) -> usize {
            (self.next() % bound as u64) as usize
        }
    }

    #[test]
    fn blank_text_is_exactly_trimmed_empty_text() {
        assert!(is_blank(""));
        assert!(is_blank(" \t"));
        assert!(is_blank("\u{3000}\u{A0}\u{85}\u{2028}\r\n"));
        assert!(!is_blank("a"));
        assert!(!is_blank(" a "));
        assert!(!is_blank("\u{200B}"));
        let all_white_space: String = WHITE_SPACE.iter().collect();
        assert!(is_blank(&all_white_space));
        assert!(!is_blank(&format!("{all_white_space}-{all_white_space}")));

        let mut buffer = [0u8; 4];
        for scalar in (0..=0x10FFFFu32).filter_map(char::from_u32) {
            let text: &str = scalar.encode_utf8(&mut buffer);
            assert_eq!(is_blank(text), text.trim().is_empty(), "{scalar:?}");
            assert_eq!(is_blank(text), WHITE_SPACE.contains(&scalar), "{scalar:?}");
        }
    }

    #[test]
    fn split_list_item_returns_the_pattern_capture_groups() {
        assert_eq!(split_list_item("- a"), Some(("", "a")));
        assert_eq!(
            split_list_item("    - src/ # x"),
            Some(("    ", "src/ # x"))
        );
        assert_eq!(split_list_item("\t- a"), Some(("\t", "a")));
        assert_eq!(split_list_item("\u{3000}- a"), Some(("\u{3000}", "a")));
        assert_eq!(
            split_list_item("\u{A0} \u{85}- a"),
            Some(("\u{A0} \u{85}", "a"))
        );
        assert_eq!(split_list_item("\n- a"), Some(("\n", "a")));
        assert_eq!(split_list_item("-  a"), Some(("", " a")));
        assert_eq!(split_list_item("- \ta"), Some(("", "\ta")));
        assert_eq!(split_list_item("- a\r"), Some(("", "a\r")));
        assert_eq!(split_list_item("- \r"), Some(("", "\r")));
        assert_eq!(
            split_list_item("- \u{e9}\u{3000}"),
            Some(("", "\u{e9}\u{3000}"))
        );
        assert_eq!(split_list_item("- - a"), Some(("", "- a")));

        for line in [
            "",
            "-",
            "- ",
            "-a",
            "a- b",
            " -",
            "  - ",
            "-\ta",
            "- a\nb",
            "- \n",
            "\u{200B}- a",
            "--  a",
            "* a",
            "\u{3000}-\u{3000}a",
        ] {
            assert_eq!(split_list_item(line), None, "{line:?}");
        }
    }

    #[test]
    fn split_list_item_matches_the_previous_regex_on_every_short_line() {
        let regex = list_item_regex();
        let alphabet = [
            ' ', '-', 'a', '\t', '\n', '\r', '\u{A0}', '\u{3000}', '\u{85}', '\u{e9}', '\u{200B}',
        ];
        let mut line = String::new();
        let mut checked = 0usize;
        for len in 0..=5u32 {
            for mut code in 0..alphabet.len().pow(len) {
                line.clear();
                for _ in 0..len {
                    line.push(alphabet[code % alphabet.len()]);
                    code /= alphabet.len();
                }
                let expected = regex.captures(&line).map(|captures| {
                    (
                        captures.get(1).expect("group 1").as_str(),
                        captures.get(2).expect("group 2").as_str(),
                    )
                });
                assert_eq!(split_list_item(&line), expected, "{line:?}");
                if let Some((indent, content)) = split_list_item(&line) {
                    // The groups are the line's own bytes at the pattern's
                    // offsets.
                    let captures = regex.captures(&line).expect("matched above");
                    assert_eq!(indent.len(), captures.get(1).expect("group 1").end());
                    assert_eq!(
                        line.len() - content.len(),
                        captures.get(2).expect("group 2").start()
                    );
                }
                checked += 1;
            }
        }
        assert_eq!(checked, (0..=5).map(|len| alphabet.len().pow(len)).sum());
    }

    #[test]
    fn layout_entry_line_reports_each_error_in_the_parser_order() {
        fn check(line: &str) -> Result<&str, LayoutError> {
            layout_entry_line(IndentState::new(), line).map(|entry| entry.content)
        }

        // Blank lines of every whitespace kind are blank before any shape
        // check.
        for line in ["", " ", "\t", "\u{3000}", "\u{A0}  ", "\r"] {
            assert_eq!(check(line), Err(LayoutError::BlankLine), "{line:?}");
        }
        // Not the list-item shape.
        for line in ["a", "-", "-a", "- ", "  -", "-\ta", "* a", "\u{200B}- a"] {
            assert_eq!(check(line), Err(LayoutError::InvalidListFormat), "{line:?}");
        }
        // Indentation that is whitespace but not spaces.
        for line in [
            "\t- a",
            " \t- a",
            "\u{3000}- a",
            "\u{A0}- a",
            "\u{85}- a",
            "\n- a",
        ] {
            assert_eq!(
                check(line),
                Err(LayoutError::InvalidIndentationLevel),
                "{line:?}"
            );
        }
        // More than one delimiter space or a tab after the dash.
        for line in ["-  a", "- \ta", "  -  a"] {
            assert_eq!(check(line), Err(LayoutError::InvalidListFormat), "{line:?}");
        }
        // A non-space indentation is reported before a doubled delimiter.
        assert_eq!(check("\t-  a"), Err(LayoutError::InvalidIndentationLevel));
        // The entry text may contain anything but a line feed, including a CR
        // and non-ASCII whitespace after its first scalar.
        assert_eq!(check("- a\r"), Ok("a\r"));
        assert_eq!(check("- \u{3000}a"), Ok("\u{3000}a"));
        assert_eq!(check("- \u{e9} # \u{e9}"), Ok("\u{e9} # \u{e9}"));
    }

    #[test]
    fn the_first_line_must_be_at_depth_zero() {
        assert_eq!(depths(&["- a"], &[true]), Ok(vec![0]));
        for indent in [1, 2, 16] {
            let line = format!("{}- a", " ".repeat(indent));
            assert_eq!(
                depths(&[&line], &[true]),
                Err((0, LayoutError::InvalidIndentationLevel)),
                "{indent}"
            );
        }
        assert_eq!(
            depths(&[&format!("{}- a", " ".repeat(17))], &[true]),
            Err((0, LayoutError::InvalidIndentationLevel))
        );
    }

    #[test]
    fn the_first_indented_line_sets_a_unit_of_one_to_sixteen_spaces() {
        for unit in [1usize, 2, 3, 4, 16] {
            let child = format!("{}- b", " ".repeat(unit));
            let grandchild = format!("{}- c", " ".repeat(2 * unit));
            let lines = ["- a", child.as_str(), grandchild.as_str(), "- d"];
            assert_eq!(
                depths(&lines, &all_own(&lines)),
                Ok(vec![0, 1, 2, 0]),
                "{unit}"
            );

            let state = advance(
                &layout_entry_line(IndentState::new(), "- a").expect("root"),
                true,
            );
            let entry = layout_entry_line(state, &child).expect("child");
            assert_eq!(entry.unit, Some(unit));
            assert_eq!(entry.depth, 1);
        }
        let child = format!("{}- b", " ".repeat(17));
        assert_eq!(
            depths(&["- a", &child], &[true, true]),
            Err((1, LayoutError::InvalidIndentationLevel))
        );
    }

    #[test]
    fn every_indentation_is_a_multiple_of_the_unit() {
        let lines = ["- a", "  - b", "    - c", "   - d"];
        assert_eq!(
            depths(&lines, &all_own(&lines)),
            Err((3, LayoutError::InvalidIndentationLevel))
        );
        let lines = ["- a", "   - b", "      - c", "   - d", "- e"];
        assert_eq!(depths(&lines, &all_own(&lines)), Ok(vec![0, 1, 2, 1, 0]));
        let lines = ["- a", "   - b", "    - c"];
        assert_eq!(
            depths(&lines, &all_own(&lines)),
            Err((2, LayoutError::InvalidIndentationLevel))
        );
        // A root line keeps the unit, and the unit survives dedents.
        let lines = ["- a", "  - b", "- c", "    - d"];
        assert_eq!(
            depths(&lines, &all_own(&lines)),
            Err((3, LayoutError::InvalidIndentationLevel))
        );
    }

    #[test]
    fn depth_grows_by_one_only_below_a_line_that_can_own_children() {
        let lines = ["- a/", "  - b", "    - c"];
        // Parent is a file, placeholder, or choice line: no children.
        assert_eq!(
            depths(&lines, &[false, true, true]),
            Err((1, LayoutError::InvalidIndentationLevel))
        );
        assert_eq!(
            depths(&lines, &[true, false, true]),
            Err((2, LayoutError::InvalidIndentationLevel))
        );
        assert_eq!(depths(&lines, &[true, true, false]), Ok(vec![0, 1, 2]));
        // Skipping a level is rejected even below a directory. (The first
        // indented line always has depth 1, because it sets the unit.)
        let lines = ["- a/", "    - b/"];
        assert_eq!(depths(&lines, &[true, true]), Ok(vec![0, 1]));
        let lines = ["- a/", "  - b/", "      - c"];
        assert_eq!(
            depths(&lines, &[true, true, true]),
            Err((2, LayoutError::InvalidIndentationLevel))
        );
        // Dedent then re-indent under a directory, and staying level under a
        // file.
        let lines = [
            "- a/", "  - b/", "    - c", "  - d/", "    - e", "- f", "- g/", "  - h",
        ];
        assert_eq!(
            depths(
                &lines,
                &[true, true, false, true, false, false, true, false]
            ),
            Ok(vec![0, 1, 2, 1, 2, 0, 0, 1])
        );
        assert_eq!(
            depths(&["- a", "- b", "- c"], &[false, false, false]),
            Ok(vec![0, 0, 0])
        );
        let lines = ["- a/", "  - b/", "- c", "  - d"];
        assert_eq!(
            depths(&lines, &[true, true, false, true]),
            Err((3, LayoutError::InvalidIndentationLevel))
        );
    }

    #[test]
    fn depth_is_bounded_by_two_hundred_fifty_six() {
        let deepest: Vec<String> = (0..=257)
            .map(|depth| format!("{}- d{depth}/", " ".repeat(depth)))
            .collect();
        let lines: Vec<&str> = deepest.iter().map(String::as_str).collect();
        let accepted = depths(&lines[..=256], &all_own(&lines[..=256])).expect("depth 256");
        assert_eq!(accepted.last(), Some(&MAX_LOGICAL_DEPTH));
        assert_eq!(
            depths(&lines, &all_own(&lines)),
            Err((257, LayoutError::InvalidIndentationLevel))
        );
        // The bound applies before parentage: a depth of 257 below a
        // line at depth 256 is rejected, and so is any wider dedent target.
        let state = IndentState {
            unit: Some(1),
            previous_depth: Some(256),
            previous_can_own_children: true,
        };
        assert_eq!(
            layout_entry_line(state, &format!("{}- x", " ".repeat(257))).map(|e| e.depth),
            Err(LayoutError::InvalidIndentationLevel)
        );
        assert_eq!(
            layout_entry_line(state, &format!("{}- x", " ".repeat(256))).map(|e| e.depth),
            Ok(256)
        );
        let state = IndentState {
            unit: Some(16),
            previous_depth: Some(256),
            previous_can_own_children: true,
        };
        assert_eq!(
            layout_entry_line(state, &format!("{}- x", " ".repeat(16 * 256))).map(|e| e.depth),
            Ok(256)
        );
        assert_eq!(
            layout_entry_line(state, &format!("{}- x", " ".repeat(16 * 257))).map(|e| e.depth),
            Err(LayoutError::InvalidIndentationLevel)
        );
    }

    #[test]
    fn advance_records_the_accepted_line() {
        let entry = EntryLine {
            depth: 3,
            content: "x",
            unit: Some(2),
        };
        for can_own in [false, true] {
            assert_eq!(
                advance(&entry, can_own),
                IndentState {
                    unit: Some(2),
                    previous_depth: Some(3),
                    previous_can_own_children: can_own,
                }
            );
        }
        assert_eq!(
            IndentState::new(),
            IndentState {
                unit: None,
                previous_depth: None,
                previous_can_own_children: false,
            }
        );
    }

    #[test]
    fn layout_matches_the_previous_parser_on_a_generated_corpus() {
        let regex = list_item_regex();
        let indents = [
            "",
            "",
            " ",
            "  ",
            "   ",
            "    ",
            "      ",
            "        ",
            "\t",
            " \t",
            "\u{3000}",
            "\u{A0}",
            "                ",
            "                 ",
        ];
        let delimiters = ["- ", "- ", "- ", "-", "-  ", "- \t", "* ", "-\t"];
        let entries = ["a", "b/", "\u{e9}", " ", "", "a\r", "x # y", "\u{3000}z"];
        let mut corpus = Corpus(0x9E37_79B9_7F4A_7C15);
        let mut lines = Vec::new();
        let mut owns = Vec::new();
        let mut accepted_bodies = 0usize;
        let mut later_errors = [0usize; 3];
        for _ in 0..20_000 {
            lines.clear();
            owns.clear();
            // Most lines follow a structured layout with a per-body unit so
            // that long bodies are often accepted; the rest are noise.
            let unit = [1, 2, 3, 4, 16, 17][corpus.below(6)];
            let mut depth = 0usize;
            for _ in 0..1 + corpus.below(6) {
                let line = match corpus.below(8) {
                    0 => ["", " ", "\t", "\u{3000}"][corpus.below(4)].to_string(),
                    1 => format!(
                        "{}{}{}",
                        indents[corpus.below(indents.len())],
                        delimiters[corpus.below(delimiters.len())],
                        entries[corpus.below(entries.len())]
                    ),
                    _ => {
                        depth = corpus.below(depth + 3);
                        format!("{}- {}", " ".repeat(depth * unit), entries[corpus.below(3)])
                    }
                };
                lines.push(line);
                owns.push(corpus.below(4) != 0);
            }
            let borrowed: Vec<&str> = lines.iter().map(String::as_str).collect();
            let outcome = kernel_layout(&borrowed, &owns);
            assert_eq!(
                outcome,
                reference_layout(&regex, &borrowed, &owns),
                "{borrowed:?} {owns:?}"
            );
            match outcome {
                Ok(accepted) if accepted.len() > 1 => accepted_bodies += 1,
                Ok(_) => {}
                Err((0, _)) => {}
                Err((_, LayoutError::BlankLine)) => later_errors[0] += 1,
                Err((_, LayoutError::InvalidListFormat)) => later_errors[1] += 1,
                Err((_, LayoutError::InvalidIndentationLevel)) => later_errors[2] += 1,
            }
        }
        // The corpus reaches every outcome past the first line.
        assert!(accepted_bodies > 500, "{accepted_bodies}");
        assert!(
            later_errors.iter().all(|&count| count > 300),
            "{later_errors:?}"
        );
    }
}
