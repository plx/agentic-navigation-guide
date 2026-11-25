//! Verified trust-anchor path reasoning: tail classification, the
//! below-anchor walk, the parent-containing reduction, and stepwise anchor
//! resolution.
//!
//! These kernels make the pure decisions of `guide_input.rs` for the
//! `docs/v0.2-contract.md` rules in "Trust anchors and path provenance" and
//! "Guide-file input". Glue parses each spelling with
//! `std::path::Path::components` (trusted) and passes only the kind of every
//! component ([`ComponentKind`]); the kernels return the classification or an
//! ordered plan with one step per component, and glue performs exactly the
//! I/O each step names, in plan order, so the error precedence of the plan
//! is the error precedence of the product:
//!
//! - [`classify_tail`]: a tail below the complete anchor spelling is
//!   parent-containing exactly when some component is `..`;
//! - [`beneath_plan`]: the ordinary below-anchor walk inspects every `Normal`
//!   component in order, each ancestor as a real directory and the last one
//!   as the final entry, and rejects at the first component that is not
//!   `Normal`; an empty tail names the anchor itself;
//! - [`parent_reduction_plan`]: the parent-containing reduction of an
//!   explicit guide tail descends into each `Normal` component, inspects the
//!   most recent uncancelled component before each `..` erases it, ascends
//!   above the anchor (recording that it left) when a `..` has nothing below
//!   the anchor to erase, ignores `.`, and rejects at the first root or
//!   prefix component;
//! - [`parent_reduction_outcome`]: after the reduction, a path that ends
//!   inside the anchor is in-anchor only if it never left; one that left and
//!   lexically returned fails closed; anything else is external;
//! - [`anchor_resolution_plan`]: an anchor spelling without `..` is resolved
//!   in one step; otherwise every `..` is applied only after resolving the
//!   complete prefix before it.
//!
//! Whether the reduced path still starts with the canonical anchor is a
//! component-wise comparison of host paths that glue performs and passes to
//! [`parent_reduction_outcome`]. The lexical model below (`model_path`)
//! states that comparison over abstract component sequences, with
//! `PathBuf::pop` at the filesystem root modeled as a no-op.
//!
//! The theorems below state the contract-level consequences:
//!
//! - `theorem_plan_follows_components`: the reduction plan has one step per
//!   component up to and including the first root or prefix component, which
//!   is rejected before any later step runs; `Normal` components descend,
//!   `.` stays, and only `..` ascends;
//! - `theorem_every_erased_component_is_inspected`: a `..` that has an
//!   uncancelled component to erase inspects exactly that component first;
//!   the component is `Normal` and everything between them is balanced;
//! - `theorem_reduction_tracks_lexical_depth`: the reduction leaves the
//!   anchor exactly when some prefix of the tail has negative lexical depth,
//!   and a tail that never leaves keeps exactly its lexical depth of
//!   uncancelled components, all of them `Normal`, analogous to
//!   `path_rules::theorem_valid_path_stays_below`;
//! - `theorem_never_left_is_in_anchor`: in the lexical model, a reduction
//!   that never left ends at the anchor followed by its uncancelled `Normal`
//!   components, so it is classified in-anchor and walks a tail of only
//!   `Normal` components;
//! - `theorem_left_path_is_never_in_anchor`: in the lexical model, a
//!   reduction that left ends at the ancestor it climbed to followed by its
//!   uncancelled names, and is rejected if that lexically returns beneath the
//!   anchor and external otherwise, never in-anchor;
//! - `theorem_in_anchor_outcome_never_left` and
//!   `theorem_leaves_and_returns_fails_closed`: an in-anchor outcome never
//!   left the anchor, and a path that leaves and then returns is rejected;
//! - `theorem_beneath_walk_accepts_only_normal_tails`: the below-anchor walk
//!   inspects only `Normal` components and reaches the final entry exactly
//!   when every component is `Normal`;
//! - `theorem_tail_class`: an in-anchor tail has no `..`;
//! - `theorem_anchor_resolution_order`: anchor resolution is direct exactly
//!   when the spelling has no `..`, and otherwise resolves the pending prefix
//!   before every `..`.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// The kind of one host path component.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComponentKind {
    /// A Windows path prefix such as `C:` or `\\server\share`.
    Prefix,
    /// The root separator.
    RootDir,
    /// `.`
    CurDir,
    /// `..`
    ParentDir,
    /// An ordinary name.
    Normal,
}

/// Whether a tail below the complete anchor spelling contains `..`.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TailClass {
    /// No `..` component: the tail is checked by the below-anchor walk.
    InAnchor,
    /// Some `..` component: the tail needs the parent-containing reduction.
    ParentContaining,
}

/// One step of the below-anchor walk.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BeneathStep {
    /// Descend into this `Normal` component, which must be a real directory.
    InspectAncestor,
    /// Descend into this last `Normal` component, the final entry.
    InspectFinal,
    /// This component is not `Normal`: reject the path.
    RejectUnresolved,
}

/// The plan of the below-anchor walk.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Debug)]
pub(crate) enum BeneathPlan {
    /// The tail is empty, so the path names the anchor itself.
    NamesAnchor,
    /// One step per component, up to and including the first rejection.
    Walk(Vec<BeneathStep>),
}

/// One step of the parent-containing reduction.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReductionStep {
    /// A `Normal` component: descend into it.
    Descend,
    /// A `.` component: stay.
    Stay,
    /// A `..` with an uncancelled component to erase: inspect the current
    /// path (that component) without following it, require a real
    /// directory, and then ascend.
    InspectThenAscend,
    /// A `..` with no uncancelled component: ascend above the anchor.
    AscendAboveAnchor,
    /// A root or prefix component: reject the path.
    RejectRootOrPrefix,
}

/// The plan of the parent-containing reduction.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Debug)]
pub(crate) struct ReductionPlan {
    /// One step per component, up to and including the first rejection.
    pub(crate) steps: Vec<ReductionStep>,
    /// Whether some `..` ascended above the anchor.
    pub(crate) left_anchor: bool,
}

/// The classification of a reduced parent-containing path.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParentOutcome {
    /// The reduced path is beneath the anchor and never left it.
    InAnchor,
    /// The reduced path is beneath the anchor after leaving it: ambiguous.
    LeavesAndReturns,
    /// The reduced path is outside the anchor.
    External,
}

/// One step of stepwise anchor resolution.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnchorStep {
    /// Append this component to the pending spelling.
    Append,
    /// A `.` component: skip it.
    Skip,
    /// A `..` component: resolve the pending spelling, require a directory,
    /// and then ascend from the resolved path.
    ResolveThenAscend,
}

/// The plan of anchor resolution.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Debug)]
pub(crate) enum AnchorResolution {
    /// No `..` component: canonicalize the spelling directly.
    Direct,
    /// Resolve stepwise, starting from the current directory when the
    /// spelling is relative, with one step per component.
    Stepwise {
        from_current_dir: bool,
        steps: Vec<AnchorStep>,
    },
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Specifications
// ---------------------------------------------------------------------------

/// A root or prefix component.
pub(crate) open spec fn is_root_or_prefix(c: ComponentKind) -> bool {
    c == ComponentKind::RootDir || c == ComponentKind::Prefix
}

/// Some component is `..`.
pub(crate) open spec fn has_parent(kinds: Seq<ComponentKind>) -> bool {
    exists|i: int| 0 <= i < kinds.len() && kinds[i] == ComponentKind::ParentDir
}

/// The tail classification.
pub(crate) open spec fn tail_class_spec(kinds: Seq<ComponentKind>) -> TailClass {
    if has_parent(kinds) {
        TailClass::ParentContaining
    } else {
        TailClass::InAnchor
    }
}

/// Every component before index `k` is `Normal`.
pub(crate) open spec fn all_normal_before(kinds: Seq<ComponentKind>, k: int) -> bool {
    forall|j: int| 0 <= j < k ==> kinds[j] == ComponentKind::Normal
}

/// The below-anchor step for component `i`.
pub(crate) open spec fn beneath_step_spec(kinds: Seq<ComponentKind>, i: int) -> BeneathStep {
    if kinds[i] != ComponentKind::Normal {
        BeneathStep::RejectUnresolved
    } else if i + 1 == kinds.len() {
        BeneathStep::InspectFinal
    } else {
        BeneathStep::InspectAncestor
    }
}

/// `steps` is the below-anchor walk of a nonempty tail: the step of every
/// component up to and including the first one that is not `Normal`.
pub(crate) open spec fn is_beneath_walk(kinds: Seq<ComponentKind>, steps: Seq<BeneathStep>) -> bool {
    &&& 0 < steps.len() <= kinds.len()
    &&& forall|i: int| 0 <= i < steps.len() ==> steps[i] == #[trigger] beneath_step_spec(kinds, i)
    &&& all_normal_before(kinds, steps.len() - 1)
    &&& (steps.len() == kinds.len() || steps.last() == BeneathStep::RejectUnresolved)
}

/// The below-anchor plan of a tail.
pub(crate) open spec fn is_beneath_plan(kinds: Seq<ComponentKind>, plan: BeneathPlan) -> bool {
    match plan {
        BeneathPlan::NamesAnchor => kinds.len() == 0,
        BeneathPlan::Walk(steps) => kinds.len() > 0 && is_beneath_walk(kinds, steps@),
    }
}

/// The reduction step for component `c` when `stack_len` components remain
/// uncancelled.
pub(crate) open spec fn reduction_step_spec(c: ComponentKind, stack_len: int) -> ReductionStep {
    match c {
        ComponentKind::Normal => ReductionStep::Descend,
        ComponentKind::CurDir => ReductionStep::Stay,
        ComponentKind::ParentDir => if stack_len > 0 {
            ReductionStep::InspectThenAscend
        } else {
            ReductionStep::AscendAboveAnchor
        },
        ComponentKind::RootDir => ReductionStep::RejectRootOrPrefix,
        ComponentKind::Prefix => ReductionStep::RejectRootOrPrefix,
    }
}

/// The reduction state after the first `k` components: the plan so far,
/// the indices of the components that remain uncancelled (oldest first),
/// whether some `..` ascended above the anchor, and whether the path was
/// rejected (which ends the reduction).
pub(crate) open spec fn reduce(kinds: Seq<ComponentKind>, k: int) -> (
    Seq<ReductionStep>,
    Seq<int>,
    bool,
    bool,
)
    decreases k,
{
    if k <= 0 {
        (Seq::empty(), Seq::empty(), false, false)
    } else {
        let prev = reduce(kinds, k - 1);
        if prev.3 {
            prev
        } else {
            let step = reduction_step_spec(kinds[k - 1], prev.1.len() as int);
            let steps = prev.0.push(step);
            match step {
                ReductionStep::Descend => (steps, prev.1.push(k - 1), prev.2, false),
                ReductionStep::Stay => (steps, prev.1, prev.2, false),
                ReductionStep::InspectThenAscend => (steps, prev.1.drop_last(), prev.2, false),
                ReductionStep::AscendAboveAnchor => (steps, prev.1, true, false),
                ReductionStep::RejectRootOrPrefix => (steps, prev.1, prev.2, true),
            }
        }
    }
}

/// The complete reduction of a tail.
pub(crate) open spec fn reduction_spec(kinds: Seq<ComponentKind>) -> (
    Seq<ReductionStep>,
    Seq<int>,
    bool,
    bool,
) {
    reduce(kinds, kinds.len() as int)
}

/// The classification of a reduced path from whether it still starts with
/// the canonical anchor and whether the reduction left the anchor, in the
/// production precedence.
pub(crate) open spec fn parent_outcome_spec(returns_to_anchor: bool, left_anchor: bool) -> ParentOutcome {
    if returns_to_anchor {
        if left_anchor {
            ParentOutcome::LeavesAndReturns
        } else {
            ParentOutcome::InAnchor
        }
    } else {
        ParentOutcome::External
    }
}

/// The lexical depth change of one component: `Normal` descends, `..`
/// ascends, and `.`, root, and prefix components stay.
pub(crate) open spec fn depth_delta(c: ComponentKind) -> int {
    match c {
        ComponentKind::Normal => 1,
        ComponentKind::ParentDir => -1,
        _ => 0,
    }
}

/// The lexical depth below the anchor after the first `k` components,
/// possibly negative.
pub(crate) open spec fn lexical_depth(kinds: Seq<ComponentKind>, k: int) -> int
    decreases k,
{
    if k <= 0 {
        0
    } else {
        lexical_depth(kinds, k - 1) + depth_delta(kinds[k - 1])
    }
}

/// The least lexical depth over the prefixes of length `0..=k`.
pub(crate) open spec fn min_depth(kinds: Seq<ComponentKind>, k: int) -> int
    decreases k,
{
    if k <= 0 {
        0
    } else if lexical_depth(kinds, k) < min_depth(kinds, k - 1) {
        lexical_depth(kinds, k)
    } else {
        min_depth(kinds, k - 1)
    }
}

/// The lexical model of the reduced path: starting at the anchor's
/// components, a `Normal` component appends its name, `..` removes the last
/// component (a no-op at the filesystem root), and every other kind stays.
pub(crate) open spec fn model_path<A>(
    anchor: Seq<A>,
    names: Seq<A>,
    kinds: Seq<ComponentKind>,
    k: int,
) -> Seq<A>
    decreases k,
{
    if k <= 0 {
        anchor
    } else {
        let prev = model_path(anchor, names, kinds, k - 1);
        match kinds[k - 1] {
            ComponentKind::Normal => prev.push(names[k - 1]),
            ComponentKind::ParentDir => if prev.len() > 0 {
                prev.drop_last()
            } else {
                prev
            },
            _ => prev,
        }
    }
}

/// The names of the uncancelled components.
pub(crate) open spec fn stack_names<A>(names: Seq<A>, stack: Seq<int>) -> Seq<A> {
    Seq::new(stack.len(), |t: int| names[stack[t]])
}

/// `prefix` is a component prefix of `path`.
pub(crate) open spec fn is_component_prefix<A>(prefix: Seq<A>, path: Seq<A>) -> bool {
    prefix.len() <= path.len() && path.take(prefix.len() as int) == prefix
}

/// The anchor-resolution step for one component.
pub(crate) open spec fn anchor_step_spec(c: ComponentKind) -> AnchorStep {
    match c {
        ComponentKind::ParentDir => AnchorStep::ResolveThenAscend,
        ComponentKind::CurDir => AnchorStep::Skip,
        _ => AnchorStep::Append,
    }
}

/// The spelling starts with a root or prefix component.
pub(crate) open spec fn starts_absolute(kinds: Seq<ComponentKind>) -> bool {
    kinds.len() > 0 && is_root_or_prefix(kinds[0])
}

/// Whether the plan resolves the spelling directly.
pub(crate) open spec fn is_direct(plan: AnchorResolution) -> bool {
    match plan {
        AnchorResolution::Direct => true,
        AnchorResolution::Stepwise { .. } => false,
    }
}

/// The steps of a stepwise plan (empty for a direct one).
pub(crate) open spec fn anchor_steps(plan: AnchorResolution) -> Seq<AnchorStep> {
    match plan {
        AnchorResolution::Direct => Seq::empty(),
        AnchorResolution::Stepwise { steps, .. } => steps@,
    }
}

/// The anchor-resolution plan of a spelling.
pub(crate) open spec fn is_anchor_resolution(kinds: Seq<ComponentKind>, plan: AnchorResolution) -> bool {
    match plan {
        AnchorResolution::Direct => !has_parent(kinds),
        AnchorResolution::Stepwise { from_current_dir, steps } => {
            &&& has_parent(kinds)
            &&& from_current_dir == !starts_absolute(kinds)
            &&& steps@.len() == kinds.len()
            &&& forall|i: int| 0 <= i < kinds.len() ==> steps@[i] == #[trigger] anchor_step_spec(kinds[i])
        },
    }
}

} // verus!

// ---------------------------------------------------------------------------
// Executable kernels
// ---------------------------------------------------------------------------

/// Return whether `kind` is `..`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == (kind == ComponentKind::ParentDir),
))]
fn is_parent(kind: ComponentKind) -> bool {
    matches!(kind, ComponentKind::ParentDir)
}

/// Return whether some component is `..`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == has_parent(kinds@),
))]
fn contains_parent(kinds: &[ComponentKind]) -> bool {
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= kinds.len(),
            forall|j: int| 0 <= j < index ==> kinds@[j] != ComponentKind::ParentDir,
        decreases kinds.len() - index
    ))]
    while index < kinds.len() {
        if is_parent(kinds[index]) {
            return true;
        }
        index += 1;
    }
    false
}

/// Classify a tail below the complete anchor spelling.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == tail_class_spec(kinds@),
))]
pub(crate) fn classify_tail(kinds: &[ComponentKind]) -> TailClass {
    if contains_parent(kinds) {
        TailClass::ParentContaining
    } else {
        TailClass::InAnchor
    }
}

/// Plan the below-anchor walk of a tail.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        is_beneath_plan(kinds@, r),
))]
pub(crate) fn beneath_plan(kinds: &[ComponentKind]) -> BeneathPlan {
    if kinds.is_empty() {
        return BeneathPlan::NamesAnchor;
    }
    let mut steps: Vec<BeneathStep> = Vec::new();
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= kinds.len(),
            kinds.len() > 0,
            steps@.len() == index,
            forall|i: int| 0 <= i < index ==> steps@[i] == #[trigger] beneath_step_spec(kinds@, i),
            all_normal_before(kinds@, index as int),
        decreases kinds.len() - index
    ))]
    while index < kinds.len() {
        match kinds[index] {
            ComponentKind::Normal => {
                if index + 1 == kinds.len() {
                    steps.push(BeneathStep::InspectFinal);
                } else {
                    steps.push(BeneathStep::InspectAncestor);
                }
            }
            _ => {
                steps.push(BeneathStep::RejectUnresolved);
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(steps@.last() == BeneathStep::RejectUnresolved);
                }
                return BeneathPlan::Walk(steps);
            }
        }
        index += 1;
    }
    BeneathPlan::Walk(steps)
}

/// Plan the parent-containing reduction of a tail.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r.steps@ == reduction_spec(kinds@).0,
        r.left_anchor == reduction_spec(kinds@).2,
))]
pub(crate) fn parent_reduction_plan(kinds: &[ComponentKind]) -> ReductionPlan {
    let mut steps: Vec<ReductionStep> = Vec::new();
    let mut depth: usize = 0;
    let mut left_anchor = false;
    let mut index: usize = 0;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut stack: Seq<int> = Seq::empty();
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= kinds.len(),
            reduce(kinds@, index as int) == (steps@, stack, left_anchor, false),
            depth == stack.len(),
            depth <= index,
        decreases kinds.len() - index
    ))]
    while index < kinds.len() {
        match kinds[index] {
            ComponentKind::Normal => {
                steps.push(ReductionStep::Descend);
                depth += 1;
                #[cfg(verus_keep_ghost)]
                proof! {
                    stack = stack.push(index as int);
                }
            }
            ComponentKind::CurDir => {
                steps.push(ReductionStep::Stay);
            }
            ComponentKind::ParentDir => {
                if depth > 0 {
                    steps.push(ReductionStep::InspectThenAscend);
                    depth -= 1;
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        stack = stack.drop_last();
                    }
                } else {
                    steps.push(ReductionStep::AscendAboveAnchor);
                    left_anchor = true;
                }
            }
            ComponentKind::RootDir | ComponentKind::Prefix => {
                steps.push(ReductionStep::RejectRootOrPrefix);
                #[cfg(verus_keep_ghost)]
                proof! {
                    lemma_rejected_is_final(kinds@, index + 1, kinds@.len() as int);
                }
                return ReductionPlan { steps, left_anchor };
            }
        }
        index += 1;
    }
    ReductionPlan { steps, left_anchor }
}

/// Classify a reduced parent-containing path from whether it still starts
/// with the canonical anchor and whether the reduction left the anchor.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == parent_outcome_spec(returns_to_anchor, left_anchor),
))]
pub(crate) fn parent_reduction_outcome(
    returns_to_anchor: bool,
    left_anchor: bool,
) -> ParentOutcome {
    if returns_to_anchor {
        if left_anchor {
            ParentOutcome::LeavesAndReturns
        } else {
            ParentOutcome::InAnchor
        }
    } else {
        ParentOutcome::External
    }
}

/// Return whether the spelling starts with a root or prefix component.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == starts_absolute(kinds@),
))]
fn starts_with_root_or_prefix(kinds: &[ComponentKind]) -> bool {
    if kinds.is_empty() {
        return false;
    }
    matches!(kinds[0], ComponentKind::RootDir | ComponentKind::Prefix)
}

/// Plan the resolution of an anchor spelling that may contain `..`.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        is_anchor_resolution(kinds@, r),
))]
pub(crate) fn anchor_resolution_plan(kinds: &[ComponentKind]) -> AnchorResolution {
    if !contains_parent(kinds) {
        return AnchorResolution::Direct;
    }
    let from_current_dir = !starts_with_root_or_prefix(kinds);
    let mut steps: Vec<AnchorStep> = Vec::new();
    let mut index: usize = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= kinds.len(),
            steps@.len() == index,
            forall|i: int| 0 <= i < index ==> steps@[i] == #[trigger] anchor_step_spec(kinds@[i]),
        decreases kinds.len() - index
    ))]
    while index < kinds.len() {
        let step = match kinds[index] {
            ComponentKind::ParentDir => AnchorStep::ResolveThenAscend,
            ComponentKind::CurDir => AnchorStep::Skip,
            ComponentKind::Prefix | ComponentKind::RootDir | ComponentKind::Normal => {
                AnchorStep::Append
            }
        };
        steps.push(step);
        index += 1;
    }
    AnchorResolution::Stepwise {
        from_current_dir,
        steps,
    }
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Reduction lemmas
// ---------------------------------------------------------------------------

/// Once rejected, the reduction state never changes.
proof fn lemma_rejected_is_final(kinds: Seq<ComponentKind>, k: int, j: int)
    requires
        0 <= k <= j,
        reduce(kinds, k).3,
    ensures
        reduce(kinds, j) == reduce(kinds, k),
    decreases j - k,
{
    if j > k {
        lemma_rejected_is_final(kinds, k, j - 1);
    }
}

/// The shape of the reduction state after `k` components.
proof fn lemma_reduce_shape(kinds: Seq<ComponentKind>, k: int)
    requires
        0 <= k <= kinds.len(),
    ensures
        ({
            let (steps, stack, left, rejected) = reduce(kinds, k);
            &&& steps.len() <= k
            &&& !rejected ==> steps.len() == k
            &&& rejected ==> steps.len() >= 1 && steps.last() == ReductionStep::RejectRootOrPrefix
            &&& forall|j: int| 0 <= j < steps.len() - 1 ==> !is_root_or_prefix(#[trigger] kinds[j])
            &&& !rejected ==> forall|j: int| 0 <= j < k ==> !is_root_or_prefix(#[trigger] kinds[j])
            &&& rejected ==> is_root_or_prefix(kinds[steps.len() - 1])
            &&& forall|i: int|
                0 <= i < steps.len() ==> {
                    &&& (steps[i] == ReductionStep::Descend) == (kinds[i] == ComponentKind::Normal)
                    &&& (steps[i] == ReductionStep::Stay) == (kinds[i] == ComponentKind::CurDir)
                    &&& (steps[i] == ReductionStep::InspectThenAscend || steps[i]
                        == ReductionStep::AscendAboveAnchor) == (kinds[i] == ComponentKind::ParentDir)
                    &&& (#[trigger] steps[i] == ReductionStep::RejectRootOrPrefix) == is_root_or_prefix(
                        kinds[i],
                    )
                }
            &&& stack.len() <= steps.len()
            &&& forall|t: int|
                0 <= t < stack.len() ==> 0 <= #[trigger] stack[t] < steps.len() && kinds[stack[t]]
                    == ComponentKind::Normal
            &&& forall|s: int, t: int| 0 <= s < t < stack.len() ==> stack[s] < stack[t]
        }),
    decreases k,
{
    if k > 0 {
        lemma_reduce_shape(kinds, k - 1);
        let prev = reduce(kinds, k - 1);
        if !prev.3 {
            let cur = reduce(kinds, k);
            assert forall|i: int| 0 <= i < cur.0.len() - 1 implies cur.0[i] == prev.0[i] by {}
            assert(cur.0[k - 1] == reduction_step_spec(kinds[k - 1], prev.1.len() as int));
            if reduction_step_spec(kinds[k - 1], prev.1.len() as int) == ReductionStep::Descend {
                assert forall|t: int| 0 <= t < cur.1.len() - 1 implies cur.1[t] == prev.1[t] by {}
            }
            if reduction_step_spec(kinds[k - 1], prev.1.len() as int)
                == ReductionStep::InspectThenAscend {
                assert forall|t: int| 0 <= t < cur.1.len() implies cur.1[t] == prev.1[t] by {}
            }
        }
    }
}

/// The least prefix depth is at most every prefix depth up to `k`, and is
/// attained by one of them.
proof fn lemma_min_depth(kinds: Seq<ComponentKind>, k: int)
    requires
        0 <= k,
    ensures
        min_depth(kinds, k) <= 0,
        min_depth(kinds, k) <= lexical_depth(kinds, k),
        forall|j: int| 0 <= j <= k ==> min_depth(kinds, k) <= #[trigger] lexical_depth(kinds, j),
        exists|j: int| 0 <= j <= k && min_depth(kinds, k) == #[trigger] lexical_depth(kinds, j),
    decreases k,
{
    if k > 0 {
        lemma_min_depth(kinds, k - 1);
        let w = choose|j: int| 0 <= j <= k - 1 && min_depth(kinds, k - 1) == #[trigger] lexical_depth(kinds, j);
        if lexical_depth(kinds, k) < min_depth(kinds, k - 1) {
            assert(min_depth(kinds, k) == lexical_depth(kinds, k));
        } else {
            assert(min_depth(kinds, k) == lexical_depth(kinds, w));
        }
    } else {
        assert(lexical_depth(kinds, 0) == 0);
    }
}

/// While the reduction is not rejected, the number of uncancelled
/// components is the lexical depth above the least prefix depth, it has
/// left the anchor exactly when that least depth is negative, and the
/// position of each uncancelled component is fixed by its depth.
proof fn lemma_reduce_depth(kinds: Seq<ComponentKind>, k: int)
    requires
        0 <= k <= kinds.len(),
        !reduce(kinds, k).3,
    ensures
        ({
            let (steps, stack, left, rejected) = reduce(kinds, k);
            &&& stack.len() == lexical_depth(kinds, k) - min_depth(kinds, k)
            &&& left == (min_depth(kinds, k) < 0)
            &&& forall|t: int|
                0 <= t < stack.len() ==> lexical_depth(kinds, #[trigger] stack[t] + 1) == min_depth(
                    kinds,
                    k,
                ) + t + 1
        }),
    decreases k,
{
    if k > 0 {
        lemma_rejected_is_final_or_not(kinds, k - 1, k);
        lemma_reduce_depth(kinds, k - 1);
        lemma_min_depth(kinds, k - 1);
        lemma_reduce_shape(kinds, k - 1);
        let prev = reduce(kinds, k - 1);
        let cur = reduce(kinds, k);
        let c = kinds[k - 1];
        assert(lexical_depth(kinds, k) == lexical_depth(kinds, k - 1) + depth_delta(c));
        if c == ComponentKind::Normal {
            assert(min_depth(kinds, k) == min_depth(kinds, k - 1));
            assert forall|t: int| 0 <= t < cur.1.len() implies lexical_depth(
                kinds,
                #[trigger] cur.1[t] + 1,
            ) == min_depth(kinds, k) + t + 1 by {
                if t < prev.1.len() {
                    assert(cur.1[t] == prev.1[t]);
                }
            }
        } else if c == ComponentKind::ParentDir {
            if prev.1.len() > 0 {
                assert(min_depth(kinds, k) == min_depth(kinds, k - 1));
                assert forall|t: int| 0 <= t < cur.1.len() implies lexical_depth(
                    kinds,
                    #[trigger] cur.1[t] + 1,
                ) == min_depth(kinds, k) + t + 1 by {
                    assert(cur.1[t] == prev.1[t]);
                }
            } else {
                assert(min_depth(kinds, k) == min_depth(kinds, k - 1) - 1);
            }
        } else {
            assert(min_depth(kinds, k) == min_depth(kinds, k - 1));
        }
    }
}

/// Rejection at `k` implies rejection at every later prefix length.
proof fn lemma_rejected_monotone(kinds: Seq<ComponentKind>, k: int, j: int)
    requires
        0 <= k <= j,
        reduce(kinds, k).3,
    ensures
        reduce(kinds, j).3,
{
    lemma_rejected_is_final(kinds, k, j);
}

/// The lexical model of the reduced path is the anchor, shortened by the
/// depth the reduction climbed above it, followed by the uncancelled names.
proof fn lemma_model_path<A>(anchor: Seq<A>, names: Seq<A>, kinds: Seq<ComponentKind>, k: int)
    requires
        0 <= k <= kinds.len(),
        !reduce(kinds, k).3,
    ensures
        ({
            let keep = if anchor.len() + min_depth(kinds, k) > 0 {
                anchor.len() + min_depth(kinds, k)
            } else {
                0
            };
            model_path(anchor, names, kinds, k) == anchor.take(keep) + stack_names(
                names,
                reduce(kinds, k).1,
            )
        }),
    decreases k,
{
    if k == 0 {
        assert(anchor.take(anchor.len() as int) =~= anchor);
        assert(anchor.take(anchor.len() as int) + stack_names(names, Seq::<int>::empty()) =~= anchor);
    } else {
        lemma_rejected_is_final_or_not(kinds, k - 1, k);
        lemma_model_path(anchor, names, kinds, k - 1);
        lemma_reduce_depth(kinds, k - 1);
        lemma_reduce_depth(kinds, k);
        lemma_min_depth(kinds, k - 1);
        let prev = reduce(kinds, k - 1);
        let cur = reduce(kinds, k);
        let c = kinds[k - 1];
        let keep0 = if anchor.len() + min_depth(kinds, k - 1) > 0 {
            anchor.len() + min_depth(kinds, k - 1)
        } else {
            0
        };
        let base = anchor.take(keep0);
        let p = model_path(anchor, names, kinds, k - 1);
        assert(p == base + stack_names(names, prev.1));
        if c == ComponentKind::Normal {
            assert(min_depth(kinds, k) == min_depth(kinds, k - 1));
            assert(stack_names(names, cur.1) =~= stack_names(names, prev.1).push(names[k - 1]));
            assert(base + stack_names(names, cur.1) =~= (base + stack_names(names, prev.1)).push(
                names[k - 1],
            ));
        } else if c == ComponentKind::ParentDir {
            if prev.1.len() > 0 {
                assert(min_depth(kinds, k) == min_depth(kinds, k - 1));
                assert(stack_names(names, cur.1) =~= stack_names(names, prev.1).drop_last());
                assert(base + stack_names(names, cur.1) =~= (base + stack_names(
                    names,
                    prev.1,
                )).drop_last());
            } else {
                assert(min_depth(kinds, k) == min_depth(kinds, k - 1) - 1);
                assert(stack_names(names, prev.1) =~= Seq::<A>::empty());
                assert(stack_names(names, cur.1) =~= Seq::<A>::empty());
                assert(p =~= base);
                if keep0 > 0 {
                    assert(base.drop_last() =~= anchor.take(keep0 - 1));
                }
            }
        } else {
            assert(min_depth(kinds, k) == min_depth(kinds, k - 1));
        }
    }
}

// ---------------------------------------------------------------------------
// Contract theorems
// ---------------------------------------------------------------------------

/// The reduction plan has one step per component up to and including the
/// first root or prefix component, which is rejected before any later step:
/// it is rejected exactly when some component is a root or prefix, and
/// otherwise covers every component. `Normal` components descend, `.` stays,
/// and only `..` ascends.
pub(crate) proof fn theorem_plan_follows_components(kinds: Seq<ComponentKind>)
    ensures
        ({
            let (steps, stack, left, rejected) = reduction_spec(kinds);
            &&& rejected == (exists|i: int| 0 <= i < kinds.len() && #[trigger] is_root_or_prefix(kinds[i]))
            &&& !rejected ==> steps.len() == kinds.len()
            &&& rejected ==> {
                &&& 1 <= steps.len() <= kinds.len()
                &&& steps.last() == ReductionStep::RejectRootOrPrefix
                &&& is_root_or_prefix(kinds[steps.len() - 1])
                &&& forall|j: int| 0 <= j < steps.len() - 1 ==> !is_root_or_prefix(#[trigger] kinds[j])
            }
            &&& forall|i: int|
                0 <= i < steps.len() ==> {
                    &&& (steps[i] == ReductionStep::Descend) == (kinds[i] == ComponentKind::Normal)
                    &&& (steps[i] == ReductionStep::Stay) == (kinds[i] == ComponentKind::CurDir)
                    &&& (steps[i] == ReductionStep::InspectThenAscend || steps[i]
                        == ReductionStep::AscendAboveAnchor) == (kinds[i] == ComponentKind::ParentDir)
                    &&& (#[trigger] steps[i] == ReductionStep::RejectRootOrPrefix) == is_root_or_prefix(
                        kinds[i],
                    )
                }
        }),
{
    let n = kinds.len() as int;
    lemma_reduce_shape(kinds, n);
    let (steps, stack, left, rejected) = reduction_spec(kinds);
    if rejected {
        assert(is_root_or_prefix(kinds[steps.len() - 1]));
    }
    if exists|i: int| 0 <= i < kinds.len() && #[trigger] is_root_or_prefix(kinds[i]) {
        let i = choose|i: int| 0 <= i < kinds.len() && #[trigger] is_root_or_prefix(kinds[i]);
        if !rejected {
            assert(!is_root_or_prefix(kinds[i]));
        }
    }
}

/// A `..` with an uncancelled component to erase inspects exactly that
/// component before ascending: the erased component comes earlier, is
/// `Normal`, and the components between them are balanced (the `..` returns
/// to the depth just after the erased component). A `..` with nothing to
/// erase ascends above the anchor and records that the path left it.
pub(crate) proof fn theorem_every_erased_component_is_inspected(kinds: Seq<ComponentKind>, i: int)
    requires
        0 <= i < kinds.len(),
        kinds[i] == ComponentKind::ParentDir,
        !reduce(kinds, i + 1).3,
    ensures
        ({
            let stack = reduce(kinds, i).1;
            let step = reduction_spec(kinds).0[i];
            &&& (stack.len() > 0) == (step == ReductionStep::InspectThenAscend)
            &&& stack.len() > 0 ==> {
                let j = stack.last();
                &&& 0 <= j < i
                &&& kinds[j] == ComponentKind::Normal
                &&& lexical_depth(kinds, j + 1) == lexical_depth(kinds, i)
                &&& lexical_depth(kinds, i + 1) == lexical_depth(kinds, j)
            }
            &&& stack.len() == 0 ==> step == ReductionStep::AscendAboveAnchor && reduction_spec(
                kinds,
            ).2
        }),
{
    let n = kinds.len() as int;
    lemma_rejected_is_final_or_not(kinds, i, i + 1);
    lemma_reduce_shape(kinds, i);
    lemma_reduce_depth(kinds, i);
    lemma_reduce_shape(kinds, n);
    lemma_prefix_steps(kinds, i + 1, n);
    let prev = reduce(kinds, i);
    assert(reduce(kinds, i + 1).0[i] == reduction_step_spec(kinds[i], prev.1.len() as int));
    if prev.1.len() > 0 {
        let t = prev.1.len() - 1;
        let j = prev.1[t];
        assert(lexical_depth(kinds, j + 1) == min_depth(kinds, i) + t + 1);
        assert(lexical_depth(kinds, j + 1) == lexical_depth(kinds, j) + 1);
    } else {
        lemma_left_is_sticky(kinds, i + 1, n);
    }
}

/// The reduction leaves the anchor exactly when some prefix of the tail has
/// negative lexical depth. A tail that never leaves has nonnegative depth at
/// every prefix and keeps exactly its lexical depth of uncancelled
/// components, all of them `Normal` and in their original order.
pub(crate) proof fn theorem_reduction_tracks_lexical_depth(kinds: Seq<ComponentKind>)
    requires
        !reduction_spec(kinds).3,
    ensures
        ({
            let (steps, stack, left, rejected) = reduction_spec(kinds);
            let n = kinds.len() as int;
            &&& left == (exists|j: int| 0 <= j <= n && #[trigger] lexical_depth(kinds, j) < 0)
            &&& !left ==> {
                &&& forall|j: int| 0 <= j <= n ==> #[trigger] lexical_depth(kinds, j) >= 0
                &&& stack.len() == lexical_depth(kinds, n)
            }
            &&& forall|t: int|
                0 <= t < stack.len() ==> 0 <= #[trigger] stack[t] < n && kinds[stack[t]]
                    == ComponentKind::Normal
            &&& forall|s: int, t: int| 0 <= s < t < stack.len() ==> stack[s] < stack[t]
        }),
{
    let n = kinds.len() as int;
    lemma_reduce_shape(kinds, n);
    lemma_reduce_depth(kinds, n);
    lemma_min_depth(kinds, n);
    let left = reduction_spec(kinds).2;
    if left {
        let w = choose|j: int| 0 <= j <= n && min_depth(kinds, n) == #[trigger] lexical_depth(kinds, j);
        assert(lexical_depth(kinds, w) < 0);
    } else {
        assert forall|j: int| 0 <= j <= n implies #[trigger] lexical_depth(kinds, j) >= 0 by {
            assert(min_depth(kinds, n) <= lexical_depth(kinds, j));
        }
        let w = choose|j: int| 0 <= j <= n && min_depth(kinds, n) == #[trigger] lexical_depth(kinds, j);
        assert(min_depth(kinds, n) == 0);
    }
}

/// In the lexical model, a reduction that never left the anchor ends at the
/// anchor followed by exactly its uncancelled `Normal` components: the
/// reduced path still starts with the anchor, so it is classified in-anchor,
/// and the tail below the anchor is those components, one per level of
/// lexical depth.
pub(crate) proof fn theorem_never_left_is_in_anchor<A>(
    anchor: Seq<A>,
    names: Seq<A>,
    kinds: Seq<ComponentKind>,
)
    requires
        !reduction_spec(kinds).3,
        !reduction_spec(kinds).2,
    ensures
        ({
            let n = kinds.len() as int;
            let path = model_path(anchor, names, kinds, n);
            let stack = reduction_spec(kinds).1;
            &&& path == anchor + stack_names(names, stack)
            &&& is_component_prefix(anchor, path)
            &&& path.skip(anchor.len() as int) == stack_names(names, stack)
            &&& parent_outcome_spec(is_component_prefix(anchor, path), false) == ParentOutcome::InAnchor
            &&& stack_names(names, stack).len() == lexical_depth(kinds, n)
            &&& forall|t: int| 0 <= t < stack.len() ==> kinds[#[trigger] stack[t]] == ComponentKind::Normal
        }),
{
    let n = kinds.len() as int;
    lemma_model_path(anchor, names, kinds, n);
    lemma_reduce_depth(kinds, n);
    lemma_min_depth(kinds, n);
    theorem_reduction_tracks_lexical_depth(kinds);
    assert(min_depth(kinds, n) == 0);
    assert(anchor.take(anchor.len() as int) =~= anchor);
    let path = model_path(anchor, names, kinds, n);
    let sn = stack_names(names, reduction_spec(kinds).1);
    assert(path.take(anchor.len() as int) =~= anchor);
    assert(path.skip(anchor.len() as int) =~= sn);
}

/// In the lexical model, a reduction that left the anchor ends at the
/// ancestor as many levels above the anchor as it climbed (the anchor's
/// first `anchor.len() + min_depth` components, or the filesystem root)
/// followed by its uncancelled names. Whether or not that path lexically
/// returns beneath the anchor, it is never in-anchor: it is ambiguous (and
/// rejected) when it returns and external otherwise.
pub(crate) proof fn theorem_left_path_is_never_in_anchor<A>(
    anchor: Seq<A>,
    names: Seq<A>,
    kinds: Seq<ComponentKind>,
)
    requires
        !reduction_spec(kinds).3,
        reduction_spec(kinds).2,
    ensures
        ({
            let n = kinds.len() as int;
            let path = model_path(anchor, names, kinds, n);
            let keep = if anchor.len() + min_depth(kinds, n) > 0 {
                anchor.len() + min_depth(kinds, n)
            } else {
                0
            };
            let returns = is_component_prefix(anchor, path);
            &&& min_depth(kinds, n) < 0
            &&& path == anchor.take(keep) + stack_names(names, reduction_spec(kinds).1)
            &&& parent_outcome_spec(returns, true) == (if returns {
                ParentOutcome::LeavesAndReturns
            } else {
                ParentOutcome::External
            })
            &&& parent_outcome_spec(returns, true) != ParentOutcome::InAnchor
        }),
{
    let n = kinds.len() as int;
    lemma_model_path(anchor, names, kinds, n);
    lemma_reduce_depth(kinds, n);
}

/// An in-anchor outcome never left the anchor, so (by
/// `theorem_reduction_tracks_lexical_depth`) its tail has nonnegative
/// lexical depth at every prefix.
pub(crate) proof fn theorem_in_anchor_outcome_never_left(returns_to_anchor: bool, left_anchor: bool)
    ensures
        parent_outcome_spec(returns_to_anchor, left_anchor) == ParentOutcome::InAnchor ==> returns_to_anchor
            && !left_anchor,
        parent_outcome_spec(returns_to_anchor, left_anchor) == ParentOutcome::External ==> !returns_to_anchor,
{
}

/// A path that leaves the anchor and then lexically returns to it is
/// rejected as ambiguous, and a path that left is never in-anchor.
pub(crate) proof fn theorem_leaves_and_returns_fails_closed(returns_to_anchor: bool)
    ensures
        parent_outcome_spec(true, true) == ParentOutcome::LeavesAndReturns,
        parent_outcome_spec(returns_to_anchor, true) != ParentOutcome::InAnchor,
{
}

/// The below-anchor walk inspects only `Normal` components, inspects every
/// component before the last as an ancestor directory, and reaches the
/// final entry exactly when every component is `Normal`; otherwise it
/// rejects at the first component that is not.
pub(crate) proof fn theorem_beneath_walk_accepts_only_normal_tails(
    kinds: Seq<ComponentKind>,
    steps: Seq<BeneathStep>,
)
    requires
        is_beneath_walk(kinds, steps),
    ensures
        forall|i: int|
            0 <= i < steps.len() && #[trigger] steps[i] != BeneathStep::RejectUnresolved ==> kinds[i]
                == ComponentKind::Normal,
        forall|i: int|
            0 <= i < steps.len() - 1 ==> #[trigger] steps[i] == BeneathStep::InspectAncestor,
        (steps.last() == BeneathStep::InspectFinal) == all_normal_before(kinds, kinds.len() as int),
        steps.last() == BeneathStep::InspectFinal ==> steps.len() == kinds.len(),
        steps.last() == BeneathStep::RejectUnresolved ==> kinds[steps.len() - 1]
            != ComponentKind::Normal,
{
    let m = steps.len() - 1;
    assert(steps[m] == beneath_step_spec(kinds, m));
    assert forall|i: int| 0 <= i < m implies #[trigger] steps[i] == BeneathStep::InspectAncestor by {
        assert(steps[i] == beneath_step_spec(kinds, i));
        assert(kinds[i] == ComponentKind::Normal);
    }
    if steps.last() == BeneathStep::InspectFinal {
        assert(m + 1 == kinds.len());
        assert forall|j: int| 0 <= j < kinds.len() implies kinds[j] == ComponentKind::Normal by {
            if j == m {
                assert(steps[m] == beneath_step_spec(kinds, m));
            }
        }
    }
    if all_normal_before(kinds, kinds.len() as int) {
        assert(kinds[m] == ComponentKind::Normal);
        assert(steps.len() == kinds.len());
    }
}

/// An in-anchor tail has no `..` component, and a parent-containing one has
/// at least one.
pub(crate) proof fn theorem_tail_class(kinds: Seq<ComponentKind>)
    ensures
        tail_class_spec(kinds) == TailClass::InAnchor ==> forall|i: int|
            0 <= i < kinds.len() ==> kinds[i] != ComponentKind::ParentDir,
        tail_class_spec(kinds) == TailClass::ParentContaining ==> exists|i: int|
            0 <= i < kinds.len() && kinds[i] == ComponentKind::ParentDir,
{
}

/// Anchor resolution is direct exactly when the spelling has no `..`;
/// otherwise it starts from the current directory exactly when the spelling
/// is relative, and resolves the pending prefix before every `..`, appending
/// every other component except `.`.
pub(crate) proof fn theorem_anchor_resolution_order(kinds: Seq<ComponentKind>, plan: AnchorResolution)
    requires
        is_anchor_resolution(kinds, plan),
    ensures
        is_direct(plan) == !has_parent(kinds),
        !is_direct(plan) ==> anchor_steps(plan).len() == kinds.len(),
        !is_direct(plan) ==> forall|i: int|
            0 <= i < kinds.len() ==> ((#[trigger] kinds[i] == ComponentKind::ParentDir) == (anchor_steps(
                plan,
            )[i] == AnchorStep::ResolveThenAscend)),
        !is_direct(plan) ==> forall|i: int|
            0 <= i < kinds.len() ==> ((#[trigger] kinds[i] == ComponentKind::CurDir) == (anchor_steps(
                plan,
            )[i] == AnchorStep::Skip)),
{
    if !is_direct(plan) {
        assert forall|i: int| 0 <= i < kinds.len() implies ((#[trigger] kinds[i]
            == ComponentKind::ParentDir) == (anchor_steps(plan)[i] == AnchorStep::ResolveThenAscend)) by {
            assert(anchor_steps(plan)[i] == anchor_step_spec(kinds[i]));
        }
        assert forall|i: int| 0 <= i < kinds.len() implies ((#[trigger] kinds[i]
            == ComponentKind::CurDir) == (anchor_steps(plan)[i] == AnchorStep::Skip)) by {
            assert(anchor_steps(plan)[i] == anchor_step_spec(kinds[i]));
        }
    }
}

// ---------------------------------------------------------------------------
// Helper lemmas for the theorems
// ---------------------------------------------------------------------------

/// Not rejected at `j` implies not rejected at any shorter prefix.
proof fn lemma_rejected_is_final_or_not(kinds: Seq<ComponentKind>, k: int, j: int)
    requires
        0 <= k <= j,
        !reduce(kinds, j).3,
    ensures
        !reduce(kinds, k).3,
{
    if reduce(kinds, k).3 {
        lemma_rejected_is_final(kinds, k, j);
    }
}

/// The plan for a prefix is a prefix of every longer plan.
proof fn lemma_prefix_steps(kinds: Seq<ComponentKind>, k: int, j: int)
    requires
        0 <= k <= j,
    ensures
        reduce(kinds, k).0.len() <= reduce(kinds, j).0.len(),
        forall|i: int| 0 <= i < reduce(kinds, k).0.len() ==> reduce(kinds, j).0[i] == #[trigger] reduce(kinds, k).0[i],
    decreases j - k,
{
    if j > k {
        lemma_prefix_steps(kinds, k, j - 1);
    }
}

/// Once the reduction has left the anchor, it stays recorded.
proof fn lemma_left_is_sticky(kinds: Seq<ComponentKind>, k: int, j: int)
    requires
        0 <= k <= j,
        reduce(kinds, k).2,
    ensures
        reduce(kinds, j).2,
    decreases j - k,
{
    if j > k {
        lemma_left_is_sticky(kinds, k, j - 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use ComponentKind::{CurDir, Normal, ParentDir, Prefix, RootDir};

    /// The reduction loop exactly as `guide_input.rs` stated it before it
    /// moved into this kernel, over component kinds.
    fn reference_reduction(kinds: &[ComponentKind]) -> (Vec<ReductionStep>, bool) {
        let mut steps = Vec::new();
        let mut stack = 0usize;
        let mut left = false;
        for kind in kinds {
            match kind {
                Normal => {
                    stack += 1;
                    steps.push(ReductionStep::Descend);
                }
                CurDir => steps.push(ReductionStep::Stay),
                ParentDir => {
                    if stack > 0 {
                        stack -= 1;
                        steps.push(ReductionStep::InspectThenAscend);
                    } else {
                        left = true;
                        steps.push(ReductionStep::AscendAboveAnchor);
                    }
                }
                RootDir | Prefix => {
                    steps.push(ReductionStep::RejectRootOrPrefix);
                    return (steps, left);
                }
            }
        }
        (steps, left)
    }

    fn all_tails(max_len: usize) -> Vec<Vec<ComponentKind>> {
        let alphabet = [Normal, ParentDir, CurDir, RootDir, Prefix];
        let mut all = vec![Vec::new()];
        let mut frontier = vec![Vec::new()];
        for _ in 0..max_len {
            let mut next = Vec::new();
            for tail in &frontier {
                for kind in alphabet {
                    let mut longer = tail.clone();
                    longer.push(kind);
                    next.push(longer);
                }
            }
            all.extend(next.iter().cloned());
            frontier = next;
        }
        all
    }

    #[test]
    fn reduction_matches_the_reference_on_every_short_tail() {
        for tail in all_tails(6) {
            let plan = parent_reduction_plan(&tail);
            let (steps, left) = reference_reduction(&tail);
            assert_eq!(plan.steps, steps, "{tail:?}");
            assert_eq!(plan.left_anchor, left, "{tail:?}");
        }
    }

    #[test]
    fn reduction_names_each_step() {
        let plan = parent_reduction_plan(&[Normal, ParentDir, ParentDir, Normal, CurDir]);
        assert_eq!(
            plan.steps,
            [
                ReductionStep::Descend,
                ReductionStep::InspectThenAscend,
                ReductionStep::AscendAboveAnchor,
                ReductionStep::Descend,
                ReductionStep::Stay,
            ]
        );
        assert!(plan.left_anchor);
        let plan = parent_reduction_plan(&[Normal, RootDir, ParentDir]);
        assert_eq!(
            plan.steps,
            [ReductionStep::Descend, ReductionStep::RejectRootOrPrefix]
        );
        assert!(!plan.left_anchor);
        let plan = parent_reduction_plan(&[Prefix]);
        assert_eq!(plan.steps, [ReductionStep::RejectRootOrPrefix]);
        let plan = parent_reduction_plan(&[]);
        assert!(plan.steps.is_empty() && !plan.left_anchor);
    }

    #[test]
    fn outcome_follows_the_production_precedence() {
        assert_eq!(
            parent_reduction_outcome(true, false),
            ParentOutcome::InAnchor
        );
        assert_eq!(
            parent_reduction_outcome(true, true),
            ParentOutcome::LeavesAndReturns
        );
        assert_eq!(
            parent_reduction_outcome(false, false),
            ParentOutcome::External
        );
        assert_eq!(
            parent_reduction_outcome(false, true),
            ParentOutcome::External
        );
    }

    #[test]
    fn tail_class_detects_any_parent() {
        for tail in all_tails(4) {
            let expected = if tail.contains(&ParentDir) {
                TailClass::ParentContaining
            } else {
                TailClass::InAnchor
            };
            assert_eq!(classify_tail(&tail), expected, "{tail:?}");
        }
    }

    /// The below-anchor walk exactly as `guide_input.rs` stated it before it
    /// moved into this kernel.
    fn reference_beneath(kinds: &[ComponentKind]) -> Option<Vec<BeneathStep>> {
        if kinds.is_empty() {
            return None;
        }
        let mut steps = Vec::new();
        for (index, kind) in kinds.iter().enumerate() {
            if *kind != Normal {
                steps.push(BeneathStep::RejectUnresolved);
                return Some(steps);
            }
            steps.push(if index + 1 == kinds.len() {
                BeneathStep::InspectFinal
            } else {
                BeneathStep::InspectAncestor
            });
        }
        Some(steps)
    }

    #[test]
    fn beneath_plan_matches_the_reference_on_every_short_tail() {
        for tail in all_tails(5) {
            let actual = match beneath_plan(&tail) {
                BeneathPlan::NamesAnchor => None,
                BeneathPlan::Walk(steps) => Some(steps),
            };
            assert_eq!(actual, reference_beneath(&tail), "{tail:?}");
        }
        match beneath_plan(&[Normal, Normal]) {
            BeneathPlan::Walk(steps) => assert_eq!(
                steps,
                [BeneathStep::InspectAncestor, BeneathStep::InspectFinal]
            ),
            BeneathPlan::NamesAnchor => panic!("a nonempty tail names an entry"),
        }
    }

    /// Anchor resolution exactly as `guide_input.rs` stated it before it
    /// moved into this kernel.
    fn reference_anchor(kinds: &[ComponentKind]) -> Option<(bool, Vec<AnchorStep>)> {
        if !kinds.contains(&ParentDir) {
            return None;
        }
        let from_current_dir = !matches!(kinds.first(), Some(Prefix | RootDir));
        let steps = kinds
            .iter()
            .map(|kind| match kind {
                ParentDir => AnchorStep::ResolveThenAscend,
                CurDir => AnchorStep::Skip,
                Prefix | RootDir | Normal => AnchorStep::Append,
            })
            .collect();
        Some((from_current_dir, steps))
    }

    #[test]
    fn anchor_resolution_matches_the_reference_on_every_short_spelling() {
        for spelling in all_tails(5) {
            let actual = match anchor_resolution_plan(&spelling) {
                AnchorResolution::Direct => None,
                AnchorResolution::Stepwise {
                    from_current_dir,
                    steps,
                } => Some((from_current_dir, steps)),
            };
            assert_eq!(actual, reference_anchor(&spelling), "{spelling:?}");
        }
    }
}
