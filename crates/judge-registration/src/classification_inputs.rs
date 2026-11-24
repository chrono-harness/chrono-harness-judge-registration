//! Resolve caller-selected classification documents using one request-bound Git reader.
use crate::{
    historical_git::{HistoricalGit, InstalledGit},
    transition::installed_views,
};
use chrono_harness::{CommandSpec, wire::Request};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    base: Vec<String>,
    candidate: Vec<String>,
    context: Value,
}

/// Resolve known interpreted documents before consulting the selected endpoint's blob reader.
pub fn documents(
    selected: &[String],
    present: &BTreeSet<&str>,
    known: &Value,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<BTreeMap<String, Value>, String> {
    let mut docs = BTreeMap::new();
    for path in selected {
        if !present.contains(path.as_str()) || docs.contains_key(path) {
            return Err(format!(
                "E_SEMANTIC_INPUT: invalid selected document {path}"
            ));
        }
        chrono_harness::relative_path(path)?;
        let doc = match known.get(path) {
            Some(doc) => doc.clone(),
            None => chrono_harness::decode(&read(path)?)
                .map_err(|error| format!("E_SEMANTIC_INPUT: {path}: {error}"))?,
        };
        docs.insert(path.clone(), doc);
    }
    Ok(docs)
}

fn prepare(req: &Request, reader: &InstalledGit, selector: &Path) -> Result<Value, String> {
    let (a, b, _) = installed_views(req, reader, None)?;
    let changed = req.delta.iter().map(|d| d.path.as_str()).collect();
    let base = a.classification_view(&changed);
    let candidate = b.classification_view(&changed);
    let protocol = &b.config()["protocol"];
    if !selector.is_absolute() {
        return Err("E_PROTOCOL: classification selector must be an absolute path".into());
    }
    let spec = CommandSpec {
        program: selector
            .to_str()
            .ok_or("classification selector is not UTF-8")?
            .into(),
        args: vec!["plan-classification-v1".into()],
        env: serde_json::from_value(req.observations["environment"]["effective"].clone())
            .map_err(|e| e.to_string())?,
        timeout_seconds: protocol["timeout_seconds"]
            .as_u64()
            .ok_or("invalid classification timeout")?,
        output_limit_bytes: serde_json::from_value(protocol["stdout_limit_bytes"].clone())
            .map_err(|e| e.to_string())?,
    };
    let input = json!({
        "base": base["endpoint"], "candidate":candidate["endpoint"], "delta":req.delta,
        "impact":req.impact,
        "prior_impacts":req.prior_results.iter().filter_map(|r|r.outputs.get("impact")).collect::<Vec<_>>(),
        "prior_costs":req.prior_results.iter().filter_map(|r|r.outputs.get("costs")).collect::<Vec<_>>(),
        "binding":{"base":req.base.commit,"candidate":req.candidate.commit,"candidate_tree":req.candidate.tree,
                   "registry_digest":req.registries.digest,"context_digest":req.context.sha256}
    });
    let result = chrono_harness::run_process_observed(
        &req.candidate.root,
        &spec,
        &chrono_harness::wire::canonical(&input)?,
        &chrono_harness::file_identity(selector)?.0,
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
    let plan: Plan = chrono_harness::decode(&result.stdout_bytes)?;
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
