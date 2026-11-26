//! Verified exclusion-glob compilation and matching.
//!
//! This kernel compiles exclusion patterns and decides them with a
//! dynamic-programming matcher. Verus proves both halves against
//! specifications that restate the exclusion language of the v0.2 contract
//! (`docs/v0.2-contract.md`, "Exclusion patterns").
//!
//! # The compiler: pattern text to compiled pattern
//!
//! `compiled_from` specifies, for every pattern text, the compiled pattern
//! (as a ghost view: `pattern_view`) or the `PatternError` that the contract's
//! grammar `pattern = component *("/" component)` prescribes:
//!
//! - the empty pattern is rejected; a text without `/` is a basename glob,
//!   except that exactly `**` is the root-relative pattern `**`; a text with
//!   `/` is root-relative, with one compiled component per piece of
//!   `split_slash` (the pieces `str::split('/')` yields:
//!   `theorem_split_slash_is_separator_split`), and the first failing piece
//!   from the left reports its error;
//! - `component_text`: an empty piece (a leading, trailing, or repeated `/`)
//!   and `.` or `..` are rejected, exactly `**` is recursive, and any other
//!   piece is a segment;
//! - `segment_tokens_text`, `class_text`, `class_members_text`, and
//!   `class_char_text` tokenize a segment left to right: `*` is zero-or-more
//!   and a `*` directly after one is an embedded `**`; `?` is any one scalar;
//!   `[` opens a class with an optional initial `!` negation, single members
//!   and ascending `a-b` ranges, the class escapes `\\`, `\]`, `\-`, and no
//!   unescaped `/`, `-`, or `]` member; `\` escapes one of `\*?[]` to a
//!   literal; an unescaped `]` is rejected; every other scalar is a literal.
//!   Each reports the first violation from the left.
//!
//! [`compile_pattern`] is proven to return exactly `compiled_from(pattern@)`
//! (`compiles_to` for `Ok`, the same `PatternError` for `Err`), so which
//! diagnostic wins is part of its specification. Because the contract fixes
//! the accepted language but not which of several violations is reported,
//! `compiled_from` follows the left-to-right order of the implementation;
//! the text-level theorems below pin down what it compiles the contract's
//! constructs to, and the exhaustive differential tests in `exclusion.rs`
//! compare it with the pre-kernel compiler. `compile_pattern` is also proven
//! to produce only well-formed patterns (`pattern_well_formed`: every segment
//! has a token, every class has at least one ascending range, no two `*`
//! tokens are adjacent, and a root-relative pattern is `**` alone or has at
//! least two components), to compile a plain name to exactly its literal
//! scalars, and never to report the three diagnostics that no input can
//! reach (`is_unreachable_error`).
//!
//! # The matcher: compiled pattern against component sequence
//!
//! Recursive definitions over the compiled tokens, independent of the
//! table-filling algorithm, which Verus proves the executable matcher
//! computes on every input:
//!
//! - `seg_match`: a segment pattern matches one complete component; `*`
//!   extends whatever its preceding tokens matched by any number of scalars,
//!   and a literal, `?`, or class consumes exactly one scalar;
//! - `path_match`: a root-relative pattern matches the complete component
//!   sequence; `**` extends whatever its preceding components matched by any
//!   number of whole components, and a segment consumes exactly one;
//! - `excluded`: a path is excluded when it has at least one component and
//!   some pattern matches it, where a basename pattern matches when any
//!   single component matches and a root-relative pattern must match the
//!   whole sequence.
//!
//! The matcher works on components, not on the path string. `seg_match` by
//! itself lets `*`, `?`, a negated class such as `[!a]`, and a range whose
//! endpoints straddle `/` such as `[.-0]` accept the scalar `/`; that `*`
//! and `?` never cross a separator therefore rests on the hypothesis
//! `path_components_normal` (every component nonempty and free of `/`),
//! which `exclusion.rs` obtains from `Path::components`.
//!
//! # Theorems over pattern text
//!
//! Each states what `compiled_from` produces for a family of texts and, for
//! every `p` with `compiles_to(text, p)` (every `Ok(p)` that
//! `compile_pattern(text)` returns), which paths `excluded(seq![p], comps)`
//! (what `matches_components(&[p], comps)` returns) accepts:
//!
//! - `theorem_text_globstar`: `**` excludes exactly the nonempty paths;
//! - `theorem_text_star`: `*` alone matches every component, so it excludes
//!   exactly the nonempty paths;
//! - `theorem_text_question_mark`: `?` excludes exactly the paths with a
//!   component of exactly one scalar;
//! - `theorem_text_escape`: `\c` is the literal `c`;
//! - `theorem_text_range_class`: `[a-b]` and `[!a-b]` are one class with the
//!   range `(a, b)`, negated exactly when written with `!`, and exclude the
//!   paths with a one-scalar component inside (outside) the range;
//! - `theorem_text_plain_name`: a plain name such as `target` excludes
//!   exactly the paths with a component equal to it (`target`,
//!   `project/target`, not `targets`);
//! - `theorem_text_basename`: any other text without `/` (and not `**`) is a
//!   basename glob that excludes a path exactly when one single component
//!   matches its segment;
//! - `theorem_text_without_star_is_fixed_width`: if such a text has no `*`,
//!   a matching component has exactly one scalar per token, and no more
//!   scalars than the text;
//! - `theorem_text_trailing_globstar` and `theorem_text_interior_globstar`
//!   (with `_plain` corollaries for plain names): `a/**` excludes exactly the
//!   paths whose first component alone is excluded by `a`, and `a/**/b`
//!   exactly the paths of at least two components whose first and last
//!   components alone are excluded by `a` and `b` (the contract's
//!   `projects/**/target` examples);
//! - `theorem_text_without_globstar_matches_piecewise`: a root-relative text
//!   without a `**` piece has no implicit leading `**/`: it excludes only
//!   paths with exactly one component per piece, each matched by its piece;
//! - `theorem_wildcards_never_cross_separators`: under
//!   `path_components_normal`, the components are exactly the `/`-separated
//!   pieces of the path string (`theorem_components_are_path_pieces`), and a
//!   basename text excludes the path exactly when one piece matches with no
//!   token consuming a `/` (`seg_match_within`,
//!   `theorem_seg_match_within_on_separator_free`,
//!   `theorem_within_never_consumes_separator`).
//!
//! # Theorems over compiled patterns
//!
//! - `theorem_seg_concat`: segment matching is ordinary concatenation, so a
//!   pattern `a b` matches exactly the strings that split into an `a` part
//!   and a `b` part;
//! - `theorem_star_matches_everything`, `theorem_question_mark_is_one_scalar`,
//!   `theorem_literal_pattern_is_exact`, and `theorem_fixed_width`: the token
//!   `*` matches any scalars, `?` exactly one, literal tokens only their own
//!   exact scalars (no substring search, case folding, or normalization),
//!   and tokens without `*` only strings of their own length, one token per
//!   scalar;
//! - `theorem_segments_match_single_components`: without `**`, each segment
//!   pattern consumes exactly one whole component, in order, so there is no
//!   implicit leading `**/`;
//! - `theorem_trailing_globstar` and `theorem_interior_globstar`: the
//!   compiled forms of `a/**` and `a/**/b`;
//! - `theorem_plain_basename_matches_exact_components`: a basename pattern of
//!   literal tokens excludes exactly the paths with a component equal to them;
//! - `theorem_globstar_matches_every_path`: the compiled pattern `**` excludes
//!   every nonempty path;
//! - `theorem_empty_path_never_excluded`: an empty component sequence (the
//!   traversal root) is never excluded, even by `**`;
//! - `theorem_union`: multiple patterns form a union;
//! - `theorem_class_nonvacuous`: a compiled class that is not negated
//!   matches at least one scalar.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

/// One compiled exclusion pattern.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum CompiledPattern {
    /// A pattern without `/`, matched against every component.
    Basename(SegmentPattern),
    /// A pattern with `/`, matched against the complete root-relative path.
    RootRelative(Vec<PathComponentPattern>),
}

/// One `/`-separated component of a root-relative pattern.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum PathComponentPattern {
    /// `**`: zero or more complete components.
    Recursive,
    /// A segment pattern matching exactly one component.
    Segment(SegmentPattern),
}

/// The tokens of one segment pattern.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct SegmentPattern {
    pub(crate) tokens: Vec<SegmentToken>,
}

/// One segment-pattern token.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum SegmentToken {
    /// One exact scalar.
    Literal(char),
    /// `*`: zero or more scalars.
    ZeroOrMore,
    /// `?`: exactly one scalar.
    Any,
    /// `[...]`: exactly one scalar from (or, negated, outside) the class.
    Class(CharacterClass),
}

/// A bracket character class: inclusive scalar ranges, optionally negated.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct CharacterClass {
    pub(crate) negated: bool,
    pub(crate) ranges: Vec<(char, char)>,
}

/// Why an exclusion pattern does not compile; `exclusion.rs` maps each
/// variant to its usage diagnostic.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PatternError {
    /// The pattern is empty.
    Empty,
    /// A leading, trailing, or repeated `/`.
    EmptyComponent,
    /// A `.` or `..` component.
    DotComponent,
    /// `**` adjacent to anything else inside a component.
    EmbeddedGlobstar,
    /// A `\` at the end of a component.
    TrailingEscape,
    /// A `\` before a scalar it cannot escape.
    UnknownEscape,
    /// An unescaped `]` outside a class.
    UnescapedClose,
    /// A `/` inside a segment.
    SeparatorInSegment,
    /// A class without its closing `]`.
    UnterminatedClass,
    /// A class without members.
    EmptyClass,
    /// A `-` range without its second endpoint.
    MissingRangeEndpoint,
    /// A range whose first endpoint exceeds its second.
    DescendingRange,
    /// A `/` inside a class.
    SeparatorInClass,
    /// An unescaped `-` that does not form a range.
    UnescapedDash,
    /// A `\` at the end of a component inside a class.
    DanglingClassEscape,
    /// A `\` inside a class before a scalar it cannot escape.
    UnknownClassEscape,
    /// An unescaped `]` where a class member is required.
    UnescapedCloseInClass,
}

#[cfg(verus_keep_ghost)]
verus! {

/// The inclusive range `r` contains `c` by Unicode scalar value.
pub(crate) open spec fn range_contains(r: (char, char), c: char) -> bool {
    r.0 <= c && c <= r.1
}

/// Some range of the class contains `c`.
pub(crate) open spec fn class_contains(ranges: Seq<(char, char)>, c: char) -> bool {
    exists|i: int| 0 <= i < ranges.len() && range_contains(#[trigger] ranges[i], c)
}

/// A class matches `c` when some range contains it, or, negated, when none
/// does.
pub(crate) open spec fn class_accepts(class: CharacterClass, c: char) -> bool {
    class_contains(class.ranges@, c) != class.negated
}

/// The single-scalar tokens: a literal matches itself, `?` matches any
/// scalar, and a class matches its members. `*` is never a single-scalar
/// token; `seg_match` treats it separately.
pub(crate) open spec fn token_accepts(token: SegmentToken, c: char) -> bool {
    match token {
        SegmentToken::Literal(expected) => c == expected,
        SegmentToken::Any => true,
        SegmentToken::Class(class) => class_accepts(class, c),
        SegmentToken::ZeroOrMore => false,
    }
}

/// `tokens` matches the complete scalar sequence `s`.
///
/// No tokens match only the empty sequence. A pattern ending in `*` matches
/// `s` when the tokens before it match some prefix `s[..k]` (the `*` takes
/// the rest). A pattern ending in a single-scalar token matches `s` when that
/// token accepts the last scalar and the tokens before it match the rest.
pub(crate) open spec fn seg_match(tokens: Seq<SegmentToken>, s: Seq<char>) -> bool
    decreases tokens.len(),
{
    if tokens.len() == 0 {
        s.len() == 0
    } else if tokens.last() is ZeroOrMore {
        exists|k: int| 0 <= k <= s.len() && seg_match(tokens.drop_last(), #[trigger] s.take(k))
    } else {
        s.len() > 0 && token_accepts(tokens.last(), s.last()) && seg_match(
            tokens.drop_last(),
            s.drop_last(),
        )
    }
}

/// `pats` matches the complete component sequence `comps`.
///
/// No pattern components match only the empty sequence. A pattern ending in
/// `**` matches when the components before it match some prefix
/// `comps[..k]` (the `**` takes the remaining whole components). A pattern
/// ending in a segment matches when that segment matches the last component
/// and the components before it match the rest.
pub(crate) open spec fn path_match(pats: Seq<PathComponentPattern>, comps: Seq<Seq<char>>) -> bool
    decreases pats.len(),
{
    if pats.len() == 0 {
        comps.len() == 0
    } else {
        match pats.last() {
            PathComponentPattern::Recursive => exists|k: int|
                0 <= k <= comps.len() && path_match(pats.drop_last(), #[trigger] comps.take(k)),
            PathComponentPattern::Segment(segment) => comps.len() > 0 && seg_match(
                segment.tokens@,
                comps.last(),
            ) && path_match(pats.drop_last(), comps.drop_last()),
        }
    }
}

/// One compiled pattern matches a component sequence: a basename pattern
/// when any single component matches it, and a root-relative pattern when it
/// matches the complete sequence.
pub(crate) open spec fn pattern_matches(pattern: CompiledPattern, comps: Seq<Seq<char>>) -> bool {
    match pattern {
        CompiledPattern::Basename(segment) => exists|i: int|
            0 <= i < comps.len() && seg_match(segment.tokens@, #[trigger] comps[i]),
        CompiledPattern::RootRelative(pats) => path_match(pats@, comps),
    }
}

/// The exclusion decision: a nonempty root-relative path is excluded when
/// any pattern of the union matches it.
pub(crate) open spec fn excluded(patterns: Seq<CompiledPattern>, comps: Seq<Seq<char>>) -> bool {
    comps.len() > 0 && exists|p: int|
        0 <= p < patterns.len() && pattern_matches(#[trigger] patterns[p], comps)
}

/// The scalar sequences of a slice of components.
pub(crate) open spec fn component_chars(components: Seq<&str>) -> Seq<Seq<char>> {
    components.map_values(|c: &str| c@)
}

/// The segment pattern that matches exactly the scalars `lit`, as compiled
/// from a pattern component without wildcards, classes, or escapes.
pub(crate) open spec fn literal_tokens(lit: Seq<char>) -> Seq<SegmentToken> {
    lit.map_values(|c: char| SegmentToken::Literal(c))
}

/// The tokens of a segment pattern component (none for `**`).
pub(crate) open spec fn segment_tokens(p: PathComponentPattern) -> Seq<SegmentToken> {
    match p {
        PathComponentPattern::Segment(segment) => segment.tokens@,
        PathComponentPattern::Recursive => Seq::empty(),
    }
}

/// A compiled class has at least one range, and every range ascends.
pub(crate) open spec fn class_well_formed(class: CharacterClass) -> bool {
    class.ranges@.len() >= 1 && forall|i: int|
        0 <= i < class.ranges@.len() ==> (#[trigger] class.ranges@[i]).0 <= class.ranges@[i].1
}

/// A compiled token is well formed: every class is.
pub(crate) open spec fn token_well_formed(token: SegmentToken) -> bool {
    match token {
        SegmentToken::Class(class) => class_well_formed(class),
        _ => true,
    }
}

/// A compiled segment has at least one token, every class is well formed,
/// and no two `*` tokens are adjacent.
pub(crate) open spec fn tokens_well_formed(tokens: Seq<SegmentToken>) -> bool {
    &&& tokens.len() >= 1
    &&& forall|i: int| 0 <= i < tokens.len() ==> token_well_formed(#[trigger] tokens[i])
    &&& forall|i: int|
        0 < i < tokens.len() ==> !(tokens[i - 1] is ZeroOrMore && (#[trigger] tokens[i]) is ZeroOrMore)
}

/// A compiled pattern component is `**` or a well-formed segment.
pub(crate) open spec fn component_well_formed(p: PathComponentPattern) -> bool {
    match p {
        PathComponentPattern::Segment(segment) => tokens_well_formed(segment.tokens@),
        PathComponentPattern::Recursive => true,
    }
}

/// A compiled pattern is well formed: a basename pattern is one well-formed
/// segment, and a root-relative pattern is a nonempty sequence of
/// well-formed components that is either `**` alone or has at least two
/// components (it came from a pattern containing `/`).
pub(crate) open spec fn pattern_well_formed(pattern: CompiledPattern) -> bool {
    match pattern {
        CompiledPattern::Basename(segment) => tokens_well_formed(segment.tokens@),
        CompiledPattern::RootRelative(pats) => {
            &&& pats@.len() >= 1
            &&& pats@.len() == 1 ==> pats@[0] is Recursive
            &&& forall|i: int| 0 <= i < pats@.len() ==> component_well_formed(#[trigger] pats@[i])
        },
    }
}

/// The scalars that carry glob syntax: `*`, `?`, `[`, `]`, `\\`, and `/`.
pub(crate) open spec fn is_glob_syntax(c: char) -> bool {
    c == '*' || c == '?' || c == '[' || c == ']' || c == '\\' || c == '/'
}

/// A pattern that is nonempty plain text: no glob syntax, and not `.` or
/// `..`.
pub(crate) open spec fn is_plain_name(s: Seq<char>) -> bool {
    s.len() > 0 && s != seq!['.'] && s != seq!['.', '.'] && forall|i: int|
        0 <= i < s.len() ==> !is_glob_syntax(#[trigger] s[i])
}

/// The diagnostics that no input can produce: segments and classes never
/// see a `/` (it always separates components first), and a class member
/// position never holds an unescaped `]` (it closes the class or ends a
/// range first).
pub(crate) open spec fn is_unreachable_error(e: PatternError) -> bool {
    e == PatternError::SeparatorInSegment || e == PatternError::SeparatorInClass || e
        == PatternError::UnescapedCloseInClass
}

/// `s[start..end]` contains no `/`.
pub(crate) open spec fn separator_free(s: Seq<char>, start: int, end: int) -> bool {
    forall|i: int| start <= i < end ==> s[i] != '/'
}

// ---------------------------------------------------------------------------
// The pattern language over text: what `compile_pattern` must produce
// ---------------------------------------------------------------------------

/// The abstract value of one compiled token, independent of the `Vec` that
/// stores a class's ranges.
pub(crate) ghost enum TokenView {
    Literal(char),
    ZeroOrMore,
    Any,
    /// Negated, and the inclusive ranges in source order.
    Class(bool, Seq<(char, char)>),
}

/// The abstract value of one compiled pattern component.
pub(crate) ghost enum ComponentView {
    Recursive,
    Segment(Seq<TokenView>),
}

/// The abstract value of one compiled pattern.
pub(crate) ghost enum PatternView {
    Basename(Seq<TokenView>),
    RootRelative(Seq<ComponentView>),
}

pub(crate) open spec fn token_view(token: SegmentToken) -> TokenView {
    match token {
        SegmentToken::Literal(c) => TokenView::Literal(c),
        SegmentToken::ZeroOrMore => TokenView::ZeroOrMore,
        SegmentToken::Any => TokenView::Any,
        SegmentToken::Class(class) => TokenView::Class(class.negated, class.ranges@),
    }
}

pub(crate) open spec fn tokens_view(tokens: Seq<SegmentToken>) -> Seq<TokenView> {
    tokens.map_values(|t: SegmentToken| token_view(t))
}

pub(crate) open spec fn component_view(component: PathComponentPattern) -> ComponentView {
    match component {
        PathComponentPattern::Recursive => ComponentView::Recursive,
        PathComponentPattern::Segment(segment) => ComponentView::Segment(
            tokens_view(segment.tokens@),
        ),
    }
}

pub(crate) open spec fn components_view(components: Seq<PathComponentPattern>) -> Seq<
    ComponentView,
> {
    components.map_values(|c: PathComponentPattern| component_view(c))
}

pub(crate) open spec fn pattern_view(pattern: CompiledPattern) -> PatternView {
    match pattern {
        CompiledPattern::Basename(segment) => PatternView::Basename(tokens_view(segment.tokens@)),
        CompiledPattern::RootRelative(components) => PatternView::RootRelative(
            components_view(components@),
        ),
    }
}

/// The views of the literal tokens for the scalars `lit`.
pub(crate) open spec fn literal_views(lit: Seq<char>) -> Seq<TokenView> {
    lit.map_values(|c: char| TokenView::Literal(c))
}

/// `s` contains a `/`.
pub(crate) open spec fn has_separator(s: Seq<char>) -> bool {
    exists|i: int| 0 <= i < s.len() && s[i] == '/'
}

/// The `/`-separated pieces of `text`, exactly as `str::split('/')` yields
/// them: a `/` starts a new piece and any other scalar extends the current
/// one, so `""` is one empty piece and `a//b` is `a`, an empty piece, `b`.
pub(crate) open spec fn split_slash(text: Seq<char>) -> Seq<Seq<char>>
    decreases text.len(),
{
    if text.len() == 0 {
        seq![Seq::<char>::empty()]
    } else {
        let rest = split_slash(text.skip(1));
        if text[0] == '/' {
            seq![Seq::<char>::empty()] + rest
        } else {
            rest.update(0, seq![text[0]] + rest[0])
        }
    }
}

/// The pieces joined by `/`: the path string of a component sequence.
pub(crate) open spec fn join_slash(pieces: Seq<Seq<char>>) -> Seq<char>
    decreases pieces.len(),
{
    if pieces.len() == 0 {
        Seq::empty()
    } else if pieces.len() == 1 {
        pieces[0]
    } else {
        pieces[0] + seq!['/'] + join_slash(pieces.skip(1))
    }
}

/// `glob-escape`: outside a class, `\` may escape `\`, `*`, `?`, `[`, `]`.
pub(crate) open spec fn is_escapable(c: char) -> bool {
    c == '\\' || c == '*' || c == '?' || c == '[' || c == ']'
}

/// Inside a class, `\` may escape `\`, `]`, `-`.
pub(crate) open spec fn is_class_escapable(c: char) -> bool {
    c == '\\' || c == ']' || c == '-'
}

/// `class-literal`: a class member written without an escape.
pub(crate) open spec fn is_class_literal(c: char) -> bool {
    c != '/' && c != ']' && c != '\\' && c != '-'
}

/// The tokens `prefix` followed by the tokens of `rest`, or `rest`'s error.
pub(crate) open spec fn prefix_tokens(
    prefix: Seq<TokenView>,
    rest: Result<Seq<TokenView>, PatternError>,
) -> Result<Seq<TokenView>, PatternError> {
    match rest {
        Ok(tokens) => Ok(prefix + tokens),
        Err(e) => Err(e),
    }
}

/// The ranges `prefix` (spelled by `used` scalars) followed by the ranges of
/// `rest`, or `rest`'s error.
pub(crate) open spec fn prefix_ranges(
    prefix: Seq<(char, char)>,
    used: int,
    rest: Result<(Seq<(char, char)>, int), PatternError>,
) -> Result<(Seq<(char, char)>, int), PatternError> {
    match rest {
        Ok((ranges, k)) => Ok((prefix + ranges, used + k)),
        Err(e) => Err(e),
    }
}

/// The components `prefix` followed by the components of `rest`, or
/// `rest`'s error.
pub(crate) open spec fn prefix_components(
    prefix: Seq<ComponentView>,
    rest: Result<Seq<ComponentView>, PatternError>,
) -> Result<Seq<ComponentView>, PatternError> {
    match rest {
        Ok(components) => Ok(prefix + components),
        Err(e) => Err(e),
    }
}

/// `class-char` at the front of `rest`: the member scalar and how many text
/// scalars spell it. `\\`, `\]`, and `\-` stand for the escaped scalar; a
/// `/`, an unescaped `-` or `]`, and any other escape are rejected.
pub(crate) open spec fn class_char_text(rest: Seq<char>) -> Result<(char, int), PatternError> {
    if rest.len() == 0 {
        Err(PatternError::UnterminatedClass)
    } else if rest[0] == '/' {
        Err(PatternError::SeparatorInClass)
    } else if rest[0] == '-' {
        Err(PatternError::UnescapedDash)
    } else if rest[0] == '\\' {
        if rest.len() < 2 {
            Err(PatternError::DanglingClassEscape)
        } else if is_class_escapable(rest[1]) {
            Ok((rest[1], 2))
        } else {
            Err(PatternError::UnknownClassEscape)
        }
    } else if rest[0] == ']' {
        Err(PatternError::UnescapedCloseInClass)
    } else {
        Ok((rest[0], 1))
    }
}

/// `*class-member "]"` at the front of `rest`: the inclusive ranges in
/// order, and how many text scalars they and the closing `]` use.
/// `nonempty` records whether a member was already read; a member is a
/// single `class-char` (the range from it to itself) or `class-char "-"
/// class-char` with ascending endpoints. The first violation from the left
/// is reported.
pub(crate) open spec fn class_members_text(rest: Seq<char>, nonempty: bool) -> Result<
    (Seq<(char, char)>, int),
    PatternError,
>
    decreases rest.len(),
{
    if rest.len() == 0 {
        Err(PatternError::UnterminatedClass)
    } else if rest[0] == ']' {
        if nonempty {
            Ok((Seq::empty(), 1))
        } else {
            Err(PatternError::EmptyClass)
        }
    } else {
        match class_char_text(rest) {
            Err(e) => Err(e),
            Ok((first, n)) => {
                let after = rest.skip(n);
                if after.len() > 0 && after[0] == '-' {
                    let endpoint = after.skip(1);
                    if endpoint.len() == 0 || endpoint[0] == ']' {
                        Err(PatternError::MissingRangeEndpoint)
                    } else {
                        match class_char_text(endpoint) {
                            Err(e) => Err(e),
                            Ok((last, m)) => if first > last {
                                Err(PatternError::DescendingRange)
                            } else {
                                prefix_ranges(
                                    seq![(first, last)],
                                    n + 1 + m,
                                    class_members_text(endpoint.skip(m), true),
                                )
                            },
                        }
                    }
                } else {
                    prefix_ranges(seq![(first, first)], n, class_members_text(after, true))
                }
            },
        }
    }
}

/// A class with negation `negated` whose members, after `skip` scalars, are
/// `members`, or `members`'s error.
pub(crate) open spec fn class_result(
    negated: bool,
    skip: int,
    members: Result<(Seq<(char, char)>, int), PatternError>,
) -> Result<(bool, Seq<(char, char)>, int), PatternError> {
    match members {
        Err(e) => Err(e),
        Ok((ranges, k)) => Ok((negated, ranges, skip + k)),
    }
}

/// `["!"] class-member *class-member "]"` after a class's `[`: whether an
/// initial `!` negates it, its ranges, and how many text scalars it uses.
pub(crate) open spec fn class_text(rest: Seq<char>) -> Result<
    (bool, Seq<(char, char)>, int),
    PatternError,
> {
    let negated = rest.len() > 0 && rest[0] == '!';
    let skip: int = if negated {
        1
    } else {
        0
    };
    class_result(negated, skip, class_members_text(rest.skip(skip), false))
}

/// `*segment-token` for the rest of a segment, tokenized left to right:
/// `*` is zero-or-more (and a second `*` directly after one is an embedded
/// `**`), `?` is any one scalar, `[` opens a class, `\` escapes one of
/// `\*?[]` to a literal, an unescaped `]` (or `/`) is rejected, and every
/// other scalar is a literal. `after_star` records whether the previous
/// token was `*`. The first violation from the left is reported.
pub(crate) open spec fn segment_tokens_text(rest: Seq<char>, after_star: bool) -> Result<
    Seq<TokenView>,
    PatternError,
>
    decreases rest.len(),
    via segment_tokens_text_decreases
{
    if rest.len() == 0 {
        Ok(Seq::empty())
    } else if rest[0] == '*' {
        if after_star {
            Err(PatternError::EmbeddedGlobstar)
        } else {
            prefix_tokens(seq![TokenView::ZeroOrMore], segment_tokens_text(rest.skip(1), true))
        }
    } else if rest[0] == '?' {
        prefix_tokens(seq![TokenView::Any], segment_tokens_text(rest.skip(1), false))
    } else if rest[0] == '[' {
        match class_text(rest.skip(1)) {
            Err(e) => Err(e),
            Ok((negated, ranges, k)) => prefix_tokens(
                seq![TokenView::Class(negated, ranges)],
                segment_tokens_text(rest.skip(1 + k), false),
            ),
        }
    } else if rest[0] == '\\' {
        if rest.len() < 2 {
            Err(PatternError::TrailingEscape)
        } else if is_escapable(rest[1]) {
            prefix_tokens(seq![TokenView::Literal(rest[1])], segment_tokens_text(rest.skip(2), false))
        } else {
            Err(PatternError::UnknownEscape)
        }
    } else if rest[0] == ']' {
        Err(PatternError::UnescapedClose)
    } else if rest[0] == '/' {
        Err(PatternError::SeparatorInSegment)
    } else {
        prefix_tokens(seq![TokenView::Literal(rest[0])], segment_tokens_text(rest.skip(1), false))
    }
}

/// A class consumes at least its closing `]` and never more than the rest of
/// its segment, so the segment tokenizer terminates.
#[via_fn]
proof fn segment_tokens_text_decreases(rest: Seq<char>, after_star: bool) {
    if rest.len() > 0 && rest[0] == '[' {
        lemma_class_text_consumes(rest.skip(1));
    }
}

/// One `/`-separated pattern piece: an empty piece is a leading, trailing,
/// or repeated `/`; `.` and `..` are rejected; exactly `**` is recursive;
/// anything else is a segment pattern.
pub(crate) open spec fn component_text(piece: Seq<char>) -> Result<ComponentView, PatternError> {
    if piece.len() == 0 {
        Err(PatternError::EmptyComponent)
    } else if piece == seq!['.'] || piece == seq!['.', '.'] {
        Err(PatternError::DotComponent)
    } else if piece == seq!['*', '*'] {
        Ok(ComponentView::Recursive)
    } else {
        match segment_tokens_text(piece, false) {
            Ok(tokens) => Ok(ComponentView::Segment(tokens)),
            Err(e) => Err(e),
        }
    }
}

/// Every piece compiled in order; the first failing piece from the left
/// reports its error.
pub(crate) open spec fn components_text(pieces: Seq<Seq<char>>) -> Result<
    Seq<ComponentView>,
    PatternError,
>
    decreases pieces.len(),
{
    if pieces.len() == 0 {
        Ok(Seq::empty())
    } else {
        match component_text(pieces[0]) {
            Err(e) => Err(e),
            Ok(c) => prefix_components(seq![c], components_text(pieces.skip(1))),
        }
    }
}

/// What `compile_pattern` produces for the pattern text `text`, following
/// the contract's grammar `pattern = component *("/" component)`: the empty
/// pattern is rejected; a pattern without `/` is a basename glob (or, if it
/// is exactly `**`, the root-relative pattern `**`); and a pattern with `/`
/// is root-relative, one compiled component per `/`-separated piece.
pub(crate) open spec fn compiled_from(text: Seq<char>) -> Result<PatternView, PatternError> {
    if text.len() == 0 {
        Err(PatternError::Empty)
    } else if !has_separator(text) {
        match component_text(text) {
            Ok(ComponentView::Recursive) => Ok(
                PatternView::RootRelative(seq![ComponentView::Recursive]),
            ),
            Ok(ComponentView::Segment(tokens)) => Ok(PatternView::Basename(tokens)),
            Err(e) => Err(e),
        }
    } else {
        match components_text(split_slash(text)) {
            Ok(components) => Ok(PatternView::RootRelative(components)),
            Err(e) => Err(e),
        }
    }
}

/// `compile_pattern(text)` may return `Ok(pattern)`: `pattern` is the
/// compilation of `text`.
pub(crate) open spec fn compiles_to(text: Seq<char>, pattern: CompiledPattern) -> bool {
    compiled_from(text) == Ok::<PatternView, PatternError>(pattern_view(pattern))
}

/// The lexers on `s[i..e]`, by index. They are opaque so that the compiler's
/// loops reason only through the index-level step lemmas below.
#[verifier::opaque]
spec fn segment_tokens_at(s: Seq<char>, i: int, e: int, after_star: bool) -> Result<
    Seq<TokenView>,
    PatternError,
> {
    segment_tokens_text(s.subrange(i, e), after_star)
}

#[verifier::opaque]
spec fn class_at(s: Seq<char>, i: int, e: int) -> Result<
    (bool, Seq<(char, char)>, int),
    PatternError,
> {
    class_text(s.subrange(i, e))
}

#[verifier::opaque]
spec fn class_members_at(s: Seq<char>, i: int, e: int, nonempty: bool) -> Result<
    (Seq<(char, char)>, int),
    PatternError,
> {
    class_members_text(s.subrange(i, e), nonempty)
}

#[verifier::opaque]
spec fn class_char_at(s: Seq<char>, i: int, e: int) -> Result<(char, int), PatternError> {
    class_char_text(s.subrange(i, e))
}

/// The hypothesis under which component sequences stand for path strings:
/// every component is nonempty and contains no `/`. `exclusion.rs` obtains
/// components from `Path::components`, whose normal components satisfy it.
pub(crate) open spec fn path_components_normal(comps: Seq<Seq<char>>) -> bool {
    forall|i: int| 0 <= i < comps.len() ==> (#[trigger] comps[i]).len() > 0 && !has_separator(comps[i])
}

/// Segment matching in which no token may consume a `/`: `*` takes only
/// `/`-free scalars and the single-scalar tokens never accept `/`. This is
/// the contract's "`*` and `?` never cross `/`" applied to a whole string.
pub(crate) open spec fn seg_match_within(tokens: Seq<SegmentToken>, s: Seq<char>) -> bool
    decreases tokens.len(),
{
    if tokens.len() == 0 {
        s.len() == 0
    } else if tokens.last() is ZeroOrMore {
        exists|k: int|
            0 <= k <= s.len() && !has_separator(s.skip(k)) && seg_match_within(
                tokens.drop_last(),
                #[trigger] s.take(k),
            )
    } else {
        s.len() > 0 && s.last() != '/' && token_accepts(tokens.last(), s.last()) && seg_match_within(
            tokens.drop_last(),
            s.drop_last(),
        )
    }
}

} // verus!

impl CharacterClass {
    /// Return whether the class matches `value`.
    #[cfg_attr(verus_keep_ghost, verus_spec(r =>
        ensures
            r == class_accepts(*self, value),
    ))]
    pub(crate) fn matches(&self, value: char) -> bool {
        let mut index: usize = 0;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                index <= self.ranges@.len(),
                forall|k: int| 0 <= k < index ==> !range_contains(#[trigger] self.ranges@[k], value),
            decreases self.ranges@.len() - index
        ))]
        while index < self.ranges.len() {
            let (start, end) = self.ranges[index];
            if start <= value && value <= end {
                // Contained: the class matches unless it is negated.
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(range_contains(self.ranges@[index as int], value));
                }
                return !self.negated;
            }
            index += 1;
        }
        // Not contained: the class matches only when it is negated.
        self.negated
    }
}

/// Return whether a single-scalar token accepts `value`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == token_accepts(*token, value),
))]
fn accepts_one(token: &SegmentToken, value: char) -> bool {
    match token {
        SegmentToken::Literal(expected) => value == *expected,
        SegmentToken::Any => true,
        SegmentToken::Class(class) => class.matches(value),
        SegmentToken::ZeroOrMore => false,
    }
}

/// Return the scalars of `value`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == value@,
))]
fn scalars(value: &str) -> Vec<char> {
    let mut characters: Vec<char> = Vec::with_capacity(value.len());
    let mut chars = value.chars();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut seen: int = 0int;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= seen <= value@.len(),
            chars.remaining() == value@.skip(seen),
            characters@ == value@.take(seen),
        decreases value@.len() - seen
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(value@.take(seen + 1) =~= value@.take(seen).push(ch));
                }
                characters.push(ch);
                #[cfg(verus_keep_ghost)]
                proof! {
                    seen = seen + 1;
                }
            }
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(value@.take(seen) =~= value@);
                }
                return characters;
            }
        }
    }
}

/// Return the matching row for an empty pattern over `count` items: of the
/// `count + 1` prefixes, only the empty one matches.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@.len() == count + 1,
        forall|i: int| 0 <= i <= count ==> #[trigger] r@[i] == (i == 0),
))]
fn initial_row(count: usize) -> Vec<bool> {
    let mut row: Vec<bool> = Vec::with_capacity(count.saturating_add(1));
    row.push(true);
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            1 <= row@.len() <= count + 1,
            forall|i: int| 0 <= i < row@.len() ==> #[trigger] row@[i] == (i == 0),
        decreases count + 1 - row@.len()
    ))]
    while row.len() <= count {
        row.push(false);
    }
    row
}

impl SegmentPattern {
    /// Return whether the pattern matches the complete component `value`.
    ///
    /// `matched[i]` records whether the tokens processed so far match the
    /// first `i` scalars of `value`; each token rewrites the row once, so the
    /// cost is proportional to tokens times scalars.
    #[cfg_attr(verus_keep_ghost, verus_spec(r =>
        ensures
            r == seg_match(self.tokens@, value@),
    ))]
    pub(crate) fn is_match(&self, value: &str) -> bool {
        // The scalars of `value`, and the row for zero tokens: only the empty
        // prefix matches.
        let characters = scalars(value);
        let mut matched = initial_row(characters.len());
        let width = matched.len();
        #[cfg(verus_keep_ghost)]
        proof! {
            let s = characters@;
            assert(self.tokens@.take(0) =~= Seq::<SegmentToken>::empty());
            assert forall|i: int| 0 <= i < width implies #[trigger] matched@[i] == seg_match(
                self.tokens@.take(0),
                s.take(i),
            ) by {}
        }

        let mut position: usize = 0;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                position <= self.tokens@.len(),
                characters@ == value@,
                width == characters@.len() + 1,
                matched@.len() == width,
                forall|i: int|
                    0 <= i < width ==> #[trigger] matched@[i] == seg_match(
                        self.tokens@.take(position as int),
                        characters@.take(i),
                    ),
            decreases self.tokens@.len() - position
        ))]
        while position < self.tokens.len() {
            let token = &self.tokens[position];
            let mut next: Vec<bool> = Vec::with_capacity(width);
            let mut index: usize = 1;
            if matches!(token, SegmentToken::ZeroOrMore) {
                next.push(matched[0]);
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_seg_step(self.tokens@, characters@, position as int, 0);
                }
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant
                        1 <= index <= width,
                        position < self.tokens@.len(),
                        *token == self.tokens@[position as int],
                        token is ZeroOrMore,
                        characters@ == value@,
                        width == characters@.len() + 1,
                        matched@.len() == width,
                        next@.len() == index,
                        forall|i: int|
                            0 <= i < width ==> #[trigger] matched@[i] == seg_match(
                                self.tokens@.take(position as int),
                                characters@.take(i),
                            ),
                        forall|i: int|
                            0 <= i < index ==> #[trigger] next@[i] == seg_match(
                                self.tokens@.take(position + 1),
                                characters@.take(i),
                            ),
                    decreases width - index
                ))]
                while index < width {
                    let value = matched[index] || next[index - 1];
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_seg_step(self.tokens@, characters@, position as int, index as int);
                    }
                    next.push(value);
                    index += 1;
                }
            } else {
                next.push(false);
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_seg_step(self.tokens@, characters@, position as int, 0);
                }
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant
                        1 <= index <= width,
                        position < self.tokens@.len(),
                        *token == self.tokens@[position as int],
                        !(token is ZeroOrMore),
                        characters@ == value@,
                        width == characters@.len() + 1,
                        matched@.len() == width,
                        next@.len() == index,
                        forall|i: int|
                            0 <= i < width ==> #[trigger] matched@[i] == seg_match(
                                self.tokens@.take(position as int),
                                characters@.take(i),
                            ),
                        forall|i: int|
                            0 <= i < index ==> #[trigger] next@[i] == seg_match(
                                self.tokens@.take(position + 1),
                                characters@.take(i),
                            ),
                    decreases width - index
                ))]
                while index < width {
                    let value = matched[index - 1] && accepts_one(token, characters[index - 1]);
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_seg_step(self.tokens@, characters@, position as int, index as int);
                    }
                    next.push(value);
                    index += 1;
                }
            }
            matched = next;
            position += 1;
        }

        #[cfg(verus_keep_ghost)]
        proof! {
            assert(self.tokens@.take(self.tokens@.len() as int) =~= self.tokens@);
            assert(characters@.take(width - 1) =~= characters@);
        }
        matched[width - 1]
    }
}

/// Return whether the root-relative pattern matches the complete component
/// sequence.
///
/// `matched[i]` records whether the pattern components processed so far
/// match the first `i` components; each pattern component rewrites the row
/// once.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == path_match(pattern@, component_chars(components@)),
))]
pub(crate) fn path_pattern_matches(pattern: &[PathComponentPattern], components: &[&str]) -> bool {
    let mut matched = initial_row(components.len());
    let width = matched.len();
    #[cfg(verus_keep_ghost)]
    proof! {
        let comps = component_chars(components@);
        assert(pattern@.take(0) =~= Seq::<PathComponentPattern>::empty());
        assert forall|i: int| 0 <= i < width implies #[trigger] matched@[i] == path_match(
            pattern@.take(0),
            comps.take(i),
        ) by {}
    }

    let mut position: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            position <= pattern@.len(),
            width == components@.len() + 1,
            matched@.len() == width,
            forall|i: int|
                0 <= i < width ==> #[trigger] matched@[i] == path_match(
                    pattern@.take(position as int),
                    component_chars(components@).take(i),
                ),
        decreases pattern@.len() - position
    ))]
    while position < pattern.len() {
        let mut next: Vec<bool> = Vec::with_capacity(width);
        let mut index: usize = 1;
        match &pattern[position] {
            PathComponentPattern::Recursive => {
                next.push(matched[0]);
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_path_step(pattern@, component_chars(components@), position as int, 0);
                }
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant
                        1 <= index <= width,
                        position < pattern@.len(),
                        pattern@[position as int] is Recursive,
                        width == components@.len() + 1,
                        matched@.len() == width,
                        next@.len() == index,
                        forall|i: int|
                            0 <= i < width ==> #[trigger] matched@[i] == path_match(
                                pattern@.take(position as int),
                                component_chars(components@).take(i),
                            ),
                        forall|i: int|
                            0 <= i < index ==> #[trigger] next@[i] == path_match(
                                pattern@.take(position + 1),
                                component_chars(components@).take(i),
                            ),
                    decreases width - index
                ))]
                while index < width {
                    let value = matched[index] || next[index - 1];
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_path_step(
                            pattern@,
                            component_chars(components@),
                            position as int,
                            index as int,
                        );
                    }
                    next.push(value);
                    index += 1;
                }
            }
            PathComponentPattern::Segment(segment) => {
                next.push(false);
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_path_step(pattern@, component_chars(components@), position as int, 0);
                }
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant
                        1 <= index <= width,
                        position < pattern@.len(),
                        pattern@[position as int] == PathComponentPattern::Segment(*segment),
                        width == components@.len() + 1,
                        matched@.len() == width,
                        next@.len() == index,
                        forall|i: int|
                            0 <= i < width ==> #[trigger] matched@[i] == path_match(
                                pattern@.take(position as int),
                                component_chars(components@).take(i),
                            ),
                        forall|i: int|
                            0 <= i < index ==> #[trigger] next@[i] == path_match(
                                pattern@.take(position + 1),
                                component_chars(components@).take(i),
                            ),
                    decreases width - index
                ))]
                while index < width {
                    let value = matched[index - 1] && segment.is_match(components[index - 1]);
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_path_step(
                            pattern@,
                            component_chars(components@),
                            position as int,
                            index as int,
                        );
                    }
                    next.push(value);
                    index += 1;
                }
            }
        }
        matched = next;
        position += 1;
    }

    #[cfg(verus_keep_ghost)]
    proof! {
        let comps = component_chars(components@);
        assert(pattern@.take(pattern@.len() as int) =~= pattern@);
        assert(comps.take(width - 1) =~= comps);
    }
    matched[width - 1]
}

/// Return whether the union of `patterns` excludes the root-relative path
/// whose normal components are `components`.
///
/// An empty component sequence (the traversal root itself) is never
/// excluded.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == excluded(patterns@, component_chars(components@)),
))]
pub(crate) fn matches_components(patterns: &[CompiledPattern], components: &[&str]) -> bool {
    if components.is_empty() {
        return false;
    }
    let mut position: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            position <= patterns@.len(),
            components@.len() > 0,
            forall|p: int|
                0 <= p < position ==> !pattern_matches(
                    #[trigger] patterns@[p],
                    component_chars(components@),
                ),
        decreases patterns@.len() - position
    ))]
    while position < patterns.len() {
        let found = match &patterns[position] {
            CompiledPattern::Basename(segment) => basename_matches(segment, components),
            CompiledPattern::RootRelative(pattern) => path_pattern_matches(pattern, components),
        };
        if found {
            return true;
        }
        position += 1;
    }
    false
}

/// Return whether any single component matches the basename pattern.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == pattern_matches(CompiledPattern::Basename(*segment), component_chars(components@)),
))]
fn basename_matches(segment: &SegmentPattern, components: &[&str]) -> bool {
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= components@.len(),
            forall|i: int|
                0 <= i < index ==> !seg_match(
                    segment.tokens@,
                    #[trigger] component_chars(components@)[i],
                ),
        decreases components@.len() - index
    ))]
    while index < components.len() {
        if segment.is_match(components[index]) {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(component_chars(components@)[index as int] == components@[index as int]@);
            }
            return true;
        }
        index += 1;
    }
    false
}

/// Compile one exclusion pattern, reporting the first violation from left
/// to right.
///
/// The result is exactly `compiled_from(pattern@)`: the compilation the
/// contract's grammar prescribes, with the same diagnostic precedence.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r matches Ok(compiled) ==> compiles_to(pattern@, compiled),
        r matches Err(e) ==> compiled_from(pattern@) == Err::<PatternView, PatternError>(e),
        r matches Ok(compiled) ==> pattern_well_formed(compiled),
        r matches Err(e) ==> !is_unreachable_error(e),
        is_plain_name(pattern@) ==> (r matches Ok(CompiledPattern::Basename(segment))
            && segment.tokens@ == literal_tokens(pattern@)),
))]
pub(crate) fn compile_pattern(pattern: &str) -> Result<CompiledPattern, PatternError> {
    let characters = scalars(pattern);
    let len = characters.len();
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_plain_name_compiles(pattern@);
    }
    if len == 0 {
        return Err(PatternError::Empty);
    }

    let first_end = component_end(&characters, 0);
    if first_end == len {
        // No `/`: a basename glob, unless the only component is `**`.
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(characters@.subrange(0, len as int) =~= pattern@);
            assert(!has_separator(pattern@));
        }
        return match compile_component(&characters, 0, len)? {
            PathComponentPattern::Recursive => {
                let compiled = vec![PathComponentPattern::Recursive];
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(components_view(compiled@) =~= seq![ComponentView::Recursive]);
                }
                Ok(CompiledPattern::RootRelative(compiled))
            }
            PathComponentPattern::Segment(segment) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    if is_plain_name(pattern@) {
                        lemma_literal_tokens_from_views(segment.tokens@, pattern@);
                    }
                }
                Ok(CompiledPattern::Basename(segment))
            }
        };
    }

    #[cfg(verus_keep_ghost)]
    proof! {
        let t = pattern@;
        // A plain name has no `/`, so it never reaches this point.
        assert(t[first_end as int] == '/');
        assert(has_separator(t));
        assert(!is_plain_name(t));
        lemma_split_at_separator(t, first_end as int);
        assert(t.take(first_end as int) =~= characters@.subrange(0, first_end as int));
        lemma_components_text_cons(t.take(first_end as int), split_slash(t.skip(first_end + 1)));
    }
    let mut components = Vec::new();
    components.push(compile_component(&characters, 0, first_end)?);
    let mut start = first_end + 1;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(components_view(components@) =~= seq![component_view(components@[0])]);
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            len == characters@.len(),
            characters@ == pattern@,
            has_separator(pattern@),
            !is_plain_name(pattern@),
            1 <= start <= len,
            components@.len() >= 1,
            components_text(split_slash(pattern@)) == prefix_components(
                components_view(components@),
                components_text(split_slash(pattern@.skip(start as int))),
            ),
            forall|i: int|
                0 <= i < components@.len() ==> component_well_formed(#[trigger] components@[i]),
        decreases len - start
    ))]
    loop {
        let end = component_end(&characters, start);
        #[cfg(verus_keep_ghost)]
        proof! {
            let rest = pattern@.skip(start as int);
            let piece = characters@.subrange(start as int, end as int);
            if end == len {
                assert(rest =~= piece);
                assert(!has_separator(rest));
                lemma_split_without_separator(rest);
                assert(seq![piece] =~= seq![piece] + Seq::<Seq<char>>::empty());
                lemma_components_text_cons(piece, Seq::<Seq<char>>::empty());
            } else {
                lemma_split_at_separator(rest, end - start);
                assert(rest.take(end - start) =~= piece);
                assert(rest.skip(end - start + 1) =~= pattern@.skip(end + 1));
                lemma_components_text_cons(piece, split_slash(pattern@.skip(end + 1)));
            }
        }
        let component = compile_component(&characters, start, end)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            let old_view = components_view(components@);
            assert(components_view(components@.push(component)) =~= old_view + seq![
                component_view(component),
            ]);
            let tail = if end == len {
                Ok::<Seq<ComponentView>, PatternError>(Seq::empty())
            } else {
                components_text(split_slash(pattern@.skip(end + 1)))
            };
            lemma_prefix_components_assoc(old_view, seq![component_view(component)], tail);
            if let Ok(cs) = tail {
                if end == len {
                    assert(old_view + seq![component_view(component)] + cs =~= old_view + seq![
                        component_view(component),
                    ]);
                }
            }
        }
        components.push(component);
        if end == len {
            return Ok(CompiledPattern::RootRelative(components));
        }
        start = end + 1;
    }
}

/// Return the index of the first `/` at or after `start`, or the length.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        start <= characters@.len(),
    ensures
        start <= r <= characters@.len(),
        r < characters@.len() ==> characters@[r as int] == '/',
        forall|i: int| start <= i < r ==> characters@[i] != '/',
))]
fn component_end(characters: &[char], start: usize) -> usize {
    let mut end = start;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            start <= end <= characters@.len(),
            forall|i: int| start <= i < end ==> characters@[i] != '/',
        decreases characters@.len() - end
    ))]
    while end < characters.len() && characters[end] != '/' {
        end += 1;
    }
    end
}

/// Compile the pattern component `characters[start..end]`, which contains
/// no `/`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        start <= end <= characters@.len(),
        separator_free(characters@, start as int, end as int),
    ensures
        r matches Ok(component) ==> component_text(characters@.subrange(start as int, end as int))
            == Ok::<ComponentView, PatternError>(component_view(component)),
        r matches Err(e) ==> component_text(characters@.subrange(start as int, end as int))
            == Err::<ComponentView, PatternError>(e),
        r matches Ok(component) ==> component_well_formed(component),
        r matches Err(e) ==> !is_unreachable_error(e),
))]
fn compile_component(
    characters: &[char],
    start: usize,
    end: usize,
) -> Result<PathComponentPattern, PatternError> {
    let width = end - start;
    #[cfg(verus_keep_ghost)]
    proof! {
        let c = characters@.subrange(start as int, end as int);
        if width >= 1 {
            assert(c[0] == characters@[start as int]);
        }
        if width >= 2 {
            assert(c[1] == characters@[start + 1]);
        }
        if width == 1 && characters@[start as int] == '.' {
            assert(c =~= seq!['.']);
        }
        if width == 2 && characters@[start as int] == '.' && characters@[start + 1] == '.' {
            assert(c =~= seq!['.', '.']);
        }
        if width == 2 && characters@[start as int] == '*' && characters@[start + 1] == '*' {
            assert(c =~= seq!['*', '*']);
        }
    }
    if width == 0 {
        return Err(PatternError::EmptyComponent);
    }
    if (width == 1 && characters[start] == '.')
        || (width == 2 && characters[start] == '.' && characters[start + 1] == '.')
    {
        return Err(PatternError::DotComponent);
    }
    if width == 2 && characters[start] == '*' && characters[start + 1] == '*' {
        return Ok(PathComponentPattern::Recursive);
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        let c = characters@.subrange(start as int, end as int);
        assert(c != seq!['.']) by {
            if c == seq!['.'] {
                assert(c.len() == 1 && c[0] == '.');
            }
        }
        assert(c != seq!['.', '.']) by {
            if c == seq!['.', '.'] {
                assert(c.len() == 2 && c[0] == '.' && c[1] == '.');
            }
        }
        assert(c != seq!['*', '*']) by {
            if c == seq!['*', '*'] {
                assert(c.len() == 2 && c[0] == '*' && c[1] == '*');
            }
        }
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_segment_tokens_at(characters@, start as int, end as int, false);
    }
    Ok(PathComponentPattern::Segment(compile_segment(
        characters, start, end,
    )?))
}

/// Compile the nonempty segment `characters[start..end]`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        start < end <= characters@.len(),
        separator_free(characters@, start as int, end as int),
    ensures
        r matches Ok(segment) ==> segment_tokens_at(characters@, start as int, end as int, false)
            == Ok::<Seq<TokenView>, PatternError>(tokens_view(segment.tokens@)),
        r matches Err(e) ==> segment_tokens_at(characters@, start as int, end as int, false)
            == Err::<Seq<TokenView>, PatternError>(e),
        r matches Ok(segment) ==> tokens_well_formed(segment.tokens@),
        r matches Err(e) ==> !is_unreachable_error(e),
))]
fn compile_segment(
    characters: &[char],
    start: usize,
    end: usize,
) -> Result<SegmentPattern, PatternError> {
    let mut tokens: Vec<SegmentToken> = Vec::new();
    let mut previous_was_star = false;
    let mut index = start;
    #[cfg(verus_keep_ghost)]
    proof! {
        let all = segment_tokens_at(characters@, start as int, end as int, false);
        assert(tokens_view(tokens@) =~= Seq::<TokenView>::empty());
        if let Ok(rest) = all {
            assert(Seq::<TokenView>::empty() + rest =~= rest);
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            start <= index <= end,
            end <= characters@.len(),
            index > start ==> tokens@.len() >= 1,
            previous_was_star == (tokens@.len() > 0 && tokens@.last() is ZeroOrMore),
            segment_tokens_at(characters@, start as int, end as int, false) == prefix_tokens(
                tokens_view(tokens@),
                segment_tokens_at(characters@, index as int, end as int, previous_was_star),
            ),
            forall|i: int| 0 <= i < tokens@.len() ==> token_well_formed(#[trigger] tokens@[i]),
            forall|i: int|
                0 < i < tokens@.len() ==> !(tokens@[i - 1] is ZeroOrMore
                    && (#[trigger] tokens@[i]) is ZeroOrMore),
            separator_free(characters@, start as int, end as int),
        decreases end - index
    ))]
    while index < end {
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_segment_step(characters@, index as int, end as int, previous_was_star);
        }
        let character = characters[index];
        index += 1;
        let token = match character {
            '*' => {
                if previous_was_star {
                    return Err(PatternError::EmbeddedGlobstar);
                }
                previous_was_star = true;
                SegmentToken::ZeroOrMore
            }
            '?' => {
                previous_was_star = false;
                SegmentToken::Any
            }
            '[' => {
                previous_was_star = false;
                let (class, next) = parse_class(characters, index, end)?;
                index = next;
                SegmentToken::Class(class)
            }
            '\\' => {
                previous_was_star = false;
                if index >= end {
                    return Err(PatternError::TrailingEscape);
                }
                let escaped = characters[index];
                index += 1;
                if !matches!(escaped, '\\' | '*' | '?' | '[' | ']') {
                    return Err(PatternError::UnknownEscape);
                }
                SegmentToken::Literal(escaped)
            }
            ']' => {
                return Err(PatternError::UnescapedClose);
            }
            '/' => {
                return Err(PatternError::SeparatorInSegment);
            }
            literal => {
                previous_was_star = false;
                SegmentToken::Literal(literal)
            }
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            let old_tokens = tokens@;
            assert(old_tokens.push(token).last() == token);
            assert(tokens_view(old_tokens.push(token)) =~= tokens_view(old_tokens) + seq![
                token_view(token),
            ]);
            lemma_prefix_tokens_assoc(
                tokens_view(old_tokens),
                seq![token_view(token)],
                segment_tokens_at(characters@, index as int, end as int, previous_was_star),
            );
        }
        tokens.push(token);
    }

    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_segment_tokens_at_end(characters@, end as int, previous_was_star);
        assert(tokens_view(tokens@) + Seq::<TokenView>::empty() =~= tokens_view(tokens@));
    }
    Ok(SegmentPattern { tokens })
}

/// Parse the class whose opening `[` precedes `characters[start]`, returning
/// it and the index after its closing `]`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        start <= end <= characters@.len(),
        separator_free(characters@, start as int, end as int),
    ensures
        r matches Ok((class, next)) ==> class_at(characters@, start as int, end as int)
            == Ok::<(bool, Seq<(char, char)>, int), PatternError>((
            class.negated,
            class.ranges@,
            next - start,
        )),
        r matches Err(e) ==> class_at(characters@, start as int, end as int) == Err::<
            (bool, Seq<(char, char)>, int),
            PatternError,
        >(e),
        r matches Ok((class, next)) ==> start < next <= end && class_well_formed(class),
        r matches Err(e) ==> !is_unreachable_error(e),
))]
fn parse_class(
    characters: &[char],
    start: usize,
    end: usize,
) -> Result<(CharacterClass, usize), PatternError> {
    let mut index = start;
    let negated = index < end && characters[index] == '!';
    if negated {
        index += 1;
    }
    let mut ranges: Vec<(char, char)> = Vec::new();
    // The members begin at `body`.
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost body: int = index as int;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_class_text_at(characters@, start as int, end as int);
        let members = class_members_at(characters@, body, end as int, false);
        if let Ok((rs, k)) = members {
            assert(ranges@ + rs =~= rs);
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            start <= body <= index <= end,
            end <= characters@.len(),
            body == if negated {
                start + 1
            } else {
                start as int
            },
            class_at(characters@, start as int, end as int) == class_result(
                negated,
                body - start,
                prefix_ranges(
                    ranges@,
                    index - body,
                    class_members_at(characters@, index as int, end as int, ranges@.len() > 0),
                ),
            ),
            forall|i: int| 0 <= i < ranges@.len() ==> (#[trigger] ranges@[i]).0 <= ranges@[i].1,
            separator_free(characters@, start as int, end as int),
        decreases end - index
    ))]
    loop {
        #[cfg(verus_keep_ghost)]
        proof! {
            lemma_members_step(characters@, index as int, end as int, ranges@.len() > 0);
        }
        if index >= end {
            return Err(PatternError::UnterminatedClass);
        }
        if characters[index] == ']' {
            if ranges.is_empty() {
                return Err(PatternError::EmptyClass);
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(ranges@ + Seq::<(char, char)>::empty() =~= ranges@);
            }
            return Ok((CharacterClass { negated, ranges }, index + 1));
        }

        // The member starts at `member`.
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost member: int = index as int;
        }
        let (first, after_first) = parse_class_character(characters, index, end)?;
        index = after_first;
        if index < end && characters[index] == '-' {
            index += 1;
            if index >= end || characters[index] == ']' {
                return Err(PatternError::MissingRangeEndpoint);
            }
            let (last, after_last) = parse_class_character(characters, index, end)?;
            index = after_last;
            if first > last {
                return Err(PatternError::DescendingRange);
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                lemma_prefix_ranges_assoc(
                    ranges@,
                    member - body,
                    seq![(first, last)],
                    index - member,
                    class_members_at(characters@, index as int, end as int, true),
                );
                assert(ranges@ + seq![(first, last)] =~= ranges@.push((first, last)));
            }
            ranges.push((first, last));
        } else {
            #[cfg(verus_keep_ghost)]
            proof! {
                lemma_prefix_ranges_assoc(
                    ranges@,
                    member - body,
                    seq![(first, first)],
                    index - member,
                    class_members_at(characters@, index as int, end as int, true),
                );
                assert(ranges@ + seq![(first, first)] =~= ranges@.push((first, first)));
            }
            ranges.push((first, first));
        }
    }
}

/// Parse one class member scalar at `characters[start]`, returning it and
/// the index after it.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        start <= end <= characters@.len(),
    ensures
        r matches Ok((c, next)) ==> class_char_at(characters@, start as int, end as int) == Ok::<
            (char, int),
            PatternError,
        >((c, next - start)),
        r matches Err(e) ==> class_char_at(characters@, start as int, end as int) == Err::<
            (char, int),
            PatternError,
        >(e),
        r matches Ok((_, next)) ==> start < next <= end,
        r == Err::<(char, usize), PatternError>(PatternError::SeparatorInClass) ==> start < end
            && characters@[start as int] == '/',
        r == Err::<(char, usize), PatternError>(PatternError::UnescapedCloseInClass) ==> start
            < end && characters@[start as int] == ']',
        r != Err::<(char, usize), PatternError>(PatternError::SeparatorInSegment),
))]
fn parse_class_character(
    characters: &[char],
    start: usize,
    end: usize,
) -> Result<(char, usize), PatternError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_class_char_at(characters@, start as int, end as int);
    }
    if start >= end {
        return Err(PatternError::UnterminatedClass);
    }
    match characters[start] {
        '/' => Err(PatternError::SeparatorInClass),
        '-' => Err(PatternError::UnescapedDash),
        '\\' => {
            if start + 1 >= end {
                return Err(PatternError::DanglingClassEscape);
            }
            let escaped = characters[start + 1];
            if !matches!(escaped, '\\' | ']' | '-') {
                return Err(PatternError::UnknownClassEscape);
            }
            Ok((escaped, start + 2))
        }
        ']' => Err(PatternError::UnescapedCloseInClass),
        literal => Ok((literal, start + 1)),
    }
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Dynamic-programming step lemmas
// ---------------------------------------------------------------------------

/// One more token: the row for `tokens[..t + 1]` from the row for
/// `tokens[..t]`.
proof fn lemma_seg_step(tokens: Seq<SegmentToken>, s: Seq<char>, t: int, i: int)
    requires
        0 <= t < tokens.len(),
        0 <= i <= s.len(),
    ensures
        tokens[t] is ZeroOrMore ==> seg_match(tokens.take(t + 1), s.take(i)) == (seg_match(
            tokens.take(t),
            s.take(i),
        ) || (i > 0 && seg_match(tokens.take(t + 1), s.take(i - 1)))),
        !(tokens[t] is ZeroOrMore) ==> seg_match(tokens.take(t + 1), s.take(i)) == (i > 0
            && seg_match(tokens.take(t), s.take(i - 1)) && token_accepts(tokens[t], s[i - 1])),
{
    let pre = tokens.take(t);
    let cur = tokens.take(t + 1);
    assert(cur.drop_last() =~= pre);
    assert(cur.last() == tokens[t]);
    if tokens[t] is ZeroOrMore {
        // seg_match(cur, s[..j]) == exists k <= j. seg_match(pre, s[..k])
        assert forall|j: int, k: int| 0 <= k <= j <= s.len() implies s.take(j).take(k) =~= s.take(
            k,
        ) by {}
        if seg_match(cur, s.take(i)) {
            let k = choose|k: int|
                0 <= k <= s.take(i).len() && seg_match(pre, #[trigger] s.take(i).take(k));
            assert(s.take(i).take(k) =~= s.take(k));
            if k < i {
                assert(s.take(i - 1).take(k) =~= s.take(k));
                assert(seg_match(cur, s.take(i - 1)));
            } else {
                assert(s.take(i).take(i) =~= s.take(i));
            }
        }
        if seg_match(pre, s.take(i)) {
            assert(s.take(i).take(i) =~= s.take(i));
            assert(seg_match(pre, s.take(i).take(i)));
        }
        if i > 0 && seg_match(cur, s.take(i - 1)) {
            let k = choose|k: int|
                0 <= k <= s.take(i - 1).len() && seg_match(pre, #[trigger] s.take(i - 1).take(k));
            assert(s.take(i - 1).take(k) =~= s.take(k));
            assert(s.take(i).take(k) =~= s.take(k));
            assert(seg_match(pre, s.take(i).take(k)));
        }
    } else {
        if i > 0 {
            assert(s.take(i).drop_last() =~= s.take(i - 1));
            assert(s.take(i).last() == s[i - 1]);
        }
    }
}

/// One more pattern component: the row for `pats[..t + 1]` from the row for
/// `pats[..t]`.
proof fn lemma_path_step(pats: Seq<PathComponentPattern>, comps: Seq<Seq<char>>, t: int, i: int)
    requires
        0 <= t < pats.len(),
        0 <= i <= comps.len(),
    ensures
        pats[t] is Recursive ==> path_match(pats.take(t + 1), comps.take(i)) == (path_match(
            pats.take(t),
            comps.take(i),
        ) || (i > 0 && path_match(pats.take(t + 1), comps.take(i - 1)))),
        forall|segment: SegmentPattern|
            pats[t] == PathComponentPattern::Segment(segment) ==> path_match(
                pats.take(t + 1),
                comps.take(i),
            ) == (i > 0 && path_match(pats.take(t), comps.take(i - 1)) && seg_match(
                segment.tokens@,
                comps[i - 1],
            )),
{
    let pre = pats.take(t);
    let cur = pats.take(t + 1);
    assert(cur.drop_last() =~= pre);
    assert(cur.last() == pats[t]);
    if pats[t] is Recursive {
        if path_match(cur, comps.take(i)) {
            let k = choose|k: int|
                0 <= k <= comps.take(i).len() && path_match(pre, #[trigger] comps.take(i).take(k));
            assert(comps.take(i).take(k) =~= comps.take(k));
            if k < i {
                assert(comps.take(i - 1).take(k) =~= comps.take(k));
                assert(path_match(cur, comps.take(i - 1)));
            }
        }
        if path_match(pre, comps.take(i)) {
            assert(comps.take(i).take(i) =~= comps.take(i));
            assert(path_match(pre, comps.take(i).take(i)));
        }
        if i > 0 && path_match(cur, comps.take(i - 1)) {
            let k = choose|k: int|
                0 <= k <= comps.take(i - 1).len() && path_match(
                    pre,
                    #[trigger] comps.take(i - 1).take(k),
                );
            assert(comps.take(i - 1).take(k) =~= comps.take(k));
            assert(comps.take(i).take(k) =~= comps.take(k));
            assert(path_match(pre, comps.take(i).take(k)));
        }
    } else {
        if i > 0 {
            assert(comps.take(i).drop_last() =~= comps.take(i - 1));
            assert(comps.take(i).last() == comps[i - 1]);
        }
    }
}

// ---------------------------------------------------------------------------
// Compiler lemmas
// ---------------------------------------------------------------------------

proof fn lemma_prefix_ranges_assoc(
    a: Seq<(char, char)>,
    u: int,
    b: Seq<(char, char)>,
    v: int,
    rest: Result<(Seq<(char, char)>, int), PatternError>,
)
    ensures
        prefix_ranges(a, u, prefix_ranges(b, v, rest)) == prefix_ranges(a + b, u + v, rest),
{
    if let Ok((ranges, k)) = rest {
        assert(a + (b + ranges) =~= (a + b) + ranges);
    }
}

proof fn lemma_prefix_tokens_assoc(
    a: Seq<TokenView>,
    b: Seq<TokenView>,
    rest: Result<Seq<TokenView>, PatternError>,
)
    ensures
        prefix_tokens(a, prefix_tokens(b, rest)) == prefix_tokens(a + b, rest),
{
    if let Ok(tokens) = rest {
        assert(a + (b + tokens) =~= (a + b) + tokens);
    }
}

proof fn lemma_prefix_components_assoc(
    a: Seq<ComponentView>,
    b: Seq<ComponentView>,
    rest: Result<Seq<ComponentView>, PatternError>,
)
    ensures
        prefix_components(a, prefix_components(b, rest)) == prefix_components(a + b, rest),
{
    if let Ok(components) = rest {
        assert(a + (b + components) =~= (a + b) + components);
    }
}

/// `class_members_text` at the start of `s[i..e]`, restated by index: end
/// of text, the closing `]`, or a member (with an optional `-` and second
/// endpoint) followed by more members.
proof fn lemma_members_step(s: Seq<char>, i: int, e: int, nonempty: bool)
    requires
        0 <= i <= e <= s.len(),
    ensures
        class_members_at(s, i, e, nonempty) == if i >= e {
            Err(PatternError::UnterminatedClass)
        } else if s[i] == ']' {
            if nonempty {
                Ok((Seq::empty(), 1))
            } else {
                Err(PatternError::EmptyClass)
            }
        } else {
            match class_char_at(s, i, e) {
                Err(x) => Err(x),
                Ok((first, n)) => if i + n < e && s[i + n] == '-' {
                    if i + n + 1 >= e || s[i + n + 1] == ']' {
                        Err(PatternError::MissingRangeEndpoint)
                    } else {
                        match class_char_at(s, i + n + 1, e) {
                            Err(x) => Err(x),
                            Ok((last, m)) => if first > last {
                                Err(PatternError::DescendingRange)
                            } else {
                                prefix_ranges(
                                    seq![(first, last)],
                                    n + 1 + m,
                                    class_members_at(s, i + n + 1 + m, e, true),
                                )
                            },
                        }
                    }
                } else {
                    prefix_ranges(seq![(first, first)], n, class_members_at(s, i + n, e, true))
                },
            }
        },
{
    reveal(class_members_at);
    reveal(class_char_at);
    let rest = s.subrange(i, e);
    if i < e {
        assert(rest[0] == s[i]);
        if let Ok((first, n)) = class_char_text(rest) {
            assert(rest.skip(n) =~= s.subrange(i + n, e));
            if i + n < e {
                assert(rest.skip(n)[0] == s[i + n]);
                assert(rest.skip(n).skip(1) =~= s.subrange(i + n + 1, e));
                if i + n + 1 < e {
                    assert(rest.skip(n).skip(1)[0] == s[i + n + 1]);
                }
                if let Ok((last, m)) = class_char_text(s.subrange(i + n + 1, e)) {
                    assert(s.subrange(i + n + 1, e).skip(m) =~= s.subrange(i + n + 1 + m, e));
                }
            }
        }
    }
}

/// A text without `/` is its own single piece.
proof fn lemma_split_without_separator(t: Seq<char>)
    requires
        !has_separator(t),
    ensures
        split_slash(t) == seq![t],
    decreases t.len(),
{
    if t.len() == 0 {
        assert(t =~= Seq::<char>::empty());
    } else {
        let tail = t.skip(1);
        assert(!has_separator(tail)) by {
            if has_separator(tail) {
                let i = choose|i: int| 0 <= i < tail.len() && tail[i] == '/';
                assert(t[i + 1] == '/');
            }
        }
        lemma_split_without_separator(tail);
        assert(t[0] != '/');
        assert(seq![t[0]] + tail =~= t);
        assert(seq![tail].update(0, seq![t[0]] + tail) =~= seq![t]);
    }
}

/// The first `/` of `t`, at `b`, ends the first piece.
proof fn lemma_split_at_separator(t: Seq<char>, b: int)
    requires
        0 <= b < t.len(),
        t[b] == '/',
        separator_free(t, 0, b),
    ensures
        split_slash(t) == seq![t.take(b)] + split_slash(t.skip(b + 1)),
    decreases b,
{
    if b == 0 {
        assert(t.take(0) =~= Seq::<char>::empty());
    } else {
        let tail = t.skip(1);
        assert(tail.skip(b) =~= t.skip(b + 1));
        lemma_split_at_separator(tail, b - 1);
        assert(t[0] != '/');
        assert(seq![t[0]] + tail.take(b - 1) =~= t.take(b));
        let rest = split_slash(tail);
        assert(rest == seq![tail.take(b - 1)] + split_slash(t.skip(b + 1)));
        assert(rest.update(0, seq![t[0]] + rest[0]) =~= seq![t.take(b)] + split_slash(
            t.skip(b + 1),
        ));
    }
}

/// Compiling a first piece and then the rest.
proof fn lemma_components_text_cons(first: Seq<char>, rest: Seq<Seq<char>>)
    ensures
        components_text(seq![first] + rest) == match component_text(first) {
            Err(e) => Err(e),
            Ok(c) => prefix_components(seq![c], components_text(rest)),
        },
{
    let pieces = seq![first] + rest;
    assert(pieces[0] == first);
    assert(pieces.skip(1) =~= rest);
}

/// A segment without glob syntax is its literal scalars.
proof fn lemma_plain_segment(rest: Seq<char>, after_star: bool)
    requires
        forall|i: int| 0 <= i < rest.len() ==> !is_glob_syntax(#[trigger] rest[i]),
    ensures
        segment_tokens_text(rest, after_star) == Ok::<Seq<TokenView>, PatternError>(
            literal_views(rest),
        ),
    decreases rest.len(),
{
    if rest.len() == 0 {
        assert(literal_views(rest) =~= Seq::<TokenView>::empty());
    } else {
        let tail = rest.skip(1);
        assert(!is_glob_syntax(rest[0]));
        assert forall|i: int| 0 <= i < tail.len() implies !is_glob_syntax(#[trigger] tail[i]) by {
            assert(tail[i] == rest[i + 1]);
        }
        lemma_plain_segment(tail, false);
        assert(seq![TokenView::Literal(rest[0])] + literal_views(tail) =~= literal_views(rest));
    }
}

/// A plain name compiles to the basename pattern of its literal scalars.
proof fn lemma_plain_name_compiles(t: Seq<char>)
    ensures
        is_plain_name(t) ==> compiled_from(t) == Ok::<PatternView, PatternError>(
            PatternView::Basename(literal_views(t)),
        ),
{
    if is_plain_name(t) {
        assert(!has_separator(t)) by {
            if has_separator(t) {
                let i = choose|i: int| 0 <= i < t.len() && t[i] == '/';
                assert(!is_glob_syntax(t[i]));
            }
        }
        assert(!is_glob_syntax(t[0]));
        assert(t != seq!['*', '*']) by {
            if t == seq!['*', '*'] {
                assert(t[0] == '*');
            }
        }
        lemma_plain_segment(t, false);
    }
}

/// Literal token views determine the tokens.
proof fn lemma_literal_tokens_from_views(tokens: Seq<SegmentToken>, lit: Seq<char>)
    requires
        tokens_view(tokens) == literal_views(lit),
    ensures
        tokens == literal_tokens(lit),
{
    assert(tokens.len() == tokens_view(tokens).len());
    assert forall|i: int| 0 <= i < tokens.len() implies #[trigger] tokens[i] == literal_tokens(
        lit,
    )[i] by {
        assert(token_view(tokens[i]) == tokens_view(tokens)[i]);
        assert(tokens_view(tokens)[i] == TokenView::Literal(lit[i]));
    }
    assert(tokens =~= literal_tokens(lit));
}

/// `class_text` on `s[start..e]`, restated by index.
proof fn lemma_class_text_at(s: Seq<char>, start: int, e: int)
    requires
        0 <= start <= e <= s.len(),
    ensures
        ({
            let negated = start < e && s[start] == '!';
            let skip: int = if negated {
                1
            } else {
                0
            };
            class_at(s, start, e) == class_result(
                negated,
                skip,
                class_members_at(s, start + skip, e, false),
            )
        }),
{
    reveal(class_at);
    reveal(class_members_at);
    let rest = s.subrange(start, e);
    assert(rest.skip(0) =~= rest);
    if start < e {
        assert(rest[0] == s[start]);
        assert(rest.skip(1) =~= s.subrange(start + 1, e));
    }
}

/// `segment_tokens_text` at the start of `s[i..e]`, restated by index.
proof fn lemma_segment_step(s: Seq<char>, i: int, e: int, after_star: bool)
    requires
        0 <= i < e <= s.len(),
    ensures
        segment_tokens_at(s, i, e, after_star) == if s[i] == '*' {
            if after_star {
                Err(PatternError::EmbeddedGlobstar)
            } else {
                prefix_tokens(seq![TokenView::ZeroOrMore], segment_tokens_at(s, i + 1, e, true))
            }
        } else if s[i] == '?' {
            prefix_tokens(seq![TokenView::Any], segment_tokens_at(s, i + 1, e, false))
        } else if s[i] == '[' {
            match class_at(s, i + 1, e) {
                Err(x) => Err(x),
                Ok((negated, ranges, k)) => prefix_tokens(
                    seq![TokenView::Class(negated, ranges)],
                    segment_tokens_at(s, i + 1 + k, e, false),
                ),
            }
        } else if s[i] == '\\' {
            if i + 1 >= e {
                Err(PatternError::TrailingEscape)
            } else if is_escapable(s[i + 1]) {
                prefix_tokens(
                    seq![TokenView::Literal(s[i + 1])],
                    segment_tokens_at(s, i + 2, e, false),
                )
            } else {
                Err(PatternError::UnknownEscape)
            }
        } else if s[i] == ']' {
            Err(PatternError::UnescapedClose)
        } else if s[i] == '/' {
            Err(PatternError::SeparatorInSegment)
        } else {
            prefix_tokens(seq![TokenView::Literal(s[i])], segment_tokens_at(s, i + 1, e, false))
        },
{
    reveal(segment_tokens_at);
    reveal(class_at);
    let rest = s.subrange(i, e);
    assert(rest[0] == s[i]);
    assert(rest.skip(1) =~= s.subrange(i + 1, e));
    if i + 2 <= e {
        assert(rest[1] == s[i + 1]);
        assert(rest.skip(2) =~= s.subrange(i + 2, e));
    }
    if s[i] == '[' {
        lemma_class_text_consumes(s.subrange(i + 1, e));
        if let Ok((negated, ranges, k)) = class_text(s.subrange(i + 1, e)) {
            assert(rest.skip(1 + k) =~= s.subrange(i + 1 + k, e));
        }
    }
}

proof fn lemma_segment_tokens_at_end(s: Seq<char>, e: int, after_star: bool)
    requires
        0 <= e <= s.len(),
    ensures
        segment_tokens_at(s, e, e, after_star) == Ok::<Seq<TokenView>, PatternError>(
            Seq::empty(),
        ),
{
    reveal(segment_tokens_at);
}

proof fn lemma_segment_tokens_at(s: Seq<char>, i: int, e: int, after_star: bool)
    ensures
        segment_tokens_at(s, i, e, after_star) == segment_tokens_text(s.subrange(i, e), after_star),
{
    reveal(segment_tokens_at);
}

proof fn lemma_class_char_at(s: Seq<char>, i: int, e: int)
    ensures
        class_char_at(s, i, e) == class_char_text(s.subrange(i, e)),
{
    reveal(class_char_at);
}

/// A class's members use at least the closing `]` and at most the rest of
/// the text.
proof fn lemma_class_members_consumes(rest: Seq<char>, nonempty: bool)
    ensures
        class_members_text(rest, nonempty) matches Ok((_, k)) ==> 1 <= k <= rest.len(),
    decreases rest.len(),
{
    if rest.len() > 0 && rest[0] != ']' {
        if let Ok((first, n)) = class_char_text(rest) {
            let after = rest.skip(n);
            if after.len() > 0 && after[0] == '-' {
                let endpoint = after.skip(1);
                if !(endpoint.len() == 0 || endpoint[0] == ']') {
                    if let Ok((last, m)) = class_char_text(endpoint) {
                        lemma_class_members_consumes(endpoint.skip(m), true);
                    }
                }
            } else {
                lemma_class_members_consumes(after, true);
            }
        }
    }
}

/// A class uses at least its closing `]` and at most the rest of the text.
proof fn lemma_class_text_consumes(rest: Seq<char>)
    ensures
        class_text(rest) matches Ok((_, _, k)) ==> 1 <= k <= rest.len(),
{
    if rest.len() > 0 && rest[0] == '!' {
        lemma_class_members_consumes(rest.skip(1), false);
    } else {
        assert(rest.skip(0) =~= rest);
        lemma_class_members_consumes(rest, false);
    }
}

// ---------------------------------------------------------------------------
// Contract theorems
// ---------------------------------------------------------------------------

/// Segment matching is concatenation: `a b` matches exactly the scalar
/// sequences that split into a prefix matched by `a` and a suffix matched by
/// `b`. In particular the definition does not depend on matching from the
/// right: a leading `*` takes any prefix, and a leading single-scalar token
/// takes exactly the first scalar.
pub(crate) proof fn theorem_seg_concat(a: Seq<SegmentToken>, b: Seq<SegmentToken>, s: Seq<char>)
    ensures
        seg_match(a + b, s) == exists|k: int|
            0 <= k <= s.len() && seg_match(a, #[trigger] s.take(k)) && seg_match(b, s.skip(k)),
    decreases b.len(),
{
    if b.len() == 0 {
        assert(a + b =~= a);
        if seg_match(a, s) {
            assert(s.take(s.len() as int) =~= s);
            assert(s.skip(s.len() as int).len() == 0);
        }
        if exists|k: int|
            0 <= k <= s.len() && seg_match(a, #[trigger] s.take(k)) && seg_match(b, s.skip(k)) {
            let k = choose|k: int|
                0 <= k <= s.len() && seg_match(a, #[trigger] s.take(k)) && seg_match(b, s.skip(k));
            assert(s.skip(k).len() == 0);
            assert(s.take(k) =~= s);
        }
    } else {
        let b1 = b.drop_last();
        let t = b.last();
        assert((a + b).drop_last() =~= a + b1);
        assert((a + b).last() == t);
        if t is ZeroOrMore {
            if seg_match(a + b, s) {
                let j = choose|j: int| 0 <= j <= s.len() && seg_match(a + b1, #[trigger] s.take(j));
                theorem_seg_concat(a, b1, s.take(j));
                let k = choose|k: int|
                    0 <= k <= s.take(j).len() && seg_match(a, #[trigger] s.take(j).take(k))
                        && seg_match(b1, s.take(j).skip(k));
                assert(s.take(j).take(k) =~= s.take(k));
                assert(s.take(j).skip(k) =~= s.skip(k).take(j - k));
                assert(seg_match(b1, s.skip(k).take(j - k)));
                assert(seg_match(b, s.skip(k)));
                assert(seg_match(a, s.take(k)) && seg_match(b, s.skip(k)));
            }
            if exists|k: int|
                0 <= k <= s.len() && seg_match(a, #[trigger] s.take(k)) && seg_match(b, s.skip(k)) {
                let k = choose|k: int|
                    0 <= k <= s.len() && seg_match(a, #[trigger] s.take(k)) && seg_match(
                        b,
                        s.skip(k),
                    );
                let m = choose|m: int|
                    0 <= m <= s.skip(k).len() && seg_match(b1, #[trigger] s.skip(k).take(m));
                let j = k + m;
                theorem_seg_concat(a, b1, s.take(j));
                assert(s.take(j).take(k) =~= s.take(k));
                assert(s.take(j).skip(k) =~= s.skip(k).take(m));
                assert(seg_match(a, s.take(j).take(k)) && seg_match(b1, s.take(j).skip(k)));
                assert(seg_match(a + b1, s.take(j)));
            }
        } else {
            if seg_match(a + b, s) {
                let p = s.drop_last();
                theorem_seg_concat(a, b1, p);
                let k = choose|k: int|
                    0 <= k <= p.len() && seg_match(a, #[trigger] p.take(k)) && seg_match(
                        b1,
                        p.skip(k),
                    );
                assert(p.take(k) =~= s.take(k));
                assert(s.skip(k).drop_last() =~= p.skip(k));
                assert(s.skip(k).last() == s.last());
                assert(seg_match(b, s.skip(k)));
                assert(seg_match(a, s.take(k)) && seg_match(b, s.skip(k)));
            }
            if exists|k: int|
                0 <= k <= s.len() && seg_match(a, #[trigger] s.take(k)) && seg_match(b, s.skip(k)) {
                let k = choose|k: int|
                    0 <= k <= s.len() && seg_match(a, #[trigger] s.take(k)) && seg_match(
                        b,
                        s.skip(k),
                    );
                let p = s.drop_last();
                assert(s.skip(k).len() > 0);
                assert(p.take(k) =~= s.take(k));
                assert(s.skip(k).drop_last() =~= p.skip(k));
                assert(s.skip(k).last() == s.last());
                theorem_seg_concat(a, b1, p);
                assert(seg_match(a, p.take(k)) && seg_match(b1, p.skip(k)));
                assert(seg_match(a + b1, p));
            }
        }
    }
}

/// `*` alone matches every scalar sequence, including the empty one.
pub(crate) proof fn theorem_star_matches_everything(s: Seq<char>)
    ensures
        seg_match(seq![SegmentToken::ZeroOrMore], s),
{
    let tokens = seq![SegmentToken::ZeroOrMore];
    assert(tokens.drop_last() =~= Seq::<SegmentToken>::empty());
    assert(s.take(0).len() == 0);
    assert(seg_match(tokens.drop_last(), s.take(0)));
}

/// `?` alone matches exactly the sequences of one scalar.
pub(crate) proof fn theorem_question_mark_is_one_scalar(s: Seq<char>)
    ensures
        seg_match(seq![SegmentToken::Any], s) == (s.len() == 1),
{
    let tokens = seq![SegmentToken::Any];
    assert(tokens.drop_last() =~= Seq::<SegmentToken>::empty());
    assert(tokens.last() == SegmentToken::Any);
    if seg_match(tokens, s) {
        assert(seg_match(tokens.drop_last(), s.drop_last()));
    }
    if s.len() == 1 {
        assert(s.drop_last().len() == 0);
        assert(seg_match(tokens.drop_last(), s.drop_last()));
    }
}

/// A pattern without `*` matches exactly the sequences of its own length
/// whose scalars each satisfy the token in the same position: every literal,
/// `?`, and class consumes exactly one scalar, in order.
pub(crate) proof fn theorem_fixed_width(tokens: Seq<SegmentToken>, s: Seq<char>)
    requires
        forall|i: int| 0 <= i < tokens.len() ==> !(#[trigger] tokens[i] is ZeroOrMore),
    ensures
        seg_match(tokens, s) == (s.len() == tokens.len() && forall|i: int|
            0 <= i < s.len() ==> token_accepts(tokens[i], #[trigger] s[i])),
    decreases tokens.len(),
{
    if tokens.len() > 0 {
        let pre = tokens.drop_last();
        assert(!(tokens[tokens.len() - 1] is ZeroOrMore));
        assert forall|i: int| 0 <= i < pre.len() implies !(#[trigger] pre[i] is ZeroOrMore) by {
            assert(pre[i] == tokens[i]);
        }
        if s.len() > 0 {
            theorem_fixed_width(pre, s.drop_last());
            if seg_match(tokens, s) {
                assert forall|i: int| 0 <= i < s.len() implies token_accepts(
                    tokens[i],
                    #[trigger] s[i],
                ) by {
                    if i < s.len() - 1 {
                        assert(s.drop_last()[i] == s[i]);
                        assert(pre[i] == tokens[i]);
                    }
                }
            }
            if s.len() == tokens.len() && forall|i: int|
                0 <= i < s.len() ==> token_accepts(tokens[i], #[trigger] s[i]) {
                assert forall|i: int| 0 <= i < s.drop_last().len() implies token_accepts(
                    pre[i],
                    #[trigger] s.drop_last()[i],
                ) by {
                    assert(s.drop_last()[i] == s[i]);
                    assert(pre[i] == tokens[i]);
                }
                assert(token_accepts(tokens[s.len() - 1], s[s.len() - 1]));
            }
        }
    }
}

/// A pattern of literal scalars matches exactly those scalars: matching is
/// exact, case-sensitive, normalization-preserving, and never a substring
/// search.
pub(crate) proof fn theorem_literal_pattern_is_exact(lit: Seq<char>, s: Seq<char>)
    ensures
        seg_match(literal_tokens(lit), s) == (s == lit),
{
    let tokens = literal_tokens(lit);
    assert forall|i: int| 0 <= i < tokens.len() implies !(#[trigger] tokens[i] is ZeroOrMore) by {}
    theorem_fixed_width(tokens, s);
    if s == lit {
        assert forall|i: int| 0 <= i < s.len() implies token_accepts(tokens[i], #[trigger] s[i]) by {}
    }
    if seg_match(tokens, s) {
        assert forall|i: int| 0 <= i < s.len() implies #[trigger] s[i] == lit[i] by {
            assert(token_accepts(tokens[i], s[i]));
        }
        assert(s =~= lit);
    }
}

/// Without `**`, each segment pattern consumes exactly one whole component,
/// in order, so a root-relative pattern has no implicit leading `**/`. That
/// `*`, `?`, and classes then never match across a `/` also needs the
/// components to be free of `/` (`theorem_wildcards_never_cross_separators`).
pub(crate) proof fn theorem_segments_match_single_components(
    pats: Seq<PathComponentPattern>,
    comps: Seq<Seq<char>>,
)
    requires
        forall|i: int| 0 <= i < pats.len() ==> (#[trigger] pats[i]) is Segment,
    ensures
        path_match(pats, comps) == (comps.len() == pats.len() && forall|i: int|
            0 <= i < comps.len() ==> seg_match(segment_tokens(pats[i]), #[trigger] comps[i])),
    decreases pats.len(),
{
    if pats.len() > 0 {
        let pre = pats.drop_last();
        assert(pats[pats.len() - 1] is Segment);
        assert forall|i: int| 0 <= i < pre.len() implies (#[trigger] pre[i]) is Segment by {
            assert(pre[i] == pats[i]);
        }
        if comps.len() > 0 {
            theorem_segments_match_single_components(pre, comps.drop_last());
            if path_match(pats, comps) {
                assert forall|i: int| 0 <= i < comps.len() implies seg_match(
                    segment_tokens(pats[i]),
                    #[trigger] comps[i],
                ) by {
                    if i < comps.len() - 1 {
                        assert(comps.drop_last()[i] == comps[i]);
                        assert(pre[i] == pats[i]);
                    }
                }
            }
            if comps.len() == pats.len() && forall|i: int|
                0 <= i < comps.len() ==> seg_match(segment_tokens(pats[i]), #[trigger] comps[i]) {
                assert forall|i: int| 0 <= i < comps.drop_last().len() implies seg_match(
                    segment_tokens(pre[i]),
                    #[trigger] comps.drop_last()[i],
                ) by {
                    assert(comps.drop_last()[i] == comps[i]);
                    assert(pre[i] == pats[i]);
                }
                assert(seg_match(segment_tokens(pats[comps.len() - 1]), comps[comps.len() - 1]));
            }
        }
    }
}

/// A root-relative pattern of one segment matches exactly the one-component
/// paths that the segment matches.
proof fn lemma_single_segment(segment: SegmentPattern, comps: Seq<Seq<char>>)
    ensures
        path_match(seq![PathComponentPattern::Segment(segment)], comps) == (comps.len() == 1
            && seg_match(segment.tokens@, comps[0])),
{
    let pats = seq![PathComponentPattern::Segment(segment)];
    assert(pats.drop_last() =~= Seq::<PathComponentPattern>::empty());
    assert(pats.last() == PathComponentPattern::Segment(segment));
    if comps.len() > 0 {
        assert(path_match(pats.drop_last(), comps.drop_last()) == (comps.drop_last().len() == 0));
    }
    if comps.len() == 1 {
        assert(comps.last() == comps[0]);
    }
}

/// A trailing `**` matches the component its prefix matches and everything
/// beneath it: `project/**` matches `project` and `project/a/b`, and nothing
/// outside `project`.
pub(crate) proof fn theorem_trailing_globstar(first: SegmentPattern, comps: Seq<Seq<char>>)
    ensures
        path_match(
            seq![PathComponentPattern::Segment(first), PathComponentPattern::Recursive],
            comps,
        ) == (comps.len() >= 1 && seg_match(first.tokens@, comps[0])),
{
    let pats = seq![PathComponentPattern::Segment(first), PathComponentPattern::Recursive];
    let head = seq![PathComponentPattern::Segment(first)];
    assert(pats.drop_last() =~= head);
    assert(pats.last() == PathComponentPattern::Recursive);
    if path_match(pats, comps) {
        let k = choose|k: int| 0 <= k <= comps.len() && path_match(head, #[trigger] comps.take(k));
        lemma_single_segment(first, comps.take(k));
        assert(comps.take(k)[0] == comps[0]);
    }
    if comps.len() >= 1 && seg_match(first.tokens@, comps[0]) {
        lemma_single_segment(first, comps.take(1));
        assert(comps.take(1)[0] == comps[0]);
        assert(path_match(head, comps.take(1)));
    }
}

/// An interior `**` matches zero or more whole components between its
/// neighbours: `projects/**/target` matches `projects/target` and
/// `projects/a/b/target`, but not `projects/a/b/target/file` or
/// `other/projects/target`.
pub(crate) proof fn theorem_interior_globstar(
    first: SegmentPattern,
    last: SegmentPattern,
    comps: Seq<Seq<char>>,
)
    ensures
        path_match(
            seq![
                PathComponentPattern::Segment(first),
                PathComponentPattern::Recursive,
                PathComponentPattern::Segment(last),
            ],
            comps,
        ) == (comps.len() >= 2 && seg_match(first.tokens@, comps[0]) && seg_match(
            last.tokens@,
            comps[comps.len() - 1],
        )),
{
    let pats = seq![
        PathComponentPattern::Segment(first),
        PathComponentPattern::Recursive,
        PathComponentPattern::Segment(last),
    ];
    let prefix = seq![PathComponentPattern::Segment(first), PathComponentPattern::Recursive];
    assert(pats.drop_last() =~= prefix);
    assert(pats.last() == PathComponentPattern::Segment(last));
    if comps.len() > 0 {
        theorem_trailing_globstar(first, comps.drop_last());
        assert(comps.last() == comps[comps.len() - 1]);
        if comps.len() >= 2 {
            assert(comps.drop_last()[0] == comps[0]);
        }
    }
}

/// The root-relative pattern `**` matches every component sequence, so the
/// compiled pattern `**` excludes every nonempty path
/// (`theorem_text_globstar` shows that the text `**` compiles to it).
pub(crate) proof fn theorem_globstar_matches_every_path(
    pattern: CompiledPattern,
    comps: Seq<Seq<char>>,
)
    requires
        pattern matches CompiledPattern::RootRelative(pats) && pats@ == seq![
            PathComponentPattern::Recursive,
        ],
    ensures
        pattern_matches(pattern, comps),
        excluded(seq![pattern], comps) == (comps.len() > 0),
{
    let pats = seq![PathComponentPattern::Recursive];
    assert(pats.drop_last() =~= Seq::<PathComponentPattern>::empty());
    assert(comps.take(0).len() == 0);
    assert(path_match(pats.drop_last(), comps.take(0)));
    assert(path_match(pats, comps));
    if comps.len() > 0 {
        assert(seq![pattern][0] == pattern);
    }
}

/// A plain-name pattern such as `target` (which `compile_pattern` compiles
/// to a basename pattern of its literal scalars) matches a path exactly when
/// some component, at any depth, is that name: `target` matches `target` and
/// `project/target` but not `targets`.
pub(crate) proof fn theorem_plain_basename_matches_exact_components(
    segment: SegmentPattern,
    lit: Seq<char>,
    comps: Seq<Seq<char>>,
)
    requires
        segment.tokens@ == literal_tokens(lit),
    ensures
        pattern_matches(CompiledPattern::Basename(segment), comps) == exists|i: int|
            0 <= i < comps.len() && #[trigger] comps[i] == lit,
{
    if pattern_matches(CompiledPattern::Basename(segment), comps) {
        let i = choose|i: int| 0 <= i < comps.len() && seg_match(segment.tokens@, #[trigger] comps[i]);
        theorem_literal_pattern_is_exact(lit, comps[i]);
    }
    if exists|i: int| 0 <= i < comps.len() && #[trigger] comps[i] == lit {
        let i = choose|i: int| 0 <= i < comps.len() && #[trigger] comps[i] == lit;
        theorem_literal_pattern_is_exact(lit, comps[i]);
    }
}

/// A compiled class is never vacuous: a well-formed class that is not
/// negated matches the first endpoint of each of its ranges.
pub(crate) proof fn theorem_class_nonvacuous(class: CharacterClass, i: int)
    requires
        class_well_formed(class),
        !class.negated,
        0 <= i < class.ranges@.len(),
    ensures
        class_accepts(class, class.ranges@[i].0),
{
    assert(range_contains(class.ranges@[i], class.ranges@[i].0));
}

/// An empty component sequence (the traversal root itself) is never
/// excluded, whatever the patterns.
pub(crate) proof fn theorem_empty_path_never_excluded(patterns: Seq<CompiledPattern>)
    ensures
        !excluded(patterns, Seq::empty()),
{
}

/// Multiple patterns form a union: a path is excluded exactly when it is
/// nonempty and one of the patterns alone excludes it.
pub(crate) proof fn theorem_union(patterns: Seq<CompiledPattern>, comps: Seq<Seq<char>>)
    ensures
        excluded(patterns, comps) == exists|p: int|
            0 <= p < patterns.len() && excluded(seq![#[trigger] patterns[p]], comps),
{
    if excluded(patterns, comps) {
        let p = choose|p: int| 0 <= p < patterns.len() && pattern_matches(#[trigger] patterns[p], comps);
        assert(seq![patterns[p]][0] == patterns[p]);
        assert(excluded(seq![patterns[p]], comps));
    }
    if exists|p: int| 0 <= p < patterns.len() && excluded(seq![#[trigger] patterns[p]], comps) {
        let p = choose|p: int| 0 <= p < patterns.len() && excluded(seq![#[trigger] patterns[p]], comps);
        let q = choose|q: int| 0 <= q < 1 && pattern_matches(#[trigger] seq![patterns[p]][q], comps);
        assert(seq![patterns[p]][q] == patterns[p]);
    }
}


// ---------------------------------------------------------------------------
// From compiled patterns back to pattern text
// ---------------------------------------------------------------------------

/// What a token view accepts as one scalar; `token_accepts` restated on
/// views.
pub(crate) open spec fn view_accepts(token: TokenView, c: char) -> bool {
    match token {
        TokenView::Literal(expected) => c == expected,
        TokenView::Any => true,
        TokenView::Class(negated, ranges) => class_contains(ranges, c) != negated,
        TokenView::ZeroOrMore => false,
    }
}

/// `s` has one scalar per token view and each token accepts its scalar.
pub(crate) open spec fn fits_tokens(tokens: Seq<TokenView>, s: Seq<char>) -> bool {
    s.len() == tokens.len() && forall|j: int| 0 <= j < s.len() ==> view_accepts(tokens[j], #[trigger] s[j])
}

proof fn lemma_components_view(components: Seq<PathComponentPattern>)
    ensures
        components_view(components).len() == components.len(),
        forall|i: int|
            0 <= i < components.len() ==> #[trigger] components_view(components)[i]
                == component_view(components[i]),
{
}

proof fn lemma_tokens_view(tokens: Seq<SegmentToken>)
    ensures
        tokens_view(tokens).len() == tokens.len(),
        forall|i: int|
            0 <= i < tokens.len() ==> #[trigger] tokens_view(tokens)[i] == token_view(tokens[i]),
{
}

proof fn lemma_token_view(a: SegmentToken, b: SegmentToken)
    requires
        token_view(a) == token_view(b),
    ensures
        (a is ZeroOrMore) == (b is ZeroOrMore),
        forall|c: char| token_accepts(a, c) == token_accepts(b, c),
        forall|c: char| token_accepts(a, c) == view_accepts(token_view(a), c),
{
}

/// Matching depends only on the token views: the compiled `Vec` holding a
/// class's ranges is irrelevant beyond its contents.
proof fn lemma_seg_match_view(t1: Seq<SegmentToken>, t2: Seq<SegmentToken>, s: Seq<char>)
    requires
        tokens_view(t1) == tokens_view(t2),
    ensures
        seg_match(t1, s) == seg_match(t2, s),
    decreases t1.len(),
{
    assert(t1.len() == tokens_view(t1).len());
    assert(t2.len() == tokens_view(t2).len());
    if t1.len() > 0 {
        let n = t1.len() - 1;
        assert(token_view(t1[n]) == tokens_view(t1)[n]);
        assert(token_view(t2[n]) == tokens_view(t2)[n]);
        lemma_token_view(t1.last(), t2.last());
        assert forall|i: int| 0 <= i < n implies #[trigger] tokens_view(t1.drop_last())[i]
            == tokens_view(t2.drop_last())[i] by {
            assert(tokens_view(t1)[i] == tokens_view(t2)[i]);
            assert(t1.drop_last()[i] == t1[i]);
            assert(t2.drop_last()[i] == t2[i]);
            assert(tokens_view(t1)[i] == token_view(t1[i]));
            assert(tokens_view(t2)[i] == token_view(t2[i]));
            assert(tokens_view(t1.drop_last())[i] == token_view(t1.drop_last()[i]));
            assert(tokens_view(t2.drop_last())[i] == token_view(t2.drop_last()[i]));
        }
        assert(tokens_view(t1.drop_last()) =~= tokens_view(t2.drop_last()));
        if t1.last() is ZeroOrMore {
            assert forall|k: int| 0 <= k <= s.len() implies seg_match(t1.drop_last(), #[trigger] s.take(k))
                == seg_match(t2.drop_last(), s.take(k)) by {
                lemma_seg_match_view(t1.drop_last(), t2.drop_last(), s.take(k));
            }
        } else {
            lemma_seg_match_view(t1.drop_last(), t2.drop_last(), s.drop_last());
        }
    }
}

/// A single pattern excludes a path exactly when the path is nonempty and
/// the pattern matches it.
proof fn lemma_excluded_single(p: CompiledPattern, comps: Seq<Seq<char>>)
    ensures
        excluded(seq![p], comps) == (comps.len() > 0 && pattern_matches(p, comps)),
{
    assert(seq![p][0] == p);
}

/// A basename pattern whose token views have no `*` excludes a path exactly
/// when some component fits the tokens one scalar each.
proof fn lemma_fixed_basename_excludes(p: CompiledPattern, views: Seq<TokenView>, comps: Seq<Seq<char>>)
    requires
        pattern_view(p) == PatternView::Basename(views),
        forall|j: int| 0 <= j < views.len() ==> !(#[trigger] views[j] is ZeroOrMore),
    ensures
        excluded(seq![p], comps) == exists|i: int| 0 <= i < comps.len() && fits_tokens(views, #[trigger] comps[i]),
{
    lemma_excluded_single(p, comps);
    if let CompiledPattern::Basename(segment) = p {
        let tokens = segment.tokens@;
        assert(tokens.len() == views.len());
        assert forall|j: int| 0 <= j < tokens.len() implies !(#[trigger] tokens[j] is ZeroOrMore) by {
            assert(token_view(tokens[j]) == views[j]);
        }
        assert forall|i: int| 0 <= i < comps.len() implies seg_match(tokens, #[trigger] comps[i])
            == fits_tokens(views, comps[i]) by {
            theorem_fixed_width(tokens, comps[i]);
            assert forall|j: int| 0 <= j < tokens.len() implies forall|c: char|
                token_accepts(tokens[j], c) == view_accepts(#[trigger] views[j], c) by {
                assert(token_view(tokens[j]) == views[j]);
                lemma_token_view(tokens[j], tokens[j]);
            }
            if seg_match(tokens, comps[i]) {
                assert forall|j: int| 0 <= j < comps[i].len() implies view_accepts(
                    views[j],
                    #[trigger] comps[i][j],
                ) by {
                    assert(token_accepts(tokens[j], comps[i][j]));
                }
            }
            if fits_tokens(views, comps[i]) {
                assert forall|j: int| 0 <= j < comps[i].len() implies token_accepts(
                    tokens[j],
                    #[trigger] comps[i][j],
                ) by {
                    assert(view_accepts(views[j], comps[i][j]));
                }
            }
        }
        if pattern_matches(p, comps) {
            let i = choose|i: int| 0 <= i < comps.len() && seg_match(tokens, #[trigger] comps[i]);
            assert(fits_tokens(views, comps[i]));
        }
        if exists|i: int| 0 <= i < comps.len() && fits_tokens(views, #[trigger] comps[i]) {
            let i = choose|i: int| 0 <= i < comps.len() && fits_tokens(views, #[trigger] comps[i]);
            assert(seg_match(tokens, comps[i]));
        }
    }
}

/// A text without `/` that compiles to a segment is a basename glob.
proof fn lemma_basename_text(text: Seq<char>)
    requires
        text.len() > 0,
        !has_separator(text),
    ensures
        compiled_from(text) == match component_text(text) {
            Ok(ComponentView::Recursive) => Ok(
                PatternView::RootRelative(seq![ComponentView::Recursive]),
            ),
            Ok(ComponentView::Segment(tokens)) => Ok(PatternView::Basename(tokens)),
            Err(e) => Err(e),
        },
{
}

/// A single-scalar text other than `/` and `.` is its own segment piece.
proof fn lemma_short_piece(text: Seq<char>)
    requires
        1 <= text.len() <= 2,
        text[0] != '/',
        text[0] != '.',
        text.len() == 2 ==> text[0] != '*' && text[1] != '/',
    ensures
        !has_separator(text),
        component_text(text) == match segment_tokens_text(text, false) {
            Ok(tokens) => Ok(ComponentView::Segment(tokens)),
            Err(e) => Err::<ComponentView, PatternError>(e),
        },
{
    assert(text != seq!['.']) by {
        if text == seq!['.'] {
            assert(text[0] == '.');
        }
    }
    assert(text != seq!['.', '.']) by {
        if text == seq!['.', '.'] {
            assert(text[0] == '.');
        }
    }
    assert(text != seq!['*', '*']) by {
        if text == seq!['*', '*'] {
            assert(text[0] == '*');
        }
    }
}

// ---------------------------------------------------------------------------
// Contract theorems over pattern text
// ---------------------------------------------------------------------------
//
// Each theorem below computes `compiled_from` for a family of pattern texts
// and states, for every `p` with `compiles_to(text, p)` (that is, every
// `Ok(p)` that `compile_pattern(text)` can return), which paths
// `excluded(seq![p], comps)` (that is, `matches_components(&[p], comps)`)
// accepts.

/// The text `**` compiles to the root-relative pattern `**`, which excludes
/// exactly the nonempty paths.
pub(crate) proof fn theorem_text_globstar(p: CompiledPattern, comps: Seq<Seq<char>>)
    ensures
        compiled_from(seq!['*', '*']) == Ok::<PatternView, PatternError>(
            PatternView::RootRelative(seq![ComponentView::Recursive]),
        ),
        compiles_to(seq!['*', '*'], p) ==> (excluded(seq![p], comps) <==> comps.len() > 0),
{
    let text = seq!['*', '*'];
    assert(!has_separator(text));
    lemma_basename_text(text);
    if compiles_to(text, p) {
        assert(pattern_view(p) == PatternView::RootRelative(seq![ComponentView::Recursive]));
        if let CompiledPattern::RootRelative(pats) = p {
            assert(components_view(pats@) == seq![ComponentView::Recursive]);
            assert(components_view(pats@).len() == pats@.len());
            assert(pats@.len() == 1);
            assert(components_view(pats@)[0] == component_view(pats@[0]));
            assert(component_view(pats@[0]) == ComponentView::Recursive);
            assert(pats@ =~= seq![PathComponentPattern::Recursive]);
            theorem_globstar_matches_every_path(p, comps);
        }
    }
}

/// The text `*` compiles to the basename pattern `*`, which matches every
/// component (so it excludes exactly the nonempty paths).
pub(crate) proof fn theorem_text_star(p: CompiledPattern, comps: Seq<Seq<char>>)
    ensures
        compiled_from(seq!['*']) == Ok::<PatternView, PatternError>(
            PatternView::Basename(seq![TokenView::ZeroOrMore]),
        ),
        compiles_to(seq!['*'], p) ==> (excluded(seq![p], comps) <==> comps.len() > 0),
{
    let text = seq!['*'];
    lemma_short_piece(text);
    assert(text.skip(1) =~= Seq::<char>::empty());
    assert(segment_tokens_text(text.skip(1), true) == Ok::<Seq<TokenView>, PatternError>(
        Seq::empty(),
    ));
    assert(seq![TokenView::ZeroOrMore] + Seq::<TokenView>::empty() =~= seq![TokenView::ZeroOrMore]);
    assert(segment_tokens_text(text, false) == Ok::<Seq<TokenView>, PatternError>(
        seq![TokenView::ZeroOrMore],
    ));
    lemma_basename_text(text);
    if compiles_to(text, p) {
        lemma_excluded_single(p, comps);
        if let CompiledPattern::Basename(segment) = p {
            let star = seq![SegmentToken::ZeroOrMore];
            assert(tokens_view(star) =~= seq![TokenView::ZeroOrMore]);
            if comps.len() > 0 {
                lemma_seg_match_view(segment.tokens@, star, comps[0]);
                theorem_star_matches_everything(comps[0]);
            }
        }
    }
}

/// The text `?` compiles to the basename pattern `?`, which excludes
/// exactly the paths with a component of exactly one scalar.
pub(crate) proof fn theorem_text_question_mark(p: CompiledPattern, comps: Seq<Seq<char>>)
    ensures
        compiled_from(seq!['?']) == Ok::<PatternView, PatternError>(
            PatternView::Basename(seq![TokenView::Any]),
        ),
        compiles_to(seq!['?'], p) ==> (excluded(seq![p], comps) <==> exists|i: int|
            0 <= i < comps.len() && (#[trigger] comps[i]).len() == 1),
{
    let text = seq!['?'];
    lemma_short_piece(text);
    assert(text.skip(1) =~= Seq::<char>::empty());
    assert(segment_tokens_text(text.skip(1), false) == Ok::<Seq<TokenView>, PatternError>(
        Seq::empty(),
    ));
    assert(seq![TokenView::Any] + Seq::<TokenView>::empty() =~= seq![TokenView::Any]);
    assert(segment_tokens_text(text, false) == Ok::<Seq<TokenView>, PatternError>(
        seq![TokenView::Any],
    ));
    lemma_basename_text(text);
    if compiles_to(text, p) {
        let views = seq![TokenView::Any];
        lemma_fixed_basename_excludes(p, views, comps);
        assert forall|i: int| 0 <= i < comps.len() implies fits_tokens(views, #[trigger] comps[i])
            == (comps[i].len() == 1) by {}
    }
}

/// An escape `\c` (for `c` one of `\`, `*`, `?`, `[`, `]`) compiles to the
/// literal `c`, which excludes exactly the paths with a component equal to
/// `c` alone.
pub(crate) proof fn theorem_text_escape(c: char, p: CompiledPattern, comps: Seq<Seq<char>>)
    requires
        is_escapable(c),
    ensures
        compiled_from(seq!['\\', c]) == Ok::<PatternView, PatternError>(
            PatternView::Basename(seq![TokenView::Literal(c)]),
        ),
        compiles_to(seq!['\\', c], p) ==> (excluded(seq![p], comps) <==> exists|i: int|
            0 <= i < comps.len() && #[trigger] comps[i] == seq![c]),
{
    let text = seq!['\\', c];
    lemma_short_piece(text);
    assert(text.skip(2) =~= Seq::<char>::empty());
    assert(segment_tokens_text(text.skip(2), false) == Ok::<Seq<TokenView>, PatternError>(
        Seq::empty(),
    ));
    assert(seq![TokenView::Literal(c)] + Seq::<TokenView>::empty() =~= seq![TokenView::Literal(c)]);
    assert(segment_tokens_text(text, false) == Ok::<Seq<TokenView>, PatternError>(
        seq![TokenView::Literal(c)],
    ));
    lemma_basename_text(text);
    if compiles_to(text, p) {
        let views = seq![TokenView::Literal(c)];
        lemma_fixed_basename_excludes(p, views, comps);
        assert forall|i: int| 0 <= i < comps.len() implies fits_tokens(views, #[trigger] comps[i])
            == (comps[i] == seq![c]) by {
            if fits_tokens(views, comps[i]) {
                assert(comps[i] =~= seq![c]);
            }
        }
    }
}

/// A one-range class `[a-b]`, or `[!a-b]` when `negated`, compiles to that
/// class (negated exactly when written with `!`, endpoints in order), which
/// excludes exactly the paths with a one-scalar component inside the range
/// (outside it, when negated).
pub(crate) proof fn theorem_text_range_class(
    negated: bool,
    a: char,
    b: char,
    p: CompiledPattern,
    comps: Seq<Seq<char>>,
)
    requires
        is_class_literal(a),
        is_class_literal(b),
        a <= b,
        !negated ==> a != '!',
    ensures
        ({
            let text = if negated {
                seq!['[', '!', a, '-', b, ']']
            } else {
                seq!['[', a, '-', b, ']']
            };
            &&& compiled_from(text) == Ok::<PatternView, PatternError>(
                PatternView::Basename(seq![TokenView::Class(negated, seq![(a, b)])]),
            )
            &&& compiles_to(text, p) ==> (excluded(seq![p], comps) <==> exists|i: int|
                0 <= i < comps.len() && (#[trigger] comps[i]).len() == 1 && ((a <= comps[i][0]
                    && comps[i][0] <= b) != negated))
        }),
{
    let text = if negated {
        seq!['[', '!', a, '-', b, ']']
    } else {
        seq!['[', a, '-', b, ']']
    };
    let skip: int = if negated {
        1
    } else {
        0
    };
    // The class body after `[` and any `!`: `a-b]`.
    let after_open = text.skip(1);
    let body = after_open.skip(skip);
    assert(body =~= seq![a, '-', b, ']']);
    assert(after_open.len() > 0);
    assert((after_open[0] == '!') == negated);
    assert(class_char_text(body) == Ok::<(char, int), PatternError>((a, 1)));
    let dash = body.skip(1);
    assert(dash =~= seq!['-', b, ']']);
    let endpoint = dash.skip(1);
    assert(endpoint =~= seq![b, ']']);
    assert(class_char_text(endpoint) == Ok::<(char, int), PatternError>((b, 1)));
    let close = endpoint.skip(1);
    assert(close =~= seq![']']);
    assert(class_members_text(close, true) == Ok::<(Seq<(char, char)>, int), PatternError>(
        (Seq::empty(), 1),
    ));
    assert(seq![(a, b)] + Seq::<(char, char)>::empty() =~= seq![(a, b)]);
    assert(class_members_text(body, false) == Ok::<(Seq<(char, char)>, int), PatternError>(
        (seq![(a, b)], 4),
    ));
    assert(class_text(after_open) == Ok::<(bool, Seq<(char, char)>, int), PatternError>(
        (negated, seq![(a, b)], skip + 4),
    ));
    assert(text.skip(1 + skip + 4) =~= Seq::<char>::empty());
    assert(segment_tokens_text(text.skip(1 + skip + 4), false) == Ok::<
        Seq<TokenView>,
        PatternError,
    >(Seq::empty()));
    let views = seq![TokenView::Class(negated, seq![(a, b)])];
    assert(views + Seq::<TokenView>::empty() =~= views);
    assert(segment_tokens_text(text, false) == Ok::<Seq<TokenView>, PatternError>(views));
    assert(!has_separator(text));
    assert(text != seq!['.']);
    assert(text != seq!['.', '.']);
    assert(text != seq!['*', '*']);
    lemma_basename_text(text);
    if compiles_to(text, p) {
        lemma_fixed_basename_excludes(p, views, comps);
        assert forall|i: int| 0 <= i < comps.len() implies fits_tokens(views, #[trigger] comps[i])
            == (comps[i].len() == 1 && ((a <= comps[i][0] && comps[i][0] <= b) != negated)) by {
            let r = seq![(a, b)];
            if comps[i].len() == 1 {
                let x = comps[i][0];
                if class_contains(r, x) {
                    let k = choose|k: int| 0 <= k < r.len() && range_contains(#[trigger] r[k], x);
                    assert(k == 0);
                }
                if a <= x && x <= b {
                    assert(range_contains(r[0], x));
                }
            }
        }
    }
}

/// A plain name (no glob syntax, not `.` or `..`) compiles to the basename
/// pattern of its literal scalars, which excludes exactly the paths with a
/// component equal to the name: `target` excludes `target` and
/// `project/target` but not `targets` or `TARGET`.
pub(crate) proof fn theorem_text_plain_name(text: Seq<char>, p: CompiledPattern, comps: Seq<Seq<char>>)
    requires
        is_plain_name(text),
    ensures
        compiled_from(text) == Ok::<PatternView, PatternError>(
            PatternView::Basename(literal_views(text)),
        ),
        compiles_to(text, p) ==> (excluded(seq![p], comps) <==> exists|i: int|
            0 <= i < comps.len() && #[trigger] comps[i] == text),
{
    lemma_plain_name_compiles(text);
    if compiles_to(text, p) {
        lemma_excluded_single(p, comps);
        if let CompiledPattern::Basename(segment) = p {
            lemma_literal_tokens_from_views(segment.tokens@, text);
            theorem_plain_basename_matches_exact_components(segment, text, comps);
        }
    }
}

/// A text without `/` other than `**` that compiles is a basename glob:
/// it excludes a path exactly when some single component matches its
/// segment.
pub(crate) proof fn theorem_text_basename(text: Seq<char>, p: CompiledPattern, comps: Seq<Seq<char>>)
    requires
        compiles_to(text, p),
        !has_separator(text),
        text != seq!['*', '*'],
    ensures
        p matches CompiledPattern::Basename(segment) && (excluded(seq![p], comps) <==> exists|i: int|
            0 <= i < comps.len() && seg_match(segment.tokens@, #[trigger] comps[i])),
{
    lemma_excluded_single(p, comps);
    assert(text.len() > 0);
    lemma_basename_text(text);
    assert(component_text(text) != Ok::<ComponentView, PatternError>(ComponentView::Recursive));
}

/// The tokens of a segment without `*` contain no zero-or-more token, and a
/// segment never has more tokens than scalars.
proof fn lemma_segment_tokens_shape(rest: Seq<char>, after_star: bool)
    ensures
        segment_tokens_text(rest, after_star) matches Ok(tokens) ==> tokens.len() <= rest.len() && (
        (forall|j: int| 0 <= j < rest.len() ==> #[trigger] rest[j] != '*') ==> forall|j: int|
            0 <= j < tokens.len() ==> !(#[trigger] tokens[j] is ZeroOrMore)),
    decreases rest.len(),
{
    if rest.len() > 0 {
        let tail = rest.skip(1);
        assert forall|j: int| 0 <= j < tail.len() implies tail[j] == rest[j + 1] by {}
        if rest[0] == '[' {
            lemma_class_text_consumes(tail);
            if let Ok((negated, ranges, k)) = class_text(tail) {
                let after = rest.skip(1 + k);
                assert forall|j: int| 0 <= j < after.len() implies after[j] == rest[j + 1 + k] by {}
                lemma_segment_tokens_shape(after, false);
            }
        } else if rest[0] == '\\' && rest.len() >= 2 {
            let after = rest.skip(2);
            assert forall|j: int| 0 <= j < after.len() implies after[j] == rest[j + 2] by {}
            lemma_segment_tokens_shape(after, false);
        } else if rest[0] == '*' {
            lemma_segment_tokens_shape(tail, true);
        } else {
            lemma_segment_tokens_shape(tail, false);
        }
        if let Ok(tokens) = segment_tokens_text(rest, after_star) {
            assert(tokens.len() >= 1);
            assert forall|j: int| 1 <= j < tokens.len() implies tokens[j] == tokens.skip(1)[j - 1] by {}
        }
    }
}

/// `*` and `?` never match across `/` for a single-segment pattern without
/// `*`: every token consumes exactly one scalar, so a matching component has
/// exactly as many scalars as the pattern has tokens, and never more than
/// the pattern text has scalars.
pub(crate) proof fn theorem_text_without_star_is_fixed_width(
    text: Seq<char>,
    p: CompiledPattern,
    s: Seq<char>,
)
    requires
        compiles_to(text, p),
        !has_separator(text),
        forall|j: int| 0 <= j < text.len() ==> #[trigger] text[j] != '*',
    ensures
        p matches CompiledPattern::Basename(segment) && segment.tokens@.len() <= text.len() && (
        seg_match(segment.tokens@, s) ==> s.len() == segment.tokens@.len()),
{
    assert(text.len() > 0);
    assert(text != seq!['*', '*']) by {
        if text == seq!['*', '*'] {
            assert(text[0] == '*');
        }
    }
    lemma_basename_text(text);
    assert(text != seq!['.']);
    lemma_segment_tokens_shape(text, false);
    if let CompiledPattern::Basename(segment) = p {
        let tokens = segment.tokens@;
        let views = tokens_view(tokens);
        assert(tokens.len() == views.len());
        assert forall|j: int| 0 <= j < tokens.len() implies !(#[trigger] tokens[j] is ZeroOrMore) by {
            assert(token_view(tokens[j]) == views[j]);
        }
        theorem_fixed_width(tokens, s);
    }
}

/// `a/**` for a `/`-free text `a` that compiles to the basename segment
/// `ts` compiles to the root-relative pattern `ts/**`; it excludes exactly
/// the paths whose first component alone is excluded by `a`: that component
/// and everything beneath it, and nothing outside it.
pub(crate) proof fn theorem_text_trailing_globstar(
    a: Seq<char>,
    p: CompiledPattern,
    q: CompiledPattern,
    comps: Seq<Seq<char>>,
)
    requires
        !has_separator(a),
        a != seq!['*', '*'],
    ensures
        compiled_from(a) matches Ok(PatternView::Basename(ts)) ==> compiled_from(
            a + seq!['/', '*', '*'],
        ) == Ok::<PatternView, PatternError>(
            PatternView::RootRelative(seq![ComponentView::Segment(ts), ComponentView::Recursive]),
        ),
        compiles_to(a, q) && compiles_to(a + seq!['/', '*', '*'], p) ==> (excluded(seq![p], comps)
            <==> (comps.len() >= 1 && excluded(seq![q], comps.take(1)))),
{
    let text = a + seq!['/', '*', '*'];
    if compiled_from(a) is Ok {
        lemma_compile_globstar_suffix(a);
    }
    if compiles_to(a, q) && compiles_to(text, p) {
        lemma_excluded_single(p, comps);
        lemma_excluded_single(q, comps.take(1));
        lemma_basename_text(a);
        if let (CompiledPattern::RootRelative(pats), CompiledPattern::Basename(qs)) = (p, q) {
            lemma_components_view(pats@);
            assert(pats@.len() == 2);
            assert(component_view(pats@[1]) == ComponentView::Recursive);
            if let PathComponentPattern::Segment(first) = pats@[0] {
                assert(pats@ =~= seq![PathComponentPattern::Segment(first), PathComponentPattern::Recursive]);
                theorem_trailing_globstar(first, comps);
                if comps.len() >= 1 {
                    lemma_seg_match_view(first.tokens@, qs.tokens@, comps[0]);
                    assert(comps.take(1)[0] == comps[0]);
                    if pattern_matches(q, comps.take(1)) {
                        let i = choose|i: int| 0 <= i < comps.take(1).len() && seg_match(qs.tokens@, #[trigger] comps.take(1)[i]);
                        assert(i == 0);
                    }
                }
            } else {
                assert(component_view(pats@[0]) == ComponentView::Recursive);
            }
        }
    }
}

/// `a/**` compiles piecewise.
proof fn lemma_compile_globstar_suffix(a: Seq<char>)
    requires
        !has_separator(a),
        a != seq!['*', '*'],
        compiled_from(a) is Ok,
    ensures
        compiled_from(a) matches Ok(PatternView::Basename(ts)) ==> compiled_from(a + seq!['/', '*', '*'])
            == Ok::<PatternView, PatternError>(
            PatternView::RootRelative(seq![ComponentView::Segment(ts), ComponentView::Recursive]),
        ),
{
    let text = a + seq!['/', '*', '*'];
    let star2 = seq!['*', '*'];
    assert(a.len() > 0);
    lemma_basename_text(a);
    assert(text[a.len() as int] == '/');
    assert(has_separator(text));
    assert(separator_free(text, 0, a.len() as int)) by {
        assert forall|i: int| 0 <= i < a.len() implies text[i] != '/' by {
            assert(text[i] == a[i]);
        }
    }
    lemma_split_at_separator(text, a.len() as int);
    assert(text.take(a.len() as int) =~= a);
    assert(text.skip(a.len() as int + 1) =~= star2);
    assert(!has_separator(star2));
    lemma_split_without_separator(star2);
    lemma_components_text_cons(a, seq![star2]);
    assert(seq![star2] =~= seq![star2] + Seq::<Seq<char>>::empty());
    lemma_components_text_cons(star2, Seq::<Seq<char>>::empty());
    assert(seq![ComponentView::Recursive] + Seq::<ComponentView>::empty() =~= seq![ComponentView::Recursive]);
    if let Ok(PatternView::Basename(ts)) = compiled_from(a) {
        assert(seq![ComponentView::Segment(ts)] + seq![ComponentView::Recursive] =~= seq![
            ComponentView::Segment(ts),
            ComponentView::Recursive,
        ]);
    }
}

/// `a/**/b` for `/`-free texts `a` and `b` that compile to basename
/// segments compiles to `a`, `**`, `b`; it excludes exactly the paths of at
/// least two components whose first component alone is excluded by `a` and
/// whose last component alone is excluded by `b`.
pub(crate) proof fn theorem_text_interior_globstar(
    a: Seq<char>,
    b: Seq<char>,
    p: CompiledPattern,
    qa: CompiledPattern,
    qb: CompiledPattern,
    comps: Seq<Seq<char>>,
)
    requires
        !has_separator(a),
        !has_separator(b),
        a != seq!['*', '*'],
        b != seq!['*', '*'],
    ensures
        compiled_from(a) matches Ok(PatternView::Basename(ta)) ==> (compiled_from(b) matches Ok(
            PatternView::Basename(tb),
        ) ==> compiled_from(a + seq!['/', '*', '*', '/'] + b) == Ok::<PatternView, PatternError>(
            PatternView::RootRelative(
                seq![
                    ComponentView::Segment(ta),
                    ComponentView::Recursive,
                    ComponentView::Segment(tb),
                ],
            ),
        )),
        compiles_to(a, qa) && compiles_to(b, qb) && compiles_to(a + seq!['/', '*', '*', '/'] + b, p)
            ==> (excluded(seq![p], comps) <==> (comps.len() >= 2 && excluded(seq![qa], comps.take(1))
            && excluded(seq![qb], comps.skip(comps.len() - 1)))),
{
    let text = a + seq!['/', '*', '*', '/'] + b;
    if compiled_from(a) is Ok && compiled_from(b) is Ok {
        lemma_compile_interior_globstar(a, b);
    }
    if compiles_to(a, qa) && compiles_to(b, qb) && compiles_to(text, p) {
        lemma_excluded_single(p, comps);
        lemma_excluded_single(qa, comps.take(1));
        lemma_excluded_single(qb, comps.skip(comps.len() - 1));
        lemma_basename_text(a);
        lemma_basename_text(b);
        if let (
            CompiledPattern::RootRelative(pats),
            CompiledPattern::Basename(sa),
            CompiledPattern::Basename(sb),
        ) = (p, qa, qb) {
            lemma_components_view(pats@);
            assert(pats@.len() == 3);
            assert(component_view(pats@[1]) == ComponentView::Recursive);
            if let (PathComponentPattern::Segment(first), PathComponentPattern::Segment(last)) = (
                pats@[0],
                pats@[2],
            ) {
                assert(pats@ =~= seq![
                    PathComponentPattern::Segment(first),
                    PathComponentPattern::Recursive,
                    PathComponentPattern::Segment(last),
                ]);
                theorem_interior_globstar(first, last, comps);
                if comps.len() >= 1 {
                    let n = comps.len() - 1;
                    lemma_seg_match_view(first.tokens@, sa.tokens@, comps[0]);
                    lemma_seg_match_view(last.tokens@, sb.tokens@, comps[n]);
                    assert(comps.take(1)[0] == comps[0]);
                    assert(comps.skip(n)[0] == comps[n]);
                    if pattern_matches(qa, comps.take(1)) {
                        let i = choose|i: int| 0 <= i < comps.take(1).len() && seg_match(sa.tokens@, #[trigger] comps.take(1)[i]);
                        assert(i == 0);
                    }
                    if pattern_matches(qb, comps.skip(n)) {
                        let i = choose|i: int| 0 <= i < comps.skip(n).len() && seg_match(sb.tokens@, #[trigger] comps.skip(n)[i]);
                        assert(i == 0);
                    }
                }
            } else {
                if pats@[0] is Recursive {
                    assert(component_view(pats@[0]) == ComponentView::Recursive);
                } else {
                    assert(component_view(pats@[2]) == ComponentView::Recursive);
                }
            }
        }
    }
}

/// `a/**/b` compiles piecewise.
proof fn lemma_compile_interior_globstar(a: Seq<char>, b: Seq<char>)
    requires
        !has_separator(a),
        !has_separator(b),
        a != seq!['*', '*'],
        b != seq!['*', '*'],
        compiled_from(a) is Ok,
        compiled_from(b) is Ok,
    ensures
        compiled_from(a) matches Ok(PatternView::Basename(ta)) ==> (compiled_from(b) matches Ok(
            PatternView::Basename(tb),
        ) ==> compiled_from(a + seq!['/', '*', '*', '/'] + b) == Ok::<PatternView, PatternError>(
            PatternView::RootRelative(
                seq![
                    ComponentView::Segment(ta),
                    ComponentView::Recursive,
                    ComponentView::Segment(tb),
                ],
            ),
        )),
{
    let text = a + seq!['/', '*', '*', '/'] + b;
    let star2 = seq!['*', '*'];
    let rest1 = seq!['*', '*', '/'] + b;
    assert(a.len() > 0);
    assert(b.len() > 0);
    lemma_basename_text(a);
    lemma_basename_text(b);
    assert(text[a.len() as int] == '/');
    assert(has_separator(text));
    assert(separator_free(text, 0, a.len() as int)) by {
        assert forall|i: int| 0 <= i < a.len() implies text[i] != '/' by {
            assert(text[i] == a[i]);
        }
    }
    lemma_split_at_separator(text, a.len() as int);
    assert(text.take(a.len() as int) =~= a);
    assert(text.skip(a.len() as int + 1) =~= rest1);
    lemma_split_at_separator(rest1, 2);
    assert(rest1.take(2) =~= star2);
    assert(rest1.skip(3) =~= b);
    lemma_split_without_separator(b);
    lemma_components_text_cons(a, seq![star2] + seq![b]);
    lemma_components_text_cons(star2, seq![b]);
    assert(seq![b] =~= seq![b] + Seq::<Seq<char>>::empty());
    lemma_components_text_cons(b, Seq::<Seq<char>>::empty());
    if let (Ok(PatternView::Basename(ta)), Ok(PatternView::Basename(tb))) = (
        compiled_from(a),
        compiled_from(b),
    ) {
        assert(seq![ComponentView::Segment(tb)] + Seq::<ComponentView>::empty() =~= seq![
            ComponentView::Segment(tb),
        ]);
        assert(seq![ComponentView::Recursive] + seq![ComponentView::Segment(tb)] =~= seq![
            ComponentView::Recursive,
            ComponentView::Segment(tb),
        ]);
        assert(seq![ComponentView::Segment(ta)] + seq![
            ComponentView::Recursive,
            ComponentView::Segment(tb),
        ] =~= seq![
            ComponentView::Segment(ta),
            ComponentView::Recursive,
            ComponentView::Segment(tb),
        ]);
    }
}

/// With plain names, `a/**` excludes exactly `a` and everything beneath it.
pub(crate) proof fn theorem_text_trailing_globstar_plain(a: Seq<char>, p: CompiledPattern, comps: Seq<Seq<char>>)
    requires
        is_plain_name(a),
    ensures
        compiled_from(a + seq!['/', '*', '*']) is Ok,
        compiles_to(a + seq!['/', '*', '*'], p) ==> (excluded(seq![p], comps) <==> (comps.len() >= 1
            && comps[0] == a)),
{
    lemma_plain_name_compiles(a);
    lemma_plain_not_globstar(a);
    lemma_compile_globstar_suffix(a);
    if compiles_to(a + seq!['/', '*', '*'], p) {
        lemma_excluded_single(p, comps);
        if let CompiledPattern::RootRelative(pats) = p {
            lemma_components_view(pats@);
            assert(pats@.len() == 2);
            assert(component_view(pats@[1]) == ComponentView::Recursive);
            if let PathComponentPattern::Segment(first) = pats@[0] {
                assert(pats@ =~= seq![PathComponentPattern::Segment(first), PathComponentPattern::Recursive]);
                theorem_trailing_globstar(first, comps);
                lemma_literal_tokens_from_views(first.tokens@, a);
                if comps.len() >= 1 {
                    theorem_literal_pattern_is_exact(a, comps[0]);
                }
            } else {
                assert(component_view(pats@[0]) == ComponentView::Recursive);
            }
        }
    }
}

/// With plain names, `a/**/b` excludes exactly the paths of at least two
/// components that start with `a` and end with `b`: `projects/**/target`
/// excludes `projects/target` and `projects/x/y/target`, but not
/// `projects/x/target/file` or `other/projects/target`.
pub(crate) proof fn theorem_text_interior_globstar_plain(
    a: Seq<char>,
    b: Seq<char>,
    p: CompiledPattern,
    comps: Seq<Seq<char>>,
)
    requires
        is_plain_name(a),
        is_plain_name(b),
    ensures
        compiled_from(a + seq!['/', '*', '*', '/'] + b) is Ok,
        compiles_to(a + seq!['/', '*', '*', '/'] + b, p) ==> (excluded(seq![p], comps) <==> (
        comps.len() >= 2 && comps[0] == a && comps[comps.len() - 1] == b)),
{
    lemma_plain_name_compiles(a);
    lemma_plain_name_compiles(b);
    lemma_plain_not_globstar(a);
    lemma_plain_not_globstar(b);
    lemma_compile_interior_globstar(a, b);
    if compiles_to(a + seq!['/', '*', '*', '/'] + b, p) {
        lemma_excluded_single(p, comps);
        if let CompiledPattern::RootRelative(pats) = p {
            lemma_components_view(pats@);
            assert(pats@.len() == 3);
            assert(component_view(pats@[1]) == ComponentView::Recursive);
            if let (PathComponentPattern::Segment(first), PathComponentPattern::Segment(last)) = (
                pats@[0],
                pats@[2],
            ) {
                assert(pats@ =~= seq![
                    PathComponentPattern::Segment(first),
                    PathComponentPattern::Recursive,
                    PathComponentPattern::Segment(last),
                ]);
                theorem_interior_globstar(first, last, comps);
                lemma_literal_tokens_from_views(first.tokens@, a);
                lemma_literal_tokens_from_views(last.tokens@, b);
                if comps.len() >= 1 {
                    theorem_literal_pattern_is_exact(a, comps[0]);
                    theorem_literal_pattern_is_exact(b, comps[comps.len() - 1]);
                }
            } else {
                if pats@[0] is Recursive {
                    assert(component_view(pats@[0]) == ComponentView::Recursive);
                } else {
                    assert(component_view(pats@[2]) == ComponentView::Recursive);
                }
            }
        }
    }
}

/// A plain name has no `/` and is not `**`.
proof fn lemma_plain_not_globstar(a: Seq<char>)
    requires
        is_plain_name(a),
    ensures
        !has_separator(a),
        a != seq!['*', '*'],
{
    assert(!is_glob_syntax(a[0]));
    assert(!has_separator(a)) by {
        if has_separator(a) {
            let i = choose|i: int| 0 <= i < a.len() && a[i] == '/';
            assert(!is_glob_syntax(a[i]));
        }
    }
    assert(a != seq!['*', '*']) by {
        if a == seq!['*', '*'] {
            assert(a[0] == '*');
        }
    }
}

// ---------------------------------------------------------------------------
// Components versus the path string
// ---------------------------------------------------------------------------

/// `split_slash` is the separator split: its pieces contain no `/`, there
/// is at least one, and joining them with `/` gives back the text.
pub(crate) proof fn theorem_split_slash_is_separator_split(t: Seq<char>)
    ensures
        split_slash(t).len() >= 1,
        forall|i: int| 0 <= i < split_slash(t).len() ==> !has_separator(#[trigger] split_slash(t)[i]),
        join_slash(split_slash(t)) == t,
    decreases t.len(),
{
    if t.len() > 0 {
        let tail = t.skip(1);
        theorem_split_slash_is_separator_split(tail);
        let rest = split_slash(tail);
        if t[0] == '/' {
            let pieces = seq![Seq::<char>::empty()] + rest;
            assert(pieces.skip(1) =~= rest);
            assert(forall|i: int| 0 <= i < rest.len() ==> pieces[i + 1] == rest[i]);
            assert forall|i: int| 0 <= i < pieces.len() implies !has_separator(#[trigger] pieces[i]) by {
                if i > 0 {
                    assert(pieces[i] == rest[i - 1]);
                } else {
                    assert(pieces[0].len() == 0);
                }
            }
            assert(Seq::<char>::empty() + seq!['/'] + join_slash(rest) =~= t);
        } else {
            let first = seq![t[0]] + rest[0];
            let pieces = rest.update(0, first);
            assert(!has_separator(first)) by {
                if has_separator(first) {
                    let i = choose|i: int| 0 <= i < first.len() && first[i] == '/';
                    if i > 0 {
                        assert(first[i] == rest[0][i - 1]);
                    }
                }
            }
            assert forall|i: int| 0 <= i < pieces.len() implies !has_separator(#[trigger] pieces[i]) by {
                if i > 0 {
                    assert(pieces[i] == rest[i]);
                }
            }
            if rest.len() == 1 {
                assert(join_slash(rest) == rest[0]);
                assert(first =~= t);
            } else {
                assert(pieces.skip(1) =~= rest.skip(1));
                assert(join_slash(rest) == rest[0] + seq!['/'] + join_slash(rest.skip(1)));
                assert(first + seq!['/'] + join_slash(rest.skip(1)) =~= seq![t[0]] + (rest[0] + seq!['/']
                    + join_slash(rest.skip(1))));
                assert(seq![t[0]] + tail =~= t);
            }
        }
    }
}

/// Conversely, nonempty pieces without `/` are exactly the separator split
/// of their join: a component sequence that satisfies
/// `path_components_normal` is the `/`-separated pieces of its path string.
pub(crate) proof fn theorem_components_are_path_pieces(comps: Seq<Seq<char>>)
    requires
        comps.len() >= 1,
        forall|i: int| 0 <= i < comps.len() ==> !has_separator(#[trigger] comps[i]),
    ensures
        split_slash(join_slash(comps)) == comps,
    decreases comps.len(),
{
    if comps.len() == 1 {
        lemma_split_without_separator(comps[0]);
        assert(comps =~= seq![comps[0]]);
    } else {
        let first = comps[0];
        let rest = comps.skip(1);
        assert forall|i: int| 0 <= i < rest.len() implies !has_separator(#[trigger] rest[i]) by {
            assert(rest[i] == comps[i + 1]);
        }
        theorem_components_are_path_pieces(rest);
        let t = join_slash(comps);
        assert(t == first + seq!['/'] + join_slash(rest));
        assert(t[first.len() as int] == '/');
        assert(separator_free(t, 0, first.len() as int)) by {
            assert forall|i: int| 0 <= i < first.len() implies t[i] != '/' by {
                assert(t[i] == first[i]);
            }
        }
        lemma_split_at_separator(t, first.len() as int);
        assert(t.take(first.len() as int) =~= first);
        assert(t.skip(first.len() as int + 1) =~= join_slash(rest));
        assert(seq![first] + rest =~= comps);
    }
}

/// Matching on a `/`-free string agrees with the semantics in which no
/// token may consume `/`.
pub(crate) proof fn theorem_seg_match_within_on_separator_free(tokens: Seq<SegmentToken>, s: Seq<char>)
    requires
        !has_separator(s),
    ensures
        seg_match(tokens, s) == seg_match_within(tokens, s),
    decreases tokens.len(),
{
    if tokens.len() > 0 {
        if tokens.last() is ZeroOrMore {
            assert forall|k: int| 0 <= k <= s.len() implies !has_separator(#[trigger] s.take(k))
                && !has_separator(s.skip(k)) && seg_match(tokens.drop_last(), s.take(k))
                == seg_match_within(tokens.drop_last(), s.take(k)) by {
                assert(!has_separator(s.take(k))) by {
                    if has_separator(s.take(k)) {
                        let i = choose|i: int| 0 <= i < s.take(k).len() && s.take(k)[i] == '/';
                        assert(s[i] == '/');
                    }
                }
                assert(!has_separator(s.skip(k))) by {
                    if has_separator(s.skip(k)) {
                        let i = choose|i: int| 0 <= i < s.skip(k).len() && s.skip(k)[i] == '/';
                        assert(s[i + k] == '/');
                    }
                }
                theorem_seg_match_within_on_separator_free(tokens.drop_last(), s.take(k));
            }
        } else if s.len() > 0 {
            assert(s.last() != '/') by {
                if s.last() == '/' {
                    assert(s[s.len() - 1] == '/');
                }
            }
            assert(!has_separator(s.drop_last())) by {
                if has_separator(s.drop_last()) {
                    let i = choose|i: int| 0 <= i < s.drop_last().len() && s.drop_last()[i] == '/';
                    assert(s[i] == '/');
                }
            }
            theorem_seg_match_within_on_separator_free(tokens.drop_last(), s.drop_last());
        }
    }
}

/// Under the semantics `seg_match_within`, no token consumes a `/`: a
/// matched string never contains one.
pub(crate) proof fn theorem_within_never_consumes_separator(tokens: Seq<SegmentToken>, s: Seq<char>)
    requires
        seg_match_within(tokens, s),
    ensures
        !has_separator(s),
    decreases tokens.len(),
{
    if tokens.len() > 0 {
        if tokens.last() is ZeroOrMore {
            let k = choose|k: int|
                0 <= k <= s.len() && !has_separator(s.skip(k)) && seg_match_within(
                    tokens.drop_last(),
                    #[trigger] s.take(k),
                );
            theorem_within_never_consumes_separator(tokens.drop_last(), s.take(k));
            assert(!has_separator(s)) by {
                if has_separator(s) {
                    let i = choose|i: int| 0 <= i < s.len() && s[i] == '/';
                    if i < k {
                        assert(s.take(k)[i] == '/');
                    } else {
                        assert(s.skip(k)[i - k] == '/');
                    }
                }
            }
        } else {
            theorem_within_never_consumes_separator(tokens.drop_last(), s.drop_last());
            assert(!has_separator(s)) by {
                if has_separator(s) {
                    let i = choose|i: int| 0 <= i < s.len() && s[i] == '/';
                    if i < s.len() - 1 {
                        assert(s.drop_last()[i] == '/');
                    }
                }
            }
        }
    }
}

/// `*`, `?`, and classes never match across `/`. Under the hypothesis
/// `path_components_normal` (every component nonempty and `/`-free, which
/// `Path::components` establishes in `exclusion.rs`), the components the
/// matcher decides are exactly the `/`-separated pieces of the path string
/// `join_slash(comps)`, and a basename text excludes the path exactly when
/// one of those pieces is matched with no token consuming a `/`.
///
/// The hypothesis is necessary: `seg_match` itself lets `*`, `?`, a negated
/// class, and a range whose endpoints straddle `/` (such as `[.-0]`) accept
/// the scalar `/`, so applied to a string containing `/` they would match
/// across it.
pub(crate) proof fn theorem_wildcards_never_cross_separators(
    text: Seq<char>,
    p: CompiledPattern,
    comps: Seq<Seq<char>>,
)
    requires
        compiles_to(text, p),
        !has_separator(text),
        text != seq!['*', '*'],
        comps.len() >= 1,
        path_components_normal(comps),
    ensures
        split_slash(join_slash(comps)) == comps,
        p matches CompiledPattern::Basename(segment) && (excluded(seq![p], comps) <==> exists|i: int|
            0 <= i < comps.len() && seg_match_within(segment.tokens@, #[trigger] comps[i])),
{
    assert forall|i: int| 0 <= i < comps.len() implies !has_separator(#[trigger] comps[i]) by {}
    theorem_components_are_path_pieces(comps);
    theorem_text_basename(text, p, comps);
    if let CompiledPattern::Basename(segment) = p {
        assert forall|i: int| 0 <= i < comps.len() implies seg_match(segment.tokens@, #[trigger] comps[i])
            == seg_match_within(segment.tokens@, comps[i]) by {
            theorem_seg_match_within_on_separator_free(segment.tokens@, comps[i]);
        }
    }
}

/// A root-relative text whose pieces are all segments (no `**`) has no
/// implicit leading `**/` and each piece consumes exactly one component: it
/// excludes only paths with exactly as many components as it has pieces,
/// each matched by its own piece.
pub(crate) proof fn theorem_text_without_globstar_matches_piecewise(
    text: Seq<char>,
    p: CompiledPattern,
    comps: Seq<Seq<char>>,
)
    requires
        compiles_to(text, p),
        has_separator(text),
        forall|i: int| 0 <= i < split_slash(text).len() ==> #[trigger] split_slash(text)[i] != seq![
            '*',
            '*',
        ],
    ensures
        p matches CompiledPattern::RootRelative(pats) && pats@.len() == split_slash(text).len() && (
        excluded(seq![p], comps) <==> (comps.len() == pats@.len() && forall|i: int|
            0 <= i < comps.len() ==> seg_match(segment_tokens(pats@[i]), #[trigger] comps[i]))),
{
    lemma_excluded_single(p, comps);
    let pieces = split_slash(text);
    lemma_components_text_shape(pieces);
    if let CompiledPattern::RootRelative(pats) = p {
        assert(pats@.len() == components_view(pats@).len());
        assert forall|i: int| 0 <= i < pats@.len() implies (#[trigger] pats@[i]) is Segment by {
            assert(component_view(pats@[i]) == components_view(pats@)[i]);
        }
        theorem_segments_match_single_components(pats@, comps);
        theorem_split_slash_is_separator_split(text);
        assert(pats@.len() >= 1);
    }
}

/// Compiled pieces: one component per piece, recursive exactly for `**`.
proof fn lemma_components_text_shape(pieces: Seq<Seq<char>>)
    ensures
        components_text(pieces) matches Ok(cs) ==> cs.len() == pieces.len() && forall|i: int|
            0 <= i < cs.len() ==> ((#[trigger] cs[i] is Recursive) == (pieces[i] == seq!['*', '*'])),
    decreases pieces.len(),
{
    if pieces.len() > 0 {
        lemma_components_text_shape(pieces.skip(1));
        if let Ok(cs) = components_text(pieces) {
            if let Ok(c) = component_text(pieces[0]) {
                if let Ok(rest) = components_text(pieces.skip(1)) {
                    assert(cs == seq![c] + rest);
                    assert forall|i: int| 0 <= i < cs.len() implies ((#[trigger] cs[i] is Recursive) == (
                    pieces[i] == seq!['*', '*'])) by {
                        if i > 0 {
                            assert(cs[i] == rest[i - 1]);
                            assert(pieces.skip(1)[i - 1] == pieces[i]);
                        }
                    }
                }
            }
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    /// The segment matcher exactly as `exclusion.rs` stated it before it
    /// moved into this kernel, kept as an oracle.
    fn reference_is_match(pattern: &SegmentPattern, value: &str) -> bool {
        let characters = value.chars().collect::<Vec<_>>();
        let mut matched = vec![false; characters.len() + 1];
        matched[0] = true;

        for token in &pattern.tokens {
            let mut next = vec![false; characters.len() + 1];
            match token {
                SegmentToken::ZeroOrMore => {
                    next[0] = matched[0];
                    for index in 1..=characters.len() {
                        next[index] = matched[index] || next[index - 1];
                    }
                }
                SegmentToken::Literal(expected) => {
                    for index in 1..=characters.len() {
                        next[index] = matched[index - 1] && characters[index - 1] == *expected;
                    }
                }
                SegmentToken::Any => {
                    next[1..(characters.len() + 1)].copy_from_slice(&matched[..characters.len()]);
                }
                SegmentToken::Class(class) => {
                    for index in 1..=characters.len() {
                        next[index] =
                            matched[index - 1] && reference_class(class, characters[index - 1]);
                    }
                }
            }
            matched = next;
        }

        matched[characters.len()]
    }

    fn reference_class(class: &CharacterClass, value: char) -> bool {
        let contained = class
            .ranges
            .iter()
            .any(|(start, end)| *start <= value && value <= *end);
        contained != class.negated
    }

    /// The root-relative matcher as `exclusion.rs` stated it before.
    fn reference_path_matches(pattern: &[PathComponentPattern], components: &[&str]) -> bool {
        let mut matched = vec![false; components.len() + 1];
        matched[0] = true;

        for component_pattern in pattern {
            let mut next = vec![false; components.len() + 1];
            match component_pattern {
                PathComponentPattern::Recursive => {
                    next[0] = matched[0];
                    for index in 1..=components.len() {
                        next[index] = matched[index] || next[index - 1];
                    }
                }
                PathComponentPattern::Segment(segment) => {
                    for index in 1..=components.len() {
                        next[index] = matched[index - 1]
                            && reference_is_match(segment, components[index - 1]);
                    }
                }
            }
            matched = next;
        }

        matched[components.len()]
    }

    /// The token kinds used by the exhaustive comparisons.
    fn token(code: u8) -> SegmentToken {
        match code {
            0 => SegmentToken::Literal('a'),
            1 => SegmentToken::Literal('é'),
            2 => SegmentToken::Any,
            3 => SegmentToken::ZeroOrMore,
            4 => SegmentToken::Class(CharacterClass {
                negated: false,
                ranges: vec![('a', 'a'), ('x', 'z')],
            }),
            _ => SegmentToken::Class(CharacterClass {
                negated: true,
                ranges: vec![('a', 'b')],
            }),
        }
    }

    fn segment(codes: &[u8]) -> SegmentPattern {
        SegmentPattern {
            tokens: codes.iter().map(|&code| token(code)).collect(),
        }
    }

    /// Every sequence over `alphabet` of length at most `max_len`.
    fn sequences<T: Clone>(alphabet: &[T], max_len: usize) -> Vec<Vec<T>> {
        let mut all = vec![Vec::new()];
        let mut frontier = vec![Vec::new()];
        for _ in 0..max_len {
            let mut next = Vec::new();
            for prefix in &frontier {
                for item in alphabet {
                    let mut extended = prefix.clone();
                    extended.push(item.clone());
                    next.push(extended);
                }
            }
            all.extend(next.iter().cloned());
            frontier = next;
        }
        all
    }

    #[test]
    fn segment_matcher_matches_reference_on_every_short_input() {
        let patterns = sequences(&[0u8, 1, 2, 3, 4, 5], 3);
        let values = sequences(&['a', 'é', 'y', 'b'], 4)
            .into_iter()
            .map(|chars| chars.into_iter().collect::<String>())
            .collect::<Vec<_>>();
        for codes in &patterns {
            let pattern = segment(codes);
            for value in &values {
                assert_eq!(
                    pattern.is_match(value),
                    reference_is_match(&pattern, value),
                    "{codes:?} {value:?}"
                );
            }
        }
    }

    #[test]
    fn character_classes_match_members_and_negation() {
        let class = CharacterClass {
            negated: false,
            ranges: vec![('a', 'c'), ('x', 'x')],
        };
        assert!(class.matches('a'));
        assert!(class.matches('b'));
        assert!(class.matches('c'));
        assert!(class.matches('x'));
        assert!(!class.matches('d'));
        assert!(!class.matches('w'));
        let negated = CharacterClass {
            negated: true,
            ranges: vec![('0', '9')],
        };
        assert!(negated.matches('x'));
        assert!(!negated.matches('7'));
    }

    #[test]
    fn segment_tokens_consume_scalars_within_one_component() {
        assert!(segment(&[]).is_match(""));
        assert!(!segment(&[]).is_match("a"));
        assert!(segment(&[3]).is_match(""));
        assert!(segment(&[3]).is_match("anything"));
        assert!(segment(&[2]).is_match("é"));
        assert!(!segment(&[2]).is_match(""));
        assert!(!segment(&[2]).is_match("ab"));
        assert!(segment(&[0, 3]).is_match("a"));
        assert!(segment(&[0, 3]).is_match("ab"));
        assert!(!segment(&[0, 3]).is_match("ba"));
        assert!(segment(&[3, 0]).is_match("ba"));
        assert!(!segment(&[3, 0]).is_match("ab"));
    }

    #[test]
    fn path_matcher_matches_reference_on_every_short_input() {
        let kinds = [0u8, 1, 2, 3];
        let pattern_codes = sequences(&kinds, 3);
        let component_values = sequences(&["a", "é", "ab", ""], 3);
        for codes in &pattern_codes {
            let pattern = codes
                .iter()
                .map(|&code| match code {
                    0 => PathComponentPattern::Recursive,
                    1 => PathComponentPattern::Segment(segment(&[0])),
                    2 => PathComponentPattern::Segment(segment(&[3])),
                    _ => PathComponentPattern::Segment(segment(&[2, 3])),
                })
                .collect::<Vec<_>>();
            for components in &component_values {
                assert_eq!(
                    path_pattern_matches(&pattern, components),
                    reference_path_matches(&pattern, components),
                    "{codes:?} {components:?}"
                );
            }
        }
    }

    /// Whether a compiled pattern satisfies `pattern_well_formed`.
    fn well_formed(pattern: &CompiledPattern) -> bool {
        fn tokens(pattern: &SegmentPattern) -> bool {
            !pattern.tokens.is_empty()
                && pattern.tokens.iter().all(|token| match token {
                    SegmentToken::Class(class) => {
                        !class.ranges.is_empty()
                            && class.ranges.iter().all(|(start, end)| start <= end)
                    }
                    _ => true,
                })
                && pattern.tokens.windows(2).all(|pair| {
                    !matches!(pair, [SegmentToken::ZeroOrMore, SegmentToken::ZeroOrMore])
                })
        }
        match pattern {
            CompiledPattern::Basename(segment) => tokens(segment),
            CompiledPattern::RootRelative(components) => {
                !components.is_empty()
                    && (components.len() > 1
                        || matches!(components[0], PathComponentPattern::Recursive))
                    && components.iter().all(|component| match component {
                        PathComponentPattern::Recursive => true,
                        PathComponentPattern::Segment(segment) => tokens(segment),
                    })
            }
        }
    }

    #[test]
    fn compiled_patterns_are_well_formed() {
        let alphabet = ['a', '*', '?', '[', ']', '\\', '-', '!', '/', '.'];
        let mut compiled = 0;
        for pattern in sequences(&alphabet, 5) {
            let pattern = pattern.into_iter().collect::<String>();
            if let Ok(result) = compile_pattern(&pattern) {
                assert!(well_formed(&result), "{pattern:?}");
                compiled += 1;
            }
        }
        assert!(compiled > 1000, "{compiled}");
    }

    #[test]
    fn plain_names_compile_to_their_literal_scalars() {
        for name in ["target", "é", "a.b", "...", "!x", "{a,b}"] {
            match compile_pattern(name) {
                Ok(CompiledPattern::Basename(segment)) => {
                    let literals = segment
                        .tokens
                        .iter()
                        .map(|token| match token {
                            SegmentToken::Literal(character) => Some(*character),
                            _ => None,
                        })
                        .collect::<Option<String>>();
                    assert_eq!(literals.as_deref(), Some(name));
                }
                _ => panic!("{name:?} did not compile to a basename pattern"),
            }
        }
    }

    #[test]
    fn unreachable_diagnostics_need_inputs_the_compiler_never_passes() {
        // `compile_pattern` splits on `/` and checks for `]` before parsing a
        // class member, so these internal outcomes are reachable only by
        // calling the helpers directly.
        assert!(matches!(
            compile_segment(&['a', '/'], 0, 2),
            Err(PatternError::SeparatorInSegment)
        ));
        assert!(matches!(
            parse_class_character(&['/'], 0, 1),
            Err(PatternError::SeparatorInClass)
        ));
        assert!(matches!(
            parse_class_character(&[']'], 0, 1),
            Err(PatternError::UnescapedCloseInClass)
        ));
        assert!(matches!(
            parse_class_character(&[], 0, 0),
            Err(PatternError::UnterminatedClass)
        ));
        assert!(matches!(parse_class_character(&['x'], 0, 1), Ok(('x', 1))));
    }

    #[test]
    fn segment_semantics_accept_a_separator_only_inside_a_component() {
        // The documented scope note: `?`, `*`, a negated class, and a range
        // straddling `/` accept the scalar `/` when a segment is applied to
        // a string that contains one. `Path::components` never yields such a
        // component, so this is unreachable through `exclusion.rs`.
        for pattern in ["?", "*", "[!a]", "[.-0]"] {
            match compile_pattern(pattern) {
                Ok(CompiledPattern::Basename(segment)) => {
                    assert!(segment.is_match("/"), "{pattern:?}");
                }
                _ => panic!("{pattern:?} did not compile to a basename pattern"),
            }
        }
        match compile_pattern("[a-c]") {
            Ok(CompiledPattern::Basename(segment)) => assert!(!segment.is_match("/")),
            _ => panic!("[a-c] did not compile to a basename pattern"),
        }
        // Split into components, `a/b` is never one string for `a*b`.
        let star = compile_pattern("a*b").expect("valid pattern");
        assert!(!matches_components(
            std::slice::from_ref(&star),
            &["a", "b"]
        ));
        assert!(matches_components(std::slice::from_ref(&star), &["axb"]));
    }

    #[test]
    fn union_dispatches_basename_and_root_relative_patterns() {
        let basename = CompiledPattern::Basename(segment(&[0]));
        let rooted = CompiledPattern::RootRelative(vec![
            PathComponentPattern::Segment(segment(&[1])),
            PathComponentPattern::Recursive,
        ]);
        let globstar = CompiledPattern::RootRelative(vec![PathComponentPattern::Recursive]);

        // Basename patterns match any single component at any depth.
        assert!(matches_components(std::slice::from_ref(&basename), &["a"]));
        assert!(matches_components(
            std::slice::from_ref(&basename),
            &["x", "a"]
        ));
        assert!(!matches_components(
            std::slice::from_ref(&basename),
            &["ab", "x"]
        ));

        // Root-relative patterns match the complete sequence only.
        assert!(matches_components(std::slice::from_ref(&rooted), &["é"]));
        assert!(matches_components(
            std::slice::from_ref(&rooted),
            &["é", "x", "y"]
        ));
        assert!(!matches_components(
            std::slice::from_ref(&rooted),
            &["x", "é"]
        ));

        // The union is any member; the empty path is never excluded.
        let union = [basename, rooted];
        assert!(matches_components(&union, &["é", "q"]));
        assert!(matches_components(&union, &["q", "a"]));
        assert!(!matches_components(&union, &["q"]));
        assert!(!matches_components(&union, &[]));
        assert!(matches_components(std::slice::from_ref(&globstar), &["q"]));
        assert!(!matches_components(std::slice::from_ref(&globstar), &[]));
        assert!(!matches_components(&[], &["q"]));
    }
}
