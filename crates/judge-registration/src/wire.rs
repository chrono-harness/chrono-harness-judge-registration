//! Registration's inputs and findings on the published judge protocols.
#[cfg(not(verus_keep_ghost))]
use crate::runner_session::Stream;
#[cfg(not(verus_keep_ghost))]
use serde::{Deserialize, Serialize};
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use sha2::{Digest, Sha256};
#[cfg(not(verus_keep_ghost))]
use std::{collections::BTreeMap, path::PathBuf};

pub const PROTOCOL: &str = "chrono-judge/v1";
pub const INITIAL_PROTOCOL: &str = "chrono-initial-judge/v1";

#[cfg(not(verus_keep_ghost))]
pub fn json(bytes: &[u8]) -> Result<Value, String> {
    let value: serde_yaml_ng::Value =
        serde_json::from_slice(bytes).map_err(|error: serde_json::Error| {
            let message = error.to_string();
            if let Some((quoted, location)) = message
                .strip_prefix("duplicate entry with key ")
                .and_then(|tail| tail.rsplit_once(" at line "))
                && let Ok(key) = litrs::StringLit::parse(quoted)
            {
                return format!("duplicate JSON member {} at line {location}", key.value());
            }
            message
        })?;
    serde_json::to_value(value).map_err(|error| error.to_string())
}

#[cfg(not(verus_keep_ghost))]
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_value(json(bytes)?).map_err(|e| e.to_string())
}

/// Retain the exact source of an embedded document until duplicate checking.
#[cfg(not(verus_keep_ghost))]
pub(crate) fn strict_value<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let raw = Box::<serde_json::value::RawValue>::deserialize(d)?;
    decode(raw.get().as_bytes()).map_err(serde::de::Error::custom)
}

#[cfg(not(verus_keep_ghost))]
pub fn canonical<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_jcs::to_vec(value).map_err(|e| e.to_string())
}

#[cfg(not(verus_keep_ghost))]
pub fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(not(verus_keep_ghost))]
pub fn digest<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(sha256(&canonical(value)?))
}

#[cfg(not(verus_keep_ghost))]
pub fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(is_lower_hex_digit)
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Scope {
    Unit { unit: String },
    Collect { manifest: String },
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Delta {
    pub kind: String,
    pub path: String,
    pub old_blob: Option<String>,
    pub new_blob: Option<String>,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexFlag {
    pub path: String,
    pub tag: String,
}

#[cfg(not(verus_keep_ghost))]
impl IndexFlag {
    pub(crate) fn hides_changes(&self) -> bool {
        self.tag == "S"
            || self
                .tag
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_lowercase)
    }
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Checkout {
    pub head: String,
    pub tracked: Vec<String>,
    pub untracked: Vec<String>,
    pub index_flags: Vec<IndexFlag>,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub root: PathBuf,
    pub commit: String,
    pub tree: String,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registries {
    pub base: PathBuf,
    pub candidate: PathBuf,
    pub digest: String,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub path: PathBuf,
    pub sha256: String,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Executable {
    pub path: String,
    pub sha256: String,
    pub version: String,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    #[serde(default)]
    pub observations: Value,
    pub protocol: String,
    pub request_id: String,
    pub judge_id: String,
    pub mode: String,
    pub base: Endpoint,
    pub candidate: Endpoint,
    pub delta: Vec<Delta>,
    pub registries: Registries,
    pub context: Context,
    pub impact: Value,
    pub prior_results: Vec<Response>,
    pub config_path: String,
    pub checkout: Checkout,
    pub runner: Executable,
}

#[cfg(not(verus_keep_ghost))]
pub fn evidence<T: serde::de::DeserializeOwned>(operation: Value) -> Result<T, String> {
    Stream::with("judge-evidence-v1", None, |stream| {
        stream.exchange(&operation)
    })
}

/// Ask the same installed runner that launches judges to verify the fixed binding.
#[cfg(not(verus_keep_ghost))]
pub(crate) fn bound_judge_path(root: &std::path::Path, binding: &Value) -> Result<PathBuf, String> {
    Stream::with("judge-evidence-v1", Some(root), |stream| {
        stream.exchange(&json!({"operation":"bound-judge-path","binding":binding}))
    })
}

#[cfg(not(verus_keep_ghost))]
impl Request {
    pub fn validate(&self) -> Result<(), String> {
        evidence(json!({"operation":"validate-request", "request":self}))
    }

    pub fn response(&self, status: Status) -> Response {
        Response::new(PROTOCOL, &self.request_id, &self.judge_id, status)
    }
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialRequest {
    pub protocol: String,
    pub request_id: String,
    pub judge_id: String,
    pub mode: String,
    pub candidate: Endpoint,
    pub profile_path: String,
    pub profile_sha256: String,
    pub config_path: String,
    pub registry_digest: String,
    pub checkout: Checkout,
    pub runner: Executable,
    pub observations: Value,
    pub prior_results: Vec<Response>,
}

#[cfg(not(verus_keep_ghost))]
impl InitialRequest {
    pub fn validate(&self) -> Result<(), String> {
        evidence(json!({"operation":"validate-initial-request", "request":self}))
    }

    pub fn response(&self, status: Status) -> Response {
        Response::new(INITIAL_PROTOCOL, &self.request_id, &self.judge_id, status)
    }
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub protocol: String,
    pub request_id: String,
    pub judge_id: String,
    pub status: Status,
    pub findings: Vec<Finding>,
    pub evidence: Vec<Value>,
    pub outputs: BTreeMap<String, Value>,
}

#[cfg(not(verus_keep_ghost))]
impl Response {
    fn new(protocol: &str, request: &str, judge: &str, status: Status) -> Self {
        Self {
            protocol: protocol.to_owned(),
            request_id: request.to_owned(),
            judge_id: judge.to_owned(),
            status,
            findings: Vec::new(),
            evidence: Vec::new(),
            outputs: BTreeMap::new(),
        }
    }

    pub(crate) fn add_finding(&mut self, severity: Status, finding: Finding) {
        self.status = self.status.max(severity);
        self.findings.push(finding);
    }
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub code: String,
    pub level: String,
    pub message: String,
    pub delta_refs: Vec<String>,
    pub causes: Vec<String>,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Warn,
    Fail,
    Error,
}

#[cfg(not(verus_keep_ghost))]
impl Status {
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Pass | Self::Warn => 0,
            Self::Fail => 1,
            Self::Error => 2,
        }
    }
}

use vstd::prelude::*;

verus! {
fn is_lower_hex_digit(byte: u8) -> (valid: bool)
    ensures valid == (48 <= byte <= 57 || 97 <= byte <= 102),
{
    (48 <= byte && byte <= 57) || (97 <= byte && byte <= 102)
}
}
