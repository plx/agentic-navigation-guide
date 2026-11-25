//! Verified verifier decisions: placeholder accounting and item resolution.
//!
//! These kernels make the pure decisions of `Verifier::verify` for one
//! sibling list and one resolution step (`docs/v0.2-contract.md`,
//! "Placeholders and listing completeness" and "Textual-item containment"):
//!
//! - [`first_component`]: the first `/`-separated component of a listed item
//!   path, exactly as `str::split('/')` yields it; the names of the
//!   non-placeholder siblings' first components are the "mentioned" names of
//!   that sibling list;
//! - [`has_meaningful_comment`]: a comment is meaningful when it contains a
//!   scalar that is not Unicode `White_Space` (so `str::trim` leaves it
//!   nonempty). This is wider than the parser's comment normalization, which
//!   trims only SP and HTAB as the contract's `WSP` does: a comment of only
//!   other white space (for example U+3000) survives parsing but does not
//!   satisfy a placeholder. That was already the verifier's behavior;
//! - [`placeholder_satisfied`]: a placeholder is satisfied exactly when the
//!   directory snapshot has an entry that no sibling mentions, or when the
//!   placeholder carries a meaningful comment;
//! - [`resolution_step`]: one exact-snapshot resolution step resolves the
//!   final component (requiring canonical containment for a supported entry
//!   type), descends through a real directory, or reports a type mismatch.
//!
//! Which snapshot entries are unmentioned is computed by the verifier with a
//! hash set of the mentioned names and one pass over the sorted snapshot,
//! which keeps the accounting linear; the kernels receive that boolean.
//!
//! The theorems below state the contract-level consequences:
//!
//! - `theorem_placeholder_acceptance`: a placeholder is accepted exactly when
//!   some snapshot entry is unmentioned or its comment has a scalar that is
//!   not white space; with every entry mentioned, an absent, empty, or
//!   all-white-space comment is rejected;
//! - `theorem_first_component_shape`: the first component is the longest
//!   `/`-free prefix of the path, and the path is either exactly it or
//!   continues with `/`;
//! - `theorem_resolution_descends_only_through_directories`: resolution
//!   descends only through a non-final component that is a real directory,
//!   reports a type mismatch for every other non-final entry (links, reparse
//!   points, special files, regular files), and requires containment exactly
//!   for a final entry of a supported type.

use crate::entry_type::{EntryClassification, SupportedEntryKind};
#[cfg(verus_keep_ghost)]
use crate::proven::path_rules::{split_components, split_step};
#[cfg(verus_keep_ghost)]
use crate::proven::windows_names::{first_index_of, lemma_first_index_of};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::char::is_white_space;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

/// The decision of one exact-snapshot resolution step.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResolutionStep {
    /// The final component resolved; canonical containment is required when
    /// the entry has a supported type.
    Resolved { require_containment: bool },
    /// A non-final component is a real directory: check containment and
    /// descend into it.
    Descend,
    /// A non-final component is not a real directory.
    TypeMismatch,
}

#[cfg(verus_keep_ghost)]
verus! {

/// Some scalar of the comment is not Unicode `White_Space`.
pub(crate) open spec fn is_meaningful_comment(c: Seq<char>) -> bool {
    exists|i: int| 0 <= i < c.len() && !is_white_space(#[trigger] c[i])
}

/// The scalar view of an optional comment.
pub(crate) open spec fn comment_view(comment: Option<&str>) -> Option<Seq<char>> {
    match comment {
        Some(text) => Some(text@),
        None => None,
    }
}

/// A placeholder is accepted when the snapshot has an unmentioned entry or
/// its comment is meaningful.
pub(crate) open spec fn placeholder_accepted(
    has_unmentioned_entry: bool,
    comment: Option<Seq<char>>,
) -> bool {
    has_unmentioned_entry || match comment {
        Some(c) => is_meaningful_comment(c),
        None => false,
    }
}

/// The first `/`-separated component of a path (`str::split('/').next()`).
pub(crate) open spec fn first_component_spec(path: Seq<char>) -> Seq<char> {
    split_components(path)[0]
}

/// The decision of one resolution step.
pub(crate) open spec fn resolution_step_spec(
    is_last: bool,
    classification: EntryClassification,
) -> ResolutionStep {
    if is_last {
        ResolutionStep::Resolved { require_containment: classification is Ok }
    } else if classification == Ok::<SupportedEntryKind, crate::entry_type::UnsupportedEntryKind>(
        SupportedEntryKind::Directory,
    ) {
        ResolutionStep::Descend
    } else {
        ResolutionStep::TypeMismatch
    }
}

/// A `/`-free sequence splits into exactly itself.
pub(crate) proof fn lemma_split_free(s: Seq<char>)
    requires
        forall|i: int| 0 <= i < s.len() ==> s[i] != '/',
    ensures
        split_components(s) == seq![s],
    decreases s.len(),
{
    if s.len() > 0 {
        lemma_split_free(s.drop_last());
        let prev = split_components(s.drop_last());
        assert(prev == seq![s.drop_last()]);
        assert(s.last() != '/');
        assert(s.drop_last().push(s.last()) =~= s);
        assert(split_step(prev, s.last()) =~= seq![s]);
    } else {
        assert(s =~= Seq::<char>::empty());
    }
}

/// After the first `/` at `k`, every longer prefix splits into at least two
/// components, the first of which is `s[..k]`.
proof fn lemma_split_head_take(s: Seq<char>, k: int, j: int)
    requires
        0 <= k < j <= s.len(),
        s[k] == '/',
        forall|i: int| 0 <= i < k ==> s[i] != '/',
    ensures
        split_components(s.take(j)).len() >= 2,
        split_components(s.take(j))[0] == s.take(k),
    decreases j - k,
{
    let t = s.take(j);
    assert(t.drop_last() =~= s.take(j - 1));
    assert(t.last() == s[j - 1]);
    if j == k + 1 {
        lemma_split_free(s.take(k));
        assert(s.take(j - 1) =~= s.take(k));
    } else {
        lemma_split_head_take(s, k, j - 1);
    }
}

/// The first component of a path whose first `/` is at `k` is `s[..k]`.
pub(crate) proof fn lemma_split_head(s: Seq<char>, k: int)
    requires
        0 <= k < s.len(),
        s[k] == '/',
        forall|i: int| 0 <= i < k ==> s[i] != '/',
    ensures
        split_components(s)[0] == s.take(k),
{
    lemma_split_head_take(s, k, s.len() as int);
    assert(s.take(s.len() as int) =~= s);
}

} // verus!

/// Return the first `/`-separated component of `path`, exactly as
/// `path.split('/').next()` yields it.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r@ == first_component_spec(path@),
))]
pub(crate) fn first_component(path: &str) -> String {
    let mut component = String::new();
    let mut chars = path.chars();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= path@.len(),
            chars.remaining() == path@.skip(index),
            component@ == path@.take(index),
            forall|k: int| 0 <= k < index ==> path@[k] != '/',
        decreases path@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                if ch == '/' {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        lemma_split_head(path@, index);
                    }
                    return component;
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(path@.take(index + 1) =~= path@.take(index).push(ch));
                }
                component.push(ch);
                #[cfg(verus_keep_ghost)]
                proof! {
                    index = index + 1;
                }
            }
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(path@.take(index) =~= path@);
                    lemma_split_free(path@);
                }
                return component;
            }
        }
    }
}

/// Return whether `comment` contains a scalar that is not Unicode
/// `White_Space`, which is exactly when `comment.trim()` is nonempty.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == is_meaningful_comment(comment@),
))]
pub(crate) fn has_meaningful_comment(comment: &str) -> bool {
    let mut chars = comment.chars();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut index: int = 0int;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= comment@.len(),
            chars.remaining() == comment@.skip(index),
            forall|k: int| 0 <= k < index ==> is_white_space(#[trigger] comment@[k]),
        decreases comment@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                if !ch.is_whitespace() {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(!is_white_space(comment@[index]));
                    }
                    return true;
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    index = index + 1;
                }
            }
            None => return false,
        }
    }
}

/// Decide whether a placeholder is satisfied: some snapshot entry is
/// unmentioned by its siblings, or its comment is meaningful.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == placeholder_accepted(has_unmentioned_entry, comment_view(comment)),
))]
pub(crate) fn placeholder_satisfied(has_unmentioned_entry: bool, comment: Option<&str>) -> bool {
    if has_unmentioned_entry {
        return true;
    }
    match comment {
        Some(text) => has_meaningful_comment(text),
        None => false,
    }
}

/// Decide one exact-snapshot resolution step for an entry with
/// `classification`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == resolution_step_spec(is_last, classification),
))]
pub(crate) fn resolution_step(
    is_last: bool,
    classification: EntryClassification,
) -> ResolutionStep {
    if is_last {
        return ResolutionStep::Resolved {
            require_containment: classification.is_ok(),
        };
    }
    match classification {
        Ok(SupportedEntryKind::Directory) => ResolutionStep::Descend,
        _ => ResolutionStep::TypeMismatch,
    }
}

#[cfg(verus_keep_ghost)]
verus! {

/// A placeholder is accepted exactly when some snapshot entry is unmentioned
/// or its comment has a scalar that is not white space; with every entry
/// mentioned, an absent, empty, or all-white-space comment is rejected.
pub(crate) proof fn theorem_placeholder_acceptance(
    has_unmentioned_entry: bool,
    comment: Option<Seq<char>>,
)
    ensures
        placeholder_accepted(has_unmentioned_entry, comment) == (has_unmentioned_entry || (
        comment is Some && exists|i: int|
            0 <= i < comment->0.len() && !is_white_space(#[trigger] comment->0[i]))),
        !has_unmentioned_entry && comment is None ==> !placeholder_accepted(
            has_unmentioned_entry,
            comment,
        ),
        !has_unmentioned_entry && comment is Some && (forall|i: int|
            0 <= i < comment->0.len() ==> is_white_space(#[trigger] comment->0[i]))
            ==> !placeholder_accepted(has_unmentioned_entry, comment),
        has_unmentioned_entry ==> placeholder_accepted(has_unmentioned_entry, comment),
{
}

/// The first component is the longest `/`-free prefix of the path, and the
/// path is either exactly it or continues with `/`.
pub(crate) proof fn theorem_first_component_shape(path: Seq<char>)
    ensures
        first_component_spec(path).len() <= path.len(),
        first_component_spec(path) == path.take(first_component_spec(path).len() as int),
        forall|i: int|
            0 <= i < first_component_spec(path).len() ==> #[trigger] first_component_spec(path)[i]
                != '/',
        first_component_spec(path).len() == path.len() || path[first_component_spec(path).len() as int]
            == '/',
{
    crate::proven::windows_names::lemma_first_index_of_bounds(path, '/');
    let k = first_index_of(path, '/');
    if k < path.len() {
        lemma_split_head(path, k);
    } else {
        assert(path.take(k) =~= path);
        lemma_split_free(path);
    }
    assert(first_component_spec(path) =~= path.take(k));
}

/// Resolution descends only through a non-final real directory, reports a
/// type mismatch for every other non-final entry, and requires containment
/// exactly for a final entry of a supported type.
pub(crate) proof fn theorem_resolution_descends_only_through_directories(
    is_last: bool,
    classification: EntryClassification,
)
    ensures
        (resolution_step_spec(is_last, classification) == ResolutionStep::Descend) == (!is_last
            && classification == Ok::<SupportedEntryKind, crate::entry_type::UnsupportedEntryKind>(
            SupportedEntryKind::Directory,
        )),
        (resolution_step_spec(is_last, classification) == ResolutionStep::TypeMismatch) == (!is_last
            && classification != Ok::<SupportedEntryKind, crate::entry_type::UnsupportedEntryKind>(
            SupportedEntryKind::Directory,
        )),
        (resolution_step_spec(is_last, classification) == ResolutionStep::Resolved {
            require_containment: true,
        }) == (is_last && classification is Ok),
        (resolution_step_spec(is_last, classification) == ResolutionStep::Resolved {
            require_containment: false,
        }) == (is_last && classification is Err),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_type::UnsupportedEntryKind;

    #[test]
    fn first_component_matches_split() {
        for path in [
            "",
            "/",
            "a",
            "a/",
            "/a",
            "a/b",
            "a/b/c",
            "a//b",
            "é/ü",
            "name with space/x",
            "..",
            "a\\b/c",
        ] {
            assert_eq!(
                first_component(path),
                path.split('/').next().unwrap_or_default(),
                "{path:?}"
            );
        }
    }

    #[test]
    fn meaningful_comment_matches_trim() {
        for comment in [
            "",
            " ",
            "\t\n\r",
            "\u{a0}\u{2003}\u{3000}",
            "\u{85}",
            "x",
            "  x  ",
            "\u{200b}",
            "\u{feff}",
            "\u{1680}y",
        ] {
            assert_eq!(
                has_meaningful_comment(comment),
                !comment.trim().is_empty(),
                "{comment:?}"
            );
        }
    }

    #[test]
    fn placeholder_satisfaction_covers_every_branch() {
        assert!(placeholder_satisfied(true, None));
        assert!(placeholder_satisfied(true, Some("")));
        assert!(placeholder_satisfied(false, Some("future items")));
        assert!(!placeholder_satisfied(false, Some(" \t")));
        assert!(!placeholder_satisfied(false, Some("")));
        assert!(!placeholder_satisfied(false, None));
    }

    #[test]
    fn resolution_step_covers_every_branch() {
        let directory: EntryClassification = Ok(SupportedEntryKind::Directory);
        let file: EntryClassification = Ok(SupportedEntryKind::RegularFile);
        let link: EntryClassification = Err(UnsupportedEntryKind::SymbolicLink);
        assert_eq!(
            resolution_step(true, directory),
            ResolutionStep::Resolved {
                require_containment: true
            }
        );
        assert_eq!(
            resolution_step(true, file),
            ResolutionStep::Resolved {
                require_containment: true
            }
        );
        assert_eq!(
            resolution_step(true, link),
            ResolutionStep::Resolved {
                require_containment: false
            }
        );
        assert_eq!(resolution_step(false, directory), ResolutionStep::Descend);
        assert_eq!(resolution_step(false, file), ResolutionStep::TypeMismatch);
        assert_eq!(resolution_step(false, link), ResolutionStep::TypeMismatch);
    }
}
