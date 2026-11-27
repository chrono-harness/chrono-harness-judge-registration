//! Interpret fixed registration data for offline snapshot consumers.
use vstd::prelude::*;

verus! {
/// An endpoint's original documents are interpreted under the host's configuration.
pub trait EndpointDocuments: Sized {
    type Snapshot;
    type Error;
    spec fn projected(&self, config: &str) -> Result<Self::Snapshot, Self::Error>;
    fn interpret(self, config: &str) -> (result: Result<Self::Snapshot, Self::Error>)
        ensures result == self.projected(config);
}

/// Interpret the base before the candidate and preserve the first error.
pub fn interpret_pair<T: EndpointDocuments>(originals: (T, T), config: &str)
    -> (result: Result<(T::Snapshot, T::Snapshot), T::Error>)
    ensures result == match originals.0.projected(config) {
        Err(error) => Err(error),
        Ok(base) => match originals.1.projected(config) {
            Err(error) => Err(error),
            Ok(candidate) => Ok((base, candidate)),
        }
    },
{
    let (base, candidate) = originals;
    let base = match base.interpret(config) { Ok(value) => value, Err(error) => return Err(error) };
    match candidate.interpret(config) {
        Ok(value) => Ok((base, value)),
        Err(error) => Err(error),
    }
}
}

#[cfg(not(verus_keep_ghost))]
use crate::Registrations;
#[cfg(not(verus_keep_ghost))]
use crate::inputs;
#[cfg(not(verus_keep_ghost))]
use serde::Deserialize;
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Composition {
    config: String,
    base: BTreeMap<String, Value>,
    candidate: BTreeMap<String, Value>,
    selected: BTreeSet<String>,
}

#[cfg(not(verus_keep_ghost))]
impl EndpointDocuments for BTreeMap<String, Value> {
    type Snapshot = Registrations;
    type Error = String;

    fn interpret(self, config: &str) -> Result<Self::Snapshot, Self::Error> {
        Registrations::load(&self, config)
    }
}

#[cfg(not(verus_keep_ghost))]
pub fn composition(input: impl Read) -> Result<Value, String> {
    let query: Composition = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    let (old, new) = interpret_pair((query.base, query.candidate), &query.config)?;
    Ok(json!({
        "base":inputs::collection_required_files("base",&old,&new,&query.selected)?,
        "candidate":inputs::collection_required_files("candidate",&old,&new,&query.selected)?,
    }))
}
