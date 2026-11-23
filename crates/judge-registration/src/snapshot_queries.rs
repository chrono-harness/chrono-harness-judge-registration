//! Interpret fixed registration data for offline snapshot consumers.
use crate::{Registrations, inputs};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Composition {
    config: String,
    base: BTreeMap<String, Value>,
    candidate: BTreeMap<String, Value>,
    selected: BTreeSet<String>,
}

pub fn composition(input: impl Read) -> Result<Value, String> {
    let query: Composition = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    let old = Registrations::load(&query.base, &query.config)?;
    let new = Registrations::load(&query.candidate, &query.config)?;
    Ok(json!({
        "base":inputs::collection_required_files("base",&old,&new,&query.selected)?,
        "candidate":inputs::collection_required_files("candidate",&old,&new,&query.selected)?,
    }))
}

/// Inspect the original snapshot in memory. The consumer applies endpoint identity
/// checks before reporting an environment error, in its original admission order.
pub fn inspect(input: impl Read) -> Result<Value, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Query {
        snapshot: Value,
        config: Option<Value>,
    }
    let query: Query = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    inputs::snapshot_shape(&query.snapshot)?;
    let environment_error = query
        .config
        .as_ref()
        .and_then(|config| inputs::environment(config, &query.snapshot["environment"]).err());
    Ok(json!({"schema":query.snapshot["schema"], "environment_error":environment_error}))
}
