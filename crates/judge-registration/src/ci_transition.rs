//! Scoped CI consumes only candidate execution tools, the conversion and selected retirements.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
pub enum TransitionReader<T> { Observed(T), History, Configured }

pub fn transition_reader<T>(binding: Option<T>, execute: bool) -> (chosen: TransitionReader<T>)
    ensures chosen == match binding {
        Some(original) => TransitionReader::Observed(original),
        None if execute => TransitionReader::Configured,
        None => TransitionReader::History,
    },
{
    match binding {
        Some(original) => TransitionReader::Observed(original),
        None if execute => TransitionReader::Configured,
        None => TransitionReader::History,
    }
}

pub fn full_planning(full: bool, retired: usize) -> (out: Result<bool, &'static str>)
    ensures if full && retired > 0 {
        out matches Err(message) && message@ == "full scheduling determines retirements after registration"@
    } else { out == Ok::<bool, &str>(full) },
{
    if full && retired > 0 { Err("full scheduling determines retirements after registration") }
    else { Ok(full) }
}
/// CI exposes a completed Git observation only when the originating request supplied a binding.
pub(crate) fn complete_transition<T, O, E>(
    action: Result<T, E>, closed: Result<Option<O>, E>, requested: bool,
) -> (out: (Result<T, E>, Option<O>))
    ensures match closed {
        Ok(observation) => out.0 == action && out.1 == (if requested { observation } else { None }),
        Err(close_error) => out.1.is_none() && out.0 == match action {
            Err(action_error) => Err::<T, E>(action_error),
            Ok(_) => Err::<T, E>(close_error),
        },
    },
{
    super::historical_git::complete_read(action, closed, |observation: Option<O>| -> (retained: Option<O>)
        ensures retained == (if requested { observation } else { None }),
    {
        if requested { observation } else { None }
    })
}
}

#[cfg(not(verus_keep_ghost))]
use crate::{Registrations, facts, historical_git::InstalledGit, transition};
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    config: String,
    base: String,
    candidate: String,
    documents: [BTreeMap<String, Value>; 2],
    retired: BTreeSet<String>,
    mode: Mode,
    git: Option<Binding>,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    Schedule,
    FullSchedule,
    Execute {
        environment: BTreeMap<String, String>,
    },
}

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    config: String,
    git_facts: Value,
}

#[cfg(not(verus_keep_ghost))]
fn retirements(
    old: &Registrations,
    new: &Registrations,
    tests: &BTreeSet<String>,
) -> Result<Value, String> {
    let mut rows = BTreeMap::new();
    for test in tests {
        let replacement = transition::validate_test_retirement(old, new, test)?;
        let mut costs = old.filemap()["test_costs"]
            .as_array()
            .ok_or("E_RETIREMENT: old test costs missing")?
            .iter()
            .filter(|row| row["test"].as_str() == test.strip_prefix("test:"));
        let row = costs
            .next()
            .filter(|_| costs.next().is_none())
            .ok_or_else(|| format!("E_RETIREMENT: old test cost missing/ambiguous: {test}"))?;
        let reference = row["cost"].as_str().ok_or("old cost reference missing")?;
        let model = old.filemap()["cost_models"]
            .get(reference)
            .ok_or("old cost model missing")?;
        let definitions: Vec<_> = old.node_data()[test]
            .definitions
            .iter()
            .map(|d| json!({"identity":d.identity,"value":d.value}))
            .collect();
        rows.insert(
            test,
            json!({"replacement":replacement, "context":{
                "definitions":definitions,"cost":{"reference":reference,"value":model}
            }}),
        );
    }
    Ok(json!(rows))
}

#[cfg(not(verus_keep_ghost))]
pub fn query(root: &Path, input: impl Read) -> Result<Value, String> {
    let Query {
        config,
        base,
        candidate,
        documents: [old, new],
        retired,
        mode,
        git,
    } = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    facts::full_oid(&base)?;
    facts::full_oid(&candidate)?;
    let installed = std::cell::OnceCell::new();
    let reader_mode = transition_reader(git.as_ref(), matches!(mode, Mode::Execute { .. }));
    let get_reader = || {
        installed.get_or_init(|| match &reader_mode {
            TransitionReader::Observed(binding) => InstalledGit::observed(root, Some(&base), &candidate, &binding.config,
                &json!({"git_facts":binding.git_facts,"environment":binding.git_facts["environment"]})),
            TransitionReader::History =>
                InstalledGit::history(root,&base,&candidate),
            TransitionReader::Configured => {
                let reader = InstalledGit::configured(root, &base, &candidate, &config)?;
                reader.query::<()>(json!({"op":"verify-config","candidate":candidate}))?;
                Ok(reader)
            }
        }).as_ref().map_err(Clone::clone)
    };
    let result = (|| -> Result<Value, String> {
        let environment = match &mode {
            Mode::Schedule | Mode::FullSchedule => None,
            Mode::Execute { environment } => Some(environment),
        };
        // A legacy execution has no caller-bound reader to check its configuration.
        if matches!(reader_mode, TransitionReader::Configured) {
            get_reader()?;
        }
        let (old, new, view) = transition::interpret_ci_documents(
            get_reader,
            root,
            &base,
            &candidate,
            &config,
            old,
            new,
            environment,
        )?;
        if full_planning(matches!(mode, Mode::FullSchedule), retired.len())? {
            return Ok(planning(&old, &new));
        }
        Ok(
            json!({"tools":new.config()["tools"],"reused":transition::reused_tools(&view)?,
            "conversion":view["conversion"],"retirements":retirements(&old,&new,&retired)?}),
        )
    })();
    let receipt = installed
        .into_inner()
        .and_then(Result::ok)
        .map(InstalledGit::finish)
        .transpose();
    let (result, observed) = complete_transition(result, receipt, git.is_some());
    Ok(json!({"previous":base,"candidate":candidate,"git_facts":observed,"result":result}))
}

#[cfg(not(verus_keep_ghost))]
pub(crate) fn planning(old: &Registrations, new: &Registrations) -> Value {
    json!({"previous":crate::route_views::endpoint(old),
        "candidate":crate::route_views::endpoint(new),"test_operations":new.test_operations()})
}
