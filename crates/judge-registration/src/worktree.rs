//! Registration decisions consumed by a worktree that holds fixed Git inputs.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// A valid config supplies the worktree's FILEMAP path; config errors have precedence.
pub(crate) fn admitted_filemap<'a>(validated: Result<(), String>, path: Option<&'a str>)
    -> (answer: Result<&'a str, String>)
    ensures match validated {
        Err(error) => answer == Err::<&str, _>(error),
        Ok(()) => match path {
            Some(name) => answer == Ok::<_, String>(name),
            None => answer matches Err(e) && e@ == "missing filemap registry path"@,
        }
    },
{
    match validated {
        Err(error) => Err(error),
        Ok(()) => match path {
            Some(name) => Ok(name),
            None => Err("missing filemap registry path".to_string()),
        }
    }
}
}

#[cfg(not(verus_keep_ghost))]
use crate::{Registrations, facts, schema};
#[cfg(not(verus_keep_ghost))]
use serde::Deserialize;
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::collections::BTreeMap;

#[cfg(not(verus_keep_ghost))]
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

#[cfg(not(verus_keep_ghost))]
pub(super) fn custody_filemap_path(config: &Value) -> Result<&str, String> {
    let validated = schema::config(config).map_err(|error| format!("config: {error}"));
    admitted_filemap(validated, config["registries"]["filemap"].as_str())
}

#[cfg(not(verus_keep_ghost))]
pub(super) fn validate_custody_filemap(filemap: &Value) -> Result<(), String> {
    schema::filemap(filemap).map_err(|e| format!("filemap: {e}"))
}

#[cfg(not(verus_keep_ghost))]
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
