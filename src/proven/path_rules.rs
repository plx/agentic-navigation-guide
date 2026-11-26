//! Verified structural path rules.
//!
//! [`check_path_structure`] makes the structural decision of
//! `Validator::validate_path_structure` for one decoded guide path (without
//! its directory marker), with the exact production precedence. Its
//! specification restates that precedence over the `/`-separated components
//! of the path, and Verus proves the single-pass scanner agrees with it on
//! every scalar sequence.
//!
//! The theorems below state what an accepted path means for the v0.2
//! contract (`docs/v0.2-contract.md`, "Path expressions" and "Textual-item
//! containment"):
//!
//! - `theorem_split_join` and `theorem_split_unique`: the component split is
//!   the unique decomposition of a path into `/`-free components;
//! - `theorem_valid_iff_contained`: a path is accepted exactly when it is a
//!   nonempty `/` join of nonempty components that are not `.` or `..` and
//!   contain no forbidden control, and, at the guide root, it does not start
//!   with `\` and its first component has no drive prefix;
//! - `theorem_valid_path_stays_below`: joined below a directory, an accepted
//!   path descends exactly one level per component, so no prefix of it names
//!   that directory or any ancestor of it;
//! - `theorem_valid_path_is_relative`: an accepted path is nonempty, never
//!   starts with `/`, never ends with or repeats `/`, and at the guide root
//!   never starts with `\` or a drive prefix.
//!
//! How a host interprets `\` or `:` inside a later component is outside this
//! kernel: the verifier matches such components against exact directory
//! snapshots.

use crate::path_codec::is_forbidden_control_char;
#[cfg(verus_keep_ghost)]
use crate::path_codec::{
    has_drive_prefix, has_forbidden_control, is_ascii_letter, is_forbidden_control,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

/// Outcome of the structural path rules.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathVerdict {
    /// Every structural rule holds.
    Valid,
    /// An empty, rooted, repeated-separator, control-bearing, or
    /// drive-prefixed path (`SyntaxError::InvalidPathFormat`).
    InvalidFormat,
    /// A `.` or `..` component (`SyntaxError::InvalidSpecialDirectory`).
    SpecialDirectory,
}

#[cfg(verus_keep_ghost)]
verus! {

/// The `/`-separated components of `s` from left to right, exactly as
/// `str::split('/')` yields them: always at least one, possibly empty.
pub(crate) open spec fn split_components(s: Seq<char>) -> Seq<Seq<char>>
    decreases s.len(),
{
    if s.len() == 0 {
        seq![Seq::<char>::empty()]
    } else {
        split_step(split_components(s.drop_last()), s.last())
    }
}

/// Extends a component split by one scalar: `/` opens a new empty component
/// and any other scalar extends the last one.
pub(crate) open spec fn split_step(comps: Seq<Seq<char>>, c: char) -> Seq<Seq<char>> {
    if c == '/' {
        comps.push(Seq::<char>::empty())
    } else {
        comps.update(comps.len() - 1, comps.last().push(c))
    }
}

/// Joins components with `/` separators.
pub(crate) open spec fn join_components(comps: Seq<Seq<char>>) -> Seq<char>
    decreases comps.len(),
{
    if comps.len() == 0 {
        Seq::empty()
    } else if comps.len() == 1 {
        comps[0]
    } else {
        join_components(comps.drop_last()) + seq!['/'] + comps.last()
    }
}

/// The special directory components `.` and `..`.
pub(crate) open spec fn is_dot_component(c: Seq<char>) -> bool {
    c == seq!['.'] || c == seq!['.', '.']
}

/// The verdict for one raw component in production precedence: empty, then
/// `.`/`..`, then a forbidden control, then (for the first component of a
/// root-level item) a drive prefix.
pub(crate) open spec fn component_verdict(c: Seq<char>, first_at_root: bool) -> PathVerdict {
    if c.len() == 0 {
        PathVerdict::InvalidFormat
    } else if is_dot_component(c) {
        PathVerdict::SpecialDirectory
    } else if has_forbidden_control(c) {
        PathVerdict::InvalidFormat
    } else if first_at_root && has_drive_prefix(c) {
        PathVerdict::InvalidFormat
    } else {
        PathVerdict::Valid
    }
}

/// The first non-valid component verdict from index `i` on, left to right.
pub(crate) open spec fn components_verdict(
    comps: Seq<Seq<char>>,
    at_root: bool,
    i: int,
) -> PathVerdict
    decreases comps.len() - i,
{
    if i < 0 || i >= comps.len() {
        PathVerdict::Valid
    } else if component_verdict(comps[i], at_root && i == 0) != PathVerdict::Valid {
        component_verdict(comps[i], at_root && i == 0)
    } else {
        components_verdict(comps, at_root, i + 1)
    }
}

/// The production decision for one decoded path: empty, `/`-rooted, or (at
/// the guide root) `\`-rooted paths are invalid, and otherwise the first
/// failing component decides.
pub(crate) open spec fn path_verdict(s: Seq<char>, at_root: bool) -> PathVerdict {
    if s.len() == 0 || s[0] == '/' || (at_root && s[0] == '\\') {
        PathVerdict::InvalidFormat
    } else {
        components_verdict(split_components(s), at_root, 0)
    }
}

/// A component that names one entry strictly inside its parent: nonempty,
/// not `.` or `..`, and free of `/` and forbidden controls.
pub(crate) open spec fn is_child_name(c: Seq<char>) -> bool {
    c.len() > 0 && !is_dot_component(c) && !has_forbidden_control(c) && !c.contains('/')
}

/// `s` is a nonempty `/` join of child names; at the guide root it does not
/// start with `\` and its first component has no drive prefix.
pub(crate) open spec fn is_contained_relative_path(s: Seq<char>, at_root: bool) -> bool {
    exists|comps: Seq<Seq<char>>|
        #[trigger] join_components(comps) == s && comps.len() >= 1 && (forall|i: int|
            0 <= i < comps.len() ==> is_child_name(#[trigger] comps[i])) && (at_root ==> s[0]
            != '\\' && !has_drive_prefix(comps[0]))
}

/// The lexical depth below the starting directory after resolving each
/// `/`-separated component: `..` climbs one level (possibly above the
/// start), `.` and empty components (repeated separators) stay, and every
/// other component descends one level. The model treats `/` as the only
/// separator; on Windows, where `\` and drive forms are also meaningful
/// inside later components, containment additionally rests on the
/// verifier's exact matching against directory snapshots, which is not
/// part of this proof.
pub(crate) open spec fn lexical_depth(comps: Seq<Seq<char>>) -> int
    decreases comps.len(),
{
    if comps.len() == 0 {
        0
    } else if comps.last() == seq!['.', '.'] {
        lexical_depth(comps.drop_last()) - 1
    } else if comps.last().len() == 0 || comps.last() == seq!['.'] {
        lexical_depth(comps.drop_last())
    } else {
        lexical_depth(comps.drop_last()) + 1
    }
}

/// The scanner's summary of the current component `c`: its length
/// saturated at three, whether every scalar is `.`, whether it contains a
/// forbidden control, whether it starts with an ASCII letter, and whether it
/// has a drive prefix.
spec fn summarizes(
    c: Seq<char>,
    len: usize,
    all_dots: bool,
    control: bool,
    letter_first: bool,
    drive: bool,
) -> bool {
    &&& len == (if c.len() < 3 {
        c.len()
    } else {
        3
    })
    &&& all_dots == (forall|k: int| 0 <= k < c.len() ==> #[trigger] c[k] == '.')
    &&& control == has_forbidden_control(c)
    &&& letter_first == (c.len() >= 1 && is_ascii_letter(c[0]))
    &&& drive == has_drive_prefix(c)
}

/// The verdict computed from a component summary.
spec fn summary_verdict_spec(
    len: usize,
    all_dots: bool,
    control: bool,
    drive_at_root: bool,
) -> PathVerdict {
    if len == 0 {
        PathVerdict::InvalidFormat
    } else if all_dots && len <= 2 {
        PathVerdict::SpecialDirectory
    } else if control || drive_at_root {
        PathVerdict::InvalidFormat
    } else {
        PathVerdict::Valid
    }
}

} // verus!

/// Return whether `ch` is an ASCII letter.
// The ASCII letter ranges are spelled out so Verus verifies them directly
// rather than trusting a library specification of `char::is_ascii_alphabetic`.
#[allow(clippy::manual_is_ascii_check)]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_ascii_letter(ch),
))]
fn is_ascii_letter_char(ch: char) -> bool {
    matches!(ch, 'A'..='Z' | 'a'..='z')
}

/// The verdict for one completed component from the scanner's summary.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == summary_verdict_spec(len, all_dots, control, drive_at_root),
))]
fn summary_verdict(len: usize, all_dots: bool, control: bool, drive_at_root: bool) -> PathVerdict {
    if len == 0 {
        PathVerdict::InvalidFormat
    } else if all_dots && len <= 2 {
        PathVerdict::SpecialDirectory
    } else if control || drive_at_root {
        PathVerdict::InvalidFormat
    } else {
        PathVerdict::Valid
    }
}

/// Decide the structural path rules for one decoded guide path.
///
/// `at_root` is true for root-level guide items, whose decoded path is the
/// start of the full guide path: only there does a leading `\` or a
/// drive-prefixed first component reject.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == path_verdict(path@, at_root),
))]
pub(crate) fn check_path_structure(path: &str, at_root: bool) -> PathVerdict {
    let mut lead = path.chars();
    match lead.next() {
        None => return PathVerdict::InvalidFormat,
        Some(first) => {
            if first == '/' || (at_root && first == '\\') {
                return PathVerdict::InvalidFormat;
            }
        }
    }

    // One pass over the scalars, summarizing the current component and
    // deciding each component when its terminating `/` or the end arrives.
    let mut chars = path.chars();
    let mut first_component = true;
    let mut len: usize = 0;
    let mut all_dots = true;
    let mut control = false;
    let mut letter_first = false;
    let mut drive = false;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(path@.take(0) =~= Seq::<char>::empty());
        assert(summarizes(Seq::<char>::empty(), 0, true, false, false, false));
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= path@.len(),
            path@.len() > 0,
            path@[0] != '/',
            !(at_root && path@[0] == '\\'),
            chars.remaining() == path@.skip(index),
            split_components(path@.take(index)).len() >= 1,
            first_component == (split_components(path@.take(index)).len() == 1),
            summarizes(
                split_components(path@.take(index)).last(),
                len,
                all_dots,
                control,
                letter_first,
                drive,
            ),
            forall|k: int|
                0 <= k < split_components(path@.take(index)).len() - 1 ==> component_verdict(
                    #[trigger] split_components(path@.take(index))[k],
                    at_root && k == 0,
                ) == PathVerdict::Valid,
        decreases path@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_split_take_step(path@, index);
                }
                if ch == '/' {
                    let verdict = summary_verdict(
                        len,
                        all_dots,
                        control,
                        first_component && at_root && drive,
                    );
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        let comps = split_components(path@.take(index));
                        lemma_summary_verdict(
                            comps.last(),
                            len,
                            all_dots,
                            control,
                            letter_first,
                            drive,
                            first_component && at_root,
                        );
                    }
                    if !matches!(verdict, PathVerdict::Valid) {
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            lemma_verdict_at_separator(path@, at_root, index);
                        }
                        return verdict;
                    }
                    first_component = false;
                    len = 0;
                    all_dots = true;
                    control = false;
                    letter_first = false;
                    drive = false;
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(summarizes(Seq::<char>::empty(), 0, true, false, false, false));
                    }
                } else {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        let cur = split_components(path@.take(index)).last();
                        lemma_summarizes_push(cur, ch, len, all_dots, control, letter_first, drive);
                    }
                    if len == 0 {
                        letter_first = is_ascii_letter_char(ch);
                    }
                    if len == 1 {
                        drive = letter_first && ch == ':';
                    }
                    if ch != '.' {
                        all_dots = false;
                    }
                    if is_forbidden_control_char(ch) {
                        control = true;
                    }
                    if len < 3 {
                        len += 1;
                    }
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    index = index + 1;
                }
            }
            None => {
                let verdict =
                    summary_verdict(len, all_dots, control, first_component && at_root && drive);
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(path@.take(index) =~= path@);
                    let comps = split_components(path@);
                    let m = comps.len() - 1;
                    lemma_summary_verdict(
                        comps.last(),
                        len,
                        all_dots,
                        control,
                        letter_first,
                        drive,
                        first_component && at_root,
                    );
                    lemma_verdict_skip_valid(comps, at_root, m);
                    assert(comps[m] == comps.last());
                    assert(components_verdict(comps, at_root, m + 1) == PathVerdict::Valid);
                }
                return verdict;
            }
        }
    }
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Split lemmas
// ---------------------------------------------------------------------------

proof fn lemma_split_len(s: Seq<char>)
    ensures
        split_components(s).len() >= 1,
    decreases s.len(),
{
    if s.len() > 0 {
        lemma_split_len(s.drop_last());
    }
}

/// Splitting a prefix one scalar longer takes one split step.
proof fn lemma_split_take_step(s: Seq<char>, i: int)
    requires
        0 <= i < s.len(),
    ensures
        split_components(s.take(i + 1)) == split_step(split_components(s.take(i)), s[i]),
        split_components(s.take(i)).len() >= 1,
{
    assert(s.take(i + 1).drop_last() =~= s.take(i));
    lemma_split_len(s.take(i));
}

/// A split step on a concatenation only touches the right-hand split.
proof fn lemma_split_step_concat(x: Seq<Seq<char>>, y: Seq<Seq<char>>, c: char)
    requires
        y.len() >= 1,
    ensures
        split_step(x + y, c) == x + split_step(y, c),
{
    if c == '/' {
        assert((x + y).push(Seq::<char>::empty()) =~= x + y.push(Seq::<char>::empty()));
    } else {
        assert((x + y).update((x + y).len() - 1, (x + y).last().push(c)) =~= x + y.update(
            y.len() - 1,
            y.last().push(c),
        ));
    }
}

/// `split(a + "/" + b) == split(a) + split(b)`.
proof fn lemma_split_separator(a: Seq<char>, b: Seq<char>)
    ensures
        split_components(a + seq!['/'] + b) == split_components(a) + split_components(b),
    decreases b.len(),
{
    let s = a + seq!['/'] + b;
    if b.len() == 0 {
        assert(s.drop_last() =~= a);
        assert(seq![Seq::<char>::empty()] =~= split_components(b));
        assert(split_components(a).push(Seq::<char>::empty()) =~= split_components(a)
            + seq![Seq::<char>::empty()]);
    } else {
        assert(s.drop_last() =~= a + seq!['/'] + b.drop_last());
        lemma_split_separator(a, b.drop_last());
        lemma_split_len(b.drop_last());
        lemma_split_step_concat(split_components(a), split_components(b.drop_last()), b.last());
    }
}

/// Appending `/`-free text extends the last component.
proof fn lemma_split_append_free(a: Seq<char>, b: Seq<char>)
    requires
        !b.contains('/'),
    ensures
        split_components(a + b) == split_components(a).update(
            split_components(a).len() - 1,
            split_components(a).last() + b,
        ),
    decreases b.len(),
{
    lemma_split_len(a);
    let sa = split_components(a);
    if b.len() == 0 {
        assert(a + b =~= a);
        assert(sa.last() + b =~= sa.last());
        assert(sa.update(sa.len() - 1, sa.last()) =~= sa);
    } else {
        assert((a + b).drop_last() =~= a + b.drop_last());
        assert(!b.drop_last().contains('/')) by {
            if b.drop_last().contains('/') {
                let k = choose|k: int| 0 <= k < b.drop_last().len() && b.drop_last()[k] == '/';
                assert(b[k] == '/');
            }
        }
        assert(b.last() != '/') by {
            assert(b[b.len() - 1] == b.last());
        }
        lemma_split_append_free(a, b.drop_last());
        assert((sa.last() + b.drop_last()).push(b.last()) =~= sa.last() + b);
        assert(sa.update(sa.len() - 1, sa.last() + b.drop_last()).update(
            sa.len() - 1,
            (sa.last() + b.drop_last()).push(b.last()),
        ) =~= sa.update(sa.len() - 1, sa.last() + b));
    }
}

/// The join of a split is the original text, and no component contains `/`.
pub(crate) proof fn theorem_split_join(s: Seq<char>)
    ensures
        split_components(s).len() >= 1,
        join_components(split_components(s)) == s,
        forall|i: int|
            0 <= i < split_components(s).len() ==> !(#[trigger] split_components(s)[i]).contains(
                '/',
            ),
    decreases s.len(),
{
    if s.len() > 0 {
        let p = s.drop_last();
        let c = s.last();
        theorem_split_join(p);
        let a = split_components(p);
        assert(s =~= p.push(c));
        if c == '/' {
            let n = a.push(Seq::<char>::empty());
            assert(n.drop_last() =~= a);
            assert(join_components(n) =~= p + seq!['/'] + Seq::<char>::empty());
            assert forall|i: int| 0 <= i < n.len() implies !(#[trigger] n[i]).contains('/') by {
                if i < a.len() {
                    assert(n[i] == a[i]);
                } else {
                    assert(n[i] =~= Seq::<char>::empty());
                }
            }
        } else {
            let n = a.update(a.len() - 1, a.last().push(c));
            if a.len() == 1 {
                assert(join_components(n) == n[0]);
                assert(n[0] =~= p.push(c));
            } else {
                assert(n.drop_last() =~= a.drop_last());
                assert(join_components(n) =~= join_components(a.drop_last()) + seq!['/']
                    + a.last().push(c));
                assert(join_components(a) == join_components(a.drop_last()) + seq!['/']
                    + a.last());
                assert(join_components(n) =~= p.push(c));
            }
            assert forall|i: int| 0 <= i < n.len() implies !(#[trigger] n[i]).contains('/') by {
                if i < a.len() - 1 {
                    assert(n[i] == a[i]);
                } else {
                    assert(!a[i].contains('/'));
                    if n[i].contains('/') {
                        let k = choose|k: int| 0 <= k < n[i].len() && n[i][k] == '/';
                        if k < a.last().len() {
                            assert(a.last()[k] == '/');
                        }
                    }
                }
            }
        }
    }
}

/// A nonempty sequence of `/`-free components is the split of its join.
pub(crate) proof fn theorem_split_unique(comps: Seq<Seq<char>>)
    requires
        comps.len() >= 1,
        forall|i: int| 0 <= i < comps.len() ==> !(#[trigger] comps[i]).contains('/'),
    ensures
        split_components(join_components(comps)) == comps,
    decreases comps.len(),
{
    if comps.len() == 1 {
        lemma_split_append_free(Seq::<char>::empty(), comps[0]);
        assert(Seq::<char>::empty() + comps[0] =~= comps[0]);
        assert(split_components(Seq::<char>::empty()) =~= seq![Seq::<char>::empty()]);
        assert(seq![Seq::<char>::empty()].update(0, Seq::<char>::empty() + comps[0]) =~= comps);
    } else {
        let init = comps.drop_last();
        let last = comps.last();
        assert forall|i: int| 0 <= i < init.len() implies !(#[trigger] init[i]).contains('/') by {
            assert(init[i] == comps[i]);
        }
        theorem_split_unique(init);
        assert(!last.contains('/')) by {
            assert(comps[comps.len() - 1] == last);
        }
        lemma_split_separator(join_components(init), last);
        lemma_split_append_free(Seq::<char>::empty(), last);
        assert(Seq::<char>::empty() + last =~= last);
        assert(split_components(Seq::<char>::empty()) =~= seq![Seq::<char>::empty()]);
        assert(seq![Seq::<char>::empty()].update(0, Seq::<char>::empty() + last) =~= seq![last]);
        assert(init + seq![last] =~= comps);
    }
}

// ---------------------------------------------------------------------------
// Verdict lemmas
// ---------------------------------------------------------------------------

/// Valid components before `m` do not affect the verdict.
proof fn lemma_verdict_skip_valid(comps: Seq<Seq<char>>, at_root: bool, m: int)
    requires
        0 <= m,
        forall|k: int|
            0 <= k < m && k < comps.len() ==> component_verdict(
                #[trigger] comps[k],
                at_root && k == 0,
            ) == PathVerdict::Valid,
    ensures
        components_verdict(comps, at_root, 0) == components_verdict(comps, at_root, m),
    decreases m,
{
    if m > 0 {
        lemma_verdict_skip_valid(comps, at_root, m - 1);
        if m - 1 < comps.len() {
            assert(component_verdict(comps[m - 1], at_root && m - 1 == 0) == PathVerdict::Valid);
        }
    }
}

/// The verdict from `i` is valid exactly when every component from `i` on is.
proof fn lemma_verdict_valid_iff(comps: Seq<Seq<char>>, at_root: bool, i: int)
    requires
        0 <= i,
    ensures
        (components_verdict(comps, at_root, i) == PathVerdict::Valid) == (forall|k: int|
            i <= k < comps.len() ==> component_verdict(#[trigger] comps[k], at_root && k == 0)
                == PathVerdict::Valid),
    decreases comps.len() - i,
{
    if i < comps.len() {
        lemma_verdict_valid_iff(comps, at_root, i + 1);
    }
}

/// When a component ends at the separator at `index` and is not valid, it
/// decides the whole path.
proof fn lemma_verdict_at_separator(s: Seq<char>, at_root: bool, index: int)
    requires
        0 <= index < s.len(),
        s[index] == '/',
        split_components(s.take(index)).len() >= 1,
        forall|k: int|
            0 <= k < split_components(s.take(index)).len() - 1 ==> component_verdict(
                #[trigger] split_components(s.take(index))[k],
                at_root && k == 0,
            ) == PathVerdict::Valid,
        component_verdict(
            split_components(s.take(index)).last(),
            at_root && split_components(s.take(index)).len() == 1,
        ) != PathVerdict::Valid,
    ensures
        components_verdict(split_components(s), at_root, 0) == component_verdict(
            split_components(s.take(index)).last(),
            at_root && split_components(s.take(index)).len() == 1,
        ),
{
    let x = split_components(s.take(index));
    let rest = s.subrange(index + 1, s.len() as int);
    assert(s =~= s.take(index) + seq!['/'] + rest);
    lemma_split_separator(s.take(index), rest);
    lemma_split_len(rest);
    let comps = split_components(s);
    assert(comps == x + split_components(rest));
    let m = x.len() - 1;
    assert forall|k: int|
        0 <= k < m && k < comps.len() implies component_verdict(
        #[trigger] comps[k],
        at_root && k == 0,
    ) == PathVerdict::Valid by {
        assert(comps[k] == x[k]);
    }
    lemma_verdict_skip_valid(comps, at_root, m);
    assert(comps[m] == x.last());
}

/// The scanner's verdict for a summarized component is the component verdict.
proof fn lemma_summary_verdict(
    c: Seq<char>,
    len: usize,
    all_dots: bool,
    control: bool,
    letter_first: bool,
    drive: bool,
    first_at_root: bool,
)
    requires
        summarizes(c, len, all_dots, control, letter_first, drive),
    ensures
        summary_verdict_spec(len, all_dots, control, first_at_root && drive) == component_verdict(
            c,
            first_at_root,
        ),
{
    if c.len() == 1 && all_dots {
        assert(c[0] == '.');
        assert(c =~= seq!['.']);
    }
    if c.len() == 2 && all_dots {
        assert(c[0] == '.' && c[1] == '.');
        assert(c =~= seq!['.', '.']);
    }
    if c == seq!['.'] {
        assert(all_dots);
    }
    if c == seq!['.', '.'] {
        assert(c[0] == '.' && c[1] == '.');
        assert(all_dots);
    }
}

/// The scanner's summary update for one more non-separator scalar.
proof fn lemma_summarizes_push(
    c: Seq<char>,
    ch: char,
    len: usize,
    all_dots: bool,
    control: bool,
    letter_first: bool,
    drive: bool,
)
    requires
        summarizes(c, len, all_dots, control, letter_first, drive),
    ensures
        ({
            let n = c.push(ch);
            let letter_first2 = if len == 0 {
                is_ascii_letter(ch)
            } else {
                letter_first
            };
            let drive2 = if len == 1 {
                letter_first2 && ch == ':'
            } else {
                drive
            };
            summarizes(
                n,
                if len < 3 {
                    (len + 1) as usize
                } else {
                    len
                },
                all_dots && ch == '.',
                control || is_forbidden_control(ch),
                letter_first2,
                drive2,
            )
        }),
{
    let n = c.push(ch);
    assert(n[c.len() as int] == ch);
    assert forall|k: int| 0 <= k < c.len() implies #[trigger] n[k] == c[k] by {}
    if has_forbidden_control(n) {
        let k = choose|k: int| 0 <= k < n.len() && #[trigger] is_forbidden_control(n[k]);
        if k < c.len() {
            assert(is_forbidden_control(c[k]));
        }
    }
    if has_forbidden_control(c) {
        let k = choose|k: int| 0 <= k < c.len() && #[trigger] is_forbidden_control(c[k]);
        assert(is_forbidden_control(n[k]));
    }
    if is_forbidden_control(ch) {
        assert(is_forbidden_control(n[c.len() as int]));
    }
    if all_dots && ch == '.' {
        assert forall|k: int| 0 <= k < n.len() implies #[trigger] n[k] == '.' by {
            if k < c.len() {
                assert(c[k] == '.');
            }
        }
    }
    if !all_dots {
        let k = choose|k: int| 0 <= k < c.len() && !(#[trigger] c[k] == '.');
        assert(n[k] != '.');
    }
}

// ---------------------------------------------------------------------------
// Containment theorems
// ---------------------------------------------------------------------------

/// A join of nonempty components starts with its first component.
proof fn lemma_join_starts_with_first(comps: Seq<Seq<char>>)
    requires
        comps.len() >= 1,
    ensures
        join_components(comps).len() >= comps[0].len(),
        forall|k: int|
            0 <= k < comps[0].len() ==> #[trigger] join_components(comps)[k] == comps[0][k],
    decreases comps.len(),
{
    if comps.len() > 1 {
        lemma_join_starts_with_first(comps.drop_last());
        assert(comps.drop_last()[0] == comps[0]);
    }
}

/// A path is valid exactly when it is a nonempty `/` join of child names
/// that, at the guide root, does not start with `\` and has no drive prefix
/// on its first component.
pub(crate) proof fn theorem_valid_iff_contained(s: Seq<char>, at_root: bool)
    ensures
        (path_verdict(s, at_root) == PathVerdict::Valid) == is_contained_relative_path(s, at_root),
{
    theorem_split_join(s);
    let comps = split_components(s);
    if path_verdict(s, at_root) == PathVerdict::Valid {
        lemma_verdict_valid_iff(comps, at_root, 0);
        assert forall|i: int| 0 <= i < comps.len() implies is_child_name(#[trigger] comps[i]) by {
            assert(component_verdict(comps[i], at_root && i == 0) == PathVerdict::Valid);
        }
        if at_root {
            assert(component_verdict(comps[0], true) == PathVerdict::Valid);
        }
        assert(join_components(comps) == s);
    }
    if is_contained_relative_path(s, at_root) {
        let w = choose|w: Seq<Seq<char>>|
            #[trigger] join_components(w) == s && w.len() >= 1 && (forall|i: int|
                0 <= i < w.len() ==> is_child_name(#[trigger] w[i])) && (at_root ==> s[0] != '\\'
                && !has_drive_prefix(w[0]));
        assert forall|i: int| 0 <= i < w.len() implies !(#[trigger] w[i]).contains('/') by {
            assert(is_child_name(w[i]));
        }
        theorem_split_unique(w);
        assert(comps == w);
        lemma_join_starts_with_first(w);
        assert(is_child_name(w[0]));
        assert(s[0] == w[0][0]);
        assert(s[0] != '/') by {
            if s[0] == '/' {
                assert(w[0].contains('/'));
            }
        }
        lemma_verdict_valid_iff(comps, at_root, 0);
        assert forall|k: int| 0 <= k < comps.len() implies component_verdict(
            #[trigger] comps[k],
            at_root && k == 0,
        ) == PathVerdict::Valid by {
            assert(is_child_name(w[k]));
        }
    }
}

/// Joined below a starting directory, a valid path descends exactly one
/// level per component: every nonempty prefix of its components resolves
/// strictly below the start, so it never names the start itself or any
/// ancestor of it.
pub(crate) proof fn theorem_valid_path_stays_below(s: Seq<char>, at_root: bool, k: int)
    requires
        path_verdict(s, at_root) == PathVerdict::Valid,
        0 <= k <= split_components(s).len(),
    ensures
        lexical_depth(split_components(s).take(k)) == k,
    decreases k,
{
    let comps = split_components(s);
    if k > 0 {
        theorem_valid_path_stays_below(s, at_root, k - 1);
        lemma_verdict_valid_iff(comps, at_root, 0);
        assert(component_verdict(comps[k - 1], at_root && k - 1 == 0) == PathVerdict::Valid);
        assert(comps.take(k).drop_last() =~= comps.take(k - 1));
        assert(comps.take(k).last() == comps[k - 1]);
    } else {
        assert(comps.take(0) =~= Seq::<Seq<char>>::empty());
    }
}

/// A valid path is not absolute or rooted: it is nonempty, does not start
/// with `/`, and at the guide root does not start with `\` or a drive prefix.
/// It also has no empty component, so it never ends with `/` or repeats one.
pub(crate) proof fn theorem_valid_path_is_relative(s: Seq<char>, at_root: bool)
    requires
        path_verdict(s, at_root) == PathVerdict::Valid,
    ensures
        s.len() > 0,
        s[0] != '/',
        at_root ==> s[0] != '\\' && !has_drive_prefix(s),
        s[s.len() - 1] != '/',
        forall|i: int| 0 <= i < s.len() - 1 ==> !(s[i] == '/' && #[trigger] s[i + 1] == '/'),
{
    theorem_split_join(s);
    let comps = split_components(s);
    lemma_verdict_valid_iff(comps, at_root, 0);
    if at_root {
        assert(component_verdict(comps[0], true) == PathVerdict::Valid);
        lemma_join_starts_with_first(comps);
        if has_drive_prefix(s) {
            // The first component cannot end before the `:` at index one.
            assert(comps[0].len() > 0);
            if comps[0].len() == 1 {
                assert(s[0] == comps[0][0]);
                lemma_separator_after_first(s, comps);
                assert(s[1] == '/');
            } else {
                assert(s[1] == comps[0][1]);
            }
            assert(has_drive_prefix(comps[0]));
        }
    }
    if s[s.len() - 1] == '/' {
        let p = s.drop_last();
        assert(s =~= p + seq!['/'] + Seq::<char>::empty());
        lemma_split_separator(p, Seq::<char>::empty());
        assert(comps.last() =~= Seq::<char>::empty());
        assert(component_verdict(comps[comps.len() - 1], at_root && comps.len() - 1 == 0)
            == PathVerdict::Valid);
    }
    assert forall|i: int| 0 <= i < s.len() - 1 implies !(s[i] == '/' && #[trigger] s[i + 1]
        == '/') by {
        if s[i] == '/' && s[i + 1] == '/' {
            let a = s.take(i);
            let b = s.subrange(i + 1, s.len() as int);
            assert(s =~= a + seq!['/'] + b);
            lemma_split_separator(a, b);
            assert(b =~= seq!['/'] + b.drop_first());
            assert(b =~= Seq::<char>::empty() + seq!['/'] + b.drop_first());
            lemma_split_separator(Seq::<char>::empty(), b.drop_first());
            lemma_split_len(a);
            let x = split_components(a);
            assert(split_components(Seq::<char>::empty()) =~= seq![Seq::<char>::empty()]);
            assert(comps[x.len() as int] =~= Seq::<char>::empty());
            assert(component_verdict(comps[x.len() as int], at_root && x.len() == 0)
                == PathVerdict::Valid);
        }
    }
}

/// In a path whose first component has one scalar, a `/` follows it.
proof fn lemma_separator_after_first(s: Seq<char>, comps: Seq<Seq<char>>)
    requires
        comps == split_components(s),
        comps.len() >= 1,
        comps[0].len() == 1,
        s.len() >= 2,
        join_components(comps) == s,
    ensures
        s[1] == '/',
{
    if comps.len() == 1 {
        assert(s == comps[0]);
    } else {
        lemma_join_second_separator(comps);
    }
}

/// With at least two components, the first separator follows the first one.
proof fn lemma_join_second_separator(comps: Seq<Seq<char>>)
    requires
        comps.len() >= 2,
    ensures
        join_components(comps).len() > comps[0].len(),
        join_components(comps)[comps[0].len() as int] == '/',
    decreases comps.len(),
{
    let init = comps.drop_last();
    if comps.len() == 2 {
        assert(join_components(init) == comps[0]);
    } else {
        lemma_join_second_separator(init);
        assert(init[0] == comps[0]);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    /// The structural rules exactly as `Validator::validate_path_structure`
    /// stated them before they moved into this kernel.
    fn reference(path: &str, at_root: bool) -> PathVerdict {
        if path.is_empty() || path.starts_with('/') || (at_root && path.starts_with('\\')) {
            return PathVerdict::InvalidFormat;
        }
        for (index, component) in path.split('/').enumerate() {
            if component.is_empty() {
                return PathVerdict::InvalidFormat;
            }
            if component == "." || component == ".." {
                return PathVerdict::SpecialDirectory;
            }
            if component.chars().any(|ch| ch <= '\u{1f}' || ch == '\u{7f}') {
                return PathVerdict::InvalidFormat;
            }
            let bytes = component.as_bytes();
            if at_root
                && index == 0
                && bytes.len() >= 2
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
            {
                return PathVerdict::InvalidFormat;
            }
        }
        PathVerdict::Valid
    }

    #[test]
    fn verdicts_cover_every_rule_in_precedence_order() {
        use PathVerdict::{InvalidFormat, SpecialDirectory, Valid};
        let cases: &[(&str, bool, PathVerdict)] = &[
            ("", true, InvalidFormat),
            ("", false, InvalidFormat),
            ("/abs", true, InvalidFormat),
            ("/abs", false, InvalidFormat),
            ("\\root", true, InvalidFormat),
            ("\\literal", false, Valid),
            ("\\/..", true, InvalidFormat),
            ("\\/..", false, SpecialDirectory),
            ("a", true, Valid),
            ("src/main.rs", true, Valid),
            ("a//b", true, InvalidFormat),
            ("a/", true, InvalidFormat),
            (".", true, SpecialDirectory),
            ("..", false, SpecialDirectory),
            ("a/./b", true, SpecialDirectory),
            ("a/../b", true, SpecialDirectory),
            ("...", true, Valid),
            (".a", true, Valid),
            ("a.", true, Valid),
            ("a/.../b", true, Valid),
            ("tab\tname", true, InvalidFormat),
            ("del\u{7f}", true, InvalidFormat),
            ("ok/\u{0}", true, InvalidFormat),
            ("\u{1}/..", true, InvalidFormat),
            ("../\u{1}", true, SpecialDirectory),
            ("a//..", true, InvalidFormat),
            ("C:", true, InvalidFormat),
            ("c:/x", true, InvalidFormat),
            ("C:notes", true, InvalidFormat),
            ("C:", false, Valid),
            ("dir/C:notes", true, Valid),
            ("C:/..", true, InvalidFormat),
            ("1:", true, Valid),
            ("é:", true, Valid),
            ("CC:", true, Valid),
            ("C", true, Valid),
            ("C/:", true, Valid),
            ("目录/résumé|draft(1).txt", true, Valid),
            ("src\\..\\main.rs", true, Valid),
        ];
        for &(path, at_root, expected) in cases {
            assert_eq!(
                check_path_structure(path, at_root),
                expected,
                "{path:?} at_root={at_root}"
            );
            assert_eq!(reference(path, at_root), expected, "{path:?} reference");
        }
    }

    #[test]
    fn verdict_matches_reference_on_every_short_path() {
        let alphabet = ['a', 'C', '.', '/', '\\', ':', '\u{0}', '\u{7f}', 'é', ' '];
        let mut paths = vec![String::new()];
        let mut frontier = vec![String::new()];
        for _ in 0..5 {
            let mut next = Vec::new();
            for prefix in &frontier {
                for ch in alphabet {
                    let mut path = prefix.clone();
                    path.push(ch);
                    next.push(path);
                }
            }
            paths.extend(next.iter().cloned());
            frontier = next;
        }
        for path in &paths {
            for at_root in [false, true] {
                assert_eq!(
                    check_path_structure(path, at_root),
                    reference(path, at_root),
                    "{path:?} at_root={at_root}"
                );
            }
        }
    }
}
