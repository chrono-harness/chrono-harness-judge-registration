//! 独立边界：缺少参与操作的声明须拒绝，额外声明和值不影响覆盖。
#![allow(unexpected_cfgs)]
#[cfg(verus_keep_ghost)]
#[path = "../../judge-registration/src/verified_scheduling.rs"]
mod policy;
#[cfg(not(verus_keep_ghost))]
use chrono_judge_registration::verified_scheduling as policy;
use std::collections::{BTreeMap, BTreeSet};
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

// Use the same abstract key twice for a duplicate. The wire query exercises separately decoded names.
proof fn selected_and_overridden_operations(first: String, second: String, blank: String)
    requires first@.len() > 0, second@.len() > 0, first != second, blank@.len() == 0,
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
    let empty: BTreeSet<usize> = BTreeSet::new();
    let mut operations: BTreeSet<usize> = BTreeSet::new();
    operations.insert(2);
    operations.insert(4);
    let mut claims: BTreeMap<usize, bool> = BTreeMap::new();

    let empty_covered = policy::claims_cover_operations(&empty, &claims);
    assert(empty_covered);
    let absent = policy::claims_cover_operations(&operations, &claims);
    assert(!absent);

    claims.insert(2, true);
    let missing_last = policy::claims_cover_operations(&operations, &claims);
    assert(!missing_last);
    claims.insert(9, false);
    let equal_count_wrong_keys = policy::claims_cover_operations(&operations, &claims);
    assert(!equal_count_wrong_keys);

    claims.insert(4, true);
    let extra_production_claim = policy::claims_cover_operations(&operations, &claims);
    assert(extra_production_claim);
    let no_participants = policy::claims_cover_operations(&empty, &claims);
    assert(no_participants);
    operations.insert(9);
    let false_value_is_still_a_claim = policy::claims_cover_operations(&operations, &claims);
    assert(false_value_is_still_a_claim);

    operations.insert(1);
    assert(operations@.contains(1usize));
    assert(!claims@.contains_key(1usize));
    let missing_first = policy::claims_cover_operations(&operations, &claims);
    assert(!missing_first);
}

}
