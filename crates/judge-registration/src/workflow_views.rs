//! Interpreted registration data used when workflow verifies a sealed request.
use crate::{Registrations, execution, inputs, transition};
use chrono_harness::wire::Request;
use serde_json::{Value, json};
use std::collections::BTreeMap;

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

/// Defer individual declaration errors until workflow reaches the corresponding obligation.
fn tested_changes(old: &Registrations, current: &Registrations) -> Value {
    let before = old.node_data();
    let after = current.node_data();
    let mut ambiguities = BTreeMap::new();
    let mut removed_tests = BTreeMap::new();
    for (node, group) in &before {
        if !node.starts_with("test:") {
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
        if !after.contains_key(node) {
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
            if row.get("tests_for").is_some() {
                continue;
            }
            let id = row["id"].as_str().unwrap();
            if rows.iter().any(|r| r["id"] == id) {
                continue;
            }
            let replacement =
                transition::retirement_declaration(current, kind, id).and_then(|decl| {
                    let replacement = decl["replacement"].as_str();
                    if replacement.is_some_and(|name| !rows.iter().any(|r| r["id"] == name)) {
                        return Err("E_RETIREMENT: replacement identity missing".into());
                    }
                    Ok(replacement)
                });
            removed_owners.push(json!({"kind":kind,"id":id,"replacement":replacement}));
        }
    }
    json!({"requests":transition::retirement_requests(current),"ambiguities":ambiguities,
           "removed_tests":removed_tests,"removed_owners":removed_owners})
}

pub(crate) fn project(
    req: &Request,
    old: &Registrations,
    current: &Registrations,
    view: Value,
) -> Value {
    let unit = matches!(req.scope, Some(chrono_harness::units::Scope::Unit { .. }));
    json!({
        "config":current.config(), "projects":current.projects(), "filemap":current.filemap(),
        "judges":current.judges(), "workflow":current.workflow(),
        "methods":execution::methods(current.projects()),"test_operations":current.test_operations(),
        "effective_inputs":inputs::validate(req, old, current),
        "changes":if unit { Value::Null } else { tested_changes(old,current) },
        "conversion":view["conversion"]
    })
}
