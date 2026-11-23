//! Resolve only the test retirements requested by a planner.
use crate::{Registrations, downstream_validator, validate_test_retirement};
use chrono_harness::{facts, units, wire::Request};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, io::Read, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Migration {
    units: Vec<String>,
    originals: units::FullManifest,
}

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase", deny_unknown_fields)]
enum Endpoints {
    Documents {
        config: String,
        base: BTreeMap<String, Value>,
        candidate: BTreeMap<String, Value>,
    },
    Collection {
        request: Request,
        migration: Option<Migration>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    endpoints: Endpoints,
    judge: String,
    retired_tests: Vec<String>,
}

pub fn query(root: &Path, input: impl Read) -> Result<Value, String> {
    let Query {
        endpoints,
        judge,
        retired_tests,
    } = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    let (old, current, request_id) = match endpoints {
        Endpoints::Documents {
            config,
            base,
            candidate,
        } => (
            Registrations::load(&base, &config)?,
            Registrations::load(&candidate, &config)?,
            None,
        ),
        Endpoints::Collection { request, migration } => {
            if request.candidate.root != root
                || !matches!(request.scope, Some(units::Scope::Collect { .. }))
            {
                return Err(
                    "route collection requires this checkout and a collection scope".into(),
                );
            }
            let reader = facts::Reader::for_request(&request)?;
            let (old, current, _) =
                crate::views_for_collection_inputs_with(&request, &reader, &|units| {
                    migration
                        .as_ref()
                        .filter(|m| m.units == units)
                        .map(|m| m.originals.clone())
                        .ok_or_else(|| {
                            "missing original migration inputs for the selected units".to_owned()
                        })
                })?;
            (old, current, Some(request.request_id))
        }
    };
    let mut decisions = BTreeMap::new();
    for test in retired_tests {
        decisions
            .entry(test.clone())
            .or_insert_with(|| validate_test_retirement(&old, &current, &test));
    }
    Ok(
        json!({"request_id":request_id, "downstream":downstream_validator(&current, &judge), "retirements":decisions}),
    )
}
