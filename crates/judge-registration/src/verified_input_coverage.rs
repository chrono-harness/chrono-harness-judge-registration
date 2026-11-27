//! Input-coverage row state is a proved policy; JSON remains an external adapter.
#![allow(unexpected_cfgs)]
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

}
