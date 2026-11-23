//! Scoped CI consumes only candidate execution tools, the conversion and selected retirements.
use crate::{Registrations, facts, transition};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

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

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    Schedule,
    Execute {
        environment: BTreeMap<String, String>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    config: String,
    git_facts: Value,
}

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
    let reader = std::cell::OnceCell::new();
    let get_reader = || {
        reader.get_or_init(|| match (&git, &mode) {
            (Some(binding), _) => facts::Reader::from_observations(
                root, &binding.config, &candidate,
                &json!({"git_facts":binding.git_facts,"environment":binding.git_facts["environment"]}),
            ),
            (None, Mode::Schedule) => Ok(facts::Reader::legacy()),
            (None, Mode::Execute { .. }) => {
                let reader = facts::Reader::for_config(root, &config)?;
                reader.verify_config(root, &candidate)?;
                Ok(reader)
            }
        }).as_ref().map_err(Clone::clone)
    };
    let result = (|| -> Result<Value, String> {
        let environment = match &mode {
            Mode::Schedule => None,
            Mode::Execute { environment } => Some(environment),
        };
        // A legacy execution has no caller-bound reader to check its configuration.
        if git.is_none() && environment.is_some() {
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
        Ok(
            json!({"tools":new.config()["tools"],"reused":transition::reused_tools(&view)?,
            "conversion":view["conversion"],"retirements":retirements(&old,&new,&retired)?}),
        )
    })();
    let observed = git.and_then(|_| reader.into_inner()?.ok().map(|reader| reader.observation()));
    Ok(json!({"previous":base,"candidate":candidate,"git_facts":observed,"result":result}))
}
