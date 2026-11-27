//! Independent expectations for bytes of a component from std::path.
#![allow(unexpected_cfgs)]
#[cfg(not(verus_keep_ghost))]
use chrono_judge_registration as registration;
#[cfg(verus_keep_ghost)]
#[path = "../../judge-registration/src/lib.rs"]
mod registration;
use registration::verified_path_components as policy;
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::vpanic as panic;

#[cfg(verus_keep_ghost)]
#[path = "registration_contract.rs"]
mod registration_contract;

verus! {
#[cfg(verus_keep_ghost)]
fn replacement_and_joint_removal_use_the_matching_validator_and_return_original_failures() {
    use registration::transition::decide_retirement;
    let valid = |_to: &u8| -> (r: Result<(), u8>) ensures r == Ok::<(), u8>(()), { Ok(()) };
    let missing = |_to: &u8| -> (r: Result<(), u8>) ensures r == Err::<(), u8>(8), { Err(8) };
    let never_replace = |_to: &u8| -> (r: Result<(), u8>) requires false, { Ok(()) };
    let joint = || -> (r: Result<(), u8>) ensures r == Ok::<(), u8>(()), { Ok(()) };
    let unpaired = || -> (r: Result<(), u8>) ensures r == Err::<(), u8>(13), { Err(13) };
    let never_joint = || -> (r: Result<(), u8>) requires false, { Ok(()) };
    let to = 37u8;
    let accepted = decide_retirement(Some(&to), valid, never_joint);
    assert(accepted == Ok::<_, u8>(Some(&to)));
    let rejected = decide_retirement(Some(&to), missing, never_joint);
    assert(rejected == Err::<Option<&u8>, _>(8u8));
    let accepted = decide_retirement(None, never_replace, joint);
    assert(accepted == Ok::<_, u8>(None::<&u8>));
    let rejected = decide_retirement(None, never_replace, unpaired);
    assert(rejected == Err::<Option<&u8>, _>(13u8));
}

#[cfg(verus_keep_ghost)]
fn workflow_reports_only_missing_baseline_tests() {
    use registration::workflow_views::{test_change, TestChange};
    broadcast use vstd::array::group_array_axioms;
    let baseline_test = [b't', b'e', b's', b't', b':', b'a'];
    let ordinary_node = [b't', b'e', b's', b't', b's', b':', b'a'];
    assert(baseline_test@.take(5) =~= seq![b't', b'e', b's', b't', b':']);
    assert(ordinary_node@[4] != b':');
    let present = || -> (found: bool) ensures found, { true };
    let missing = || -> (found: bool) ensures !found, { false };
    let ordinary = test_change(&ordinary_node, missing);
    assert(ordinary == TestChange::Other);
    let retained = test_change(&baseline_test, present);
    assert(retained == TestChange::Present);
    let removed = test_change(&baseline_test, missing);
    assert(removed == TestChange::Removed);
}

#[cfg(verus_keep_ghost)]
fn collection_queries_require_both_the_checkout_and_the_collection_scope() {
    use registration::test_retirements::read_collection;
    let no_read = || -> (r: Result<u8, String>) requires false, { Ok(0) };
    let wrong_checkout = read_collection(false, true, no_read);
    let wrong_scope = read_collection(true, false, no_read);
    assert(wrong_checkout matches Err(e) && e@ == "route collection requires this checkout and a collection scope"@);
    assert(wrong_scope matches Err(e) && e@ == "route collection requires this checkout and a collection scope"@);
    let underlying = || -> (r: Result<u8, String>) ensures r matches Err(e) && e@ == "original"@, { Err("original".to_string()) };
    let failed = read_collection(true, true, underlying);
    assert(failed matches Err(e) && e@ == "original"@);
    let success = || -> (r: Result<u8, String>) ensures r == Ok::<u8, String>(41), { Ok(41) };
    let resolved = read_collection(true, true, success);
    assert(resolved == Ok::<u8, String>(41));
}

#[cfg(verus_keep_ghost)]
fn read_completion_retains_the_original_action_and_only_publishes_a_completed_observation() {
    use registration::historical_git::complete_read;
    let publish = |receipt: u8| -> (out: Option<u8>) ensures out == Some(receipt), { Some(receipt) };
    let forbidden = |_receipt: u8| -> (out: Option<u8>) requires false, { None };
    let success = complete_read(Ok::<u8, u8>(7), Ok(31u8), publish);
    assert(success == (Ok::<u8, u8>(7), Some(31u8)));
    let failure = complete_read(Err::<u8, u8>(5), Ok(31u8), publish);
    assert(failure == (Err::<u8, u8>(5), Some(31u8)));
    let suppress = |_receipt: u8| -> (out: Option<u8>) ensures out.is_none(), { None };
    let omitted = complete_read(Err::<u8, u8>(5), Ok(31u8), suppress);
    assert(omitted == (Err::<u8, u8>(5), None::<u8>));
    let close_failed = complete_read(Ok::<u8, u8>(7), Err(9u8), forbidden);
    assert(close_failed == (Err::<u8, u8>(9), None::<u8>));
    let both_failed = complete_read(Err::<u8, u8>(5), Err(9u8), forbidden);
    assert(both_failed == (Err::<u8, u8>(5), None::<u8>));
}

#[cfg(verus_keep_ghost)]
fn ci_observations_follow_the_binding_and_preserve_the_first_error() {
    use registration::ci_transition::complete_transition;
    let requested = complete_transition(Err::<u8, u8>(5), Ok(Some(31u8)), true);
    assert(requested == (Err::<u8, u8>(5), Some(31u8)));
    let unrequested = complete_transition(Ok::<u8, u8>(7), Ok(Some(31u8)), false);
    assert(unrequested == (Ok::<u8, u8>(7), None::<u8>));
    let unopened = complete_transition(Ok::<u8, u8>(7), Ok(None::<u8>), true);
    assert(unopened == (Ok::<u8, u8>(7), None::<u8>));
    let close_failed = complete_transition(Ok::<u8, u8>(7), Err::<Option<u8>, u8>(9), false);
    assert(close_failed == (Err::<u8, u8>(9), None::<u8>));
    let both_failed = complete_transition(Err::<u8, u8>(5), Err::<Option<u8>, u8>(9), true);
    assert(both_failed == (Err::<u8, u8>(5), None::<u8>));
}


fn expect(condition: bool)
    requires condition,
{
    if !condition { panic!(); }
}

pub struct Original(pub u8);

impl registration::impact_views::EndpointDocuments for Original {
    type Snapshot = u8;
    type Error = u8;
    open spec fn projected(&self, config: &str) -> Result<u8,u8> {
        if self.0 < 128 { Ok(self.0) } else { Err(self.0) }
    }
    fn interpret(self, _config: &str) -> (result: Result<u8,u8>) {
        if self.0 < 128 { Ok(self.0) } else { Err(self.0) }
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn endpoint_documents_keep_each_side_and_first_error() {
    use registration::impact_views::interpret_pair;
    expect(matches!(interpret_pair((Original(3),Original(7)), "config"), Ok((3,7))));
    expect(matches!(interpret_pair((Original(3),Original(201)), "config"), Err(201)));
    expect(matches!(interpret_pair((Original(155),Original(7)), "config"), Err(155)));
    expect(matches!(interpret_pair((Original(155),Original(201)), "config"), Err(155)));
}

#[cfg(verus_keep_ghost)]
fn checkout_facts_determine_dirtiness(observed: &str, different: &str, changes: usize)
    requires observed@ != different@, changes > 0,
{
    expect(!registration::checkout_is_dirty(observed, observed, 0, 0));
    expect(registration::checkout_is_dirty(observed, different, 0, 0));
    expect(registration::checkout_is_dirty(observed, observed, changes, 0));
    expect(registration::checkout_is_dirty(observed, observed, 0, changes));
}



#[cfg_attr(not(verus_keep_ghost), test)]
fn literal_prefixes_include_only_matching_bytes() {
    broadcast use vstd::array::group_array_axioms;
    // ASCII o, u, t, slash, and a. The uppercase O differs by a single byte.
    let directory = [111u8, 117, 116, 47];
    let name = [111u8, 117, 116];
    expect(policy::has_byte_prefix(&[111u8, 117, 116, 47, 97], &directory));
    expect(policy::has_byte_prefix(&directory, &directory));
    expect(!policy::has_byte_prefix(&name, &directory));
    expect(!policy::has_byte_prefix(&[111u8, 117, 116, 45, 97], &directory));
    expect(!policy::has_byte_prefix(&[79u8, 117, 116, 47, 97], &directory));
    expect(policy::has_byte_prefix(&[111u8, 117, 116, 115], &name));
    expect(policy::has_byte_prefix(&name, &[]));
}

// An asymmetric result distinguishes reversal from preserving repeated paths.
#[cfg(verus_keep_ghost)]
fn ordered_repeats(directory: String, first: String, artifact: String, second: String)
    requires !policy::matches_directory(first, directory),
        policy::matches_directory(artifact, directory),
        !policy::matches_directory(second, directory),
{
    broadcast use vstd::array::group_array_axioms;
    let dirs = [directory];
    let paths = [first.clone(), artifact, first.clone(), second.clone()];
    let kept = policy::outside_artifacts(&dirs, &paths);
    proof {
        reveal_with_fuel(Seq::<_>::filter, 5);
    }
    expect(kept.len() == 3);
    expect(kept[0] == first);
    expect(kept[1] == first);
    expect(kept[2] == second);
}

}
