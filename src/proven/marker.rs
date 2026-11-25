//! Verified marker-line classification.
//!
//! This kernel decides whether one complete guide-document line is ordinary
//! Markdown, an exact opening or closing marker, or a malformed marker
//! candidate. Its specification restates the v0.2 marker grammar
//! (`docs/v0.2-contract.md`, "Document and marker grammar") as existential
//! index decompositions of the line, independent of the scanning algorithm,
//! and Verus proves that the executable classifier agrees with it on every
//! byte sequence.
//!
//! The grammar is ASCII apart from the rejected UTF-8 byte-order mark, and
//! UTF-8 never encodes a non-ASCII scalar with an ASCII byte, so classifying
//! the UTF-8 bytes of a line is equivalent to classifying its scalars.

// Verus reasons about array literals element by element but treats
// byte-string literals as opaque, so the literals below stay spelled out.
#![allow(clippy::byte_char_slices)]

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Classification of one complete document line.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkerClass {
    /// Not a marker candidate: ordinary prologue, epilogue, or body text.
    Ordinary,
    /// Exactly `<agentic-navigation-guide>` with optional outer whitespace.
    Opening,
    /// The single permitted `ignore=true` attributed opening marker.
    IgnoredOpening,
    /// Exactly `</agentic-navigation-guide>` with optional outer whitespace.
    Closing,
    /// An opening-marker candidate that matches no permitted production.
    MalformedOpening,
    /// A closing-marker candidate that matches no permitted production.
    MalformedClosing,
}

#[cfg(verus_keep_ghost)]
verus! {

/// `WSP = SP / HTAB`.
pub(crate) open spec fn is_wsp(b: u8) -> bool {
    b == 0x20u8 || b == 0x09u8
}

/// `s[i..j]` is a (possibly empty) run of `WSP`.
pub(crate) open spec fn wsp_run(s: Seq<u8>, i: int, j: int) -> bool {
    0 <= i <= j <= s.len() && forall|k: int| i <= k < j ==> #[trigger] is_wsp(s[k])
}

/// The literal `lit` occurs in `s` starting at index `i`.
pub(crate) open spec fn lit_at(s: Seq<u8>, i: int, lit: Seq<u8>) -> bool {
    0 <= i && i + lit.len() <= s.len() && s.subrange(i, i + lit.len()) == lit
}

/// `<agentic-navigation-guide`
pub(crate) open spec fn opening_name() -> Seq<u8> {
    seq![
        b'<', b'a', b'g', b'e', b'n', b't', b'i', b'c', b'-', b'n', b'a', b'v', b'i',
        b'g', b'a', b't', b'i', b'o', b'n', b'-', b'g', b'u', b'i', b'd', b'e',
    ]
}

/// `</agentic-navigation-guide`
pub(crate) open spec fn closing_name() -> Seq<u8> {
    seq![
        b'<', b'/', b'a', b'g', b'e', b'n', b't', b'i', b'c', b'-', b'n', b'a', b'v',
        b'i', b'g', b'a', b't', b'i', b'o', b'n', b'-', b'g', b'u', b'i', b'd', b'e',
    ]
}

pub(crate) open spec fn ignore_word() -> Seq<u8> {
    seq![b'i', b'g', b'n', b'o', b'r', b'e']
}

/// The attribute value: `true` or `"true"`.
pub(crate) open spec fn true_value(quoted: bool) -> Seq<u8> {
    if quoted {
        seq![b'"', b't', b'r', b'u', b'e', b'"']
    } else {
        seq![b't', b'r', b'u', b'e']
    }
}

/// The UTF-8 encoding of U+FEFF.
pub(crate) open spec fn bom_bytes() -> Seq<u8> {
    seq![0xEFu8, 0xBBu8, 0xBFu8]
}

/// `active-opening-line = outer-ows "<agentic-navigation-guide>" outer-ows`
pub(crate) open spec fn active_opening_line(s: Seq<u8>) -> bool {
    exists|p: int|
        wsp_run(s, 0, p) && lit_at(s, p, opening_name()) && lit_at(s, p + 25, seq![b'>'])
            && wsp_run(s, p + 26, s.len() as int)
}

/// `ignored-opening-line = outer-ows "<agentic-navigation-guide" rws
///  "ignore" ows "=" ows ("true" / DQUOTE "true" DQUOTE) ows ">" outer-ows`
pub(crate) open spec fn ignored_opening_line(s: Seq<u8>) -> bool {
    exists|p: int, q1: int, q2: int, q3: int, quoted: bool, q4: int|
        wsp_run(s, 0, p) && lit_at(s, p, opening_name())
            && wsp_run(s, p + 25, q1) && q1 > p + 25
            && lit_at(s, q1, ignore_word())
            && wsp_run(s, q1 + 6, q2) && lit_at(s, q2, seq![b'='])
            && wsp_run(s, q2 + 1, q3) && lit_at(s, q3, true_value(quoted))
            && wsp_run(s, q3 + true_value(quoted).len(), q4) && lit_at(s, q4, seq![b'>'])
            && wsp_run(s, q4 + 1, s.len() as int)
}

/// `closing-line = outer-ows "</agentic-navigation-guide>" outer-ows`
pub(crate) open spec fn closing_line(s: Seq<u8>) -> bool {
    exists|p: int|
        wsp_run(s, 0, p) && lit_at(s, p, closing_name()) && lit_at(s, p + 26, seq![b'>'])
            && wsp_run(s, p + 27, s.len() as int)
}

/// After removing outer whitespace, the line starts with `name`.
pub(crate) open spec fn candidate(s: Seq<u8>, name: Seq<u8>) -> bool {
    exists|p: int| wsp_run(s, 0, p) && lit_at(s, p, name)
}

/// First index at or after `i` that is not `WSP` (or `s.len()`).
pub(crate) open spec fn skip_wsp(s: Seq<u8>, i: int) -> int
    decreases s.len() - i,
{
    if 0 <= i < s.len() && is_wsp(s[i]) {
        skip_wsp(s, i + 1)
    } else {
        i
    }
}

/// Index after the longest run of `WSP` and byte-order marks starting at `i`.
pub(crate) open spec fn skip_bom_run(s: Seq<u8>, i: int) -> int
    decreases s.len() - i,
{
    let j = skip_wsp(s, i);
    if i <= j && lit_at(s, j, bom_bytes()) {
        skip_bom_run(s, j + 3)
    } else {
        j
    }
}

/// Outer whitespace, a byte-order mark, any run of further byte-order marks
/// and whitespace, and then a marker name. A BOM is never part of a marker,
/// so such a line is a rejected marker candidate rather than Markdown.
pub(crate) open spec fn bom_prefixed(s: Seq<u8>, name: Seq<u8>) -> bool {
    let p = skip_wsp(s, 0);
    lit_at(s, p, bom_bytes()) && lit_at(s, skip_bom_run(s, p + 3), name)
}

/// The contract classification of one complete line.
pub(crate) open spec fn marker_class(s: Seq<u8>) -> MarkerClass {
    if bom_prefixed(s, opening_name()) {
        MarkerClass::MalformedOpening
    } else if bom_prefixed(s, closing_name()) {
        MarkerClass::MalformedClosing
    } else if candidate(s, opening_name()) {
        if active_opening_line(s) {
            MarkerClass::Opening
        } else if ignored_opening_line(s) {
            MarkerClass::IgnoredOpening
        } else {
            MarkerClass::MalformedOpening
        }
    } else if candidate(s, closing_name()) {
        if closing_line(s) {
            MarkerClass::Closing
        } else {
            MarkerClass::MalformedClosing
        }
    } else {
        MarkerClass::Ordinary
    }
}

proof fn lemma_skip_wsp_bounds(s: Seq<u8>, i: int)
    requires
        0 <= i <= s.len(),
    ensures
        wsp_run(s, i, skip_wsp(s, i)),
        skip_wsp(s, i) == s.len() || !is_wsp(s[skip_wsp(s, i)]),
    decreases s.len() - i,
{
    if i < s.len() && is_wsp(s[i]) {
        lemma_skip_wsp_bounds(s, i + 1);
    }
}

/// A whitespace run that ends before a non-whitespace byte is exactly the
/// greedy skip.
proof fn lemma_skip_wsp_unique(s: Seq<u8>, i: int, j: int)
    requires
        wsp_run(s, i, j),
        j == s.len() || !is_wsp(s[j]),
    ensures
        skip_wsp(s, i) == j,
    decreases j - i,
{
    if i < j {
        assert(is_wsp(s[i]));
        lemma_skip_wsp_unique(s, i + 1, j);
    }
}

proof fn lemma_skip_bom_run_bounds(s: Seq<u8>, i: int)
    requires
        0 <= i <= s.len(),
    ensures
        i <= skip_bom_run(s, i) <= s.len(),
        !lit_at(s, skip_bom_run(s, i), bom_bytes()),
        skip_bom_run(s, i) == s.len() || !is_wsp(s[skip_bom_run(s, i)]),
    decreases s.len() - i,
{
    lemma_skip_wsp_bounds(s, i);
    let j = skip_wsp(s, i);
    if i <= j && lit_at(s, j, bom_bytes()) {
        lemma_skip_bom_run_bounds(s, j + 3);
    }
}

} // verus!

/// `WSP = SP / HTAB`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures r == is_wsp(byte)
))]
pub(crate) fn is_marker_wsp(byte: u8) -> bool {
    byte == b' ' || byte == b'\t'
}

/// Return whether `lit` occurs in `line` at `pos`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures r == lit_at(line@, pos as int, lit@)
))]
fn literal_at(line: &[u8], pos: usize, lit: &[u8]) -> bool {
    if pos > line.len() || line.len() - pos < lit.len() {
        return false;
    }
    let mut k: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            k <= lit.len(),
            pos + lit.len() <= line.len(),
            forall|m: int| 0 <= m < k ==> line@[pos + m] == lit@[m],
        decreases lit.len() - k
    ))]
    while k < lit.len() {
        if line[pos + k] != lit[k] {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(line@.subrange(pos as int, pos + lit.len())[k as int] != lit@[k as int]);
            }
            return false;
        }
        k += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(line@.subrange(pos as int, pos + lit.len()) =~= lit@);
    }
    true
}

/// Return the first index at or after `pos` (and below `limit`) that is not
/// `WSP`, or `limit`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        pos <= limit <= line.len(),
    ensures
        pos <= r <= limit,
        wsp_run(line@, pos as int, r as int),
        r == limit || !is_wsp(line@[r as int]),
))]
fn skip_marker_wsp(line: &[u8], pos: usize, limit: usize) -> usize {
    let mut idx = pos;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            pos <= idx <= limit,
            limit <= line.len(),
            wsp_run(line@, pos as int, idx as int),
        decreases limit - idx
    ))]
    while idx < limit && is_marker_wsp(line[idx]) {
        idx += 1;
    }
    idx
}

/// Classify one complete line (without its line ending) against the exact
/// v0.2 marker grammar.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == marker_class(line@),
))]
pub(crate) fn classify_marker_line(line: &[u8]) -> MarkerClass {
    let opening: [u8; 25] = [
        b'<', b'a', b'g', b'e', b'n', b't', b'i', b'c', b'-', b'n', b'a', b'v', b'i', b'g', b'a',
        b't', b'i', b'o', b'n', b'-', b'g', b'u', b'i', b'd', b'e',
    ];
    let closing: [u8; 26] = [
        b'<', b'/', b'a', b'g', b'e', b'n', b't', b'i', b'c', b'-', b'n', b'a', b'v', b'i', b'g',
        b'a', b't', b'i', b'o', b'n', b'-', b'g', b'u', b'i', b'd', b'e',
    ];
    let bom: [u8; 3] = [0xEF, 0xBB, 0xBF];
    let opening = opening.as_slice();
    let closing = closing.as_slice();
    let bom = bom.as_slice();
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(opening@ =~= opening_name());
        assert(closing@ =~= closing_name());
        assert(bom@ =~= bom_bytes());
    }

    let len = line.len();
    let start = skip_marker_wsp(line, 0, len);
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_skip_wsp_unique(line@, 0, start as int);
    }

    if literal_at(line, start, bom) {
        let after = skip_bom_sequence(line, start + 3);
        if literal_at(line, after, opening) {
            return MarkerClass::MalformedOpening;
        }
        if literal_at(line, after, closing) {
            return MarkerClass::MalformedClosing;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_not_candidate_after_bom(line@, start as int, opening_name());
            lemma_not_candidate_after_bom(line@, start as int, closing_name());
        }
        return MarkerClass::Ordinary;
    }

    if literal_at(line, start, opening) {
        return classify_opening_candidate(line, start);
    }

    if literal_at(line, start, closing) {
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_lit_first(line@, start as int, closing_name());
            lemma_not_bom_prefixed(line@, start as int);
            lemma_not_candidate(line@, start as int, opening_name());
            assert(candidate(line@, closing_name()));
        }
        let gt = start + 26;
        if gt < len && line[gt] == b'>' {
            let end = skip_marker_wsp(line, gt + 1, len);
            if end == len {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(line@.subrange(gt as int, gt + 1) =~= seq![b'>']);
                    assert(closing_line(line@));
                }
                return MarkerClass::Closing;
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_not_closing_line(line@, start as int);
        }
        return MarkerClass::MalformedClosing;
    }

    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_not_bom_prefixed(line@, start as int);
        lemma_not_candidate(line@, start as int, opening_name());
        lemma_not_candidate(line@, start as int, closing_name());
    }
    MarkerClass::Ordinary
}

/// Return the index after the longest run of `WSP` and byte-order marks
/// starting at `pos`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        pos <= line.len(),
    ensures
        r == skip_bom_run(line@, pos as int),
        r <= line.len(),
))]
fn skip_bom_sequence(line: &[u8], pos: usize) -> usize {
    let bom: [u8; 3] = [0xEF, 0xBB, 0xBF];
    let bom = bom.as_slice();
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(bom@ =~= bom_bytes());
    }
    let len = line.len();
    let mut idx = pos;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            pos <= idx <= len,
            len == line.len(),
            bom@ == bom_bytes(),
            skip_bom_run(line@, idx as int) == skip_bom_run(line@, pos as int),
        decreases len - idx
    ))]
    loop {
        let next = skip_marker_wsp(line, idx, len);
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_skip_wsp_unique(line@, idx as int, next as int);
        }
        if literal_at(line, next, bom) {
            idx = next + 3;
        } else {
            return next;
        }
    }
}

/// Classify a line whose trimmed content starts with the opening name at
/// `start`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        wsp_run(line@, 0, start as int),
        lit_at(line@, start as int, opening_name()),
        !lit_at(line@, start as int, bom_bytes()),
    ensures
        r == marker_class(line@),
))]
fn classify_opening_candidate(line: &[u8], start: usize) -> MarkerClass {
    let len = line.len();
    let ignore: [u8; 6] = [b'i', b'g', b'n', b'o', b'r', b'e'];
    let quoted_true: [u8; 6] = [b'"', b't', b'r', b'u', b'e', b'"'];
    let bare_true: [u8; 4] = [b't', b'r', b'u', b'e'];
    let ignore = ignore.as_slice();
    let quoted_true = quoted_true.as_slice();
    let bare_true = bare_true.as_slice();
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(ignore@ =~= ignore_word());
        assert(quoted_true@ =~= true_value(true));
        assert(bare_true@ =~= true_value(false));
        lemma_not_bom_prefixed(line@, start as int);
        assert(candidate(line@, opening_name()));
        lemma_closing_excludes_opening(line@, start as int);
    }

    let name_end = start + 25;

    // Bare opening marker.
    if name_end < len && line[name_end] == b'>' {
        let end = skip_marker_wsp(line, name_end + 1, len);
        if end == len {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(line@.subrange(name_end as int, name_end + 1) =~= seq![b'>']);
                assert(active_opening_line(line@));
            }
            return MarkerClass::Opening;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_not_active_opening(line@, start as int);
            lemma_gt_excludes_ignored(line@, start as int);
        }
        return MarkerClass::MalformedOpening;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_not_active_opening(line@, start as int);
    }

    // Attributed opening marker: rws "ignore" ows "=" ows value ows ">" outer-ows.
    let q1 = skip_marker_wsp(line, name_end, len);
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_skip_wsp_unique(line@, name_end as int, q1 as int);
    }
    if q1 == name_end || !literal_at(line, q1, ignore) {
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_not_ignored_at_q1(line@, start as int, q1 as int);
        }
        return MarkerClass::MalformedOpening;
    }
    let q2 = skip_marker_wsp(line, q1 + 6, len);
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_skip_wsp_unique(line@, q1 + 6, q2 as int);
    }
    if q2 >= len || line[q2] != b'=' {
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_not_ignored_at_q2(line@, start as int, q1 as int, q2 as int);
        }
        return MarkerClass::MalformedOpening;
    }
    let q3 = skip_marker_wsp(line, q2 + 1, len);
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_skip_wsp_unique(line@, q2 + 1, q3 as int);
    }
    let value_len: usize = if literal_at(line, q3, quoted_true) {
        6
    } else if literal_at(line, q3, bare_true) {
        4
    } else {
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_not_ignored_at_q3(line@, start as int, q1 as int, q2 as int, q3 as int);
        }
        return MarkerClass::MalformedOpening;
    };
    let q4 = skip_marker_wsp(line, q3 + value_len, len);
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_skip_wsp_unique(line@, q3 + value_len, q4 as int);
    }
    if q4 < len && line[q4] == b'>' {
        let end = skip_marker_wsp(line, q4 + 1, len);
        if end == len {
            #[cfg(verus_keep_ghost)]
            proof! {
                let quoted = value_len == 6;
                assert(true_value(quoted).len() == value_len);
                assert(line@.subrange(q2 as int, q2 + 1) =~= seq![b'=']);
                assert(line@.subrange(q4 as int, q4 + 1) =~= seq![b'>']);
                assert(lit_at(line@, q3 as int, true_value(quoted)));
                assert(wsp_run(line@, 0, start as int) && lit_at(line@, start as int, opening_name())
                    && wsp_run(line@, start + 25, q1 as int) && q1 > start + 25
                    && lit_at(line@, q1 as int, ignore_word())
                    && wsp_run(line@, q1 + 6, q2 as int) && lit_at(line@, q2 as int, seq![b'='])
                    && wsp_run(line@, q2 + 1, q3 as int) && lit_at(line@, q3 as int, true_value(quoted))
                    && wsp_run(line@, q3 + true_value(quoted).len(), q4 as int)
                    && lit_at(line@, q4 as int, seq![b'>'])
                    && wsp_run(line@, q4 + 1, line@.len() as int));
                assert(ignored_opening_line(line@));
            }
            return MarkerClass::IgnoredOpening;
        }
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_not_ignored_at_q4(
            line@,
            start as int,
            q1 as int,
            q2 as int,
            q3 as int,
            value_len == 6,
            q4 as int,
        );
    }
    MarkerClass::MalformedOpening
}

#[cfg(verus_keep_ghost)]
verus! {

/// Any outer-whitespace decomposition that is followed by a byte that is not
/// `WSP` starts at the unique greedy position.
proof fn lemma_prefix_position(s: Seq<u8>, start: int, p: int)
    requires
        wsp_run(s, 0, start),
        start == s.len() || !is_wsp(s[start]),
        wsp_run(s, 0, p),
        p < s.len(),
        !is_wsp(s[p]),
    ensures
        p == start,
{
    if p < start {
        assert(is_wsp(s[p]));
    }
    if p > start {
        assert(is_wsp(s[start]));
    }
}

proof fn lemma_lit_first(s: Seq<u8>, p: int, lit: Seq<u8>)
    requires
        lit_at(s, p, lit),
        lit.len() > 0,
    ensures
        s[p] == lit[0],
        forall|k: int| 0 <= k < lit.len() ==> s[p + k] == #[trigger] lit[k],
{
    assert forall|k: int| 0 <= k < lit.len() implies s[p + k] == #[trigger] lit[k] by {
        assert(s.subrange(p, p + lit.len())[k] == lit[k]);
    }
    assert(s.subrange(p, p + lit.len())[0] == lit[0]);
}

proof fn lemma_not_candidate(s: Seq<u8>, start: int, name: Seq<u8>)
    requires
        wsp_run(s, 0, start),
        start == s.len() || !is_wsp(s[start]),
        !lit_at(s, start, name),
        name.len() > 0,
        !is_wsp(name[0]),
    ensures
        !candidate(s, name),
{
    if candidate(s, name) {
        let p = choose|p: int| wsp_run(s, 0, p) && lit_at(s, p, name);
        lemma_lit_first(s, p, name);
        lemma_prefix_position(s, start, p);
    }
}

/// A line that begins (after outer whitespace) with a byte-order mark is not
/// a plain marker candidate.
proof fn lemma_not_candidate_after_bom(s: Seq<u8>, start: int, name: Seq<u8>)
    requires
        wsp_run(s, 0, start),
        start == s.len() || !is_wsp(s[start]),
        lit_at(s, start, bom_bytes()),
        !lit_at(s, skip_bom_run(s, start + 3), name),
        name.len() > 0,
        name[0] == b'<',
    ensures
        !candidate(s, name),
        !bom_prefixed(s, name),
{
    lemma_lit_first(s, start, bom_bytes());
    lemma_skip_wsp_unique(s, 0, start);
    if lit_at(s, start, name) {
        lemma_lit_first(s, start, name);
    }
    lemma_not_candidate(s, start, name);
}

proof fn lemma_not_bom_prefixed(s: Seq<u8>, start: int)
    requires
        wsp_run(s, 0, start),
        start == s.len() || !is_wsp(s[start]),
        !lit_at(s, start, bom_bytes()),
    ensures
        !bom_prefixed(s, opening_name()),
        !bom_prefixed(s, closing_name()),
{
    lemma_skip_wsp_unique(s, 0, start);
}

proof fn lemma_closing_excludes_opening(s: Seq<u8>, start: int)
    requires
        wsp_run(s, 0, start),
        lit_at(s, start, opening_name()),
    ensures
        !lit_at(s, start, closing_name()),
{
    lemma_lit_first(s, start, opening_name());
    if lit_at(s, start, closing_name()) {
        lemma_lit_first(s, start, closing_name());
        assert(opening_name()[1] == b'a');
        assert(closing_name()[1] == b'/');
    }
}

proof fn lemma_opening_position(s: Seq<u8>, start: int, p: int)
    requires
        wsp_run(s, 0, start),
        lit_at(s, start, opening_name()),
        wsp_run(s, 0, p),
        lit_at(s, p, opening_name()),
    ensures
        p == start,
{
    lemma_lit_first(s, start, opening_name());
    lemma_lit_first(s, p, opening_name());
    lemma_prefix_position(s, start, p);
}

proof fn lemma_not_active_opening(s: Seq<u8>, start: int)
    requires
        wsp_run(s, 0, start),
        lit_at(s, start, opening_name()),
        !(start + 25 < s.len() && s[start + 25] == b'>' && wsp_run(s, start + 26, s.len() as int)),
    ensures
        !active_opening_line(s),
{
    if active_opening_line(s) {
        let p = choose|p: int|
            wsp_run(s, 0, p) && lit_at(s, p, opening_name()) && lit_at(s, p + 25, seq![b'>'])
                && wsp_run(s, p + 26, s.len() as int);
        lemma_opening_position(s, start, p);
        lemma_lit_first(s, p + 25, seq![b'>']);
    }
}

proof fn lemma_gt_excludes_ignored(s: Seq<u8>, start: int)
    requires
        wsp_run(s, 0, start),
        lit_at(s, start, opening_name()),
        start + 25 < s.len(),
        s[start + 25] == b'>',
    ensures
        !ignored_opening_line(s),
{
    if ignored_opening_line(s) {
        let (p, q1, q2, q3, quoted, q4) = choose|p: int, q1: int, q2: int, q3: int, quoted: bool, q4: int|
            wsp_run(s, 0, p) && lit_at(s, p, opening_name())
                && wsp_run(s, p + 25, q1) && q1 > p + 25
                && lit_at(s, q1, ignore_word())
                && wsp_run(s, q1 + 6, q2) && lit_at(s, q2, seq![b'='])
                && wsp_run(s, q2 + 1, q3) && lit_at(s, q3, true_value(quoted))
                && wsp_run(s, q3 + true_value(quoted).len(), q4) && lit_at(s, q4, seq![b'>'])
                && wsp_run(s, q4 + 1, s.len() as int);
        lemma_opening_position(s, start, p);
        assert(is_wsp(s[p + 25]));
    }
}

/// Fixes the attribute positions of any ignored-marker decomposition to the
/// greedy positions computed by the scanner.
proof fn lemma_ignored_positions(
    s: Seq<u8>,
    start: int,
    p: int,
    q1: int,
    q2: int,
    q3: int,
    quoted: bool,
    q4: int,
)
    requires
        wsp_run(s, 0, start),
        lit_at(s, start, opening_name()),
        wsp_run(s, 0, p) && lit_at(s, p, opening_name()),
        wsp_run(s, p + 25, q1) && q1 > p + 25,
        lit_at(s, q1, ignore_word()),
        wsp_run(s, q1 + 6, q2) && lit_at(s, q2, seq![b'=']),
        wsp_run(s, q2 + 1, q3) && lit_at(s, q3, true_value(quoted)),
        wsp_run(s, q3 + true_value(quoted).len(), q4) && lit_at(s, q4, seq![b'>']),
        wsp_run(s, q4 + 1, s.len() as int),
    ensures
        p == start,
        q1 == skip_wsp(s, start + 25),
        q2 == skip_wsp(s, q1 + 6),
        q3 == skip_wsp(s, q2 + 1),
        q4 == skip_wsp(s, q3 + true_value(quoted).len()),
{
    lemma_opening_position(s, start, p);
    lemma_lit_first(s, q1, ignore_word());
    lemma_skip_wsp_unique(s, p + 25, q1);
    lemma_lit_first(s, q2, seq![b'=']);
    lemma_skip_wsp_unique(s, q1 + 6, q2);
    lemma_lit_first(s, q3, true_value(quoted));
    lemma_skip_wsp_unique(s, q2 + 1, q3);
    lemma_lit_first(s, q4, seq![b'>']);
    lemma_skip_wsp_unique(s, q3 + true_value(quoted).len(), q4);
}

proof fn lemma_not_ignored_at_q1(s: Seq<u8>, start: int, q1: int)
    requires
        wsp_run(s, 0, start),
        lit_at(s, start, opening_name()),
        q1 == skip_wsp(s, start + 25),
        q1 == start + 25 || !lit_at(s, q1, ignore_word()),
    ensures
        !ignored_opening_line(s),
{
    if ignored_opening_line(s) {
        let (p, r1, r2, r3, quoted, r4) = choose|p: int, q1: int, q2: int, q3: int, quoted: bool, q4: int|
            wsp_run(s, 0, p) && lit_at(s, p, opening_name())
                && wsp_run(s, p + 25, q1) && q1 > p + 25
                && lit_at(s, q1, ignore_word())
                && wsp_run(s, q1 + 6, q2) && lit_at(s, q2, seq![b'='])
                && wsp_run(s, q2 + 1, q3) && lit_at(s, q3, true_value(quoted))
                && wsp_run(s, q3 + true_value(quoted).len(), q4) && lit_at(s, q4, seq![b'>'])
                && wsp_run(s, q4 + 1, s.len() as int);
        lemma_ignored_positions(s, start, p, r1, r2, r3, quoted, r4);
    }
}

proof fn lemma_not_ignored_at_q2(s: Seq<u8>, start: int, q1: int, q2: int)
    requires
        wsp_run(s, 0, start),
        lit_at(s, start, opening_name()),
        q1 == skip_wsp(s, start + 25),
        q2 == skip_wsp(s, q1 + 6),
        !(q2 < s.len() && s[q2] == b'='),
    ensures
        !ignored_opening_line(s),
{
    if ignored_opening_line(s) {
        let (p, r1, r2, r3, quoted, r4) = choose|p: int, q1: int, q2: int, q3: int, quoted: bool, q4: int|
            wsp_run(s, 0, p) && lit_at(s, p, opening_name())
                && wsp_run(s, p + 25, q1) && q1 > p + 25
                && lit_at(s, q1, ignore_word())
                && wsp_run(s, q1 + 6, q2) && lit_at(s, q2, seq![b'='])
                && wsp_run(s, q2 + 1, q3) && lit_at(s, q3, true_value(quoted))
                && wsp_run(s, q3 + true_value(quoted).len(), q4) && lit_at(s, q4, seq![b'>'])
                && wsp_run(s, q4 + 1, s.len() as int);
        lemma_ignored_positions(s, start, p, r1, r2, r3, quoted, r4);
        lemma_lit_first(s, r2, seq![b'=']);
    }
}

proof fn lemma_not_ignored_at_q3(s: Seq<u8>, start: int, q1: int, q2: int, q3: int)
    requires
        wsp_run(s, 0, start),
        lit_at(s, start, opening_name()),
        q1 == skip_wsp(s, start + 25),
        q2 == skip_wsp(s, q1 + 6),
        q3 == skip_wsp(s, q2 + 1),
        !lit_at(s, q3, true_value(true)),
        !lit_at(s, q3, true_value(false)),
    ensures
        !ignored_opening_line(s),
{
    if ignored_opening_line(s) {
        let (p, r1, r2, r3, quoted, r4) = choose|p: int, q1: int, q2: int, q3: int, quoted: bool, q4: int|
            wsp_run(s, 0, p) && lit_at(s, p, opening_name())
                && wsp_run(s, p + 25, q1) && q1 > p + 25
                && lit_at(s, q1, ignore_word())
                && wsp_run(s, q1 + 6, q2) && lit_at(s, q2, seq![b'='])
                && wsp_run(s, q2 + 1, q3) && lit_at(s, q3, true_value(quoted))
                && wsp_run(s, q3 + true_value(quoted).len(), q4) && lit_at(s, q4, seq![b'>'])
                && wsp_run(s, q4 + 1, s.len() as int);
        lemma_ignored_positions(s, start, p, r1, r2, r3, quoted, r4);
    }
}

proof fn lemma_not_ignored_at_q4(
    s: Seq<u8>,
    start: int,
    q1: int,
    q2: int,
    q3: int,
    quoted_scan: bool,
    q4: int,
)
    requires
        wsp_run(s, 0, start),
        lit_at(s, start, opening_name()),
        q1 == skip_wsp(s, start + 25),
        q2 == skip_wsp(s, q1 + 6),
        q3 == skip_wsp(s, q2 + 1),
        lit_at(s, q3, true_value(quoted_scan)),
        quoted_scan || !lit_at(s, q3, true_value(true)),
        q4 == skip_wsp(s, q3 + true_value(quoted_scan).len()),
        !(q4 < s.len() && s[q4] == b'>' && wsp_run(s, q4 + 1, s.len() as int)),
    ensures
        !ignored_opening_line(s),
{
    if ignored_opening_line(s) {
        let (p, r1, r2, r3, quoted, r4) = choose|p: int, q1: int, q2: int, q3: int, quoted: bool, q4: int|
            wsp_run(s, 0, p) && lit_at(s, p, opening_name())
                && wsp_run(s, p + 25, q1) && q1 > p + 25
                && lit_at(s, q1, ignore_word())
                && wsp_run(s, q1 + 6, q2) && lit_at(s, q2, seq![b'='])
                && wsp_run(s, q2 + 1, q3) && lit_at(s, q3, true_value(quoted))
                && wsp_run(s, q3 + true_value(quoted).len(), q4) && lit_at(s, q4, seq![b'>'])
                && wsp_run(s, q4 + 1, s.len() as int);
        lemma_ignored_positions(s, start, p, r1, r2, r3, quoted, r4);
        // Both value spellings begin differently, so the scanned spelling is
        // the decomposition's spelling.
        lemma_lit_first(s, q3, true_value(quoted_scan));
        lemma_lit_first(s, q3, true_value(quoted));
        assert(quoted == quoted_scan);
        lemma_lit_first(s, r4, seq![b'>']);
    }
}

proof fn lemma_not_closing_line(s: Seq<u8>, start: int)
    requires
        wsp_run(s, 0, start),
        lit_at(s, start, closing_name()),
        !(start + 26 < s.len() && s[start + 26] == b'>' && wsp_run(s, start + 27, s.len() as int)),
    ensures
        !closing_line(s),
{
    if closing_line(s) {
        let p = choose|p: int|
            wsp_run(s, 0, p) && lit_at(s, p, closing_name()) && lit_at(s, p + 26, seq![b'>'])
                && wsp_run(s, p + 27, s.len() as int);
        lemma_lit_first(s, start, closing_name());
        lemma_lit_first(s, p, closing_name());
        lemma_prefix_position(s, start, p);
        lemma_lit_first(s, p + 26, seq![b'>']);
    }
}

} // verus!
