//! Input-coverage row state is a proved policy; JSON remains an external adapter.
#![allow(unexpected_cfgs)]
use vstd::{contrib::exec_spec::ExecSpecEq, prelude::*};

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
    let declared_complete = <&str>::exec_eq(closure_status, "declared-complete");
    if <&str>::exec_eq(state, "bound") || <&str>::exec_eq(state, "absent") {
        binding_count > 0
    } else if <&str>::exec_eq(state, "not-applicable") || <&str>::exec_eq(state, "unresolved") {
        binding_count == 0
            && !(<&str>::exec_eq(state, "unresolved") && declared_complete)
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
    <&str>::exec_eq(state, "absent")
}

}
