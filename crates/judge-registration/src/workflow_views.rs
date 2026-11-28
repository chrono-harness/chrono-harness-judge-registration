//! Interpreted registration data used when workflow verifies a sealed request.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
pub fn retired_owner(test_owner: bool, still_declared: bool) -> (removed: bool)
    ensures removed == (!test_owner && !still_declared),
{ !test_owner && !still_declared }

pub fn replacement_exists(declared: bool, found: bool) -> (out: Result<(), &'static str>)
    ensures if declared && !found {
        out matches Err(message) && message@ == "E_RETIREMENT: replacement identity missing"@
    } else { out == Ok::<(), &str>(()) },
{
    if declared && !found { Err("E_RETIREMENT: replacement identity missing") } else { Ok(()) }
}


pub(crate) enum TestChange { Other, Present, Removed }

/// Only baseline test nodes can be ambiguous or removed; candidate membership decides removal.
pub(crate) fn test_change<F>(node: &[u8], present: F) -> (change: TestChange)
    where F: FnOnce() -> bool,
    requires present.requires(()),
    ensures if super::verified_path_components::byte_prefix(node@, seq![b't', b'e', b's', b't', b':']) {
        exists|found: bool| #[trigger] present.ensures((), found)
            && change == (if found { TestChange::Present } else { TestChange::Removed })
    } else { change == TestChange::Other },
{
    let prefix = [b't', b'e', b's', b't', b':'];
    broadcast use vstd::array::group_array_axioms;
    assert(prefix@ =~= seq![b't', b'e', b's', b't', b':']);
    if !super::verified_path_components::has_byte_prefix(node, &prefix) { return TestChange::Other; }
    let found = present();
    assert(present.ensures((), found));
    if found { TestChange::Present } else { TestChange::Removed }
}
}

#[cfg(not(verus_keep_ghost))]
use crate::wire::Request;
#[cfg(not(verus_keep_ghost))]
use crate::{Registrations, execution, inputs, transition};
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::collections::BTreeMap;

#[cfg(not(verus_keep_ghost))]
pub fn query(root: &std::path::Path, input: impl std::io::Read) -> Result<Value, String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Query {
        request: Request,
        #[serde(default)]
        replay: bool,
        originals: [BTreeMap<String, String>; 2],
    }
    let query: Query = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    crate::impact_views::workflow(root, query.request, query.replay, query.originals)
}

#[cfg(not(verus_keep_ghost))]
/// Defer individual declaration errors until workflow reaches the corresponding obligation.
fn tested_changes(old: &Registrations, current: &Registrations) -> Value {
    let before = old.node_data();
    let after = current.node_data();
    let mut ambiguities = BTreeMap::new();
    let mut removed_tests = BTreeMap::new();
    for (node, group) in &before {
        let change = test_change(node.as_bytes(), || after.contains_key(node));
        if matches!(change, TestChange::Other) {
            continue;
        }
        if group.definitions.len() > 1 {
            let ids: Vec<_> = group
                .definitions
                .iter()
                .map(|d| d.identity.clone())
                .collect();
            ambiguities.insert(
                node,
                json!({
                    "repaired":transition::ambiguity_repaired(current, node, &ids),"definitions":ids
                }),
            );
        }
        if matches!(change, TestChange::Removed) {
            removed_tests.insert(
                node,
                transition::validate_test_retirement(old, current, node),
            );
        }
    }
    let mut removed_owners = vec![];
    for (collection, kind) in [
        ("projects", "project"),
        ("scripts", "script"),
        ("judges", "judge"),
    ] {
        let old_rows: Vec<_> = if kind == "judge" {
            old.judges()[collection]
                .as_array()
                .unwrap()
                .iter()
                .collect()
        } else {
            before
                .iter()
                .filter(|(id, _)| id.starts_with(&format!("{kind}:")))
                .flat_map(|(_, node)| node.definitions.iter().map(|d| d.value))
                .collect()
        };
        let rows = if kind == "judge" {
            &current.judges()[collection]
        } else {
            &current.projects()[collection]
        };
        let rows = rows.as_array().unwrap();
        for row in old_rows {
            let id = row["id"].as_str().unwrap();
            if !retired_owner(
                row.get("tests_for").is_some(),
                rows.iter().any(|r| r["id"] == id),
            ) {
                continue;
            }
            let replacement =
                transition::retirement_declaration(current, kind, id).and_then(|decl| {
                    let replacement = decl["replacement"].as_str();
                    replacement_exists(
                        replacement.is_some(),
                        replacement.is_some_and(|name| rows.iter().any(|r| r["id"] == name)),
                    )?;
                    Ok(replacement)
                });
            removed_owners.push(json!({"kind":kind,"id":id,"replacement":replacement}));
        }
    }
    json!({"requests":transition::retirement_requests(current),"ambiguities":ambiguities,
           "removed_tests":removed_tests,"removed_owners":removed_owners})
}

#[cfg(not(verus_keep_ghost))]
pub(crate) fn project(
    req: &Request,
    old: &Registrations,
    current: &Registrations,
    view: Value,
) -> Value {
    let unit = matches!(req.scope, Some(crate::wire::Scope::Unit { .. }));
    json!({
        "config":current.config(), "projects":current.projects(), "filemap":current.filemap(),
        "judges":current.judges(), "workflow":current.workflow(),
        "methods":execution::methods(current.projects()),"test_operations":current.test_operations(),
        "effective_inputs":inputs::validate(req, old, current),
        "changes":if unit { Value::Null } else { tested_changes(old,current) },
        "conversion":view["conversion"]
    })
}
