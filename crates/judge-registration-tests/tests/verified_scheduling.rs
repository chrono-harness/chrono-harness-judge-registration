//! 独立边界：缺少参与操作的声明须拒绝，额外声明和值不影响覆盖。
#![allow(unexpected_cfgs)]
#[cfg(verus_keep_ghost)]
#[path = "../../judge-registration/src/execution.rs"]
mod execution;
#[cfg(not(verus_keep_ghost))]
use chrono_judge_registration::execution;
#[cfg(not(verus_keep_ghost))]
use chrono_judge_registration::verified_scheduling as policy;
#[cfg(verus_keep_ghost)]
use execution::verified_scheduling as policy;
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use vstd::vpanic as panic;

verus! {

/// The deployed check is an input of this proof and its native consumer.
pub fn native_check_script() -> &'static str {
    include_str!("../../../check.sh")
}
struct OneConflict { plan: String, changed: String }
impl execution::ClaimConflicts for OneConflict {
    closed spec fn overlap(&self, left: &String, right: &String) -> bool {
        self.plan@ == left@ && self.changed@ == right@
    }
    fn overlaps(&self, left: &String, right: &String) -> (yes: bool) {
        self.plan.as_str() == left.as_str() && self.changed.as_str() == right.as_str()
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn conflicts_at_either_endpoint_select_any_operation_and_any_changed_claim() {
    use execution::affected_plan;
    proof { reveal_strlit("build"); reveal_strlit("read"); reveal_strlit("left"); reveal_strlit("right"); }
    let build = "build".to_owned();
    let read = "read".to_owned();
    let left = "left".to_owned();
    let right = "right".to_owned();
    let operations = vec![build.clone(), read.clone()];
    let changed = vec![&left, &right];
    let needed = OneConflict { plan: read.clone(), changed: right.clone() };
    let unrelated = OneConflict { plan: left.clone(), changed: build.clone() };
    let old_only = affected_plan(false, false, &operations, &changed, &needed, &unrelated);
    let new_only = affected_plan(false, false, &operations, &changed, &unrelated, &needed);
    let neither = affected_plan(false, false, &operations, &changed, &unrelated, &unrelated);
    assert(execution::affected_claim(&operations@[1], changed@[1], &needed, &unrelated));
    assert(execution::affected_claim(&operations@[1], changed@[1], &unrelated, &needed));
    assert forall|i:int| 0 <= i < operations.len() implies
        !execution::affected_operation(&#[trigger] operations@[i], changed@, &unrelated, &unrelated) by {
        if i == 0 { assert(operations@[i]@ == build@); } else { assert(operations@[i]@ == read@); }
    }
    expect_protocol(old_only && new_only && !neither);
    let empty: Vec<&String> = Vec::new();
    expect_protocol(affected_plan(true, false, &operations, &empty, &unrelated, &unrelated));
    expect_protocol(affected_plan(false, true, &operations, &empty, &unrelated, &unrelated));
    expect_protocol(!affected_plan(false, false, &operations, &empty, &needed, &needed));
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn execution_group_requires_a_test_project_and_its_default_identity() {
    use execution::{explicit_action_key, explicit_groups_allowed, includes_default_group};
    expect_protocol(explicit_groups_allowed(true, true));
    expect_protocol(!explicit_groups_allowed(true, false));
    expect_protocol(!explicit_groups_allowed(false, true));
    expect_protocol(!includes_default_group(0, false));
    expect_protocol(!includes_default_group(1, false));
    expect_protocol(includes_default_group(2, true));
    expect_protocol(matches!(explicit_action_key(None), Err(())));
    match explicit_action_key(Some("check")) {
        Ok(key) => expect_protocol(key == "check"),
        Err(_) => expect_protocol(false),
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn omitted_groups_only_select_the_existing_eligible_execute_action() {
    use execution::default_test_action as binds;
    expect_protocol(binds(true, true, true, false, false, false));
    expect_protocol(binds(true, true, false, true, false, false));
    expect_protocol(!binds(true, true, false, false, true, true));
    expect_protocol(binds(true, false, false, false, true, false));
    expect_protocol(binds(true, false, false, false, false, true));
    expect_protocol(!binds(true, false, true, true, false, false));
    expect_protocol(!binds(false, true, true, true, true, true));
    expect_protocol(!binds(false, false, true, true, true, true));
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn empty_or_repeated_actions_precede_missing_action_and_keep_a_present_value() {
    use execution::{TestActionError, test_action};
    proof { reveal_strlit("test"); reveal_strlit("check"); reveal_strlit(""); }
    let empty = test_action("", "check", true, None::<u8>);
    let empty_key = test_action("test", "", true, None::<u8>);
    let repeated = test_action("test", "check", false, None::<u8>);
    let missing = test_action("test", "check", true, None::<u8>);
    let value = test_action("test", "check", true, Some(7u8));
    expect_protocol(matches!(empty, Err(TestActionError::EmptyOrRepeated)));
    expect_protocol(matches!(empty_key, Err(TestActionError::EmptyOrRepeated)));
    expect_protocol(matches!(repeated, Err(TestActionError::EmptyOrRepeated)));
    expect_protocol(matches!(missing, Err(TestActionError::Missing)));
    expect_protocol(matches!(value, Ok(7)));
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn duplicate_explicit_tests_fail_and_repeated_methods_stay_in_order() {
    use execution::{append_declaration, duplicate_test_identity};
    expect_protocol(!duplicate_test_identity(false, true, false));
    expect_protocol(!duplicate_test_identity(true, false, false));
    expect_protocol(duplicate_test_identity(true, true, false));
    expect_protocol(duplicate_test_identity(true, false, true));
    let mut methods = Vec::new();
    append_declaration(&mut methods, 3u64);
    append_declaration(&mut methods, 8u64);
    append_declaration(&mut methods, 3u64);
    expect_protocol(methods.len() == 3);
    expect_protocol(methods[0] == 3 && methods[1] == 8 && methods[2] == 3);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn persisted_decimal_starts_after_safe_integers() {
    expect_protocol(!policy::decimal_timeout(9007199254740991u64));
    expect_protocol(policy::decimal_timeout(9007199254740992u64));
    expect_protocol(policy::decimal_timeout(18446744073709551615u64));
}

// Keep these checks executable in both the native and verified test.
fn expect_protocol(ok: bool)
    requires ok,
{
    if !ok { panic!(); }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn protocol_field_errors_precede_launch_and_positive_values_are_preserved() {
    use policy::ProtocolLimitError;
    let missing = policy::protocol_limits(None, Some(0));
    expect_protocol(matches!(missing, Err(ProtocolLimitError::Timeout)));
    let zero = policy::protocol_limits(Some(0), None);
    expect_protocol(matches!(zero, Err(ProtocolLimitError::Timeout)));
    let invalid_output = policy::protocol_limits(Some(1), Some(0));
    expect_protocol(matches!(invalid_output, Err(ProtocolLimitError::Output)));
    let valid = policy::protocol_limits(Some(1), Some(u64::MAX));
    match valid {
        Ok((time, size)) => expect_protocol(time == 1 && size == u64::MAX),
        Err(_) => expect_protocol(false),
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn execution_plan_limits_are_inclusive_and_require_time_and_output() {
    let limit = 64;
    let max = policy::Bounds { timeout_seconds: u64::MAX, output_limit_bytes: limit };
    let low = policy::Bounds { timeout_seconds: 1, output_limit_bytes: 1 };
    let too_large = policy::Bounds { timeout_seconds: 1, output_limit_bytes: 65 };
    let zero_time = policy::Bounds { timeout_seconds: 0, output_limit_bytes: 1 };
    let zero_output = policy::Bounds { timeout_seconds: 1, output_limit_bytes: 0 };
    let ok = max.valid(limit); assert(ok);
    let ok = low.valid(limit); assert(ok);
    let ok = too_large.valid(limit); assert(!ok);
    let ok = zero_time.valid(limit); assert(!ok);
    let ok = zero_output.valid(limit); assert(!ok);
}

// The same wire name compares equal across independently allocated strings.
proof fn selected_and_overridden_operations(first: String, second: String, blank: String)
    requires first@.len() > 0, second@.len() > 0, first@ != second@, blank@.len() == 0,
{
    let selected = seq![first, second];
    let upper = policy::Bounds { timeout_seconds: 1, output_limit_bytes: 64 };
    let bad = policy::Bounds { timeout_seconds: 1, output_limit_bytes: 65 };
    assert(policy::admitted_operations(selected, 2));
    assert(!policy::admitted_operations(seq![first, first], 2));
    assert(!policy::admitted_operations(seq![first, blank], 2));
    assert(policy::permitted_override(selected, second, upper, 64));
    assert(!policy::permitted_override(selected, second, bad, 64));
    assert(!policy::permitted_override(selected, blank, upper, 64));
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn coverage_requires_all_participants_and_allows_production_claims() {
    let a = "build".to_owned();
    let copy = "build".to_owned();
    let b = "publish".to_owned();
    let extra = "release".to_owned();
    proof { reveal_strlit("build"); reveal_strlit("publish"); reveal_strlit("release"); }
    let operations = vec![&a, &b, &a];
    let empty: Vec<&String> = Vec::new();
    let mut claims: Vec<&String> = Vec::new();

    let empty_covered = execution::claims_cover_operations(&empty, &claims);
    expect_protocol(empty_covered);
    let missing_all = execution::claims_cover_operations(&operations, &claims);
    assert(operations@[0]@ == a@);
    assert(!execution::claim_named(claims@, a@));
    expect_protocol(!missing_all);
    claims.push(&copy);
    claims.push(&extra);
    let missing_last = execution::claims_cover_operations(&operations, &claims);
    assert(operations@[1]@ == b@);
    assert(!execution::claim_named(claims@, b@));
    expect_protocol(!missing_last);
    claims.push(&b);
    let all_covered = execution::claims_cover_operations(&operations, &claims);
    assert forall|i: int| 0 <= i < operations.len() implies execution::claim_named(claims@, #[trigger] operations@[i]@) by {
        if i == 1 { assert(claims@[2]@ == operations@[i]@); }
        else { assert(claims@[0]@ == operations@[i]@); }
    }
    expect_protocol(all_covered);
    let no_participants = execution::claims_cover_operations(&empty, &claims);
    expect_protocol(no_participants);
    let last_only = vec![&b];
    let pair = vec![&extra, &b];
    let missing_first = execution::claims_cover_operations(&pair, &last_only);
    assert(!execution::claim_named(last_only@, pair@[0]@));
    expect_protocol(!missing_first);
}

}

#[cfg(not(verus_keep_ghost))]
#[test]
fn persisted_decimal_timeouts_match_original_numeric_plan_declarations() {
    use serde_json::{Value, json};
    let plan = json!({"operations":["b"],"timeout_seconds":1,"output_limit_bytes":1,
                      "operation_bounds":{"b":{"timeout_seconds":1,"output_limit_bytes":1}}});
    let read = |plans: Value| {
        let q =
            json!({"filemap":{"execution_plans":plans}, "projects":{"projects":[],"scripts":[]}});
        execution::plan_declarations(q.to_string().as_bytes())
    };
    for seconds in [1_u64 << 53, (1 << 53) + 1, u64::MAX] {
        let mut numbers = plan.clone();
        numbers["timeout_seconds"] = json!(seconds);
        numbers["operation_bounds"]["b"]["timeout_seconds"] = json!(seconds);
        let mut persisted = numbers.clone();
        persisted["timeout_seconds"] = json!(seconds.to_string());
        persisted["operation_bounds"]["b"]["timeout_seconds"] = json!(seconds.to_string());
        assert_eq!(
            read(json!({"test:ok":persisted})).unwrap(),
            read(json!({"test:ok":numbers})).unwrap()
        );
    }
    for invalid in [
        "1",
        "9007199254740991",
        "09007199254740993",
        "+9007199254740993",
        "18446744073709551616",
    ] {
        let mut persisted = plan.clone();
        persisted["timeout_seconds"] = json!(invalid);
        assert!(read(json!({"test:ok":persisted})).is_err());
    }
}

#[cfg(not(verus_keep_ghost))]
#[test]
fn native_check_matches_the_compiled_host_input() {
    let deployed = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../check.sh");
    assert_eq!(
        std::fs::read(&deployed)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", deployed.display())),
        native_check_script().as_bytes()
    );
}
