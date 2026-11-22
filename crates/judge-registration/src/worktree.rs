//! Registration decisions consumed by a worktree that holds fixed Git inputs.
use crate::{Registrations, facts, schema};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
enum Query {
    Snapshot {
        entry: String,
        documents: BTreeMap<String, Value>,
    },
    CustodyConfig {
        config: Value,
    },
    CustodyFilemap {
        filemap: Value,
    },
    NonartifactPaths {
        config: Value,
        paths: Vec<String>,
    },
}

pub(super) fn custody_filemap_path(config: &Value) -> Result<&str, String> {
    schema::config(config).map_err(|e| format!("config: {e}"))?;
    config["registries"]["filemap"]
        .as_str()
        .ok_or_else(|| "missing filemap registry path".into())
}

pub(super) fn validate_custody_filemap(filemap: &Value) -> Result<(), String> {
    schema::filemap(filemap).map_err(|e| format!("filemap: {e}"))
}

pub fn query(input: impl std::io::Read) -> Result<Value, String> {
    match serde_json::from_reader(input).map_err(|e| e.to_string())? {
        Query::Snapshot { entry, documents } => {
            let admitted = Registrations::load(&documents, &entry)?;
            // The caller already holds the source config, projects and workflow.
            // Only FILEMAP is transformed by registration's configuration selection.
            Ok(
                json!({"effective_config":admitted.effective_config_path(),"filemap":admitted.filemap()}),
            )
        }
        Query::CustodyConfig { config } => {
            Ok(json!({"filemap_path":custody_filemap_path(&config)?}))
        }
        Query::CustodyFilemap { filemap } => {
            validate_custody_filemap(&filemap)?;
            Ok(json!({"valid":true}))
        }
        Query::NonartifactPaths { config, paths } => {
            let directories = facts::artifact_directories(&config)?;
            Ok(json!({"paths":super::outside_artifacts(&directories,&paths)}))
        }
    }
}
