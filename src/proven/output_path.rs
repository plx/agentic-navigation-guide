//! Verified output-destination decisions: spelling, route, and the
//! read-only parent rule.
//!
//! These kernels make the pure decisions of `cli/output.rs` for the
//! `docs/v0.2-contract.md` rules in "Filesystem output destinations" (shared
//! by `init` and `dump --output`). Glue reads the requested spelling as raw
//! bytes (Unix) or UTF-16 code units (Windows), splits paths with
//! `std::path::Path::components` (trusted) and passes only component kinds,
//! and reads permission bits; it maps each verdict to the existing
//! diagnostic, in the existing order:
//!
//! - [`unix_output_spelling`] and [`windows_output_spelling`]: a requested
//!   spelling is rejected when it is empty, then when it ends with a
//!   separator (`/`, or on Windows `\` or `/`), then when its final
//!   separator-delimited segment is `.` or `..`. On Windows these checks run
//!   on the raw code units before the namespace-prefix check
//!   (`windows_names::has_namespace_prefix`) and before host component
//!   parsing;
//! - [`is_plain_descent`]: a tail below the lexical generation root is a
//!   nonempty sequence of ordinary names;
//! - [`output_route`]: a destination takes the in-root route exactly when its
//!   lexical spelling lies below the lexical generation root by a plain
//!   descent; any other nonempty tail below the root spelling (one with a
//!   `.`, `..`, root, or prefix component) takes the ordered parent reduction
//!   of `anchor::parent_reduction_plan` before any route is chosen; only a
//!   spelling that does not start with the complete root spelling, or that
//!   names the root itself, takes the explicit external route directly;
//! - [`mode_admits_creation`]: on Unix, a parent directory whose mode grants
//!   no write permission or no search permission to any class is read-only,
//!   before the identity-specific `access` check.
//!
//! The theorems below state the contract-level consequences:
//!
//! - `theorem_output_spelling_rule`: for the final separator-delimited
//!   segment of a spelling, the verdict is `Empty` exactly for the empty
//!   spelling, `TrailingSeparator` exactly when that segment is empty,
//!   `DotSegment` exactly when it is `.` or `..`, and `Ordinary` otherwise;
//!   `theorem_final_segment_exists` shows every spelling has exactly one
//!   such segment;
//! - `theorem_accepted_unix_output_names_a_file` and
//!   `theorem_accepted_windows_output_names_a_file`: an accepted spelling
//!   ends in a nonempty final segment that contains no separator and is
//!   neither `.` nor `..`;
//! - `theorem_plain_descent_walks_to_its_final_entry`: an in-root
//!   destination tail has no `..` (so `anchor::classify_tail` calls it
//!   in-anchor), its lexical depth increases by one per component, and the
//!   verified below-anchor walk of that tail inspects every component and
//!   reaches its final entry; conversely, a tail whose below-anchor walk
//!   reaches its final entry is a plain descent;
//! - `theorem_output_route`: the route is in-root exactly for a plain descent
//!   below the root spelling, and the external route is taken directly only
//!   when the spelling does not start with the root spelling or names the
//!   root itself; in particular, a tail below the root spelling that contains
//!   `..` always takes the parent reduction, never the external route
//!   directly, and an in-root tail contains no `..`;
//! - `theorem_mode_rule`: creation is admitted exactly when some class has
//!   write permission and some class has search permission.

// Verus reasons about array literals element by element but treats
// byte-string literals as opaque, so byte comparisons stay spelled out.
#![allow(clippy::byte_char_slices)]

use crate::proven::anchor::ComponentKind;
#[cfg(verus_keep_ghost)]
use crate::proven::anchor::{
    all_normal_before, beneath_step_spec, has_parent, is_beneath_walk, lexical_depth,
    tail_class_spec, theorem_beneath_walk_accepts_only_normal_tails, BeneathStep, TailClass,
};
#[cfg(verus_keep_ghost)]
use crate::proven::windows_names::is_separator_unit;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// The verdict on a requested output spelling, in production precedence.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputSpelling {
    /// The spelling ends in an ordinary final segment.
    Ordinary,
    /// The spelling is empty.
    Empty,
    /// The spelling ends with a directory separator.
    TrailingSeparator,
    /// The final separator-delimited segment is `.` or `..`.
    DotSegment,
}

/// The route a requested destination takes, decided from whether its lexical
/// spelling starts with the complete lexical generation-root spelling and
/// from the kind of every component of the remaining tail.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputRoute {
    /// A plain descent below the root spelling: walk it from the canonical
    /// generation root.
    InRoot,
    /// Any other nonempty tail below the root spelling: reduce it in order
    /// from the canonical generation root before choosing a route.
    ReduceParents,
    /// A spelling that does not start with the root spelling, or that names
    /// the root itself: explicit external authority.
    External,
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Specifications
// ---------------------------------------------------------------------------

/// The Unix separator `/`.
pub(crate) open spec fn is_unix_separator(b: u8) -> bool {
    b == b'/'
}

/// The final separator-delimited segment of `s` is `.` or `..`: the spelling
/// ends with `dot`, preceded by its start or a separator, or with two `dot`
/// units, preceded by its start or a separator.
pub(crate) open spec fn ends_in_dot_segment<A>(s: Seq<A>, sep: spec_fn(A) -> bool, dot: A) -> bool {
    let n = s.len() as int;
    ||| (n >= 1 && s[n - 1] == dot && (n == 1 || sep(s[n - 2])))
    ||| (n >= 2 && s[n - 1] == dot && s[n - 2] == dot && (n == 2 || sep(s[n - 3])))
}

/// The spelling rule over the code units of a spelling, for separator
/// predicate `sep` and dot unit `dot`, in production precedence.
pub(crate) open spec fn spelling_rule<A>(s: Seq<A>, sep: spec_fn(A) -> bool, dot: A) -> OutputSpelling {
    if s.len() == 0 {
        OutputSpelling::Empty
    } else if sep(s.last()) {
        OutputSpelling::TrailingSeparator
    } else if ends_in_dot_segment(s, sep, dot) {
        OutputSpelling::DotSegment
    } else {
        OutputSpelling::Ordinary
    }
}

/// The Unix rule over the bytes of a spelling.
pub(crate) open spec fn unix_spelling_spec(s: Seq<u8>) -> OutputSpelling {
    spelling_rule(s, |b: u8| is_unix_separator(b), b'.')
}

/// The Windows rule over the UTF-16 code units of a spelling.
pub(crate) open spec fn windows_spelling_spec(s: Seq<u16>) -> OutputSpelling {
    spelling_rule(s, |u: u16| is_separator_unit(u), '.' as u16)
}

/// `k` starts the final separator-delimited segment of `s`: it is the start
/// of `s` or follows a separator, and no separator follows it.
pub(crate) open spec fn final_segment_start<A>(s: Seq<A>, sep: spec_fn(A) -> bool, k: int) -> bool {
    &&& 0 <= k <= s.len()
    &&& (k == 0 || sep(s[k - 1]))
    &&& forall|j: int| k <= j < s.len() ==> !sep(#[trigger] s[j])
}

/// The tail is a nonempty sequence of ordinary names.
pub(crate) open spec fn plain_descent(kinds: Seq<ComponentKind>) -> bool {
    kinds.len() > 0 && all_normal_before(kinds, kinds.len() as int)
}

/// The route of a destination whose spelling starts with the complete root
/// spelling exactly when `beneath_root`, with tail component kinds `kinds`.
pub(crate) open spec fn output_route_spec(beneath_root: bool, kinds: Seq<ComponentKind>) -> OutputRoute {
    if !beneath_root || kinds.len() == 0 {
        OutputRoute::External
    } else if plain_descent(kinds) {
        OutputRoute::InRoot
    } else {
        OutputRoute::ReduceParents
    }
}

/// Some permission class may write: `0o200`, `0o020`, or `0o002`.
pub(crate) open spec fn some_write_permission(mode: u32) -> bool {
    (mode & 0o200u32) != 0 || (mode & 0o020u32) != 0 || (mode & 0o002u32) != 0
}

/// Some permission class may search: `0o100`, `0o010`, or `0o001`.
pub(crate) open spec fn some_search_permission(mode: u32) -> bool {
    (mode & 0o100u32) != 0 || (mode & 0o010u32) != 0 || (mode & 0o001u32) != 0
}

} // verus!

// ---------------------------------------------------------------------------
// Executable kernels
// ---------------------------------------------------------------------------

/// Decide a requested Unix output spelling from its raw bytes.
#[cfg(any(unix, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == unix_spelling_spec(bytes@),
))]
pub(crate) fn unix_output_spelling(bytes: &[u8]) -> OutputSpelling {
    let n = bytes.len();
    if n == 0 {
        return OutputSpelling::Empty;
    }
    if bytes[n - 1] == b'/' {
        return OutputSpelling::TrailingSeparator;
    }
    let single = bytes[n - 1] == b'.' && (n == 1 || bytes[n - 2] == b'/');
    let double =
        n >= 2 && bytes[n - 1] == b'.' && bytes[n - 2] == b'.' && (n == 2 || bytes[n - 3] == b'/');
    if single || double {
        OutputSpelling::DotSegment
    } else {
        OutputSpelling::Ordinary
    }
}

/// Return whether `unit` is a Windows separator, `\` or `/`.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_separator_unit(unit),
))]
fn is_windows_separator(unit: u16) -> bool {
    unit == '\\' as u16 || unit == '/' as u16
}

/// Decide a requested Windows output spelling from its UTF-16 code units.
#[cfg(any(windows, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == windows_spelling_spec(units@),
))]
pub(crate) fn windows_output_spelling(units: &[u16]) -> OutputSpelling {
    let n = units.len();
    if n == 0 {
        return OutputSpelling::Empty;
    }
    if is_windows_separator(units[n - 1]) {
        return OutputSpelling::TrailingSeparator;
    }
    let dot = '.' as u16;
    let single = units[n - 1] == dot && (n == 1 || is_windows_separator(units[n - 2]));
    let double = n >= 2
        && units[n - 1] == dot
        && units[n - 2] == dot
        && (n == 2 || is_windows_separator(units[n - 3]));
    if single || double {
        OutputSpelling::DotSegment
    } else {
        OutputSpelling::Ordinary
    }
}

/// Return whether a tail below the lexical generation root is a nonempty
/// sequence of ordinary names, which selects the in-root route.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == plain_descent(kinds@),
))]
pub(crate) fn is_plain_descent(kinds: &[ComponentKind]) -> bool {
    if kinds.is_empty() {
        return false;
    }
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= kinds.len(),
            all_normal_before(kinds@, index as int),
        decreases kinds.len() - index
    ))]
    while index < kinds.len() {
        if !matches!(kinds[index], ComponentKind::Normal) {
            return false;
        }
        index += 1;
    }
    true
}

/// Choose the route of a destination from whether its lexical spelling
/// starts with the complete lexical generation-root spelling and from the
/// kind of every component of the remaining tail.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == output_route_spec(beneath_root, kinds@),
))]
pub(crate) fn output_route(beneath_root: bool, kinds: &[ComponentKind]) -> OutputRoute {
    if !beneath_root || kinds.is_empty() {
        OutputRoute::External
    } else if is_plain_descent(kinds) {
        OutputRoute::InRoot
    } else {
        OutputRoute::ReduceParents
    }
}

/// Return whether a parent directory's permission bits admit creating an
/// entry: some class may write and some class may search.
#[cfg(any(unix, test, verus_keep_ghost))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == (some_write_permission(mode) && some_search_permission(mode)),
))]
pub(crate) fn mode_admits_creation(mode: u32) -> bool {
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_permission_masks(mode);
    }
    mode & 0o222 != 0 && mode & 0o111 != 0
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Lemmas
// ---------------------------------------------------------------------------

/// The combined masks test the three classes.
proof fn lemma_permission_masks(mode: u32)
    ensures
        ((mode & 0o222u32) != 0) == some_write_permission(mode),
        ((mode & 0o111u32) != 0) == some_search_permission(mode),
{
    assert(((mode & 0o222u32) != 0) == ((mode & 0o200u32) != 0 || (mode & 0o020u32) != 0 || (mode
        & 0o002u32) != 0)) by (bit_vector);
    assert(((mode & 0o111u32) != 0) == ((mode & 0o100u32) != 0 || (mode & 0o010u32) != 0 || (mode
        & 0o001u32) != 0)) by (bit_vector);
}

/// The final segment starts after the last separator in `s[..k]`, or at the
/// start when there is none.
proof fn lemma_final_segment_of_prefix<A>(s: Seq<A>, sep: spec_fn(A) -> bool, k: int) -> (start: int)
    requires
        0 <= k <= s.len(),
        forall|j: int| k <= j < s.len() ==> !sep(#[trigger] s[j]),
    ensures
        final_segment_start(s, sep, start),
        start <= k,
    decreases k,
{
    if k == 0 || sep(s[k - 1]) {
        k
    } else {
        lemma_final_segment_of_prefix(s, sep, k - 1)
    }
}

// ---------------------------------------------------------------------------
// Contract theorems
// ---------------------------------------------------------------------------

/// Every spelling has exactly one final separator-delimited segment.
pub(crate) proof fn theorem_final_segment_exists<A>(s: Seq<A>, sep: spec_fn(A) -> bool)
    ensures
        exists|k: int| final_segment_start(s, sep, k),
        forall|k1: int, k2: int|
            final_segment_start(s, sep, k1) && final_segment_start(s, sep, k2) ==> k1 == k2,
{
    let k = lemma_final_segment_of_prefix(s, sep, s.len() as int);
    assert(final_segment_start(s, sep, k));
    assert forall|k1: int, k2: int|
        final_segment_start(s, sep, k1) && final_segment_start(s, sep, k2) implies k1 == k2 by {
        if k1 < k2 {
            assert(sep(s[k2 - 1]));
            assert(!sep(s[k2 - 1]));
        } else if k2 < k1 {
            assert(sep(s[k1 - 1]));
            assert(!sep(s[k1 - 1]));
        }
    }
}

/// The spelling rule, stated on the final separator-delimited segment
/// `s[k..]`: the verdict is `Empty` exactly for the empty spelling,
/// `TrailingSeparator` exactly when the final segment of a nonempty spelling
/// is empty, `DotSegment` exactly when the final segment is `.` or `..`, and
/// `Ordinary` exactly when it is a nonempty segment other than those two.
pub(crate) proof fn theorem_output_spelling_rule<A>(s: Seq<A>, sep: spec_fn(A) -> bool, dot: A, k: int)
    requires
        !sep(dot),
        final_segment_start(s, sep, k),
    ensures
        ({
            let n = s.len() as int;
            let segment = s.subrange(k, n);
            let dot_name = segment == seq![dot] || segment == seq![dot, dot];
            &&& (spelling_rule(s, sep, dot) == OutputSpelling::Empty) == (n == 0)
            &&& (spelling_rule(s, sep, dot) == OutputSpelling::TrailingSeparator) == (n > 0 && k
                == n)
            &&& (spelling_rule(s, sep, dot) == OutputSpelling::DotSegment) == (k < n && dot_name)
            &&& (spelling_rule(s, sep, dot) == OutputSpelling::Ordinary) == (k < n && !dot_name)
        }),
{
    let n = s.len() as int;
    let segment = s.subrange(k, n);
    if n > 0 {
        if sep(s[n - 1]) {
            // A trailing separator ends the final segment.
            assert(k == n);
        } else {
            if k == n {
                assert(sep(s[k - 1]));
            }
            assert(k < n);
            if ends_in_dot_segment(s, sep, dot) {
                if s[n - 1] == dot && (n == 1 || sep(s[n - 2])) {
                    assert(final_segment_start(s, sep, n - 1));
                    theorem_final_segment_exists(s, sep);
                    assert(k == n - 1);
                    assert(segment =~= seq![dot]);
                } else {
                    assert(final_segment_start(s, sep, n - 2));
                    theorem_final_segment_exists(s, sep);
                    assert(k == n - 2);
                    assert(segment =~= seq![dot, dot]);
                }
            } else {
                if segment == seq![dot] {
                    assert(segment.len() == 1);
                    assert(k == n - 1);
                    assert(s[n - 1] == segment[0]);
                }
                if segment == seq![dot, dot] {
                    assert(segment.len() == 2);
                    assert(k == n - 2);
                    assert(s[n - 1] == segment[1]);
                    assert(s[n - 2] == segment[0]);
                }
            }
        }
    }
}

/// An accepted Unix spelling ends in a nonempty final segment that contains
/// no `/` and is neither `.` nor `..`.
pub(crate) proof fn theorem_accepted_unix_output_names_a_file(s: Seq<u8>)
    requires
        unix_spelling_spec(s) == OutputSpelling::Ordinary,
    ensures
        exists|k: int|
            {
                &&& final_segment_start(s, |b: u8| is_unix_separator(b), k)
                &&& k < s.len()
                &&& forall|j: int| k <= j < s.len() ==> #[trigger] s[j] != b'/'
                &&& s.subrange(k, s.len() as int) != seq![b'.']
                &&& s.subrange(k, s.len() as int) != seq![b'.', b'.']
            },
{
    let sep = |b: u8| is_unix_separator(b);
    theorem_final_segment_exists(s, sep);
    let k = choose|k: int| final_segment_start(s, sep, k);
    theorem_output_spelling_rule(s, sep, b'.', k);
    assert forall|j: int| k <= j < s.len() implies #[trigger] s[j] != b'/' by {
        assert(!sep(s[j]));
    }
}

/// An accepted Windows spelling ends in a nonempty final segment that
/// contains neither `\` nor `/` and is neither `.` nor `..`.
pub(crate) proof fn theorem_accepted_windows_output_names_a_file(s: Seq<u16>)
    requires
        windows_spelling_spec(s) == OutputSpelling::Ordinary,
    ensures
        exists|k: int|
            {
                &&& final_segment_start(s, |u: u16| is_separator_unit(u), k)
                &&& k < s.len()
                &&& forall|j: int| k <= j < s.len() ==> !is_separator_unit(#[trigger] s[j])
                &&& s.subrange(k, s.len() as int) != seq!['.' as u16]
                &&& s.subrange(k, s.len() as int) != seq!['.' as u16, '.' as u16]
            },
{
    let sep = |u: u16| is_separator_unit(u);
    theorem_final_segment_exists(s, sep);
    let k = choose|k: int| final_segment_start(s, sep, k);
    theorem_output_spelling_rule(s, sep, '.' as u16, k);
    assert forall|j: int| k <= j < s.len() implies !is_separator_unit(#[trigger] s[j]) by {
        assert(!sep(s[j]));
    }
}

/// The lexical depth of a tail of ordinary names is its length at every
/// prefix.
proof fn lemma_plain_depth(kinds: Seq<ComponentKind>, j: int)
    requires
        0 <= j <= kinds.len(),
        all_normal_before(kinds, kinds.len() as int),
    ensures
        lexical_depth(kinds, j) == j,
    decreases j,
{
    if j > 0 {
        lemma_plain_depth(kinds, j - 1);
    }
}

/// An in-root destination tail has no `..`, so the anchor kernel classifies
/// it in-anchor; its lexical depth grows by one per component, so it never
/// rises to or above the generation root; and the verified below-anchor walk
/// of that tail inspects every component, each before the last as an
/// ancestor directory, and reaches its final entry. Conversely, a nonempty
/// tail whose below-anchor walk reaches its final entry is a plain descent.
pub(crate) proof fn theorem_plain_descent_walks_to_its_final_entry(
    kinds: Seq<ComponentKind>,
    steps: Seq<BeneathStep>,
)
    requires
        is_beneath_walk(kinds, steps),
    ensures
        plain_descent(kinds) == (steps.last() == BeneathStep::InspectFinal),
        plain_descent(kinds) ==> {
            &&& !has_parent(kinds)
            &&& tail_class_spec(kinds) == TailClass::InAnchor
            &&& steps.len() == kinds.len()
            &&& forall|i: int| 0 <= i < steps.len() - 1 ==> #[trigger] steps[i] == BeneathStep::InspectAncestor
            &&& forall|j: int| 0 <= j <= kinds.len() ==> #[trigger] lexical_depth(kinds, j) == j
        },
{
    theorem_beneath_walk_accepts_only_normal_tails(kinds, steps);
    if plain_descent(kinds) {
        assert forall|j: int| 0 <= j <= kinds.len() implies #[trigger] lexical_depth(kinds, j) == j by {
            lemma_plain_depth(kinds, j);
        }
        if has_parent(kinds) {
            let i = choose|i: int| 0 <= i < kinds.len() && kinds[i] == ComponentKind::ParentDir;
            assert(kinds[i] == ComponentKind::Normal);
        }
    }
}

/// The route is in-root exactly for a plain descent below the root spelling
/// and external exactly when the spelling does not start with the root
/// spelling or names the root itself; every other tail below the root
/// spelling takes the parent reduction. In particular, a tail below the root
/// spelling that contains `..` is always reduced and never routed external
/// directly, and an in-root tail contains no `..`, so the anchor kernel
/// classifies it in-anchor.
pub(crate) proof fn theorem_output_route(beneath_root: bool, kinds: Seq<ComponentKind>)
    ensures
        (output_route_spec(beneath_root, kinds) == OutputRoute::InRoot) == (beneath_root
            && plain_descent(kinds)),
        (output_route_spec(beneath_root, kinds) == OutputRoute::External) == (!beneath_root
            || kinds.len() == 0),
        (output_route_spec(beneath_root, kinds) == OutputRoute::ReduceParents) == (beneath_root
            && exists|i: int| 0 <= i < kinds.len() && #[trigger] kinds[i] != ComponentKind::Normal),
        beneath_root && has_parent(kinds) ==> output_route_spec(beneath_root, kinds)
            == OutputRoute::ReduceParents,
        output_route_spec(beneath_root, kinds) == OutputRoute::InRoot ==> !has_parent(kinds)
            && tail_class_spec(kinds) == TailClass::InAnchor,
{
    if has_parent(kinds) {
        let i = choose|i: int| 0 <= i < kinds.len() && kinds[i] == ComponentKind::ParentDir;
        assert(kinds[i] != ComponentKind::Normal);
    }
    if exists|i: int| 0 <= i < kinds.len() && #[trigger] kinds[i] != ComponentKind::Normal {
        let i = choose|i: int| 0 <= i < kinds.len() && #[trigger] kinds[i] != ComponentKind::Normal;
        assert(!all_normal_before(kinds, kinds.len() as int));
    }
    if kinds.len() > 0 && !plain_descent(kinds) {
        assert(!all_normal_before(kinds, kinds.len() as int));
        let i = choose|i: int| 0 <= i < kinds.len() && kinds[i] != ComponentKind::Normal;
        assert(kinds[i] != ComponentKind::Normal);
    }
}

/// Creation is admitted exactly when some class has write permission and
/// some class has search permission; a mode with no write bit or no search
/// bit is read-only whatever the current identity.
pub(crate) proof fn theorem_mode_rule(mode: u32)
    ensures
        (some_write_permission(mode) && some_search_permission(mode)) == ((mode & 0o222u32) != 0 && (
        mode & 0o111u32) != 0),
        (mode & 0o222u32) == 0 ==> !some_write_permission(mode),
        (mode & 0o111u32) == 0 ==> !some_search_permission(mode),
{
    lemma_permission_masks(mode);
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use ComponentKind::{CurDir, Normal, ParentDir, Prefix, RootDir};

    /// The Unix spelling rule exactly as `validate_output_spelling` stated
    /// it before it moved into this kernel.
    fn reference_unix(bytes: &[u8]) -> OutputSpelling {
        if bytes.is_empty() {
            return OutputSpelling::Empty;
        }
        if bytes.ends_with(b"/") {
            return OutputSpelling::TrailingSeparator;
        }
        if bytes
            .rsplit(|unit| *unit == b'/')
            .next()
            .is_some_and(|component| component == b"." || component == b"..")
        {
            return OutputSpelling::DotSegment;
        }
        OutputSpelling::Ordinary
    }

    /// The Windows spelling rule exactly as `validate_output_spelling` stated
    /// it before it moved into this kernel.
    fn reference_windows(raw: &[u16]) -> OutputSpelling {
        if raw.is_empty() {
            return OutputSpelling::Empty;
        }
        if raw
            .last()
            .is_some_and(|unit| *unit == u16::from(b'\\') || *unit == u16::from(b'/'))
        {
            return OutputSpelling::TrailingSeparator;
        }
        let final_component = raw
            .rsplit(|unit| *unit == u16::from(b'\\') || *unit == u16::from(b'/'))
            .next()
            .unwrap_or_default();
        if final_component == [u16::from(b'.')]
            || final_component == [u16::from(b'.'), u16::from(b'.')]
        {
            return OutputSpelling::DotSegment;
        }
        OutputSpelling::Ordinary
    }

    fn all_spellings(alphabet: &[u8], max_len: usize) -> Vec<Vec<u8>> {
        let mut all = vec![Vec::new()];
        let mut frontier = vec![Vec::new()];
        for _ in 0..max_len {
            let mut next = Vec::new();
            for spelling in &frontier {
                for byte in alphabet {
                    let mut longer = spelling.clone();
                    longer.push(*byte);
                    next.push(longer);
                }
            }
            all.extend(next.iter().cloned());
            frontier = next;
        }
        all
    }

    #[test]
    fn unix_spelling_matches_the_previous_rule() {
        for spelling in all_spellings(b"/.a\\", 6) {
            assert_eq!(
                unix_output_spelling(&spelling),
                reference_unix(&spelling),
                "{spelling:?}"
            );
        }
        assert_eq!(unix_output_spelling(b""), OutputSpelling::Empty);
        assert_eq!(
            unix_output_spelling(b"dir/"),
            OutputSpelling::TrailingSeparator
        );
        assert_eq!(unix_output_spelling(b"dir/.."), OutputSpelling::DotSegment);
        assert_eq!(unix_output_spelling(b"."), OutputSpelling::DotSegment);
        assert_eq!(unix_output_spelling(b"dir/..."), OutputSpelling::Ordinary);
        assert_eq!(unix_output_spelling(b"a.."), OutputSpelling::Ordinary);
    }

    #[test]
    fn windows_spelling_matches_the_previous_rule() {
        for spelling in all_spellings(b"/.a\\", 6) {
            let units: Vec<u16> = spelling.iter().copied().map(u16::from).collect();
            assert_eq!(
                windows_output_spelling(&units),
                reference_windows(&units),
                "{spelling:?}"
            );
        }
        let units = |text: &str| text.encode_utf16().collect::<Vec<_>>();
        assert_eq!(
            windows_output_spelling(&units(r"C:\dir\")),
            OutputSpelling::TrailingSeparator
        );
        assert_eq!(
            windows_output_spelling(&units(r"C:\dir\..")),
            OutputSpelling::DotSegment
        );
        assert_eq!(
            windows_output_spelling(&units(r"C:\dir\out.md")),
            OutputSpelling::Ordinary
        );
        assert_eq!(windows_output_spelling(&[]), OutputSpelling::Empty);
    }

    #[test]
    fn plain_descent_requires_a_nonempty_tail_of_names() {
        assert!(!is_plain_descent(&[]));
        assert!(is_plain_descent(&[Normal]));
        assert!(is_plain_descent(&[Normal, Normal, Normal]));
        for other in [CurDir, ParentDir, RootDir, Prefix] {
            assert!(!is_plain_descent(&[other]));
            assert!(!is_plain_descent(&[Normal, other]));
            assert!(!is_plain_descent(&[other, Normal]));
        }
    }

    #[test]
    fn output_route_reduces_every_non_plain_tail_below_the_root() {
        assert_eq!(output_route(false, &[Normal]), OutputRoute::External);
        assert_eq!(
            output_route(false, &[Normal, ParentDir]),
            OutputRoute::External
        );
        assert_eq!(output_route(true, &[]), OutputRoute::External);
        assert_eq!(output_route(true, &[Normal]), OutputRoute::InRoot);
        assert_eq!(output_route(true, &[Normal, Normal]), OutputRoute::InRoot);
        assert_eq!(
            output_route(true, &[Normal, ParentDir, Normal]),
            OutputRoute::ReduceParents
        );
        assert_eq!(
            output_route(true, &[ParentDir, Normal]),
            OutputRoute::ReduceParents
        );
        for other in [CurDir, ParentDir, RootDir, Prefix] {
            assert_eq!(output_route(true, &[other]), OutputRoute::ReduceParents);
            assert_eq!(
                output_route(true, &[Normal, other, Normal]),
                OutputRoute::ReduceParents
            );
        }
    }

    #[test]
    fn mode_rule_matches_the_previous_rule() {
        for mode in 0..=0o7777u32 {
            let previous_read_only = mode & 0o222 == 0 || mode & 0o111 == 0;
            assert_eq!(mode_admits_creation(mode), !previous_read_only, "{mode:o}");
        }
        assert!(mode_admits_creation(0o700));
        assert!(!mode_admits_creation(0o666));
        assert!(!mode_admits_creation(0o555));
    }
}
