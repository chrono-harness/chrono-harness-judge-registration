//! Independent identities for the actual registration node variants.
#![allow(unexpected_cfgs)]
#[cfg(verus_keep_ghost)]
#[path = "../../judge-registration/src/verified_nodes.rs"]
mod policy;
#[cfg(not(verus_keep_ghost))]
use chrono_judge_registration::verified_nodes as policy;
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
#[cfg_attr(not(verus_keep_ghost), test)]
fn closure_roles_cover_exactly_executable_and_external_inventory_kinds() {
    use policy::NodeKind;
    assert(NodeKind::Project.input_consumer()); assert(NodeKind::Script.input_consumer());
    assert(NodeKind::Test.input_consumer()); assert(NodeKind::Judge.input_consumer());
    assert(!NodeKind::File.input_consumer()); assert(!NodeKind::Input.input_consumer());
    assert(!NodeKind::Tool.input_consumer()); assert(!NodeKind::Environment.input_consumer());
    assert(NodeKind::Input.external_input()); assert(NodeKind::Tool.external_input());
    assert(NodeKind::Environment.external_input()); assert(!NodeKind::File.external_input());
    assert(!NodeKind::Project.external_input()); assert(!NodeKind::Script.external_input());
    assert(!NodeKind::Test.external_input()); assert(!NodeKind::Judge.external_input());
}


#[cfg_attr(not(verus_keep_ghost), test)]
fn node_variants_retain_identity_and_are_distinct() {
    use policy::NodeKind::*;
    let kinds = vec![File, Project, Script, Input, Test, Judge, Tool, Environment];
    let mut i = 0;
    while i < kinds.len()
        invariant 0 <= i <= kinds.len(), kinds.len() == 8,
            kinds@ == seq![File, Project, Script, Input, Test, Judge, Tool, Environment],
        decreases kinds.len() - i,
    {
        let copy = kinds[i].clone();
        assert(copy == kinds@[i as int]);
        let mut j = 0;
        while j < kinds.len()
            invariant 0 <= j <= kinds.len(), 0 <= i < kinds.len(), kinds.len() == 8,
                kinds@ == seq![File, Project, Script, Input, Test, Judge, Tool, Environment],
                copy == kinds@[i as int],
            decreases kinds.len() - j,
        {
            let equal = copy == kinds[j];
            assert(equal == (i == j));
            j += 1;
        }
        i += 1;
    }
}

}
