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
