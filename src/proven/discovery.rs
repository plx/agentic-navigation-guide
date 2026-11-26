//! Verified per-entry decision of recursive guide discovery.
//!
//! These kernels decide what `recursive::find_guides` does with one
//! enumerated directory entry (`docs/v0.2-contract.md`, "Recursive discovery
//! and empty searches" and "Guide-file input"):
//!
//! - [`prunes_before_inspection`]: an entry matched by an exclusion is
//!   skipped before any metadata access, so an excluded directory is never
//!   inspected or enumerated and an excluded matching entry is omitted before
//!   inclusion;
//! - [`inspected_entry_action`]: for an entry that is not excluded, from its
//!   non-following metadata and whether its name is the guide name: a link or
//!   reparse entry with the guide name is validated as a guide (which rejects
//!   it), any other link is skipped without traversal, a non-link entry with
//!   the guide name is validated and accepted as a guide (a directory with
//!   that name is rejected by that validation), a non-link directory is
//!   enumerated later, and everything else is skipped.
//!
//! Glue supplies the exclusion verdict, the name comparison, and the
//! link/directory facts of the metadata, and performs the I/O each action
//! names, in the order the actions are decided.
//!
//! Scope of the proof: Verus proves the per-entry decision table. That the
//! traversal in `src/recursive.rs` consults the exclusion verdict before any
//! metadata call, and pushes exactly the `Descend` entries onto its worklist,
//! are properties of that unverified loop; `pushed_entries` models the loop
//! so the worklist theorem can be stated, but nothing ties the model to the
//! production code.
//!
//! The theorems below state the contract-level consequences:
//!
//! - `theorem_excluded_entries_are_never_inspected`: an excluded entry is
//!   pruned whatever its metadata would say, and every other entry is
//!   inspected;
//! - `theorem_links_are_never_descended`: a link or reparse entry is never
//!   enumerated and never accepted as a guide;
//! - `theorem_guide_named_entries_are_never_descended`: an entry with the
//!   guide name is never enumerated, and unless excluded it is always
//!   validated as a guide, so an unsafe matching entry is an error rather
//!   than a silently skipped one;
//! - `theorem_descended_entries_are_real_included_directories`: every
//!   enumerated entry is a non-link directory that is not excluded and does
//!   not have the guide name;
//! - `theorem_accepted_guides_are_included_non_links`: every accepted guide
//!   has the guide name, is not excluded, and is not a link;
//! - `theorem_worklist_contains_only_real_included_directories`: in the
//!   `pushed_entries` model of the traversal loop, over any sequence of
//!   enumerated entries, every entry pushed onto the worklist is a non-link,
//!   non-excluded directory without the guide name.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// The action for one enumerated entry that is not excluded (an excluded
/// entry is pruned before any metadata access).
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiscoveryAction {
    /// A link or reparse entry with the guide name: validate it as a guide,
    /// which rejects it.
    ValidateLink,
    /// Any other link or reparse entry: skip it without traversal.
    SkipLink,
    /// A non-link entry with the guide name: validate it and accept it.
    AcceptGuide,
    /// A non-link directory without the guide name: enumerate it later.
    Descend,
    /// Any other entry: skip it.
    Skip,
}

#[cfg(verus_keep_ghost)]
verus! {

/// The observable facts about one enumerated entry. `link_like` and
/// `is_directory` come from the entry's non-following metadata, which is
/// only read when the entry is not excluded.
pub(crate) struct EntryFacts {
    pub(crate) excluded: bool,
    pub(crate) matches_guide_name: bool,
    pub(crate) link_like: bool,
    pub(crate) is_directory: bool,
}

/// The discovery decision for one enumerated entry: `None` prunes an
/// excluded entry without inspecting it, and every other entry takes the
/// action its metadata and name decide.
pub(crate) open spec fn discovery_action_spec(
    excluded: bool,
    matches_guide_name: bool,
    link_like: bool,
    is_directory: bool,
) -> Option<DiscoveryAction> {
    if excluded {
        None
    } else if link_like {
        if matches_guide_name {
            Some(DiscoveryAction::ValidateLink)
        } else {
            Some(DiscoveryAction::SkipLink)
        }
    } else if matches_guide_name {
        Some(DiscoveryAction::AcceptGuide)
    } else if is_directory {
        Some(DiscoveryAction::Descend)
    } else {
        Some(DiscoveryAction::Skip)
    }
}

/// The decision for observed entry facts.
pub(crate) open spec fn action_of(facts: EntryFacts) -> Option<DiscoveryAction> {
    discovery_action_spec(
        facts.excluded,
        facts.matches_guide_name,
        facts.link_like,
        facts.is_directory,
    )
}

/// The action validates the entry as a guide (and so opens no traversal).
pub(crate) open spec fn validates_as_guide(action: Option<DiscoveryAction>) -> bool {
    action == Some(DiscoveryAction::ValidateLink) || action == Some(DiscoveryAction::AcceptGuide)
}

/// The indices of the entries a traversal pushes onto its worklist, in
/// enumeration order.
pub(crate) open spec fn pushed_entries(entries: Seq<EntryFacts>) -> Seq<int>
    decreases entries.len(),
{
    if entries.len() == 0 {
        Seq::empty()
    } else if action_of(entries.last()) == Some(DiscoveryAction::Descend) {
        pushed_entries(entries.drop_last()).push(entries.len() - 1)
    } else {
        pushed_entries(entries.drop_last())
    }
}

} // verus!

/// Return whether an enumerated entry is pruned before its metadata is
/// read: exactly when it is excluded.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        r == excluded,
        r ==> forall|m: bool, l: bool, d: bool|
            #[trigger] discovery_action_spec(excluded, m, l, d) is None,
        !r ==> forall|m: bool, l: bool, d: bool|
            #[trigger] discovery_action_spec(excluded, m, l, d) is Some,
))]
pub(crate) fn prunes_before_inspection(excluded: bool) -> bool {
    excluded
}

/// Decide the action for an entry that is not excluded, from whether its
/// name is the guide name and its non-following metadata.
#[cfg_attr(verus_keep_ghost, verus_spec(r =>
    ensures
        Some(r) == discovery_action_spec(false, matches_guide_name, link_like, is_directory),
))]
pub(crate) fn inspected_entry_action(
    matches_guide_name: bool,
    link_like: bool,
    is_directory: bool,
) -> DiscoveryAction {
    if link_like {
        if matches_guide_name {
            DiscoveryAction::ValidateLink
        } else {
            DiscoveryAction::SkipLink
        }
    } else if matches_guide_name {
        DiscoveryAction::AcceptGuide
    } else if is_directory {
        DiscoveryAction::Descend
    } else {
        DiscoveryAction::Skip
    }
}

#[cfg(verus_keep_ghost)]
verus! {

/// An excluded entry is pruned whatever its metadata would say, so the
/// decision needs no metadata; every other entry is inspected.
pub(crate) proof fn theorem_excluded_entries_are_never_inspected(
    m: bool,
    l: bool,
    d: bool,
    m2: bool,
    l2: bool,
    d2: bool,
)
    ensures
        discovery_action_spec(true, m, l, d) is None,
        discovery_action_spec(true, m, l, d) == discovery_action_spec(true, m2, l2, d2),
        discovery_action_spec(false, m, l, d) is Some,
{
}

/// A link or reparse entry is never enumerated and never accepted as a
/// guide.
pub(crate) proof fn theorem_links_are_never_descended(excluded: bool, m: bool, d: bool)
    ensures
        discovery_action_spec(excluded, m, true, d) != Some(DiscoveryAction::Descend),
        discovery_action_spec(excluded, m, true, d) != Some(DiscoveryAction::AcceptGuide),
        !excluded ==> discovery_action_spec(excluded, m, true, d) == Some(
            if m {
                DiscoveryAction::ValidateLink
            } else {
                DiscoveryAction::SkipLink
            },
        ),
{
}

/// An entry with the guide name is never enumerated; unless excluded, it is
/// always validated as a guide (a link or a directory with that name is
/// therefore an error, never a silently skipped entry).
pub(crate) proof fn theorem_guide_named_entries_are_never_descended(
    excluded: bool,
    l: bool,
    d: bool,
)
    ensures
        discovery_action_spec(excluded, true, l, d) != Some(DiscoveryAction::Descend),
        !excluded ==> validates_as_guide(discovery_action_spec(excluded, true, l, d)),
        discovery_action_spec(excluded, true, l, d) is None || validates_as_guide(
            discovery_action_spec(excluded, true, l, d),
        ),
{
}

/// Every enumerated entry is a non-link directory that is not excluded and
/// does not have the guide name.
pub(crate) proof fn theorem_descended_entries_are_real_included_directories(
    excluded: bool,
    m: bool,
    l: bool,
    d: bool,
)
    ensures
        (discovery_action_spec(excluded, m, l, d) == Some(DiscoveryAction::Descend)) == (!excluded
            && !m && !l && d),
{
}

/// Every accepted guide has the guide name, is not excluded, and is not a
/// link; only entries with the guide name are ever validated as guides.
pub(crate) proof fn theorem_accepted_guides_are_included_non_links(
    excluded: bool,
    m: bool,
    l: bool,
    d: bool,
)
    ensures
        (discovery_action_spec(excluded, m, l, d) == Some(DiscoveryAction::AcceptGuide)) == (
        !excluded && m && !l),
        validates_as_guide(discovery_action_spec(excluded, m, l, d)) ==> m && !excluded,
{
}

/// Over any sequence of enumerated entries, the worklist holds exactly the
/// non-link, non-excluded directories without the guide name, in
/// enumeration order.
pub(crate) proof fn theorem_worklist_contains_only_real_included_directories(
    entries: Seq<EntryFacts>,
)
    ensures
        forall|t: int|
            0 <= t < pushed_entries(entries).len() ==> {
                let i = #[trigger] pushed_entries(entries)[t];
                &&& 0 <= i < entries.len()
                &&& !entries[i].excluded
                &&& !entries[i].link_like
                &&& entries[i].is_directory
                &&& !entries[i].matches_guide_name
            },
        forall|i: int|
            0 <= i < entries.len() && action_of(#[trigger] entries[i]) == Some(DiscoveryAction::Descend)
                ==> pushed_entries(entries).contains(i),
        forall|s: int, t: int|
            0 <= s < t < pushed_entries(entries).len() ==> pushed_entries(entries)[s]
                < pushed_entries(entries)[t],
    decreases entries.len(),
{
    if entries.len() > 0 {
        let init = entries.drop_last();
        theorem_worklist_contains_only_real_included_directories(init);
        let p = pushed_entries(init);
        let q = pushed_entries(entries);
        assert forall|t: int| 0 <= t < q.len() implies {
            let i = #[trigger] q[t];
            &&& 0 <= i < entries.len()
            &&& !entries[i].excluded
            &&& !entries[i].link_like
            &&& entries[i].is_directory
            &&& !entries[i].matches_guide_name
        } by {
            if t < p.len() {
                assert(q[t] == p[t]);
                assert(entries[p[t]] == init[p[t]]);
            } else {
                assert(q[t] == entries.len() - 1);
                assert(entries[entries.len() - 1] == entries.last());
            }
        }
        assert forall|i: int|
            0 <= i < entries.len() && action_of(#[trigger] entries[i]) == Some(DiscoveryAction::Descend)
                implies q.contains(i) by {
            if i < entries.len() - 1 {
                assert(entries[i] == init[i]);
                assert(p.contains(i));
                let t = choose|t: int| 0 <= t < p.len() && p[t] == i;
                assert(q[t] == i);
            } else {
                assert(entries[i] == entries.last());
                assert(q[q.len() - 1] == i);
            }
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    /// The decision exactly as `find_guides` stated it before it moved into
    /// this kernel.
    fn reference(
        excluded: bool,
        matches: bool,
        link_like: bool,
        is_dir: bool,
    ) -> Option<DiscoveryAction> {
        if excluded {
            return None;
        }
        if link_like {
            if matches {
                return Some(DiscoveryAction::ValidateLink);
            }
            return Some(DiscoveryAction::SkipLink);
        }
        if matches {
            return Some(DiscoveryAction::AcceptGuide);
        }
        if is_dir {
            return Some(DiscoveryAction::Descend);
        }
        Some(DiscoveryAction::Skip)
    }

    #[test]
    fn every_entry_fact_combination_matches_the_reference() {
        for excluded in [false, true] {
            assert_eq!(prunes_before_inspection(excluded), excluded);
            for matches in [false, true] {
                for link_like in [false, true] {
                    for is_dir in [false, true] {
                        let expected = reference(excluded, matches, link_like, is_dir);
                        let actual = if prunes_before_inspection(excluded) {
                            None
                        } else {
                            Some(inspected_entry_action(matches, link_like, is_dir))
                        };
                        assert_eq!(
                            actual, expected,
                            "excluded={excluded} matches={matches} link={link_like} dir={is_dir}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn named_actions_cover_every_branch() {
        assert_eq!(
            inspected_entry_action(true, true, false),
            DiscoveryAction::ValidateLink
        );
        assert_eq!(
            inspected_entry_action(false, true, true),
            DiscoveryAction::SkipLink
        );
        assert_eq!(
            inspected_entry_action(true, false, true),
            DiscoveryAction::AcceptGuide
        );
        assert_eq!(
            inspected_entry_action(false, false, true),
            DiscoveryAction::Descend
        );
        assert_eq!(
            inspected_entry_action(false, false, false),
            DiscoveryAction::Skip
        );
    }
}
