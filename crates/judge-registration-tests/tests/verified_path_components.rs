//! Independent expectations for bytes of a component from std::path.
#![allow(unexpected_cfgs)]
#[cfg(not(verus_keep_ghost))]
use chrono_judge_registration::verified_path_components as policy;
#[cfg(verus_keep_ghost)]
#[path = "../../judge-registration/src/verified_path_components.rs"]
mod policy;
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::vpanic as panic;

verus! {

fn expect(condition: bool)
    requires condition,
{
    if !condition { panic!(); }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn metadata_acceptance_distinguishes_all_ascii_cases_from_neighbors() {
    broadcast use vstd::array::group_array_axioms;
    expect(policy::is_git_metadata(&[46u8, 103u8, 105u8, 116u8]));
    expect(policy::is_git_metadata(&[46u8, 103u8, 105u8, 84u8]));
    expect(policy::is_git_metadata(&[46u8, 103u8, 73u8, 116u8]));
    expect(policy::is_git_metadata(&[46u8, 103u8, 73u8, 84u8]));
    expect(policy::is_git_metadata(&[46u8, 71u8, 105u8, 116u8]));
    expect(policy::is_git_metadata(&[46u8, 71u8, 105u8, 84u8]));
    expect(policy::is_git_metadata(&[46u8, 71u8, 73u8, 116u8]));
    expect(policy::is_git_metadata(&[46u8, 71u8, 73u8, 84u8]));
    expect(!policy::is_git_metadata(&[46u8, 103, 105, 116, 104, 117, 98]));
    expect(!policy::is_git_metadata(&[120u8, 46, 103, 105, 116]));
    expect(!policy::is_git_metadata(&[46u8, 103, 105]));
    expect(!policy::is_git_metadata(&[46u8, 103, 105, 117]));
    expect(!policy::is_git_metadata(&[46u8, 103, 196, 177, 116]));
}

}
