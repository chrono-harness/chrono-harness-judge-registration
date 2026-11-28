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

#[cfg(verus_keep_ghost)]
use vstd::vpanic as panic;

verus! {
fn expect_role(ok: bool) requires ok { if !ok { panic!(); } }
#[cfg_attr(not(verus_keep_ghost), test)]
fn closure_roles_cover_exactly_executable_and_external_inventory_kinds() {
    use policy::NodeKind;
    expect_role(NodeKind::Project.input_consumer()); expect_role(NodeKind::Script.input_consumer());
    expect_role(NodeKind::Test.input_consumer()); expect_role(NodeKind::Judge.input_consumer());
    expect_role(!NodeKind::File.input_consumer()); expect_role(!NodeKind::Input.input_consumer());
    expect_role(!NodeKind::Tool.input_consumer()); expect_role(!NodeKind::Environment.input_consumer());
    expect_role(NodeKind::Input.external_input()); expect_role(NodeKind::Tool.external_input());
    expect_role(NodeKind::Environment.external_input()); expect_role(!NodeKind::File.external_input());
    expect_role(!NodeKind::Project.external_input()); expect_role(!NodeKind::Script.external_input());
    expect_role(!NodeKind::Test.external_input()); expect_role(!NodeKind::Judge.external_input());
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
