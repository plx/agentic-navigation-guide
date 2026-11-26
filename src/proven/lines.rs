//! Verified physical-line splitting.
//!
//! [`split_lines`] cuts a guide document into the physical lines that the
//! marker, envelope, and layout kernels classify. Its specification,
//! [`physical_lines`], is `str::lines` as the standard library documents and
//! implements it: the text is cut at every LF, which is dropped together with
//! one CR immediately before it; a final LF ends the last line instead of
//! starting an empty one; and empty text has no lines. A CR that does not
//! precede an LF stays in its line.
//!
//! The contract-level meaning (`docs/v0.2-contract.md`, "Document and marker
//! grammar": `line-ending = LF / CRLF`, "CRLF and LF documents have identical
//! meaning", "A lone CR is not a line ending") is proven separately (these
//! proofs exist only in the Verus build):
//!
//! - `theorem_lines_decompose` and `theorem_lines_unique`: the lines are the
//!   only way to write the text as lines without LF, each followed by its
//!   line ending (CRLF, or LF after a line that does not end in CR), except
//!   that a nonempty last line may end the text;
//! - `theorem_lines_without_lone_cr`: when every CR begins a CRLF pair, every
//!   line consists only of `non-line-ending` scalars; with
//!   `theorem_lone_cr_bytes_are_scalars`, `theorem_checked_document_lines`
//!   states this for every document that passes the parser's byte-level
//!   lone-CR check (`document::first_lone_cr_line`);
//! - `theorem_lines_without_cr`: on text without CR the lines are the
//!   LF-separated pieces, without the empty piece after a final LF, which is
//!   the `lines_spec` that `proofs/round_trip.rs` states for `str::lines`.
//!
//! [`is_blank_body`] decides the parser's empty-body rule over the body
//! lines: every line consists only of Unicode `White_Space`, which
//! `theorem_blank_body_is_blank_join` shows is exactly when the lines joined
//! with LF are whitespace-only text (the parser's former test).
//!
//! Trusted glue outside this file: the parser hands the whole decoded
//! document to [`split_lines`], numbers the returned lines `1, 2, ...`, and
//! passes the lines strictly between the envelope's marker lines to
//! [`is_blank_body`] and then, one by one, to the layout kernel.

#[cfg(verus_keep_ghost)]
use crate::proven::document::{is_lone_cr, no_lone_cr};
#[cfg(verus_keep_ghost)]
use crate::proven::layout::all_white_space;
use crate::proven::layout::is_blank;
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

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Specification.
// ---------------------------------------------------------------------------

/// `t` contains no U+000A LINE FEED.
pub(crate) open spec fn lf_free(t: Seq<char>) -> bool {
    forall|k: int| 0 <= k < t.len() ==> #[trigger] t[k] != '\n'
}

/// `t` contains no U+000D CARRIAGE RETURN.
pub(crate) open spec fn cr_free(t: Seq<char>) -> bool {
    forall|k: int| 0 <= k < t.len() ==> #[trigger] t[k] != '\r'
}

/// `t` is nonempty and ends with a carriage return.
pub(crate) open spec fn ends_with_cr(t: Seq<char>) -> bool {
    t.len() > 0 && t.last() == '\r'
}

/// The pieces of `t` between line feeds, as `str::split('\n')` yields them:
/// always at least one, possibly empty.
pub(crate) open spec fn lf_pieces(t: Seq<char>) -> Seq<Seq<char>>
    decreases t.len(),
{
    if t.len() == 0 {
        seq![Seq::<char>::empty()]
    } else {
        let p = lf_pieces(t.drop_last());
        if t.last() == '\n' {
            p.push(Seq::empty())
        } else {
            p.update(p.len() - 1, p.last().push(t.last()))
        }
    }
}

/// A piece that a line feed terminates, without one carriage return before
/// that line feed.
pub(crate) open spec fn strip_cr(piece: Seq<char>) -> Seq<char> {
    if ends_with_cr(piece) {
        piece.drop_last()
    } else {
        piece
    }
}

/// The physical lines of `t`, as `str::lines` yields them: every piece
/// before a line feed without one carriage return before that line feed,
/// then the piece after the last line feed unless it is empty.
pub(crate) open spec fn physical_lines(t: Seq<char>) -> Seq<Seq<char>> {
    let p = lf_pieces(t);
    let n = if t.len() == 0 || t.last() == '\n' {
        p.len() - 1
    } else {
        p.len() as int
    };
    Seq::new(n as nat, |i: int| if i < p.len() - 1 { strip_cr(p[i]) } else { p[i] })
}

// ---------------------------------------------------------------------------
// Declarative contract vocabulary.
// ---------------------------------------------------------------------------

/// `LF`.
pub(crate) open spec fn lf() -> Seq<char> {
    seq!['\n']
}

/// `CRLF`.
pub(crate) open spec fn crlf() -> Seq<char> {
    seq!['\r', '\n']
}

/// Each line followed by its ending, in order.
pub(crate) open spec fn terminated_text(ls: Seq<Seq<char>>, ends: Seq<Seq<char>>) -> Seq<char>
    decreases ls.len(),
{
    if ls.len() == 0 || ends.len() == 0 {
        Seq::empty()
    } else {
        ls[0] + ends[0] + terminated_text(ls.skip(1), ends.skip(1))
    }
}

/// The ending of line `i` of `n` lines is admissible for that line: CRLF,
/// LF after a line that does not end in CR (that CR would belong to the
/// ending), or, for a nonempty last line only, the end of the text.
pub(crate) open spec fn admissible_ending(line: Seq<char>, end: Seq<char>, last: bool) -> bool {
    ||| end == crlf()
    ||| end == lf() && !ends_with_cr(line)
    ||| last && end == Seq::<char>::empty() && line.len() > 0
}

/// `t` is the lines `ls`, line `i` followed by `ends[i]`, where no line
/// contains LF and every ending is admissible.
pub(crate) open spec fn line_decomposition(
    t: Seq<char>,
    ls: Seq<Seq<char>>,
    ends: Seq<Seq<char>>,
) -> bool {
    &&& ls.len() == ends.len()
    &&& t == terminated_text(ls, ends)
    &&& forall|i: int| 0 <= i < ls.len() ==> lf_free(#[trigger] ls[i])
    &&& forall|i: int|
        0 <= i < ls.len() ==> admissible_ending(#[trigger] ls[i], ends[i], i == ls.len() - 1)
}

/// Character `i` of `t` is a carriage return that does not begin a CRLF
/// pair.
pub(crate) open spec fn is_lone_cr_char(t: Seq<char>, i: int) -> bool {
    0 <= i < t.len() && t[i] == '\r' && !(i + 1 < t.len() && t[i + 1] == '\n')
}

/// Every carriage return in `t` begins a CRLF pair.
pub(crate) open spec fn no_lone_cr_char(t: Seq<char>) -> bool {
    forall|i: int| 0 <= i < t.len() ==> !#[trigger] is_lone_cr_char(t, i)
}

// ---------------------------------------------------------------------------
// Pieces and lines.
// ---------------------------------------------------------------------------

proof fn lemma_pieces_nonempty(t: Seq<char>)
    ensures
        lf_pieces(t).len() >= 1,
    decreases t.len(),
{
    if t.len() > 0 {
        lemma_pieces_nonempty(t.drop_last());
    }
}

/// Text without a line feed is one piece.
proof fn lemma_pieces_lf_free(b: Seq<char>)
    requires
        lf_free(b),
    ensures
        lf_pieces(b) == seq![b],
    decreases b.len(),
{
    if b.len() == 0 {
        assert(b =~= Seq::<char>::empty());
    } else {
        lemma_pieces_lf_free(b.drop_last());
        assert(b.last() == b[b.len() - 1]);
        assert(b.drop_last().push(b.last()) =~= b);
        assert(seq![b.drop_last()].update(0, b.drop_last().push(b.last())) =~= seq![b]);
    }
}

/// A line feed separates the pieces of the text before it from those of the
/// text after it.
proof fn lemma_pieces_append(a: Seq<char>, b: Seq<char>)
    ensures
        lf_pieces(a + seq!['\n'] + b) == lf_pieces(a) + lf_pieces(b),
    decreases b.len(),
{
    let t = a + seq!['\n'] + b;
    if b.len() == 0 {
        assert(t.drop_last() =~= a);
        assert(t.last() == '\n');
        assert(lf_pieces(a).push(Seq::<char>::empty()) =~= lf_pieces(a) + seq![
            Seq::<char>::empty(),
        ]);
    } else {
        assert(t.drop_last() =~= a + seq!['\n'] + b.drop_last());
        assert(t.last() == b.last());
        lemma_pieces_append(a, b.drop_last());
        lemma_pieces_nonempty(b.drop_last());
        let x = lf_pieces(a);
        let y = lf_pieces(b.drop_last());
        if b.last() == '\n' {
            assert((x + y).push(Seq::<char>::empty()) =~= x + y.push(Seq::<char>::empty()));
        } else {
            assert((x + y).last() == y.last());
            assert((x + y).update((x + y).len() - 1, y.last().push(b.last())) =~= x + y.update(
                y.len() - 1,
                y.last().push(b.last()),
            ));
        }
    }
}

/// Empty text has no lines.
proof fn lemma_lines_empty(t: Seq<char>)
    requires
        t.len() == 0,
    ensures
        physical_lines(t) == Seq::<Seq<char>>::empty(),
{
    assert(physical_lines(t) =~= Seq::<Seq<char>>::empty());
}

/// Nonempty text without a line feed is one line.
proof fn lemma_lines_single(t: Seq<char>)
    requires
        t.len() > 0,
        lf_free(t),
    ensures
        physical_lines(t) == seq![t],
{
    lemma_pieces_lf_free(t);
    assert(t.last() == t[t.len() - 1]);
    assert(physical_lines(t) =~= seq![t]);
}

/// The first line of text whose first line feed follows `raw`.
proof fn lemma_lines_step(raw: Seq<char>, after: Seq<char>)
    requires
        lf_free(raw),
    ensures
        physical_lines(raw + seq!['\n'] + after) == seq![strip_cr(raw)] + physical_lines(after),
{
    let t = raw + seq!['\n'] + after;
    lemma_pieces_append(raw, after);
    lemma_pieces_lf_free(raw);
    lemma_pieces_nonempty(after);
    let p = lf_pieces(t);
    let q = lf_pieces(after);
    assert(p =~= seq![raw] + q);
    if after.len() > 0 {
        assert(t.last() == after.last());
    } else {
        assert(t.last() == '\n');
    }
    let lhs = physical_lines(t);
    let rhs = seq![strip_cr(raw)] + physical_lines(after);
    assert(lhs.len() == rhs.len());
    assert forall|i: int| 0 <= i < lhs.len() implies lhs[i] == rhs[i] by {
        if i > 0 {
            assert(p[i] == q[i - 1]);
        }
    }
    assert(lhs =~= rhs);
}

/// Every piece of carriage-return-free text is carriage-return free.
proof fn lemma_pieces_cr_free(t: Seq<char>)
    requires
        cr_free(t),
    ensures
        forall|i: int| 0 <= i < lf_pieces(t).len() ==> cr_free(#[trigger] lf_pieces(t)[i]),
    decreases t.len(),
{
    if t.len() > 0 {
        let s = t.drop_last();
        assert(cr_free(s)) by {
            assert forall|k: int| 0 <= k < s.len() implies #[trigger] s[k] != '\r' by {
                assert(s[k] == t[k]);
            }
        }
        lemma_pieces_cr_free(s);
        lemma_pieces_nonempty(s);
        let p = lf_pieces(s);
        assert(t.last() == t[t.len() - 1]);
        if t.last() != '\n' {
            let last = p.last().push(t.last());
            assert(cr_free(p[p.len() - 1]));
            assert forall|k: int| 0 <= k < last.len() implies #[trigger] last[k] != '\r' by {
                if k < last.len() - 1 {
                    assert(last[k] == p[p.len() - 1][k]);
                }
            }
            assert forall|i: int| 0 <= i < lf_pieces(t).len() implies cr_free(
                #[trigger] lf_pieces(t)[i],
            ) by {
                if i < p.len() - 1 {
                    assert(lf_pieces(t)[i] == p[i]);
                }
            }
        } else {
            assert forall|i: int| 0 <= i < lf_pieces(t).len() implies cr_free(
                #[trigger] lf_pieces(t)[i],
            ) by {
                if i < p.len() {
                    assert(lf_pieces(t)[i] == p[i]);
                }
            }
        }
    }
}

/// Text without a line feed before index `k`, where a line feed is: `t`
/// splits at its first line feed, or has none.
proof fn lemma_first_lf(t: Seq<char>) -> (k: int)
    ensures
        lf_free(t) ==> k == t.len(),
        !lf_free(t) ==> 0 <= k < t.len() && t[k] == '\n' && lf_free(t.take(k)),
    decreases t.len(),
{
    if t.len() == 0 {
        0
    } else {
        let s = t.drop_last();
        let j = lemma_first_lf(s);
        if !lf_free(s) {
            assert(t.take(j) =~= s.take(j));
            assert(t[j] == s[j]);
            j
        } else if t.last() == '\n' {
            assert(t.take(s.len() as int) =~= s);
            (t.len() - 1) as int
        } else {
            assert forall|k: int| 0 <= k < t.len() implies #[trigger] t[k] != '\n' by {
                if k < s.len() {
                    assert(t[k] == s[k]);
                }
            }
            t.len() as int
        }
    }
}

/// The text from the first line feed on, split into what comes before it,
/// the line feed, and what follows.
proof fn lemma_split_at_first_lf(t: Seq<char>, k: int)
    requires
        0 <= k < t.len(),
        t[k] == '\n',
    ensures
        t == t.take(k) + seq!['\n'] + t.skip(k + 1),
{
    assert(t =~= t.take(k) + seq!['\n'] + t.skip(k + 1));
}

proof fn lemma_terminated_cons(ls: Seq<Seq<char>>, ends: Seq<Seq<char>>)
    requires
        ls.len() > 0,
        ends.len() > 0,
    ensures
        terminated_text(ls, ends) == ls[0] + ends[0] + terminated_text(ls.skip(1), ends.skip(1)),
{
}

// ---------------------------------------------------------------------------
// Contract-level theorems.
// ---------------------------------------------------------------------------

/// The text is its physical lines, each followed by an admissible ending:
/// CRLF, or LF after a line that does not end in CR, except that a nonempty
/// last line may end the text.
pub(crate) proof fn theorem_lines_decompose(t: Seq<char>) -> (ends: Seq<Seq<char>>)
    ensures
        line_decomposition(t, physical_lines(t), ends),
    decreases t.len(),
{
    let ls = physical_lines(t);
    if t.len() == 0 {
        lemma_lines_empty(t);
        let ends = Seq::<Seq<char>>::empty();
        assert(terminated_text(ls, ends) =~= t);
        ends
    } else if lf_free(t) {
        lemma_lines_single(t);
        let ends = seq![Seq::<char>::empty()];
        assert(ls.skip(1) =~= Seq::<Seq<char>>::empty());
        assert(terminated_text(ls.skip(1), ends.skip(1)) =~= Seq::<char>::empty());
        assert(terminated_text(ls, ends) =~= t);
        ends
    } else {
        let k = lemma_first_lf(t);
        let raw = t.take(k);
        let after = t.skip(k + 1);
        lemma_split_at_first_lf(t, k);
        lemma_lines_step(raw, after);
        let rest = theorem_lines_decompose(after);
        let la = physical_lines(after);
        let line = strip_cr(raw);
        let end = if ends_with_cr(raw) {
            crlf()
        } else {
            lf()
        };
        let ends = seq![end] + rest;
        assert(ls == seq![line] + la);
        assert(ls.skip(1) =~= la);
        assert(ends.skip(1) =~= rest);
        if ends_with_cr(raw) {
            assert(raw =~= line + seq!['\r']);
            assert(line + end =~= raw + seq!['\n']);
        } else {
            assert(line + end =~= raw + seq!['\n']);
        }
        assert(terminated_text(ls, ends) == line + end + terminated_text(la, rest));
        assert(t =~= raw + seq!['\n'] + after);
        assert(terminated_text(ls, ends) =~= t);
        assert(lf_free(line)) by {
            assert forall|j: int| 0 <= j < line.len() implies #[trigger] line[j] != '\n' by {
                assert(line[j] == raw[j]);
            }
        }
        assert forall|i: int| 0 <= i < ls.len() implies lf_free(#[trigger] ls[i]) by {
            if i > 0 {
                assert(ls[i] == la[i - 1]);
            }
        }
        assert forall|i: int| 0 <= i < ls.len() implies admissible_ending(
            #[trigger] ls[i],
            ends[i],
            i == ls.len() - 1,
        ) by {
            if i > 0 {
                assert(ls[i] == la[i - 1]);
                assert(ends[i] == rest[i - 1]);
                assert(admissible_ending(la[i - 1], rest[i - 1], i - 1 == la.len() - 1));
            }
        }
        ends
    }
}

/// The physical lines are the only decomposition of the text into lines
/// without LF and admissible endings, so [`physical_lines`] is determined by
/// [`line_decomposition`] alone.
pub(crate) proof fn theorem_lines_unique(t: Seq<char>, ls: Seq<Seq<char>>, ends: Seq<Seq<char>>)
    requires
        line_decomposition(t, ls, ends),
    ensures
        ls == physical_lines(t),
    decreases ls.len(),
{
    if ls.len() == 0 {
        assert(t.len() == 0);
        lemma_lines_empty(t);
        assert(ls =~= physical_lines(t));
    } else {
        let line = ls[0];
        let end = ends[0];
        let tl = ls.skip(1);
        let te = ends.skip(1);
        let rest = terminated_text(tl, te);
        lemma_terminated_cons(ls, ends);
        assert(lf_free(ls[0]));
        assert(admissible_ending(ls[0], ends[0], 0 == ls.len() - 1));
        assert forall|i: int| 0 <= i < tl.len() implies lf_free(#[trigger] tl[i]) by {
            assert(tl[i] == ls[i + 1]);
        }
        assert forall|i: int| 0 <= i < tl.len() implies admissible_ending(
            #[trigger] tl[i],
            te[i],
            i == tl.len() - 1,
        ) by {
            assert(tl[i] == ls[i + 1]);
            assert(te[i] == ends[i + 1]);
            assert(admissible_ending(ls[i + 1], ends[i + 1], i + 1 == ls.len() - 1));
        }
        if end == Seq::<char>::empty() {
            // Only a nonempty last line may end the text.
            assert(ls.len() == 1);
            assert(tl.len() == 0);
            assert(rest =~= Seq::<char>::empty());
            assert(t =~= line);
            lemma_lines_single(t);
            assert(ls =~= seq![line]);
        } else {
            theorem_lines_unique(rest, tl, te);
            let raw = if end == crlf() {
                line + seq!['\r']
            } else {
                line
            };
            assert(raw + seq!['\n'] =~= line + end);
            assert(t =~= raw + seq!['\n'] + rest);
            assert(lf_free(raw)) by {
                assert forall|j: int| 0 <= j < raw.len() implies #[trigger] raw[j] != '\n' by {
                    if j < line.len() {
                        assert(raw[j] == line[j]);
                    }
                }
            }
            lemma_lines_step(raw, rest);
            if end == crlf() {
                assert(raw.drop_last() =~= line);
            }
            assert(strip_cr(raw) == line);
            assert(ls =~= seq![line] + tl);
        }
    }
}

/// When every carriage return begins a CRLF pair, which the parser's
/// lone-CR check establishes before it splits a document, every physical
/// line consists only of `non-line-ending` scalars: it contains neither CR
/// nor LF.
pub(crate) proof fn theorem_lines_without_lone_cr(t: Seq<char>)
    requires
        no_lone_cr_char(t),
    ensures
        forall|i: int|
            0 <= i < physical_lines(t).len() ==> lf_free(#[trigger] physical_lines(t)[i])
                && cr_free(physical_lines(t)[i]),
{
    let ends = theorem_lines_decompose(t);
    lemma_decomposition_cr_free(t, physical_lines(t), ends);
}

/// The lines of a decomposition of text without a lone carriage return
/// contain no carriage return.
proof fn lemma_decomposition_cr_free(t: Seq<char>, ls: Seq<Seq<char>>, ends: Seq<Seq<char>>)
    requires
        line_decomposition(t, ls, ends),
        no_lone_cr_char(t),
    ensures
        forall|i: int| 0 <= i < ls.len() ==> lf_free(#[trigger] ls[i]) && cr_free(ls[i]),
    decreases ls.len(),
{
    if ls.len() > 0 {
        let line = ls[0];
        let end = ends[0];
        let tl = ls.skip(1);
        let te = ends.skip(1);
        let rest = terminated_text(tl, te);
        lemma_terminated_cons(ls, ends);
        assert(t == line + end + rest);
        assert(admissible_ending(ls[0], ends[0], 0 == ls.len() - 1));
        assert(lf_free(ls[0]));
        // A carriage return in the first line is followed, in the text, by
        // the next scalar of the line, by the line ending, or by nothing.
        assert forall|j: int| 0 <= j < line.len() implies #[trigger] line[j] != '\r' by {
            assert(t[j] == line[j]);
            assert(!is_lone_cr_char(t, j));
            if line[j] == '\r' {
                if j + 1 < line.len() {
                    assert(t[j + 1] == line[j + 1]);
                } else if end == crlf() {
                    assert(t[j + 1] == '\r');
                } else if end == lf() {
                    assert(line.last() == line[j]);
                } else {
                    assert(t.len() == line.len());
                }
            }
        }
        // The rest of the text has no lone carriage return either.
        let off = (line.len() + end.len()) as int;
        assert forall|k: int| 0 <= k < rest.len() implies !#[trigger] is_lone_cr_char(rest, k) by {
            assert(t[off + k] == rest[k]);
            assert(!is_lone_cr_char(t, off + k));
            if k + 1 < rest.len() {
                assert(t[off + k + 1] == rest[k + 1]);
            }
        }
        assert forall|i: int| 0 <= i < tl.len() implies lf_free(#[trigger] tl[i]) by {
            assert(tl[i] == ls[i + 1]);
        }
        assert forall|i: int| 0 <= i < tl.len() implies admissible_ending(
            #[trigger] tl[i],
            te[i],
            i == tl.len() - 1,
        ) by {
            assert(tl[i] == ls[i + 1]);
            assert(te[i] == ends[i + 1]);
            assert(admissible_ending(ls[i + 1], ends[i + 1], i + 1 == ls.len() - 1));
        }
        if end == Seq::<char>::empty() {
            assert(tl.len() == 0);
        }
        lemma_decomposition_cr_free(rest, tl, te);
        assert forall|i: int| 0 <= i < ls.len() implies lf_free(#[trigger] ls[i]) && cr_free(
            ls[i],
        ) by {
            if i > 0 {
                assert(ls[i] == tl[i - 1]);
            }
        }
    }
}

/// The parser's byte-level lone-CR check
/// ([`crate::proven::document::first_lone_cr_line`], which returns `None`
/// exactly under `no_lone_cr`) on the UTF-8 encoding of a text is the
/// scalar-level rule: CR and LF are ASCII, and UTF-8 encodes no other scalar
/// with an ASCII byte.
pub(crate) proof fn theorem_lone_cr_bytes_are_scalars(t: Seq<char>)
    requires
        no_lone_cr(encode_utf8(t)),
    ensures
        no_lone_cr_char(t),
{
    let bytes = encode_utf8(t);
    assert forall|i: int| 0 <= i < t.len() implies !#[trigger] is_lone_cr_char(t, i) by {
        if t[i] == '\r' {
            let rest = t.skip(i);
            let tail = rest.drop_first();
            let b = encode_utf8(t.take(i)).len() as int;
            lemma_encode_split(t, i);
            assert(rest[0] == '\r');
            assert(encode_utf8(rest) == encode_scalar(rest[0] as u32) + encode_utf8(tail));
            assert(13u32 & 0x7F == 13u32) by (bit_vector);
            assert(encode_scalar('\r' as u32) =~= seq![13u8]);
            assert(bytes[b] == 13u8);
            assert(!is_lone_cr(bytes, b));
            assert(bytes[b + 1] == encode_utf8(tail)[0]);
            if tail.len() == 0 {
                assert(encode_utf8(tail) =~= Seq::<u8>::empty());
            }
            assert(tail.len() > 0);
            encode_utf8_first_scalar(tail);
            assert(10u8 & 0x7F == 10u8) by (bit_vector);
            assert(tail[0] as u32 == 10u32);
            char_u32_cast(tail[0], 10u32);
            char_u32_cast('\n', 10u32);
            assert(t[i + 1] == tail[0]);
        }
    }
}

/// A document that passes the parser's lone-CR check splits into physical
/// lines that consist only of `non-line-ending` scalars.
pub(crate) proof fn theorem_checked_document_lines(t: Seq<char>)
    requires
        no_lone_cr(encode_utf8(t)),
    ensures
        forall|i: int|
            0 <= i < physical_lines(t).len() ==> lf_free(#[trigger] physical_lines(t)[i])
                && cr_free(physical_lines(t)[i]),
{
    theorem_lone_cr_bytes_are_scalars(t);
    theorem_lines_without_lone_cr(t);
}

/// On text without a carriage return, the physical lines are the pieces
/// between line feeds, without the empty piece after a final line feed:
/// the `lines_spec` that `proofs/round_trip.rs` states for `str::lines`.
pub(crate) proof fn theorem_lines_without_cr(t: Seq<char>)
    requires
        cr_free(t),
    ensures
        physical_lines(t) == if t.len() == 0 {
            Seq::<Seq<char>>::empty()
        } else if t.last() == '\n' {
            lf_pieces(t).drop_last()
        } else {
            lf_pieces(t)
        },
{
    lemma_pieces_cr_free(t);
    lemma_pieces_nonempty(t);
    let p = lf_pieces(t);
    assert forall|i: int| 0 <= i < p.len() implies strip_cr(#[trigger] p[i]) == p[i] by {
        if p[i].len() > 0 {
            assert(cr_free(p[i]));
            assert(p[i].last() == p[i][p[i].len() - 1]);
        }
    }
    if t.len() == 0 {
        lemma_lines_empty(t);
    } else if t.last() == '\n' {
        assert(physical_lines(t) =~= p.drop_last());
    } else {
        assert(physical_lines(t) =~= p);
    }
}

/// `lines.join("\n")`.
pub(crate) open spec fn join_lf(lines: Seq<Seq<char>>) -> Seq<char>
    decreases lines.len(),
{
    if lines.len() == 0 {
        Seq::empty()
    } else if lines.len() == 1 {
        lines[0]
    } else {
        join_lf(lines.drop_last()) + seq!['\n'] + lines.last()
    }
}

proof fn lemma_white_space_concat(a: Seq<char>, b: Seq<char>)
    ensures
        all_white_space(a + b) <==> all_white_space(a) && all_white_space(b),
{
    let c = a + b;
    if all_white_space(a) && all_white_space(b) {
        assert forall|k: int| 0 <= k < c.len() implies is_white_space(#[trigger] c[k]) by {
            if k < a.len() {
                assert(c[k] == a[k]);
            } else {
                assert(c[k] == b[k - a.len()]);
            }
        }
    }
    if all_white_space(c) {
        assert forall|k: int| 0 <= k < a.len() implies is_white_space(#[trigger] a[k]) by {
            assert(c[k] == a[k]);
        }
        assert forall|k: int| 0 <= k < b.len() implies is_white_space(#[trigger] b[k]) by {
            assert(c[a.len() + k] == b[k]);
        }
    }
}

/// The lines joined with LF are whitespace-only text exactly when every
/// line is, because LF is `White_Space`; in particular no lines join to
/// empty, blank text. The parser's empty-body rule ([`is_blank_body`]) is
/// therefore the rule it applied to the joined body before this kernel.
pub(crate) proof fn theorem_blank_body_is_blank_join(ls: Seq<Seq<char>>)
    ensures
        all_white_space(join_lf(ls)) <==> forall|i: int|
            0 <= i < ls.len() ==> all_white_space(#[trigger] ls[i]),
    decreases ls.len(),
{
    if ls.len() == 0 {
        assert(join_lf(ls) =~= Seq::<char>::empty());
    } else if ls.len() == 1 {
    } else {
        let p = ls.drop_last();
        theorem_blank_body_is_blank_join(p);
        lemma_white_space_concat(join_lf(p) + seq!['\n'], ls.last());
        lemma_white_space_concat(join_lf(p), seq!['\n']);
        assert(is_white_space(seq!['\n'][0]));
        assert(ls.last() == ls[ls.len() - 1]);
        assert forall|i: int| 0 <= i < p.len() implies #[trigger] p[i] == ls[i] by {}
        if forall|i: int| 0 <= i < ls.len() ==> all_white_space(#[trigger] ls[i]) {
            assert forall|i: int| 0 <= i < p.len() implies all_white_space(#[trigger] p[i]) by {
                assert(all_white_space(ls[i]));
            }
            assert(all_white_space(ls[ls.len() - 1]));
        }
        if (forall|i: int| 0 <= i < p.len() ==> all_white_space(#[trigger] p[i]))
            && all_white_space(ls.last()) {
            assert forall|i: int| 0 <= i < ls.len() implies all_white_space(#[trigger] ls[i]) by {
                if i < p.len() {
                    assert(all_white_space(p[i]));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// UTF-8 lemmas for slicing at scalar boundaries.
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

} // verus!

/// Split the first physical line off nonempty `text`: return the line
/// without its line ending and the text after that ending (empty when the
/// line ends the text).
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        text@.len() > 0,
    ensures
        r.1@.len() < text@.len(),
        physical_lines(text@) == seq![r.0@] + physical_lines(r.1@),
))]
fn split_first_line(text: &str) -> (&str, &str) {
    // The slice length bounds the encoded length by `usize::MAX`, which rules
    // out overflow below. Only the proof reads it; it is computed in every
    // build so verified and shipped code stay identical. `str::len` would not
    // do: Verus specifies it as a truncating cast of the encoded length.
    #[allow(clippy::needless_as_bytes)]
    let _total_len = text.as_bytes().len();
    let mut chars = text.chars();
    // The byte offset of the next scalar, and the byte offset where the line
    // ends if that scalar is a line feed: one carriage return before the
    // line feed belongs to the line ending.
    let mut offset: usize = 0;
    let mut line_end: usize = 0;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost mut count: int = 0int; }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            text@.len() > 0,
            0 <= count <= text@.len(),
            chars.remaining() == text@.skip(count),
            lf_free(text@.take(count)),
            offset == encode_utf8(text@.take(count)).len(),
            ends_with_cr(text@.take(count)) ==> count > 0
                && line_end == encode_utf8(text@.take(count - 1)).len(),
            !ends_with_cr(text@.take(count)) ==> line_end == offset,
            _total_len == encode_utf8(text@).len(),
        decreases text@.len() - count
    ))]
    loop {
        match chars.next() {
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(text@.take(count) =~= text@);
                    lemma_prefix_char_boundary(text@, count);
                    lemma_lines_single(text@);
                }
                let (line, after) = text.split_at(offset);
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_prefix_view(text@, count, line@);
                    lemma_suffix_view(text@, count, after@);
                    assert(after@ =~= Seq::<char>::empty());
                    lemma_lines_empty(after@);
                    assert(seq![line@] + physical_lines(after@) =~= seq![text@]);
                }
                return (line, after);
            }
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(text@.skip(count)[0] == text@[count]);
                    assert(text@.skip(count).drop_first() =~= text@.skip(count + 1));
                    lemma_encode_take_push(text@, count);
                    lemma_encode_split(text@, count + 1);
                }
                let width = ch.len_utf8();
                if ch == '\n' {
                    #[cfg(verus_keep_ghost)]
                    proof_decl! {
                        let ghost raw: Seq<char> = text@.take(count);
                        let ghost kept: int = if ends_with_cr(raw) { count - 1 } else { count };
                    }
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_prefix_char_boundary(text@, kept);
                        lemma_prefix_char_boundary(text@, count + 1);
                    }
                    let (line, _) = text.split_at(line_end);
                    let (_, after) = text.split_at(offset + width);
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_prefix_view(text@, kept, line@);
                        lemma_suffix_view(text@, count + 1, after@);
                        if ends_with_cr(raw) {
                            assert(raw.drop_last() =~= text@.take(count - 1));
                        }
                        assert(line@ == strip_cr(raw));
                        lemma_split_at_first_lf(text@, count);
                        lemma_lines_step(raw, after@);
                    }
                    return (line, after);
                }
                line_end = if ch == '\r' { offset } else { offset + width };
                offset += width;
                #[cfg(verus_keep_ghost)]
                proof! {
                    let next = text@.take(count + 1);
                    assert(next =~= text@.take(count).push(ch));
                    assert(next.drop_last() =~= text@.take(count));
                    assert forall|k: int| 0 <= k < next.len() implies #[trigger] next[k] != '\n' by {
                        if k < count {
                            assert(next[k] == text@.take(count)[k]);
                        }
                    }
                    count = count + 1;
                }
            }
        }
    }
}

/// Split `text` into its physical lines, exactly as `str::lines` does: at
/// every LF, dropping the LF and one CR immediately before it, with no empty
/// line after a final LF.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@.len() == physical_lines(text@).len(),
        forall|i: int| 0 <= i < r@.len() ==> #[trigger] r@[i]@ == physical_lines(text@)[i],
))]
pub(crate) fn split_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = Vec::new();
    let mut rest = text;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            lines@.len() + physical_lines(rest@).len() == physical_lines(text@).len(),
            forall|i: int| 0 <= i < lines@.len() ==> #[trigger] lines@[i]@
                == physical_lines(text@)[i],
            forall|j: int| 0 <= j < physical_lines(rest@).len() ==> #[trigger] physical_lines(
                rest@,
            )[j] == physical_lines(text@)[lines@.len() + j],
        decreases rest@.len()
    ))]
    while !rest.is_empty() {
        let (line, after) = split_first_line(rest);
        #[cfg(verus_keep_ghost)]
        proof! {
            let pr = physical_lines(rest@);
            let pa = physical_lines(after@);
            assert(pr == seq![line@] + pa);
            assert(pr[0] == line@);
            assert forall|j: int| 0 <= j < pa.len() implies #[trigger] pa[j] == physical_lines(
                text@,
            )[lines@.len() + 1 + j] by {
                assert(pa[j] == pr[j + 1]);
            }
        }
        lines.push(line);
        rest = after;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_lines_empty(rest@);
    }
    lines
}

/// Return whether the body `lines` is empty text: every line consists only
/// of Unicode `White_Space` scalars, which holds when there is no line.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == forall|i: int| 0 <= i < lines@.len() ==> all_white_space(#[trigger] lines@[i]@),
))]
pub(crate) fn is_blank_body(lines: &[&str]) -> bool {
    let mut idx: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            idx <= lines@.len(),
            forall|i: int| 0 <= i < idx ==> all_white_space(#[trigger] lines@[i]@),
        decreases lines@.len() - idx
    ))]
    while idx < lines.len() {
        if !is_blank(lines[idx]) {
            return false;
        }
        idx += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_like_str_lines_on_examples() {
        for text in [
            "",
            "a",
            "\n",
            "\r\n",
            "a\n",
            "a\r\n",
            "a\nb",
            "a\r\nb\r\n",
            "\n\n",
            "a\n\nb\n",
            "a\r",
            "\r",
            "a\r\r\n",
            "a\n\r",
            "\u{e9}\r\n\u{3000}\n\u{1F600}",
            "<agentic-navigation-guide>\r\n- a\r\n</agentic-navigation-guide>\r\n",
        ] {
            assert_eq!(
                split_lines(text),
                text.lines().collect::<Vec<_>>(),
                "{text:?}"
            );
        }
    }

    #[test]
    fn split_first_line_returns_the_line_and_the_text_after_its_ending() {
        assert_eq!(split_first_line("a"), ("a", ""));
        assert_eq!(split_first_line("a\nb"), ("a", "b"));
        assert_eq!(split_first_line("a\r\nb"), ("a", "b"));
        assert_eq!(split_first_line("a\r\r\nb"), ("a\r", "b"));
        assert_eq!(split_first_line("\n"), ("", ""));
        assert_eq!(split_first_line("\r"), ("\r", ""));
        assert_eq!(split_first_line("\u{e9}\r\u{e9}\n"), ("\u{e9}\r\u{e9}", ""));
    }

    /// Every string of length at most 10 over `a`, CR, LF, and a two-byte
    /// scalar (lone carriage returns included) splits exactly as
    /// `str::lines` splits it.
    #[test]
    fn splits_like_str_lines_on_every_short_string() {
        let alphabet = ['a', '\r', '\n', '\u{e9}'];
        let mut text = String::new();
        for len in 0..=10u32 {
            for mut code in 0..4usize.pow(len) {
                text.clear();
                for _ in 0..len {
                    text.push(alphabet[code % 4]);
                    code /= 4;
                }
                assert_eq!(
                    split_lines(&text),
                    text.lines().collect::<Vec<_>>(),
                    "{text:?}"
                );
            }
        }
    }

    #[test]
    fn blank_body_is_the_blank_joined_body() {
        for lines in [
            &[][..],
            &[""][..],
            &["", " \t", "\u{3000}"][..],
            &["- a"][..],
            &["", "- a"][..],
            &[" ", "x", " "][..],
        ] {
            assert_eq!(
                is_blank_body(lines),
                is_blank(&lines.join("\n")),
                "{lines:?}"
            );
        }
    }

    /// The returned lines are sub-slices of the input, as `str::lines`
    /// returns them, so callers can rely on their addresses.
    #[test]
    fn lines_borrow_from_the_input() {
        let text = "ab\r\ncd\nef";
        let range = text.as_bytes().as_ptr_range();
        for line in split_lines(text) {
            let start = line.as_ptr();
            assert!(range.start <= start && start <= range.end);
        }
    }
}
