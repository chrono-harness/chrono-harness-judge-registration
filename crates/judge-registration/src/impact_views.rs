//! Registration facts used by FILEMAP for a pair of fixed endpoints.
use crate::{Registrations, ambiguity_repaired, execution::Scheduling};
use chrono_harness::facts;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

type Documents = BTreeMap<String, String>;

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
enum Operation {
    Endpoints {
        config: String,
        base: String,
        candidate: String,
        #[serde(default)]
        scheduling: bool,
    },
    Request {
        request: chrono_harness::wire::Request,
    },
    Collection {
        request: chrono_harness::wire::Request,
        migration: Option<MigrationInputs>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    operation: Operation,
    originals: [Documents; 2],
}

fn snapshot(config: &str, documents: Documents) -> Result<facts::RegistrySnapshot, String> {
    let values: BTreeMap<_, _> = documents
        .iter()
        .map(|(path, original)| {
            Ok::<_, String>((path.clone(), chrono_harness::json(original.as_bytes())?))
        })
        .collect::<Result<_, _>>()?;
    let identity = facts::registry_identity(&values, config)
        .map_err(|error| format!("configuration selection: {error}"))?;
    Ok(facts::RegistrySnapshot {
        bytes: documents
            .into_iter()
            .map(|(k, v)| (k, v.into_bytes()))
            .collect(),
        values,
        entry_path: identity.entry_path,
        effective_path: identity.effective_path,
        selection: identity.selection,
    })
}

fn endpoint(registry: &Registrations) -> Result<Value, String> {
    let nodes = registry.node_data();
    let mut aliases = BTreeMap::<String, BTreeSet<String>>::new();
    for (test, bindings) in crate::execution::test_bindings(registry.projects())? {
        for binding in bindings {
            aliases
                .entry(binding.owner)
                .or_default()
                .insert(test.clone());
        }
    }
    let conflicts = registry
        .filemap()
        .get("execution_scheduling")
        .map(|value| serde_json::from_value::<Scheduling>(value.clone()))
        .transpose()
        .map_err(|error| error.to_string())?
        .map(|policy| {
            policy
                .claims
                .keys()
                .map(|operation| {
                    let related: Vec<_> = policy
                        .claims
                        .keys()
                        .filter(|other| *other == operation || policy.conflicts(operation, other))
                        .cloned()
                        .collect();
                    (operation.clone(), related)
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    Ok(json!({
        "config":registry.config(), "effective_config_path":registry.effective_config_path(),
        "filemap":registry.filemap(), "projects":registry.projects(), "judges":registry.judges(),
        "workflow":registry.workflow(), "original_schemas":registry.original_schemas(),
        "nodes":nodes, "active_test_aliases":aliases, "claim_conflicts":conflicts,
    }))
}

fn pair(old: &Registrations, current: &Registrations) -> Result<Value, String> {
    let repaired: BTreeSet<_> = old
        .node_data()
        .into_iter()
        .filter(|(_, view)| view.definitions.len() > 1)
        .filter_map(|(id, view)| {
            let definitions: Vec<_> = view.definitions.into_iter().map(|d| d.identity).collect();
            ambiguity_repaired(current, &id, &definitions).then_some(id)
        })
        .collect();
    Ok(json!({"base":endpoint(old)?, "candidate":endpoint(current)?, "repaired":repaired}))
}

/// Use the original collection evidence only for the exact requested units.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationInputs {
    units: Vec<String>,
    originals: chrono_harness::units::FullManifest,
}

pub fn query(root: &Path, input: impl Read) -> Result<Value, String> {
    let Query {
        operation,
        originals: [before, after],
    } = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    let (request, migration, collection) = match operation {
        Operation::Endpoints {
            config,
            base,
            candidate,
            scheduling,
        } => {
            facts::full_oid(&base)?;
            facts::full_oid(&candidate)?;
            let before = snapshot(&config, before)?.values;
            let after = snapshot(&config, after)?.values;
            let (old, current, facts) = if scheduling {
                let reader = facts::Reader::for_config(root, &config)?;
                let (old, current, _) = crate::interpret_scheduling_with_reader(
                    &reader, root, &base, &candidate, &config, before, after,
                )?;
                (old, current, Some(reader.observation()))
            } else {
                (
                    Registrations::load(&before, &config)?,
                    Registrations::load(&after, &config)?,
                    None,
                )
            };
            let mut reply = pair(&old, &current)?;
            reply["commits"] = json!([base, candidate]);
            if let Some(facts) = facts {
                reply["git_facts"] = facts;
            }
            return Ok(reply);
        }
        Operation::Request { request } => (request, None, false),
        Operation::Collection { request, migration } => (request, migration, true),
    };
    request.validate()?;
    if root != request.candidate.root
        || (collection
            && !matches!(
                request.scope,
                Some(chrono_harness::units::Scope::Collect { .. })
            ))
    {
        return Err("impact query requires its request checkout and matching scope".into());
    }
    let reader = std::cell::OnceCell::new();
    let get_reader = || {
        reader
            .get_or_init(|| facts::Reader::for_request(&request))
            .as_ref()
            .map_err(Clone::clone)
    };
    let load = |units: &[String]| {
        migration
            .as_ref()
            .filter(|original| original.units == units)
            .map(|original| original.originals.clone())
            .ok_or_else(|| "missing original migration inputs for the selected units".to_owned())
    };
    let project = || {
        let (old, current, view) = crate::transition::views_from_snapshots(
            &request,
            snapshot(&request.config_path, before)?,
            snapshot(&request.config_path, after)?,
            if collection { Some(&load) } else { None },
            get_reader,
        )?;
        let inputs = crate::inputs::validate(&request, &old, &current)?;
        let mut out = pair(&old, &current)?;
        match crate::inputs::effective_environments(&inputs, &old, &current) {
            Ok(env) => out["environments"] = json!(env),
            Err(error) => out["environment_error"] = json!(error),
        }
        if !collection {
            out["registration_view"] = view;
        }
        Ok::<_, String>(out)
    };
    let mut reply = match project() {
        Ok(value) => value,
        Err(error) => json!({"error":error}),
    };
    reply["request_id"] = json!(request.request_id);
    reply["commits"] = json!([request.base.commit, request.candidate.commit]);
    if let Some(Ok(reader)) = reader.get() {
        reply["git_facts"] = reader.observation();
    }
    Ok(reply)
}
