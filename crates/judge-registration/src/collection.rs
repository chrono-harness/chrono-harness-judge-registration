//! Registration's inputs from the published collection and unit-selection protocols.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// Only declared execution units are interpreted by the runner's full-profile reader.
pub(crate) fn declared_units<T, E, F>(declared: bool, read: F) -> (answer: Result<Option<T>, E>)
    where F: FnOnce() -> Result<Option<T>, E>,
    requires declared ==> read.requires(()),
    ensures if declared { read.ensures((), answer) } else { answer == Ok(None) },
{
    if declared { read() } else { Ok(None) }
}

/// Candidate assignment is read first; historical assignment has meaning only for
/// a base projection when that named unit was declared at the base.
pub fn assignments<T, E, F>(candidate: Result<T, E>, base: bool, old_unit: bool, read_old: F)
    -> (out: Result<(T, Option<T>), E>)
    where F: FnOnce() -> Result<T, E>,
    requires candidate.is_ok() && base && old_unit ==> read_old.requires(()),
    ensures match candidate {
        Err(error) => out == Err::<(T, Option<T>), E>(error),
        Ok(current) => if base && old_unit {
            exists|old: Result<T, E>| read_old.ensures((), old) && out == match old {
                Ok(plans) => Ok((current, Some(plans))),
                Err(error) => Err(error),
            }
        } else { out == Ok((current, None)) },
    },
{
    let current = match candidate { Ok(value) => value, Err(error) => return Err(error) };
    if base && old_unit {
        match read_old() {
            Ok(plans) => Ok((current, Some(plans))),
            Err(error) => Err(error),
        }
    } else { Ok((current, None)) }
}

}

#[cfg(not(verus_keep_ghost))]
use crate::wire;
#[cfg(not(verus_keep_ghost))]
use serde::{Deserialize, Serialize};
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::collections::BTreeSet;

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactTransport {
    pub source_directory: String,
    pub directory: String,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FullManifest {
    pub schema: String,
    pub reports: Vec<Report>,
}

#[cfg(not(verus_keep_ghost))]
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

#[cfg(not(verus_keep_ghost))]
pub fn execution_units(config: &Value) -> Result<Option<Value>, String> {
    declared_units(config.get("execution_units").is_some(), || {
        wire::evidence(json!({"operation":"full-execution-units","profile":config}))
    })
}

#[cfg(not(verus_keep_ghost))]
pub fn required(units: &Value, selected: &BTreeSet<String>) -> Result<Vec<String>, String> {
    wire::evidence(json!({"operation":"required-units","units":units,"selected":selected}))
}
