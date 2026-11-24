//! Fixed registration documents and their identities on the runner protocols.
use crate::wire;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigIdentity {
    pub entry_path: String,
    pub effective_path: String,
    pub selection: Option<Value>,
}

pub struct RegistrySnapshot {
    pub entry_path: String,
    pub effective_path: String,
    pub selection: Option<Value>,
    pub values: BTreeMap<String, Value>,
    pub bytes: BTreeMap<String, Vec<u8>>,
}

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

pub fn registry_digest(
    base: &BTreeMap<String, Value>,
    candidate: &BTreeMap<String, Value>,
) -> Result<String, String> {
    wire::digest(&json!({"base":base,"candidate":candidate}))
}

pub fn artifact_directories(config: &Value) -> Result<Vec<String>, String> {
    wire::evidence(json!({"operation":"artifact-directories","config":config}))
}

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

pub fn utf8(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes).map_err(|_| "E_INPUT_UNSUPPORTED: non UTF-8 Git output".into())
}
