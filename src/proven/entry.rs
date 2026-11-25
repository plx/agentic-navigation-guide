//! Verified parsing of one entry's text: the part of an entry line after
//! `indent "- "`.
//!
//! [`parse_entry`] decides, in the parser's diagnostic order, whether the
//! text is the placeholder, one quoted or bare path, or a choice list, and
//! returns the decoded paths, the directory marker, and the normalized
//! comment, or the error kind with the exact text the diagnostic renders.
//! Verus proves that its result is `entry_spec` on every scalar sequence.
//!
//! `entry_spec` follows `docs/v0.2-contract.md` ("Active body grammar",
//! "Path expressions", "Choice lists", and "Placeholders and listing
//! completeness"):
//!
//! - an entry that starts with `"` is a whole quoted path, decoded with the
//!   contract decoder `crate::path_codec::decode_quoted_chars`, followed by
//!   an optional directory `/`, spaces, and nothing or a comment;
//! - any other entry is split at its first unescaped `#`, loses its trailing
//!   unquoted spaces, and is then the placeholder when it is exactly `...`,
//!   or a bare path expression (`crate::proven::choice`).
//!
//! The theorems at the end of this file state what an accepted entry means:
//!
//! - `theorem_placeholder_iff_bare_ellipsis`: the placeholder is recognized
//!   exactly when the unquoted, comment-stripped, right-trimmed expression is
//!   `...`, and `theorem_quoted_ellipsis_is_a_file`: `"..."` is the regular
//!   file `...`;
//! - `theorem_bare_paths_are_contained`: every path accepted from a bare or
//!   choice expression passes the validator's structural rules
//!   (`crate::proven::path_rules::path_verdict` is `Valid`) and is therefore
//!   a contained relative path, with no `...` component and no edge space;
//! - `theorem_choice_list_shape`: an accepted choice list has between 2 and
//!   256 pairwise-distinct expansions with one shared parent text and one
//!   shared sequence of parent components, none of them a directory, and its
//!   line cannot own children; `theorem_choice_expansions_pass_bare_rules`
//!   shows every expansion also passes the bare single-path rules, which the
//!   previous parser re-checked after expansion;
//! - `theorem_quoted_path_decoder`: a quoted entry is accepted with path `d`
//!   exactly when the contract decoder `decode_quoted_path` yields `d` and an
//!   accepted tail, and `theorem_quoted_component_entry`: the canonical
//!   quoted form of every representable name round-trips, while
//!   `theorem_plain_bare_entry` reduces plain bare text (no `#`, `\`, `[`,
//!   `]`, or double quote) to the bare single-path rules;
//! - `theorem_comment_is_normalized`: a returned comment is nonempty and has
//!   no leading or trailing space or tab;
//! - `theorem_quoted_paths_defer_containment`: the parser accepts every
//!   nonempty quoted name without CR or LF, including absolute, parent,
//!   repeated-separator, drive, and control-bearing names, so containment of
//!   quoted paths rests entirely on the validator.
//!
//! Trusted glue outside this file: the parser passes the layout kernel's
//! entry text and `depth == 0`, builds one guide item per returned path with
//! a copy of the comment, and maps each [`EntryFailure`] to its diagnostic
//! and message text. Uniqueness of choice expansions is verified in
//! `crate::proven::choice` by comparing each alternative with the earlier
//! ones (at most 256); full-path uniqueness across entries remains the
//! validator's.

#[cfg(verus_keep_ghost)]
use crate::path_codec::{
    decode_quoted_chars, decode_quoted_path, has_drive_prefix, has_forbidden_control,
    is_reserved_ellipsis, option_string_view, quote_escape, quote_escape_char, quote_escape_fn,
    quoted_component,
};
#[cfg(verus_keep_ghost)]
use crate::proven::choice::{
    alternatives_spec, choice_list_spec, decode_escapes, expansion, expansion_error_at, expansions,
    first_expansion_error, has_dot_component, has_invalid_component, invalid_decoded_component,
    lemma_alternatives_shape, parent_len, parent_of, root_drive, scan_expression_spec, scan_from,
    strings_view, valid_choice_expansion, ScanRegion, ScanView,
};
use crate::proven::choice::{
    component_report, decode_escapes_exec, expand_choice_list, scan_expression, string_from_chars,
    ChoiceError, ExpressionScan,
};
#[cfg(verus_keep_ghost)]
use crate::proven::path_rules::{
    is_child_name, is_contained_relative_path, is_dot_component, join_components, path_verdict,
    split_components, theorem_split_join, theorem_valid_iff_contained, PathVerdict,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

/// Which diagnostic rejects an entry.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryErrorKind {
    /// `SyntaxError::InvalidPathFormat`.
    InvalidPath,
    /// `SyntaxError::InvalidSpecialDirectory`.
    SpecialDirectory,
    /// `SyntaxError::InvalidWildcardSyntax` with the error's message.
    Choice(ChoiceError),
}

/// A rejected entry: the diagnostic and the text it renders.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EntryFailure {
    pub(crate) kind: EntryErrorKind,
    /// The whole entry text, the bare expression, a decoded path, or empty
    /// text, as the diagnostic requires.
    pub(crate) subject: String,
}

/// What an accepted entry denotes.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EntryShape {
    /// The unquoted `...`.
    Placeholder,
    /// One quoted or bare path without its directory marker.
    Single { path: String, directory: bool },
    /// The expansions of a choice list: sibling regular files.
    Choice(Vec<String>),
}

/// An accepted entry with its normalized comment.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParsedEntry {
    pub(crate) shape: EntryShape,
    pub(crate) comment: Option<String>,
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

/// What an accepted entry denotes, over scalars.
pub(crate) enum ShapeView {
    Placeholder,
    Single { path: Seq<char>, directory: bool },
    Choice { paths: Seq<Seq<char>> },
}

/// An accepted entry over scalars.
pub(crate) struct EntryView {
    pub(crate) shape: ShapeView,
    pub(crate) comment: Option<Seq<char>>,
}

/// A rejected entry over scalars.
pub(crate) struct FailureView {
    pub(crate) kind: EntryErrorKind,
    pub(crate) subject: Seq<char>,
}

pub(crate) open spec fn shape_view(s: EntryShape) -> ShapeView {
    match s {
        EntryShape::Placeholder => ShapeView::Placeholder,
        EntryShape::Single { path, directory } => ShapeView::Single { path: path@, directory },
        EntryShape::Choice(paths) => ShapeView::Choice { paths: strings_view(paths@) },
    }
}

pub(crate) open spec fn failure_view(f: EntryFailure) -> FailureView {
    FailureView { kind: f.kind, subject: f.subject@ }
}

pub(crate) open spec fn entry_view(e: ParsedEntry) -> EntryView {
    EntryView { shape: shape_view(e.shape), comment: option_string_view(e.comment) }
}

pub(crate) open spec fn entry_result_view(r: Result<ParsedEntry, EntryFailure>) -> Result<
    EntryView,
    FailureView,
> {
    match r {
        Ok(e) => Ok(entry_view(e)),
        Err(f) => Err(failure_view(f)),
    }
}

pub(crate) open spec fn shape_result_view(r: Result<EntryShape, EntryFailure>) -> Result<
    ShapeView,
    FailureView,
> {
    match r {
        Ok(s) => Ok(shape_view(s)),
        Err(f) => Err(failure_view(f)),
    }
}

pub(crate) open spec fn failure(kind: EntryErrorKind, subject: Seq<char>) -> FailureView {
    FailureView { kind, subject }
}

// ---------------------------------------------------------------------------
// Comments and layout
// ---------------------------------------------------------------------------

/// `SP / HTAB`.
pub(crate) open spec fn is_hspace(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// `s` without its leading spaces and tabs.
pub(crate) open spec fn trim_start_hspace(s: Seq<char>) -> Seq<char>
    decreases s.len(),
{
    if s.len() > 0 && is_hspace(s[0]) {
        trim_start_hspace(s.drop_first())
    } else {
        s
    }
}

/// `s` without its trailing spaces and tabs.
pub(crate) open spec fn trim_end_hspace(s: Seq<char>) -> Seq<char>
    decreases s.len(),
{
    if s.len() > 0 && is_hspace(s.last()) {
        trim_end_hspace(s.drop_last())
    } else {
        s
    }
}

/// Comment text without its leading and trailing spaces and tabs; empty
/// text is no comment.
pub(crate) open spec fn normalize_comment(t: Seq<char>) -> Option<Seq<char>> {
    let u = trim_end_hspace(trim_start_hspace(t));
    if u.len() == 0 {
        None
    } else {
        Some(u)
    }
}

/// `s` without its leading U+0020 spaces.
pub(crate) open spec fn trim_start_spaces(s: Seq<char>) -> Seq<char>
    decreases s.len(),
{
    if s.len() > 0 && s[0] == ' ' {
        trim_start_spaces(s.drop_first())
    } else {
        s
    }
}

/// `s` without its trailing U+0020 spaces (`line-padding`).
pub(crate) open spec fn trim_end_spaces(s: Seq<char>) -> Seq<char>
    decreases s.len(),
{
    if s.len() > 0 && s.last() == ' ' {
        trim_end_spaces(s.drop_last())
    } else {
        s
    }
}

/// The index of the first unescaped `#` at or after the token start `i`
/// (or `s.len()`): a backslash and the following scalar form one token.
pub(crate) open spec fn comment_start(s: Seq<char>, i: int) -> int
    decreases s.len() - i,
{
    if i >= s.len() {
        s.len() as int
    } else if s[i] == '#' {
        i
    } else if s[i] == '\\' {
        if i + 1 >= s.len() {
            s.len() as int
        } else {
            comment_start(s, i + 2)
        }
    } else {
        comment_start(s, i + 1)
    }
}

// ---------------------------------------------------------------------------
// Quoted paths
// ---------------------------------------------------------------------------

/// The text after a quoted path's closing quote: an optional directory `/`,
/// then spaces, then nothing or a comment. Returns the directory flag and
/// the normalized comment.
pub(crate) open spec fn quoted_tail(rest: Seq<char>) -> Option<(bool, Option<Seq<char>>)> {
    let directory = rest.len() > 0 && rest[0] == '/';
    let after = if directory {
        rest.drop_first()
    } else {
        rest
    };
    let r = trim_start_spaces(after);
    if r.len() == 0 {
        Some((directory, None))
    } else if r[0] == '#' {
        Some((directory, normalize_comment(r.drop_first())))
    } else {
        None
    }
}

/// A whole quoted path entry. Malformed quoting or tails render the whole
/// entry; an empty decoded path renders empty text.
pub(crate) open spec fn quoted_entry_spec(content: Seq<char>) -> Result<EntryView, FailureView> {
    match decode_quoted_chars(content.drop_first()) {
        None => Err(failure(EntryErrorKind::InvalidPath, content)),
        Some((decoded, rest)) => if decoded.len() == 0 {
            Err(failure(EntryErrorKind::InvalidPath, Seq::empty()))
        } else {
            match quoted_tail(rest) {
                None => Err(failure(EntryErrorKind::InvalidPath, content)),
                Some((directory, comment)) => Ok(
                    EntryView { shape: ShapeView::Single { path: decoded, directory }, comment },
                ),
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Bare paths
// ---------------------------------------------------------------------------

/// The parser's rules for one decoded bare path without its directory
/// marker: a `.` or `..` component anywhere is a special directory; then an
/// empty path, a leading `/`, ` `, or (at the root) `\`, a trailing ` `, an
/// invalid component, or a root drive prefix is an invalid format.
pub(crate) open spec fn bare_path_rule(d: Seq<char>, at_root: bool) -> PathVerdict {
    if has_dot_component(d) {
        PathVerdict::SpecialDirectory
    } else if d.len() == 0 || d[0] == '/' || (at_root && d[0] == '\\') || d[0] == ' ' || d.last()
        == ' ' || has_invalid_component(d) || root_drive(d, at_root) {
        PathVerdict::InvalidFormat
    } else {
        PathVerdict::Valid
    }
}

/// A decoded bare path without a choice list: one trailing `/` is the
/// directory marker, and the rest must satisfy [`bare_path_rule`]. Errors
/// render the decoded path without its marker.
pub(crate) open spec fn single_path_spec(p: Seq<char>, at_root: bool) -> Result<
    ShapeView,
    FailureView,
> {
    let directory = p.len() > 0 && p.last() == '/';
    let d = if directory {
        p.drop_last()
    } else {
        p
    };
    match bare_path_rule(d, at_root) {
        PathVerdict::SpecialDirectory => Err(failure(EntryErrorKind::SpecialDirectory, d)),
        PathVerdict::InvalidFormat => Err(failure(EntryErrorKind::InvalidPath, d)),
        PathVerdict::Valid => Ok(ShapeView::Single { path: d, directory }),
    }
}

/// A nonempty bare expression: the scan, then either the decoded single
/// path or the choice list. Choice-list errors render the expression.
pub(crate) open spec fn expression_spec(expr: Seq<char>, at_root: bool) -> Result<
    ShapeView,
    FailureView,
> {
    match scan_expression_spec(expr) {
        Err(e) => Err(failure(EntryErrorKind::Choice(e), expr)),
        Ok(ScanView::Plain) => single_path_spec(decode_escapes(expr), at_root),
        Ok(ScanView::Choice { open, close }) => match choice_list_spec(
            expr.take(open),
            expr.subrange(open + 1, close),
            expr.skip(close + 1),
            at_root,
        ) {
            Err(e) => Err(failure(EntryErrorKind::Choice(e), expr)),
            Ok(paths) => Ok(ShapeView::Choice { paths }),
        },
    }
}

/// The bare expression of an unquoted entry: the text before the first
/// unescaped `#` without its trailing spaces.
pub(crate) open spec fn bare_expression(content: Seq<char>) -> Seq<char> {
    trim_end_spaces(content.take(comment_start(content, 0)))
}

/// The normalized comment of an unquoted entry.
pub(crate) open spec fn bare_comment(content: Seq<char>) -> Option<Seq<char>> {
    let h = comment_start(content, 0);
    if h < content.len() {
        normalize_comment(content.skip(h + 1))
    } else {
        None
    }
}

/// An unquoted entry: empty expression, the placeholder, or a bare path
/// expression, with the comment attached to whatever it denotes.
pub(crate) open spec fn bare_entry_spec(content: Seq<char>, at_root: bool) -> Result<
    EntryView,
    FailureView,
> {
    let expr = bare_expression(content);
    let comment = bare_comment(content);
    if expr.len() == 0 {
        Err(failure(EntryErrorKind::InvalidPath, Seq::empty()))
    } else if expr == seq!['.', '.', '.'] {
        Ok(EntryView { shape: ShapeView::Placeholder, comment })
    } else {
        match expression_spec(expr, at_root) {
            Ok(shape) => Ok(EntryView { shape, comment }),
            Err(f) => Err(f),
        }
    }
}

/// The contract meaning of one entry's text (after `indent "- "`), where
/// `at_root` says whether the entry is at depth 0.
pub(crate) open spec fn entry_spec(content: Seq<char>, at_root: bool) -> Result<
    EntryView,
    FailureView,
> {
    if content.len() > 0 && content[0] == '"' {
        quoted_entry_spec(content)
    } else {
        bare_entry_spec(content, at_root)
    }
}

/// Whether an accepted entry's line may own indented children: exactly one
/// directory.
pub(crate) open spec fn can_own_children(v: EntryView) -> bool {
    match v.shape {
        ShapeView::Single { directory, .. } => directory,
        _ => false,
    }
}

/// The `(path, is_directory)` items an accepted entry contributes.
pub(crate) open spec fn entry_items(v: EntryView) -> Seq<(Seq<char>, bool)> {
    match v.shape {
        ShapeView::Placeholder => Seq::empty(),
        ShapeView::Single { path, directory } => seq![(path, directory)],
        ShapeView::Choice { paths } => paths.map_values(|p: Seq<char>| (p, false)),
    }
}

/// Prefixes decoded text onto a decoder result.
spec fn prepend_decoded(d: Seq<char>, r: Option<(Seq<char>, Seq<char>)>) -> Option<
    (Seq<char>, Seq<char>),
> {
    match r {
        Some((x, rest)) => Some((d + x, rest)),
        None => None,
    }
}

} // verus!

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl ParsedEntry {
    /// Whether the entry's line may own indented children: exactly one
    /// directory.
    #[cfg_attr(verus_keep_ghost, verus_spec(r =>
        ensures
            r == can_own_children(entry_view(*self)),
    ))]
    pub(crate) fn can_own_children(&self) -> bool {
        match &self.shape {
            EntryShape::Single { directory, .. } => *directory,
            _ => false,
        }
    }
}

/// Return the scalars of `text`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == text@,
))]
fn collect_chars(text: &str) -> Vec<char> {
    let mut out: Vec<char> = Vec::with_capacity(text.len());
    let mut chars = text.chars();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(text@.take(0) =~= Seq::<char>::empty());
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= text@.len(),
            chars.remaining() == text@.skip(index),
            out@ == text@.take(index),
        decreases text@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(text@.skip(index)[0] == text@[index]);
                    assert(text@.take(index + 1) =~= text@.take(index).push(ch));
                }
                out.push(ch);
                #[cfg(verus_keep_ghost)]
                proof! {
                    index = index + 1;
                }
            }
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(text@.take(index) =~= text@);
                }
                return out;
            }
        }
    }
}

/// Return the normalized comment `s[lo..hi]`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        lo <= hi <= s@.len(),
    ensures
        option_string_view(r) == normalize_comment(s@.subrange(lo as int, hi as int)),
))]
fn normalized_comment(s: &[char], lo: usize, hi: usize) -> Option<String> {
    let mut start = lo;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            lo <= start <= hi,
            hi <= s@.len(),
            trim_start_hspace(s@.subrange(lo as int, hi as int)) == trim_start_hspace(
                s@.subrange(start as int, hi as int),
            ),
        decreases hi - start
    ))]
    while start < hi && (s[start] == ' ' || s[start] == '\t') {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.subrange(start as int, hi as int).drop_first() =~= s@.subrange(
                start + 1,
                hi as int,
            ));
        }
        start += 1;
    }
    let mut end = hi;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            start <= end <= hi,
            hi <= s@.len(),
            start == hi || !is_hspace(s@[start as int]),
            trim_start_hspace(s@.subrange(lo as int, hi as int)) == s@.subrange(
                start as int,
                hi as int,
            ),
            trim_end_hspace(s@.subrange(start as int, hi as int)) == trim_end_hspace(
                s@.subrange(start as int, end as int),
            ),
        decreases end - start
    ))]
    while end > start && (s[end - 1] == ' ' || s[end - 1] == '\t') {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.subrange(start as int, end as int).drop_last() =~= s@.subrange(
                start as int,
                end - 1,
            ));
        }
        end -= 1;
    }
    if start == end {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.subrange(start as int, end as int).len() == 0);
        }
        return None;
    }
    Some(string_from_chars(&s[start..end]))
}

/// Return the index of the first unescaped `#` in `s`, or `s.len()`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == comment_start(s@, 0),
        r <= s@.len(),
))]
fn find_comment_start(s: &[char]) -> usize {
    let n = s.len();
    let mut i: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            i <= n,
            n == s@.len(),
            comment_start(s@, 0) == comment_start(s@, i as int),
        decreases n - i
    ))]
    while i < n {
        if s[i] == '#' {
            return i;
        }
        if s[i] == '\\' {
            if i + 1 >= n {
                return n;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    n
}

/// Parse a quoted entry; `s` starts with `"`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        s@.len() > 0,
        s@[0] == '"',
    ensures
        entry_result_view(r) == quoted_entry_spec(s@),
))]
fn parse_quoted_entry(s: &[char]) -> Result<ParsedEntry, EntryFailure> {
    let n = s.len();
    let mut decoded = String::new();
    let mut i: usize = 1;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(s@.skip(1) =~= s@.drop_first());
        match decode_quoted_chars(s@.skip(1)) {
            Some((x, rest)) => {
                assert(Seq::<char>::empty() + x =~= x);
            },
            None => {},
        }
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            1 <= i <= n,
            n == s@.len(),
            decode_quoted_chars(s@.drop_first()) == prepend_decoded(
                decoded@,
                decode_quoted_chars(s@.skip(i as int)),
            ),
        decreases n - i
    ))]
    while i < n && s[i] != '"' {
        let c = s[i];
        #[cfg(verus_keep_ghost)]
        proof! {
            let t = s@.skip(i as int);
            assert(t[0] == c);
            assert(t.drop_first() =~= s@.skip(i + 1));
            if i + 1 < n {
                assert(t[1] == s@[i + 1]);
                assert(t.subrange(2, t.len() as int) =~= s@.skip(i + 2));
            }
        }
        if c == '\\' {
            if i + 1 >= n || !(s[i + 1] == '"' || s[i + 1] == '\\') {
                return Err(EntryFailure {
                    kind: EntryErrorKind::InvalidPath,
                    subject: string_from_chars(s),
                });
            }
            let escaped = s[i + 1];
            #[cfg(verus_keep_ghost)]
            proof! {
                match decode_quoted_chars(s@.skip(i + 2)) {
                    Some((x, rest)) => {
                        assert(decoded@ + (seq![escaped] + x) =~= decoded@.push(escaped) + x);
                    },
                    None => {},
                }
            }
            decoded.push(escaped);
            i += 2;
        } else if c == '\r' || c == '\n' {
            return Err(EntryFailure {
                kind: EntryErrorKind::InvalidPath,
                subject: string_from_chars(s),
            });
        } else {
            #[cfg(verus_keep_ghost)]
            proof! {
                match decode_quoted_chars(s@.skip(i + 1)) {
                    Some((x, rest)) => {
                        assert(decoded@ + (seq![c] + x) =~= decoded@.push(c) + x);
                    },
                    None => {},
                }
            }
            decoded.push(c);
            i += 1;
        }
    }
    if i >= n {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.skip(i as int).len() == 0);
        }
        return Err(EntryFailure {
            kind: EntryErrorKind::InvalidPath,
            subject: string_from_chars(s),
        });
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        let t = s@.skip(i as int);
        assert(t[0] == '"');
        assert(t.drop_first() =~= s@.skip(i + 1));
        assert(decoded@ + Seq::<char>::empty() =~= decoded@);
    }
    if decoded.is_empty() {
        return Err(EntryFailure {
            kind: EntryErrorKind::InvalidPath,
            subject: String::new(),
        });
    }
    let mut j = i + 1;
    let directory = j < n && s[j] == '/';
    #[cfg(verus_keep_ghost)]
    proof! {
        let rest = s@.skip(i + 1);
        if j < n {
            assert(rest[0] == s@[j as int]);
            assert(rest.drop_first() =~= s@.skip(j + 1));
        }
    }
    if directory {
        j += 1;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            i < j <= n,
            n == s@.len(),
            trim_start_spaces(
                if directory {
                    s@.skip(i + 1).drop_first()
                } else {
                    s@.skip(i + 1)
                },
            ) == trim_start_spaces(s@.skip(j as int)),
        decreases n - j
    ))]
    while j < n && s[j] == ' ' {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.skip(j as int)[0] == s@[j as int]);
            assert(s@.skip(j as int).drop_first() =~= s@.skip(j + 1));
        }
        j += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        let r = s@.skip(j as int);
        if j < n {
            assert(r[0] == s@[j as int]);
            assert(r.drop_first() =~= s@.subrange(j + 1, n as int));
        }
    }
    let comment = if j == n {
        None
    } else if s[j] == '#' {
        normalized_comment(s, j + 1, n)
    } else {
        return Err(EntryFailure {
            kind: EntryErrorKind::InvalidPath,
            subject: string_from_chars(s),
        });
    };
    Ok(ParsedEntry {
        shape: EntryShape::Single {
            path: decoded,
            directory,
        },
        comment,
    })
}

/// Apply the bare single-path rules to the decoded path `p`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        shape_result_view(r) == single_path_spec(p@, at_root),
))]
fn parse_single_path(p: &[char], at_root: bool) -> Result<EntryShape, EntryFailure> {
    let n = p.len();
    let directory = n > 0 && p[n - 1] == '/';
    let m = if directory { n - 1 } else { n };
    let d = &p[0..m];
    #[cfg(verus_keep_ghost)]
    proof! {
        if directory {
            assert(d@ =~= p@.drop_last());
        } else {
            assert(d@ =~= p@);
        }
    }
    let report = component_report(d);
    if report.dot {
        return Err(EntryFailure {
            kind: EntryErrorKind::SpecialDirectory,
            subject: string_from_chars(d),
        });
    }
    if m == 0
        || d[0] == '/'
        || (at_root && d[0] == '\\')
        || d[0] == ' '
        || d[m - 1] == ' '
        || report.invalid
        || (at_root && report.drive)
    {
        return Err(EntryFailure {
            kind: EntryErrorKind::InvalidPath,
            subject: string_from_chars(d),
        });
    }
    Ok(EntryShape::Single {
        path: string_from_chars(d),
        directory,
    })
}

/// Parse a nonempty bare expression.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        shape_result_view(r) == expression_spec(expr@, at_root),
))]
fn parse_expression(expr: &[char], at_root: bool) -> Result<EntryShape, EntryFailure> {
    match scan_expression(expr) {
        Err(e) => Err(EntryFailure {
            kind: EntryErrorKind::Choice(e),
            subject: string_from_chars(expr),
        }),
        Ok(ExpressionScan::Plain) => {
            let decoded = decode_escapes_exec(expr);
            parse_single_path(decoded.as_slice(), at_root)
        }
        Ok(ExpressionScan::Choice { open, close }) => {
            let n = expr.len();
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(expr@.subrange(0, open as int) =~= expr@.take(open as int));
                assert(expr@.subrange(close + 1, n as int) =~= expr@.skip(close + 1));
            }
            match expand_choice_list(
                &expr[0..open],
                &expr[open + 1..close],
                &expr[close + 1..n],
                at_root,
            ) {
                Ok(paths) => Ok(EntryShape::Choice(paths)),
                Err(e) => Err(EntryFailure {
                    kind: EntryErrorKind::Choice(e),
                    subject: string_from_chars(expr),
                }),
            }
        }
    }
}

/// Parse an unquoted entry.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        entry_result_view(r) == bare_entry_spec(s@, at_root),
))]
fn parse_bare_entry(s: &[char], at_root: bool) -> Result<ParsedEntry, EntryFailure> {
    let n = s.len();
    let h = find_comment_start(s);
    let mut e = h;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(s@.take(h as int) == s@.take(e as int));
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            e <= h,
            h <= n,
            n == s@.len(),
            trim_end_spaces(s@.take(h as int)) == trim_end_spaces(s@.take(e as int)),
        decreases e
    ))]
    while e > 0 && s[e - 1] == ' ' {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.take(e as int).drop_last() =~= s@.take(e - 1));
        }
        e -= 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        if e > 0 {
            assert(s@.take(e as int).last() == s@[e - 1]);
        }
        assert(bare_expression(s@) == s@.take(e as int));
        assert(s@.subrange(h + 1, n as int) =~= s@.skip(h + 1));
    }
    let comment = if h < n {
        normalized_comment(s, h + 1, n)
    } else {
        None
    };
    if e == 0 {
        return Err(EntryFailure {
            kind: EntryErrorKind::InvalidPath,
            subject: String::new(),
        });
    }
    if e == 3 && s[0] == '.' && s[1] == '.' && s[2] == '.' {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.take(3) =~= seq!['.', '.', '.']);
        }
        return Ok(ParsedEntry {
            shape: EntryShape::Placeholder,
            comment,
        });
    }
    let expr = &s[0..e];
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(expr@ =~= s@.take(e as int));
        if expr@ == seq!['.', '.', '.'] {
            assert(expr@[0] == '.' && expr@[1] == '.' && expr@[2] == '.');
        }
    }
    match parse_expression(expr, at_root) {
        Ok(shape) => Ok(ParsedEntry { shape, comment }),
        Err(failure) => Err(failure),
    }
}

/// Parse one entry's text (after `indent "- "`); `at_root` says whether the
/// entry is at depth 0.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        entry_result_view(r) == entry_spec(content@, at_root),
))]
pub(crate) fn parse_entry(content: &str, at_root: bool) -> Result<ParsedEntry, EntryFailure> {
    let chars = collect_chars(content);
    let s = chars.as_slice();
    if !s.is_empty() && s[0] == '"' {
        parse_quoted_entry(s)
    } else {
        parse_bare_entry(s, at_root)
    }
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Supporting lemmas
// ---------------------------------------------------------------------------

/// What one accepted bare path guarantees (after directory-marker removal).
pub(crate) open spec fn contained_bare_path(p: Seq<char>, at_root: bool) -> bool {
    &&& path_verdict(p, at_root) == PathVerdict::Valid
    &&& is_contained_relative_path(p, at_root)
    &&& forall|i: int|
        0 <= i < split_components(p).len() ==> !is_reserved_ellipsis(
            #[trigger] split_components(p)[i],
        )
    &&& p[0] != ' '
    &&& p.last() != ' '
}

/// A path that satisfies the bare rules is a contained relative path.
proof fn lemma_bare_rule_contained(d: Seq<char>, at_root: bool)
    requires
        bare_path_rule(d, at_root) == PathVerdict::Valid,
    ensures
        contained_bare_path(d, at_root),
{
    theorem_split_join(d);
    let comps = split_components(d);
    assert forall|i: int| 0 <= i < comps.len() implies is_child_name(#[trigger] comps[i]) by {
        assert(!invalid_decoded_component(comps[i]));
    }
    assert forall|i: int| 0 <= i < comps.len() implies !is_reserved_ellipsis(
        #[trigger] comps[i],
    ) by {
        assert(!invalid_decoded_component(comps[i]));
    }
    assert(join_components(comps) == d && comps.len() >= 1 && (forall|i: int|
        0 <= i < comps.len() ==> is_child_name(#[trigger] comps[i])) && (at_root ==> d[0] != '\\'
        && !has_drive_prefix(comps[0])));
    assert(is_contained_relative_path(d, at_root));
    theorem_valid_iff_contained(d, at_root);
}

/// Every valid expansion passes the bare single-path rules unchanged and
/// is not a directory, so the parser's per-expansion bare check (which ran
/// after expansion in earlier releases) can never reject one.
pub(crate) proof fn theorem_choice_expansions_pass_bare_rules(e: Seq<char>, at_root: bool)
    requires
        valid_choice_expansion(e, at_root),
    ensures
        bare_path_rule(e, at_root) == PathVerdict::Valid,
        single_path_spec(e, at_root) == Ok::<ShapeView, FailureView>(
            ShapeView::Single { path: e, directory: false },
        ),
        contained_bare_path(e, at_root),
{
    if has_dot_component(e) {
        let k = choose|k: int|
            0 <= k < split_components(e).len() && #[trigger] is_dot_component(
                split_components(e)[k],
            );
        assert(invalid_decoded_component(split_components(e)[k]));
    }
    lemma_bare_rule_contained(e, at_root);
}

/// No error from `k` on means no error at any alternative from `k` on.
proof fn lemma_no_expansion_error(
    prefix: Seq<char>,
    choices: Seq<Seq<char>>,
    suffix: Seq<char>,
    at_root: bool,
    k: int,
)
    requires
        0 <= k,
        first_expansion_error(prefix, choices, suffix, at_root, k) is None,
    ensures
        forall|m: int|
            k <= m < choices.len() ==> #[trigger] expansion_error_at(
                prefix,
                choices,
                suffix,
                at_root,
                m,
            ) is None,
    decreases choices.len() - k,
{
    if k < choices.len() {
        lemma_no_expansion_error(prefix, choices, suffix, at_root, k + 1);
    }
}

/// The parent length of `x + "/" + name` for a `/`-free `name`.
proof fn lemma_parent_len_after_separator(x: Seq<char>, name: Seq<char>)
    requires
        !name.contains('/'),
    ensures
        parent_len(x + seq!['/'] + name) == x.len(),
    decreases name.len(),
{
    let s = x + seq!['/'] + name;
    if name.len() == 0 {
        assert(s.last() == '/');
    } else {
        assert(s.last() == name.last());
        assert(name[name.len() - 1] == name.last());
        assert(s.drop_last() =~= x + seq!['/'] + name.drop_last());
        assert(!name.drop_last().contains('/')) by {
            if name.drop_last().contains('/') {
                let k = choose|k: int| 0 <= k < name.drop_last().len() && name.drop_last()[k] == '/';
                assert(name[k] == '/');
            }
        }
        lemma_parent_len_after_separator(x, name.drop_last());
    }
}

/// A `/`-free text has no parent.
proof fn lemma_parent_len_free(s: Seq<char>)
    requires
        !s.contains('/'),
    ensures
        parent_len(s) == 0,
    decreases s.len(),
{
    if s.len() > 0 {
        assert(s[s.len() - 1] == s.last());
        assert(!s.drop_last().contains('/')) by {
            if s.drop_last().contains('/') {
                let k = choose|k: int| 0 <= k < s.drop_last().len() && s.drop_last()[k] == '/';
                assert(s[k] == '/');
            }
        }
        lemma_parent_len_free(s.drop_last());
    }
}

/// A join is at least as long as its first component.
proof fn lemma_join_len(comps: Seq<Seq<char>>)
    requires
        comps.len() >= 1,
    ensures
        join_components(comps).len() >= comps[0].len(),
    decreases comps.len(),
{
    if comps.len() > 1 {
        lemma_join_len(comps.drop_last());
        assert(comps.drop_last()[0] == comps[0]);
    }
}

/// The parent text of `s` is the join of every component but the last, and
/// a single-component text has an empty parent.
proof fn lemma_parent_components(s: Seq<char>)
    ensures
        split_components(s).len() == 1 ==> parent_of(s) == Seq::<char>::empty(),
        split_components(s).len() >= 2 ==> {
            &&& parent_of(s) == join_components(split_components(s).drop_last())
            &&& split_components(parent_of(s)) == split_components(s).drop_last()
        },
{
    theorem_split_join(s);
    let comps = split_components(s);
    if comps.len() == 1 {
        assert(join_components(comps) == comps[0]);
        assert(!comps[0].contains('/'));
        lemma_parent_len_free(s);
        assert(s.take(0) =~= Seq::<char>::empty());
    } else {
        let init = comps.drop_last();
        let x = join_components(init);
        assert(s == x + seq!['/'] + comps.last());
        assert(!comps[comps.len() - 1].contains('/'));
        lemma_parent_len_after_separator(x, comps.last());
        assert(s.take(x.len() as int) =~= x);
        assert forall|i: int| 0 <= i < init.len() implies !(#[trigger] init[i]).contains('/') by {
            assert(init[i] == comps[i]);
        }
        crate::proven::path_rules::theorem_split_unique(init);
    }
}

/// Two valid expansions with the same parent text have the same sequence of
/// parent components.
proof fn lemma_same_parent_components(a: Seq<char>, b: Seq<char>, at_root: bool)
    requires
        valid_choice_expansion(a, at_root),
        valid_choice_expansion(b, at_root),
        parent_of(a) == parent_of(b),
    ensures
        split_components(a).drop_last() == split_components(b).drop_last(),
{
    lemma_parent_components(a);
    lemma_parent_components(b);
    theorem_split_join(a);
    theorem_split_join(b);
    let ca = split_components(a);
    let cb = split_components(b);
    if ca.len() >= 2 {
        assert(!invalid_decoded_component(ca[0]));
        assert(ca.drop_last()[0] == ca[0]);
        lemma_join_len(ca.drop_last());
        assert(parent_of(a).len() > 0);
    }
    if cb.len() >= 2 {
        assert(!invalid_decoded_component(cb[0]));
        assert(cb.drop_last()[0] == cb[0]);
        lemma_join_len(cb.drop_last());
        assert(parent_of(b).len() > 0);
    }
    if ca.len() == 1 && cb.len() == 1 {
        assert(ca.drop_last() =~= cb.drop_last());
    }
}

/// `trim_start_hspace(t)` drops a maximal run of spaces and tabs.
proof fn lemma_trim_start_hspace(t: Seq<char>)
    ensures
        exists|a: int|
            0 <= a <= t.len() && trim_start_hspace(t) == t.skip(a) && (forall|k: int|
                0 <= k < a ==> is_hspace(#[trigger] t[k])) && (a == t.len() || !is_hspace(t[a])),
    decreases t.len(),
{
    if t.len() > 0 && is_hspace(t[0]) {
        lemma_trim_start_hspace(t.drop_first());
        let a = choose|a: int|
            0 <= a <= t.drop_first().len() && trim_start_hspace(t.drop_first())
                == t.drop_first().skip(a) && (forall|k: int|
                0 <= k < a ==> is_hspace(#[trigger] t.drop_first()[k])) && (a
                == t.drop_first().len() || !is_hspace(t.drop_first()[a]));
        assert(t.drop_first().skip(a) =~= t.skip(a + 1));
        assert forall|k: int| 0 <= k < a + 1 implies is_hspace(#[trigger] t[k]) by {
            if k > 0 {
                assert(t[k] == t.drop_first()[k - 1]);
            }
        }
        if a + 1 < t.len() {
            assert(t[a + 1] == t.drop_first()[a]);
        }
    } else {
        assert(t.skip(0) =~= t);
    }
}

/// `trim_end_hspace(t)` drops a maximal run of spaces and tabs.
proof fn lemma_trim_end_hspace(t: Seq<char>)
    ensures
        exists|b: int|
            0 <= b <= t.len() && trim_end_hspace(t) == t.take(b) && (forall|k: int|
                b <= k < t.len() ==> is_hspace(#[trigger] t[k])) && (b == 0 || !is_hspace(
                t[b - 1],
            )),
    decreases t.len(),
{
    if t.len() > 0 && is_hspace(t.last()) {
        lemma_trim_end_hspace(t.drop_last());
        let b = choose|b: int|
            0 <= b <= t.drop_last().len() && trim_end_hspace(t.drop_last()) == t.drop_last().take(
                b,
            ) && (forall|k: int|
                b <= k < t.drop_last().len() ==> is_hspace(#[trigger] t.drop_last()[k])) && (b
                == 0 || !is_hspace(t.drop_last()[b - 1]));
        assert(t.drop_last().take(b) =~= t.take(b));
        assert forall|k: int| b <= k < t.len() implies is_hspace(#[trigger] t[k]) by {
            if k < t.len() - 1 {
                assert(t[k] == t.drop_last()[k]);
            }
        }
        if b > 0 {
            assert(t[b - 1] == t.drop_last()[b - 1]);
        }
    } else {
        assert(t.take(t.len() as int) =~= t);
    }
}

/// A normalized comment is a nonempty infix of the comment text without
/// leading or trailing spaces or tabs, and only spaces and tabs surround it.
proof fn lemma_normalize_comment(t: Seq<char>)
    ensures
        normalize_comment(t) matches Some(u) ==> {
            &&& u.len() > 0
            &&& !is_hspace(u[0])
            &&& !is_hspace(u.last())
            &&& exists|a: int, b: int|
                0 <= a <= b <= t.len() && u == t.subrange(a, b) && (forall|k: int|
                    0 <= k < a ==> is_hspace(#[trigger] t[k])) && (forall|k: int|
                    b <= k < t.len() ==> is_hspace(#[trigger] t[k]))
        },
{
    lemma_trim_start_hspace(t);
    let y = trim_start_hspace(t);
    let a = choose|a: int|
        0 <= a <= t.len() && trim_start_hspace(t) == t.skip(a) && (forall|k: int|
            0 <= k < a ==> is_hspace(#[trigger] t[k])) && (a == t.len() || !is_hspace(t[a]));
    lemma_trim_end_hspace(y);
    let b = choose|b: int|
        0 <= b <= y.len() && trim_end_hspace(y) == y.take(b) && (forall|k: int|
            b <= k < y.len() ==> is_hspace(#[trigger] y[k])) && (b == 0 || !is_hspace(y[b - 1]));
    let u = trim_end_hspace(y);
    if u.len() > 0 {
        assert(u[0] == y[0]);
        assert(y[0] == t[a]);
        assert(u.last() == y[b - 1]);
        assert(u =~= t.subrange(a, a + b));
        assert forall|k: int| a + b <= k < t.len() implies is_hspace(#[trigger] t[k]) by {
            assert(t[k] == y[k - a]);
        }
    }
}

/// Decoding the escaped body of any CR- and LF-free name stops at the
/// closing quote and yields the name.
proof fn lemma_quote_escape_decodes(s: Seq<char>, rest: Seq<char>)
    requires
        !s.contains('\r'),
        !s.contains('\n'),
    ensures
        decode_quoted_chars(quote_escape(s) + seq!['"'] + rest) == Some((s, rest)),
    decreases s.len(),
{
    let t = quote_escape(s) + seq!['"'] + rest;
    if s.len() == 0 {
        assert(quote_escape(s) =~= Seq::<char>::empty());
        assert(t =~= seq!['"'] + rest);
        assert(t.drop_first() =~= rest);
    } else {
        let c = s[0];
        let tail = s.drop_first();
        assert(s.map_values(quote_escape_fn()).drop_first() =~= tail.map_values(quote_escape_fn()));
        assert(quote_escape(s) == quote_escape_char(c) + quote_escape(tail));
        assert(!tail.contains('\r')) by {
            if tail.contains('\r') {
                let k = choose|k: int| 0 <= k < tail.len() && tail[k] == '\r';
                assert(s[k + 1] == '\r');
            }
        }
        assert(!tail.contains('\n')) by {
            if tail.contains('\n') {
                let k = choose|k: int| 0 <= k < tail.len() && tail[k] == '\n';
                assert(s[k + 1] == '\n');
            }
        }
        assert(c != '\r' && c != '\n') by {
            assert(s[0] == c);
        }
        lemma_quote_escape_decodes(tail, rest);
        let after = quote_escape(tail) + seq!['"'] + rest;
        assert(t =~= quote_escape_char(c) + after);
        if c == '"' || c == '\\' {
            assert(t.subrange(2, t.len() as int) =~= after);
        } else {
            assert(t.drop_first() =~= after);
        }
        assert(seq![c] + tail =~= s);
    }
}

/// A name without a double quote or backslash is its own escaped body.
proof fn lemma_quote_escape_plain(s: Seq<char>)
    requires
        forall|k: int| 0 <= k < s.len() ==> #[trigger] s[k] != '"' && s[k] != '\\',
    ensures
        quote_escape(s) == s,
    decreases s.len(),
{
    if s.len() == 0 {
        assert(quote_escape(s) =~= s);
    } else {
        let tail = s.drop_first();
        assert(s.map_values(quote_escape_fn()).drop_first() =~= tail.map_values(quote_escape_fn()));
        assert(quote_escape(s) == quote_escape_char(s[0]) + quote_escape(tail));
        assert forall|k: int| 0 <= k < tail.len() implies #[trigger] tail[k] != '"' && tail[k]
            != '\\' by {
            assert(tail[k] == s[k + 1]);
        }
        lemma_quote_escape_plain(tail);
        assert(seq![s[0]] + tail =~= s);
    }
}

/// No `#`, `\\`, `[`, `]`, or double quote: text in which a bare expression
/// has no comment, escape, choice list, or quoting.
pub(crate) open spec fn is_plain_bare_text(s: Seq<char>) -> bool {
    forall|k: int|
        0 <= k < s.len() ==> {
            let c = #[trigger] s[k];
            c != '#' && c != '\\' && c != '[' && c != ']' && c != '"'
        }
}

proof fn lemma_plain_comment_start(s: Seq<char>, i: int)
    requires
        0 <= i <= s.len(),
        is_plain_bare_text(s),
    ensures
        comment_start(s, i) == s.len(),
    decreases s.len() - i,
{
    if i < s.len() {
        let c = s[i];
        assert(c != '#' && c != '\\');
        lemma_plain_comment_start(s, i + 1);
    }
}

proof fn lemma_plain_scan(s: Seq<char>, i: int)
    requires
        0 <= i <= s.len(),
        is_plain_bare_text(s),
    ensures
        scan_from(s, i, ScanRegion::Prefix) == Ok::<ScanView, ChoiceError>(ScanView::Plain),
    decreases s.len() - i,
{
    if i < s.len() {
        let c = s[i];
        assert(c != '\\' && c != '[' && c != ']' && c != '"');
        lemma_plain_scan(s, i + 1);
    }
}

proof fn lemma_plain_decode(s: Seq<char>)
    requires
        forall|k: int| 0 <= k < s.len() ==> #[trigger] s[k] != '\\',
    ensures
        decode_escapes(s) == s,
    decreases s.len(),
{
    if s.len() > 0 {
        assert(s[0] != '\\');
        assert forall|k: int| 0 <= k < s.drop_first().len() implies #[trigger] s.drop_first()[k]
            != '\\' by {
            assert(s.drop_first()[k] == s[k + 1]);
        }
        lemma_plain_decode(s.drop_first());
        assert(seq![s[0]] + s.drop_first() =~= s);
    }
}

// ---------------------------------------------------------------------------
// Contract theorems
// ---------------------------------------------------------------------------

/// The placeholder is recognized exactly when the entry is unquoted and its
/// comment-stripped, right-trimmed expression is exactly `...`; a quoted
/// `"..."` is the regular file `...`.
pub(crate) proof fn theorem_placeholder_iff_bare_ellipsis(content: Seq<char>, at_root: bool)
    ensures
        (entry_spec(content, at_root) matches Ok(v) && v.shape is Placeholder) == (!(
        content.len() > 0 && content[0] == '"') && bare_expression(content) == seq!['.', '.', '.']),
{
    if !(content.len() > 0 && content[0] == '"') {
        let expr = bare_expression(content);
        if expr.len() > 0 && expr != seq!['.', '.', '.'] {
            match scan_expression_spec(expr) {
                Ok(ScanView::Plain) => {
                    let p = decode_escapes(expr);
                    let directory = p.len() > 0 && p.last() == '/';
                    let d = if directory {
                        p.drop_last()
                    } else {
                        p
                    };
                    assert(bare_path_rule(d, at_root) == PathVerdict::Valid
                        || bare_path_rule(d, at_root) == PathVerdict::InvalidFormat
                        || bare_path_rule(d, at_root) == PathVerdict::SpecialDirectory);
                },
                _ => {},
            }
        }
    }
}

/// A quoted `"..."` entry is the regular file `...`, never the placeholder.
pub(crate) proof fn theorem_quoted_ellipsis_is_a_file(at_root: bool)
    ensures
        entry_spec(seq!['"', '.', '.', '.', '"'], at_root) == Ok::<EntryView, FailureView>(
            EntryView {
                shape: ShapeView::Single { path: seq!['.', '.', '.'], directory: false },
                comment: None,
            },
        ),
{
    let s = seq!['.', '.', '.'];
    lemma_quote_escape_plain(s);
    assert(!s.contains('\r') && !s.contains('\n'));
    lemma_quote_escape_decodes(s, Seq::empty());
    assert(quoted_component(s) =~= seq!['"', '.', '.', '.', '"']);
    assert(quoted_component(s) + Seq::<char>::empty() =~= quoted_component(s));
    theorem_quoted_paths_defer_containment(s, false, at_root);
}

/// Every path accepted from an unquoted entry, bare or from a choice list,
/// passes the validator's structural rules and is a contained relative
/// path without a `...` component or an edge space.
pub(crate) proof fn theorem_bare_paths_are_contained(content: Seq<char>, at_root: bool)
    requires
        !(content.len() > 0 && content[0] == '"'),
    ensures
        entry_spec(content, at_root) matches Ok(v) ==> forall|k: int|
            0 <= k < entry_items(v).len() ==> contained_bare_path(
                (#[trigger] entry_items(v)[k]).0,
                at_root,
            ),
{
    let expr = bare_expression(content);
    if entry_spec(content, at_root) is Ok && expr.len() > 0 && expr != seq!['.', '.', '.'] {
        let v = entry_spec(content, at_root)->Ok_0;
        match scan_expression_spec(expr) {
            Ok(ScanView::Plain) => {
                let p = decode_escapes(expr);
                let directory = p.len() > 0 && p.last() == '/';
                let d = if directory {
                    p.drop_last()
                } else {
                    p
                };
                lemma_bare_rule_contained(d, at_root);
                assert(entry_items(v) =~= seq![(d, directory)]);
            },
            Ok(ScanView::Choice { open, close }) => {
                let prefix = decode_escapes(expr.take(open));
                let suffix = decode_escapes(expr.skip(close + 1));
                let cs = alternatives_spec(expr.subrange(open + 1, close))->Ok_0;
                lemma_no_expansion_error(prefix, cs, suffix, at_root, 0);
                let paths = expansions(prefix, cs, suffix);
                assert forall|k: int| 0 <= k < entry_items(v).len() implies contained_bare_path(
                    (#[trigger] entry_items(v)[k]).0,
                    at_root,
                ) by {
                    assert(expansion_error_at(prefix, cs, suffix, at_root, k) is None);
                    assert(entry_items(v)[k].0 == paths[k]);
                    theorem_choice_expansions_pass_bare_rules(paths[k], at_root);
                }
            },
            Err(_) => {},
        }
    }
}

/// An accepted choice list comes from an unquoted entry and has between 2
/// and 256 pairwise-distinct regular-file expansions that share one parent
/// text and one sequence of parent components; its line cannot own
/// children.
pub(crate) proof fn theorem_choice_list_shape(content: Seq<char>, at_root: bool)
    ensures
        entry_spec(content, at_root) matches Ok(v) ==> (v.shape matches ShapeView::Choice {
            paths,
        } ==> {
            &&& !(content.len() > 0 && content[0] == '"')
            &&& 2 <= paths.len() <= 256
            &&& forall|j: int, k: int| 0 <= j < k < paths.len() ==> paths[j] != paths[k]
            &&& forall|k: int|
                0 <= k < paths.len() ==> parent_of(#[trigger] paths[k]) == parent_of(paths[0])
            &&& forall|k: int|
                0 <= k < paths.len() ==> split_components(#[trigger] paths[k]).drop_last()
                    == split_components(paths[0]).drop_last()
            &&& forall|k: int|
                0 <= k < paths.len() ==> valid_choice_expansion(#[trigger] paths[k], at_root)
            &&& !can_own_children(v)
        }),
{
    if content.len() > 0 && content[0] == '"' {
        return;
    }
    let expr = bare_expression(content);
    if entry_spec(content, at_root) is Ok && expr.len() > 0 && expr != seq!['.', '.', '.'] {
        match scan_expression_spec(expr) {
            Ok(ScanView::Choice { open, close }) => {
                let block = expr.subrange(open + 1, close);
                let prefix = decode_escapes(expr.take(open));
                let suffix = decode_escapes(expr.skip(close + 1));
                lemma_alternatives_shape(block);
                let cs = alternatives_spec(block)->Ok_0;
                lemma_no_expansion_error(prefix, cs, suffix, at_root, 0);
                let paths = expansions(prefix, cs, suffix);
                assert(expansion_error_at(prefix, cs, suffix, at_root, 0) is None);
                assert forall|j: int, k: int| 0 <= j < k < paths.len() implies paths[j]
                    != paths[k] by {
                    assert(expansion_error_at(prefix, cs, suffix, at_root, k) is None);
                    assert(paths[j] == expansion(prefix, cs[j], suffix));
                }
                assert forall|k: int| 0 <= k < paths.len() implies parent_of(#[trigger] paths[k])
                    == parent_of(paths[0]) && valid_choice_expansion(paths[k], at_root) by {
                    assert(expansion_error_at(prefix, cs, suffix, at_root, k) is None);
                }
                assert forall|k: int| 0 <= k < paths.len() implies split_components(
                    #[trigger] paths[k],
                ).drop_last() == split_components(paths[0]).drop_last() by {
                    assert(expansion_error_at(prefix, cs, suffix, at_root, k) is None);
                    lemma_same_parent_components(paths[k], paths[0], at_root);
                }
            },
            _ => {},
        }
    }
}

/// Plain bare text denotes itself: an entry without `#`, `\\`, `[`, `]`, or
/// a double quote, not ending with a space and not `...`, has no comment and
/// is exactly the single bare path (with its directory marker) that the
/// bare single-path rules decide. This is the bare half of the canonical
/// round trip; `theorem_quoted_component_entry` is the quoted half.
pub(crate) proof fn theorem_plain_bare_entry(s: Seq<char>, at_root: bool)
    requires
        s.len() > 0,
        is_plain_bare_text(s),
        s.last() != ' ',
        s != seq!['.', '.', '.'],
    ensures
        entry_spec(s, at_root) == match single_path_spec(s, at_root) {
            Ok(shape) => Ok::<EntryView, FailureView>(EntryView { shape, comment: None }),
            Err(f) => Err::<EntryView, FailureView>(f),
        },
{
    assert(s[0] != '"');
    lemma_plain_comment_start(s, 0);
    assert(s.take(s.len() as int) =~= s);
    assert(bare_expression(s) == s);
    lemma_plain_scan(s, 0);
    assert forall|k: int| 0 <= k < s.len() implies #[trigger] s[k] != '\\' by {
        let c = s[k];
        assert(c != '\\');
    }
    lemma_plain_decode(s);
}

/// A quoted entry is accepted exactly as the contract decoder
/// `decode_quoted_path` and the tail grammar say: with the decoded path, the
/// directory marker after the closing quote, and the comment.
pub(crate) proof fn theorem_quoted_path_decoder(content: Seq<char>, at_root: bool)
    requires
        content.len() > 0,
        content[0] == '"',
    ensures
        match decode_quoted_path(content) {
            None => entry_spec(content, at_root) is Err,
            Some((d, rest)) => match quoted_tail(rest) {
                None => entry_spec(content, at_root) is Err,
                Some((directory, comment)) => entry_spec(content, at_root) == Ok::<
                    EntryView,
                    FailureView,
                >(EntryView { shape: ShapeView::Single { path: d, directory }, comment }),
            },
        },
{
}

/// The parser accepts the quoted form of every nonempty name without CR or
/// LF, with or without a directory marker, as exactly that name: it checks
/// no component rule for quoted paths, so `"/etc/passwd"`, `".."`, `"a//b"`,
/// `"C:x"`, and names with other controls are all accepted here and their
/// containment rests entirely on the validator
/// (`crate::proven::path_rules::check_path_structure`).
pub(crate) proof fn theorem_quoted_paths_defer_containment(
    d: Seq<char>,
    directory: bool,
    at_root: bool,
)
    requires
        d.len() > 0,
        !d.contains('\r'),
        !d.contains('\n'),
    ensures
        entry_spec(
            quoted_component(d) + if directory {
                seq!['/']
            } else {
                Seq::<char>::empty()
            },
            at_root,
        ) == Ok::<EntryView, FailureView>(
            EntryView { shape: ShapeView::Single { path: d, directory }, comment: None },
        ),
{
    let tail = if directory {
        seq!['/']
    } else {
        Seq::<char>::empty()
    };
    let content = quoted_component(d) + tail;
    lemma_quote_escape_decodes(d, tail);
    assert(content.drop_first() =~= quote_escape(d) + seq!['"'] + tail);
    assert(content[0] == '"');
    if directory {
        assert(tail.drop_first() =~= Seq::<char>::empty());
    }
}

/// Canonical quoted names round-trip: the quoted form of a nonempty
/// control-free name is decoded by the contract decoder and by the parser
/// to exactly that name.
pub(crate) proof fn theorem_quoted_component_entry(d: Seq<char>, directory: bool, at_root: bool)
    requires
        d.len() > 0,
        !has_forbidden_control(d),
    ensures
        decode_quoted_path(quoted_component(d)) == Some((d, Seq::<char>::empty())),
        entry_spec(
            quoted_component(d) + if directory {
                seq!['/']
            } else {
                Seq::<char>::empty()
            },
            at_root,
        ) == Ok::<EntryView, FailureView>(
            EntryView { shape: ShapeView::Single { path: d, directory }, comment: None },
        ),
{
    crate::path_codec::theorem_quoted_component_decodes(d, Seq::empty());
    assert(quoted_component(d) + Seq::<char>::empty() =~= quoted_component(d));
    assert(!d.contains('\r')) by {
        if d.contains('\r') {
            let k = choose|k: int| 0 <= k < d.len() && d[k] == '\r';
            assert(crate::path_codec::is_forbidden_control(d[k]));
        }
    }
    assert(!d.contains('\n')) by {
        if d.contains('\n') {
            let k = choose|k: int| 0 <= k < d.len() && d[k] == '\n';
            assert(crate::path_codec::is_forbidden_control(d[k]));
        }
    }
    theorem_quoted_paths_defer_containment(d, directory, at_root);
}

/// Every returned comment is nonempty and has no leading or trailing space
/// or tab (`lemma_normalize_comment` further shows it is the infix of the
/// raw comment text that only spaces and tabs surround).
pub(crate) proof fn theorem_comment_is_normalized(content: Seq<char>, at_root: bool)
    ensures
        entry_spec(content, at_root) matches Ok(v) ==> (v.comment matches Some(c) ==> {
            &&& c.len() > 0
            &&& !is_hspace(c[0])
            &&& !is_hspace(c.last())
        }),
{
    if content.len() > 0 && content[0] == '"' {
        match decode_quoted_chars(content.drop_first()) {
            Some((decoded, rest)) => {
                let directory = rest.len() > 0 && rest[0] == '/';
                let after = if directory {
                    rest.drop_first()
                } else {
                    rest
                };
                let r = trim_start_spaces(after);
                if r.len() > 0 {
                    lemma_normalize_comment(r.drop_first());
                }
            },
            None => {},
        }
    } else {
        let h = comment_start(content, 0);
        if h < content.len() {
            lemma_normalize_comment(content.skip(h + 1));
        }
    }
}

} // verus!

/// The entry-text functions of the parser before this kernel, kept verbatim
/// (as free functions) as the differential-testing oracle for
/// [`parse_entry`] and its parser glue.
#[cfg(test)]
pub(crate) mod legacy_oracle {
    #![allow(clippy::all)]

    use crate::errors::{AppError, Result, SyntaxError};
    use crate::path_codec::{has_windows_drive_prefix, render_utf8_component};
    use crate::types::FilesystemItem;
    use std::collections::HashSet;

    const MIN_CHOICE_ALTERNATIVES: usize = 2;
    const MAX_CHOICE_ALTERNATIVES: usize = 256;

    #[derive(Clone, Copy)]
    enum ChoiceState {
        LeadingLayout,
        Unquoted,
        Quoted,
        QuotedClosed,
    }

    enum ParsedPathExpression {
        Placeholder,
        Bare(String),
        Quoted { path: String, directory: bool },
    }

    /// The previous per-line item construction of `parse_guide_content`.
    pub(crate) fn legacy_entry_items(
        content: &str,
        line_number: usize,
        at_root: bool,
    ) -> std::result::Result<(Vec<FilesystemItem>, bool), SyntaxError> {
        match entry_items(content, line_number, at_root) {
            Ok(result) => Ok(result),
            Err(AppError::Syntax(error)) => Err(error),
            Err(other) => panic!("the entry parser only reports syntax errors: {other:?}"),
        }
    }

    fn entry_items(
        content: &str,
        line_number: usize,
        at_root: bool,
    ) -> Result<(Vec<FilesystemItem>, bool)> {
        let (expression, comment) = parse_path_comment(content, line_number)?;
        let (parsed_items, line_can_own_children) = match expression {
            ParsedPathExpression::Placeholder => (
                vec![FilesystemItem::Placeholder {
                    comment: comment.clone(),
                }],
                false,
            ),
            ParsedPathExpression::Quoted { path, directory } => {
                let item = if directory {
                    FilesystemItem::Directory {
                        path,
                        comment: comment.clone(),
                        children: Vec::new(),
                    }
                } else {
                    FilesystemItem::File {
                        path,
                        comment: comment.clone(),
                    }
                };
                (vec![item], directory)
            }
            ParsedPathExpression::Bare(path) => {
                let (expanded_paths, is_choice) =
                    expand_wildcard_path_with_kind(&path, line_number, at_root)?;
                let line_can_own_children =
                    !is_choice && expanded_paths.len() == 1 && expanded_paths[0].ends_with('/');
                let mut parsed_items = Vec::with_capacity(expanded_paths.len());
                for expanded in expanded_paths {
                    validate_bare_expansion(&expanded, line_number, at_root)?;
                    if expanded.ends_with('/') {
                        parsed_items.push(FilesystemItem::Directory {
                            path: expanded
                                .strip_suffix('/')
                                .expect("ends_with established one directory marker")
                                .to_string(),
                            comment: comment.clone(),
                            children: Vec::new(),
                        });
                    } else {
                        parsed_items.push(FilesystemItem::File {
                            path: expanded,
                            comment: comment.clone(),
                        });
                    }
                }
                (parsed_items, line_can_own_children)
            }
        };
        Ok((parsed_items, line_can_own_children))
    }

    /// Parse path and optional comment from item content
    fn parse_path_comment(
        content: &str,
        line_number: usize,
    ) -> Result<(ParsedPathExpression, Option<String>)> {
        if content.starts_with('"') {
            return parse_quoted_path_comment(content, line_number);
        }

        let (raw_path, raw_comment) = split_path_comment(content);
        let path = raw_path.trim_end_matches(' ').to_string();
        let comment = normalize_comment(raw_comment);

        if path.is_empty() {
            return Err(invalid_path_error(line_number, "").into());
        }

        if path == "..." {
            return Ok((ParsedPathExpression::Placeholder, comment));
        }

        Ok((ParsedPathExpression::Bare(path), comment))
    }

    fn parse_quoted_path_comment(
        content: &str,
        line_number: usize,
    ) -> Result<(ParsedPathExpression, Option<String>)> {
        let mut decoded = String::new();
        let mut chars = content.char_indices();
        let Some((_, '"')) = chars.next() else {
            unreachable!("caller established an opening quote");
        };
        let mut closing_end = None;

        while let Some((index, ch)) = chars.next() {
            match ch {
                '"' => {
                    closing_end = Some(index + ch.len_utf8());
                    break;
                }
                '\\' => {
                    let Some((_, escaped)) = chars.next() else {
                        return Err(invalid_path_error(line_number, content).into());
                    };
                    if !matches!(escaped, '"' | '\\') {
                        return Err(invalid_path_error(line_number, content).into());
                    }
                    decoded.push(escaped);
                }
                '\r' | '\n' => {
                    return Err(invalid_path_error(line_number, content).into());
                }
                _ => decoded.push(ch),
            }
        }

        let Some(closing_end) = closing_end else {
            return Err(invalid_path_error(line_number, content).into());
        };
        if decoded.is_empty() {
            return Err(invalid_path_error(line_number, "").into());
        }

        let mut remainder = &content[closing_end..];
        let directory = remainder.starts_with('/');
        if directory {
            remainder = &remainder[1..];
        }
        remainder = remainder.trim_start_matches(' ');

        let comment = if remainder.is_empty() {
            None
        } else if let Some(comment) = remainder.strip_prefix('#') {
            normalize_comment(Some(comment))
        } else {
            return Err(invalid_path_error(line_number, content).into());
        };

        Ok((
            ParsedPathExpression::Quoted {
                path: decoded,
                directory,
            },
            comment,
        ))
    }

    fn normalize_comment(comment: Option<&str>) -> Option<String> {
        comment.and_then(|value| {
            let trimmed = value.trim_matches([' ', '\t']);
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
    }

    /// Split a bare list item at the first unescaped `#`.
    ///
    /// Quotes inside a choice are not whole-path quotes and deliberately do
    /// not suppress the outer comment delimiter.
    fn split_path_comment(content: &str) -> (&str, Option<&str>) {
        let mut escaped = false;

        for (idx, ch) in content.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }

            if ch == '\\' {
                escaped = true;
                continue;
            }

            if ch == '#' {
                return (&content[..idx], Some(&content[idx + 1..]));
            }
        }

        (content, None)
    }

    fn invalid_path_error(line: usize, path: &str) -> SyntaxError {
        SyntaxError::InvalidPathFormat {
            line,
            path: render_utf8_component(path),
        }
    }

    fn invalid_wildcard_error(line: usize, path: &str, message: impl Into<String>) -> SyntaxError {
        SyntaxError::InvalidWildcardSyntax {
            line,
            path: render_utf8_component(path),
            message: message.into(),
        }
    }

    /// Process escape sequences in a string, converting escaped characters to their literal forms.
    ///
    /// Handles the following escape sequences:
    /// - `\"` → `"`
    /// - `\,` → `,`
    /// - `\\` → `\`
    /// - `\[` → `[`
    /// - `\]` → `]`
    /// - `\#` → `#`
    /// - `\ ` → a literal U+0020 space
    ///
    /// # Arguments
    /// * `s` - The string containing escape sequences
    ///
    /// # Returns
    /// A new string with escape sequences processed
    fn process_escapes(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(&next) = chars.peek() {
                    // Consume the escaped character
                    chars.next();
                    result.push(next);
                } else {
                    // Trailing backslash - just include it
                    result.push(ch);
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Expand wildcard choices within a path, if present.
    ///
    /// This function processes paths containing choice blocks (syntax: `prefix[choice1, choice2]suffix`)
    /// and expands them into multiple paths. It supports:
    /// - Multiple choices separated by commas: `Foo[.h, .cpp]` → `["Foo.h", "Foo.cpp"]`
    /// - Quoted strings to preserve commas and special chars: `Foo["a, b", c]`
    /// - Escape sequences for literal special characters: `\,`, `\"`, `\\`, `\[`, `\]`
    /// - Prefix and suffix around the choice block: `src[/main, /lib].rs` → `["src/main.rs", "src/lib.rs"]`
    ///
    /// Escape sequences are preserved during parsing and processed at the end,
    /// ensuring consistent handling across prefix, choices, and suffix.
    ///
    /// # Arguments
    /// * `path` - The path potentially containing a choice block
    /// * `line_number` - Line number in the source file for error reporting
    ///
    /// # Returns
    /// A vector of expanded paths. Returns a single-element vector if no choice block is present.
    ///
    /// # Errors
    /// Returns `SyntaxError::InvalidWildcardSyntax` if:
    /// - The choice block is malformed (unterminated, invalid escapes, etc.)
    /// - Multiple choice blocks are present (only one is allowed per path)
    /// - The choice block is empty or contains only whitespace
    ///
    /// # Examples
    /// ```text
    /// // Single expansion (no choice block)
    /// expand_wildcard_path("foo.rs", 1) → Ok(vec!["foo.rs"])
    ///
    /// // Multiple choices
    /// expand_wildcard_path("File[.h, .cpp]", 1) → Ok(vec!["File.h", "File.cpp"])
    ///
    /// // With prefix and suffix
    /// expand_wildcard_path("src[/main, /lib].rs", 1) → Ok(vec!["src/main.rs", "src/lib.rs"])
    ///
    /// // Quoted strings and escapes
    /// expand_wildcard_path("file[\"a, b\", \\,c]", 1) → Ok(vec!["filea, b", "file,c"])
    /// ```
    /// Expand a path and report whether it contained a physical choice list.
    ///
    /// Hierarchy validation needs the source-level distinction because even a
    /// valid choice is not exactly one directory entry and therefore cannot
    /// own an indented child.
    fn expand_wildcard_path_with_kind(
        path: &str,
        line_number: usize,
        at_root: bool,
    ) -> Result<(Vec<String>, bool)> {
        let mut prefix = String::new();
        let mut suffix = String::new();
        let mut block_content = String::new();

        let mut in_block = false;
        let mut block_found = false;
        let mut in_quotes = false;
        let mut iter = path.chars().peekable();

        while let Some(ch) = iter.next() {
            match ch {
                '\\' => {
                    let next = iter.next().ok_or_else(|| {
                        invalid_wildcard_error(line_number, path, "incomplete escape sequence")
                    })?;
                    if !in_block && !valid_unquoted_choice_escape(next) {
                        return Err(invalid_wildcard_error(
                            line_number,
                            path,
                            "invalid escape in bare path",
                        )
                        .into());
                    }

                    // Preserve escape sequences consistently across prefix, block, and suffix
                    if in_block {
                        block_content.push('\\');
                        block_content.push(next);
                    } else if block_found {
                        suffix.push('\\');
                        suffix.push(next);
                    } else {
                        prefix.push('\\');
                        prefix.push(next);
                    }
                }
                '[' if !in_block => {
                    if block_found {
                        return Err(invalid_wildcard_error(
                            line_number,
                            path,
                            "multiple wildcard choice blocks are not supported",
                        )
                        .into());
                    }
                    block_found = true;
                    in_block = true;
                    in_quotes = false;
                }
                ']' if in_block && !in_quotes => {
                    in_block = false;
                    in_quotes = false;
                }
                ']' if in_block => {
                    block_content.push(ch);
                }
                ']' => {
                    return Err(invalid_wildcard_error(
                        line_number,
                        path,
                        "unmatched closing bracket",
                    )
                    .into());
                }
                '"' if in_block => {
                    in_quotes = !in_quotes;
                    block_content.push(ch);
                }
                '"' => {
                    return Err(invalid_wildcard_error(
                        line_number,
                        path,
                        "unescaped quote in bare path",
                    )
                    .into());
                }
                _ => {
                    if in_block {
                        block_content.push(ch);
                    } else if block_found {
                        suffix.push(ch);
                    } else {
                        prefix.push(ch);
                    }
                }
            }
        }

        if in_block {
            return Err(invalid_wildcard_error(
                line_number,
                path,
                "unterminated wildcard choice block",
            )
            .into());
        }

        if !block_found {
            // No wildcard block - just process escapes in the prefix and return
            return Ok((vec![process_escapes(&prefix)], false));
        }

        let choices = parse_choice_block(&block_content, path, line_number)?;
        let mut results = Vec::with_capacity(choices.len());

        // Process escapes in prefix and suffix once
        let processed_prefix = process_escapes(&prefix);
        let processed_suffix = process_escapes(&suffix);
        let mut unique_results = HashSet::with_capacity(choices.len());
        let mut expected_parent = None;

        for choice in choices {
            // Process escapes in each choice and combine with prefix/suffix
            let processed_choice = process_escapes(&choice);
            let mut expanded = processed_prefix.clone();
            expanded.push_str(&processed_choice);
            expanded.push_str(&processed_suffix);

            if !valid_choice_expansion(&expanded, at_root) {
                return Err(invalid_wildcard_error(
                    line_number,
                    path,
                    "choice expansions must be valid regular-file paths",
                )
                .into());
            }

            let parent = expanded
                .rsplit_once('/')
                .map_or("", |(parent, _)| parent)
                .to_string();
            if expected_parent
                .as_ref()
                .is_some_and(|expected: &String| expected != &parent)
            {
                return Err(invalid_wildcard_error(
                    line_number,
                    path,
                    "choice expansions must be sibling files",
                )
                .into());
            }
            expected_parent.get_or_insert(parent);

            if !unique_results.insert(expanded.clone()) {
                return Err(invalid_wildcard_error(
                    line_number,
                    path,
                    "choice expansions must be unique",
                )
                .into());
            }
            results.push(expanded);
        }

        Ok((results, true))
    }

    fn valid_choice_expansion(path: &str, at_root: bool) -> bool {
        if path.is_empty()
            || path == "..."
            || path.starts_with('/')
            || (at_root && path.starts_with('\\'))
            || path.starts_with(' ')
            || path.ends_with(['/', ' '])
        {
            return false;
        }

        let mut components = path.split('/');
        let Some(first) = components.next() else {
            return false;
        };
        if invalid_decoded_component(first) || (at_root && has_windows_drive_prefix(first)) {
            return false;
        }

        components.all(|component| !invalid_decoded_component(component))
    }

    fn invalid_decoded_component(component: &str) -> bool {
        component.is_empty()
            || matches!(component, "." | ".." | "...")
            || component.chars().any(|ch| ch <= '\u{1f}' || ch == '\u{7f}')
    }

    fn validate_bare_expansion(path: &str, line_number: usize, at_root: bool) -> Result<()> {
        let decoded = path.strip_suffix('/').unwrap_or(path);
        if decoded
            .split('/')
            .any(|component| matches!(component, "." | ".."))
        {
            return Err(SyntaxError::InvalidSpecialDirectory {
                line: line_number,
                path: render_utf8_component(decoded),
            }
            .into());
        }

        let mut components = decoded.split('/');
        let first = components.next().unwrap_or_default();
        let invalid = decoded.is_empty()
            || decoded.starts_with('/')
            || (at_root && decoded.starts_with('\\'))
            || decoded.starts_with(' ')
            || decoded.ends_with(' ')
            || invalid_decoded_component(first)
            || (at_root && has_windows_drive_prefix(first))
            || components.any(invalid_decoded_component);
        if invalid {
            return Err(invalid_path_error(line_number, decoded).into());
        }
        Ok(())
    }

    /// Parse the contents of a wildcard choice block into individual options.
    ///
    /// Takes the content between `[` and `]` and splits it into individual choices.
    /// This is a helper function for `expand_wildcard_path`.
    ///
    /// # Parsing Rules
    /// - Choices are separated by commas (`,`)
    /// - Commas inside quoted strings (`"..."`) are not treated as separators
    /// - Surrounding spaces and tabs outside quotes are ignored
    /// - Interior unquoted whitespace is preserved
    /// - Whitespace inside quotes is preserved
    /// - Escape sequences (`\,`, `\"`, etc.) are preserved for later processing
    /// - Quote characters (`"`) toggle quote mode but are not included in output
    ///
    /// # Arguments
    /// * `content` - The string content between `[` and `]` (without the brackets)
    /// * `path` - The full original path for error messages
    /// * `line_number` - Line number in the source file for error reporting
    ///
    /// # Returns
    /// A vector of choice strings with escape sequences still intact (to be processed by caller).
    ///
    /// # Errors
    /// Returns `SyntaxError::InvalidWildcardSyntax` if:
    /// - Quote strings are unterminated
    /// - Escape sequences are incomplete (trailing backslash)
    /// - The choice block is empty or all choices are empty/whitespace
    /// - The choice count falls outside 2–256
    ///
    /// # Examples
    /// ```text
    /// parse_choice_block("a, b, c", "path", 1) → Ok(vec!["a", "b", "c"])
    /// parse_choice_block("\"a, b\", c", "path", 1) → Ok(vec!["a, b", "c"])
    /// parse_choice_block("\\,a, b", "path", 1) → Ok(vec!["\\,a", "b"])  // Escape preserved
    /// parse_choice_block("  a  ,  b  ", "path", 1) → Ok(vec!["a", "b"])  // Trimmed
    /// ```
    fn parse_choice_block(content: &str, path: &str, line_number: usize) -> Result<Vec<String>> {
        let mut choices = Vec::new();
        let mut current = String::new();
        let mut pending_layout = String::new();
        let mut chars = content.chars().peekable();
        let mut state = ChoiceState::LeadingLayout;

        while let Some(ch) = chars.next() {
            match state {
                ChoiceState::LeadingLayout => match ch {
                    ' ' | '\t' => {}
                    ',' => push_choice_before_separator(
                        &mut choices,
                        String::new(),
                        path,
                        line_number,
                    )?,
                    '"' => state = ChoiceState::Quoted,
                    '\\' => {
                        let next = chars.next().ok_or_else(|| {
                            invalid_wildcard_error(line_number, path, "incomplete escape sequence")
                        })?;
                        if !valid_unquoted_choice_escape(next) {
                            return Err(invalid_wildcard_error(
                                line_number,
                                path,
                                "invalid escape in unquoted choice",
                            )
                            .into());
                        }
                        current.push('\\');
                        current.push(next);
                        state = ChoiceState::Unquoted;
                    }
                    '[' | ']' => {
                        return Err(invalid_wildcard_error(
                            line_number,
                            path,
                            "unescaped bracket in unquoted choice",
                        )
                        .into());
                    }
                    _ => {
                        current.push(ch);
                        state = ChoiceState::Unquoted;
                    }
                },
                ChoiceState::Unquoted => match ch {
                    ',' => {
                        pending_layout.clear();
                        push_choice_before_separator(
                            &mut choices,
                            std::mem::take(&mut current),
                            path,
                            line_number,
                        )?;
                        state = ChoiceState::LeadingLayout;
                    }
                    ' ' | '\t' => pending_layout.push(ch),
                    '\\' => {
                        let next = chars.next().ok_or_else(|| {
                            invalid_wildcard_error(line_number, path, "incomplete escape sequence")
                        })?;
                        if !valid_unquoted_choice_escape(next) {
                            return Err(invalid_wildcard_error(
                                line_number,
                                path,
                                "invalid escape in unquoted choice",
                            )
                            .into());
                        }
                        current.push_str(&pending_layout);
                        pending_layout.clear();
                        current.push('\\');
                        current.push(next);
                    }
                    '"' => {
                        return Err(invalid_wildcard_error(
                            line_number,
                            path,
                            "quote must begin a choice alternative",
                        )
                        .into());
                    }
                    '[' | ']' => {
                        return Err(invalid_wildcard_error(
                            line_number,
                            path,
                            "unescaped bracket in unquoted choice",
                        )
                        .into());
                    }
                    _ => {
                        current.push_str(&pending_layout);
                        pending_layout.clear();
                        current.push(ch);
                    }
                },
                ChoiceState::Quoted => match ch {
                    '"' => state = ChoiceState::QuotedClosed,
                    '\\' => {
                        let next = chars.next().ok_or_else(|| {
                            invalid_wildcard_error(line_number, path, "incomplete escape sequence")
                        })?;
                        if !matches!(next, '"' | '\\' | '#') {
                            return Err(invalid_wildcard_error(
                                line_number,
                                path,
                                "invalid escape in quoted choice",
                            )
                            .into());
                        }
                        current.push('\\');
                        current.push(next);
                    }
                    _ => current.push(ch),
                },
                ChoiceState::QuotedClosed => match ch {
                    ' ' | '\t' => {}
                    ',' => {
                        push_choice_before_separator(
                            &mut choices,
                            std::mem::take(&mut current),
                            path,
                            line_number,
                        )?;
                        state = ChoiceState::LeadingLayout;
                    }
                    _ => {
                        return Err(invalid_wildcard_error(
                            line_number,
                            path,
                            "quoted choice must end before the separator",
                        )
                        .into());
                    }
                },
            }
        }

        if matches!(state, ChoiceState::Quoted) {
            return Err(invalid_wildcard_error(
                line_number,
                path,
                "unterminated quoted string in wildcard choices",
            )
            .into());
        }

        pending_layout.clear();
        if choices.len() >= MAX_CHOICE_ALTERNATIVES {
            return Err(choice_count_error(path, line_number).into());
        }
        choices.push(current);

        // Validate that the choice block is not empty
        if choices.is_empty() || choices.iter().all(|c| c.is_empty()) {
            return Err(
                invalid_wildcard_error(line_number, path, "choice block cannot be empty").into(),
            );
        }

        if !(MIN_CHOICE_ALTERNATIVES..=MAX_CHOICE_ALTERNATIVES).contains(&choices.len()) {
            return Err(choice_count_error(path, line_number).into());
        }

        Ok(choices)
    }

    fn push_choice_before_separator(
        choices: &mut Vec<String>,
        choice: String,
        path: &str,
        line_number: usize,
    ) -> Result<()> {
        // A separator guarantees that one more alternative follows. Reject
        // before allocating the 256th completed token, which would imply at
        // least 257 alternatives once the final token is added.
        if choices.len() >= MAX_CHOICE_ALTERNATIVES - 1 {
            return Err(choice_count_error(path, line_number).into());
        }
        choices.push(choice);
        Ok(())
    }

    fn choice_count_error(path: &str, line_number: usize) -> SyntaxError {
        invalid_wildcard_error(
            line_number,
            path,
            format!(
                "choice list must contain between {MIN_CHOICE_ALTERNATIVES} and \
                 {MAX_CHOICE_ALTERNATIVES} alternatives"
            ),
        )
    }

    fn valid_unquoted_choice_escape(ch: char) -> bool {
        matches!(ch, '#' | '\\' | '[' | ']' | ',' | '"' | ' ')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single(path: &str, directory: bool, comment: Option<&str>) -> ParsedEntry {
        ParsedEntry {
            shape: EntryShape::Single {
                path: path.to_string(),
                directory,
            },
            comment: comment.map(str::to_string),
        }
    }

    fn failure(kind: EntryErrorKind, subject: &str) -> EntryFailure {
        EntryFailure {
            kind,
            subject: subject.to_string(),
        }
    }

    #[test]
    fn placeholders_are_the_exact_unquoted_ellipsis() {
        assert_eq!(
            parse_entry("...", true),
            Ok(ParsedEntry {
                shape: EntryShape::Placeholder,
                comment: None
            })
        );
        assert_eq!(
            parse_entry("...   #\t future \t", false),
            Ok(ParsedEntry {
                shape: EntryShape::Placeholder,
                comment: Some("future".to_string())
            })
        );
        assert_eq!(parse_entry("\"...\"", true), Ok(single("...", false, None)));
        assert_eq!(parse_entry("....", true), Ok(single("....", false, None)));
        assert_eq!(
            parse_entry("a/.../b", true),
            Err(failure(EntryErrorKind::InvalidPath, "a/.../b"))
        );
        assert!(!parse_entry("...", true).unwrap().can_own_children());
    }

    #[test]
    fn quoted_paths_decode_only_quote_and_backslash_escapes() {
        assert_eq!(
            parse_entry(r#"" a#[b], \"c\" \\ "/  # note "#, true),
            Ok(single(r#" a#[b], "c" \ "#, true, Some("note")))
        );
        assert!(parse_entry("\"src\"/", true).unwrap().can_own_children());
        assert_eq!(
            parse_entry("\"/etc/..//x\"", true),
            Ok(single("/etc/..//x", false, None))
        );
        assert_eq!(parse_entry("\"a\"#", true), Ok(single("a", false, None)));
        for malformed in [
            r#""a\q""#, r#""a\"#, "\"a", "\"a\nb\"", "\"a\rb\"", "\"a\"x", "\"a\"//",
        ] {
            assert_eq!(
                parse_entry(malformed, true),
                Err(failure(EntryErrorKind::InvalidPath, malformed)),
                "{malformed:?}"
            );
        }
        assert_eq!(
            parse_entry("\"\"/ # c", true),
            Err(failure(EntryErrorKind::InvalidPath, ""))
        );
    }

    #[test]
    fn bare_paths_follow_the_comment_padding_and_component_rules() {
        assert_eq!(
            parse_entry(r"docs/issue\#1.md   # ticket #1 ", true),
            Ok(single("docs/issue#1.md", false, Some("ticket #1")))
        );
        assert_eq!(
            parse_entry("src/  #  ", true),
            Ok(single("src", true, None))
        );
        assert!(parse_entry("src/", true).unwrap().can_own_children());
        assert!(!parse_entry("src", true).unwrap().can_own_children());
        assert_eq!(
            parse_entry("   # only a comment", true),
            Err(failure(EntryErrorKind::InvalidPath, ""))
        );
        assert_eq!(
            parse_entry("a/./b/", true),
            Err(failure(EntryErrorKind::SpecialDirectory, "a/./b"))
        );
        assert_eq!(
            parse_entry("/./x", true),
            Err(failure(EntryErrorKind::SpecialDirectory, "/./x"))
        );
        for (text, subject) in [
            ("/etc", "/etc"),
            ("foo//", "foo/"),
            ("a//b", "a//b"),
            (r"\ a", " a"),
            ("C:/x", "C:/x"),
            ("a\u{1}b", "a\u{1}b"),
        ] {
            assert_eq!(
                parse_entry(text, true),
                Err(failure(EntryErrorKind::InvalidPath, subject)),
                "{text:?}"
            );
        }
        assert_eq!(
            parse_entry(r"\\x", true),
            Err(failure(EntryErrorKind::InvalidPath, r"\x"))
        );
        assert_eq!(parse_entry(r"\\x", false), Ok(single(r"\x", false, None)));
        assert_eq!(parse_entry("C:x", false), Ok(single("C:x", false, None)));
        assert_eq!(
            parse_entry("dir/C:notes", true),
            Ok(single("dir/C:notes", false, None))
        );
        // Contract deviation kept for compatibility: a trailing escaped
        // space loses its space to line padding and reports a dangling
        // escape instead of an invalid edge space.
        assert_eq!(
            parse_entry(r"foo\ ", true),
            Err(failure(
                EntryErrorKind::Choice(ChoiceError::IncompleteEscape),
                r"foo\"
            ))
        );
    }

    #[test]
    fn choice_lists_expand_to_sibling_files_sharing_the_comment() {
        assert_eq!(
            parse_entry("F[.h, .cpp] # pair", true),
            Ok(ParsedEntry {
                shape: EntryShape::Choice(vec!["F.h".to_string(), "F.cpp".to_string()]),
                comment: Some("pair".to_string()),
            })
        );
        assert!(!parse_entry("F[.h, .cpp]", true).unwrap().can_own_children());
        assert_eq!(
            parse_entry("F[a, a]  # c", true),
            Err(failure(
                EntryErrorKind::Choice(ChoiceError::DuplicateExpansion),
                "F[a, a]"
            ))
        );
        assert_eq!(
            parse_entry("F[a, b", true),
            Err(failure(
                EntryErrorKind::Choice(ChoiceError::UnterminatedBlock),
                "F[a, b"
            ))
        );
    }

    #[test]
    fn comments_are_trimmed_of_spaces_and_tabs_only() {
        assert_eq!(
            normalized_comment(&[' ', '\t', 'x', ' ', 'y', '\t'], 0, 6),
            Some("x y".to_string())
        );
        assert_eq!(normalized_comment(&[' ', '\t'], 0, 2), None);
        assert_eq!(
            normalized_comment(&['\u{a0}', 'x'], 0, 2),
            Some("\u{a0}x".to_string())
        );
        assert_eq!(normalized_comment(&['x'], 1, 1), None);
        assert_eq!(find_comment_start(&['a', '\\', '#', '#']), 3);
        assert_eq!(find_comment_start(&['a', '\\']), 2);
        assert_eq!(find_comment_start(&['a']), 1);
        assert_eq!(collect_chars("aé😀"), vec!['a', 'é', '😀']);
    }
}
