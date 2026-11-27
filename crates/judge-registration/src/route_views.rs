//! Registration-owned inputs for the route planner's native judge protocol.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
pub(crate) enum InputValidation { Collection, Live }

/// Routes also bind the request context before deciding how to validate retained inputs.
pub(crate) fn input_validation(bound: bool, context: Option<&str>, expected: &str, collect: bool)
    -> (decision: Result<InputValidation, &'static str>)
    ensures if bound && super::facts::same_text(context, expected) {
        decision == Ok::<_, &str>(if collect { InputValidation::Collection } else { InputValidation::Live })
    } else {
        decision matches Err(message) && message@ == "route input registration view binding mismatch"@
    },
{
    if !bound || !super::facts::matches_text(context, expected) {
        return Err("route input registration view binding mismatch");
    }
    Ok(if collect { InputValidation::Collection } else { InputValidation::Live })
}
}

#[cfg(not(verus_keep_ghost))]
use crate::wire::{Request, Scope};
#[cfg(not(verus_keep_ghost))]
use crate::{Registrations, execution, historical_git::InstalledGit, inputs, transition};
#[cfg(not(verus_keep_ghost))]
use serde::Deserialize;
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::{collections::BTreeSet, io::Read};

#[cfg(not(verus_keep_ghost))]
pub(crate) fn endpoint(r: &Registrations) -> Value {
    json!({"config":r.config(), "filemap":r.filemap(), "projects":r.projects()})
}

#[cfg(not(verus_keep_ghost))]
/// Interpret the actual fixed commits once, retaining the producing Git reader on errors too.
pub fn views(input: impl Read) -> Result<Value, String> {
    let request: Request = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    let (result, observation) = InstalledGit::with_request(&request, |reader| {
        let (old, current, view) = transition::installed_views(&request, reader, None)?;
        Ok(json!({"base":endpoint(&old),"candidate":endpoint(&current),
            "methods":execution::methods(current.projects()),"plans":execution::plans(current.filemap()),
            "scheduling":execution::scheduling(current.filemap(),current.projects(),&current.config()["artifacts"]),
            "test_operations":current.test_operations(),"reused":transition::reused_tools(&view),"registration_view":view}))
    });
    let mut reply = json!({"request_id":request.request_id,"commits":[request.base.commit,request.candidate.commit],"result":result});
    if let Some(observation) = observation {
        reply["git_facts"] = observation;
    }
    Ok(reply)
}

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    request: Request,
    view: Value,
    selected: BTreeSet<String>,
}

#[cfg(not(verus_keep_ghost))]
pub fn effective_inputs(input: impl Read) -> Result<Value, String> {
    let Inputs {
        request,
        view,
        selected,
    } = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    request.validate()?;
    let validation = input_validation(
        crate::facts::input_view_matches(&view, &request),
        view["binding"]["context"].as_str(),
        &request.context.sha256,
        matches!(request.scope, Some(Scope::Collect { .. })),
    )?;
    let (old, current) = transition::load_view(&view, &request.config_path)?;
    let effective = match validation {
        InputValidation::Collection => {
            inputs::validate_collection(&request, &old, &current, &selected)?
        }
        InputValidation::Live => inputs::validate(&request, &old, &current)?,
    };
    Ok(json!({"request_id":request.request_id,"effective_inputs":effective}))
}
