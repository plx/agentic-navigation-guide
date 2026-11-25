//! Verified choice-list scanning, tokenizing, and expansion.
//!
//! A bare path expression (`docs/v0.2-contract.md`, "Path expressions" and
//! "Choice lists") is checked in three phases, in the parser's diagnostic
//! order:
//!
//! 1. [`scan_expression`] walks the raw expression once, validates the
//!    escapes outside a choice list, and locates the one permitted list
//!    (`scan_from`);
//! 2. [`expand_choice_list`] splits the list body into decoded alternatives
//!    with the four-state tokenizer (`tokens_from`) and the 2..=256
//!    alternative bound;
//! 3. it then forms every expansion `prefix + alternative + suffix` and
//!    checks, alternative by alternative, that the expansion is a valid
//!    regular-file path, has the first expansion's parent, and is new
//!    (`first_expansion_error`).
//!
//! The specifications mirror the scanners' left-to-right order because that
//! order decides which diagnostic wins; the contract meaning of an accepted
//! list is proven separately in `crate::proven::entry`
//! (`theorem_choice_list_shape` and `theorem_bare_paths_are_contained`).
//! [`component_report`] and [`decode_escapes_exec`] are shared with the
//! bare single-path rules there.

use crate::path_codec::is_forbidden_control_char;
#[cfg(verus_keep_ghost)]
use crate::path_codec::{
    has_drive_prefix, has_forbidden_control, is_ascii_letter, is_forbidden_control,
    is_reserved_ellipsis,
};
#[cfg(verus_keep_ghost)]
use crate::proven::path_rules::{is_dot_component, split_components, split_step};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Largest number of alternatives in one choice list.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const MAX_CHOICE_ALTERNATIVES: usize = 256;
/// Smallest number of alternatives in one choice list.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const MIN_CHOICE_ALTERNATIVES: usize = 2;

/// Why a bare expression is not a valid choice expression. Each variant is
/// one `SyntaxError::InvalidWildcardSyntax` message.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChoiceError {
    /// A backslash ends the expression or the list.
    IncompleteEscape,
    /// An escape outside the list that is not a bare-path escape.
    InvalidBareEscape,
    /// A second `[` after the first list closed.
    MultipleBlocks,
    /// An unescaped `]` outside a list.
    UnmatchedClosingBracket,
    /// An unescaped `"` outside a list.
    UnescapedQuote,
    /// A list without its closing `]`.
    UnterminatedBlock,
    /// An escape in an unquoted alternative that is not a bare-path escape.
    InvalidUnquotedEscape,
    /// An unescaped `[` or `]` in an unquoted alternative.
    UnescapedBracket,
    /// A `"` after the start of an unquoted alternative.
    MisplacedQuote,
    /// An escape in a quoted alternative other than `\"`, `\\`, or `\#`.
    InvalidQuotedEscape,
    /// Text other than spaces or tabs after a quoted alternative.
    QuotedTrailingText,
    /// A quoted alternative without its closing quote.
    UnterminatedQuote,
    /// Every alternative is empty.
    EmptyBlock,
    /// Fewer than two or more than 256 alternatives.
    AlternativeCount,
    /// An expansion that is not a valid regular-file path.
    InvalidExpansion,
    /// An expansion whose parent differs from the first expansion's parent.
    NotSiblings,
    /// An expansion equal to an earlier one.
    DuplicateExpansion,
}

/// The location of the one choice list in a scanned expression.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExpressionScan {
    /// No choice list.
    Plain,
    /// The list's `[` is at `open` and its `]` at `close`.
    Choice { open: usize, close: usize },
}

/// Which part of the expression the scanner is in.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScanPart {
    Prefix,
    Block,
    Suffix,
}

/// The tokenizer state inside a choice list.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenState {
    /// Before an alternative: layout is skipped.
    Leading,
    /// Inside an unquoted alternative.
    Unquoted,
    /// Inside a quoted alternative.
    Quoted,
    /// After a quoted alternative's closing quote.
    Closed,
}

/// The `/`-separated component classes of one decoded path.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ComponentReport {
    /// Some component is `.` or `..`.
    pub(crate) dot: bool,
    /// Some component is empty, `.`, `..`, `...`, or has a forbidden control.
    pub(crate) invalid: bool,
    /// The first component has a drive prefix.
    pub(crate) drive: bool,
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Scalar classes
// ---------------------------------------------------------------------------

/// The scalars a bare-path escape may name: `#`, `\`, `[`, `]`, comma,
/// double quote, and U+0020.
pub(crate) open spec fn is_bare_escape(c: char) -> bool {
    c == '#' || c == '\\' || c == '[' || c == ']' || c == ',' || c == '"' || c == ' '
}

/// The scalars a quoted-choice escape may name: double quote, `\`, and `#`.
pub(crate) open spec fn is_quoted_choice_escape(c: char) -> bool {
    c == '"' || c == '\\' || c == '#'
}

/// Layout around an alternative: U+0020 SPACE and U+0009 TAB.
pub(crate) open spec fn is_layout(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// Decodes bare-path escapes: a backslash and the following scalar denote
/// that scalar, and a final lone backslash denotes itself.
pub(crate) open spec fn decode_escapes(s: Seq<char>) -> Seq<char>
    decreases s.len(),
{
    if s.len() == 0 {
        Seq::empty()
    } else if s[0] == '\\' && s.len() >= 2 {
        seq![s[1]] + decode_escapes(s.subrange(2, s.len() as int))
    } else {
        seq![s[0]] + decode_escapes(s.drop_first())
    }
}

// ---------------------------------------------------------------------------
// Phase 1: the expression scan
// ---------------------------------------------------------------------------

/// The scanner's position relative to the choice list.
pub(crate) enum ScanRegion {
    Prefix,
    Block { open: int, quoted: bool },
    Suffix { open: int, close: int },
}

/// The scan outcome with scalar indices.
pub(crate) enum ScanView {
    Plain,
    Choice { open: int, close: int },
}

/// The expression scan from index `i` in `region`: backslash escapes are
/// tokens of two scalars (validated outside the list), the first unescaped
/// `[` opens the list, the first unescaped `]` outside quotes closes it,
/// quotes toggle only inside the list, and an unescaped `]` or quote outside
/// the list is rejected.
pub(crate) open spec fn scan_from(s: Seq<char>, i: int, region: ScanRegion) -> Result<
    ScanView,
    ChoiceError,
>
    decreases s.len() - i,
{
    if i >= s.len() {
        match region {
            ScanRegion::Prefix => Ok(ScanView::Plain),
            ScanRegion::Block { .. } => Err(ChoiceError::UnterminatedBlock),
            ScanRegion::Suffix { open, close } => Ok(ScanView::Choice { open, close }),
        }
    } else {
        let c = s[i];
        if c == '\\' {
            if i + 1 >= s.len() {
                Err(ChoiceError::IncompleteEscape)
            } else if !(region is Block) && !is_bare_escape(s[i + 1]) {
                Err(ChoiceError::InvalidBareEscape)
            } else {
                scan_from(s, i + 2, region)
            }
        } else if c == '[' && !(region is Block) {
            if region is Suffix {
                Err(ChoiceError::MultipleBlocks)
            } else {
                scan_from(s, i + 1, ScanRegion::Block { open: i, quoted: false })
            }
        } else if c == ']' {
            match region {
                ScanRegion::Block { open, quoted } => if quoted {
                    scan_from(s, i + 1, region)
                } else {
                    scan_from(s, i + 1, ScanRegion::Suffix { open, close: i })
                },
                _ => Err(ChoiceError::UnmatchedClosingBracket),
            }
        } else if c == '"' {
            match region {
                ScanRegion::Block { open, quoted } => scan_from(
                    s,
                    i + 1,
                    ScanRegion::Block { open, quoted: !quoted },
                ),
                _ => Err(ChoiceError::UnescapedQuote),
            }
        } else {
            scan_from(s, i + 1, region)
        }
    }
}

/// The scan of a whole bare expression.
pub(crate) open spec fn scan_expression_spec(s: Seq<char>) -> Result<ScanView, ChoiceError> {
    scan_from(s, 0, ScanRegion::Prefix)
}

pub(crate) open spec fn scan_view(r: Result<ExpressionScan, ChoiceError>) -> Result<
    ScanView,
    ChoiceError,
> {
    match r {
        Ok(ExpressionScan::Plain) => Ok(ScanView::Plain),
        Ok(ExpressionScan::Choice { open, close }) => Ok(
            ScanView::Choice { open: open as int, close: close as int },
        ),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: alternatives
// ---------------------------------------------------------------------------

/// The tokenizer state; an unquoted alternative carries the layout seen
/// since its last non-layout scalar.
pub(crate) enum TokenSpecState {
    Leading,
    Unquoted { pending: Seq<char> },
    Quoted,
    Closed,
}

/// The end of the list: an open quote is rejected, then the final
/// alternative is added, then an all-empty list and the count bound are
/// checked. (Separators already stop at 255 completed alternatives, so the
/// final one never exceeds the bound; the previous parser's redundant check
/// before adding it is omitted.)
pub(crate) open spec fn finish_tokens(
    state: TokenSpecState,
    cur: Seq<char>,
    done: Seq<Seq<char>>,
) -> Result<Seq<Seq<char>>, ChoiceError> {
    if state is Quoted {
        Err(ChoiceError::UnterminatedQuote)
    } else {
        let all = done.push(cur);
        if forall|k: int| 0 <= k < all.len() ==> (#[trigger] all[k]).len() == 0 {
            Err(ChoiceError::EmptyBlock)
        } else if all.len() < 2 || all.len() > 256 {
            Err(ChoiceError::AlternativeCount)
        } else {
            Ok(all)
        }
    }
}

/// The decoded alternatives of the list body `s` from index `i`: the
/// four-state tokenizer with the decoded current alternative `cur` and the
/// completed alternatives `done`.
pub(crate) open spec fn tokens_from(
    s: Seq<char>,
    i: int,
    state: TokenSpecState,
    cur: Seq<char>,
    done: Seq<Seq<char>>,
) -> Result<Seq<Seq<char>>, ChoiceError>
    decreases s.len() - i,
{
    if i >= s.len() {
        finish_tokens(state, cur, done)
    } else {
        let c = s[i];
        match state {
            TokenSpecState::Leading => {
                if is_layout(c) {
                    tokens_from(s, i + 1, state, cur, done)
                } else if c == ',' {
                    if done.len() >= 255 {
                        Err(ChoiceError::AlternativeCount)
                    } else {
                        tokens_from(s, i + 1, state, cur, done.push(Seq::empty()))
                    }
                } else if c == '"' {
                    tokens_from(s, i + 1, TokenSpecState::Quoted, cur, done)
                } else if c == '\\' {
                    if i + 1 >= s.len() {
                        Err(ChoiceError::IncompleteEscape)
                    } else if !is_bare_escape(s[i + 1]) {
                        Err(ChoiceError::InvalidUnquotedEscape)
                    } else {
                        tokens_from(
                            s,
                            i + 2,
                            TokenSpecState::Unquoted { pending: Seq::empty() },
                            cur.push(s[i + 1]),
                            done,
                        )
                    }
                } else if c == '[' || c == ']' {
                    Err(ChoiceError::UnescapedBracket)
                } else {
                    tokens_from(
                        s,
                        i + 1,
                        TokenSpecState::Unquoted { pending: Seq::empty() },
                        cur.push(c),
                        done,
                    )
                }
            },
            TokenSpecState::Unquoted { pending } => {
                if c == ',' {
                    if done.len() >= 255 {
                        Err(ChoiceError::AlternativeCount)
                    } else {
                        tokens_from(s, i + 1, TokenSpecState::Leading, Seq::empty(), done.push(cur))
                    }
                } else if is_layout(c) {
                    tokens_from(s, i + 1, TokenSpecState::Unquoted { pending: pending.push(c) }, cur, done)
                } else if c == '\\' {
                    if i + 1 >= s.len() {
                        Err(ChoiceError::IncompleteEscape)
                    } else if !is_bare_escape(s[i + 1]) {
                        Err(ChoiceError::InvalidUnquotedEscape)
                    } else {
                        tokens_from(
                            s,
                            i + 2,
                            TokenSpecState::Unquoted { pending: Seq::empty() },
                            (cur + pending).push(s[i + 1]),
                            done,
                        )
                    }
                } else if c == '"' {
                    Err(ChoiceError::MisplacedQuote)
                } else if c == '[' || c == ']' {
                    Err(ChoiceError::UnescapedBracket)
                } else {
                    tokens_from(
                        s,
                        i + 1,
                        TokenSpecState::Unquoted { pending: Seq::empty() },
                        (cur + pending).push(c),
                        done,
                    )
                }
            },
            TokenSpecState::Quoted => {
                if c == '"' {
                    tokens_from(s, i + 1, TokenSpecState::Closed, cur, done)
                } else if c == '\\' {
                    if i + 1 >= s.len() {
                        Err(ChoiceError::IncompleteEscape)
                    } else if !is_quoted_choice_escape(s[i + 1]) {
                        Err(ChoiceError::InvalidQuotedEscape)
                    } else {
                        tokens_from(s, i + 2, state, cur.push(s[i + 1]), done)
                    }
                } else {
                    tokens_from(s, i + 1, state, cur.push(c), done)
                }
            },
            TokenSpecState::Closed => {
                if is_layout(c) {
                    tokens_from(s, i + 1, state, cur, done)
                } else if c == ',' {
                    if done.len() >= 255 {
                        Err(ChoiceError::AlternativeCount)
                    } else {
                        tokens_from(s, i + 1, TokenSpecState::Leading, Seq::empty(), done.push(cur))
                    }
                } else {
                    Err(ChoiceError::QuotedTrailingText)
                }
            },
        }
    }
}

/// The decoded alternatives of a whole list body.
pub(crate) open spec fn alternatives_spec(block: Seq<char>) -> Result<Seq<Seq<char>>, ChoiceError> {
    tokens_from(block, 0, TokenSpecState::Leading, Seq::empty(), Seq::empty())
}

// ---------------------------------------------------------------------------
// Phase 3: expansions
// ---------------------------------------------------------------------------

/// A component that no decoded bare path or expansion may contain: empty,
/// `.`, `..`, the reserved `...`, or one with a forbidden control.
pub(crate) open spec fn invalid_decoded_component(c: Seq<char>) -> bool {
    c.len() == 0 || is_dot_component(c) || is_reserved_ellipsis(c) || has_forbidden_control(c)
}

/// Some `/`-separated component of `p` is `.` or `..`.
pub(crate) open spec fn has_dot_component(p: Seq<char>) -> bool {
    exists|k: int|
        0 <= k < split_components(p).len() && #[trigger] is_dot_component(split_components(p)[k])
}

/// Some `/`-separated component of `p` is invalid.
pub(crate) open spec fn has_invalid_component(p: Seq<char>) -> bool {
    exists|k: int|
        0 <= k < split_components(p).len() && #[trigger] invalid_decoded_component(
            split_components(p)[k],
        )
}

/// At the guide root, the first component of `p` has a drive prefix.
pub(crate) open spec fn root_drive(p: Seq<char>, at_root: bool) -> bool {
    at_root && has_drive_prefix(split_components(p)[0])
}

/// An expansion is a valid regular-file path: nonempty, not `...`, not
/// starting with `/`, ` `, or (at the root) `\`, not ending with `/` or ` `,
/// with no invalid component and no root drive prefix.
pub(crate) open spec fn valid_choice_expansion(p: Seq<char>, at_root: bool) -> bool {
    &&& p.len() > 0
    &&& !is_reserved_ellipsis(p)
    &&& p[0] != '/'
    &&& !(at_root && p[0] == '\\')
    &&& p[0] != ' '
    &&& p.last() != '/'
    &&& p.last() != ' '
    &&& !has_invalid_component(p)
    &&& !root_drive(p, at_root)
}

/// The number of scalars before the last `/` of `s` (zero without one).
pub(crate) open spec fn parent_len(s: Seq<char>) -> int
    decreases s.len(),
{
    if s.len() == 0 {
        0
    } else if s.last() == '/' {
        s.len() - 1
    } else {
        parent_len(s.drop_last())
    }
}

/// The text before the last `/` of `s`, or empty text without one.
pub(crate) open spec fn parent_of(s: Seq<char>) -> Seq<char> {
    s.take(parent_len(s))
}

/// The expansion of one alternative.
pub(crate) open spec fn expansion(prefix: Seq<char>, choice: Seq<char>, suffix: Seq<char>) -> Seq<
    char,
> {
    prefix + choice + suffix
}

/// Every expansion, in alternative order.
pub(crate) open spec fn expansions(
    prefix: Seq<char>,
    choices: Seq<Seq<char>>,
    suffix: Seq<char>,
) -> Seq<Seq<char>> {
    Seq::new(choices.len(), |k: int| expansion(prefix, choices[k], suffix))
}

/// The first failing rule for alternative `k`: validity, then a shared
/// parent with the first expansion, then uniqueness.
pub(crate) open spec fn expansion_error_at(
    prefix: Seq<char>,
    choices: Seq<Seq<char>>,
    suffix: Seq<char>,
    at_root: bool,
    k: int,
) -> Option<ChoiceError> {
    let e = expansion(prefix, choices[k], suffix);
    if !valid_choice_expansion(e, at_root) {
        Some(ChoiceError::InvalidExpansion)
    } else if parent_of(e) != parent_of(expansion(prefix, choices[0], suffix)) {
        Some(ChoiceError::NotSiblings)
    } else if exists|j: int| 0 <= j < k && #[trigger] expansion(prefix, choices[j], suffix) == e {
        Some(ChoiceError::DuplicateExpansion)
    } else {
        None
    }
}

/// The first failing alternative from `k` on.
pub(crate) open spec fn first_expansion_error(
    prefix: Seq<char>,
    choices: Seq<Seq<char>>,
    suffix: Seq<char>,
    at_root: bool,
    k: int,
) -> Option<ChoiceError>
    decreases choices.len() - k,
{
    if k < 0 || k >= choices.len() {
        None
    } else if expansion_error_at(prefix, choices, suffix, at_root, k) is Some {
        expansion_error_at(prefix, choices, suffix, at_root, k)
    } else {
        first_expansion_error(prefix, choices, suffix, at_root, k + 1)
    }
}

/// A whole choice list: the raw prefix before `[`, the list body, and the
/// raw suffix after `]` yield the expansions or the first error.
pub(crate) open spec fn choice_list_spec(
    raw_prefix: Seq<char>,
    block: Seq<char>,
    raw_suffix: Seq<char>,
    at_root: bool,
) -> Result<Seq<Seq<char>>, ChoiceError> {
    match alternatives_spec(block) {
        Err(e) => Err(e),
        Ok(choices) => {
            let prefix = decode_escapes(raw_prefix);
            let suffix = decode_escapes(raw_suffix);
            match first_expansion_error(prefix, choices, suffix, at_root, 0) {
                Some(e) => Err(e),
                None => Ok(expansions(prefix, choices, suffix)),
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

pub(crate) open spec fn char_vecs_view(v: Seq<Vec<char>>) -> Seq<Seq<char>> {
    v.map_values(|x: Vec<char>| x@)
}

pub(crate) open spec fn strings_view(v: Seq<String>) -> Seq<Seq<char>> {
    v.map_values(|x: String| x@)
}

// ---------------------------------------------------------------------------
// Component summaries
// ---------------------------------------------------------------------------

/// The scanner's summary of the current component `c`: its length
/// saturated at four, whether every scalar is `.`, whether it has a
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
    &&& len == (if c.len() < 4 {
        c.len()
    } else {
        4
    })
    &&& all_dots == (forall|k: int| 0 <= k < c.len() ==> #[trigger] c[k] == '.')
    &&& control == has_forbidden_control(c)
    &&& letter_first == (c.len() >= 1 && is_ascii_letter(c[0]))
    &&& drive == has_drive_prefix(c)
}

/// Some component before index `m` is `.` or `..`.
spec fn dot_before(comps: Seq<Seq<char>>, m: int) -> bool {
    exists|k: int| 0 <= k < m && #[trigger] is_dot_component(comps[k])
}

/// Some component before index `m` is invalid.
spec fn invalid_before(comps: Seq<Seq<char>>, m: int) -> bool {
    exists|k: int| 0 <= k < m && #[trigger] invalid_decoded_component(comps[k])
}

} // verus!

/// Return whether `ch` may follow a backslash in a bare path or an unquoted
/// alternative.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_bare_escape(ch),
))]
pub(crate) fn is_bare_escape_char(ch: char) -> bool {
    matches!(ch, '#' | '\\' | '[' | ']' | ',' | '"' | ' ')
}

/// Return whether `ch` is a space or a tab.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_layout(ch),
))]
fn is_layout_char(ch: char) -> bool {
    ch == ' ' || ch == '\t'
}

/// Return the scalars of `s` decoded with `decode_escapes`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == decode_escapes(s@),
))]
pub(crate) fn decode_escapes_exec(s: &[char]) -> Vec<char> {
    let n = s.len();
    let mut out: Vec<char> = Vec::with_capacity(n);
    let mut i: usize = 0;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(s@.subrange(0, n as int) =~= s@);
        assert(out@ + s@ =~= s@);
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            i <= n,
            n == s@.len(),
            decode_escapes(s@) == out@ + decode_escapes(s@.subrange(i as int, n as int)),
        decreases n - i
    ))]
    while i < n {
        #[cfg(verus_keep_ghost)]
        proof! {
            let t = s@.subrange(i as int, n as int);
            assert(t[0] == s@[i as int]);
            assert(t.drop_first() =~= s@.subrange(i + 1, n as int));
            if i + 1 < n {
                assert(t[1] == s@[i + 1]);
                assert(t.subrange(2, t.len() as int) =~= s@.subrange(i + 2, n as int));
            }
        }
        if s[i] == '\\' && i + 1 < n {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(out@.push(s@[i + 1]) + decode_escapes(s@.subrange(i + 2, n as int))
                    =~= out@ + decode_escapes(s@.subrange(i as int, n as int)));
            }
            out.push(s[i + 1]);
            i += 2;
        } else {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(out@.push(s@[i as int]) + decode_escapes(s@.subrange(i + 1, n as int))
                    =~= out@ + decode_escapes(s@.subrange(i as int, n as int)));
            }
            out.push(s[i]);
            i += 1;
        }
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(s@.subrange(n as int, n as int) =~= Seq::<char>::empty());
        assert(out@ + Seq::<char>::empty() =~= out@);
    }
    out
}

#[cfg(verus_keep_ghost)]
verus! {

spec fn region_of(part: ScanPart, open: usize, close: usize, quoted: bool) -> ScanRegion {
    match part {
        ScanPart::Prefix => ScanRegion::Prefix,
        ScanPart::Block => ScanRegion::Block { open: open as int, quoted },
        ScanPart::Suffix => ScanRegion::Suffix { open: open as int, close: close as int },
    }
}

spec fn token_state_view(state: TokenState, pending: Seq<char>) -> TokenSpecState {
    match state {
        TokenState::Leading => TokenSpecState::Leading,
        TokenState::Unquoted => TokenSpecState::Unquoted { pending },
        TokenState::Quoted => TokenSpecState::Quoted,
        TokenState::Closed => TokenSpecState::Closed,
    }
}

spec fn some_nonempty(done: Seq<Seq<char>>) -> bool {
    exists|k: int| 0 <= k < done.len() && (#[trigger] done[k]).len() > 0
}

} // verus!

/// Scan a bare expression once: validate the escapes outside the choice
/// list and locate the one permitted list.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        scan_view(r) == scan_expression_spec(expr@),
        r matches Ok(ExpressionScan::Choice { open, close }) ==> open < close < expr@.len(),
))]
pub(crate) fn scan_expression(expr: &[char]) -> Result<ExpressionScan, ChoiceError> {
    let n = expr.len();
    let mut i: usize = 0;
    let mut part = ScanPart::Prefix;
    let mut open: usize = 0;
    let mut close: usize = 0;
    let mut quoted = false;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            i <= n,
            n == expr@.len(),
            scan_expression_spec(expr@) == scan_from(
                expr@,
                i as int,
                region_of(part, open, close, quoted),
            ),
            matches!(part, ScanPart::Block) ==> open < i,
            matches!(part, ScanPart::Suffix) ==> open < close < i,
        decreases n - i
    ))]
    while i < n {
        let c = expr[i];
        let in_block = matches!(part, ScanPart::Block);
        if c == '\\' {
            if i + 1 >= n {
                return Err(ChoiceError::IncompleteEscape);
            }
            if !in_block && !is_bare_escape_char(expr[i + 1]) {
                return Err(ChoiceError::InvalidBareEscape);
            }
            i += 2;
        } else if c == '[' && !in_block {
            if matches!(part, ScanPart::Suffix) {
                return Err(ChoiceError::MultipleBlocks);
            }
            part = ScanPart::Block;
            open = i;
            quoted = false;
            i += 1;
        } else if c == ']' {
            if !in_block {
                return Err(ChoiceError::UnmatchedClosingBracket);
            }
            if !quoted {
                part = ScanPart::Suffix;
                close = i;
            }
            i += 1;
        } else if c == '"' {
            if !in_block {
                return Err(ChoiceError::UnescapedQuote);
            }
            quoted = !quoted;
            i += 1;
        } else {
            i += 1;
        }
    }
    match part {
        ScanPart::Prefix => Ok(ExpressionScan::Plain),
        ScanPart::Block => Err(ChoiceError::UnterminatedBlock),
        ScanPart::Suffix => Ok(ExpressionScan::Choice { open, close }),
    }
}

/// Append `s[lo..hi]` to `out`.
#[cfg_attr(verus_keep_ghost, verus_spec(
    requires
        lo <= hi <= s@.len(),
    ensures
        final(out)@ == old(out)@ + s@.subrange(lo as int, hi as int),
))]
fn push_range(out: &mut Vec<char>, s: &[char], lo: usize, hi: usize) {
    let mut k = lo;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            lo <= k <= hi,
            hi <= s@.len(),
            out@ == old(out)@ + s@.subrange(lo as int, k as int),
        decreases hi - k
    ))]
    while k < hi {
        out.push(s[k]);
        k += 1;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.subrange(lo as int, k as int) =~= s@.subrange(lo as int, k - 1).push(s@[k - 1]));
        }
    }
}

/// Split a choice-list body into its decoded alternatives.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        match r {
            Ok(choices) => alternatives_spec(block@) == Ok::<Seq<Seq<char>>, ChoiceError>(
                char_vecs_view(choices@),
            ),
            Err(e) => alternatives_spec(block@) == Err::<Seq<Seq<char>>, ChoiceError>(e),
        },
))]
fn split_alternatives(block: &[char]) -> Result<Vec<Vec<char>>, ChoiceError> {
    let n = block.len();
    let mut i: usize = 0;
    let mut state = TokenState::Leading;
    let mut pending_start: usize = 0;
    let mut cur: Vec<char> = Vec::new();
    let mut done: Vec<Vec<char>> = Vec::new();
    let mut nonempty = false;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(char_vecs_view(done@) =~= Seq::<Seq<char>>::empty());
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            i <= n,
            n == block@.len(),
            pending_start <= i,
            done@.len() <= 255,
            alternatives_spec(block@) == tokens_from(
                block@,
                i as int,
                token_state_view(state, block@.subrange(pending_start as int, i as int)),
                cur@,
                char_vecs_view(done@),
            ),
            nonempty == some_nonempty(char_vecs_view(done@)),
        decreases n - i
    ))]
    while i < n {
        let c = block[i];
        match state {
            TokenState::Leading => {
                if is_layout_char(c) {
                    i += 1;
                } else if c == ',' {
                    if done.len() >= MAX_CHOICE_ALTERNATIVES - 1 {
                        return Err(ChoiceError::AlternativeCount);
                    }
                    let empty: Vec<char> = Vec::new();
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_push_view(done@, empty);
                    }
                    done.push(empty);
                    i += 1;
                } else if c == '"' {
                    state = TokenState::Quoted;
                    i += 1;
                } else if c == '\\' {
                    if i + 1 >= n {
                        return Err(ChoiceError::IncompleteEscape);
                    }
                    let next = block[i + 1];
                    if !is_bare_escape_char(next) {
                        return Err(ChoiceError::InvalidUnquotedEscape);
                    }
                    cur.push(next);
                    state = TokenState::Unquoted;
                    i += 2;
                    pending_start = i;
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(block@.subrange(i as int, i as int) =~= Seq::<char>::empty());
                    }
                } else if c == '[' || c == ']' {
                    return Err(ChoiceError::UnescapedBracket);
                } else {
                    cur.push(c);
                    state = TokenState::Unquoted;
                    i += 1;
                    pending_start = i;
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(block@.subrange(i as int, i as int) =~= Seq::<char>::empty());
                    }
                }
            }
            TokenState::Unquoted => {
                if c == ',' {
                    if done.len() >= MAX_CHOICE_ALTERNATIVES - 1 {
                        return Err(ChoiceError::AlternativeCount);
                    }
                    let finished = cur;
                    cur = Vec::new();
                    if !finished.is_empty() {
                        nonempty = true;
                    }
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_push_view(done@, finished);
                    }
                    done.push(finished);
                    state = TokenState::Leading;
                    i += 1;
                } else if is_layout_char(c) {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(block@.subrange(pending_start as int, i + 1) =~= block@.subrange(
                            pending_start as int,
                            i as int,
                        ).push(c));
                    }
                    i += 1;
                } else if c == '\\' {
                    if i + 1 >= n {
                        return Err(ChoiceError::IncompleteEscape);
                    }
                    let next = block[i + 1];
                    if !is_bare_escape_char(next) {
                        return Err(ChoiceError::InvalidUnquotedEscape);
                    }
                    push_range(&mut cur, block, pending_start, i);
                    cur.push(next);
                    i += 2;
                    pending_start = i;
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(block@.subrange(i as int, i as int) =~= Seq::<char>::empty());
                    }
                } else if c == '"' {
                    return Err(ChoiceError::MisplacedQuote);
                } else if c == '[' || c == ']' {
                    return Err(ChoiceError::UnescapedBracket);
                } else {
                    push_range(&mut cur, block, pending_start, i);
                    cur.push(c);
                    i += 1;
                    pending_start = i;
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(block@.subrange(i as int, i as int) =~= Seq::<char>::empty());
                    }
                }
            }
            TokenState::Quoted => {
                if c == '"' {
                    state = TokenState::Closed;
                    i += 1;
                } else if c == '\\' {
                    if i + 1 >= n {
                        return Err(ChoiceError::IncompleteEscape);
                    }
                    let next = block[i + 1];
                    if !(next == '"' || next == '\\' || next == '#') {
                        return Err(ChoiceError::InvalidQuotedEscape);
                    }
                    cur.push(next);
                    i += 2;
                } else {
                    cur.push(c);
                    i += 1;
                }
            }
            TokenState::Closed => {
                if is_layout_char(c) {
                    i += 1;
                } else if c == ',' {
                    if done.len() >= MAX_CHOICE_ALTERNATIVES - 1 {
                        return Err(ChoiceError::AlternativeCount);
                    }
                    let finished = cur;
                    cur = Vec::new();
                    if !finished.is_empty() {
                        nonempty = true;
                    }
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_push_view(done@, finished);
                    }
                    done.push(finished);
                    state = TokenState::Leading;
                    i += 1;
                } else {
                    return Err(ChoiceError::QuotedTrailingText);
                }
            }
        }
    }
    if matches!(state, TokenState::Quoted) {
        return Err(ChoiceError::UnterminatedQuote);
    }
    let last_nonempty = !cur.is_empty();
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_push_view(done@, cur);
    }
    done.push(cur);
    if !(nonempty || last_nonempty) {
        return Err(ChoiceError::EmptyBlock);
    }
    if done.len() < MIN_CHOICE_ALTERNATIVES || done.len() > MAX_CHOICE_ALTERNATIVES {
        return Err(ChoiceError::AlternativeCount);
    }
    Ok(done)
}

#[cfg(verus_keep_ghost)]
verus! {

proof fn lemma_push_view(done: Seq<Vec<char>>, x: Vec<char>)
    ensures
        char_vecs_view(done.push(x)) == char_vecs_view(done).push(x@),
        some_nonempty(char_vecs_view(done.push(x))) == (some_nonempty(char_vecs_view(done))
            || x@.len() > 0),
{
    let v = char_vecs_view(done);
    assert(char_vecs_view(done.push(x)) =~= v.push(x@));
    let w = v.push(x@);
    if some_nonempty(w) {
        let k = choose|k: int| 0 <= k < w.len() && (#[trigger] w[k]).len() > 0;
        if k < v.len() {
            assert(v[k] == w[k]);
        }
    }
    if some_nonempty(v) {
        let k = choose|k: int| 0 <= k < v.len() && (#[trigger] v[k]).len() > 0;
        assert(w[k] == v[k]);
    }
    if x@.len() > 0 {
        assert(w[v.len() as int] == x@);
    }
}

} // verus!

/// Return the scalars of `s` as a string.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == s@,
))]
pub(crate) fn string_from_chars(s: &[char]) -> String {
    let mut out = String::new();
    let mut k: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            k <= s@.len(),
            out@ == s@.take(k as int),
        decreases s@.len() - k
    ))]
    while k < s.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.take(k + 1) =~= s@.take(k as int).push(s@[k as int]));
        }
        out.push(s[k]);
        k += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(s@.take(k as int) =~= s@);
    }
    out
}

/// Return `a + b + c`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == a@ + b@ + c@,
))]
fn concat3(a: &[char], b: &[char], c: &[char]) -> Vec<char> {
    let mut out: Vec<char> = Vec::new();
    push_range(&mut out, a, 0, a.len());
    push_range(&mut out, b, 0, b.len());
    push_range(&mut out, c, 0, c.len());
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(a@.subrange(0, a@.len() as int) =~= a@);
        assert(b@.subrange(0, b@.len() as int) =~= b@);
        assert(c@.subrange(0, c@.len() as int) =~= c@);
        assert(Seq::<char>::empty() + a@ =~= a@);
    }
    out
}

/// Return whether the first `m` scalars of `a` and `b` agree.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        m <= a@.len(),
        m <= b@.len(),
    ensures
        r == (a@.take(m as int) == b@.take(m as int)),
))]
fn same_prefix(a: &[char], b: &[char], m: usize) -> bool {
    let mut k: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            k <= m,
            m <= a@.len(),
            m <= b@.len(),
            forall|j: int| 0 <= j < k ==> a@[j] == b@[j],
        decreases m - k
    ))]
    while k < m {
        if a[k] != b[k] {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(a@.take(m as int)[k as int] != b@.take(m as int)[k as int]);
            }
            return false;
        }
        k += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(a@.take(m as int) =~= b@.take(m as int));
    }
    true
}

/// Return whether `a` and `b` are the same scalars.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == (a@ == b@),
))]
fn same_chars(a: &[char], b: &[char]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let equal = same_prefix(a, b, a.len());
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(a@.take(a@.len() as int) =~= a@);
        assert(b@.take(b@.len() as int) =~= b@);
    }
    equal
}

/// Return the number of scalars before the last `/` of `s` (zero without
/// one).
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == parent_len(s@),
        r <= s@.len(),
))]
fn parent_length(s: &[char]) -> usize {
    let mut i = s.len();
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(s@.take(i as int) =~= s@);
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            i <= s@.len(),
            parent_len(s@.take(i as int)) == parent_len(s@),
        decreases i
    ))]
    while i > 0 && s[i - 1] != '/' {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.take(i as int).drop_last() =~= s@.take(i - 1));
        }
        i -= 1;
    }
    if i == 0 {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.take(0) =~= Seq::<char>::empty());
        }
        0
    } else {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(s@.take(i as int).last() == s@[i - 1]);
        }
        i - 1
    }
}

/// Return whether `a` and `b` have the same text before their last `/`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == (parent_of(a@) == parent_of(b@)),
))]
fn same_parent(a: &[char], b: &[char]) -> bool {
    let la = parent_length(a);
    let lb = parent_length(b);
    if la != lb {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(parent_of(a@).len() == la);
            assert(parent_of(b@).len() == lb);
        }
        return false;
    }
    same_prefix(a, b, la)
}

/// Return the component classes of the decoded path `p`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r.dot == has_dot_component(p@),
        r.invalid == has_invalid_component(p@),
        r.drive == has_drive_prefix(split_components(p@)[0]),
))]
pub(crate) fn component_report(p: &[char]) -> ComponentReport {
    let n = p.len();
    let mut i: usize = 0;
    let mut first = true;
    let mut first_drive = false;
    let mut dot = false;
    let mut invalid = false;
    let mut len: usize = 0;
    let mut all_dots = true;
    let mut control = false;
    let mut letter_first = false;
    let mut drive = false;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(p@.take(0) =~= Seq::<char>::empty());
        assert(summarizes(Seq::<char>::empty(), 0, true, false, false, false));
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            i <= n,
            n == p@.len(),
            split_components(p@.take(i as int)).len() >= 1,
            first == (split_components(p@.take(i as int)).len() == 1),
            !first ==> first_drive == has_drive_prefix(split_components(p@.take(i as int))[0]),
            dot == dot_before(
                split_components(p@.take(i as int)),
                split_components(p@.take(i as int)).len() - 1,
            ),
            invalid == invalid_before(
                split_components(p@.take(i as int)),
                split_components(p@.take(i as int)).len() - 1,
            ),
            summarizes(
                split_components(p@.take(i as int)).last(),
                len,
                all_dots,
                control,
                letter_first,
                drive,
            ),
        decreases n - i
    ))]
    while i < n {
        let ch = p[i];
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_split_take_step(p@, i as int);
            lemma_before_step(split_components(p@.take(i as int)), ch);
        }
        if ch == '/' {
            #[cfg(verus_keep_ghost)]
            proof! {
                lemma_summary_judgments(
                    split_components(p@.take(i as int)).last(),
                    len,
                    all_dots,
                    control,
                    letter_first,
                    drive,
                );
            }
            if all_dots && (len == 1 || len == 2) {
                dot = true;
            }
            if len == 0 || (all_dots && len <= 3) || control {
                invalid = true;
            }
            if first {
                first_drive = drive;
                first = false;
            }
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
                lemma_summarizes_push(
                    split_components(p@.take(i as int)).last(),
                    ch,
                    len,
                    all_dots,
                    control,
                    letter_first,
                    drive,
                );
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
            if len < 4 {
                len += 1;
            }
        }
        i += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(p@.take(i as int) =~= p@);
        let comps = split_components(p@);
        lemma_summary_judgments(comps.last(), len, all_dots, control, letter_first, drive);
        lemma_before_all(comps);
    }
    ComponentReport {
        dot: dot || (all_dots && (len == 1 || len == 2)),
        invalid: invalid || len == 0 || (all_dots && len <= 3) || control,
        drive: if first { drive } else { first_drive },
    }
}

/// Return whether `ch` is an ASCII letter.
// The ASCII letter ranges are spelled out so Verus verifies them directly.
#[allow(clippy::manual_is_ascii_check)]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_ascii_letter(ch),
))]
fn is_ascii_letter_char(ch: char) -> bool {
    matches!(ch, 'A'..='Z' | 'a'..='z')
}

/// Return whether one expansion is a valid regular-file path.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == valid_choice_expansion(p@, at_root),
))]
fn is_valid_choice_expansion(p: &[char], at_root: bool) -> bool {
    let n = p.len();
    if n == 0 {
        return false;
    }
    if n == 3 && p[0] == '.' && p[1] == '.' && p[2] == '.' {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(p@ =~= seq!['.', '.', '.']);
        }
        return false;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        if is_reserved_ellipsis(p@) {
            assert(p@[0] == '.' && p@[1] == '.' && p@[2] == '.');
        }
    }
    let first = p[0];
    let last = p[n - 1];
    if first == '/' || (at_root && first == '\\') || first == ' ' || last == '/' || last == ' ' {
        return false;
    }
    let report = component_report(p);
    !report.invalid && !(at_root && report.drive)
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Component lemmas
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
        summarizes(
            c.push(ch),
            if len < 4 {
                (len + 1) as usize
            } else {
                len
            },
            all_dots && ch == '.',
            control || is_forbidden_control(ch),
            if len == 0 {
                is_ascii_letter(ch)
            } else {
                letter_first
            },
            if len == 1 {
                letter_first && ch == ':'
            } else {
                drive
            },
        ),
{
    let d = c.push(ch);
    assert(d[d.len() - 1] == ch);
    assert forall|k: int| 0 <= k < c.len() implies d[k] == #[trigger] c[k] by {}
    if all_dots && ch == '.' {
        assert forall|k: int| 0 <= k < d.len() implies #[trigger] d[k] == '.' by {
            if k < c.len() {
                assert(d[k] == c[k]);
            }
        }
    }
    if !(all_dots && ch == '.') {
        if !all_dots {
            let k = choose|k: int| 0 <= k < c.len() && #[trigger] c[k] != '.';
            assert(d[k] != '.');
        } else {
            assert(d[d.len() - 1] != '.');
        }
    }
    if control {
        let k = choose|k: int| 0 <= k < c.len() && #[trigger] is_forbidden_control(c[k]);
        assert(is_forbidden_control(d[k]));
    }
    if is_forbidden_control(ch) {
        assert(is_forbidden_control(d[d.len() - 1]));
    }
    if has_forbidden_control(d) {
        let k = choose|k: int| 0 <= k < d.len() && #[trigger] is_forbidden_control(d[k]);
        if k < c.len() {
            assert(is_forbidden_control(c[k]));
        }
    }
    if c.len() >= 1 {
        assert(d[0] == c[0]);
    }
    if c.len() >= 2 {
        assert(d[1] == c[1]);
    }
}

proof fn lemma_summary_judgments(
    c: Seq<char>,
    len: usize,
    all_dots: bool,
    control: bool,
    letter_first: bool,
    drive: bool,
)
    requires
        summarizes(c, len, all_dots, control, letter_first, drive),
    ensures
        is_dot_component(c) == (all_dots && (len == 1 || len == 2)),
        invalid_decoded_component(c) == (len == 0 || (all_dots && len <= 3) || control),
{
    if all_dots && len == 1 {
        assert(c =~= seq!['.']);
    }
    if all_dots && len == 2 {
        assert(c =~= seq!['.', '.']);
    }
    if all_dots && len == 3 {
        assert(c =~= seq!['.', '.', '.']);
    }
    if c == seq!['.'] {
        assert(c[0] == '.');
    }
    if c == seq!['.', '.'] {
        assert(c[0] == '.' && c[1] == '.');
    }
    if c == seq!['.', '.', '.'] {
        assert(c[0] == '.' && c[1] == '.' && c[2] == '.');
    }
}

/// How one split step changes the components before the current one.
proof fn lemma_before_step(comps: Seq<Seq<char>>, c: char)
    requires
        comps.len() >= 1,
    ensures
        ({
            let next = split_step(comps, c);
            &&& next.len() >= 1
            &&& c == '/' ==> {
                &&& next.len() == comps.len() + 1
                &&& next.last() == Seq::<char>::empty()
                &&& next[0] == comps[0]
                &&& dot_before(next, next.len() - 1) == (dot_before(comps, comps.len() - 1)
                    || is_dot_component(comps.last()))
                &&& invalid_before(next, next.len() - 1) == (invalid_before(
                    comps,
                    comps.len() - 1,
                ) || invalid_decoded_component(comps.last()))
            }
            &&& c != '/' ==> {
                &&& next.len() == comps.len()
                &&& next.last() == comps.last().push(c)
                &&& comps.len() > 1 ==> next[0] == comps[0]
                &&& dot_before(next, next.len() - 1) == dot_before(comps, comps.len() - 1)
                &&& invalid_before(next, next.len() - 1) == invalid_before(
                    comps,
                    comps.len() - 1,
                )
            }
        }),
{
    let next = split_step(comps, c);
    let m = comps.len() - 1;
    if c == '/' {
        assert(next[m] == comps.last());
        assert forall|k: int| 0 <= k < comps.len() implies next[k] == #[trigger] comps[k] by {}
        if dot_before(next, next.len() - 1) {
            let k = choose|k: int| 0 <= k < next.len() - 1 && #[trigger] is_dot_component(next[k]);
            assert(next[k] == comps[k]);
        }
        if dot_before(comps, m) {
            let k = choose|k: int| 0 <= k < m && #[trigger] is_dot_component(comps[k]);
            assert(next[k] == comps[k]);
        }
        if invalid_before(next, next.len() - 1) {
            let k = choose|k: int|
                0 <= k < next.len() - 1 && #[trigger] invalid_decoded_component(next[k]);
            assert(next[k] == comps[k]);
        }
        if invalid_before(comps, m) {
            let k = choose|k: int| 0 <= k < m && #[trigger] invalid_decoded_component(comps[k]);
            assert(next[k] == comps[k]);
        }
    } else {
        assert forall|k: int| 0 <= k < m implies next[k] == #[trigger] comps[k] by {}
        if dot_before(next, m) {
            let k = choose|k: int| 0 <= k < m && #[trigger] is_dot_component(next[k]);
            assert(next[k] == comps[k]);
        }
        if dot_before(comps, m) {
            let k = choose|k: int| 0 <= k < m && #[trigger] is_dot_component(comps[k]);
            assert(next[k] == comps[k]);
        }
        if invalid_before(next, m) {
            let k = choose|k: int| 0 <= k < m && #[trigger] invalid_decoded_component(next[k]);
            assert(next[k] == comps[k]);
        }
        if invalid_before(comps, m) {
            let k = choose|k: int| 0 <= k < m && #[trigger] invalid_decoded_component(comps[k]);
            assert(next[k] == comps[k]);
        }
    }
}

/// The classes of all components are those before the last and the last.
proof fn lemma_before_all(comps: Seq<Seq<char>>)
    requires
        comps.len() >= 1,
    ensures
        dot_before(comps, comps.len() as int) == (dot_before(comps, comps.len() - 1)
            || is_dot_component(comps.last())),
        invalid_before(comps, comps.len() as int) == (invalid_before(comps, comps.len() - 1)
            || invalid_decoded_component(comps.last())),
{
    let m = comps.len() - 1;
    if dot_before(comps, comps.len() as int) && !dot_before(comps, m) {
        let k = choose|k: int| 0 <= k < comps.len() && #[trigger] is_dot_component(comps[k]);
        assert(k == m);
    }
    if is_dot_component(comps.last()) {
        assert(is_dot_component(comps[m]));
    }
    if invalid_before(comps, comps.len() as int) && !invalid_before(comps, m) {
        let k = choose|k: int|
            0 <= k < comps.len() && #[trigger] invalid_decoded_component(comps[k]);
        assert(k == m);
    }
    if invalid_decoded_component(comps.last()) {
        assert(invalid_decoded_component(comps[m]));
    }
}

} // verus!

/// Form and check every expansion, alternative by alternative.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        choices@.len() >= 1,
    ensures
        match r {
            Ok(paths) => {
                &&& first_expansion_error(
                    prefix@,
                    char_vecs_view(choices@),
                    suffix@,
                    at_root,
                    0,
                ) is None
                &&& strings_view(paths@) == expansions(
                    prefix@,
                    char_vecs_view(choices@),
                    suffix@,
                )
            },
            Err(e) => first_expansion_error(
                prefix@,
                char_vecs_view(choices@),
                suffix@,
                at_root,
                0,
            ) == Some(e),
        },
))]
fn expand_alternatives(
    prefix: &[char],
    choices: &[Vec<char>],
    suffix: &[char],
    at_root: bool,
) -> Result<Vec<String>, ChoiceError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost cs: Seq<Seq<char>> = char_vecs_view(choices@);
    }
    let n = choices.len();
    let first = concat3(prefix, choices[0].as_slice(), suffix);
    let mut paths: Vec<String> = Vec::with_capacity(n);
    let mut k: usize = 0;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(cs[0] == choices@[0]@);
        assert(strings_view(paths@) =~= expansions(prefix@, cs, suffix@).take(0));
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            k <= n,
            n == choices@.len(),
            cs == char_vecs_view(choices@),
            first@ == expansion(prefix@, cs[0], suffix@),
            strings_view(paths@) == expansions(prefix@, cs, suffix@).take(k as int),
            first_expansion_error(prefix@, cs, suffix@, at_root, 0) == first_expansion_error(
                prefix@,
                cs,
                suffix@,
                at_root,
                k as int,
            ),
        decreases n - k
    ))]
    while k < n {
        let current = choices[k].as_slice();
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(cs[k as int] == current@);
        }
        let expanded = concat3(prefix, current, suffix);
        if !is_valid_choice_expansion(expanded.as_slice(), at_root) {
            return Err(ChoiceError::InvalidExpansion);
        }
        if !same_parent(expanded.as_slice(), first.as_slice()) {
            return Err(ChoiceError::NotSiblings);
        }
        let mut j: usize = 0;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                j <= k,
                k < n,
                n == choices@.len(),
                cs == char_vecs_view(choices@),
                cs[k as int] == current@,
                expanded@ == expansion(prefix@, cs[k as int], suffix@),
                valid_choice_expansion(expanded@, at_root),
                first@ == expansion(prefix@, cs[0], suffix@),
                parent_of(expanded@) == parent_of(first@),
                first_expansion_error(prefix@, cs, suffix@, at_root, 0) == first_expansion_error(
                    prefix@,
                    cs,
                    suffix@,
                    at_root,
                    k as int,
                ),
                forall|m: int| 0 <= m < j ==> cs[m] != cs[k as int],
            decreases k - j
        ))]
        while j < k {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(cs[j as int] == choices@[j as int]@);
            }
            if same_chars(choices[j].as_slice(), current) {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(expansion(prefix@, cs[j as int], suffix@) == expanded@);
                }
                return Err(ChoiceError::DuplicateExpansion);
            }
            j += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert forall|m: int| 0 <= m < k implies #[trigger] expansion(prefix@, cs[m], suffix@)
                != expanded@ by {
                if expansion(prefix@, cs[m], suffix@) == expanded@ {
                    lemma_expansion_injective(prefix@, cs[m], cs[k as int], suffix@);
                }
            }
            assert(expansion_error_at(prefix@, cs, suffix@, at_root, k as int) is None);
        }
        let path = string_from_chars(expanded.as_slice());
        #[cfg(verus_keep_ghost)]
        proof! {
            let all = expansions(prefix@, cs, suffix@);
            assert(strings_view(paths@.push(path)) =~= strings_view(paths@).push(path@));
            assert(all[k as int] == expanded@);
            assert(all.take(k + 1) =~= all.take(k as int).push(all[k as int]));
        }
        paths.push(path);
        k += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(expansions(prefix@, cs, suffix@).take(n as int) =~= expansions(prefix@, cs, suffix@));
    }
    Ok(paths)
}

/// Expand one choice list: the raw prefix before `[`, the list body, and
/// the raw suffix after `]`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        match r {
            Ok(paths) => choice_list_spec(raw_prefix@, block@, raw_suffix@, at_root) == Ok::<
                Seq<Seq<char>>,
                ChoiceError,
            >(strings_view(paths@)),
            Err(e) => choice_list_spec(raw_prefix@, block@, raw_suffix@, at_root) == Err::<
                Seq<Seq<char>>,
                ChoiceError,
            >(e),
        },
))]
pub(crate) fn expand_choice_list(
    raw_prefix: &[char],
    block: &[char],
    raw_suffix: &[char],
    at_root: bool,
) -> Result<Vec<String>, ChoiceError> {
    let choices = split_alternatives(block)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_alternatives_shape(block@);
    }
    let prefix = decode_escapes_exec(raw_prefix);
    let suffix = decode_escapes_exec(raw_suffix);
    expand_alternatives(prefix.as_slice(), &choices, suffix.as_slice(), at_root)
}

#[cfg(verus_keep_ghost)]
verus! {

/// Equal expansions come from equal alternatives.
pub(crate) proof fn lemma_expansion_injective(
    prefix: Seq<char>,
    a: Seq<char>,
    b: Seq<char>,
    suffix: Seq<char>,
)
    requires
        expansion(prefix, a, suffix) == expansion(prefix, b, suffix),
    ensures
        a == b,
{
    let ea = expansion(prefix, a, suffix);
    let eb = expansion(prefix, b, suffix);
    assert(ea.len() == eb.len());
    assert forall|i: int| 0 <= i < a.len() implies a[i] == b[i] by {
        assert(ea[prefix.len() + i] == a[i]);
        assert(eb[prefix.len() + i] == b[i]);
    }
    assert(a =~= b);
}

/// An accepted list body has between 2 and 256 alternatives, at least one
/// of them nonempty.
pub(crate) proof fn lemma_alternatives_shape(block: Seq<char>)
    ensures
        alternatives_spec(block) matches Ok(all) ==> {
            &&& 2 <= all.len() <= 256
            &&& exists|k: int| 0 <= k < all.len() && (#[trigger] all[k]).len() > 0
        },
{
    lemma_tokens_shape(block, 0, TokenSpecState::Leading, Seq::empty(), Seq::empty());
}

proof fn lemma_tokens_shape(
    s: Seq<char>,
    i: int,
    state: TokenSpecState,
    cur: Seq<char>,
    done: Seq<Seq<char>>,
)
    ensures
        tokens_from(s, i, state, cur, done) matches Ok(all) ==> {
            &&& 2 <= all.len() <= 256
            &&& exists|k: int| 0 <= k < all.len() && (#[trigger] all[k]).len() > 0
        },
    decreases s.len() - i,
{
    if i >= s.len() {
        if finish_tokens(state, cur, done) is Ok {
            let all = done.push(cur);
            assert(!(forall|k: int| 0 <= k < all.len() ==> (#[trigger] all[k]).len() == 0));
        }
    } else {
        let c = s[i];
        match state {
            TokenSpecState::Leading => {
                if is_layout(c) {
                    lemma_tokens_shape(s, i + 1, state, cur, done);
                } else if c == ',' {
                    if done.len() < 255 {
                        lemma_tokens_shape(s, i + 1, state, cur, done.push(Seq::empty()));
                    }
                } else if c == '"' {
                    lemma_tokens_shape(s, i + 1, TokenSpecState::Quoted, cur, done);
                } else if c == '\\' {
                    if i + 1 < s.len() {
                        lemma_tokens_shape(
                            s,
                            i + 2,
                            TokenSpecState::Unquoted { pending: Seq::empty() },
                            cur.push(s[i + 1]),
                            done,
                        );
                    }
                } else if !(c == '[' || c == ']') {
                    lemma_tokens_shape(
                        s,
                        i + 1,
                        TokenSpecState::Unquoted { pending: Seq::empty() },
                        cur.push(c),
                        done,
                    );
                }
            },
            TokenSpecState::Unquoted { pending } => {
                if c == ',' {
                    if done.len() < 255 {
                        lemma_tokens_shape(
                            s,
                            i + 1,
                            TokenSpecState::Leading,
                            Seq::empty(),
                            done.push(cur),
                        );
                    }
                } else if is_layout(c) {
                    lemma_tokens_shape(
                        s,
                        i + 1,
                        TokenSpecState::Unquoted { pending: pending.push(c) },
                        cur,
                        done,
                    );
                } else if c == '\\' {
                    if i + 1 < s.len() {
                        lemma_tokens_shape(
                            s,
                            i + 2,
                            TokenSpecState::Unquoted { pending: Seq::empty() },
                            (cur + pending).push(s[i + 1]),
                            done,
                        );
                    }
                } else if !(c == '"' || c == '[' || c == ']') {
                    lemma_tokens_shape(
                        s,
                        i + 1,
                        TokenSpecState::Unquoted { pending: Seq::empty() },
                        (cur + pending).push(c),
                        done,
                    );
                }
            },
            TokenSpecState::Quoted => {
                if c == '"' {
                    lemma_tokens_shape(s, i + 1, TokenSpecState::Closed, cur, done);
                } else if c == '\\' {
                    if i + 1 < s.len() {
                        lemma_tokens_shape(s, i + 2, state, cur.push(s[i + 1]), done);
                    }
                } else {
                    lemma_tokens_shape(s, i + 1, state, cur.push(c), done);
                }
            },
            TokenSpecState::Closed => {
                if is_layout(c) {
                    lemma_tokens_shape(s, i + 1, state, cur, done);
                } else if c == ',' {
                    if done.len() < 255 {
                        lemma_tokens_shape(
                            s,
                            i + 1,
                            TokenSpecState::Leading,
                            Seq::empty(),
                            done.push(cur),
                        );
                    }
                }
            },
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    fn scan(text: &str) -> Result<ExpressionScan, ChoiceError> {
        scan_expression(&chars(text))
    }

    fn alternatives(block: &str) -> Result<Vec<String>, ChoiceError> {
        split_alternatives(&chars(block)).map(|choices| {
            choices
                .iter()
                .map(|choice| choice.iter().collect())
                .collect()
        })
    }

    fn expand(text: &str, at_root: bool) -> Result<Vec<String>, ChoiceError> {
        let s = chars(text);
        match scan_expression(&s)? {
            ExpressionScan::Choice { open, close } => {
                expand_choice_list(&s[..open], &s[open + 1..close], &s[close + 1..], at_root)
            }
            ExpressionScan::Plain => panic!("{text:?} has no choice list"),
        }
    }

    #[test]
    fn scan_locates_the_one_list_and_rejects_in_scan_order() {
        assert_eq!(scan(r"a\#b\ c"), Ok(ExpressionScan::Plain));
        assert_eq!(
            scan("a[b, c]d"),
            Ok(ExpressionScan::Choice { open: 1, close: 6 })
        );
        assert_eq!(
            scan(r#"a["]", \q]d"#),
            Ok(ExpressionScan::Choice { open: 1, close: 9 })
        );
        assert_eq!(scan(r"a\"), Err(ChoiceError::IncompleteEscape));
        assert_eq!(scan(r"a\q"), Err(ChoiceError::InvalidBareEscape));
        assert_eq!(scan("a[b]c[d]"), Err(ChoiceError::MultipleBlocks));
        assert_eq!(
            scan("a[b[c]"),
            Ok(ExpressionScan::Choice { open: 1, close: 5 })
        );
        assert_eq!(scan("a]"), Err(ChoiceError::UnmatchedClosingBracket));
        assert_eq!(scan("a[b]]"), Err(ChoiceError::UnmatchedClosingBracket));
        assert_eq!(scan("a\"b"), Err(ChoiceError::UnescapedQuote));
        assert_eq!(scan("a[b]\""), Err(ChoiceError::UnescapedQuote));
        assert_eq!(scan("a[b, c"), Err(ChoiceError::UnterminatedBlock));
        assert_eq!(scan(r#"a["]"#), Err(ChoiceError::UnterminatedBlock));
        // A scan error later in the text wins over an invalid alternative.
        assert_eq!(scan(r"a[\q, b"), Err(ChoiceError::UnterminatedBlock));
    }

    #[test]
    fn alternatives_are_decoded_with_layout_trimmed() {
        assert_eq!(
            alternatives("\"with , comma\", \\,space, \"literal []\""),
            Ok(vec![
                "with , comma".to_string(),
                ",space".to_string(),
                "literal []".to_string()
            ])
        );
        assert_eq!(
            alternatives("  a b\t ,\t\\ c\\  , \"\\\"\\\\\\#\" \t"),
            Ok(vec![
                "a b".to_string(),
                " c ".to_string(),
                "\"\\#".to_string()
            ])
        );
        assert_eq!(
            alternatives(", .local"),
            Ok(vec![String::new(), ".local".to_string()])
        );
        assert_eq!(alternatives("a,"), Ok(vec!["a".to_string(), String::new()]));
    }

    #[test]
    fn alternatives_report_each_tokenizer_error() {
        assert_eq!(alternatives(r"\"), Err(ChoiceError::IncompleteEscape));
        assert_eq!(alternatives(r"a\"), Err(ChoiceError::IncompleteEscape));
        assert_eq!(alternatives(r#""a\"#), Err(ChoiceError::IncompleteEscape));
        assert_eq!(alternatives(r"\q"), Err(ChoiceError::InvalidUnquotedEscape));
        assert_eq!(
            alternatives(r"a\q"),
            Err(ChoiceError::InvalidUnquotedEscape)
        );
        assert_eq!(alternatives("["), Err(ChoiceError::UnescapedBracket));
        assert_eq!(alternatives("a]"), Err(ChoiceError::UnescapedBracket));
        assert_eq!(alternatives("a\""), Err(ChoiceError::MisplacedQuote));
        assert_eq!(
            alternatives(r#""\q""#),
            Err(ChoiceError::InvalidQuotedEscape)
        );
        assert_eq!(alternatives("\"a\"b"), Err(ChoiceError::QuotedTrailingText));
        assert_eq!(alternatives("\"a"), Err(ChoiceError::UnterminatedQuote));
        assert_eq!(alternatives(" , "), Err(ChoiceError::EmptyBlock));
        assert_eq!(alternatives(""), Err(ChoiceError::EmptyBlock));
        assert_eq!(alternatives("a"), Err(ChoiceError::AlternativeCount));
        assert_eq!(alternatives("\"a\""), Err(ChoiceError::AlternativeCount));
    }

    #[test]
    fn the_choice_bound_rejects_before_tokenizing_unbounded_alternatives() {
        // The 256th separator promises a 257th alternative, so the count
        // error wins before the dangling escape after it is read.
        let over = format!("{}\\", "a,".repeat(MAX_CHOICE_ALTERNATIVES));
        assert_eq!(alternatives(&over), Err(ChoiceError::AlternativeCount));
        let quoted_over = format!("{}\"a\"", "\"a\",".repeat(MAX_CHOICE_ALTERNATIVES));
        assert_eq!(
            alternatives(&quoted_over),
            Err(ChoiceError::AlternativeCount)
        );
        let empty_over = ",".repeat(MAX_CHOICE_ALTERNATIVES);
        assert_eq!(
            alternatives(&empty_over),
            Err(ChoiceError::AlternativeCount)
        );
        let at_limit: Vec<String> = (0..MAX_CHOICE_ALTERNATIVES)
            .map(|index| format!("v{index}"))
            .collect();
        assert_eq!(
            alternatives(&at_limit.join(",")).map(|choices| choices.len()),
            Ok(MAX_CHOICE_ALTERNATIVES)
        );
    }

    #[test]
    fn expansions_are_valid_sibling_files_in_order() {
        assert_eq!(
            expand(r"src[/main, /lib].rs", true),
            Ok(vec!["src/main.rs".to_string(), "src/lib.rs".to_string()])
        );
        assert_eq!(
            expand(r"a\#[b, c]\ d", true),
            Ok(vec!["a#b d".to_string(), "a#c d".to_string()])
        );
        assert_eq!(expand("x[a/b, c/d]y", true), Err(ChoiceError::NotSiblings));
        assert_eq!(
            expand("x[a, a]", true),
            Err(ChoiceError::DuplicateExpansion)
        );
        assert_eq!(
            expand("x[a, b, a]", true),
            Err(ChoiceError::DuplicateExpansion)
        );
        assert_eq!(expand("[a, b]/", true), Err(ChoiceError::InvalidExpansion));
        assert_eq!(
            expand("[\\\\a, b]", true),
            Err(ChoiceError::InvalidExpansion)
        );
        assert_eq!(
            expand("[\\\\a, \\\\b]", false),
            Ok(vec!["\\a".to_string(), "\\b".to_string()])
        );
        assert_eq!(expand("[C:a, b]", true), Err(ChoiceError::InvalidExpansion));
        assert_eq!(
            expand("[C:a, C:b]", false),
            Ok(vec!["C:a".to_string(), "C:b".to_string()])
        );
        assert_eq!(expand("[..., x]", true), Err(ChoiceError::InvalidExpansion));
        assert_eq!(expand("a/[., x]", true), Err(ChoiceError::InvalidExpansion));
        assert_eq!(expand("a[/, x]", true), Err(ChoiceError::InvalidExpansion));
        assert_eq!(
            expand("[\" a\", b]", true),
            Err(ChoiceError::InvalidExpansion)
        );
        assert_eq!(
            expand("[\"a \", b]", true),
            Err(ChoiceError::InvalidExpansion)
        );
        assert_eq!(expand("[/a, b]", true), Err(ChoiceError::InvalidExpansion));
        assert_eq!(expand("[, x]", true), Err(ChoiceError::InvalidExpansion));
        assert_eq!(
            expand("a[\"\u{7f}\", b]", true),
            Err(ChoiceError::InvalidExpansion)
        );
        assert_eq!(expand("a[\\,, b]", true).map(|paths| paths.len()), Ok(2));
    }

    #[test]
    fn component_report_classifies_every_component() {
        let report = |text: &str| component_report(&chars(text));
        assert_eq!(
            report("a/b"),
            ComponentReport {
                dot: false,
                invalid: false,
                drive: false
            }
        );
        assert_eq!(
            report("a/./b"),
            ComponentReport {
                dot: true,
                invalid: true,
                drive: false
            }
        );
        assert_eq!(
            report(".."),
            ComponentReport {
                dot: true,
                invalid: true,
                drive: false
            }
        );
        assert_eq!(
            report("a/.../b"),
            ComponentReport {
                dot: false,
                invalid: true,
                drive: false
            }
        );
        assert_eq!(
            report("a/..../b"),
            ComponentReport {
                dot: false,
                invalid: false,
                drive: false
            }
        );
        assert_eq!(
            report("a//b"),
            ComponentReport {
                dot: false,
                invalid: true,
                drive: false
            }
        );
        assert_eq!(
            report(""),
            ComponentReport {
                dot: false,
                invalid: true,
                drive: false
            }
        );
        assert_eq!(
            report("a\tb"),
            ComponentReport {
                dot: false,
                invalid: true,
                drive: false
            }
        );
        assert_eq!(
            report("C:x/y"),
            ComponentReport {
                dot: false,
                invalid: false,
                drive: true
            }
        );
        assert_eq!(
            report("z:"),
            ComponentReport {
                dot: false,
                invalid: false,
                drive: true
            }
        );
        assert_eq!(
            report("x/C:y"),
            ComponentReport {
                dot: false,
                invalid: false,
                drive: false
            }
        );
        assert_eq!(
            report("1:x"),
            ComponentReport {
                dot: false,
                invalid: false,
                drive: false
            }
        );
    }

    #[test]
    fn helpers_decode_compare_and_find_parents() {
        let decoded: String = decode_escapes_exec(&chars(r"a\#b\\c\")).iter().collect();
        assert_eq!(decoded, r"a#b\c\");
        assert_eq!(string_from_chars(&chars("é/x")), "é/x");
        assert_eq!(parent_length(&chars("a/b/c")), 3);
        assert_eq!(parent_length(&chars("/c")), 0);
        assert_eq!(parent_length(&chars("abc")), 0);
        assert!(same_parent(&chars("a/b"), &chars("a/c")));
        assert!(!same_parent(&chars("a/b"), &chars("x/c")));
        assert!(!same_parent(&chars("a/b"), &chars("ab/c")));
        assert!(same_chars(&chars("ab"), &chars("ab")));
        assert!(!same_chars(&chars("ab"), &chars("a")));
        assert!(!same_chars(&chars("ab"), &chars("ac")));
        assert!(is_bare_escape_char(' '));
        assert!(!is_bare_escape_char('q'));
    }
}
