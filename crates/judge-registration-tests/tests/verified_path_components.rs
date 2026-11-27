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

verus! {

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
