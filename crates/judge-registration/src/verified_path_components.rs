//! Protect Git metadata and classify observed paths against literal artifact prefixes.
#![allow(unexpected_cfgs)]
use vstd::{contrib::exec_spec::ExecSpecEq, prelude::*};

verus! {

pub open spec fn metadata_name(name: Seq<u8>) -> bool {
    name.len() == 4 && name[0] == 46
        && (name[1] == 103 || name[1] == 71)
        && (name[2] == 105 || name[2] == 73)
        && (name[3] == 116 || name[3] == 84)
}

pub fn is_git_metadata(name: &[u8]) -> (protected: bool)
    ensures protected == metadata_name(name@),
{
    name.len() == 4 && name[0] == 46u8
        && (name[1] == 103u8 || name[1] == 71u8)
        && (name[2] == 105u8 || name[2] == 73u8)
        && (name[3] == 116u8 || name[3] == 84u8)
}

pub open spec fn byte_prefix(path: Seq<u8>, prefix: Seq<u8>) -> bool {
    prefix.len() <= path.len() && path.take(prefix.len() as int) =~= prefix
}

pub fn has_byte_prefix(path: &[u8], prefix: &[u8]) -> (matches: bool)
    ensures matches == byte_prefix(path@, prefix@),
{
    if path.len() < prefix.len() { return false; }
    let (head, _) = path.split_at(prefix.len());
    proof { assert(head.deep_view() =~= head@); assert(prefix.deep_view() =~= prefix@); }
    <&[u8]>::exec_eq(head, prefix)
}

pub open spec fn matches_directory(path: String, directory: String) -> bool {
    byte_prefix(vstd::utf8::encode_utf8(path@), vstd::utf8::encode_utf8(directory@))
}

pub open spec fn governed(path: String, directories: Seq<String>) -> bool {
    forall|i: int| 0 <= i < directories.len() ==> !matches_directory(path, #[trigger] directories[i])
}

fn outside_directories(path: &String, directories: &[String]) -> (keep: bool)
    ensures keep == governed(*path, directories@),
{
    for i in 0..directories.len()
        invariant forall|j: int| 0 <= j < i ==> !matches_directory(*path, #[trigger] directories@[j]),
    {
        if has_byte_prefix(path.as_str().as_bytes(), directories[i].as_str().as_bytes()) {
            assert(matches_directory(*path, directories@[i as int]));
            return false;
        }
    }
    true
}

/// Preserve the order and multiplicity of every governed untracked path.
pub fn outside_artifacts(directories: &[String], paths: &[String]) -> (kept: Vec<String>)
    ensures kept@ == paths@.filter(|path: String| governed(path, directories@)),
{
    let mut kept = Vec::new();
    for i in 0..paths.len()
        invariant kept@ == paths@.take(i as int).filter(|path: String| governed(path, directories@)),
    {
        if outside_directories(&paths[i], directories) {
            kept.push(paths[i].clone());
        }
        proof {
            let before = paths@.take(i as int);
            assert(paths@.take(i + 1) =~= before.push(paths@[i as int]));
            before.lemma_filter_push(paths@[i as int], |path: String| governed(path, directories@));
        }
    }
    assert(paths@.take(paths.len() as int) =~= paths@);
    kept
}

}
