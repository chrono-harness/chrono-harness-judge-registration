//! Registration declarations and retained inputs used by the project executor.
use crate::wire::Request;
use crate::{Registrations, execution, historical_git::InstalledGit, inputs, transition};
use chrono_harness::prepared::ArtifactTransport;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, io::Read, path::Path};

pub(crate) fn project(
    request: &Request,
    old: &Registrations,
    current: &Registrations,
    view: Value,
    validate_inputs: bool,
) -> Value {
    let definitions: BTreeMap<_, _> = current
        .node_data()
        .into_iter()
        .filter_map(|(name, row)| {
            let owners: Vec<_> = row
                .definitions
                .into_iter()
                .map(|d| d.identity)
                .filter(|id| id.starts_with("project:") || id.starts_with("script:"))
                .collect();
            (!owners.is_empty()).then_some((name, owners))
        })
        .collect();
    json!({
        "config_path": current.effective_config_path(),
        "filemap": current.filemap(),
        "definitions": definitions,
        "bindings": current.test_bindings(),
        "retirements": transition::retirement_requests(current),
        "downstream": transition::downstream_validator(current, &request.judge_id),
        "scheduling": execution::scheduling(current.filemap(), current.projects(), &current.config()["artifacts"]),
        "effective_inputs": validate_inputs.then(|| inputs::validate(request, old, current)),
        "view": view,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Views {
    request: Request,
    originals: [BTreeMap<String, String>; 2],
    validate_inputs: bool,
}

pub fn views(input: impl Read) -> Result<Value, String> {
    let data: Views = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    let root = data.request.candidate.root.clone();
    crate::impact_views::projects(&root, data.request, data.originals, data.validate_inputs)
}

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
enum Evidence {
    Live,
    Retained {
        artifacts: Value,
        transport: Option<ArtifactTransport>,
    },
    Unit {
        supplied_view: Value,
        effective_inputs: Value,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    request: Request,
    view: Value,
    evidence: Evidence,
}

pub fn evidence(root: &Path, input: impl Read) -> Result<Value, String> {
    let Query {
        request,
        view,
        evidence,
    } = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    request.validate()?;
    let bound = &view["binding"];
    if bound["base"] != request.base.commit
        || bound["candidate"] != request.candidate.commit
        || bound["registry"] != request.registries.digest
    {
        return Err("project input registration view binding mismatch".into());
    }
    let (old, current) = transition::load_view(&view, &request.config_path)?;
    let effective = match evidence {
        Evidence::Live => inputs::validate(&request, &old, &current)?,
        Evidence::Retained {
            artifacts,
            transport,
        } => inputs::validate_retained_at(
            &request,
            &old,
            &current,
            &artifacts,
            root,
            transport.as_ref(),
        )?,
        Evidence::Unit {
            supplied_view,
            effective_inputs,
        } => {
            let reader = InstalledGit::configured(
                root,
                &request.base.commit,
                &request.candidate.commit,
                &request.config_path,
            )?;
            let base = reader.snapshot("base")?;
            let candidate = reader.snapshot("candidate")?;
            let result = transition::validate_retained_snapshots(
                &request,
                &reader,
                root,
                &supplied_view,
                base,
                candidate,
            );
            let closed = reader.finish();
            result?;
            closed?;
            inputs::project_effective(&effective_inputs, &old, &current, request.scope.as_ref())?
        }
    };
    Ok(json!({"request_id":request.request_id,"effective_inputs":effective}))
}

pub fn check_projects(input: impl Read) -> Result<Value, String> {
    let registry: Value = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    crate::validate_projects(&registry)?;
    Ok(Value::Null)
}
