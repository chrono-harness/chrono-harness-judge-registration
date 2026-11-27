//! Resolve caller-selected classification documents using one request-bound Git reader.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// A selected path belongs to this endpoint exactly once. Keep its interpreted value if present.
pub(crate) fn selected_document<T>(present: bool, duplicate: bool, known: Option<&T>)
    -> (result: std::result::Result<Option<&T>, ()>)
    ensures result == if present && !duplicate { Ok(known) } else { Err(()) },
{
    if !present || duplicate { return Err(()); }
    Ok(known)
}
}

#[cfg(not(verus_keep_ghost))]
use crate::runner_session;
#[cfg(not(verus_keep_ghost))]
use crate::wire::Request;
#[cfg(not(verus_keep_ghost))]
use crate::{historical_git::InstalledGit, transition::installed_views};
#[cfg(not(verus_keep_ghost))]
use serde::Deserialize;
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    base: Vec<String>,
    candidate: Vec<String>,
    context: Value,
}

#[cfg(not(verus_keep_ghost))]
/// Resolve known interpreted documents before consulting the selected endpoint's blob reader.
pub fn documents(
    selected: &[String],
    present: &BTreeSet<&str>,
    known: &Value,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<BTreeMap<String, Value>, String> {
    let mut docs = BTreeMap::new();
    for path in selected {
        let source = selected_document(
            present.contains(path.as_str()),
            docs.contains_key(path),
            known.get(path),
        )
        .map_err(|()| format!("E_SEMANTIC_INPUT: invalid selected document {path}"))?;
        crate::schema::relative_path(path)?;
        let doc = match source {
            Some(doc) => doc.clone(),
            None => crate::wire::decode(&read(path)?)
                .map_err(|error| format!("E_SEMANTIC_INPUT: {path}: {error}"))?,
        };
        docs.insert(path.clone(), doc);
    }
    Ok(docs)
}

#[cfg(not(verus_keep_ghost))]
fn prepare(req: &Request, reader: &InstalledGit, selector: &Path) -> Result<Value, String> {
    let (a, b, _) = installed_views(req, reader, None)?;
    let changed = req.delta.iter().map(|d| d.path.as_str()).collect();
    let base = a.classification_view(&changed);
    let candidate = b.classification_view(&changed);
    let protocol = &b.config()["protocol"];
    if !selector.is_absolute() {
        return Err("E_PROTOCOL: classification selector must be an absolute path".into());
    }
    let env = serde_json::from_value(req.observations["environment"]["effective"].clone())
        .map_err(|e| e.to_string())?;
    let (timeout, output_bytes) = crate::schema::protocol_limits(protocol)?;
    let limit = usize::try_from(output_bytes).map_err(|e| e.to_string())?;
    let input = json!({
        "base": base["endpoint"], "candidate":candidate["endpoint"], "delta":req.delta,
        "impact":req.impact,
        "prior_impacts":req.prior_results.iter().filter_map(|r|r.outputs.get("impact")).collect::<Vec<_>>(),
        "prior_costs":req.prior_results.iter().filter_map(|r|r.outputs.get("costs")).collect::<Vec<_>>(),
        "binding":{"base":req.base.commit,"candidate":req.candidate.commit,"candidate_tree":req.candidate.tree,
                   "registry_digest":req.registries.digest,"context_digest":req.context.sha256}
    });
    let result: crate::transition::Process = runner_session::record(
        &req.candidate.root,
        selector
            .to_str()
            .ok_or("classification selector is not UTF-8")?,
        &["plan-classification-v1".into()],
        &env,
        timeout,
        limit,
        &crate::inputs::file_identity(selector)?.0,
        &crate::wire::canonical(&input)?,
    )?;
    if let Some(error) = result.failure {
        return Err(error);
    }
    if result.exit_code != 0 {
        return Err(if result.stderr.trim_end().is_empty() {
            format!("classification selector exited {}", result.exit_code)
        } else {
            result.stderr.trim_end().into()
        });
    }
    let plan: Plan = crate::wire::decode(&result.stdout_bytes)?;
    let named: BTreeSet<_> = [&a, &b]
        .iter()
        .flat_map(|endpoint| endpoint.config()["semantic_fields"].as_array().unwrap())
        .filter(|rule| !rule["pointers"].as_array().unwrap().is_empty())
        .filter_map(|r| r["path"].as_str())
        .collect();
    let present = |candidate: bool| {
        req.delta
            .iter()
            .filter(|d| {
                named.contains(d.path.as_str())
                    && if candidate {
                        d.new_blob.is_some()
                    } else {
                        d.old_blob.is_some()
                    }
            })
            .map(|d| d.path.as_str())
            .collect()
    };
    let read = |commit: &str, path: &str| reader.blob(&req.candidate.root, commit, path);
    let before = documents(&plan.base, &present(false), &base["documents"], |p| {
        read(&req.base.commit, p)
    })?;
    let after = documents(
        &plan.candidate,
        &present(true),
        &candidate["documents"],
        |p| read(&req.candidate.commit, p),
    )?;
    Ok(
        json!({"base":base["endpoint"],"candidate":candidate["endpoint"],"warning_code":candidate["warning_code"],
       "delta":req.delta,"before":before,"after":after,"context":plan.context}),
    )
}

#[cfg(not(verus_keep_ghost))]
/// Judge pipeline input producer. Data errors preserve every completed Git read.
pub fn query(req: &Request, selector: &Path) -> Value {
    let mut outputs = BTreeMap::new();
    let (result, observation) =
        InstalledGit::with_request(req, |reader| prepare(req, reader, selector));
    if let Some(observation) = observation {
        outputs.insert("git_facts", observation);
    }
    json!({"request_id":req.request_id,"outputs":outputs,"result":result})
}
