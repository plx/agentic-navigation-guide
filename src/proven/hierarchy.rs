//! Verified hierarchy assembly plan.
//!
//! `Parser::build_hierarchy` turns the parsed items, in source order, into a
//! tree with a stack of open directories: before each item it closes open
//! directories (each closed directory is attached to the directory below it
//! on the stack, or becomes a root), then attaches a non-directory item to
//! the innermost remaining open directory (or the roots) and pushes a
//! directory item instead. [`plan_item`] makes the only decision in that
//! loop: from the number of open directories and the item's logical depth it
//! rejects the item (`InvalidIndentationLevel`) or says how many open
//! directories to close first.
//!
//! [`assembly`] is that loop over item indices, so the contract-level
//! meaning (`docs/v0.2-contract.md`, "Indentation and parentage": "Dedenting
//! closes the deeper parents. Re-indenting is valid only beneath the
//! immediately preceding directory at the new parent depth.") is proven over
//! whole item sequences (these proofs exist only in the Verus build):
//!
//! - `theorem_assembly_accepts_exactly` and `theorem_assembly_first_error`:
//!   the assembly accepts exactly the item sequences that satisfy the layout
//!   kernel's depth rules (`layout::depth_rules`, with "is a directory" as
//!   "can own children"), and otherwise rejects the first item that breaks
//!   them;
//! - `theorem_assembly_parents`: every item is attached to its preorder
//!   parent (the latest earlier item one level shallower, with only deeper
//!   items between them), which is a directory, and exactly the depth-0
//!   items become roots;
//! - `theorem_open_directories`: the open stack holds one directory per
//!   level, at most 257, and each one sits directly above its parent, so
//!   closing it attaches it to that parent;
//! - `theorem_assembly_work`: the run closes each directory exactly once,
//!   final drain included, so the loop performs `2 * items + 2 * directories`
//!   observable steps;
//! - `theorem_parsed_items_assemble`: the items of a body that the layout
//!   kernel accepted (one or more items per line at that line's depth, and
//!   only directories from a line that can own children) are always
//!   assembled, so `build_hierarchy` never reports `InvalidIndentationLevel`
//!   for a parsed body;
//! - `theorem_positive_depths_include_one`: such a body that has any
//!   indented item has one at depth 1, so the validator's smallest positive
//!   level is 1 and its `InconsistentIndentation` check cannot fire; and
//!   `theorem_children_one_level_deeper`: every child is exactly one level
//!   below its parent, so the validator's nesting check cannot fire either.
//!
//! Trusted glue outside this file: `Parser::build_hierarchy_with_observer`
//! calls [`plan_item`] once per item, in order, with the current stack
//! length and the item's `indent_level`; closes exactly the returned number
//! of directories; pushes exactly the items whose `is_directory()` holds;
//! and drains the stack at the end. Those moves of `NavigationGuideLine`
//! values are not verified.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::proven::layout::depth_rules;
use crate::proven::layout::MAX_LOGICAL_DEPTH;

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Specification.
// ---------------------------------------------------------------------------

/// The decision for an item at `depth` when `open` directories are open:
/// `None` rejects the item; `Some(c)` closes the innermost `c` directories,
/// leaving exactly `depth` open.
pub(crate) open spec fn plan_spec(open: int, depth: int) -> Option<int> {
    if depth > 256 || depth > open {
        None
    } else {
        Some(open - depth)
    }
}

/// The assembly after a prefix of the items, by item index.
pub(crate) struct AssemblyView {
    /// The open directories, outermost first.
    pub(crate) open: Seq<int>,
    /// For each item, the item it is attached to, or `None` for a root.
    pub(crate) parents: Seq<Option<int>>,
    /// The number of directories closed so far.
    pub(crate) closed: int,
}

/// The assembly before any item.
pub(crate) open spec fn initial_assembly() -> AssemblyView {
    AssemblyView { open: Seq::empty(), parents: Seq::empty(), closed: 0 }
}

/// One loop iteration for item `n` at `depth`: close the planned number of
/// innermost open directories, attach the item to the innermost remaining
/// one (or make it a root), and open it when it is a directory.
pub(crate) open spec fn assembly_step(
    s: AssemblyView,
    n: int,
    depth: int,
    is_dir: bool,
) -> Option<AssemblyView> {
    match plan_spec(s.open.len() as int, depth) {
        None => None,
        Some(closes) => {
            let kept = s.open.take(s.open.len() - closes);
            Some(
                AssemblyView {
                    open: if is_dir {
                        kept.push(n)
                    } else {
                        kept
                    },
                    parents: s.parents.push(
                        if kept.len() == 0 {
                            None
                        } else {
                            Some(kept.last())
                        },
                    ),
                    closed: s.closed + closes,
                },
            )
        },
    }
}

/// The assembly loop over items with logical depths `depths`, where item
/// `i` is a directory exactly when `dirs[i]`: on success the open stack,
/// each item's parent, and the closed-directory count; on failure the index
/// of the rejected item.
pub(crate) open spec fn assembly(depths: Seq<int>, dirs: Seq<bool>) -> Result<AssemblyView, int>
    decreases depths.len(),
{
    if depths.len() == 0 {
        Ok(initial_assembly())
    } else {
        let n = depths.len() - 1;
        match assembly(depths.drop_last(), dirs) {
            Err(i) => Err(i),
            Ok(s) => match assembly_step(s, n, depths[n], dirs[n]) {
                None => Err(n),
                Some(t) => Ok(t),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Declarative contract vocabulary.
// ---------------------------------------------------------------------------

/// Every item after `p` and before `i` is deeper than `level`.
pub(crate) open spec fn deeper_between(depths: Seq<int>, p: int, i: int, level: int) -> bool {
    forall|q: int| p < q < i ==> #[trigger] depths[q] > level
}

/// Item `p` is the preorder parent of item `i`: an earlier item exactly one
/// level shallower, and every item between them is at least as deep as `i`
/// (so `p` is the latest earlier item at that level, and `i` lies in its
/// subtree).
pub(crate) open spec fn preorder_parent(depths: Seq<int>, i: int, p: int) -> bool {
    &&& 0 <= p < i < depths.len()
    &&& depths[p] == depths[i] - 1
    &&& deeper_between(depths, p, i, depths[i] - 1)
}

/// Item `i` is a root exactly when it is at depth 0, and otherwise is
/// attached to its preorder parent, which is a directory.
pub(crate) open spec fn attached_to_preorder_parent(
    depths: Seq<int>,
    dirs: Seq<bool>,
    parents: Seq<Option<int>>,
    i: int,
) -> bool {
    &&& parents[i] is None <==> depths[i] == 0
    &&& parents[i] matches Some(p) ==> preorder_parent(depths, i, p) && dirs[p]
}

/// The number of directories among the first `n` items.
pub(crate) open spec fn dir_count(dirs: Seq<bool>, n: int) -> int
    decreases n,
{
    if n <= 0 {
        0
    } else {
        dir_count(dirs, n - 1) + if dirs[n - 1] {
            1int
        } else {
            0int
        }
    }
}

/// The number of open directories after the first `n` items: the depth of
/// the last item, plus one when it is a directory.
pub(crate) open spec fn open_after(depths: Seq<int>, dirs: Seq<bool>, n: int) -> int {
    if n <= 0 {
        0
    } else {
        depths[n - 1] + if dirs[n - 1] {
            1int
        } else {
            0int
        }
    }
}

/// Every depth is a non-negative integer, as for `usize` depths.
pub(crate) open spec fn natural_depths(depths: Seq<int>) -> bool {
    forall|i: int| 0 <= i < depths.len() ==> 0 <= #[trigger] depths[i]
}

/// Item `n` obeys the depth rules given the items before it: its depth is
/// within 0..=256, the first item is at depth 0, and depth grows by at most
/// one and only directly below a directory.
pub(crate) open spec fn next_depth_ok(depths: Seq<int>, dirs: Seq<bool>, n: int) -> bool {
    let d = depths[n];
    &&& 0 <= d <= 256
    &&& n == 0 ==> d == 0
    &&& n > 0 ==> d <= depths[n - 1] + 1 && (d == depths[n - 1] + 1 ==> dirs[n - 1])
}

/// The first `n` items obey the depth rules.
pub(crate) open spec fn rules_upto(depths: Seq<int>, dirs: Seq<bool>, n: int) -> bool {
    forall|i: int| 0 <= i < n ==> #[trigger] next_depth_ok(depths, dirs, i)
}

/// The open stack after the first `n` items: entry `j` is a directory at
/// depth `j` with only deeper items after it, and its parent is entry
/// `j - 1` (or none for entry 0).
pub(crate) open spec fn open_stack_shape(
    depths: Seq<int>,
    dirs: Seq<bool>,
    n: int,
    s: AssemblyView,
) -> bool {
    &&& s.open.len() == open_after(depths, dirs, n)
    &&& forall|j: int| 0 <= j < s.open.len() ==> #[trigger] open_entry(depths, dirs, n, s, j)
}

/// Entry `j` of the open stack after the first `n` items.
pub(crate) open spec fn open_entry(
    depths: Seq<int>,
    dirs: Seq<bool>,
    n: int,
    s: AssemblyView,
    j: int,
) -> bool {
    let o = s.open[j];
    &&& 0 <= o < n
    &&& dirs[o]
    &&& depths[o] == j
    &&& deeper_between(depths, o, n, j)
    &&& s.parents[o] == if j == 0 {
        None::<int>
    } else {
        Some(s.open[j - 1])
    }
}

/// Every item among the first `n` is attached to its preorder parent.
pub(crate) open spec fn parents_ok(depths: Seq<int>, dirs: Seq<bool>, n: int, s: AssemblyView) -> bool {
    &&& s.parents.len() == n
    &&& forall|i: int|
        0 <= i < n ==> #[trigger] attached_to_preorder_parent(depths, dirs, s.parents, i)
}

/// What the assembly establishes after the first `n` items.
pub(crate) open spec fn assembly_summary(
    depths: Seq<int>,
    dirs: Seq<bool>,
    n: int,
    s: AssemblyView,
) -> bool {
    &&& parents_ok(depths, dirs, n, s)
    &&& open_stack_shape(depths, dirs, n, s)
    &&& s.closed + s.open.len() == dir_count(dirs, n)
}

// ---------------------------------------------------------------------------
// Depth-rule lemmas.
// ---------------------------------------------------------------------------

/// [`depth_rules`] over a prefix is [`rules_upto`] that prefix's length.
proof fn lemma_rules_take(depths: Seq<int>, dirs: Seq<bool>, k: int)
    requires
        0 <= k <= depths.len(),
        dirs.len() >= depths.len(),
    ensures
        depth_rules(depths.take(k), dirs) <==> rules_upto(depths, dirs, k),
{
    let p = depths.take(k);
    assert forall|i: int| 0 <= i < k implies #[trigger] p[i] == depths[i] by {}
    if depth_rules(p, dirs) {
        assert forall|i: int| 0 <= i < k implies #[trigger] next_depth_ok(depths, dirs, i) by {
            assert(p[i] == depths[i]);
            assert(0 <= p[i] <= 256);
            if i > 0 {
                assert(p[i - 1] == depths[i - 1]);
                assert(p[i] <= p[i - 1] + 1);
            }
        }
    }
    if rules_upto(depths, dirs, k) {
        assert forall|i: int| 0 <= i < p.len() implies 0 <= #[trigger] p[i] <= 256 by {
            assert(next_depth_ok(depths, dirs, i));
        }
        if p.len() > 0 {
            assert(next_depth_ok(depths, dirs, 0));
        }
        assert forall|i: int| 0 < i < p.len() implies #[trigger] p[i] <= p[i - 1] + 1 by {
            assert(next_depth_ok(depths, dirs, i));
            assert(p[i - 1] == depths[i - 1]);
        }
        assert forall|i: int| 0 < i < p.len() && #[trigger] p[i] == p[i - 1] + 1 implies dirs[i
            - 1] by {
            assert(next_depth_ok(depths, dirs, i));
            assert(p[i - 1] == depths[i - 1]);
        }
    }
}

/// [`depth_rules`] over the whole sequence is [`rules_upto`] its length.
proof fn lemma_rules_all(depths: Seq<int>, dirs: Seq<bool>)
    requires
        dirs.len() >= depths.len(),
    ensures
        depth_rules(depths, dirs) <==> rules_upto(depths, dirs, depths.len() as int),
{
    lemma_rules_take(depths, dirs, depths.len() as int);
    assert(depths.take(depths.len() as int) =~= depths);
}

// ---------------------------------------------------------------------------
// The assembly invariant.
// ---------------------------------------------------------------------------

proof fn lemma_take_step(depths: Seq<int>, k: int)
    requires
        0 < k <= depths.len(),
    ensures
        depths.take(k).drop_last() == depths.take(k - 1),
        depths.take(k).len() == k,
        depths.take(k)[k - 1] == depths[k - 1],
{
    assert(depths.take(k).drop_last() =~= depths.take(k - 1));
}

/// The plan accepts item `n` exactly when it obeys the depth rules.
proof fn lemma_plan_iff_rule(depths: Seq<int>, dirs: Seq<bool>, n: int, open: int)
    requires
        0 <= n < depths.len(),
        n < dirs.len(),
        0 <= depths[n],
        open == open_after(depths, dirs, n),
    ensures
        plan_spec(open, depths[n]) is Some <==> next_depth_ok(depths, dirs, n),
{
}

/// One loop iteration keeps every item attached to its preorder parent.
proof fn lemma_step_parents(depths: Seq<int>, dirs: Seq<bool>, n: int, s: AssemblyView)
    requires
        0 <= n < depths.len(),
        n < dirs.len(),
        parents_ok(depths, dirs, n, s),
        open_stack_shape(depths, dirs, n, s),
        next_depth_ok(depths, dirs, n),
        plan_spec(s.open.len() as int, depths[n]) is Some,
    ensures
        assembly_step(s, n, depths[n], dirs[n]) matches Some(t) && parents_ok(
            depths,
            dirs,
            n + 1,
            t,
        ),
{
    let d = depths[n];
    let t = assembly_step(s, n, d, dirs[n])->Some_0;
    let kept = s.open.take(d);
    assert(t.parents == s.parents.push(
        if kept.len() == 0 {
            None
        } else {
            Some(kept.last())
        },
    ));
    if d > 0 {
        let o = s.open[d - 1];
        assert(kept.last() == o);
        assert(open_entry(depths, dirs, n, s, d - 1));
        assert(preorder_parent(depths, n, o));
    }
    assert(attached_to_preorder_parent(depths, dirs, t.parents, n));
    assert forall|i: int| 0 <= i < n + 1 implies #[trigger] attached_to_preorder_parent(
        depths,
        dirs,
        t.parents,
        i,
    ) by {
        if i < n {
            assert(attached_to_preorder_parent(depths, dirs, s.parents, i));
            assert(t.parents[i] == s.parents[i]);
        }
    }
}

/// One loop iteration keeps the open stack in shape.
proof fn lemma_step_open(depths: Seq<int>, dirs: Seq<bool>, n: int, s: AssemblyView)
    requires
        0 <= n < depths.len(),
        n < dirs.len(),
        parents_ok(depths, dirs, n, s),
        open_stack_shape(depths, dirs, n, s),
        next_depth_ok(depths, dirs, n),
        plan_spec(s.open.len() as int, depths[n]) is Some,
    ensures
        assembly_step(s, n, depths[n], dirs[n]) matches Some(t) && open_stack_shape(
            depths,
            dirs,
            n + 1,
            t,
        ),
{
    let d = depths[n];
    let t = assembly_step(s, n, d, dirs[n])->Some_0;
    let kept = s.open.take(d);
    let parent = if kept.len() == 0 {
        None
    } else {
        Some(kept.last())
    };
    assert(t.parents == s.parents.push(parent));
    assert(t.open == if dirs[n] {
        kept.push(n)
    } else {
        kept
    });
    assert forall|j: int| 0 <= j < t.open.len() implies #[trigger] open_entry(
        depths,
        dirs,
        n + 1,
        t,
        j,
    ) by {
        if j < d {
            let o = s.open[j];
            assert(open_entry(depths, dirs, n, s, j));
            assert(t.open[j] == o);
            assert(t.parents[o] == s.parents[o]);
            if j > 0 {
                assert(t.open[j - 1] == s.open[j - 1]);
            }
            assert forall|q: int| o < q < n + 1 implies #[trigger] depths[q] > j by {
                if q < n {
                    assert(deeper_between(depths, o, n, j));
                }
            }
        } else {
            assert(t.open[j] == n);
            assert(t.parents[n] == parent);
            if j > 0 {
                assert(t.open[j - 1] == kept[j - 1]);
            }
        }
    }
}

/// One loop iteration keeps the closed and open directories counted.
proof fn lemma_step_work(depths: Seq<int>, dirs: Seq<bool>, n: int, s: AssemblyView)
    requires
        0 <= n < depths.len(),
        n < dirs.len(),
        s.closed + s.open.len() == dir_count(dirs, n),
        0 <= depths[n] <= s.open.len(),
        plan_spec(s.open.len() as int, depths[n]) is Some,
    ensures
        assembly_step(s, n, depths[n], dirs[n]) matches Some(t) && t.closed + t.open.len()
            == dir_count(dirs, n + 1),
{
    let d = depths[n];
    let kept = s.open.take(d);
}

/// The assembly of the first `k` items succeeds exactly when they obey the
/// depth rules; it then satisfies [`assembly_summary`], and otherwise it
/// rejects the first item that breaks them.
proof fn lemma_assembly_prefix(depths: Seq<int>, dirs: Seq<bool>, k: int)
    requires
        0 <= k <= depths.len(),
        dirs.len() >= depths.len(),
        natural_depths(depths),
    ensures
        assembly(depths.take(k), dirs) is Ok <==> rules_upto(depths, dirs, k),
        assembly(depths.take(k), dirs) matches Ok(s) ==> assembly_summary(depths, dirs, k, s),
        assembly(depths.take(k), dirs) matches Err(i) ==> 0 <= i < k && rules_upto(depths, dirs, i)
            && !next_depth_ok(depths, dirs, i),
    decreases k,
{
    if k == 0 {
        assert(depths.take(0).len() == 0);
        assert(assembly_summary(depths, dirs, 0, initial_assembly()));
    } else {
        let n = k - 1;
        lemma_take_step(depths, k);
        lemma_assembly_prefix(depths, dirs, n);
        match assembly(depths.take(n), dirs) {
            Err(i) => {},
            Ok(s) => {
                assert(0 <= depths[n]);
                lemma_plan_iff_rule(depths, dirs, n, s.open.len() as int);
                if plan_spec(s.open.len() as int, depths[n]) is Some {
                    lemma_step_parents(depths, dirs, n, s);
                    lemma_step_open(depths, dirs, n, s);
                    lemma_step_work(depths, dirs, n, s);
                    assert(rules_upto(depths, dirs, k)) by {
                        assert forall|i: int| 0 <= i < k implies #[trigger] next_depth_ok(
                            depths,
                            dirs,
                            i,
                        ) by {
                            if i < n {
                                assert(next_depth_ok(depths, dirs, i));
                            }
                        }
                    }
                } else {
                    assert(!rules_upto(depths, dirs, k)) by {
                        assert(!next_depth_ok(depths, dirs, n));
                    }
                }
            },
        }
    }
}

/// [`lemma_assembly_prefix`] for every item.
proof fn lemma_assembly(depths: Seq<int>, dirs: Seq<bool>)
    requires
        dirs.len() >= depths.len(),
        natural_depths(depths),
    ensures
        assembly(depths, dirs) is Ok <==> depth_rules(depths, dirs),
        assembly(depths, dirs) matches Ok(s) ==> assembly_summary(
            depths,
            dirs,
            depths.len() as int,
            s,
        ),
        assembly(depths, dirs) matches Err(i) ==> 0 <= i < depths.len() && rules_upto(
            depths,
            dirs,
            i,
        ) && !next_depth_ok(depths, dirs, i),
{
    lemma_assembly_prefix(depths, dirs, depths.len() as int);
    assert(depths.take(depths.len() as int) =~= depths);
    lemma_rules_all(depths, dirs);
}

// ---------------------------------------------------------------------------
// Contract-level theorems.
// ---------------------------------------------------------------------------

/// The assembly accepts exactly the item sequences that satisfy the
/// contract's depth rules, with "is a directory" as "can own children": the
/// first item at depth 0, every depth at most 256, and depth growing by at
/// most one per item and only directly below a directory.
pub(crate) proof fn theorem_assembly_accepts_exactly(depths: Seq<int>, dirs: Seq<bool>)
    requires
        dirs.len() == depths.len(),
        natural_depths(depths),
    ensures
        assembly(depths, dirs) is Ok <==> depth_rules(depths, dirs),
{
    lemma_assembly(depths, dirs);
}

/// A rejected sequence is rejected at its first item that breaks the depth
/// rules: the items before it obey them, and adding it does not.
pub(crate) proof fn theorem_assembly_first_error(depths: Seq<int>, dirs: Seq<bool>)
    requires
        dirs.len() == depths.len(),
        natural_depths(depths),
        assembly(depths, dirs) is Err,
    ensures
        ({
            let i = assembly(depths, dirs)->Err_0;
            &&& 0 <= i < depths.len()
            &&& depth_rules(depths.take(i), dirs)
            &&& !depth_rules(depths.take(i + 1), dirs)
        }),
{
    lemma_assembly(depths, dirs);
    let i = assembly(depths, dirs)->Err_0;
    lemma_rules_take(depths, dirs, i);
    lemma_rules_take(depths, dirs, i + 1);
}

/// On success, every item is attached to its preorder parent, which is a
/// directory one level shallower, and exactly the depth-0 items are roots.
pub(crate) proof fn theorem_assembly_parents(depths: Seq<int>, dirs: Seq<bool>)
    requires
        dirs.len() == depths.len(),
        natural_depths(depths),
        assembly(depths, dirs) is Ok,
    ensures
        ({
            let s = assembly(depths, dirs)->Ok_0;
            &&& s.parents.len() == depths.len()
            &&& forall|i: int|
                0 <= i < depths.len() ==> #[trigger] attached_to_preorder_parent(
                    depths,
                    dirs,
                    s.parents,
                    i,
                )
        }),
{
    lemma_assembly(depths, dirs);
}

/// On success, the open stack holds one directory per level (at most 257),
/// the latest directory at that level, and each sits directly above its
/// parent: closing entry `j` attaches it to entry `j - 1`, or makes it a
/// root when `j == 0`, which is where [`theorem_assembly_parents`] places
/// it.
pub(crate) proof fn theorem_open_directories(depths: Seq<int>, dirs: Seq<bool>)
    requires
        dirs.len() == depths.len(),
        natural_depths(depths),
        assembly(depths, dirs) is Ok,
    ensures
        ({
            let s = assembly(depths, dirs)->Ok_0;
            &&& s.open.len() <= 257
            &&& open_stack_shape(depths, dirs, depths.len() as int, s)
        }),
{
    lemma_assembly(depths, dirs);
    let n = depths.len() as int;
    if n > 0 {
        assert(0 <= depths[n - 1] <= 256);
    }
}

/// On success, the directories closed during the run plus those still open
/// (which the final drain closes) are exactly the directory items, so each
/// directory is closed once and the loop's observable work is one visit and
/// one attachment per item plus one open and one close per directory.
pub(crate) proof fn theorem_assembly_work(depths: Seq<int>, dirs: Seq<bool>)
    requires
        dirs.len() == depths.len(),
        natural_depths(depths),
        assembly(depths, dirs) is Ok,
    ensures
        ({
            let s = assembly(depths, dirs)->Ok_0;
            &&& s.closed + s.open.len() == dir_count(dirs, depths.len() as int)
            &&& dir_count(dirs, depths.len() as int) <= depths.len()
        }),
{
    lemma_assembly(depths, dirs);
    lemma_dir_count_bound(dirs, depths.len() as int);
}

proof fn lemma_dir_count_bound(dirs: Seq<bool>, n: int)
    requires
        0 <= n <= dirs.len(),
    ensures
        0 <= dir_count(dirs, n) <= n,
    decreases n,
{
    if n > 0 {
        lemma_dir_count_bound(dirs, n - 1);
    }
}

/// The items of a body accepted by the layout kernel: line `l` has depth
/// `line_depths[l]` and contributes one or more consecutive items at that
/// depth (`line_of[j]` is the line of item `j`), and every item of a line
/// that can own children is a directory.
pub(crate) open spec fn expands_lines(
    line_depths: Seq<int>,
    owns: Seq<bool>,
    depths: Seq<int>,
    dirs: Seq<bool>,
    line_of: Seq<int>,
) -> bool {
    &&& dirs.len() == depths.len()
    &&& line_of.len() == depths.len()
    &&& owns.len() >= line_depths.len()
    &&& depths.len() > 0 ==> line_of[0] == 0
    &&& forall|j: int|
        0 <= j < depths.len() ==> {
            let l = #[trigger] line_of[j];
            &&& 0 <= l < line_depths.len()
            &&& depths[j] == line_depths[l]
            &&& owns[l] ==> dirs[j]
        }
    &&& forall|j: int|
        0 < j < depths.len() ==> #[trigger] line_of[j] == line_of[j - 1] || line_of[j] == line_of[j
            - 1] + 1
}

/// The items of every body that satisfies the layout kernel's depth rules
/// obey the same rules, so the assembly accepts them: `build_hierarchy`
/// never reports `InvalidIndentationLevel` for a parsed body, and attaches
/// every item to its preorder parent.
pub(crate) proof fn theorem_parsed_items_assemble(
    line_depths: Seq<int>,
    owns: Seq<bool>,
    depths: Seq<int>,
    dirs: Seq<bool>,
    line_of: Seq<int>,
)
    requires
        depth_rules(line_depths, owns),
        expands_lines(line_depths, owns, depths, dirs, line_of),
    ensures
        natural_depths(depths),
        depth_rules(depths, dirs),
        assembly(depths, dirs) is Ok,
{
    assert forall|j: int| 0 <= j < depths.len() implies 0 <= #[trigger] depths[j] <= 256 by {
        let l = line_of[j];
        assert(0 <= line_depths[l] <= 256);
    }
    if depths.len() > 0 {
        assert(line_of[0] == 0);
    }
    assert forall|j: int| 0 < j < depths.len() implies #[trigger] depths[j] <= depths[j - 1] + 1 by {
        let l = line_of[j];
        let k = line_of[j - 1];
        if l == k + 1 {
            assert(line_depths[l] <= line_depths[l - 1] + 1);
        }
    }
    assert forall|j: int| 0 < j < depths.len() && #[trigger] depths[j] == depths[j - 1] + 1 implies dirs[j
        - 1] by {
        let l = line_of[j];
        let k = line_of[j - 1];
        assert(l == k + 1);
        assert(line_depths[l] == line_depths[l - 1] + 1);
        assert(owns[k]);
    }
    lemma_assembly(depths, dirs);
}

/// Under the depth rules, a sequence with an indented item has an item at
/// depth 1: the validator's smallest positive indentation level is 1, which
/// divides every level, so its `InconsistentIndentation` check never fires
/// on a parsed guide.
pub(crate) proof fn theorem_positive_depths_include_one(depths: Seq<int>, dirs: Seq<bool>)
    requires
        depth_rules(depths, dirs),
        exists|i: int| 0 <= i < depths.len() && #[trigger] depths[i] > 0,
    ensures
        exists|j: int| 0 <= j < depths.len() && #[trigger] depths[j] == 1,
    decreases depths.len(),
{
    let n = depths.len() as int;
    let p = depths.drop_last();
    assert forall|i: int| 0 <= i < n - 1 implies #[trigger] p[i] == depths[i] by {}
    assert(depth_rules(p, dirs)) by {
        assert forall|i: int| 0 < i < p.len() implies #[trigger] p[i] <= p[i - 1] + 1 by {
            assert(depths[i] <= depths[i - 1] + 1);
        }
        assert forall|i: int| 0 < i < p.len() && #[trigger] p[i] == p[i - 1] + 1 implies dirs[i
            - 1] by {
            assert(depths[i] == depths[i - 1] + 1);
        }
    }
    if exists|i: int| 0 <= i < p.len() && #[trigger] p[i] > 0 {
        theorem_positive_depths_include_one(p, dirs);
        let j = choose|j: int| 0 <= j < p.len() && #[trigger] p[j] == 1;
        assert(depths[j] == p[j]);
    } else {
        // The last item is the first indented one; the item before it is at
        // depth 0.
        let i = choose|i: int| 0 <= i < depths.len() && #[trigger] depths[i] > 0;
        if i < n - 1 {
            assert(p[i] == depths[i]);
        }
        assert(i == n - 1);
        assert(i > 0);
        assert(p[i - 1] == depths[i - 1]);
        assert(depths[i] <= depths[i - 1] + 1);
        assert(depths[i] == 1);
    }
}

/// On success, every child is exactly one level below its parent, so the
/// validator's nesting check never fires on an assembled parsed guide.
pub(crate) proof fn theorem_children_one_level_deeper(depths: Seq<int>, dirs: Seq<bool>)
    requires
        dirs.len() == depths.len(),
        natural_depths(depths),
        assembly(depths, dirs) is Ok,
    ensures
        ({
            let s = assembly(depths, dirs)->Ok_0;
            forall|i: int|
                0 <= i < depths.len() ==> (#[trigger] s.parents[i] matches Some(p) ==> depths[i]
                    == depths[p] + 1)
        }),
{
    lemma_assembly(depths, dirs);
    let s = assembly(depths, dirs)->Ok_0;
    assert forall|i: int| 0 <= i < depths.len() implies (#[trigger] s.parents[i] matches Some(p)
        ==> depths[i] == depths[p] + 1) by {
        assert(attached_to_preorder_parent(depths, dirs, s.parents, i));
    }
}

} // verus!

/// Decide what the hierarchy assembly does with an item at logical `depth`
/// while `open_directories` directories are open: `None` rejects it with
/// `InvalidIndentationLevel` (deeper than 256, or deeper than the open
/// directories allow); `Some(closes)` closes the innermost `closes`
/// directories first, leaving exactly `depth` open.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r is Some <==> plan_spec(open_directories as int, depth as int) is Some,
        r matches Some(closes) ==> plan_spec(open_directories as int, depth as int) == Some(
            closes as int,
        ),
))]
pub(crate) fn plan_item(open_directories: usize, depth: usize) -> Option<usize> {
    if depth > MAX_LOGICAL_DEPTH || depth > open_directories {
        None
    } else {
        Some(open_directories - depth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_rejects_depths_beyond_the_open_directories_or_the_bound() {
        assert_eq!(plan_item(0, 0), Some(0));
        assert_eq!(plan_item(0, 1), None);
        assert_eq!(plan_item(3, 4), None);
        assert_eq!(plan_item(3, 3), Some(0));
        assert_eq!(plan_item(3, 1), Some(2));
        assert_eq!(plan_item(3, 0), Some(3));
        assert_eq!(plan_item(MAX_LOGICAL_DEPTH + 1, MAX_LOGICAL_DEPTH), Some(1));
        assert_eq!(
            plan_item(MAX_LOGICAL_DEPTH + 1, MAX_LOGICAL_DEPTH + 1),
            None
        );
        assert_eq!(plan_item(usize::MAX, usize::MAX), None);
        assert_eq!(plan_item(usize::MAX, 0), Some(usize::MAX));
    }

    /// The previous loop's decision: close while deeper, then require the
    /// stack length to equal the depth.
    fn reference_plan(open: usize, depth: usize) -> Option<usize> {
        if depth > MAX_LOGICAL_DEPTH {
            return None;
        }
        let mut len = open;
        let mut closes = 0;
        while len > depth {
            len -= 1;
            closes += 1;
        }
        (len == depth).then_some(closes)
    }

    #[test]
    fn plan_matches_the_previous_loop() {
        for open in 0..=MAX_LOGICAL_DEPTH + 2 {
            for depth in 0..=MAX_LOGICAL_DEPTH + 2 {
                assert_eq!(
                    plan_item(open, depth),
                    reference_plan(open, depth),
                    "{open} {depth}"
                );
            }
        }
    }
}
