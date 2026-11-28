//! Input-coverage row state is a proved policy; JSON remains an external adapter.
#![allow(unexpected_cfgs)]
use vstd::hash_set::StringHashSet;
use vstd::prelude::*;

verus! {

#[verifier::inline]
pub open spec fn valid_state(
    state: Seq<char>, binding_count: usize, declared_complete: bool,
) -> bool {
    if state == "bound"@ || state == "absent"@ {
        binding_count > 0
    } else if state == "not-applicable"@ || state == "unresolved"@ {
        binding_count == 0
            && !(state == "unresolved"@ && declared_complete)
    } else {
        false
    }
}

pub fn row_state_valid(
    state: &str, binding_count: usize, closure_status: &str,
) -> (valid: bool)
    ensures valid == valid_state(
        state@, binding_count,
        closure_status@ == "declared-complete"@,
    ),
{
    let declared_complete = closure_status == "declared-complete";
    if state == "bound" || state == "absent" {
        binding_count > 0
    } else if state == "not-applicable" || state == "unresolved" {
        binding_count == 0
            && !(state == "unresolved" && declared_complete)
    } else {
        false
    }
}

#[verifier::inline]
pub open spec fn absent_state(state: Seq<char>) -> bool {
    state == "absent"@
}

pub fn is_absent(state: &str) -> (absent: bool)
    ensures absent == absent_state(state@),
{
    state == "absent"
}

/// Only a declared prerequisite of a needed consumer can join the input closure.
pub open spec fn closed_inputs(edges: Seq<(String, String)>, names: Set<Seq<char>>) -> bool {
    forall|i: int| 0 <= i < edges.len() && names.contains(#[trigger] edges[i].1@)
        ==> names.contains(edges[i].0@)
}

pub open spec fn minimal_inputs(
    edges: Seq<(String, String)>, seeds: Set<Seq<char>>, names: Set<Seq<char>>,
) -> bool {
    forall|superset: Set<Seq<char>>| seeds.subset_of(superset) && #[trigger] closed_inputs(edges, superset)
        ==> names.subset_of(superset)
}

pub fn carries_input(kind: &str) -> (allowed: bool)
    ensures allowed == (kind@ == "compile"@ || kind@ == "build-input"@
        || kind@ == "runtime-input"@ || kind@ == "test-execution"@),
{
    kind == "compile" || kind == "build-input" || kind == "runtime-input" || kind == "test-execution"
}

/// Saturate the reverse edges and explicit bindings without admitting an unrelated node.
pub fn input_closure(edges: &[(String, String)], initial: StringHashSet) -> (result: StringHashSet)
    requires initial.well_formed(),
    ensures result.well_formed(), initial@.subset_of(result@), closed_inputs(edges@, result@),
        minimal_inputs(edges@, initial@, result@),
{
    let ghost seeds = initial@;
    let mut needed = initial;
    let ghost sources = edges@.map_values(|e: (String, String)| e.0@);
    let ghost possible = sources.to_set();
    loop
        invariant needed.well_formed(), seeds == initial@, seeds.subset_of(needed@), minimal_inputs(edges@, seeds, needed@),
            sources == edges@.map_values(|e: (String, String)| e.0@), possible == sources.to_set(),
        decreases possible.difference(needed@).len(),
    {
        let ghost previous = needed@;
        let mut changed = false;
        for i in 0..edges.len()
            invariant needed.well_formed(), seeds == initial@,
                sources == edges@.map_values(|e: (String, String)| e.0@), possible == sources.to_set(),
                previous.subset_of(needed@), seeds.subset_of(needed@),
                minimal_inputs(edges@, seeds, needed@),
                !changed ==> needed@ == previous,
                changed ==> exists|name: Seq<char>| possible.contains(name) && !previous.contains(name) && needed@.contains(name),
                !changed ==> forall|j: int| 0 <= j < i && needed@.contains(#[trigger] edges@[j].1@)
                    ==> needed@.contains(edges@[j].0@),
        {
            let (source, consumer) = &edges[i];
            if needed.contains(consumer.as_str()) {
                let ghost known = needed@;
                if needed.insert(source.clone()) {
                    proof {
                        assert(possible.contains(source@)) by {
                            assert(sources[i as int] == source@);
                            assert(sources.contains(source@));
                        }
                        assert forall|superset: Set<Seq<char>>| seeds.subset_of(superset)
                            && #[trigger] closed_inputs(edges@, superset) implies needed@.subset_of(superset) by {
                            assert(known.subset_of(superset));
                            assert(superset.contains(edges@[i as int].1@));
                        }
                    }
                    changed = true;
                }
            }
        }
        if !changed { return needed; }
        proof {
            let name = choose|name: Seq<char>| possible.contains(name) && !previous.contains(name) && needed@.contains(name);
            let old_missing = possible.difference(previous);
            let new_missing = possible.difference(needed@);
            assert(old_missing.contains(name));
            assert(new_missing.subset_of(old_missing.remove(name)));
            vstd::set_lib::lemma_len_subset(new_missing, old_missing.remove(name));
        }
    }
}

}
