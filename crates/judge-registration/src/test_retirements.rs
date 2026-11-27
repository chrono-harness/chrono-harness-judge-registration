//! Resolve only the test retirements requested by a planner.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// Retained collection reads are scoped to the candidate's checkout and collection request.
pub(crate) fn read_collection<T, F>(
    same_checkout: bool, collection_scope: bool, read: F,
) -> (out: Result<T, String>)
    where F: FnOnce() -> Result<T, String>,
    requires (same_checkout && collection_scope) ==> read.requires(()),
    ensures if same_checkout && collection_scope { read.ensures((), out) } else {
        out matches Err(error) && error@ == "route collection requires this checkout and a collection scope"@
    },
{
    if !same_checkout || !collection_scope {
        return Err("route collection requires this checkout and a collection scope".to_string());
    }
    read()
}
}

#[cfg(not(verus_keep_ghost))]
use crate::collection;
#[cfg(not(verus_keep_ghost))]
use crate::wire::Request;
#[cfg(not(verus_keep_ghost))]
use crate::{downstream_validator, validate_test_retirement};
#[cfg(not(verus_keep_ghost))]
use crate::{historical_git::InstalledGit, transition::installed_views};
#[cfg(not(verus_keep_ghost))]
use serde::Deserialize;
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::{collections::BTreeMap, io::Read, path::Path};

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Migration {
    units: Vec<String>,
    originals: collection::FullManifest,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase", deny_unknown_fields)]
enum Endpoints {
    Documents {
        config: String,
        base: BTreeMap<String, Value>,
        candidate: BTreeMap<String, Value>,
    },
    View {
        config: String,
        view: Value,
    },
    Collection {
        request: Request,
        migration: Option<Migration>,
    },
}

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    endpoints: Endpoints,
    judge: String,
    retired_tests: Vec<String>,
}

#[cfg(not(verus_keep_ghost))]
pub fn query(root: &Path, input: impl Read) -> Result<Value, String> {
    let Query {
        endpoints,
        judge,
        retired_tests,
    } = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    let (old, current, request_id) = match endpoints {
        Endpoints::Documents {
            config,
            base,
            candidate,
        } => {
            let (old, current) =
                crate::snapshot_queries::interpret_pair((base, candidate), &config)?;
            (old, current, None)
        }
        Endpoints::View { config, view } => {
            let (old, current) = crate::transition::load_view(&view, &config)?;
            (old, current, None)
        }
        Endpoints::Collection { request, migration } => read_collection(
            request.candidate.root == root,
            matches!(request.scope, Some(crate::wire::Scope::Collect { .. })),
            || {
                let reports = |units: &[String]| {
                    migration
                        .as_ref()
                        .filter(|m| m.units == units)
                        .map(|m| m.originals.clone())
                        .ok_or_else(|| {
                            "missing original migration inputs for the selected units".to_owned()
                        })
                };
                let (old, current, _) = InstalledGit::with_request(&request, |reader| {
                    installed_views(&request, reader, Some(&reports))
                })
                .0?;
                Ok((old, current, Some(request.request_id)))
            },
        )?,
    };
    let mut decisions = BTreeMap::new();
    for test in retired_tests {
        decisions
            .entry(test.clone())
            .or_insert_with(|| validate_test_retirement(&old, &current, &test));
    }
    Ok(
        json!({"request_id":request_id, "downstream":downstream_validator(&current, &judge), "retirements":decisions}),
    )
}
