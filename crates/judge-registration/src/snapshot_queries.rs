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
