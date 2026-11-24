//! Registration's inputs from the published collection and unit-selection protocols.
use crate::wire;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactTransport {
    pub source_directory: String,
    pub directory: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FullManifest {
    pub schema: String,
    pub reports: Vec<Report>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub unit: String,
    pub path: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judge_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<ArtifactTransport>,
}

pub fn execution_units(config: &Value) -> Result<Option<Value>, String> {
    if config.get("execution_units").is_none() {
        return Ok(None);
    }
    wire::evidence(json!({"operation":"full-execution-units","profile":config}))
}

pub fn required(units: &Value, selected: &BTreeSet<String>) -> Result<Vec<String>, String> {
    wire::evidence(json!({"operation":"required-units","units":units,"selected":selected}))
}
