//! Verified tree-level guide rules: unique full paths, no entry below a
//! regular file, and placeholder siblings.
//!
//! These kernels make the decisions of `Validator::validate_syntax` that look
//! at more than one entry (`docs/v0.2-contract.md`, "Path expressions" and
//! "Placeholders and listing completeness"; contract cases
//! `path-duplicate-decoded`, `path-entry-below-file`, `placeholder-adjacent`,
//! and `placeholder-child`):
//!
//! - [`first_repeated_full_path`] receives the guide's entries in preorder,
//!   each with the index of the directory entry that owns it, and reports the
//!   first non-placeholder entry whose full path (its owner's full path, `/`,
//!   and its own decoded path) equals the full path of an earlier
//!   non-placeholder entry, together with that full path;
//! - [`first_entry_below_file`] receives the same listing, with each entry's
//!   kind, and reports the first entry whose full path continues a regular
//!   file entry's full path with `/` (an entry listed below a regular file,
//!   which no filesystem can satisfy), together with the shortest such
//!   regular-file path;
//! - [`check_placeholder_siblings`] receives the shapes of one sibling list
//!   and reports, in list order, the first placeholder that is followed by
//!   another placeholder (as the index of the second) or that owns children.
//!
//! The duplicate search builds each full path once, as the hash set it
//! replaces did, and keeps a chained hash index of the earlier entries. The
//! index's hash is a polynomial over the full path's scalars modulo the
//! Mersenne prime 2^31 - 1, computed incrementally from the owner's hash so
//! that each entry hashes only its own path, followed by a modular affine
//! mix, with keys the caller draws at random. Running time is linear in the
//! total length of the full paths plus the length of the chains walked;
//! random keys keep chains short in expectation for guide text chosen
//! before the keys are drawn. The proof does not depend on the keys or on
//! the quality of the hash: it uses only that the hash is a function of the
//! full path text, so equal texts always meet in the same chain, and two
//! entries are reported equal only after their texts compare equal.
//!
//! The theorems below state the contract-level consequences:
//!
//! - `theorem_join_identity`: for nonempty sequences of `/`-free components
//!   (every path that `path_rules` accepts splits into such components),
//!   `/`-joined text equality is component-sequence equality;
//! - `theorem_full_path_is_join`: when every listed path is nonempty, an
//!   entry's full path text is the `/` join of its decoded component
//!   sequence (its owner's components followed by its own);
//! - `theorem_duplicate_check_decides_decoded_identity`: under the same
//!   condition, the duplicate check succeeds exactly when no two
//!   non-placeholder entries decode to the same component sequence, and
//!   otherwise reports the first entry in preorder that repeats an earlier
//!   entry's component sequence; `theorem_path_rules_give_nonempty_paths`
//!   discharges the condition for entries that `path_rules` accepts;
//! - `theorem_below_file_is_component_prefix`: under the same condition, an
//!   entry lies below a regular file exactly when some regular file entry's
//!   decoded component sequence is a proper prefix of the entry's;
//! - `theorem_repeated_path_outcome_unique` and
//!   `theorem_placeholder_outcome_unique`: each specification determines its
//!   outcome, so the production glue's mapping to an error is fixed;
//! - `theorem_placeholder_rules`: a sibling list is accepted exactly when no
//!   two placeholders are adjacent and no placeholder owns children;
//! - `theorem_childless_placeholders`: when no placeholder owns children, the
//!   verdict is never the children error, and an adjacency verdict names the
//!   second placeholder of the first adjacent pair.
//!
//! What the kernels receive is computed by unverified glue in
//! `src/validator.rs`: the preorder listing of the guide tree with owner
//! indices, and each sibling's shape. A placeholder's shape never owns
//! children, because `FilesystemItem::Placeholder` has no children field;
//! the parser also rejects an indented line below a placeholder
//! (`src/proven/layout.rs`), so contract case `placeholder-child` fails
//! before validation.

#[cfg(verus_keep_ghost)]
use crate::proven::path_rules::{
    join_components, path_verdict, split_components, theorem_split_join, theorem_split_unique,
    theorem_valid_path_is_relative, PathVerdict,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

/// The modulus of the duplicate index's hash, the Mersenne prime 2^31 - 1.
/// Products of two residues fit in `u64`.
#[cfg_attr(verus_keep_ghost, verus_verify)]
const HASH_MODULUS: u64 = 0x7FFF_FFFF;

/// The end of a hash chain.
#[cfg_attr(verus_keep_ghost, verus_verify)]
const NO_ENTRY: usize = usize::MAX;

/// One guide entry in preorder.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PathEntry<'a> {
    /// The index of the directory entry that owns this entry, or `None` at
    /// the guide root. An index that does not precede the entry is treated
    /// as `None`.
    pub(crate) parent: Option<usize>,
    /// The decoded path, without the directory marker.
    pub(crate) path: &'a str,
    /// Whether the entry is a placeholder, which has no full path.
    pub(crate) placeholder: bool,
    /// Whether the entry is a directory rather than a regular file (ignored
    /// for a placeholder).
    pub(crate) directory: bool,
}

/// Keys for the duplicate index's hash. Any values give the same outcome.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct HashKeys {
    /// The polynomial's evaluation point.
    pub(crate) base: u64,
    /// The affine mix's multiplier.
    pub(crate) scale: u64,
    /// The affine mix's offset.
    pub(crate) shift: u64,
}

/// Outcome of the unique-full-path rule.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DuplicateCheck {
    /// No two non-placeholder entries share a full path.
    Unique,
    /// The entry at `index` is the first whose full path repeats an earlier
    /// entry's full path (`SyntaxError::InvalidPathFormat`).
    Duplicate {
        /// The repeating entry's preorder index.
        index: usize,
        /// The shared full path.
        full_path: String,
    },
}

/// Outcome of the rule that no entry is listed below a regular file.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParentCheck {
    /// No entry's full path continues a regular file's full path with `/`.
    Consistent,
    /// The entry at `index` is the first, in preorder, whose full path
    /// continues a regular file's full path with `/`
    /// (`SyntaxError::EntryBelowFile`).
    BelowFile {
        /// The entry's preorder index.
        index: usize,
        /// The shortest such regular file's full path.
        file_path: String,
    },
}

/// The shape of one sibling for the placeholder rules.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SiblingShape {
    /// A file or directory entry.
    Entry,
    /// A placeholder, and whether it owns at least one child.
    Placeholder {
        /// Whether the placeholder owns at least one child.
        has_children: bool,
    },
}

/// Outcome of the placeholder rules for one sibling list.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlaceholderVerdict {
    /// No two placeholders are adjacent and no placeholder owns children.
    Valid,
    /// The sibling at `index` is a placeholder directly after another
    /// placeholder (`SyntaxError::AdjacentPlaceholders`).
    Adjacent {
        /// The second placeholder's index in the sibling list.
        index: usize,
    },
    /// The placeholder at `index` owns children
    /// (`SyntaxError::PlaceholderWithChildren`).
    WithChildren {
        /// The placeholder's index in the sibling list.
        index: usize,
    },
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Full paths
// ---------------------------------------------------------------------------

/// Entry `k` exists and is not a placeholder.
pub(crate) open spec fn is_listed(entries: Seq<PathEntry>, k: int) -> bool {
    0 <= k < entries.len() && !entries[k].placeholder
}

/// The index of the entry that owns entry `k`, or `-1` at the guide root
/// (also when the recorded owner does not precede `k`).
pub(crate) open spec fn parent_index(entries: Seq<PathEntry>, k: int) -> int {
    match entries[k].parent {
        Some(p) => if (p as int) < k {
            p as int
        } else {
            -1
        },
        None => -1,
    }
}

/// `parent/path`, or `path` alone below an empty parent text.
pub(crate) open spec fn join_below(parent: Seq<char>, path: Seq<char>) -> Seq<char> {
    if parent.len() == 0 {
        path
    } else {
        parent + seq!['/'] + path
    }
}

/// The full path text of entry `k`: its owner's full path joined with its
/// own path. A placeholder (or a missing entry) has the empty text.
pub(crate) open spec fn entry_full_path(entries: Seq<PathEntry>, k: int) -> Seq<char>
    decreases k,
{
    if !is_listed(entries, k) {
        Seq::empty()
    } else {
        let p = parent_index(entries, k);
        let parent = if p >= 0 {
            entry_full_path(entries, p)
        } else {
            Seq::empty()
        };
        join_below(parent, entries[k].path@)
    }
}

/// Entry `k` is listed and its full path equals an earlier listed entry's.
pub(crate) open spec fn repeats_earlier(entries: Seq<PathEntry>, k: int) -> bool {
    is_listed(entries, k) && exists|i: int|
        0 <= i < k && is_listed(entries, i) && #[trigger] entry_full_path(entries, i)
            == entry_full_path(entries, k)
}

/// The outcome of the unique-full-path rule: success exactly when no entry
/// repeats an earlier one, and otherwise the first entry that does, with
/// its full path.
pub(crate) open spec fn repeated_path_outcome(entries: Seq<PathEntry>, r: DuplicateCheck) -> bool {
    match r {
        DuplicateCheck::Unique => forall|k: int|
            0 <= k < entries.len() ==> !#[trigger] repeats_earlier(entries, k),
        DuplicateCheck::Duplicate { index, full_path } => {
            &&& repeats_earlier(entries, index as int)
            &&& forall|k: int| 0 <= k < index ==> !#[trigger] repeats_earlier(entries, k)
            &&& full_path@ == entry_full_path(entries, index as int)
        },
    }
}

// ---------------------------------------------------------------------------
// Decoded identity
// ---------------------------------------------------------------------------

/// The decoded component sequence of entry `k`: its owner's components
/// followed by the `/`-separated components of its own path.
pub(crate) open spec fn entry_components(entries: Seq<PathEntry>, k: int) -> Seq<Seq<char>>
    decreases k,
{
    if !is_listed(entries, k) {
        Seq::empty()
    } else {
        let p = parent_index(entries, k);
        let parent = if p >= 0 {
            entry_components(entries, p)
        } else {
            Seq::empty()
        };
        parent + split_components(entries[k].path@)
    }
}

/// Entry `k` is listed and decodes to the same component sequence as an
/// earlier listed entry.
pub(crate) open spec fn repeats_earlier_decoded(entries: Seq<PathEntry>, k: int) -> bool {
    is_listed(entries, k) && exists|i: int|
        0 <= i < k && is_listed(entries, i) && #[trigger] entry_components(entries, i)
            == entry_components(entries, k)
}

/// Every listed entry has a nonempty path.
pub(crate) open spec fn nonempty_paths(entries: Seq<PathEntry>) -> bool {
    forall|k: int| #[trigger] is_listed(entries, k) ==> entries[k].path@.len() > 0
}

/// Every listed entry's path passes the structural path rules, at the guide
/// root exactly for entries without an owner.
pub(crate) open spec fn paths_pass_path_rules(entries: Seq<PathEntry>) -> bool {
    forall|k: int| #[trigger]
        is_listed(entries, k) ==> path_verdict(entries[k].path@, entries[k].parent is None)
            == PathVerdict::Valid
}

/// Joining two nonempty component sequences puts one `/` between their
/// joins.
pub(crate) proof fn lemma_join_append(a: Seq<Seq<char>>, b: Seq<Seq<char>>)
    requires
        a.len() >= 1,
        b.len() >= 1,
    ensures
        join_components(a + b) == join_components(a) + seq!['/'] + join_components(b),
    decreases b.len(),
{
    let ab = a + b;
    if b.len() == 1 {
        assert(ab.drop_last() =~= a);
        assert(ab.last() == b[0]);
    } else {
        assert(ab.drop_last() =~= a + b.drop_last());
        assert(ab.last() == b.last());
        lemma_join_append(a, b.drop_last());
        assert(join_components(ab) =~= join_components(a) + seq!['/'] + join_components(b));
    }
}

/// For nonempty sequences of `/`-free components, which is every split of a
/// path that `path_rules` accepts, `/`-joined text equality is exactly
/// component-sequence equality.
pub(crate) proof fn theorem_join_identity(a: Seq<Seq<char>>, b: Seq<Seq<char>>)
    requires
        a.len() >= 1,
        b.len() >= 1,
        forall|i: int| 0 <= i < a.len() ==> !(#[trigger] a[i]).contains('/'),
        forall|i: int| 0 <= i < b.len() ==> !(#[trigger] b[i]).contains('/'),
    ensures
        (join_components(a) == join_components(b)) == (a == b),
{
    theorem_split_unique(a);
    theorem_split_unique(b);
}

/// When every listed path is nonempty, a listed entry's full path is the
/// nonempty `/` join of its nonempty sequence of `/`-free decoded
/// components.
pub(crate) proof fn theorem_full_path_is_join(entries: Seq<PathEntry>, k: int)
    requires
        nonempty_paths(entries),
        is_listed(entries, k),
    ensures
        entry_components(entries, k).len() >= 1,
        join_components(entry_components(entries, k)) == entry_full_path(entries, k),
        forall|i: int|
            0 <= i < entry_components(entries, k).len() ==> !(#[trigger] entry_components(
                entries,
                k,
            )[i]).contains('/'),
        entry_full_path(entries, k).len() > 0,
    decreases k,
{
    let path = entries[k].path@;
    let own = split_components(path);
    theorem_split_join(path);
    assert(is_listed(entries, k));
    assert(path.len() > 0);
    let p = parent_index(entries, k);
    let comps = entry_components(entries, k);
    if p >= 0 && is_listed(entries, p) {
        theorem_full_path_is_join(entries, p);
        let pc = entry_components(entries, p);
        assert(comps == pc + own);
        lemma_join_append(pc, own);
        assert(entry_full_path(entries, k) == entry_full_path(entries, p) + seq!['/'] + path);
        assert forall|i: int| 0 <= i < comps.len() implies !(#[trigger] comps[i]).contains('/') by {
            if i < pc.len() {
                assert(comps[i] == pc[i]);
            } else {
                assert(comps[i] == own[i - pc.len()]);
            }
        }
    } else {
        if p >= 0 {
            assert(entry_full_path(entries, p) == Seq::<char>::empty());
            assert(entry_components(entries, p) == Seq::<Seq<char>>::empty());
        }
        assert(comps =~= own);
        assert(entry_full_path(entries, k) == path);
    }
}

/// When every listed path is nonempty, two listed entries have the same
/// full path text exactly when they decode to the same component sequence.
pub(crate) proof fn theorem_text_identity_is_decoded_identity(
    entries: Seq<PathEntry>,
    i: int,
    j: int,
)
    requires
        nonempty_paths(entries),
        is_listed(entries, i),
        is_listed(entries, j),
    ensures
        (entry_full_path(entries, i) == entry_full_path(entries, j)) == (entry_components(
            entries,
            i,
        ) == entry_components(entries, j)),
{
    theorem_full_path_is_join(entries, i);
    theorem_full_path_is_join(entries, j);
    theorem_join_identity(entry_components(entries, i), entry_components(entries, j));
}

/// When every listed path is nonempty, an entry repeats an earlier entry's
/// full path text exactly when it repeats an earlier entry's decoded
/// component sequence.
pub(crate) proof fn theorem_repeats_iff_decoded(entries: Seq<PathEntry>, k: int)
    requires
        nonempty_paths(entries),
    ensures
        repeats_earlier(entries, k) == repeats_earlier_decoded(entries, k),
{
    if repeats_earlier(entries, k) {
        let i = choose|i: int|
            0 <= i < k && is_listed(entries, i) && #[trigger] entry_full_path(entries, i)
                == entry_full_path(entries, k);
        theorem_text_identity_is_decoded_identity(entries, i, k);
        assert(entry_components(entries, i) == entry_components(entries, k));
    }
    if repeats_earlier_decoded(entries, k) {
        let i = choose|i: int|
            0 <= i < k && is_listed(entries, i) && #[trigger] entry_components(entries, i)
                == entry_components(entries, k);
        theorem_text_identity_is_decoded_identity(entries, i, k);
        assert(entry_full_path(entries, i) == entry_full_path(entries, k));
    }
}

/// Entries whose paths pass the structural path rules have nonempty paths.
pub(crate) proof fn theorem_path_rules_give_nonempty_paths(entries: Seq<PathEntry>)
    requires
        paths_pass_path_rules(entries),
    ensures
        nonempty_paths(entries),
{
    assert forall|k: int| #[trigger] is_listed(entries, k) implies entries[k].path@.len() > 0 by {
        theorem_valid_path_is_relative(entries[k].path@, entries[k].parent is None);
    }
}

/// Contract rule "Two entries MUST NOT decode to the same full guide path":
/// when every listed path is nonempty (in particular when every path passes
/// `path_rules`), the duplicate check succeeds exactly when no two distinct
/// non-placeholder entries decode to the same component sequence, and a
/// reported entry is the first in preorder whose decoded component sequence
/// repeats an earlier entry's.
pub(crate) proof fn theorem_duplicate_check_decides_decoded_identity(
    entries: Seq<PathEntry>,
    r: DuplicateCheck,
)
    requires
        nonempty_paths(entries),
        repeated_path_outcome(entries, r),
    ensures
        (r is Unique) == (forall|i: int, j: int|
            0 <= i < j < entries.len() && is_listed(entries, i) && is_listed(entries, j)
                ==> #[trigger] entry_components(entries, i) != #[trigger] entry_components(
                entries,
                j,
            )),
        match r {
            DuplicateCheck::Unique => true,
            DuplicateCheck::Duplicate { index, .. } => {
                &&& repeats_earlier_decoded(entries, index as int)
                &&& forall|k: int| 0 <= k < index ==> !#[trigger] repeats_earlier_decoded(entries, k)
            },
        },
{
    let distinct = forall|i: int, j: int|
        0 <= i < j < entries.len() && is_listed(entries, i) && is_listed(entries, j)
            ==> #[trigger] entry_components(entries, i) != #[trigger] entry_components(entries, j);
    match r {
        DuplicateCheck::Unique => {
            assert forall|i: int, j: int|
                0 <= i < j < entries.len() && is_listed(entries, i) && is_listed(
                    entries,
                    j,
                ) implies #[trigger] entry_components(entries, i) != #[trigger] entry_components(
                entries,
                j,
            ) by {
                theorem_repeats_iff_decoded(entries, j);
                assert(!repeats_earlier(entries, j));
            }
        },
        DuplicateCheck::Duplicate { index, .. } => {
            assert forall|k: int| 0 <= k < index implies !#[trigger] repeats_earlier_decoded(
                entries,
                k,
            ) by {
                theorem_repeats_iff_decoded(entries, k);
                assert(!repeats_earlier(entries, k));
            }
            theorem_repeats_iff_decoded(entries, index as int);
            assert(repeats_earlier_decoded(entries, index as int));
            let i = choose|i: int|
                0 <= i < index && is_listed(entries, i) && #[trigger] entry_components(entries, i)
                    == entry_components(entries, index as int);
            assert(!distinct);
        },
    }
}

/// The specification determines the outcome: two outcomes that both meet
/// it name the same entry and the same full path text.
pub(crate) proof fn theorem_repeated_path_outcome_unique(
    entries: Seq<PathEntry>,
    r1: DuplicateCheck,
    r2: DuplicateCheck,
)
    requires
        repeated_path_outcome(entries, r1),
        repeated_path_outcome(entries, r2),
    ensures
        match (r1, r2) {
            (DuplicateCheck::Unique, DuplicateCheck::Unique) => true,
            (
                DuplicateCheck::Duplicate { index: a, full_path: p },
                DuplicateCheck::Duplicate { index: b, full_path: q },
            ) => a == b && p@ == q@,
            _ => false,
        },
{
    match (r1, r2) {
        (DuplicateCheck::Duplicate { index: a, .. }, DuplicateCheck::Duplicate { index: b, .. }) => {
            if a < b {
                assert(!repeats_earlier(entries, a as int));
            } else if b < a {
                assert(!repeats_earlier(entries, b as int));
            }
        },
        (DuplicateCheck::Duplicate { index: a, .. }, DuplicateCheck::Unique) => {
            assert(!repeats_earlier(entries, a as int));
        },
        (DuplicateCheck::Unique, DuplicateCheck::Duplicate { index: b, .. }) => {
            assert(!repeats_earlier(entries, b as int));
        },
        _ => {},
    }
}

// ---------------------------------------------------------------------------
// Entries below regular files
// ---------------------------------------------------------------------------

/// Entry `i` is a listed regular file.
pub(crate) open spec fn is_listed_file(entries: Seq<PathEntry>, i: int) -> bool {
    is_listed(entries, i) && !entries[i].directory
}

/// Some listed regular file has the full path `p`.
pub(crate) open spec fn is_file_path(entries: Seq<PathEntry>, p: Seq<char>) -> bool {
    exists|i: int| is_listed_file(entries, i) && #[trigger] entry_full_path(entries, i) == p
}

/// `q[j]` is a `/` and `q[..j]` is a listed regular file's full path.
pub(crate) open spec fn file_prefix_at(entries: Seq<PathEntry>, q: Seq<char>, j: int) -> bool {
    0 <= j < q.len() && q[j] == '/' && is_file_path(entries, q.take(j))
}

/// Entry `k` is listed and its full path continues a listed regular file's
/// full path with `/`: the guide places it below a regular file, which no
/// filesystem can satisfy.
pub(crate) open spec fn below_file(entries: Seq<PathEntry>, k: int) -> bool {
    is_listed(entries, k) && exists|j: int|
        #[trigger] file_prefix_at(entries, entry_full_path(entries, k), j)
}

/// The outcome of the below-a-file rule: success exactly when no entry lies
/// below a regular file, and otherwise the first entry that does, with the
/// shortest regular-file path it continues.
pub(crate) open spec fn below_file_outcome(entries: Seq<PathEntry>, r: ParentCheck) -> bool {
    match r {
        ParentCheck::Consistent => forall|k: int|
            0 <= k < entries.len() ==> !#[trigger] below_file(entries, k),
        ParentCheck::BelowFile { index, file_path } => {
            &&& below_file(entries, index as int)
            &&& forall|k: int| 0 <= k < index ==> !#[trigger] below_file(entries, k)
            &&& exists|j: int|
                {
                    &&& #[trigger] file_prefix_at(entries, entry_full_path(entries, index as int), j)
                    &&& file_path@ == entry_full_path(entries, index as int).take(j)
                    &&& forall|m: int|
                        0 <= m < j ==> !#[trigger] file_prefix_at(
                            entries,
                            entry_full_path(entries, index as int),
                            m,
                        )
                }
        },
    }
}

/// The components before the `m`th are joined, and a `/` follows them, at
/// the start of the whole join.
proof fn lemma_join_prefix(c: Seq<Seq<char>>, m: int)
    requires
        1 <= m < c.len(),
    ensures
        join_components(c.take(m)).len() < join_components(c).len(),
        join_components(c)[join_components(c.take(m)).len() as int] == '/',
        join_components(c).take(join_components(c.take(m)).len() as int) == join_components(
            c.take(m),
        ),
{
    let a = c.take(m);
    let b = c.skip(m);
    assert(c =~= a + b);
    lemma_join_append(a, b);
    let ja = join_components(a);
    assert(join_components(c).take(ja.len() as int) =~= ja);
}

/// For `/`-free components, every `/` in their join ends the join of a
/// nonempty proper prefix of the components.
proof fn lemma_slash_ends_prefix(c: Seq<Seq<char>>, j: int)
    requires
        c.len() >= 1,
        forall|i: int| 0 <= i < c.len() ==> !(#[trigger] c[i]).contains('/'),
        0 <= j < join_components(c).len(),
        join_components(c)[j] == '/',
    ensures
        exists|m: int|
            1 <= m < c.len() && join_components(c).take(j) == #[trigger] join_components(c.take(m)),
    decreases c.len(),
{
    let q = join_components(c);
    if c.len() == 1 {
        assert(q == c[0]);
        assert(c[0].contains('/'));
    } else {
        let init = c.drop_last();
        let last = c.last();
        let ji = join_components(init);
        assert(q == ji + seq!['/'] + last);
        if j < ji.len() {
            assert(q[j] == ji[j]);
            assert forall|i: int| 0 <= i < init.len() implies !(#[trigger] init[i]).contains('/') by {
                assert(init[i] == c[i]);
            }
            lemma_slash_ends_prefix(init, j);
            let m = choose|m: int|
                1 <= m < init.len() && ji.take(j) == #[trigger] join_components(init.take(m));
            assert(init.take(m) =~= c.take(m));
            assert(q.take(j) =~= ji.take(j));
        } else if j == ji.len() {
            assert(init =~= c.take(c.len() - 1));
            assert(q.take(j) =~= ji);
        } else {
            assert(q[j] == last[j - ji.len() - 1]);
            assert(!c[c.len() - 1].contains('/'));
            assert(last.contains('/'));
        }
    }
}

/// Contract rule "no entry below a regular file": when every listed path is
/// nonempty (in particular when every path passes `path_rules`), an entry
/// lies below a regular file exactly when some listed regular file's
/// decoded component sequence is a proper prefix of the entry's, whatever
/// hierarchy spelling either uses.
pub(crate) proof fn theorem_below_file_is_component_prefix(entries: Seq<PathEntry>, k: int)
    requires
        nonempty_paths(entries),
        is_listed(entries, k),
    ensures
        below_file(entries, k) == exists|i: int|
            {
                &&& is_listed_file(entries, i)
                &&& entry_components(entries, i).len() < entry_components(entries, k).len()
                &&& entry_components(entries, k).take(entry_components(entries, i).len() as int)
                    == #[trigger] entry_components(entries, i)
            },
{
    let b = entry_components(entries, k);
    let q = entry_full_path(entries, k);
    theorem_full_path_is_join(entries, k);
    let prefixed = exists|i: int|
        {
            &&& is_listed_file(entries, i)
            &&& entry_components(entries, i).len() < b.len()
            &&& b.take(entry_components(entries, i).len() as int) == #[trigger] entry_components(
                entries,
                i,
            )
        };
    if below_file(entries, k) {
        let j = choose|j: int| #[trigger] file_prefix_at(entries, q, j);
        let i = choose|i: int|
            is_listed_file(entries, i) && #[trigger] entry_full_path(entries, i) == q.take(j);
        lemma_slash_ends_prefix(b, j);
        let m = choose|m: int| 1 <= m < b.len() && q.take(j) == #[trigger] join_components(b.take(m));
        theorem_full_path_is_join(entries, i);
        let a = entry_components(entries, i);
        assert forall|x: int| 0 <= x < b.take(m).len() implies !(#[trigger] b.take(m)[x]).contains(
            '/',
        ) by {
            assert(b.take(m)[x] == b[x]);
        }
        theorem_join_identity(a, b.take(m));
        assert(a == b.take(m));
        assert(prefixed);
    }
    if prefixed {
        let i = choose|i: int|
            {
                &&& is_listed_file(entries, i)
                &&& entry_components(entries, i).len() < b.len()
                &&& b.take(entry_components(entries, i).len() as int) == #[trigger] entry_components(
                    entries,
                    i,
                )
            };
        let a = entry_components(entries, i);
        theorem_full_path_is_join(entries, i);
        let m = a.len() as int;
        lemma_join_prefix(b, m);
        let j = join_components(b.take(m)).len() as int;
        assert(entry_full_path(entries, i) == q.take(j));
        assert(file_prefix_at(entries, q, j));
    }
}

// ---------------------------------------------------------------------------
// The duplicate index
// ---------------------------------------------------------------------------

spec fn hash_modulus() -> int {
    0x7FFF_FFFF
}

/// The polynomial hash at `base`, modulo 2^31 - 1, of `s` appended to text
/// whose hash is `start`; every scalar is shifted by one so no scalar
/// hashes like an absent one.
spec fn hash_from(base: int, start: int, s: Seq<char>) -> int
    decreases s.len(),
{
    if s.len() == 0 {
        start
    } else {
        (hash_from(base, start, s.drop_last()) * base + (s.last() as u32) as int + 1)
            % hash_modulus()
    }
}

/// The polynomial hash of `s` at `base`.
spec fn poly_hash(base: int, s: Seq<char>) -> int {
    hash_from(base, 0, s)
}

/// One scalar's hashing step.
proof fn lemma_hash_one(base: int, start: int, ch: char)
    ensures
        hash_from(base, start, seq![ch]) == (start * base + (ch as u32) as int + 1)
            % hash_modulus(),
{
    assert(seq![ch].drop_last() =~= Seq::<char>::empty());
    assert(seq![ch].last() == ch);
    assert(hash_from(base, start, Seq::<char>::empty()) == start);
}

/// Hashing is incremental: the hash of `a + b` continues from the hash of
/// `a`.
proof fn lemma_hash_concat(base: int, start: int, a: Seq<char>, b: Seq<char>)
    ensures
        hash_from(base, hash_from(base, start, a), b) == hash_from(base, start, a + b),
    decreases b.len(),
{
    if b.len() == 0 {
        assert(a + b =~= a);
    } else {
        lemma_hash_concat(base, start, a, b.drop_last());
        assert((a + b).drop_last() =~= a + b.drop_last());
        assert((a + b).last() == b.last());
    }
}

/// The chain that a hash value selects.
spec fn bucket_of(hash: int, scale: int, shift: int, buckets: int) -> int {
    ((scale * hash + shift) % hash_modulus()) % buckets
}

/// The latest index below `k` whose bucket is `b`, or `-1`.
spec fn prev_in(bs: Seq<int>, k: int, b: int) -> int
    decreases k,
{
    if k <= 0 {
        -1
    } else if bs[k - 1] == b {
        k - 1
    } else {
        prev_in(bs, k - 1, b)
    }
}

/// A chain link: `NO_ENTRY` for `-1`.
spec fn link(x: int) -> usize {
    if x < 0 {
        usize::MAX
    } else {
        x as usize
    }
}

proof fn lemma_prev_in(bs: Seq<int>, k: int, b: int)
    requires
        0 <= k <= bs.len(),
    ensures
        -1 <= prev_in(bs, k, b) < k,
        prev_in(bs, k, b) >= 0 ==> bs[prev_in(bs, k, b)] == b,
        forall|j: int| prev_in(bs, k, b) < j < k ==> bs[j] != b,
    decreases k,
{
    if k > 0 && bs[k - 1] != b {
        lemma_prev_in(bs, k - 1, b);
    }
}

proof fn lemma_prev_in_push(bs: Seq<int>, v: int, k: int, b: int)
    requires
        0 <= k <= bs.len(),
    ensures
        prev_in(bs.push(v), k, b) == prev_in(bs, k, b),
    decreases k,
{
    if k > 0 {
        assert(bs.push(v)[k - 1] == bs[k - 1]);
        lemma_prev_in_push(bs, v, k - 1, b);
    }
}

} // verus!

/// One hashing step: the hash of text whose hash is `hash`, extended by `ch`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        hash < HASH_MODULUS,
        base < HASH_MODULUS,
    ensures
        r < HASH_MODULUS,
        r as int == hash_from(base as int, hash as int, seq![ch]),
))]
fn hash_step(hash: u64, ch: char, base: u64) -> u64 {
    #[cfg(verus_keep_ghost)]
    proof! {
        lemma_hash_one(base as int, hash as int, ch);
        assert((hash as int) * (base as int) < 0x7FFF_FFFFint * 0x7FFF_FFFFint) by (nonlinear_arith)
            requires
                hash < 0x7FFF_FFFFu64,
                base < 0x7FFF_FFFFu64,
        ;
    }
    (hash * base + (ch as u32 as u64) + 1) % HASH_MODULUS
}

/// The hash of text whose hash is `start`, extended by `text`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        start < HASH_MODULUS,
        base < HASH_MODULUS,
    ensures
        r < HASH_MODULUS,
        r as int == hash_from(base as int, start as int, text@),
))]
fn hash_extend(start: u64, text: &str, base: u64) -> u64 {
    let mut hash = start;
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
            base < HASH_MODULUS,
            chars.remaining() == text@.skip(index),
            hash < HASH_MODULUS,
            hash as int == hash_from(base as int, start as int, text@.take(index)),
        decreases text@.len() - index
    ))]
    loop {
        match chars.next() {
            Some(ch) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(text@.take(index + 1).drop_last() =~= text@.take(index));
                    assert(text@.take(index + 1).last() == ch);
                    lemma_hash_one(base as int, hash as int, ch);
                }
                hash = hash_step(hash, ch, base);
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
                return hash;
            }
        }
    }
}

/// The chain that `hash` selects among `buckets` chains.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        hash < HASH_MODULUS,
        scale < HASH_MODULUS,
        shift < HASH_MODULUS,
        buckets > 0,
    ensures
        r < buckets,
        r as int == bucket_of(hash as int, scale as int, shift as int, buckets as int),
))]
fn bucket_index(hash: u64, scale: u64, shift: u64, buckets: usize) -> usize {
    #[cfg(verus_keep_ghost)]
    proof! {
        assert((scale as int) * (hash as int) < 0x7FFF_FFFFint * 0x7FFF_FFFFint) by (nonlinear_arith)
            requires
                hash < 0x7FFF_FFFFu64,
                scale < 0x7FFF_FFFFu64,
        ;
    }
    let mixed = (scale * hash + shift) % HASH_MODULUS;
    (mixed as usize) % buckets
}

/// Find the first non-placeholder entry, in preorder, whose full path
/// repeats an earlier non-placeholder entry's full path.
///
/// An entry's full path is its own path below a guide-root entry, and
/// otherwise its owner's full path, `/`, and its own path (an owner whose
/// full path is empty contributes nothing). `keys` only affects running
/// time.
#[cfg_attr(verus_keep_ghost, verifier::loop_isolation(false))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        repeated_path_outcome(entries@, r),
))]
pub(crate) fn first_repeated_full_path(
    entries: &[PathEntry<'_>],
    keys: HashKeys,
) -> DuplicateCheck {
    let count = entries.len();
    if count == 0 {
        return DuplicateCheck::Unique;
    }
    let base = keys.base % HASH_MODULUS;
    let scale = keys.scale % (HASH_MODULUS - 1) + 1;
    let shift = keys.shift % HASH_MODULUS;

    // `full[i]` is entry `i`'s full path and `hashes[i]` its hash; chain
    // `b` starts at `head[b]` and continues through `next`, visiting the
    // earlier entries in bucket `b` from the latest to the earliest.
    let mut full: Vec<String> = Vec::with_capacity(count);
    let mut hashes: Vec<u64> = Vec::with_capacity(count);
    let mut next: Vec<usize> = Vec::with_capacity(count);
    let mut head: Vec<usize> = vec![NO_ENTRY; count];
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut bs: Seq<int> = Seq::<int>::empty();
    }
    let mut k: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            count == entries@.len(),
            count > 0,
            0 <= k <= count,
            base < HASH_MODULUS,
            scale < HASH_MODULUS,
            shift < HASH_MODULUS,
            full@.len() == k,
            hashes@.len() == k,
            next@.len() == k,
            bs.len() == k,
            head@.len() == count,
            forall|i: int| 0 <= i < k ==> (#[trigger] full@[i])@ == entry_full_path(entries@, i),
            forall|i: int|
                0 <= i < k && is_listed(entries@, i) ==> #[trigger] hashes@[i] as int
                    == poly_hash(base as int, entry_full_path(entries@, i)),
            forall|i: int|
                0 <= i < k && is_listed(entries@, i) ==> #[trigger] bs[i] == bucket_of(
                    hashes@[i] as int,
                    scale as int,
                    shift as int,
                    count as int,
                ),
            forall|i: int| 0 <= i < k && !is_listed(entries@, i) ==> #[trigger] bs[i] == -1,
            forall|i: int|
                0 <= i < k && is_listed(entries@, i) ==> #[trigger] next@[i] == link(
                    prev_in(bs, i, bs[i]),
                ),
            forall|b: int| 0 <= b < count ==> #[trigger] head@[b] == link(prev_in(bs, k as int, b)),
            forall|j: int| 0 <= j < k ==> !#[trigger] repeats_earlier(entries@, j),
        decreases count - k
    ))]
    while k < count {
        let entry = &entries[k];
        if entry.placeholder {
            full.push(String::new());
            hashes.push(0);
            next.push(NO_ENTRY);
            #[cfg(verus_keep_ghost)]
            proof! {
                let old_bs = bs;
                bs = bs.push(-1);
                assert forall|i: int|
                    0 <= i < k && is_listed(entries@, i) implies #[trigger] next@[i] == link(
                    prev_in(bs, i, bs[i]),
                ) by {
                    lemma_prev_in_push(old_bs, -1, i, old_bs[i]);
                }
                assert forall|b: int| 0 <= b < count implies #[trigger] head@[b] == link(
                    prev_in(bs, k + 1, b),
                ) by {
                    lemma_prev_in_push(old_bs, -1, k as int, b);
                }
                assert(!repeats_earlier(entries@, k as int));
            }
        } else {
            // Below a nonempty owner path, the hash continues from the
            // owner's hash, so each entry hashes only its own path.
            let mut text = String::new();
            let mut hash: u64 = 0;
            let mut prefixed = false;
            if let Some(parent) = entry.parent {
                if parent < k {
                    let parent_text = &full[parent];
                    if !parent_text.is_empty() {
                        text.push_str(parent_text.as_str());
                        text.push('/');
                        prefixed = true;
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            assert(is_listed(entries@, parent as int));
                        }
                        hash = hash_step(hashes[parent], '/', base);
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            let owner = entry_full_path(entries@, parent as int);
                            lemma_hash_concat(base as int, 0, owner, seq!['/']);
                            assert(text@ =~= owner + seq!['/']);
                            assert(hash as int == poly_hash(base as int, text@));
                        }
                    }
                }
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                if text@.len() == 0 {
                    assert(text@ =~= Seq::<char>::empty());
                    assert(hash == 0);
                }
                assert(hash as int == poly_hash(base as int, text@));
            }
            let prefix_hash = hash;
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost prefix: Seq<char> = text@;
            }
            text.push_str(entry.path);
            let hash = hash_extend(prefix_hash, entry.path, base);
            #[cfg(verus_keep_ghost)]
            proof! {
                lemma_hash_concat(base as int, 0, prefix, entry.path@);
                assert(text@ == entry_full_path(entries@, k as int));
            }
            let bucket = bucket_index(hash, scale, shift, count);

            let mut candidate = head[bucket];
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost mut at: int = prev_in(bs, k as int, bucket as int);
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                lemma_prev_in(bs, k as int, bucket as int);
            }
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    candidate == link(at),
                    -1 <= at < k,
                    at >= 0 ==> bs[at] == bucket as int,
                    forall|j: int|
                        at < j < k && #[trigger] bs[j] == bucket as int ==> entry_full_path(
                            entries@,
                            j,
                        ) != text@,
                decreases at + 1
            ))]
            while candidate != NO_ENTRY {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(is_listed(entries@, at)) by {
                        if !is_listed(entries@, at) {
                            assert(bs[at] == -1);
                        }
                    }
                }
                if hashes[candidate] == hash && full[candidate] == text {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(entry_full_path(entries@, at) == entry_full_path(entries@, k as int));
                        assert(repeats_earlier(entries@, k as int));
                    }
                    return DuplicateCheck::Duplicate {
                        index: k,
                        full_path: text,
                    };
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_prev_in(bs, at, bucket as int);
                    assert(entry_full_path(entries@, at) != text@);
                    at = prev_in(bs, at, bucket as int);
                }
                candidate = next[candidate];
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(!repeats_earlier(entries@, k as int)) by {
                    if repeats_earlier(entries@, k as int) {
                        let i = choose|i: int|
                            0 <= i < k && is_listed(entries@, i) && #[trigger] entry_full_path(
                                entries@,
                                i,
                            ) == entry_full_path(entries@, k as int);
                        assert(bs[i] == bucket as int);
                    }
                }
            }
            next.push(head[bucket]);
            head[bucket] = k;
            // Appending to the owner's text grew the buffer geometrically;
            // keep an exact-size copy instead, as the previous hash set did.
            if prefixed {
                full.push(text.clone());
            } else {
                full.push(text);
            }
            hashes.push(hash);
            #[cfg(verus_keep_ghost)]
            proof! {
                let old_bs = bs;
                bs = bs.push(bucket as int);
                assert forall|i: int|
                    0 <= i < k + 1 && is_listed(entries@, i) implies #[trigger] next@[i] == link(
                    prev_in(bs, i, bs[i]),
                ) by {
                    if i < k {
                        lemma_prev_in_push(old_bs, bucket as int, i, old_bs[i]);
                    } else {
                        lemma_prev_in_push(old_bs, bucket as int, k as int, bucket as int);
                    }
                }
                assert forall|b: int| 0 <= b < count implies #[trigger] head@[b] == link(
                    prev_in(bs, k + 1, b),
                ) by {
                    lemma_prev_in_push(old_bs, bucket as int, k as int, b);
                }
            }
        }
        k += 1;
    }
    DuplicateCheck::Unique
}

/// The full path text of entry `k`, given the full paths of the entries
/// before it: its owner's full path, `/`, and its own path, or its own path
/// alone at the guide root or below an owner whose full path is empty; a
/// placeholder's is empty.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    requires
        k < entries@.len(),
        full@.len() == k,
        forall|i: int| 0 <= i < k ==> (#[trigger] full@[i])@ == entry_full_path(entries@, i),
    ensures
        r@ == entry_full_path(entries@, k as int),
))]
fn full_path_of(entries: &[PathEntry<'_>], full: &[String], k: usize) -> String {
    let entry = &entries[k];
    let mut text = String::new();
    if entry.placeholder {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(text@ =~= entry_full_path(entries@, k as int));
        }
        return text;
    }
    if let Some(parent) = entry.parent {
        if parent < k {
            let parent_text = &full[parent];
            if !parent_text.is_empty() {
                text.push_str(parent_text.as_str());
                text.push('/');
            }
        }
    }
    text.push_str(entry.path);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(text@ =~= entry_full_path(entries@, k as int));
    }
    text
}

/// Find the first entry, in preorder, whose full path continues a regular
/// file entry's full path with `/`: an entry listed below a regular file.
///
/// Full paths are built as in [`first_repeated_full_path`]. The regular
/// files' full paths go into a chained hash index; then every `/`-ended
/// prefix of every entry's full path is looked up, shortest first. `keys`
/// only affects running time, which is linear in the total length of the
/// full paths plus the length of the chains walked.
// Verus checks iterator loops written as `loop` over `match next()`.
#[allow(clippy::while_let_loop)]
#[cfg_attr(verus_keep_ghost, verifier::loop_isolation(false))]
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        below_file_outcome(entries@, r),
))]
pub(crate) fn first_entry_below_file(entries: &[PathEntry<'_>], keys: HashKeys) -> ParentCheck {
    let count = entries.len();
    if count == 0 {
        return ParentCheck::Consistent;
    }
    let base = keys.base % HASH_MODULUS;
    let scale = keys.scale % (HASH_MODULUS - 1) + 1;
    let shift = keys.shift % HASH_MODULUS;

    // `full[i]` is entry `i`'s full path. For a regular file, `hashes[i]` is
    // its hash, and chain `b` starts at `head[b]` and continues through
    // `next`, visiting the files in bucket `b` from the latest to the
    // earliest.
    let mut full: Vec<String> = Vec::with_capacity(count);
    let mut hashes: Vec<u64> = Vec::with_capacity(count);
    let mut next: Vec<usize> = Vec::with_capacity(count);
    let mut head: Vec<usize> = vec![NO_ENTRY; count];
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut bs: Seq<int> = Seq::<int>::empty();
    }
    let mut k: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            count == entries@.len(),
            count > 0,
            0 <= k <= count,
            base < HASH_MODULUS,
            scale < HASH_MODULUS,
            shift < HASH_MODULUS,
            full@.len() == k,
            hashes@.len() == k,
            next@.len() == k,
            bs.len() == k,
            head@.len() == count,
            forall|i: int| 0 <= i < k ==> (#[trigger] full@[i])@ == entry_full_path(entries@, i),
            forall|i: int|
                0 <= i < k && is_listed_file(entries@, i) ==> #[trigger] hashes@[i] as int
                    == poly_hash(base as int, entry_full_path(entries@, i)),
            forall|i: int|
                0 <= i < k && is_listed_file(entries@, i) ==> #[trigger] bs[i] == bucket_of(
                    hashes@[i] as int,
                    scale as int,
                    shift as int,
                    count as int,
                ),
            forall|i: int| 0 <= i < k && !is_listed_file(entries@, i) ==> #[trigger] bs[i] == -1,
            forall|i: int|
                0 <= i < k && is_listed_file(entries@, i) ==> #[trigger] next@[i] == link(
                    prev_in(bs, i, bs[i]),
                ),
            forall|b: int| 0 <= b < count ==> #[trigger] head@[b] == link(prev_in(bs, k as int, b)),
        decreases count - k
    ))]
    while k < count {
        let text = full_path_of(entries, full.as_slice(), k);
        let entry = &entries[k];
        if !entry.placeholder && !entry.directory {
            let hash = hash_extend(0, text.as_str(), base);
            let bucket = bucket_index(hash, scale, shift, count);
            next.push(head[bucket]);
            head[bucket] = k;
            hashes.push(hash);
            #[cfg(verus_keep_ghost)]
            proof! {
                let old_bs = bs;
                bs = bs.push(bucket as int);
                assert forall|i: int|
                    0 <= i < k + 1 && is_listed_file(entries@, i) implies #[trigger] next@[i]
                    == link(prev_in(bs, i, bs[i])) by {
                    if i < k {
                        lemma_prev_in_push(old_bs, bucket as int, i, old_bs[i]);
                    } else {
                        lemma_prev_in_push(old_bs, bucket as int, k as int, bucket as int);
                    }
                }
                assert forall|b: int| 0 <= b < count implies #[trigger] head@[b] == link(
                    prev_in(bs, k + 1, b),
                ) by {
                    lemma_prev_in_push(old_bs, bucket as int, k as int, b);
                }
            }
        } else {
            next.push(NO_ENTRY);
            hashes.push(0);
            #[cfg(verus_keep_ghost)]
            proof! {
                let old_bs = bs;
                bs = bs.push(-1);
                assert forall|i: int|
                    0 <= i < k && is_listed_file(entries@, i) implies #[trigger] next@[i] == link(
                    prev_in(bs, i, bs[i]),
                ) by {
                    lemma_prev_in_push(old_bs, -1, i, old_bs[i]);
                }
                assert forall|b: int| 0 <= b < count implies #[trigger] head@[b] == link(
                    prev_in(bs, k + 1, b),
                ) by {
                    lemma_prev_in_push(old_bs, -1, k as int, b);
                }
            }
        }
        full.push(text);
        k += 1;
    }

    let mut k: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            count == entries@.len(),
            0 <= k <= count,
            full@.len() == count,
            hashes@.len() == count,
            next@.len() == count,
            bs.len() == count,
            head@.len() == count,
            forall|j: int| 0 <= j < k ==> !#[trigger] below_file(entries@, j),
        decreases count - k
    ))]
    while k < count {
        if !entries[k].placeholder {
            let text = &full[k];
            let mut chars = text.as_str().chars();
            let mut prefix = String::new();
            let mut hash: u64 = 0;
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost mut index: int = 0int;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(text@.take(0) =~= Seq::<char>::empty());
                assert(prefix@ =~= Seq::<char>::empty());
            }
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    0 <= index <= text@.len(),
                    text@ == entry_full_path(entries@, k as int),
                    chars.remaining() == text@.skip(index),
                    prefix@ == text@.take(index),
                    hash < HASH_MODULUS,
                    hash as int == poly_hash(base as int, prefix@),
                    forall|m: int| 0 <= m < index ==> !#[trigger] file_prefix_at(entries@, text@, m),
                decreases text@.len() - index
            ))]
            loop {
                match chars.next() {
                    Some(ch) => {
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            assert(text@.skip(index)[0] == text@[index]);
                        }
                        if ch == '/' {
                            let bucket = bucket_index(hash, scale, shift, count);
                            let mut candidate = head[bucket];
                            #[cfg(verus_keep_ghost)]
                            proof_decl! {
                                let ghost mut at: int = prev_in(bs, count as int, bucket as int);
                            }
                            #[cfg(verus_keep_ghost)]
                            proof! {
                                lemma_prev_in(bs, count as int, bucket as int);
                            }
                            #[cfg_attr(verus_keep_ghost, verus_spec(
                                invariant
                                    candidate == link(at),
                                    -1 <= at < count,
                                    at >= 0 ==> bs[at] == bucket as int,
                                    forall|j: int|
                                        at < j < count && #[trigger] bs[j] == bucket as int
                                            ==> entry_full_path(entries@, j) != prefix@,
                                decreases at + 1
                            ))]
                            while candidate != NO_ENTRY {
                                #[cfg(verus_keep_ghost)]
                                proof! {
                                    assert(is_listed_file(entries@, at)) by {
                                        if !is_listed_file(entries@, at) {
                                            assert(bs[at] == -1);
                                        }
                                    }
                                }
                                if hashes[candidate] == hash && full[candidate] == prefix {
                                    #[cfg(verus_keep_ghost)]
                                    proof! {
                                        assert(entry_full_path(entries@, at) == prefix@);
                                        assert(is_file_path(entries@, text@.take(index)));
                                        assert(file_prefix_at(entries@, text@, index));
                                        assert(below_file(entries@, k as int));
                                    }
                                    return ParentCheck::BelowFile {
                                        index: k,
                                        file_path: prefix,
                                    };
                                }
                                #[cfg(verus_keep_ghost)]
                                proof! {
                                    lemma_prev_in(bs, at, bucket as int);
                                    assert(entry_full_path(entries@, at) != prefix@);
                                    at = prev_in(bs, at, bucket as int);
                                }
                                candidate = next[candidate];
                            }
                            #[cfg(verus_keep_ghost)]
                            proof! {
                                assert(!is_file_path(entries@, prefix@)) by {
                                    if is_file_path(entries@, prefix@) {
                                        let i = choose|i: int|
                                            is_listed_file(entries@, i) && #[trigger] entry_full_path(
                                                entries@,
                                                i,
                                            ) == prefix@;
                                        assert(bs[i] == bucket as int);
                                    }
                                }
                                assert(!file_prefix_at(entries@, text@, index));
                            }
                        }
                        #[cfg(verus_keep_ghost)]
                        proof_decl! {
                            let ghost before: Seq<char> = prefix@;
                        }
                        prefix.push(ch);
                        hash = hash_step(hash, ch, base);
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            lemma_hash_concat(base as int, 0, before, seq![ch]);
                            assert(before + seq![ch] =~= before.push(ch));
                            assert(text@.take(index + 1) =~= text@.take(index).push(ch));
                            index = index + 1;
                        }
                    }
                    None => {
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            assert(index == text@.len());
                            assert(!below_file(entries@, k as int)) by {
                                if below_file(entries@, k as int) {
                                    let j = choose|j: int| #[trigger] file_prefix_at(
                                        entries@,
                                        entry_full_path(entries@, k as int),
                                        j,
                                    );
                                    assert(!file_prefix_at(entries@, text@, j));
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
        k += 1;
    }
    ParentCheck::Consistent
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Placeholder siblings
// ---------------------------------------------------------------------------

/// The sibling is a placeholder.
pub(crate) open spec fn is_placeholder_shape(shape: SiblingShape) -> bool {
    shape is Placeholder
}

/// Siblings `i` and `i + 1` are both placeholders.
pub(crate) open spec fn adjacent_pair(shapes: Seq<SiblingShape>, i: int) -> bool {
    0 <= i && i + 1 < shapes.len() && is_placeholder_shape(shapes[i]) && is_placeholder_shape(
        shapes[i + 1],
    )
}

/// Sibling `i` is a placeholder that owns children.
pub(crate) open spec fn childful_placeholder(shapes: Seq<SiblingShape>, i: int) -> bool {
    0 <= i < shapes.len() && shapes[i] == (SiblingShape::Placeholder { has_children: true })
}

/// Sibling `i` violates a placeholder rule: it is a placeholder followed by
/// another placeholder, or a placeholder that owns children.
pub(crate) open spec fn placeholder_offense(shapes: Seq<SiblingShape>, i: int) -> bool {
    adjacent_pair(shapes, i) || childful_placeholder(shapes, i)
}

/// The outcome of the placeholder rules: success exactly when no sibling
/// offends, and otherwise the first offending sibling decides, with
/// adjacency checked before children.
pub(crate) open spec fn placeholder_outcome(shapes: Seq<SiblingShape>, r: PlaceholderVerdict) -> bool {
    match r {
        PlaceholderVerdict::Valid => forall|i: int|
            0 <= i < shapes.len() ==> !#[trigger] placeholder_offense(shapes, i),
        PlaceholderVerdict::Adjacent { index } => {
            &&& adjacent_pair(shapes, index - 1)
            &&& forall|i: int| 0 <= i < index - 1 ==> !#[trigger] placeholder_offense(shapes, i)
        },
        PlaceholderVerdict::WithChildren { index } => {
            &&& childful_placeholder(shapes, index as int)
            &&& !adjacent_pair(shapes, index as int)
            &&& forall|i: int| 0 <= i < index ==> !#[trigger] placeholder_offense(shapes, i)
        },
    }
}

/// Contract rules "Two placeholders MUST NOT be adjacent" and "It MUST NOT
/// have children": a sibling list is accepted exactly when both hold.
pub(crate) proof fn theorem_placeholder_rules(shapes: Seq<SiblingShape>, r: PlaceholderVerdict)
    requires
        placeholder_outcome(shapes, r),
    ensures
        (r == PlaceholderVerdict::Valid) == ((forall|i: int| !#[trigger] adjacent_pair(shapes, i))
            && (forall|i: int| !#[trigger] childful_placeholder(shapes, i))),
{
    if r == PlaceholderVerdict::Valid {
        assert forall|i: int| !#[trigger] adjacent_pair(shapes, i) by {
            if 0 <= i < shapes.len() {
                assert(!placeholder_offense(shapes, i));
            }
        }
        assert forall|i: int| !#[trigger] childful_placeholder(shapes, i) by {
            if 0 <= i < shapes.len() {
                assert(!placeholder_offense(shapes, i));
            }
        }
    }
}

/// With no placeholder owning children (every parsed guide: the
/// placeholder variant has no children field), the verdict is never the
/// children error, and an adjacency verdict names the second placeholder of
/// the first adjacent pair.
pub(crate) proof fn theorem_childless_placeholders(shapes: Seq<SiblingShape>, r: PlaceholderVerdict)
    requires
        placeholder_outcome(shapes, r),
        forall|i: int|
            0 <= i < shapes.len() ==> #[trigger] shapes[i] != (SiblingShape::Placeholder {
                has_children: true,
            }),
    ensures
        !(r is WithChildren),
        r == PlaceholderVerdict::Valid ==> forall|i: int| !#[trigger] adjacent_pair(shapes, i),
        match r {
            PlaceholderVerdict::Adjacent { index } => {
                &&& 1 <= index < shapes.len()
                &&& is_placeholder_shape(shapes[index - 1])
                &&& is_placeholder_shape(shapes[index as int])
                &&& forall|i: int| 0 <= i < index - 1 ==> !#[trigger] adjacent_pair(shapes, i)
            },
            _ => true,
        },
{
    theorem_placeholder_rules(shapes, r);
    if let PlaceholderVerdict::Adjacent { index } = r {
        assert forall|i: int| 0 <= i < index - 1 implies !#[trigger] adjacent_pair(shapes, i) by {
            assert(!placeholder_offense(shapes, i));
        }
    }
}

/// The specification determines the verdict.
pub(crate) proof fn theorem_placeholder_outcome_unique(
    shapes: Seq<SiblingShape>,
    r1: PlaceholderVerdict,
    r2: PlaceholderVerdict,
)
    requires
        placeholder_outcome(shapes, r1),
        placeholder_outcome(shapes, r2),
    ensures
        r1 == r2,
{
    // Each non-valid verdict names its first offending sibling.
    let first1: int = match r1 {
        PlaceholderVerdict::Valid => shapes.len() as int,
        PlaceholderVerdict::Adjacent { index } => index - 1,
        PlaceholderVerdict::WithChildren { index } => index as int,
    };
    let first2: int = match r2 {
        PlaceholderVerdict::Valid => shapes.len() as int,
        PlaceholderVerdict::Adjacent { index } => index - 1,
        PlaceholderVerdict::WithChildren { index } => index as int,
    };
    assert(first1 < shapes.len() ==> placeholder_offense(shapes, first1));
    assert(first2 < shapes.len() ==> placeholder_offense(shapes, first2));
    if first1 < first2 {
        assert(!placeholder_offense(shapes, first1));
    } else if first2 < first1 {
        assert(!placeholder_offense(shapes, first2));
    }
}

} // verus!

/// Check the placeholder rules for one sibling list, in list order: a
/// placeholder followed by another placeholder reports the second one;
/// otherwise a placeholder that owns children reports itself.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        placeholder_outcome(shapes@, r),
))]
pub(crate) fn check_placeholder_siblings(shapes: &[SiblingShape]) -> PlaceholderVerdict {
    let count = shapes.len();
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            count == shapes@.len(),
            0 <= index <= count,
            forall|i: int| 0 <= i < index ==> !#[trigger] placeholder_offense(shapes@, i),
        decreases count - index
    ))]
    while index < count {
        if let SiblingShape::Placeholder { has_children } = shapes[index] {
            if index + 1 < count && matches!(shapes[index + 1], SiblingShape::Placeholder { .. }) {
                return PlaceholderVerdict::Adjacent { index: index + 1 };
            }
            if has_children {
                return PlaceholderVerdict::WithChildren { index };
            }
        }
        index += 1;
    }
    PlaceholderVerdict::Valid
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: HashKeys = HashKeys {
        base: 0x0123_4567_89AB_CDEF,
        scale: 0xFEDC_BA98_7654_3210,
        shift: 0x0F0F_0F0F_0F0F_0F0F,
    };

    /// A directory entry.
    fn entry(parent: Option<usize>, path: &str) -> PathEntry<'_> {
        PathEntry {
            parent,
            path,
            placeholder: false,
            directory: true,
        }
    }

    /// A regular-file entry.
    fn file(parent: Option<usize>, path: &str) -> PathEntry<'_> {
        PathEntry {
            parent,
            path,
            placeholder: false,
            directory: false,
        }
    }

    fn placeholder(parent: Option<usize>) -> PathEntry<'static> {
        PathEntry {
            parent,
            path: "...",
            placeholder: true,
            directory: false,
        }
    }

    fn below(index: usize, file_path: &str) -> ParentCheck {
        ParentCheck::BelowFile {
            index,
            file_path: file_path.to_string(),
        }
    }

    /// Every entry's full path, as the kernels define it.
    fn reference_full_paths(entries: &[PathEntry<'_>]) -> Vec<String> {
        let mut full: Vec<String> = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            if entry.placeholder {
                full.push(String::new());
                continue;
            }
            let parent = match entry.parent {
                Some(parent) if parent < index => full[parent].as_str(),
                _ => "",
            };
            full.push(if parent.is_empty() {
                entry.path.to_string()
            } else {
                format!("{parent}/{}", entry.path)
            });
        }
        full
    }

    /// The below-a-file rule by brute force: the first entry with a
    /// `/`-ended prefix equal to some regular file's full path, shortest
    /// prefix first.
    fn reference_below_file(entries: &[PathEntry<'_>]) -> ParentCheck {
        let full = reference_full_paths(entries);
        let files: Vec<&str> = entries
            .iter()
            .zip(&full)
            .filter(|(entry, _)| !entry.placeholder && !entry.directory)
            .map(|(_, path)| path.as_str())
            .collect();
        for (index, entry) in entries.iter().enumerate() {
            if entry.placeholder {
                continue;
            }
            for (at, ch) in full[index].char_indices() {
                if ch == '/' && files.contains(&&full[index][..at]) {
                    return below(index, &full[index][..at]);
                }
            }
        }
        ParentCheck::Consistent
    }

    fn duplicate(index: usize, full_path: &str) -> DuplicateCheck {
        DuplicateCheck::Duplicate {
            index,
            full_path: full_path.to_string(),
        }
    }

    /// The rule as `Validator::validate_unique_full_paths` computed it with
    /// a hash set, over the same preorder listing.
    fn reference(entries: &[PathEntry<'_>]) -> DuplicateCheck {
        let mut seen = std::collections::HashSet::new();
        let mut full: Vec<String> = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            if entry.placeholder {
                full.push(String::new());
                continue;
            }
            let parent = match entry.parent {
                Some(parent) if parent < index => full[parent].as_str(),
                _ => "",
            };
            let text = if parent.is_empty() {
                entry.path.to_string()
            } else {
                format!("{parent}/{}", entry.path)
            };
            if !seen.insert(text.clone()) {
                return DuplicateCheck::Duplicate {
                    index,
                    full_path: text,
                };
            }
            full.push(text);
        }
        DuplicateCheck::Unique
    }

    #[test]
    fn unique_full_paths_are_accepted() {
        assert_eq!(first_repeated_full_path(&[], KEYS), DuplicateCheck::Unique);
        let entries = [
            entry(None, "src"),
            entry(Some(0), "main.rs"),
            placeholder(Some(0)),
            entry(None, "src/lib.rs"),
            placeholder(None),
            entry(None, "README.md"),
        ];
        assert_eq!(
            first_repeated_full_path(&entries, KEYS),
            DuplicateCheck::Unique
        );
    }

    #[test]
    fn repeated_full_paths_report_the_first_later_entry() {
        let entries = [entry(None, "a"), entry(None, "b"), entry(None, "a")];
        assert_eq!(first_repeated_full_path(&entries, KEYS), duplicate(2, "a"));

        // Different hierarchy spellings reach the same full path.
        let entries = [
            entry(None, "src"),
            entry(Some(0), "main.rs"),
            entry(None, "src/main.rs"),
        ];
        assert_eq!(
            first_repeated_full_path(&entries, KEYS),
            duplicate(2, "src/main.rs")
        );

        // The first repeating entry in preorder wins over a later one whose
        // original appears earlier.
        let entries = [
            entry(None, "x"),
            entry(None, "y"),
            entry(None, "y"),
            entry(None, "x"),
        ];
        assert_eq!(first_repeated_full_path(&entries, KEYS), duplicate(2, "y"));
    }

    #[test]
    fn placeholders_have_no_full_path() {
        let entries = [
            placeholder(None),
            placeholder(None),
            entry(None, "..."),
            entry(None, "..."),
        ];
        assert_eq!(
            first_repeated_full_path(&entries, KEYS),
            duplicate(3, "...")
        );
        // An owner that is a placeholder or that does not precede the entry
        // contributes nothing, and neither does an empty owner path.
        let entries = [placeholder(None), entry(Some(0), "a"), entry(Some(5), "a")];
        assert_eq!(first_repeated_full_path(&entries, KEYS), duplicate(2, "a"));
        let entries = [entry(None, ""), entry(Some(0), "a"), entry(None, "a")];
        assert_eq!(first_repeated_full_path(&entries, KEYS), duplicate(2, "a"));
    }

    #[test]
    fn hash_collisions_fall_back_to_text_comparison() {
        // With every key zero, a path's hash is its last scalar plus one and
        // its chain is that hash modulo the entry count, so "xa", "ya", and
        // "a" share a hash and "f" shares their chain with another hash.
        let keys = HashKeys {
            base: 0,
            scale: 0,
            shift: 0,
        };
        assert_eq!(hash_extend(0, "xa", 0), u64::from('a') + 1);
        assert_eq!(hash_extend(0, "f", 0) % 5, (u64::from('a') + 1) % 5);
        let entries = [
            entry(None, "xa"),
            entry(None, "ya"),
            entry(None, "f"),
            entry(None, "a"),
            entry(None, "ya"),
        ];
        assert_eq!(first_repeated_full_path(&entries, keys), duplicate(4, "ya"));
        assert_eq!(
            first_repeated_full_path(&entries[..4], keys),
            DuplicateCheck::Unique
        );
    }

    #[test]
    fn duplicate_search_matches_the_hash_set_reference() {
        const NAMES: [&str; 5] = ["a", "b", "a/b", "b/a", "\u{e9}"];
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut step = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for round in 0..2000 {
            let length = (step() % 9) as usize;
            let mut entries = Vec::with_capacity(length);
            for index in 0..length {
                let roll = step();
                let parent = if index == 0 || roll % 3 == 0 {
                    None
                } else {
                    Some((roll / 3) as usize % (index + 1))
                };
                if roll % 7 == 0 {
                    entries.push(placeholder(parent));
                } else {
                    entries.push(entry(parent, NAMES[(roll / 21) as usize % NAMES.len()]));
                }
            }
            let keys = HashKeys {
                base: step(),
                scale: step(),
                shift: if round % 2 == 0 { 0 } else { step() },
            };
            assert_eq!(
                first_repeated_full_path(&entries, keys),
                reference(&entries),
                "{entries:?}"
            );
        }
    }

    #[test]
    fn entries_below_regular_files_are_reported() {
        assert_eq!(first_entry_below_file(&[], KEYS), ParentCheck::Consistent);
        // Directories may own entries, at any spelling.
        let entries = [
            entry(None, "src"),
            file(Some(0), "main.rs"),
            file(None, "src/lib.rs"),
            file(None, "src/cli/mod.rs"),
            placeholder(Some(0)),
            file(None, "README.md"),
        ];
        assert_eq!(
            first_entry_below_file(&entries, KEYS),
            ParentCheck::Consistent
        );
        // A file before or after the entry below it.
        let entries = [file(None, "a"), file(None, "a/b")];
        assert_eq!(first_entry_below_file(&entries, KEYS), below(1, "a"));
        let entries = [file(None, "a/b"), file(None, "a")];
        assert_eq!(first_entry_below_file(&entries, KEYS), below(0, "a"));
        // Through a directory entry and a multi-component path.
        let entries = [entry(None, "x"), file(Some(0), "y/z"), file(None, "x/y")];
        assert_eq!(first_entry_below_file(&entries, KEYS), below(1, "x/y"));
        // The shortest file prefix is named.
        let entries = [file(None, "a"), file(None, "a/b"), file(None, "a/b/c")];
        assert_eq!(first_entry_below_file(&entries, KEYS), below(1, "a"));
        let entries = [file(None, "a/b/c"), file(None, "a/b"), file(None, "a")];
        assert_eq!(first_entry_below_file(&entries, KEYS), below(0, "a"));
        // Only a whole component counts as a parent.
        let entries = [file(None, "a"), file(None, "ab/c"), file(None, "a.d")];
        assert_eq!(
            first_entry_below_file(&entries, KEYS),
            ParentCheck::Consistent
        );
        // Placeholders are not files.
        let entries = [placeholder(None), file(None, ".../x")];
        assert_eq!(
            first_entry_below_file(&entries, KEYS),
            ParentCheck::Consistent
        );
    }

    #[test]
    fn below_file_search_matches_the_brute_force_reference() {
        const NAMES: [&str; 6] = ["a", "b", "a/b", "b/a", "a/b/a", "\u{e9}"];
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut step = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut reported = 0;
        for round in 0..4000 {
            let length = (step() % 9) as usize;
            let mut entries = Vec::with_capacity(length);
            for index in 0..length {
                let roll = step();
                let parent = if index == 0 || roll % 3 == 0 {
                    None
                } else {
                    Some((roll / 3) as usize % (index + 1))
                };
                let name = NAMES[(roll / 21) as usize % NAMES.len()];
                entries.push(match roll % 11 {
                    0 => placeholder(parent),
                    1..=5 => file(parent, name),
                    _ => entry(parent, name),
                });
            }
            let keys = HashKeys {
                base: step(),
                scale: step(),
                shift: if round % 2 == 0 { 0 } else { step() },
            };
            let expected = reference_below_file(&entries);
            if expected != ParentCheck::Consistent {
                reported += 1;
            }
            assert_eq!(
                first_entry_below_file(&entries, keys),
                expected,
                "{entries:?}"
            );
            // Hash collisions never change the outcome.
            let zero = HashKeys {
                base: 0,
                scale: 0,
                shift: 0,
            };
            assert_eq!(first_entry_below_file(&entries, zero), expected);
        }
        assert!(reported > 500, "the corpus must reach the rejection");
    }

    #[test]
    fn hash_and_bucket_stay_in_range() {
        let modulus = HASH_MODULUS;
        assert_eq!(hash_extend(0, "", 7), 0);
        assert_eq!(hash_extend(0, "a", 7), u64::from('a') + 1);
        assert_eq!(
            hash_extend(0, "ab", 7),
            ((u64::from('a') + 1) * 7 + u64::from('b') + 1)
        );
        assert_eq!(
            hash_extend(hash_extend(0, "a/", 7), "b", 7),
            hash_extend(0, "a/b", 7)
        );
        assert_eq!(hash_step(3, 'a', 7), 3 * 7 + u64::from('a') + 1);
        assert!(hash_extend(modulus - 1, "\u{10FFFF}\u{10FFFF}", modulus - 1) < modulus);
        assert!(bucket_index(modulus - 1, modulus - 1, modulus - 1, 3) < 3);
    }

    fn shapes(text: &str) -> Vec<SiblingShape> {
        text.chars()
            .map(|ch| match ch {
                'e' => SiblingShape::Entry,
                'p' => SiblingShape::Placeholder {
                    has_children: false,
                },
                _ => SiblingShape::Placeholder { has_children: true },
            })
            .collect()
    }

    #[test]
    fn placeholder_siblings_follow_list_order() {
        assert_eq!(check_placeholder_siblings(&[]), PlaceholderVerdict::Valid);
        assert_eq!(
            check_placeholder_siblings(&shapes("pepep")),
            PlaceholderVerdict::Valid
        );
        assert_eq!(
            check_placeholder_siblings(&shapes("epp")),
            PlaceholderVerdict::Adjacent { index: 2 }
        );
        assert_eq!(
            check_placeholder_siblings(&shapes("eCe")),
            PlaceholderVerdict::WithChildren { index: 1 }
        );
        // Adjacency of a placeholder is checked before its own children,
        // and an earlier offense wins.
        assert_eq!(
            check_placeholder_siblings(&shapes("Cp")),
            PlaceholderVerdict::Adjacent { index: 1 }
        );
        assert_eq!(
            check_placeholder_siblings(&shapes("Cepp")),
            PlaceholderVerdict::WithChildren { index: 0 }
        );
        assert_eq!(
            check_placeholder_siblings(&shapes("pepC")),
            PlaceholderVerdict::Adjacent { index: 3 }
        );
    }
}
