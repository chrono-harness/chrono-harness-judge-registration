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
#[cfg_attr(not(verus_keep_ghost), test)]
fn full_registry_checks_each_declared_selection_field() {
    use registration::registry_selection_matches as selected;
    expect(selected(false, false, false, false, false, false, false));
    expect(selected(true, true, true, true, true, true, true));
    expect(!selected(true, false, true, true, true, true, true));
    expect(!selected(true, true, false, true, true, true, true));
    expect(!selected(true, true, true, false, true, true, true));
    expect(!selected(true, true, true, true, false, true, true));
    expect(!selected(true, true, true, true, true, false, true));
    expect(!selected(true, true, true, true, true, true, false));
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn full_registry_requires_both_roots_digest_and_exact_disk_bytes() {
    use registration::{publishes_registration, registry_binding_matches, registry_bytes_match};
    proof { reveal_strlit("expected"); reveal_strlit("different"); }
    expect(registry_binding_matches("expected", "expected", true, true));
    expect(!registry_binding_matches("different", "expected", true, true));
    expect(!registry_binding_matches("expected", "expected", false, true));
    expect(!registry_binding_matches("expected", "expected", true, false));
    let original: &[u8] = &[123, 125, 10];
    let changed_byte: &[u8] = &[123, 125, 32];
    let (removed_byte, _) = original.split_at(2);
    expect(registry_bytes_match(original, original));
    expect(!registry_bytes_match(original, changed_byte));
    expect(!registry_bytes_match(original, removed_byte));
    expect(publishes_registration(true));
    expect(!publishes_registration(false));
}

#[cfg(verus_keep_ghost)]
fn full_context_requires_fixed_identities_operation_and_supported_mode() {
    use registration::{context_identity_matches, context_mode_matches};
    proof {
        reveal_strlit("base"); reveal_strlit("head"); reveal_strlit("validate.delta");
        reveal_strlit("check"); reveal_strlit("delivery"); reveal_strlit("integration");
    }
    let base = Some("base");
    let head = Some("head");
    let operation = Some("validate.delta");
    expect(context_identity_matches(true, base, head, base, operation, "base", "head"));
    expect(!context_identity_matches(false, base, head, base, operation, "base", "head"));
    expect(!context_identity_matches(true, head, head, base, operation, "base", "head"));
    expect(!context_identity_matches(true, base, base, base, operation, "base", "head"));
    expect(!context_identity_matches(true, base, head, head, operation, "base", "head"));
    expect(!context_identity_matches(true, base, head, base, Some("check"), "base", "head"));
    expect(!context_identity_matches(true, base, head, base, None, "base", "head"));
    expect(context_mode_matches(false, None));
    expect(context_mode_matches(true, Some("integration")));
    expect(context_mode_matches(true, Some("delivery")));
    expect(!context_mode_matches(true, Some("check")));
    expect(!context_mode_matches(true, None));
}

#[cfg(verus_keep_ghost)]
fn first_inventory_requires_all_proposed_documents_and_inactive_enforcement() {
    use registration::initial::initial_state;
    broadcast use vstd::array::group_array_axioms;
    proof { reveal_strlit("proposed"); reveal_strlit("active"); reveal_strlit("not-implemented"); }
    let pending = Some("proposed");
    let statuses = [pending; 5];
    let accepted = initial_state(statuses, Some("not-implemented"));
    let active = initial_state(statuses, Some("active"));
    let absent = initial_state(statuses, None);
    assert(accepted && !active && !absent);
    for i in 0..5usize {
        let mut changed = statuses;
        changed[i] = Some("active");
        assert(changed@[i as int] == Some("active"));
        assert(!registration::facts::same_text(changed@[i as int], "proposed"));
        let accepted = initial_state(changed, Some("not-implemented"));
        assert(!accepted);
    }
}

#[cfg(verus_keep_ghost)]
fn document_lookup_preserves_the_path_and_its_bytes() {
    use registration::registrations::registered_document;
    let original = 73u8;
    let fetch = |p: &str| -> (r: Option<&u8>) ensures
        r == (if p@ == "registered"@ { Some(&original) } else { None }),
    { if p == "registered" { Some(&original) } else { None } };
    proof { reveal_strlit("other"); reveal_strlit("registered"); }
    let good = registered_document(Some("registered"), fetch);
    let absent = registered_document(Some("other"), fetch);
    let no_path = registered_document(None, fetch);
    assert(good == Ok::<_, &str>(&original));
    assert(absent matches Err(e) && e@ == "missing registry bytes"@);
    assert(no_path matches Err(e) && e@ == "missing registry path"@);
}

#[cfg(verus_keep_ghost)]
fn worktree_requires_validation_before_accepting_its_declared_filemap() {
    use registration::worktree::admitted_filemap;
    let accepted = admitted_filemap(Ok(()), Some("map.json"));
    let absent = admitted_filemap(Ok(()), None);
    let wrong = admitted_filemap(Err("config error".to_string()), Some("map.json"));
    let both = admitted_filemap(Err("config error".to_string()), None);
    assert(accepted == Ok::<_, String>("map.json"));
    assert(absent matches Err(e) && e@ == "missing filemap registry path"@);
    assert(wrong matches Err(e) && e@ == "config error"@);
    assert(both matches Err(e) && e@ == "config error"@);
}

#[cfg(verus_keep_ghost)]
fn omitted_units_skip_the_reader_and_declared_units_preserve_its_outcome() {
    use registration::collection::declared_units;
    let unused = || -> (r: Result<Option<u8>, u8>) requires false, { Ok(None) };
    let units = || -> (r: Result<Option<u8>, u8>) ensures r == Ok::<_,u8>(Some(81u8)), { Ok(Some(81)) };
    let failed = || -> (r: Result<Option<u8>, u8>) ensures r == Err::<Option<u8>,_>(15u8), { Err(15) };
    let absent = declared_units(false, unused);
    let present = declared_units(true, units);
    let error = declared_units(true, failed);
    assert(absent == Ok::<Option<u8>, u8>(None));
    assert(present == Ok::<_, u8>(Some(81u8)));
    assert(error == Err::<Option<u8>, _>(15u8));
}

#[cfg(verus_keep_ghost)]
fn coverage_marks_required_rows_only_once() {
    use registration::input_coverage::admit_row;
    let not_registered = || -> (inserted: bool) requires false, { true };
    let new_row = || -> (inserted: bool) ensures inserted, { true };
    let already_present = || -> (inserted: bool) ensures !inserted, { false };
    let unknown = admit_row(false, not_registered);
    let first = admit_row(true, new_row);
    let duplicate = admit_row(true, already_present);
    assert(first && !duplicate && !unknown);
}

#[cfg(verus_keep_ghost)]
fn classification_requires_exactly_one_selector() {
    use registration::entry::classification_selector;
    proof { reveal_strlit("classification-inputs-v1"); reveal_strlit("classification-views-v1"); }
    let named = Some("classification-inputs-v1");
    let allowed = classification_selector(named, Some(83u8), 0);
    let absent = classification_selector(named, None::<u8>, 0);
    let extra = classification_selector(named, Some(83u8), 1);
    let different = classification_selector(Some("classification-views-v1"), Some(83u8), 0);
    let unnamed = classification_selector(None, Some(83u8), 0);
    assert(allowed == Some(Ok::<u8, &str>(83u8)));
    assert(absent matches Some(Err(e)) && e@ == "E_USAGE: classification-inputs-v1 SELECTOR"@);
    assert(extra matches Some(Err(e)) && e@ == "E_USAGE: classification-inputs-v1 SELECTOR"@);
    assert(different.is_none() && unnamed.is_none());
}

#[cfg(verus_keep_ghost)]
fn document_selection_preserves_a_known_document_and_requests_an_unknown_one() {
    use registration::classification_inputs::selected_document;
    let value = 16u8;
    let known = selected_document(true, false, Some(&value));
    let read = selected_document::<u8>(true, false, None);
    let missing = selected_document(false, false, Some(&value));
    let repeated = selected_document(true, true, Some(&value));
    assert(known == Ok::<_, ()>(Some(&value)));
    assert(read == Ok::<Option<&u8>, _>(None));
    assert(missing == Err::<Option<&u8>, _>(()) && repeated == Err::<Option<&u8>, _>(()));
}

#[cfg(verus_keep_ghost)]
fn command_record_keeps_the_unmodified_payload_after_one_header_line() {
    use registration::runner_session::command_input;
    broadcast use vstd::array::group_array_axioms;
    let mut header = Vec::new(); header.push(b'{'); header.push(b'}');
    let empty = command_input(header.clone(), &[]);
    let payload = [0u8, b'\n', 255u8];
    let framed = command_input(header, &payload);
    assert(empty@ =~= seq![b'{', b'}', b'\n']);
    assert(framed@ =~= seq![b'{', b'}', b'\n', 0u8, b'\n', 255u8]);
}

#[cfg(verus_keep_ghost)]
fn protocol_reports_the_original_invalid_field_and_preserves_both_limits() {
    use registration::schema::registration_limits;
    let accepted = registration_limits(Some(17), Some(455));
    let both_bad = registration_limits(None, Some(0));
    let no_time = registration_limits(Some(0), Some(455));
    let no_space = registration_limits(Some(17), None);
    let zero_space = registration_limits(Some(17), Some(0));
    assert(accepted == Ok::<_, &str>((17u64, 455u64)));
    assert(both_bad matches Err(e) && e@ == "invalid protocol timeout_seconds"@);
    assert(no_time matches Err(e) && e@ == "invalid protocol timeout_seconds"@);
    assert(no_space matches Err(e) && e@ == "invalid protocol stdout_limit_bytes"@);
    assert(zero_space matches Err(e) && e@ == "invalid protocol stdout_limit_bytes"@);
}

#[cfg(verus_keep_ghost)]
fn input_view_requires_all_three_original_identities() {
    use registration::facts::InputViewIdentity;
    let id = InputViewIdentity { base: "base", candidate: "head", registry: "digest" };
    proof { reveal_strlit("base"); reveal_strlit("head"); reveal_strlit("digest"); }
    let actual = id.matches(Some("base"), Some("head"), Some("digest"));
    assert(actual);
    let actual = id.matches(Some("head"), Some("base"), Some("digest"));
    assert(!actual);
    let actual = id.matches(Some("base"), Some("base"), Some("digest"));
    assert(!actual);
    let actual = id.matches(Some("base"), Some("head"), Some("head"));
    assert(!actual);
    let actual = id.matches(None, Some("head"), Some("digest"));
    assert(!actual);
    let actual = id.matches(Some("base"), None, Some("digest"));
    assert(!actual);
    let actual = id.matches(Some("base"), Some("head"), None);
    assert(!actual);
}

#[cfg(verus_keep_ghost)]
fn routes_bind_the_context_before_choosing_the_input_evidence() {
    use registration::route_views::{input_validation, InputValidation};
    proof { reveal_strlit("context"); reveal_strlit("other"); }
    let live = input_validation(true, Some("context"), "context", false);
    assert(live == Ok::<_, &str>(InputValidation::Live));
    let collect = input_validation(true, Some("context"), "context", true);
    assert(collect == Ok::<_, &str>(InputValidation::Collection));
    let changed = input_validation(true, Some("other"), "context", false);
    let missing = input_validation(true, None, "context", true);
    let unbound = input_validation(false, Some("context"), "context", true);
    assert(changed matches Err(e) && e@ == "route input registration view binding mismatch"@);
    assert(missing matches Err(e) && e@ == "route input registration view binding mismatch"@);
    assert(unbound matches Err(e) && e@ == "route input registration view binding mismatch"@);
}

#[cfg(verus_keep_ghost)]
fn executor_sees_projects_and_scripts_but_excludes_lookalikes() {
    use registration::project_views::project_owner;
    broadcast use vstd::array::group_array_axioms;
    let project = [b'p', b'r', b'o', b'j', b'e', b'c', b't', b':', b'a'];
    let script = [b's', b'c', b'r', b'i', b'p', b't', b':', b'b'];
    let lookalike = [b's', b'c', b'r', b'i', b'p', b't', b's', b':', b'b'];
    let short = [b'p', b'r', b'o', b'j', b'e', b'c', b't'];
    let unrelated = [b't', b'e', b's', b't', b':', b'a'];
    assert(project@.take(8) =~= seq![b'p', b'r', b'o', b'j', b'e', b'c', b't', b':']);
    assert(script@.take(7) =~= seq![b's', b'c', b'r', b'i', b'p', b't', b':']);
    assert(lookalike@[6] != b':');
    let included = project_owner(&project);
    assert(included);
    let included = project_owner(&script);
    assert(included);
    let included = project_owner(&lookalike);
    assert(!included);
    let included = project_owner(&short);
    assert(!included);
    let included = project_owner(&unrelated);
    assert(!included);
}

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
