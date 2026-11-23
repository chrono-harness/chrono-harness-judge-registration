//! Registration-owned inputs for the route planner's native judge protocol.
use crate::{Registrations, execution, facts, inputs, transition};
use chrono_harness::{units::Scope, wire::Request};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, io::Read};

pub(crate) fn endpoint(r: &Registrations) -> Value {
    json!({"config":r.config(), "filemap":r.filemap(), "projects":r.projects()})
}

/// Interpret the actual fixed commits once, retaining the producing Git reader on errors too.
pub fn views(input: impl Read) -> Result<Value, String> {
    let request: Request = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    let reader = facts::Reader::for_request(&request);
    let result = reader.as_ref().map_err(Clone::clone).and_then(|reader| {
        let (old, current, view) = transition::views_with_reader(&request, reader)?;
        Ok(json!({"base":endpoint(&old),"candidate":endpoint(&current),
            "methods":execution::methods(current.projects()),"plans":execution::plans(current.filemap()),
            "scheduling":execution::scheduling(current.filemap(),current.projects(),&current.config()["artifacts"]),
            "test_operations":current.test_operations(),"reused":transition::reused_tools(&view),"registration_view":view}))
    });
    let mut reply = json!({"request_id":request.request_id,"commits":[request.base.commit,request.candidate.commit],"result":result});
    if let Ok(reader) = reader
        && reader.is_bound()
    {
        reply["git_facts"] = reader.observation();
    }
    Ok(reply)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    request: Request,
    view: Value,
    selected: BTreeSet<String>,
}

pub fn effective_inputs(input: impl Read) -> Result<Value, String> {
    let Inputs {
        request,
        view,
        selected,
    } = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    request.validate()?;
    let binding = &view["binding"];
    if binding["base"] != request.base.commit
        || binding["candidate"] != request.candidate.commit
        || binding["context"] != request.context.sha256
        || binding["registry"] != request.registries.digest
    {
        return Err("route input registration view binding mismatch".into());
    }
    let (old, current) = transition::load_view(&view, &request.config_path)?;
    let effective = if matches!(request.scope, Some(Scope::Collect { .. })) {
        inputs::validate_collection(&request, &old, &current, &selected)?
    } else {
        inputs::validate(&request, &old, &current)?
    };
    Ok(json!({"request_id":request.request_id,"effective_inputs":effective}))
}
