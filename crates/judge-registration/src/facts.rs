//! Fixed registration documents and their identities on the runner protocols.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
pub(crate) open spec fn same_text(observed: Option<&str>, expected: &str) -> bool {
    match observed { Some(value) => value@ == expected@, None => false }
}

pub(crate) fn matches_text(observed: Option<&str>, expected: &str) -> (matched: bool)
    ensures matched == same_text(observed, expected),
{
    match observed { Some(value) => value == expected, None => false }
}

/// Identity of the originally requested pair and its combined registration digest.
pub(crate) struct InputViewIdentity<'a> {
    pub base: &'a str,
    pub candidate: &'a str,
    pub registry: &'a str,
}

impl InputViewIdentity<'_> {
    pub(crate) fn matches(&self, base: Option<&str>, candidate: Option<&str>, registry: Option<&str>)
        -> (accepted: bool)
        ensures accepted == (same_text(base, self.base)
            && same_text(candidate, self.candidate) && same_text(registry, self.registry)),
    {
        matches_text(base, self.base) && matches_text(candidate, self.candidate) && matches_text(registry, self.registry)
    }
}
}

#[cfg(not(verus_keep_ghost))]
pub(crate) fn input_view_matches(view: &Value, request: &crate::wire::Request) -> bool {
    let expected = InputViewIdentity {
        base: &request.base.commit,
        candidate: &request.candidate.commit,
        registry: &request.registries.digest,
    };
    let binding = &view["binding"];
    expected.matches(
        binding["base"].as_str(),
        binding["candidate"].as_str(),
        binding["registry"].as_str(),
    )
}

#[cfg(not(verus_keep_ghost))]
use crate::wire;
#[cfg(not(verus_keep_ghost))]
use serde::{Deserialize, Serialize};
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::collections::BTreeMap;

#[cfg(not(verus_keep_ghost))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigIdentity {
    pub entry_path: String,
    pub effective_path: String,
    pub selection: Option<Value>,
}

#[cfg(not(verus_keep_ghost))]
pub struct RegistrySnapshot {
    pub entry_path: String,
    pub effective_path: String,
    pub selection: Option<Value>,
    pub values: BTreeMap<String, Value>,
    pub bytes: BTreeMap<String, Vec<u8>>,
}

#[cfg(not(verus_keep_ghost))]
pub fn registry_identity(
    documents: &BTreeMap<String, Value>,
    entry: &str,
) -> Result<ConfigIdentity, String> {
    let config = documents.get(entry).ok_or("missing configuration entry")?;
    if config.get("schema").is_none() {
        return Ok(ConfigIdentity {
            entry_path: entry.into(),
            effective_path: entry.into(),
            selection: None,
        });
    }
    wire::evidence(json!({"operation":"registry-identity","entry":entry,"documents":documents}))
}

#[cfg(not(verus_keep_ghost))]
pub fn registry_digest(
    base: &BTreeMap<String, Value>,
    candidate: &BTreeMap<String, Value>,
) -> Result<String, String> {
    wire::digest(&json!({"base":base,"candidate":candidate}))
}

#[cfg(not(verus_keep_ghost))]
pub fn artifact_directories(config: &Value) -> Result<Vec<String>, String> {
    wire::evidence(json!({"operation":"artifact-directories","config":config}))
}

#[cfg(not(verus_keep_ghost))]
pub fn full_oid(oid: &str) -> Result<(), String> {
    if matches!(oid.len(), 40 | 64)
        && oid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && oid.bytes().any(|b| b != b'0')
    {
        Ok(())
    } else {
        Err("expected full nonzero lowercase commit OID".into())
    }
}

#[cfg(not(verus_keep_ghost))]
pub fn utf8(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes).map_err(|_| "E_INPUT_UNSUPPORTED: non UTF-8 Git output".into())
}
