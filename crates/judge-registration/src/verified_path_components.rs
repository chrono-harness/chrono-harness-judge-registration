//! Git metadata admission for components returned by the platform path parser.
#![allow(unexpected_cfgs)]
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

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

}
