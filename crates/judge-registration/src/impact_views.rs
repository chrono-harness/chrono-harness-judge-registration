//! Registration facts used by FILEMAP for a pair of fixed endpoints.
#![allow(unexpected_cfgs)]
use vstd::prelude::*;

verus! {
/// An endpoint's original documents are interpreted under the host's configuration.
pub trait EndpointDocuments: Sized {
    type Snapshot;
    type Error;
    spec fn projected(&self, config: &str) -> Result<Self::Snapshot, Self::Error>;
    fn interpret(self, config: &str) -> (result: Result<Self::Snapshot, Self::Error>)
        ensures result == self.projected(config);
}

/// Interpret the base before the candidate and preserve the first error.
pub fn interpret_pair<T: EndpointDocuments>(originals: (T, T), config: &str)
    -> (result: Result<(T::Snapshot, T::Snapshot), T::Error>)
    ensures result == match originals.0.projected(config) {
        Err(error) => Err(error),
        Ok(base) => match originals.1.projected(config) {
            Err(error) => Err(error),
            Ok(candidate) => Ok((base, candidate)),
        }
    },
{
    let (base, candidate) = originals;
    let base = match base.interpret(config) { Ok(value) => value, Err(error) => return Err(error) };
    match candidate.interpret(config) {
        Ok(value) => Ok((base, value)),
        Err(error) => Err(error),
    }
}
}

#[cfg(not(verus_keep_ghost))]
use crate::facts;
#[cfg(not(verus_keep_ghost))]
use crate::{Registrations, ambiguity_repaired, execution::Scheduling};
#[cfg(not(verus_keep_ghost))]
use serde::Deserialize;
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

#[cfg(not(verus_keep_ghost))]
type Documents = BTreeMap<String, String>;

#[cfg(not(verus_keep_ghost))]
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
    #[serde(skip)]
    Projects {
        request: crate::wire::Request,
        validate_inputs: bool,
    },
    #[serde(skip)]
    Workflow {
        request: crate::wire::Request,
        #[serde(default)]
        replay: bool,
    },
    Request {
        request: crate::wire::Request,
    },
    CiCollection {
        request: crate::wire::Request,
        migration: Option<MigrationInputs>,
    },
    Collection {
        request: crate::wire::Request,
        migration: Option<MigrationInputs>,
    },
}

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    operation: Operation,
    originals: (Documents, Documents),
}

#[cfg(not(verus_keep_ghost))]
impl EndpointDocuments for Documents {
    type Snapshot = facts::RegistrySnapshot;
    type Error = String;

    fn interpret(self, config: &str) -> Result<Self::Snapshot, Self::Error> {
        snapshot(config, self)
    }
}

#[cfg(not(verus_keep_ghost))]
pub(crate) fn snapshot(
    config: &str,
    documents: Documents,
) -> Result<facts::RegistrySnapshot, String> {
    let values: BTreeMap<_, _> = documents
        .iter()
        .map(|(path, original)| {
            Ok::<_, String>((path.clone(), crate::wire::json(original.as_bytes())?))
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

#[cfg(not(verus_keep_ghost))]
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

#[cfg(not(verus_keep_ghost))]
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
#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationInputs {
    units: Vec<String>,
    originals: crate::collection::FullManifest,
}

#[cfg(not(verus_keep_ghost))]
pub fn query(root: &Path, input: impl Read) -> Result<Value, String> {
    let query = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    project(root, query)
}

#[cfg(not(verus_keep_ghost))]
pub(crate) fn workflow(
    root: &Path,
    request: crate::wire::Request,
    replay: bool,
    originals: [Documents; 2],
) -> Result<Value, String> {
    project(
        root,
        Query {
            operation: Operation::Workflow { request, replay },
            originals: originals.into(),
        },
    )
}

#[cfg(not(verus_keep_ghost))]
pub(crate) fn projects(
    root: &Path,
    request: crate::wire::Request,
    originals: [Documents; 2],
    validate_inputs: bool,
) -> Result<Value, String> {
    project(
        root,
        Query {
            operation: Operation::Projects {
                request,
                validate_inputs,
            },
            originals: originals.into(),
        },
    )
}

#[cfg(not(verus_keep_ghost))]
fn project(root: &Path, query: Query) -> Result<Value, String> {
    let Query {
        operation,
        originals,
    } = query;
    let project_inputs = match &operation {
        Operation::Projects {
            validate_inputs, ..
        } => Some(*validate_inputs),
        _ => None,
    };
    let ci_collection = matches!(operation, Operation::CiCollection { .. });
    let workflow = matches!(operation, Operation::Workflow { .. });
    let replay = matches!(operation, Operation::Workflow { replay: true, .. });
    let (request, migration, collection) = match operation {
        Operation::Endpoints {
            config,
            base,
            candidate,
            scheduling,
        } => {
            facts::full_oid(&base)?;
            facts::full_oid(&candidate)?;
            let (before, after) = interpret_pair(originals, &config)?;
            let (before, after) = (before.values, after.values);
            let (old, current, facts) = if scheduling {
                let reader = crate::historical_git::InstalledGit::configured(
                    root, &base, &candidate, &config,
                )?;
                reader.query::<()>(json!({"op":"verify-config","candidate":candidate}))?;
                let result = crate::transition::interpret_ci_documents(
                    || Ok(&reader),
                    root,
                    &base,
                    &candidate,
                    &config,
                    before,
                    after,
                    None,
                );
                let observation = reader.finish();
                let (old, current, _) = result?;
                (old, current, Some(observation?))
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
        Operation::Projects { request, .. }
        | Operation::Workflow { request, .. }
        | Operation::Request { request } => (request, None, false),
        Operation::CiCollection { request, migration }
        | Operation::Collection { request, migration } => (request, migration, true),
    };
    request.validate()?;
    if root != request.candidate.root
        || (collection && !matches!(request.scope, Some(crate::wire::Scope::Collect { .. })))
    {
        return Err("impact query requires its request checkout and matching scope".into());
    }
    let installed = std::cell::OnceCell::new();
    let get_reader = || {
        installed
            .get_or_init(|| crate::historical_git::InstalledGit::open(&request))
            .as_ref()
            .map_err(Clone::clone)
    };
    let needed = std::cell::RefCell::new(None);
    let load = |units: &[String]| {
        if ci_collection && migration.is_none() {
            needed.replace(Some(units.to_vec()));
        }
        migration
            .as_ref()
            .filter(|original| original.units == units)
            .map(|original| original.originals.clone())
            .ok_or_else(|| "missing original migration inputs for the selected units".to_owned())
    };
    let project = || {
        let (before, after) = interpret_pair(originals, &request.config_path)?;
        let (old, current, view) = crate::transition::views_from_snapshots(
            &request,
            before,
            after,
            if collection { Some(&load) } else { None },
            get_reader,
            !ci_collection,
        )?;
        if ci_collection {
            return Ok(crate::ci_transition::planning(&old, &current));
        }
        if let Some(validate_inputs) = project_inputs {
            return Ok(crate::project_views::project(
                &request,
                &old,
                &current,
                view,
                validate_inputs,
            ));
        }
        if workflow {
            return Ok(if replay {
                json!({"replayed":true})
            } else {
                crate::workflow_views::project(&request, &old, &current, view)
            });
        }
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
    let projected = project();
    let mut reply = if ci_collection {
        match (needed.into_inner(), projected) {
            (Some(units), Err(_)) => json!({"outcome":{"needed":units}}),
            (_, Ok(planning)) => json!({"outcome":{"ready":planning}}),
            (_, Err(error)) => json!({"outcome":{"error":error}}),
        }
    } else {
        match projected {
            Ok(value) => value,
            Err(error) => json!({"error":error}),
        }
    };
    reply["request_id"] = json!(request.request_id);
    reply["commits"] = json!([request.base.commit, request.candidate.commit]);
    if let Some(Ok(installed)) = installed.into_inner() {
        reply["git_facts"] = installed.finish()?;
    }
    Ok(reply)
}
