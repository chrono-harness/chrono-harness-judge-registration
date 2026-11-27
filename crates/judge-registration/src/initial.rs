//! Initial inventory checks share reference policy without inventing prior history.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// An inventory root declares every governed registry proposed and has never activated enforcement.
pub(crate) fn initial_state(statuses: [Option<&str>; 5], enforcement: Option<&str>)
    -> (accepted: bool)
    ensures accepted == (super::facts::same_text(enforcement, "not-implemented")
        && forall|i: int| 0 <= i < 5 ==> super::facts::same_text(#[trigger] statuses@[i], "proposed")),
{
    broadcast use vstd::array::group_array_axioms;
    for i in 0..5usize
        invariant forall|j: int| 0 <= j < i ==> super::facts::same_text(#[trigger] statuses@[j], "proposed"),
    {
        if !super::facts::matches_text(statuses[i], "proposed") { return false; }
    }
    super::facts::matches_text(enforcement, "not-implemented")
}
}

#[cfg(not(verus_keep_ghost))]
use crate::facts;
#[cfg(not(verus_keep_ghost))]
use crate::inputs::checked_path;
#[cfg(not(verus_keep_ghost))]
use crate::wire::sha256;
#[cfg(not(verus_keep_ghost))]
use crate::wire::{self, Response, Status};
#[cfg(not(verus_keep_ghost))]
use crate::{Registrations, Result, historical_git::InstalledGit, issue};
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::fs;

#[cfg(not(verus_keep_ghost))]
use crate::wire::InitialRequest as Request;
#[cfg(not(verus_keep_ghost))]
pub fn judge(req: &Request) -> Response {
    let mut response = req.response(Status::Pass);
    let result = req.validate().and_then(|()| {
        let reader = InstalledGit::observed(
            &req.candidate.root,
            None,
            &req.candidate.commit,
            &req.config_path,
            &req.observations,
        )?;
        let result = evaluate(req, &mut response, &reader);
        let recorded = reader.record(&mut response);
        result.and(recorded)
    });
    if let Err(e) = result {
        issue(&mut response, "E_INITIAL_INPUT", e, "/request", true);
    }
    response
}

#[cfg(not(verus_keep_ghost))]
fn evaluate(req: &Request, response: &mut Response, reader: &InstalledGit) -> Result<()> {
    req.validate()?;
    let root = &req.candidate.root;
    let actual: std::path::PathBuf = reader.query(json!({"op":"checkout-root"}))?;
    if actual != *root
        || reader.query::<String>(json!({"op":"verify-tree","endpoint":"candidate"}))?
            != req.candidate.tree
        || !reader
            .query::<Vec<String>>(json!({"op":"parents","commit":req.candidate.commit}))?
            .is_empty()
    {
        return Err("initial requires the actual checkout root and a parentless candidate".into());
    }
    if !req.profile_path.starts_with(".chrono-harness/") || req.profile_path == req.config_path {
        return Err("initial profile must name a separate host config".into());
    }
    let bytes = fs::read(checked_path(root, &req.profile_path)?).map_err(|e| e.to_string())?;
    if sha256(&bytes) != req.profile_sha256
        || bytes != reader.blob(root, &req.candidate.commit, &req.profile_path)?
    {
        return Err("initial profile differs from fixed candidate".into());
    }
    let profile: Value = crate::wire::decode(&bytes)?;
    wire::evidence::<()>(json!({"operation":"validate-initial-profile","profile":profile}))?;
    if profile["host_config"] != req.config_path {
        return Err("initial host config mismatch".into());
    }
    let binding = profile["judges"]
        .as_array()
        .ok_or("initial judge bindings")?
        .iter()
        .find(|b| b["id"] == req.judge_id)
        .ok_or("initial judge unregistered")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let image = wire::bound_judge_path(root, binding)?;
    if fs::canonicalize(image).map_err(|e| e.to_string())?
        != fs::canonicalize(&exe).map_err(|e| e.to_string())?
        || binding["sha256"] != sha256(&fs::read(&exe).map_err(|e| e.to_string())?)
        || binding["version"] != env!("CARGO_PKG_VERSION")
    {
        return Err("initial binding differs from actual registration executable".into());
    }
    let snapshot = reader.snapshot("candidate")?;
    let values = snapshot.values;
    if wire::digest(&values)? != req.registry_digest {
        return Err("initial registry digest mismatch".into());
    }
    for (path, original) in &snapshot.bytes {
        if &fs::read(checked_path(root, path)?).map_err(|e| e.to_string())? != original {
            return Err(format!("checkout registry differs from candidate: {path}"));
        }
    }
    let binding = &req.observations["registry_binding"];
    if snapshot.selection.is_some()
        && (binding["entry_path"] != snapshot.entry_path
            || binding["effective_path"] != snapshot.effective_path
            || binding["selection"] != snapshot.selection.clone().unwrap_or(Value::Null))
    {
        return Err("initial registry selection binding mismatch".into());
    }
    let r = Registrations::load(&values, &req.config_path)?;
    // The selector entry is metadata, not one of the five status-bearing
    // registry documents.  Check the documents owned by the loaded view so a
    // selected initial profile is judged by the same proposed-state contract
    // as a direct profile.
    let statuses = [
        r.config(),
        r.judges(),
        r.projects(),
        r.filemap(),
        r.workflow(),
    ]
    .map(|document| document["status"].as_str());
    if !initial_state(statuses, r.config()["enforcement"].as_str()) {
        return Err(
            "initial inventory retains proposed registries; activation requires a real DELTA"
                .into(),
        );
    }
    let observed = reader.checkout_excluding(
        root,
        &req.candidate.commit,
        &facts::artifact_directories(r.config())?,
    )?;
    let nonartifact = |paths: &[String]| -> Vec<String> {
        paths
            .iter()
            .filter(|p| {
                !r.config()["artifacts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| p.starts_with(a["path"].as_str().unwrap()))
            })
            .cloned()
            .collect()
    };
    if observed.head != req.candidate.commit
        || observed.head != req.checkout.head
        || !observed.tracked.is_empty()
        || !req.checkout.tracked.is_empty()
        || !nonartifact(&observed.untracked).is_empty()
        || !nonartifact(&req.checkout.untracked).is_empty()
        || observed.index_flags != req.checkout.index_flags
    {
        return Err("initial candidate checkout is dirty or changed".into());
    }
    if observed.index_flags.iter().any(|flag| flag.hides_changes()) {
        return Err("initial inventory rejects unsupported index flags".into());
    }
    let inventory = reader.inventory()?;
    if inventory.delta.is_some() {
        return Err("initial inventory returned a baseline".into());
    }
    let tree = inventory.entries;
    if tree
        .values()
        .any(|e| e.kind != "blob" || !matches!(e.mode.as_str(), "100644" | "100755" | "120000"))
    {
        return Err("initial inventory rejects unsupported tree entries".into());
    }
    crate::references(
        reader,
        root,
        &req.candidate.commit,
        None,
        &r,
        &[],
        &tree,
        response,
    )?;
    if response.status == Status::Pass {
        response.outputs.insert("inventory".into(), json!({"scope":"schema-references-checkout",
            "candidate":req.candidate.commit,"tree":req.candidate.tree,"files":tree.len(),
            "registry_digest":req.registry_digest,"profile_sha256":req.profile_sha256,
            "governance":"not-evaluated","input_closure":r.config()["input_closure"],
            "remaining":["activation","judge execution for a real DELTA","complete input declarations and observations"]}));
    }
    Ok(())
}
