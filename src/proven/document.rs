//! Verified document-level scans: lone carriage returns and the guide
//! envelope.
//!
//! Two decisions about a whole guide document live here.
//!
//! - [`first_lone_cr_line`] rejects a carriage return that is not part of a
//!   CRLF pair (`docs/v0.2-contract.md`, "Document and marker grammar": "A
//!   lone CR is not a line ending and is invalid anywhere in the document")
//!   and reports the physical line on which the first one occurs.
//! - [`scan_marker_line`] and [`finish_envelope`] locate the one guide block
//!   from the per-line marker classifications produced by the verified
//!   [`crate::proven::marker`] kernel ("A second opening or closing marker
//!   line, including a nested block, is invalid").
//!
//! The envelope decision is a small state machine over the per-line classes.
//! Its specification is that state machine, so the contract-level meaning is
//! stated and proven separately over whole class sequences (these proofs
//! exist only in the Verus build):
//! `theorem_envelope_accepts_exactly` characterizes accepted documents
//! declaratively; `theorem_envelope_reports_first_offense`,
//! `theorem_envelope_without_opening`, and `theorem_envelope_without_closing`
//! fix the reported error and its line; and
//! `theorem_envelope_accepts_without_offense` shows that these cases cover
//! every document.
//!
//! Trusted glue outside this file: `str::lines` splits the document into the
//! physical lines whose classes are fed to the scan, the parser feeds those
//! classes in order with line numbers `1, 2, ...`, and it joins the prologue,
//! body, and epilogue line ranges back together.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

use crate::proven::marker::MarkerClass;

/// Return the 1-based line of the first carriage return that is not
/// immediately followed by a line feed, or `None` when every carriage return
/// begins a CRLF pair.
///
/// The line is one more than the number of line feeds before that carriage
/// return, which is the physical line number under both LF and CRLF endings.
/// CR and LF are ASCII and UTF-8 never encodes another scalar with an ASCII
/// byte, so scanning the bytes of a `str` is scanning its scalars.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r is None <==> no_lone_cr(bytes@),
        r matches Some(line) ==> exists|i: int|
            first_lone_cr(bytes@, i) && line == lf_count(bytes@, i) + 1,
))]
pub(crate) fn first_lone_cr_line(bytes: &[u8]) -> Option<usize> {
    let len = bytes.len();
    let mut idx: usize = 0;
    let mut line_feeds: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            idx <= len,
            len == bytes@.len(),
            line_feeds == lf_count(bytes@, idx as int),
            line_feeds <= idx,
            forall|j: int| 0 <= j < idx ==> !is_lone_cr(bytes@, j),
        decreases len - idx
    ))]
    while idx < len {
        let byte = bytes[idx];
        if byte == b'\r' {
            if !(idx + 1 < len && bytes[idx + 1] == b'\n') {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(first_lone_cr(bytes@, idx as int));
                }
                return Some(line_feeds + 1);
            }
        } else if byte == b'\n' {
            line_feeds += 1;
        }
        idx += 1;
    }
    None
}

/// An error in the guide envelope, with the exact line the parser reports.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnvelopeError {
    /// A second opening marker, or a closing marker after the block closed.
    MultipleGuideBlocks { line: usize },
    /// A malformed opening candidate, a closing marker before any opening
    /// marker, or (at line 1) a document without an opening marker.
    MissingOpeningMarker { line: usize },
    /// A malformed closing candidate, or (at the last line) an opening marker
    /// that is never closed.
    MissingClosingMarker { line: usize },
}

/// Progress of the envelope scan after a prefix of the document's lines.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnvelopeScan {
    /// No marker line has been seen.
    BeforeOpening,
    /// The opening marker is on `opening_line`; no closing marker yet.
    InBlock { opening_line: usize, ignore: bool },
    /// The block opened on `opening_line` and closed on `closing_line`.
    AfterClosing {
        opening_line: usize,
        closing_line: usize,
        ignore: bool,
    },
}

/// The located guide block: 1-based marker lines and the ignore flag.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Envelope {
    pub(crate) opening_line: usize,
    pub(crate) closing_line: usize,
    pub(crate) ignore: bool,
}

#[cfg(verus_keep_ghost)]
verus! {

/// Byte `i` is a carriage return that does not begin a CRLF pair.
pub(crate) open spec fn is_lone_cr(s: Seq<u8>, i: int) -> bool {
    0 <= i < s.len() && s[i] == 0x0Du8 && !(i + 1 < s.len() && s[i + 1] == 0x0Au8)
}

/// Every carriage return in `s` is immediately followed by a line feed.
pub(crate) open spec fn no_lone_cr(s: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < s.len() ==> !#[trigger] is_lone_cr(s, i)
}

/// Byte `i` is the first lone carriage return in `s`.
pub(crate) open spec fn first_lone_cr(s: Seq<u8>, i: int) -> bool {
    is_lone_cr(s, i) && forall|j: int| 0 <= j < i ==> !#[trigger] is_lone_cr(s, j)
}

/// The number of line feeds among the first `n` bytes of `s`.
pub(crate) open spec fn lf_count(s: Seq<u8>, n: int) -> int
    decreases n,
{
    if n <= 0 || n > s.len() {
        0
    } else {
        lf_count(s, n - 1) + if s[n - 1] == 0x0Au8 { 1int } else { 0int }
    }
}

/// `Opening` or `IgnoredOpening`.
pub(crate) open spec fn is_opening(c: MarkerClass) -> bool {
    c == MarkerClass::Opening || c == MarkerClass::IgnoredOpening
}

/// The envelope scan's response to the class of the line numbered `line`.
pub(crate) open spec fn envelope_step(
    scan: EnvelopeScan,
    line: usize,
    class: MarkerClass,
) -> Result<EnvelopeScan, EnvelopeError> {
    match class {
        MarkerClass::Ordinary => Ok(scan),
        MarkerClass::MalformedOpening => Err(EnvelopeError::MissingOpeningMarker { line }),
        MarkerClass::MalformedClosing => Err(EnvelopeError::MissingClosingMarker { line }),
        MarkerClass::Closing => match scan {
            EnvelopeScan::BeforeOpening => Err(EnvelopeError::MissingOpeningMarker { line }),
            EnvelopeScan::InBlock { opening_line, ignore } => Ok(
                EnvelopeScan::AfterClosing { opening_line, closing_line: line, ignore },
            ),
            EnvelopeScan::AfterClosing { .. } => Err(EnvelopeError::MultipleGuideBlocks { line }),
        },
        _ => match scan {
            EnvelopeScan::BeforeOpening => Ok(
                EnvelopeScan::InBlock {
                    opening_line: line,
                    ignore: class == MarkerClass::IgnoredOpening,
                },
            ),
            _ => Err(EnvelopeError::MultipleGuideBlocks { line }),
        },
    }
}

/// The envelope decision once every line has been scanned.
pub(crate) open spec fn envelope_finish(
    scan: EnvelopeScan,
    line_count: usize,
) -> Result<Envelope, EnvelopeError> {
    match scan {
        EnvelopeScan::BeforeOpening => Err(EnvelopeError::MissingOpeningMarker { line: 1 }),
        EnvelopeScan::InBlock { .. } => Err(
            EnvelopeError::MissingClosingMarker { line: line_count },
        ),
        EnvelopeScan::AfterClosing { opening_line, closing_line, ignore } => Ok(
            Envelope { opening_line, closing_line, ignore },
        ),
    }
}

/// The scan over the classes of lines `1..=cs.len()`, in order, stopping at
/// the first error.
pub(crate) open spec fn envelope_scan_lines(cs: Seq<MarkerClass>) -> Result<
    EnvelopeScan,
    EnvelopeError,
>
    decreases cs.len(),
{
    if cs.len() == 0 {
        Ok(EnvelopeScan::BeforeOpening)
    } else {
        match envelope_scan_lines(cs.drop_last()) {
            Ok(scan) => envelope_step(scan, cs.len() as usize, cs.last()),
            Err(e) => Err(e),
        }
    }
}

/// The whole-document envelope decision for the line classes `cs`.
pub(crate) open spec fn locate_envelope(cs: Seq<MarkerClass>) -> Result<Envelope, EnvelopeError> {
    match envelope_scan_lines(cs) {
        Ok(scan) => envelope_finish(scan, cs.len() as usize),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Declarative contract vocabulary.
// ---------------------------------------------------------------------------

/// Some line before index `i` is an opening marker.
pub(crate) open spec fn opening_before(cs: Seq<MarkerClass>, i: int) -> bool {
    exists|j: int| 0 <= j < i && #[trigger] is_opening(cs[j])
}

/// Some line before index `i` is a closing marker.
pub(crate) open spec fn closing_before(cs: Seq<MarkerClass>, i: int) -> bool {
    exists|j: int| 0 <= j < i && #[trigger] cs[j] == MarkerClass::Closing
}

/// The line at index `i` cannot appear in any valid document with the
/// preceding lines: a malformed marker candidate, an opening marker after
/// another marker, or a closing marker that does not directly follow the
/// single opening marker.
pub(crate) open spec fn offending(cs: Seq<MarkerClass>, i: int) -> bool {
    match cs[i] {
        MarkerClass::Ordinary => false,
        MarkerClass::MalformedOpening => true,
        MarkerClass::MalformedClosing => true,
        MarkerClass::Closing => !opening_before(cs, i) || closing_before(cs, i),
        _ => opening_before(cs, i) || closing_before(cs, i),
    }
}

/// Line index `i` is the earliest offending line.
pub(crate) open spec fn first_offending(cs: Seq<MarkerClass>, i: int) -> bool {
    0 <= i < cs.len() && offending(cs, i) && forall|j: int|
        0 <= j < i ==> !#[trigger] offending(cs, j)
}

/// The error reported for the offending line at index `i` (line `i + 1`).
pub(crate) open spec fn offense_error(cs: Seq<MarkerClass>, i: int) -> EnvelopeError {
    let line = (i + 1) as usize;
    match cs[i] {
        MarkerClass::MalformedOpening => EnvelopeError::MissingOpeningMarker { line },
        MarkerClass::MalformedClosing => EnvelopeError::MissingClosingMarker { line },
        MarkerClass::Closing => if opening_before(cs, i) {
            EnvelopeError::MultipleGuideBlocks { line }
        } else {
            EnvelopeError::MissingOpeningMarker { line }
        },
        _ => EnvelopeError::MultipleGuideBlocks { line },
    }
}

/// The document is exactly one guide block: index `start` is the only
/// opening marker line, index `end > start` the only closing marker line,
/// every other line is ordinary, and `ignore` records the attributed form.
pub(crate) open spec fn single_envelope(
    cs: Seq<MarkerClass>,
    start: int,
    end: int,
    ignore: bool,
) -> bool {
    &&& 0 <= start < end < cs.len()
    &&& is_opening(cs[start])
    &&& cs[end] == MarkerClass::Closing
    &&& forall|i: int|
        0 <= i < cs.len() && i != start && i != end ==> #[trigger] cs[i] == MarkerClass::Ordinary
    &&& ignore == (cs[start] == MarkerClass::IgnoredOpening)
}

/// What the scan knows after the first `n` lines, stated without the scan.
pub(crate) open spec fn prefix_summary(
    cs: Seq<MarkerClass>,
    n: int,
    r: Result<EnvelopeScan, EnvelopeError>,
) -> bool {
    match r {
        Err(e) => exists|i: int| 0 <= i < n && first_offending(cs, i) && e == offense_error(cs, i),
        Ok(EnvelopeScan::BeforeOpening) => forall|j: int|
            0 <= j < n ==> #[trigger] cs[j] == MarkerClass::Ordinary,
        Ok(EnvelopeScan::InBlock { opening_line, ignore }) => {
            let o = opening_line - 1;
            &&& 0 <= o < n
            &&& is_opening(cs[o])
            &&& ignore == (cs[o] == MarkerClass::IgnoredOpening)
            &&& forall|j: int| 0 <= j < n && j != o ==> #[trigger] cs[j] == MarkerClass::Ordinary
        },
        Ok(EnvelopeScan::AfterClosing { opening_line, closing_line, ignore }) => {
            let o = opening_line - 1;
            let c = closing_line - 1;
            &&& 0 <= o < c < n
            &&& is_opening(cs[o])
            &&& cs[c] == MarkerClass::Closing
            &&& ignore == (cs[o] == MarkerClass::IgnoredOpening)
            &&& forall|j: int|
                0 <= j < n && j != o && j != c ==> #[trigger] cs[j] == MarkerClass::Ordinary
        },
    }
}

proof fn lemma_scan_take(cs: Seq<MarkerClass>, n: int)
    requires
        0 < n <= cs.len(),
    ensures
        cs.take(n).drop_last() == cs.take(n - 1),
        cs.take(n).last() == cs[n - 1],
        cs.take(n).len() == n,
{
    assert(cs.take(n).drop_last() =~= cs.take(n - 1));
}

/// The first offending line is unique.
proof fn lemma_first_offending_unique(cs: Seq<MarkerClass>, i: int, k: int)
    requires
        first_offending(cs, i),
        first_offending(cs, k),
    ensures
        i == k,
{
    if i < k {
        assert(!offending(cs, i));
    }
    if k < i {
        assert(!offending(cs, k));
    }
}

/// The scan of the first `n` lines is described by [`prefix_summary`].
proof fn lemma_scan_prefix(cs: Seq<MarkerClass>, n: int)
    requires
        0 <= n <= cs.len(),
        cs.len() <= usize::MAX,
    ensures
        prefix_summary(cs, n, envelope_scan_lines(cs.take(n))),
    decreases n,
{
    if n == 0 {
        assert(cs.take(0).len() == 0);
    } else {
        lemma_scan_take(cs, n);
        lemma_scan_prefix(cs, n - 1);
        let prev = envelope_scan_lines(cs.take(n - 1));
        let i = n - 1;
        let line = n as usize;
        assert(envelope_scan_lines(cs.take(n)) == match prev {
            Ok(scan) => envelope_step(scan, line, cs[i]),
            Err(e) => Err::<EnvelopeScan, EnvelopeError>(e),
        });
        match prev {
            Err(e) => {},
            Ok(scan) => {
                // No earlier line offends: every earlier line is ordinary or
                // one of the (at most two) recorded marker lines.
                assert forall|j: int| 0 <= j < i implies !#[trigger] offending(cs, j) by {
                    lemma_prefix_line_not_offending(cs, i, scan, j);
                }
                lemma_step_summary(cs, i, scan);
            },
        }
    }
}

/// A line inside a successfully scanned prefix does not offend.
proof fn lemma_prefix_line_not_offending(cs: Seq<MarkerClass>, n: int, scan: EnvelopeScan, j: int)
    requires
        0 <= j < n <= cs.len(),
        prefix_summary(cs, n, Ok(scan)),
    ensures
        !offending(cs, j),
{
    match scan {
        EnvelopeScan::BeforeOpening => {
            assert(cs[j] == MarkerClass::Ordinary);
        },
        EnvelopeScan::InBlock { opening_line, ignore } => {
            let o = opening_line - 1;
            if j != o {
                assert(cs[j] == MarkerClass::Ordinary);
            } else {
                assert forall|k: int| 0 <= k < j implies !#[trigger] is_opening(cs[k]) by {
                    assert(cs[k] == MarkerClass::Ordinary);
                }
                assert forall|k: int| 0 <= k < j implies #[trigger] cs[k] != MarkerClass::Closing by {
                    assert(cs[k] == MarkerClass::Ordinary);
                }
            }
        },
        EnvelopeScan::AfterClosing { opening_line, closing_line, ignore } => {
            let o = opening_line - 1;
            let c = closing_line - 1;
            if j != o && j != c {
                assert(cs[j] == MarkerClass::Ordinary);
            } else if j == o {
                assert forall|k: int| 0 <= k < j implies !#[trigger] is_opening(cs[k]) by {
                    assert(cs[k] == MarkerClass::Ordinary);
                }
                assert forall|k: int| 0 <= k < j implies #[trigger] cs[k] != MarkerClass::Closing by {
                    assert(cs[k] == MarkerClass::Ordinary);
                }
            } else {
                assert(is_opening(cs[o]));
                assert(opening_before(cs, j));
                assert forall|k: int| 0 <= k < j implies #[trigger] cs[k] != MarkerClass::Closing by {
                    if k != o {
                        assert(cs[k] == MarkerClass::Ordinary);
                    }
                }
            }
        },
    }
}

/// One scan step from a successfully scanned prefix of `n` lines.
proof fn lemma_step_summary(cs: Seq<MarkerClass>, n: int, scan: EnvelopeScan)
    requires
        0 <= n < cs.len(),
        cs.len() <= usize::MAX,
        prefix_summary(cs, n, Ok(scan)),
        forall|j: int| 0 <= j < n ==> !#[trigger] offending(cs, j),
    ensures
        prefix_summary(cs, n + 1, envelope_step(scan, (n + 1) as usize, cs[n])),
{
    let class = cs[n];
    let r = envelope_step(scan, (n + 1) as usize, class);
    match scan {
        EnvelopeScan::BeforeOpening => {
            assert(!opening_before(cs, n));
            assert(!closing_before(cs, n));
            if class == MarkerClass::Closing || class == MarkerClass::MalformedOpening
                || class == MarkerClass::MalformedClosing {
                assert(first_offending(cs, n));
            }
        },
        EnvelopeScan::InBlock { opening_line, ignore } => {
            let o = opening_line - 1;
            assert(is_opening(cs[o]));
            assert(opening_before(cs, n));
            assert(!closing_before(cs, n)) by {
                assert forall|k: int| 0 <= k < n implies #[trigger] cs[k] != MarkerClass::Closing by {
                    if k != o {
                        assert(cs[k] == MarkerClass::Ordinary);
                    }
                }
            }
            if class != MarkerClass::Ordinary && class != MarkerClass::Closing {
                assert(first_offending(cs, n));
            }
        },
        EnvelopeScan::AfterClosing { opening_line, closing_line, ignore } => {
            let o = opening_line - 1;
            let c = closing_line - 1;
            assert(is_opening(cs[o]));
            assert(opening_before(cs, n));
            assert(closing_before(cs, n));
            if class != MarkerClass::Ordinary {
                assert(first_offending(cs, n));
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Contract-level theorems.
// ---------------------------------------------------------------------------

/// A document is accepted exactly when it consists of one guide block: one
/// opening marker line, one later closing marker line, and only ordinary
/// lines otherwise. The reported marker lines are those lines (1-based) and
/// the ignore flag is set exactly for the attributed opening marker.
pub(crate) proof fn theorem_envelope_accepts_exactly(cs: Seq<MarkerClass>)
    requires
        cs.len() <= usize::MAX,
    ensures
        locate_envelope(cs) matches Ok(env) ==> single_envelope(
            cs,
            env.opening_line - 1,
            env.closing_line - 1,
            env.ignore,
        ),
        forall|start: int, end: int, ignore: bool|
            #[trigger] single_envelope(cs, start, end, ignore) ==> locate_envelope(cs) == Ok::<
                Envelope,
                EnvelopeError,
            >(
                Envelope {
                    opening_line: (start + 1) as usize,
                    closing_line: (end + 1) as usize,
                    ignore,
                },
            ),
{
    lemma_scan_prefix(cs, cs.len() as int);
    assert(cs.take(cs.len() as int) =~= cs);
    assert forall|start: int, end: int, ignore: bool|
        #[trigger] single_envelope(cs, start, end, ignore) implies locate_envelope(cs) == Ok::<
            Envelope,
            EnvelopeError,
        >(
            Envelope {
                opening_line: (start + 1) as usize,
                closing_line: (end + 1) as usize,
                ignore,
            },
        ) by {
        lemma_single_envelope_has_no_offense(cs, start, end, ignore);
        match envelope_scan_lines(cs) {
            Err(e) => {
                let i = choose|i: int|
                    0 <= i < cs.len() && first_offending(cs, i) && e == offense_error(cs, i);
                assert(!offending(cs, i));
            },
            Ok(EnvelopeScan::BeforeOpening) => {
                assert(cs[start] == MarkerClass::Ordinary);
            },
            Ok(EnvelopeScan::InBlock { opening_line, ignore: ig }) => {
                let o = opening_line - 1;
                assert(cs[end] == MarkerClass::Ordinary || end == o);
                assert(cs[start] == MarkerClass::Ordinary || start == o);
            },
            Ok(EnvelopeScan::AfterClosing { opening_line, closing_line, ignore: ig }) => {
                let o = opening_line - 1;
                let c = closing_line - 1;
                if start != o {
                    assert(start == c || cs[start] == MarkerClass::Ordinary);
                }
                if end != c {
                    assert(end == o || cs[end] == MarkerClass::Ordinary);
                }
            },
        }
    }
}

/// A single-envelope document has no offending line.
proof fn lemma_single_envelope_has_no_offense(
    cs: Seq<MarkerClass>,
    start: int,
    end: int,
    ignore: bool,
)
    requires
        single_envelope(cs, start, end, ignore),
    ensures
        forall|i: int| 0 <= i < cs.len() ==> !#[trigger] offending(cs, i),
{
    assert forall|i: int| 0 <= i < cs.len() implies !#[trigger] offending(cs, i) by {
        if i == start {
            assert forall|k: int| 0 <= k < i implies !#[trigger] is_opening(cs[k]) by {
                assert(cs[k] == MarkerClass::Ordinary);
            }
            assert forall|k: int| 0 <= k < i implies #[trigger] cs[k] != MarkerClass::Closing by {
                assert(cs[k] == MarkerClass::Ordinary);
            }
        } else if i == end {
            assert(opening_before(cs, i));
            assert forall|k: int| 0 <= k < i implies #[trigger] cs[k] != MarkerClass::Closing by {
                if k != start {
                    assert(cs[k] == MarkerClass::Ordinary);
                }
            }
        } else {
            assert(cs[i] == MarkerClass::Ordinary);
        }
    }
}

/// When some line offends, the document is rejected with the error of the
/// earliest offending line, at that line: `MissingOpeningMarker` for a
/// malformed opening candidate or a closing marker before any opening marker,
/// `MissingClosingMarker` for a malformed closing candidate, and
/// `MultipleGuideBlocks` for any other extra marker line.
pub(crate) proof fn theorem_envelope_reports_first_offense(cs: Seq<MarkerClass>, i: int)
    requires
        cs.len() <= usize::MAX,
        first_offending(cs, i),
    ensures
        locate_envelope(cs) == Err::<Envelope, EnvelopeError>(offense_error(cs, i)),
{
    lemma_scan_prefix(cs, cs.len() as int);
    assert(cs.take(cs.len() as int) =~= cs);
    match envelope_scan_lines(cs) {
        Err(e) => {
            let k = choose|k: int|
                0 <= k < cs.len() && first_offending(cs, k) && e == offense_error(cs, k);
            lemma_first_offending_unique(cs, i, k);
        },
        Ok(scan) => {
            lemma_prefix_line_not_offending(cs, cs.len() as int, scan, i);
        },
    }
}

/// Without offending lines and without any opening marker, the document is
/// rejected as missing its opening marker at line 1.
pub(crate) proof fn theorem_envelope_without_opening(cs: Seq<MarkerClass>)
    requires
        cs.len() <= usize::MAX,
        forall|i: int| 0 <= i < cs.len() ==> !#[trigger] offending(cs, i),
        forall|i: int| 0 <= i < cs.len() ==> !#[trigger] is_opening(cs[i]),
    ensures
        locate_envelope(cs) == Err::<Envelope, EnvelopeError>(
            EnvelopeError::MissingOpeningMarker { line: 1 },
        ),
{
    lemma_scan_prefix(cs, cs.len() as int);
    assert(cs.take(cs.len() as int) =~= cs);
    match envelope_scan_lines(cs) {
        Err(e) => {
            let k = choose|k: int|
                0 <= k < cs.len() && first_offending(cs, k) && e == offense_error(cs, k);
            assert(offending(cs, k));
        },
        Ok(EnvelopeScan::BeforeOpening) => {},
        Ok(EnvelopeScan::InBlock { opening_line, ignore }) => {
            assert(is_opening(cs[opening_line - 1]));
        },
        Ok(EnvelopeScan::AfterClosing { opening_line, closing_line, ignore }) => {
            assert(is_opening(cs[opening_line - 1]));
        },
    }
}

/// Without offending lines, an opening marker that is never followed by a
/// closing marker is reported as a missing closing marker at the last line.
pub(crate) proof fn theorem_envelope_without_closing(cs: Seq<MarkerClass>)
    requires
        cs.len() <= usize::MAX,
        forall|i: int| 0 <= i < cs.len() ==> !#[trigger] offending(cs, i),
        exists|i: int| 0 <= i < cs.len() && #[trigger] is_opening(cs[i]),
        forall|i: int| 0 <= i < cs.len() ==> #[trigger] cs[i] != MarkerClass::Closing,
    ensures
        locate_envelope(cs) == Err::<Envelope, EnvelopeError>(
            EnvelopeError::MissingClosingMarker { line: cs.len() as usize },
        ),
{
    lemma_scan_prefix(cs, cs.len() as int);
    assert(cs.take(cs.len() as int) =~= cs);
    let w = choose|i: int| 0 <= i < cs.len() && #[trigger] is_opening(cs[i]);
    match envelope_scan_lines(cs) {
        Err(e) => {
            let k = choose|k: int|
                0 <= k < cs.len() && first_offending(cs, k) && e == offense_error(cs, k);
            assert(offending(cs, k));
        },
        Ok(EnvelopeScan::BeforeOpening) => {
            assert(cs[w] == MarkerClass::Ordinary);
        },
        Ok(EnvelopeScan::InBlock { opening_line, ignore }) => {},
        Ok(EnvelopeScan::AfterClosing { opening_line, closing_line, ignore }) => {
            assert(cs[closing_line - 1] == MarkerClass::Closing);
        },
    }
}

/// Without offending lines, a document with an opening and a closing marker
/// line is accepted. With the three theorems above this covers every
/// document: it is accepted exactly as [`single_envelope`] describes, or it
/// is rejected with the first offense, the missing opening marker at line 1,
/// or the missing closing marker at the last line.
pub(crate) proof fn theorem_envelope_accepts_without_offense(cs: Seq<MarkerClass>)
    requires
        cs.len() <= usize::MAX,
        forall|i: int| 0 <= i < cs.len() ==> !#[trigger] offending(cs, i),
        exists|i: int| 0 <= i < cs.len() && #[trigger] is_opening(cs[i]),
        exists|i: int| 0 <= i < cs.len() && #[trigger] cs[i] == MarkerClass::Closing,
    ensures
        locate_envelope(cs) is Ok,
{
    lemma_scan_prefix(cs, cs.len() as int);
    assert(cs.take(cs.len() as int) =~= cs);
    let w = choose|i: int| 0 <= i < cs.len() && #[trigger] is_opening(cs[i]);
    let c = choose|i: int| 0 <= i < cs.len() && #[trigger] cs[i] == MarkerClass::Closing;
    match envelope_scan_lines(cs) {
        Err(e) => {
            let k = choose|k: int|
                0 <= k < cs.len() && first_offending(cs, k) && e == offense_error(cs, k);
            assert(offending(cs, k));
        },
        Ok(EnvelopeScan::BeforeOpening) => {
            assert(cs[w] == MarkerClass::Ordinary);
        },
        Ok(EnvelopeScan::InBlock { opening_line, ignore }) => {
            let o = opening_line - 1;
            if c != o {
                assert(cs[c] == MarkerClass::Ordinary);
            }
            assert(is_opening(cs[o]));
        },
        Ok(EnvelopeScan::AfterClosing { opening_line, closing_line, ignore }) => {},
    }
}

} // verus!

/// Advance the envelope scan over the line numbered `line`, whose marker
/// class is `class`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == envelope_step(scan, line, class),
))]
pub(crate) fn scan_marker_line(
    scan: EnvelopeScan,
    line: usize,
    class: MarkerClass,
) -> Result<EnvelopeScan, EnvelopeError> {
    match class {
        MarkerClass::Ordinary => Ok(scan),
        MarkerClass::MalformedOpening => Err(EnvelopeError::MissingOpeningMarker { line }),
        MarkerClass::MalformedClosing => Err(EnvelopeError::MissingClosingMarker { line }),
        MarkerClass::Opening | MarkerClass::IgnoredOpening => match scan {
            EnvelopeScan::BeforeOpening => Ok(EnvelopeScan::InBlock {
                opening_line: line,
                ignore: matches!(class, MarkerClass::IgnoredOpening),
            }),
            EnvelopeScan::InBlock { .. } | EnvelopeScan::AfterClosing { .. } => {
                Err(EnvelopeError::MultipleGuideBlocks { line })
            }
        },
        MarkerClass::Closing => match scan {
            EnvelopeScan::BeforeOpening => Err(EnvelopeError::MissingOpeningMarker { line }),
            EnvelopeScan::InBlock {
                opening_line,
                ignore,
            } => Ok(EnvelopeScan::AfterClosing {
                opening_line,
                closing_line: line,
                ignore,
            }),
            EnvelopeScan::AfterClosing { .. } => Err(EnvelopeError::MultipleGuideBlocks { line }),
        },
    }
}

/// Finish the envelope scan of a document with `line_count` lines.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == envelope_finish(scan, line_count),
))]
pub(crate) fn finish_envelope(
    scan: EnvelopeScan,
    line_count: usize,
) -> Result<Envelope, EnvelopeError> {
    match scan {
        EnvelopeScan::BeforeOpening => Err(EnvelopeError::MissingOpeningMarker { line: 1 }),
        EnvelopeScan::InBlock { .. } => {
            Err(EnvelopeError::MissingClosingMarker { line: line_count })
        }
        EnvelopeScan::AfterClosing {
            opening_line,
            closing_line,
            ignore,
        } => Ok(Envelope {
            opening_line,
            closing_line,
            ignore,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lone-CR scan as the parser implemented it before the kernel.
    fn reference_lone_cr_line(bytes: &[u8]) -> Option<usize> {
        let mut idx = 0;
        let mut line = 1;
        while idx < bytes.len() {
            match bytes[idx] {
                b'\r' if bytes.get(idx + 1) == Some(&b'\n') => {
                    idx += 2;
                    line += 1;
                }
                b'\r' => return Some(line),
                b'\n' => {
                    idx += 1;
                    line += 1;
                }
                _ => idx += 1,
            }
        }
        None
    }

    /// The envelope loop as the parser implemented it before the kernel,
    /// with malformed candidates reported as `Parser::classify_marker_line`
    /// reports them.
    fn reference_envelope(classes: &[MarkerClass]) -> Result<Envelope, EnvelopeError> {
        let mut start_idx = None;
        let mut end_idx = None;
        let mut ignore = false;
        for (idx, class) in classes.iter().enumerate() {
            let line = idx + 1;
            match class {
                MarkerClass::Ordinary => {}
                MarkerClass::MalformedOpening => {
                    return Err(EnvelopeError::MissingOpeningMarker { line })
                }
                MarkerClass::MalformedClosing => {
                    return Err(EnvelopeError::MissingClosingMarker { line })
                }
                MarkerClass::Opening | MarkerClass::IgnoredOpening => {
                    if start_idx.is_some() || end_idx.is_some() {
                        return Err(EnvelopeError::MultipleGuideBlocks { line });
                    }
                    start_idx = Some(idx);
                    ignore = *class == MarkerClass::IgnoredOpening;
                }
                MarkerClass::Closing => match (start_idx, end_idx) {
                    (Some(_), None) => end_idx = Some(idx),
                    (None, None) => return Err(EnvelopeError::MissingOpeningMarker { line }),
                    _ => return Err(EnvelopeError::MultipleGuideBlocks { line }),
                },
            }
        }
        let start = start_idx.ok_or(EnvelopeError::MissingOpeningMarker { line: 1 })?;
        let end = end_idx.ok_or(EnvelopeError::MissingClosingMarker {
            line: classes.len(),
        })?;
        Ok(Envelope {
            opening_line: start + 1,
            closing_line: end + 1,
            ignore,
        })
    }

    /// The kernel fold exactly as `Parser::extract_guide_block` drives it.
    fn locate(classes: &[MarkerClass]) -> Result<Envelope, EnvelopeError> {
        let mut scan = EnvelopeScan::BeforeOpening;
        for (idx, class) in classes.iter().enumerate() {
            scan = scan_marker_line(scan, idx + 1, *class)?;
        }
        finish_envelope(scan, classes.len())
    }

    const CLASSES: [MarkerClass; 6] = [
        MarkerClass::Ordinary,
        MarkerClass::Opening,
        MarkerClass::IgnoredOpening,
        MarkerClass::Closing,
        MarkerClass::MalformedOpening,
        MarkerClass::MalformedClosing,
    ];

    use MarkerClass::{Closing as C, IgnoredOpening as I, Opening as O, Ordinary as N};

    #[test]
    fn lone_cr_scan_accepts_documents_without_lone_carriage_returns() {
        for text in [
            "",
            "abc",
            "a\nb",
            "a\r\nb",
            "\r\n",
            "a\r\n\r\nb\n",
            "\u{e9}\r\n\u{3000}",
        ] {
            assert_eq!(first_lone_cr_line(text.as_bytes()), None, "{text:?}");
        }
    }

    #[test]
    fn lone_cr_scan_reports_the_physical_line_of_the_first_lone_cr() {
        let cases: [(&str, usize); 12] = [
            ("\r", 1),
            ("a\rb", 1),
            ("\r\r\n", 1),
            ("a\nb\rc", 2),
            ("a\r\nb\r", 2),
            ("a\r\nb\r\nc\rd", 3),
            ("a\r\n\r\nb\rx", 3),
            ("\n\n\r", 3),
            ("\rx\ry", 1),
            ("a\n\r\r\n", 2),
            ("\u{e9}\u{3000}\r", 1),
            ("x\r\n\ny\r\n\rz", 4),
        ];
        for (text, line) in cases {
            assert_eq!(first_lone_cr_line(text.as_bytes()), Some(line), "{text:?}");
        }
    }

    #[test]
    fn lone_cr_scan_matches_the_previous_parser_on_every_short_document() {
        let alphabet = *b"a\r\n";
        let mut bytes = Vec::new();
        for len in 0..=9u32 {
            for mut code in 0..3usize.pow(len) {
                bytes.clear();
                for _ in 0..len {
                    bytes.push(alphabet[code % 3]);
                    code /= 3;
                }
                assert_eq!(
                    first_lone_cr_line(&bytes),
                    reference_lone_cr_line(&bytes),
                    "{bytes:?}"
                );
            }
        }
    }

    #[test]
    fn scan_step_covers_every_class_in_every_state() {
        let before = EnvelopeScan::BeforeOpening;
        let open = EnvelopeScan::InBlock {
            opening_line: 2,
            ignore: false,
        };
        let open_ignored = EnvelopeScan::InBlock {
            opening_line: 2,
            ignore: true,
        };
        let closed = EnvelopeScan::AfterClosing {
            opening_line: 2,
            closing_line: 4,
            ignore: true,
        };
        let line = 7;

        for scan in [before, open, closed] {
            assert_eq!(scan_marker_line(scan, line, N), Ok(scan));
            assert_eq!(
                scan_marker_line(scan, line, MarkerClass::MalformedOpening),
                Err(EnvelopeError::MissingOpeningMarker { line })
            );
            assert_eq!(
                scan_marker_line(scan, line, MarkerClass::MalformedClosing),
                Err(EnvelopeError::MissingClosingMarker { line })
            );
        }

        assert_eq!(
            scan_marker_line(before, line, O),
            Ok(EnvelopeScan::InBlock {
                opening_line: line,
                ignore: false
            })
        );
        assert_eq!(
            scan_marker_line(before, line, I),
            Ok(EnvelopeScan::InBlock {
                opening_line: line,
                ignore: true
            })
        );
        for scan in [open, closed] {
            for class in [O, I] {
                assert_eq!(
                    scan_marker_line(scan, line, class),
                    Err(EnvelopeError::MultipleGuideBlocks { line })
                );
            }
        }

        assert_eq!(
            scan_marker_line(before, line, C),
            Err(EnvelopeError::MissingOpeningMarker { line })
        );
        assert_eq!(
            scan_marker_line(open, line, C),
            Ok(EnvelopeScan::AfterClosing {
                opening_line: 2,
                closing_line: line,
                ignore: false
            })
        );
        assert_eq!(
            scan_marker_line(open_ignored, line, C),
            Ok(EnvelopeScan::AfterClosing {
                opening_line: 2,
                closing_line: line,
                ignore: true
            })
        );
        assert_eq!(
            scan_marker_line(closed, line, C),
            Err(EnvelopeError::MultipleGuideBlocks { line })
        );
    }

    #[test]
    fn finish_reports_missing_markers_or_the_located_block() {
        assert_eq!(
            finish_envelope(EnvelopeScan::BeforeOpening, 9),
            Err(EnvelopeError::MissingOpeningMarker { line: 1 })
        );
        assert_eq!(
            finish_envelope(
                EnvelopeScan::InBlock {
                    opening_line: 3,
                    ignore: false
                },
                9
            ),
            Err(EnvelopeError::MissingClosingMarker { line: 9 })
        );
        assert_eq!(
            finish_envelope(
                EnvelopeScan::AfterClosing {
                    opening_line: 3,
                    closing_line: 5,
                    ignore: true
                },
                9
            ),
            Ok(Envelope {
                opening_line: 3,
                closing_line: 5,
                ignore: true
            })
        );
    }

    #[test]
    fn envelope_accepts_one_block_and_records_the_ignore_flag() {
        assert_eq!(
            locate(&[N, O, N, N, C, N]),
            Ok(Envelope {
                opening_line: 2,
                closing_line: 5,
                ignore: false
            })
        );
        assert_eq!(
            locate(&[I, C]),
            Ok(Envelope {
                opening_line: 1,
                closing_line: 2,
                ignore: true
            })
        );
    }

    #[test]
    fn envelope_reports_the_first_offending_line() {
        let multiple = |line| Err(EnvelopeError::MultipleGuideBlocks { line });
        let opening = |line| Err(EnvelopeError::MissingOpeningMarker { line });
        let closing = |line| Err(EnvelopeError::MissingClosingMarker { line });
        let malformed_opening = MarkerClass::MalformedOpening;
        let malformed_closing = MarkerClass::MalformedClosing;

        assert_eq!(locate(&[O, N, O, C]), multiple(3));
        assert_eq!(locate(&[O, I, C]), multiple(2));
        assert_eq!(locate(&[O, C, O, C]), multiple(3));
        assert_eq!(locate(&[O, C, N, C]), multiple(4));
        assert_eq!(locate(&[N, C, O, C]), opening(2));
        assert_eq!(locate(&[N, malformed_opening, O, C]), opening(2));
        assert_eq!(locate(&[O, malformed_closing, C]), closing(2));
        assert_eq!(locate(&[O, C, malformed_opening]), opening(3));
        // An earlier envelope error wins over a later malformed candidate and
        // the other way round.
        assert_eq!(locate(&[O, O, malformed_closing]), multiple(2));
        assert_eq!(locate(&[O, malformed_closing, O]), closing(2));
    }

    #[test]
    fn envelope_reports_missing_markers_at_the_document_edges() {
        let no_opening = Err(EnvelopeError::MissingOpeningMarker { line: 1 });
        assert_eq!(locate(&[]), no_opening);
        assert_eq!(locate(&[N, N, N]), no_opening);
        assert_eq!(
            locate(&[N, O, N, N]),
            Err(EnvelopeError::MissingClosingMarker { line: 4 })
        );
        assert_eq!(
            locate(&[I]),
            Err(EnvelopeError::MissingClosingMarker { line: 1 })
        );
    }

    #[test]
    fn envelope_matches_the_previous_parser_on_every_short_document() {
        let mut classes = Vec::new();
        for len in 0..=6u32 {
            for mut code in 0..CLASSES.len().pow(len) {
                classes.clear();
                for _ in 0..len {
                    classes.push(CLASSES[code % CLASSES.len()]);
                    code /= CLASSES.len();
                }
                assert_eq!(
                    locate(&classes),
                    reference_envelope(&classes),
                    "{classes:?}"
                );
            }
        }
    }
}
