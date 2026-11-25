//! Verified exclusion-glob compilation and matching.
//!
//! This kernel compiles exclusion patterns and decides them with a
//! dynamic-programming matcher. Its specification restates the exclusion
//! language of the v0.2 contract (`docs/v0.2-contract.md`, "Exclusion
//! patterns") as recursive definitions over the compiled tokens, independent
//! of the table-filling algorithm, and Verus proves that the executable
//! matcher agrees with it on every input:
//!
//! - [`seg_match`]: a segment pattern matches one complete component; `*`
//!   extends whatever its preceding tokens matched by any number of scalars,
//!   and a literal, `?`, or class consumes exactly one scalar;
//! - [`path_match`]: a root-relative pattern matches the complete component
//!   sequence; `**` extends whatever its preceding components matched by any
//!   number of whole components, and a segment consumes exactly one;
//! - [`excluded`]: a path is excluded when it has at least one component and
//!   some pattern matches it, where a basename pattern matches when any
//!   single component matches and a root-relative pattern must match the
//!   whole sequence.
//!
//! [`compile_pattern`] is verified to produce only well-formed patterns
//! ([`pattern_well_formed`]: every segment has a token, every class has at
//! least one ascending range, no two `*` tokens are adjacent, and a
//! root-relative pattern is `**` alone or has at least two components), to
//! compile a plain name to exactly its literal scalars, and never to report
//! the three diagnostics that no input can reach ([`is_unreachable_error`]).
//! Its accept/reject decisions and diagnostic precedence are checked against
//! the previous implementation by exhaustive differential tests in
//! `exclusion.rs`.
//!
//! The theorems below state contract-level consequences:
//!
//! - `theorem_seg_concat`: segment matching is ordinary concatenation, so a
//!   pattern `a b` matches exactly the strings that split into an `a` part
//!   and a `b` part;
//! - `theorem_star_matches_everything`, `theorem_question_mark_is_one_scalar`,
//!   `theorem_literal_pattern_is_exact`, and `theorem_fixed_width`: `*`
//!   matches any scalars, `?` exactly one, a literal-only pattern only its
//!   own exact scalars (no substring search, case folding, or
//!   normalization), and a pattern without `*` only strings of its own
//!   length, one token per scalar;
//! - `theorem_segments_match_single_components`: without `**`, each segment
//!   pattern consumes exactly one whole component, in order, so `*` and `?`
//!   never match across a `/` and there is no implicit leading `**/`;
//! - `theorem_trailing_globstar` and `theorem_interior_globstar`: `a/**`
//!   matches `a` and everything beneath it, and `a/**/b` matches exactly the
//!   paths of at least two components that start with an `a` match and end
//!   with a `b` match (the contract's `projects/**/target` examples);
//! - `theorem_plain_basename_matches_exact_components`: a plain name such as
//!   `target` excludes exactly the paths with a component equal to it;
//! - `theorem_globstar_matches_every_path`: the pattern `**` excludes every
//!   nonempty path;
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
/// token; [`seg_match`] treats it separately.
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
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r matches Ok(compiled) ==> pattern_well_formed(compiled),
        r matches Err(e) ==> !is_unreachable_error(e),
        is_plain_name(pattern@) ==> (r matches Ok(CompiledPattern::Basename(segment))
            && segment.tokens@ == literal_tokens(pattern@)),
))]
pub(crate) fn compile_pattern(pattern: &str) -> Result<CompiledPattern, PatternError> {
    let characters = scalars(pattern);
    let len = characters.len();
    if len == 0 {
        return Err(PatternError::Empty);
    }

    let first_end = component_end(&characters, 0);
    if first_end == len {
        // No `/`: a basename glob, unless the only component is `**`.
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(characters@.subrange(0, len as int) =~= pattern@);
        }
        return match compile_component(&characters, 0, len)? {
            PathComponentPattern::Recursive => Ok(CompiledPattern::RootRelative(vec![
                PathComponentPattern::Recursive,
            ])),
            PathComponentPattern::Segment(segment) => Ok(CompiledPattern::Basename(segment)),
        };
    }

    #[cfg(verus_keep_ghost)]
    proof! {
        // A plain name has no `/`, so it never reaches this point.
        if is_plain_name(pattern@) {
            assert(!is_glob_syntax(pattern@[first_end as int]));
        }
        assert(!is_plain_name(pattern@));
    }
    let mut components = Vec::new();
    components.push(compile_component(&characters, 0, first_end)?);
    let mut start = first_end + 1;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            len == characters@.len(),
            !is_plain_name(pattern@),
            1 <= start <= len,
            components@.len() >= 1,
            forall|i: int|
                0 <= i < components@.len() ==> component_well_formed(#[trigger] components@[i]),
        decreases len - start
    ))]
    loop {
        let end = component_end(&characters, start);
        components.push(compile_component(&characters, start, end)?);
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
        r matches Ok(component) ==> component_well_formed(component),
        r matches Err(e) ==> !is_unreachable_error(e),
        is_plain_name(characters@.subrange(start as int, end as int)) ==> (r matches Ok(
            PathComponentPattern::Segment(segment),
        ) && segment.tokens@ == literal_tokens(characters@.subrange(start as int, end as int))),
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
        if width == 1 && characters@[start as int] == '.' {
            assert(c =~= seq!['.']);
        }
        if width == 2 && characters@[start as int] == '.' && characters@[start + 1] == '.' {
            assert(c =~= seq!['.', '.']);
        }
        if width == 2 && characters@[start as int] == '*' {
            assert(c[0] == '*');
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
        if is_plain_name(c) {
            assert forall|i: int| start <= i < end implies !is_glob_syntax(
                #[trigger] characters@[i],
            ) by {
                assert(c[i - start] == characters@[i]);
            }
        }
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
        r matches Ok(segment) ==> tokens_well_formed(segment.tokens@),
        r matches Err(e) ==> !is_unreachable_error(e),
        (forall|i: int| start <= i < end ==> !is_glob_syntax(#[trigger] characters@[i])) ==> (r
            matches Ok(segment) && segment.tokens@ == literal_tokens(
            characters@.subrange(start as int, end as int),
        )),
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
        assert(literal_tokens(characters@.subrange(start as int, start as int)) =~= Seq::<
            SegmentToken,
        >::empty());
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            start <= index <= end,
            end <= characters@.len(),
            index > start ==> tokens@.len() >= 1,
            previous_was_star == (tokens@.len() > 0 && tokens@.last() is ZeroOrMore),
            forall|i: int| 0 <= i < tokens@.len() ==> token_well_formed(#[trigger] tokens@[i]),
            forall|i: int|
                0 < i < tokens@.len() ==> !(tokens@[i - 1] is ZeroOrMore
                    && (#[trigger] tokens@[i]) is ZeroOrMore),
            separator_free(characters@, start as int, end as int),
            (forall|i: int| start <= i < end ==> !is_glob_syntax(#[trigger] characters@[i]))
                ==> tokens@ == literal_tokens(characters@.subrange(start as int, index as int)),
        decreases end - index
    ))]
    while index < end {
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
            if forall|i: int| start <= i < end ==> !is_glob_syntax(#[trigger] characters@[i]) {
                assert(!is_glob_syntax(characters@[index - 1]));
                assert(token == SegmentToken::Literal(characters@[index - 1]));
                assert(characters@.subrange(start as int, index as int) =~= characters@.subrange(
                    start as int,
                    index - 1,
                ).push(characters@[index - 1]));
                assert(literal_tokens(characters@.subrange(start as int, index as int))
                    =~= literal_tokens(characters@.subrange(start as int, index - 1)).push(token));
            }
        }
        tokens.push(token);
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

    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            start <= index <= end,
            end <= characters@.len(),
            forall|i: int| 0 <= i < ranges@.len() ==> (#[trigger] ranges@[i]).0 <= ranges@[i].1,
            separator_free(characters@, start as int, end as int),
        decreases end - index
    ))]
    loop {
        if index >= end {
            return Err(PatternError::UnterminatedClass);
        }
        if characters[index] == ']' {
            if ranges.is_empty() {
                return Err(PatternError::EmptyClass);
            }
            return Ok((CharacterClass { negated, ranges }, index + 1));
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
            ranges.push((first, last));
        } else {
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
/// in order: `*`, `?`, and classes never match across a `/`, and a
/// root-relative pattern has no implicit leading `**/`.
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
/// compiled pattern `**` excludes every nonempty path.
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
