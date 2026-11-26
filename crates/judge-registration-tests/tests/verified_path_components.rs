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
