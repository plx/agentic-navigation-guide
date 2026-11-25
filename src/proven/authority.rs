//! Verified guide-authority routing and no-follow entry checks.
//!
//! These kernels make the pure decisions of `guide_input.rs` for the
//! `docs/v0.2-contract.md` rules in "Trust anchors and path provenance" and
//! "Guide-file input", and the matching ancestor rule of "Filesystem output
//! destinations" in `cli/output.rs`. Glue classifies the candidate with
//! `Path::strip_prefix` and [`crate::proven::anchor::classify_tail`], reads
//! entry metadata without following links, and passes only the resulting
//! facts; it maps each verdict to the existing diagnostic:
//!
//! - [`guide_route`]: which validation a candidate guide path receives, from
//!   the authority that selected it (an implicit name or an explicit path)
//!   and where it lies against the complete anchor spelling. An in-anchor
//!   candidate is always walked below the anchor, with the exact-entry check
//!   added for implicit authority; an external or parent-containing candidate
//!   is checked or reduced only under explicit authority and otherwise
//!   rejected;
//! - [`walk_entry_verdict`]: one entry inspected by the below-anchor walk is
//!   rejected when it is a link or reparse point, and an ancestor (not the
//!   final entry) is also rejected when it is not a directory;
//!   [`directory_entry_verdict`] is the ancestor rule alone, used for the
//!   component that a `..` erases and for output ancestors below the
//!   generation root;
//! - [`classify_entry`]: a final entry inspected without following it is a
//!   link, or else a regular file, a directory, or another entry, in that
//!   precedence. A guide is read only from a regular file, and an existing
//!   output destination is reported by this class.
//!
//! The theorems below state the contract-level consequences:
//!
//! - `theorem_implicit_authority_stays_beneath`: implicit authority never
//!   selects the external or parent-reduction route; it walks exactly the
//!   in-anchor candidates, always with the exact-entry check, and rejects
//!   every other candidate;
//! - `theorem_explicit_authority_routes_by_place`: explicit authority is
//!   never rejected at dispatch and selects the walk, the external final-entry
//!   check, or the parent reduction exactly by the candidate's place;
//! - `theorem_in_anchor_candidate_is_always_walked` and
//!   `theorem_only_explicit_authority_leaves_the_walk`: no authority bypasses
//!   the below-anchor walk for an in-anchor candidate, and only explicit
//!   authority reaches a route that is not the walk or a rejection;
//! - `theorem_walk_step_rejects_links_and_non_directory_ancestors`: every
//!   walk step rejects a link-like entry, rejects a non-final entry that is
//!   not a directory, and accepts exactly the other entries;
//! - `theorem_walk_accepts_only_real_directory_chains`: composed with the
//!   verified below-anchor plan, a walk passes every step exactly when every
//!   tail component is an ordinary name, no inspected entry is a link, and
//!   every entry before the last is a directory;
//! - `theorem_link_classification_precedes_type`: a link-like entry is
//!   classified as a link whatever type flags it also carries, and an entry
//!   is a regular file exactly when it is not link-like and is a file.

#[cfg(verus_keep_ghost)]
use crate::proven::anchor::{
    all_normal_before, is_beneath_walk, theorem_beneath_walk_accepts_only_normal_tails,
    BeneathStep, ComponentKind,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Who selected a guide path.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GuideAuthority {
    /// A default path or one derived from a guide-name selector.
    Implicit,
    /// `--guide` or `AGENTIC_NAVIGATION_GUIDE_PATH`.
    Explicit,
}

/// Where a candidate guide path lies against the complete anchor spelling,
/// with its tail below that spelling when it has one.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CandidatePlace<T> {
    /// Below the anchor spelling, and the tail has no `..` component.
    InAnchor(T),
    /// Not below the anchor spelling.
    External,
    /// Below the anchor spelling, and the tail has a `..` component.
    ParentContaining(T),
}

/// The validation a candidate guide path receives.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GuideRoute<T> {
    /// Walk the tail below the anchor without following links; then, when
    /// `exact_implicit_entry` is set, require a directory entry whose name
    /// matches the implicit name exactly.
    WalkBeneath { tail: T, exact_implicit_entry: bool },
    /// Inspect the explicitly selected external final entry without
    /// following it.
    CheckExternalEntry,
    /// Reduce the explicitly selected parent-containing tail from the
    /// canonical anchor.
    ReduceParents(T),
    /// An implicit guide outside the anchor or with `..`: reject it.
    RejectImplicitEscape,
}

/// What a no-follow inspection of one entry observed.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EntryFacts {
    /// A symbolic link, or on Windows any reparse point.
    pub(crate) link_like: bool,
    /// The entry is a directory.
    pub(crate) is_dir: bool,
    /// The entry is a regular file.
    pub(crate) is_file: bool,
}

/// The verdict on one entry inspected by a below-anchor walk.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WalkVerdict {
    /// The walk continues past this entry.
    Continue,
    /// The entry is a link or reparse point.
    LinkLike,
    /// The entry must be a directory and is not.
    NotDirectory,
}

/// The class of a final entry inspected without following it.
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryClass {
    /// A link or reparse point, whatever it would resolve to.
    LinkLike,
    /// A regular file.
    RegularFile,
    /// A directory.
    Directory,
    /// Any other entry, such as a device, FIFO, or socket.
    Other,
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Specifications
// ---------------------------------------------------------------------------

/// The route of a candidate: an in-anchor candidate is walked (with the
/// exact-entry check exactly under implicit authority); an external or
/// parent-containing candidate is checked or reduced under explicit
/// authority and rejected under implicit authority.
pub(crate) open spec fn guide_route_spec<T>(authority: GuideAuthority, place: CandidatePlace<T>) -> GuideRoute<T> {
    match place {
        CandidatePlace::InAnchor(tail) => GuideRoute::WalkBeneath {
            tail,
            exact_implicit_entry: authority == GuideAuthority::Implicit,
        },
        CandidatePlace::External => if authority == GuideAuthority::Explicit {
            GuideRoute::CheckExternalEntry
        } else {
            GuideRoute::RejectImplicitEscape
        },
        CandidatePlace::ParentContaining(tail) => if authority == GuideAuthority::Explicit {
            GuideRoute::ReduceParents(tail)
        } else {
            GuideRoute::RejectImplicitEscape
        },
    }
}

/// One walk step: a link is rejected first; then an entry that is not the
/// final one must be a directory.
pub(crate) open spec fn walk_entry_spec(final_entry: bool, facts: EntryFacts) -> WalkVerdict {
    if facts.link_like {
        WalkVerdict::LinkLike
    } else if !final_entry && !facts.is_dir {
        WalkVerdict::NotDirectory
    } else {
        WalkVerdict::Continue
    }
}

/// The class of a final entry: a link first, then a file, then a directory.
pub(crate) open spec fn entry_class_spec(facts: EntryFacts) -> EntryClass {
    if facts.link_like {
        EntryClass::LinkLike
    } else if facts.is_file {
        EntryClass::RegularFile
    } else if facts.is_dir {
        EntryClass::Directory
    } else {
        EntryClass::Other
    }
}

/// The walk passes every planned step: no step rejects an unresolved
/// component, and every inspected entry lets the walk continue.
pub(crate) open spec fn walk_passes(steps: Seq<BeneathStep>, facts: Seq<EntryFacts>) -> bool {
    &&& facts.len() >= steps.len()
    &&& forall|i: int| 0 <= i < steps.len() ==> #[trigger] steps[i] != BeneathStep::RejectUnresolved
    &&& forall|i: int|
        0 <= i < steps.len() ==> walk_entry_spec(#[trigger] steps[i] == BeneathStep::InspectFinal, facts[i])
            == WalkVerdict::Continue
}

/// A real directory chain: every component is an ordinary name, no entry is
/// link-like, and every entry before the last is a directory.
pub(crate) open spec fn real_directory_chain(kinds: Seq<ComponentKind>, facts: Seq<EntryFacts>) -> bool {
    &&& facts.len() >= kinds.len()
    &&& all_normal_before(kinds, kinds.len() as int)
    &&& forall|i: int| 0 <= i < kinds.len() ==> !(#[trigger] facts[i]).link_like
    &&& forall|i: int| 0 <= i < kinds.len() - 1 ==> (#[trigger] facts[i]).is_dir
}

} // verus!

// ---------------------------------------------------------------------------
// Executable kernels
// ---------------------------------------------------------------------------

/// Decide which validation a candidate guide path receives.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == guide_route_spec(authority, place),
))]
pub(crate) fn guide_route<T>(authority: GuideAuthority, place: CandidatePlace<T>) -> GuideRoute<T> {
    match place {
        CandidatePlace::InAnchor(tail) => GuideRoute::WalkBeneath {
            tail,
            exact_implicit_entry: matches!(authority, GuideAuthority::Implicit),
        },
        CandidatePlace::External => match authority {
            GuideAuthority::Explicit => GuideRoute::CheckExternalEntry,
            GuideAuthority::Implicit => GuideRoute::RejectImplicitEscape,
        },
        CandidatePlace::ParentContaining(tail) => match authority {
            GuideAuthority::Explicit => GuideRoute::ReduceParents(tail),
            GuideAuthority::Implicit => GuideRoute::RejectImplicitEscape,
        },
    }
}

/// Decide one entry inspected by the below-anchor walk.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == walk_entry_spec(final_entry, facts),
))]
pub(crate) fn walk_entry_verdict(final_entry: bool, facts: EntryFacts) -> WalkVerdict {
    if facts.link_like {
        WalkVerdict::LinkLike
    } else if !final_entry && !facts.is_dir {
        WalkVerdict::NotDirectory
    } else {
        WalkVerdict::Continue
    }
}

/// Decide an entry that must be a real directory: a walk ancestor, a
/// component erased by `..`, or an output ancestor below the generation
/// root.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == walk_entry_spec(false, facts),
))]
pub(crate) fn directory_entry_verdict(facts: EntryFacts) -> WalkVerdict {
    walk_entry_verdict(false, facts)
}

/// Classify a final entry inspected without following it.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == entry_class_spec(facts),
))]
pub(crate) fn classify_entry(facts: EntryFacts) -> EntryClass {
    if facts.link_like {
        EntryClass::LinkLike
    } else if facts.is_file {
        EntryClass::RegularFile
    } else if facts.is_dir {
        EntryClass::Directory
    } else {
        EntryClass::Other
    }
}

#[cfg(verus_keep_ghost)]
verus! {

// ---------------------------------------------------------------------------
// Contract theorems
// ---------------------------------------------------------------------------

/// Implicit authority never selects the external or parent-reduction route:
/// it walks exactly the in-anchor candidates, always adding the exact-entry
/// check, and rejects every other candidate.
pub(crate) proof fn theorem_implicit_authority_stays_beneath<T>(place: CandidatePlace<T>)
    ensures
        ({
            let route = guide_route_spec(GuideAuthority::Implicit, place);
            &&& !(route is CheckExternalEntry)
            &&& !(route is ReduceParents)
            &&& (route is WalkBeneath) == (place is InAnchor)
            &&& match route {
                GuideRoute::WalkBeneath { exact_implicit_entry, .. } => exact_implicit_entry,
                _ => true,
            }
            &&& !(place is InAnchor) ==> route is RejectImplicitEscape
        }),
{
}

/// Explicit authority is never rejected at dispatch: an in-anchor candidate
/// is walked without the implicit exact-entry check, an external one has its
/// final entry checked, and a parent-containing one is reduced, each with
/// the candidate's own tail.
pub(crate) proof fn theorem_explicit_authority_routes_by_place<T>(place: CandidatePlace<T>)
    ensures
        ({
            let route = guide_route_spec(GuideAuthority::Explicit, place);
            &&& !(route is RejectImplicitEscape)
            &&& match place {
                CandidatePlace::InAnchor(tail) => route == (GuideRoute::WalkBeneath {
                    tail,
                    exact_implicit_entry: false,
                }),
                CandidatePlace::External => route == GuideRoute::<T>::CheckExternalEntry,
                CandidatePlace::ParentContaining(tail) => route == GuideRoute::ReduceParents(tail),
            }
        }),
{
}

/// No authority bypasses the below-anchor walk for an in-anchor candidate,
/// and the walk covers that candidate's own tail.
pub(crate) proof fn theorem_in_anchor_candidate_is_always_walked<T>(authority: GuideAuthority, tail: T)
    ensures
        guide_route_spec(authority, CandidatePlace::InAnchor(tail)) == (GuideRoute::WalkBeneath {
            tail,
            exact_implicit_entry: authority == GuideAuthority::Implicit,
        }),
{
}

/// Only explicit authority reaches a route other than the walk or a
/// rejection, and only implicit authority is rejected at dispatch.
pub(crate) proof fn theorem_only_explicit_authority_leaves_the_walk<T>(
    authority: GuideAuthority,
    place: CandidatePlace<T>,
)
    ensures
        ({
            let route = guide_route_spec(authority, place);
            &&& (route is CheckExternalEntry || route is ReduceParents) ==> authority
                == GuideAuthority::Explicit
            &&& route is RejectImplicitEscape ==> authority == GuideAuthority::Implicit && !(
            place is InAnchor)
        }),
{
}

/// Every walk step rejects a link-like entry, rejects a non-final entry
/// that is not a directory, and accepts exactly the remaining entries. The
/// ancestor rule used for erased components and output ancestors is the
/// non-final step.
pub(crate) proof fn theorem_walk_step_rejects_links_and_non_directory_ancestors(
    final_entry: bool,
    facts: EntryFacts,
)
    ensures
        facts.link_like ==> walk_entry_spec(final_entry, facts) == WalkVerdict::LinkLike,
        !final_entry && !facts.is_dir ==> walk_entry_spec(final_entry, facts)
            != WalkVerdict::Continue,
        (walk_entry_spec(final_entry, facts) == WalkVerdict::Continue) == (!facts.link_like && (
        final_entry || facts.is_dir)),
        (walk_entry_spec(false, facts) == WalkVerdict::Continue) == (!facts.link_like
            && facts.is_dir),
{
}

/// Composed with the verified below-anchor plan
/// (`anchor::is_beneath_walk`): a walk passes every step exactly when the
/// tail is a real directory chain, that is, every component is an ordinary
/// name, no inspected entry is link-like, and every entry before the final
/// one is a directory. A passing walk inspects every component.
pub(crate) proof fn theorem_walk_accepts_only_real_directory_chains(
    kinds: Seq<ComponentKind>,
    steps: Seq<BeneathStep>,
    facts: Seq<EntryFacts>,
)
    requires
        is_beneath_walk(kinds, steps),
        facts.len() >= kinds.len(),
    ensures
        walk_passes(steps, facts) == real_directory_chain(kinds, facts),
        walk_passes(steps, facts) ==> steps.len() == kinds.len(),
{
    theorem_beneath_walk_accepts_only_normal_tails(kinds, steps);
    let m = steps.len() - 1;
    if walk_passes(steps, facts) {
        assert(steps[m] != BeneathStep::RejectUnresolved);
        assert(steps.last() == BeneathStep::InspectFinal);
        assert(steps.len() == kinds.len());
        assert forall|i: int| 0 <= i < kinds.len() implies !(#[trigger] facts[i]).link_like by {
            assert(walk_entry_spec(steps[i] == BeneathStep::InspectFinal, facts[i])
                == WalkVerdict::Continue);
        }
        assert forall|i: int| 0 <= i < kinds.len() - 1 implies (#[trigger] facts[i]).is_dir by {
            assert(steps[i] == BeneathStep::InspectAncestor);
            assert(walk_entry_spec(steps[i] == BeneathStep::InspectFinal, facts[i])
                == WalkVerdict::Continue);
        }
    }
    if real_directory_chain(kinds, facts) {
        assert(steps.last() == BeneathStep::InspectFinal);
        assert(steps.len() == kinds.len());
        assert forall|i: int| 0 <= i < steps.len() implies #[trigger] steps[i]
            != BeneathStep::RejectUnresolved by {
            if i < m {
                assert(steps[i] == BeneathStep::InspectAncestor);
            }
        }
        assert forall|i: int| 0 <= i < steps.len() implies walk_entry_spec(
            #[trigger] steps[i] == BeneathStep::InspectFinal,
            facts[i],
        ) == WalkVerdict::Continue by {
            assert(!facts[i].link_like);
            if i < m {
                assert(facts[i].is_dir);
            }
        }
    }
}

/// A link-like entry is classified as a link whatever type flags it also
/// carries, so a link is never read as a guide or reported as an ordinary
/// existing file; an entry is a regular file exactly when it is not
/// link-like and is a file.
pub(crate) proof fn theorem_link_classification_precedes_type(facts: EntryFacts)
    ensures
        facts.link_like ==> entry_class_spec(facts) == EntryClass::LinkLike,
        (entry_class_spec(facts) == EntryClass::RegularFile) == (!facts.link_like && facts.is_file),
        (entry_class_spec(facts) == EntryClass::Directory) == (!facts.link_like && !facts.is_file
            && facts.is_dir),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    const AUTHORITIES: [GuideAuthority; 2] = [GuideAuthority::Implicit, GuideAuthority::Explicit];

    fn all_facts() -> Vec<EntryFacts> {
        let mut all = Vec::new();
        for link_like in [false, true] {
            for is_dir in [false, true] {
                for is_file in [false, true] {
                    all.push(EntryFacts {
                        link_like,
                        is_dir,
                        is_file,
                    });
                }
            }
        }
        all
    }

    /// The dispatch exactly as `GuideAnchor::read` stated it before it moved
    /// into this kernel.
    fn reference_route(authority: GuideAuthority, place: CandidatePlace<u8>) -> GuideRoute<u8> {
        match place {
            CandidatePlace::InAnchor(tail) => GuideRoute::WalkBeneath {
                tail,
                exact_implicit_entry: authority == GuideAuthority::Implicit,
            },
            CandidatePlace::External if authority == GuideAuthority::Explicit => {
                GuideRoute::CheckExternalEntry
            }
            CandidatePlace::ParentContaining(tail) if authority == GuideAuthority::Explicit => {
                GuideRoute::ReduceParents(tail)
            }
            CandidatePlace::External | CandidatePlace::ParentContaining(_) => {
                GuideRoute::RejectImplicitEscape
            }
        }
    }

    #[test]
    fn route_matches_the_previous_dispatch() {
        for authority in AUTHORITIES {
            for place in [
                CandidatePlace::InAnchor(7),
                CandidatePlace::External,
                CandidatePlace::ParentContaining(9),
            ] {
                assert_eq!(
                    guide_route(authority, place),
                    reference_route(authority, place),
                    "{authority:?} {place:?}"
                );
            }
        }
        assert_eq!(
            guide_route(GuideAuthority::Implicit, CandidatePlace::InAnchor(1)),
            GuideRoute::WalkBeneath {
                tail: 1,
                exact_implicit_entry: true
            }
        );
        assert_eq!(
            guide_route(GuideAuthority::Implicit, CandidatePlace::<u8>::External),
            GuideRoute::RejectImplicitEscape
        );
        assert_eq!(
            guide_route(
                GuideAuthority::Explicit,
                CandidatePlace::ParentContaining(2)
            ),
            GuideRoute::ReduceParents(2)
        );
    }

    /// The per-step checks exactly as `validate_beneath_from` and
    /// `validate_component_before_parent` stated them before they moved into
    /// this kernel.
    fn reference_walk(final_entry: bool, facts: EntryFacts) -> WalkVerdict {
        if facts.link_like {
            return WalkVerdict::LinkLike;
        }
        if !final_entry && !facts.is_dir {
            return WalkVerdict::NotDirectory;
        }
        WalkVerdict::Continue
    }

    /// The final-entry classification exactly as `validate_final_entry` and
    /// `existing_entry_error` stated it before it moved into this kernel.
    fn reference_class(facts: EntryFacts) -> EntryClass {
        if facts.link_like {
            return EntryClass::LinkLike;
        }
        if !facts.is_file {
            return if facts.is_dir {
                EntryClass::Directory
            } else {
                EntryClass::Other
            };
        }
        EntryClass::RegularFile
    }

    #[test]
    fn entry_checks_match_the_previous_checks() {
        for facts in all_facts() {
            for final_entry in [false, true] {
                assert_eq!(
                    walk_entry_verdict(final_entry, facts),
                    reference_walk(final_entry, facts),
                    "{final_entry} {facts:?}"
                );
            }
            assert_eq!(
                directory_entry_verdict(facts),
                reference_walk(false, facts),
                "{facts:?}"
            );
            assert_eq!(classify_entry(facts), reference_class(facts), "{facts:?}");
        }
    }

    #[test]
    fn entry_checks_name_each_outcome() {
        let directory = EntryFacts {
            link_like: false,
            is_dir: true,
            is_file: false,
        };
        let file = EntryFacts {
            link_like: false,
            is_dir: false,
            is_file: true,
        };
        let link = EntryFacts {
            link_like: true,
            is_dir: false,
            is_file: true,
        };
        let other = EntryFacts {
            link_like: false,
            is_dir: false,
            is_file: false,
        };
        assert_eq!(walk_entry_verdict(false, directory), WalkVerdict::Continue);
        assert_eq!(walk_entry_verdict(false, file), WalkVerdict::NotDirectory);
        assert_eq!(walk_entry_verdict(true, file), WalkVerdict::Continue);
        assert_eq!(walk_entry_verdict(true, link), WalkVerdict::LinkLike);
        assert_eq!(directory_entry_verdict(link), WalkVerdict::LinkLike);
        assert_eq!(classify_entry(file), EntryClass::RegularFile);
        assert_eq!(classify_entry(directory), EntryClass::Directory);
        assert_eq!(classify_entry(link), EntryClass::LinkLike);
        assert_eq!(classify_entry(other), EntryClass::Other);
    }
}
