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
