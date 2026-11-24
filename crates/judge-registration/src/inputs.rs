//! Completeness declarations and retained endpoint bytes; a digest is not a closure proof.
use super::schema;
#[cfg(not(verus_keep_ghost))]
use crate::Registrations;
#[cfg(not(verus_keep_ghost))]
use crate::facts;
#[cfg(not(verus_keep_ghost))]
use crate::wire::{self, Request};
#[cfg(not(verus_keep_ghost))]
use chrono_harness::{file_identity, relative_path, sha256};
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::{collections::BTreeMap, path::Path};

#[cfg(not(verus_keep_ghost))]
pub fn snapshot_schema(config: &Value) -> &'static str {
    select_snapshot_schema(config["schema_version"].as_u64(), false)
}
#[cfg(not(verus_keep_ghost))]
pub fn snapshot_schema_for(r: &Registrations) -> &'static str {
    select_snapshot_schema(
        r.config()["schema_version"].as_u64(),
        r.selection().is_some(),
    )
}

#[cfg(not(verus_keep_ghost))]
pub use chrono_harness::input_file::observe_file;

/// Report the registration policy needed to collect inputs at a fixed endpoint.
#[cfg(not(verus_keep_ghost))]
pub fn input_profile(input: impl std::io::Read) -> Result<Value, String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Query {
        config: Value,
        inspect_execution_units: bool,
    }
    let query: Query = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    let credentials = if query.config["schema_version"] == 4 {
        wire::evidence::<std::collections::BTreeSet<String>>(
            json!({"operation":"credential-environment","config":query.config}),
        )?
    } else {
        Default::default()
    };
    let units = if query.inspect_execution_units {
        Some(crate::collection::execution_units(&query.config)?.is_some())
    } else {
        None
    };
    Ok(serde_json::json!({"credential_environment":credentials,"has_execution_units":units}))
}

/// Project the registration-owned part of a capture from the caller's fixed documents.
#[cfg(not(verus_keep_ghost))]
pub fn capture_plan(input: impl std::io::Read) -> Result<Value, String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Query {
        config: String,
        documents: BTreeMap<String, Value>,
        unit: Option<String>,
    }
    let query: Query = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    let endpoint = Registrations::load(&query.documents, &query.config)?;
    let scope = query.unit.map(|unit| crate::wire::Scope::Unit { unit });
    Ok(serde_json::json!({
        "effective_config": endpoint.effective_config_path(),
        "selection": endpoint.selection(),
        "schema": snapshot_schema_for(&endpoint),
        "required": required_files(&endpoint, scope.as_ref())?,
    }))
}

#[path = "verified_snapshot_header.rs"]
mod header;
#[cfg(verus_keep_ghost)]
pub use header::snapshot_format_version;
pub use header::{
    Retained, SNAPSHOT_SCHEMA, SNAPSHOT_SCHEMA_V2, SNAPSHOT_SCHEMA_V3, admit_snapshot_header,
    admit_snapshot_retained, admit_snapshot_selector_binding, select_snapshot_schema,
    snapshot_uses_selector,
};

#[path = "verified_snapshot_records.rs"]
mod records;
pub use records::*;

#[cfg(not(verus_keep_ghost))]
pub fn retained_shape(value: &Value) -> Result<Retained<'_>, String> {
    let observed = RetainedObservation {
        object: value.is_object(),
        keys: value
            .as_object()
            .into_iter()
            .flat_map(|map| map.keys().map(String::as_str))
            .collect(),
        absent: value.get("absent").map(Value::as_bool),
        bytes: value.get("bytes"),
        blob: value.get("blob").map(Value::as_str),
        digest: value["sha256"].as_str(),
        length: value["length"].as_u64(),
    };
    let (_, parsed) = parse_retained((), observed, |(), bytes: &Value| {
        (
            (),
            serde_json::from_value::<Vec<u8>>(bytes.clone()).map_err(|_| ()),
        )
    });
    parsed.map_err(|error| error.parts().concat())
}

/// Snapshot format shared by the producer and consumer; endpoint policy is separate.
#[cfg(not(verus_keep_ghost))]
pub fn snapshot_shape(value: &Value) -> Result<(), String> {
    crate::schema::object(
        value,
        &[
            "schema",
            "commit",
            "config_path",
            "config_digest",
            "environment",
            "files",
        ],
        &["effective_config_path", "selection"],
    )
    .map_err(|e| format!("E_INPUT_SNAPSHOT: {e}"))?;
    admit_snapshot_header(
        value["schema"].as_str(),
        value.get("effective_config_path").is_some(),
        value.get("selection").is_some(),
    )
    .map_err(str::to_string)?;
    if snapshot_uses_selector(value["schema"].as_str()) {
        let effective = value["effective_config_path"]
            .as_str()
            .ok_or("E_INPUT_SNAPSHOT: missing effective config path")?;
        relative_path(effective)?;
        let selection = crate::schema::object(
            &value["selection"],
            &["schema", "path", "platform", "config_path"],
            &["registry_path"],
        )
        .map_err(|e| format!("E_INPUT_SNAPSHOT: invalid selector binding: {e}"))?;
        admit_snapshot_selector_binding(
            selection["schema"].as_str(),
            selection["platform"]
                .as_str()
                .map(|text| (text, text.trim())),
            selection["path"] == value["config_path"],
            selection["config_path"] == value["effective_config_path"],
            selection["path"] == selection["config_path"],
        )
        .map_err(str::to_string)?;
        for key in ["path", "config_path"] {
            let path = selection[key]
                .as_str()
                .ok_or("E_INPUT_SNAPSHOT: selector path must be a string")?;
            relative_path(path)?;
        }
        if let Some(path) = selection.get("registry_path") {
            relative_path(
                path.as_str()
                    .ok_or("E_INPUT_SNAPSHOT: selector registry path must be a string")?,
            )?;
        }
    }
    facts::full_oid(
        value["commit"]
            .as_str()
            .ok_or("E_INPUT_SNAPSHOT: invalid commit")?,
    )?;
    relative_path(
        value["config_path"]
            .as_str()
            .ok_or("E_INPUT_SNAPSHOT: invalid config path")?,
    )?;
    if !value["config_digest"].as_str().is_some_and(wire::is_digest) {
        return Err("E_INPUT_SNAPSHOT: invalid config digest".into());
    }
    let environment = value["environment"]
        .as_object()
        .ok_or("E_INPUT_SNAPSHOT: invalid environment")?;
    if environment
        .iter()
        .any(|(name, v)| name.is_empty() || !(v.is_null() || v.is_string()))
    {
        return Err("E_INPUT_SNAPSHOT: environment values must be strings or null".into());
    }
    for (id, file) in value["files"]
        .as_object()
        .ok_or("E_INPUT_SNAPSHOT: invalid files")?
    {
        if id.is_empty() {
            return Err("E_INPUT_SNAPSHOT: empty file ID".into());
        }
        admit_snapshot_retained(value["schema"].as_str(), &retained_shape(file)?)
            .map_err(str::to_string)?;
    }
    Ok(())
}

#[cfg(not(verus_keep_ghost))]
struct RetainedInputs<'a> {
    root: &'a Path,
    session: Option<crate::runner_session::Stream>,
}

#[cfg(not(verus_keep_ghost))]
impl RetainedInputs<'_> {
    fn session(&mut self) -> Result<&mut crate::runner_session::Stream, String> {
        if self.session.is_none() {
            self.session = Some(crate::runner_session::Stream::start(
                "retained-artifacts-v1",
                Some(self.root),
            )?);
        }
        Ok(self.session.as_mut().unwrap())
    }

    fn file(&mut self, path: &str) -> Result<(String, u64), String> {
        let session = self.session()?;
        session.exchange::<()>(&json!({"operation":"retain-read","path":path}))?;
        let file: std::path::PathBuf =
            session.exchange(&json!({"operation":"checked-path","path":path}))?;
        file_identity(&file)
    }

    fn identity(
        &mut self,
        value: &Value,
        originals: Option<(&Value, Option<&crate::collection::ArtifactTransport>)>,
    ) -> Result<Option<(String, u64)>, String> {
        match retained_shape(value)? {
            Retained::Absent => Ok(None),
            Retained::Bytes(bytes) => Ok(Some((sha256(&bytes), bytes.len() as u64))),
            Retained::Blob {
                path,
                digest,
                length,
            } => {
                let observed = if let Some((artifacts, transport)) = originals {
                    #[derive(serde::Deserialize)]
                    struct Identity {
                        sha256: String,
                        length: u64,
                    }
                    let entry = artifacts
                        .get(path)
                        .ok_or_else(|| format!("missing original artifact {path}"))?;
                    let identity: Identity = self.session()?.exchange(&json!({
                        "operation":"artifact-identity", "path":path, "artifacts":{(path):entry}, "transport":transport,
                    }))?;
                    (identity.sha256, identity.length)
                } else {
                    self.file(path).map_err(|e| format!("E_INPUT_BLOB: {e}"))?
                };
                if observed.0 != digest || observed.1 != length {
                    return Err(if originals.is_some() {
                        "E_INPUT_BLOB: original input differs"
                    } else {
                        "E_INPUT_BLOB: retained content identity mismatch"
                    }
                    .into());
                }
                Ok(Some(observed))
            }
        }
    }
}
#[cfg(not(verus_keep_ghost))]
pub fn environment(config: &Value, snapshot: &Value) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let credentials = if config["schema_version"] == 4 {
        wire::evidence::<std::collections::BTreeSet<String>>(
            json!({"operation":"credential-environment","config":config}),
        )?
    } else {
        Default::default()
    };
    if let Some(values) = snapshot.as_object() {
        for key in values.keys() {
            if !config["environment"]["inherit"]
                .as_array()
                .ok_or("inherit missing")?
                .contains(&json!(key))
                && config["environment"]["values"].get(key).is_none()
            {
                return Err(format!("unregistered retained environment variable {key}"));
            }
        }
    }
    for key in config["environment"]["inherit"]
        .as_array()
        .ok_or("inherit missing")?
    {
        let key = key.as_str().ok_or("inherit key")?;
        if credentials.contains(key) && snapshot.get(key) != Some(&Value::Null) {
            return Err(
                "acquisition credentials cannot be retained as business environment inputs".into(),
            );
        }
        match snapshot
            .get(key)
            .ok_or_else(|| format!("missing retained environment variable {key}"))?
        {
            Value::Null => {}
            Value::String(value) => {
                out.insert(key.into(), value.clone());
            }
            _ => {
                return Err(
                    "environment must distinguish absent/null and string (including empty)".into(),
                );
            }
        }
    }
    for (key, value) in config["environment"]["values"]
        .as_object()
        .ok_or("environment values")?
    {
        out.insert(
            key.clone(),
            value.as_str().ok_or("environment value")?.into(),
        );
    }
    Ok(out)
}
/// Required declared files for one unit, including explicit operation owners,
/// their FILEMAP prerequisites, shared operations and governance consumers.
/// Collection starts with governance inputs; routes supplies its DELTA obligations
/// after FILEMAP impact. No IO discovery or report-derived selection.
#[cfg(not(verus_keep_ghost))]
pub fn required_files(
    r: &Registrations,
    scope: Option<&crate::wire::Scope>,
) -> Result<std::collections::BTreeSet<String>, String> {
    let selected = match scope {
        Some(crate::wire::Scope::Unit { unit }) => Some(assigned_plans(r, unit)?),
        Some(crate::wire::Scope::Collect { .. }) => Some(Default::default()),
        _ => None,
    };
    required_plan_files(r, selected.as_ref())
}

#[cfg(not(verus_keep_ghost))]
fn assigned_plans(
    r: &Registrations,
    unit: &str,
) -> Result<std::collections::BTreeSet<String>, String> {
    let plans = crate::execution::plans(r.filemap())?;
    crate::wire::evidence(json!({
        "operation":"unit-tests", "unit":unit, "units":r.config()["execution_units"]["units"],
        "selected":plans.keys().collect::<Vec<_>>(),
    }))
}

#[cfg(not(verus_keep_ghost))]
pub fn required_plan_files(
    r: &Registrations,
    selected: Option<&std::collections::BTreeSet<String>>,
) -> Result<std::collections::BTreeSet<String>, String> {
    use crate::NodeKind;
    use std::collections::BTreeSet;
    let mut all = BTreeSet::new();
    for input in r.config()["environment"]["inputs"]
        .as_array()
        .ok_or("inputs missing")?
    {
        all.insert(input["id"].as_str().ok_or("input id missing")?.to_owned());
    }
    let Some(selected) = selected else {
        return Ok(all);
    };
    if let Some(finding) = crate::input_closure_findings(r).first() {
        return Err(format!("{}: {}", finding.code, finding.message));
    }
    let plans = crate::execution::plans(r.filemap())?;
    let methods = crate::execution::methods(r.projects())?;
    let nodes = r.node_data();
    let mut needed = selected.clone();
    for test in selected {
        let definition = nodes
            .get(test)
            .and_then(|n| n.unique())
            .ok_or_else(|| format!("E_REFERENCE: missing/ambiguous input consumer {test}"))?;
        needed.insert(definition.identity.clone());
        for operation in &plans[test].operations {
            let method = methods
                .get(operation)
                .filter(|m| m.len() == 1)
                .ok_or_else(|| format!("E_REFERENCE: missing/ambiguous operation {operation}"))?;
            needed.insert(method[0].owner.clone());
            needed.insert(format!("tool:{}", method[0].tool));
        }
    }
    needed.extend(
        nodes
            .iter()
            .filter(|(_, n)| n.kind == NodeKind::Judge)
            .map(|(id, _)| id.clone()),
    );
    if let Some(id) = r.config()["facts_git"]["input"].as_str() {
        needed.insert(format!("input:{id}"));
    }
    // The whitelist carries dependencies only in their declared direction.
    // Reverse it to obtain the prerequisites of the chosen consumers.
    let mut edges = Vec::new();
    for file in r.filemap()["files"].as_array().unwrap() {
        for edge in file["edges"].as_array().unwrap() {
            edges.push((
                format!("file:{}", file["path"].as_str().unwrap()),
                edge.clone(),
            ));
        }
    }
    for edge in r.filemap()["project_edges"].as_array().unwrap() {
        edges.push((edge["from"].as_str().unwrap().to_owned(), edge.clone()));
    }
    for (from, edge) in &edges {
        for node in [from.as_str(), edge["to"].as_str().unwrap()] {
            if !nodes.contains_key(node) {
                return Err(format!("E_REFERENCE: unknown input dependency {node}"));
            }
        }
    }
    loop {
        let size = needed.len();
        for (from, edge) in &edges {
            if needed.contains(edge["to"].as_str().unwrap())
                && matches!(
                    edge["kind"].as_str(),
                    Some("compile" | "build-input" | "runtime-input" | "test-execution")
                )
            {
                needed.insert(from.clone());
            }
        }
        // Closure bindings are validated against explicit FILEMAP edges above.
        for binding in r.config()["input_closure"]["bindings"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if needed.contains(binding["consumer"].as_str().unwrap()) {
                needed.extend(
                    binding["inputs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().to_owned()),
                );
            }
        }
        if needed.len() == size {
            break;
        }
    }
    Ok(all
        .into_iter()
        .filter(|id| needed.contains(&format!("input:{id}")))
        .collect())
}

/// Project routes' selected obligations through the existing explicit unit map.
/// Retain the old assignments as well as candidate plans that existed at base.
#[cfg(not(verus_keep_ghost))]
pub fn collection_required_files(
    endpoint: &str,
    old: &Registrations,
    new: &Registrations,
    selected: &std::collections::BTreeSet<String>,
) -> Result<std::collections::BTreeSet<String>, String> {
    let units = crate::collection::execution_units(new.config())?
        .ok_or("collection execution units missing")?;
    let mut plans = std::collections::BTreeSet::new();
    for unit in crate::collection::required(&units["units"], selected)? {
        plans.extend(assigned_plans(new, &unit)?);
        if endpoint == "base"
            && old.config()["execution_units"]["units"]
                .get(&unit)
                .is_some()
        {
            plans.extend(assigned_plans(old, &unit)?);
        }
    }
    let r = if endpoint == "base" { old } else { new };
    if endpoint == "base" {
        let existing = crate::execution::plans(old.filemap())?;
        plans.retain(|p| existing.contains_key(p));
    }
    required_plan_files(r, Some(&plans))
}

#[cfg(not(verus_keep_ghost))]
pub fn endpoint_required_files(
    endpoint: &str,
    old: &Registrations,
    new: &Registrations,
    scope: Option<&crate::wire::Scope>,
) -> Result<std::collections::BTreeSet<String>, String> {
    if endpoint != "base" {
        return required_files(new, scope);
    }
    let Some(crate::wire::Scope::Unit { unit }) = scope else {
        return required_files(old, scope);
    };
    // Candidate IDs need not exist historically. Project their declared plans
    // onto the base, retaining also any plans assigned to this ID at the base.
    // Both projections use the same input closure; no global-input fallback.
    let plans = crate::execution::plans(old.filemap())?;
    let mut selected = assigned_plans(new, unit)?
        .into_iter()
        .filter(|test| plans.contains_key(test))
        .collect::<std::collections::BTreeSet<_>>();
    if old.config()["execution_units"]["units"].get(unit).is_some() {
        selected.extend(assigned_plans(old, unit)?);
    }
    required_plan_files(old, Some(&selected))
}

/// Compare original per-unit evidence with the same projection of the current
/// collection obligations, retaining exact environment and endpoint bindings.
#[cfg(not(verus_keep_ghost))]
pub fn project_effective(
    evidence: &Value,
    old: &Registrations,
    new: &Registrations,
    scope: Option<&crate::wire::Scope>,
) -> Result<Value, String> {
    if evidence.is_null() {
        return Ok(Value::Null);
    }
    if evidence["identity"] != wire::digest(&evidence["endpoints"])? {
        return Err("effective input identity mismatch".into());
    }
    let mut projected = evidence.clone();
    for endpoint in ["base", "candidate"] {
        let required = endpoint_required_files(endpoint, old, new, scope)?;
        let files = projected["endpoints"][endpoint]["files"]
            .as_object_mut()
            .ok_or("effective input files missing")?;
        for id in &required {
            if !files.contains_key(id) {
                return Err(format!("missing effective input {endpoint}: {id}"));
            }
        }
        files.retain(|id, _| required.contains(id));
    }
    projected["identity"] = json!(wire::digest(&projected["endpoints"])?);
    Ok(projected)
}

#[cfg(not(verus_keep_ghost))]
pub fn validate(req: &Request, old: &Registrations, new: &Registrations) -> Result<Value, String> {
    validate_inner(
        req,
        old,
        new,
        None,
        !matches!(req.scope, Some(crate::wire::Scope::Collect { .. })),
    )
}
/// Routes is the selection owner. It checks collection coverage before preparing
/// the zero-business collection plan, without a registration/filemap cycle.
#[cfg(not(verus_keep_ghost))]
pub fn validate_collection(
    req: &Request,
    old: &Registrations,
    new: &Registrations,
    selected: &std::collections::BTreeSet<String>,
) -> Result<Value, String> {
    for endpoint in ["base", "candidate"] {
        for id in collection_required_files(endpoint, old, new, selected)? {
            if req.observations["retained"][endpoint]["files"]
                .get(&id)
                .is_none()
            {
                return Err(format!("{endpoint}: missing retained input {id}"));
            }
        }
    }
    validate(req, old, new)
}
#[cfg(not(verus_keep_ghost))]
pub fn validate_retained_at(
    req: &Request,
    old: &Registrations,
    new: &Registrations,
    artifacts: &Value,
    root: &Path,
    transport: Option<&crate::collection::ArtifactTransport>,
) -> Result<Value, String> {
    validate_inner(req, old, new, Some((artifacts, root, transport)), false)
}
#[cfg(not(verus_keep_ghost))]
fn validate_inner(
    req: &Request,
    old: &Registrations,
    new: &Registrations,
    artifacts: Option<(&Value, &Path, Option<&crate::collection::ArtifactTransport>)>,
    live: bool,
) -> Result<Value, String> {
    let mut store = RetainedInputs {
        root: artifacts.map_or(req.candidate.root.as_path(), |(_, root, _)| root),
        session: None,
    };
    let originals = artifacts.map(|(map, _, transport)| (map, transport));
    let result = validate_endpoints(req, old, new, originals, live, &mut store);
    if let Some(mut session) = store.session {
        let closed = session.finish();
        return result.and_then(|value| closed.map(|()| value));
    }
    result
}

#[cfg(not(verus_keep_ghost))]
fn validate_endpoints(
    req: &Request,
    old: &Registrations,
    new: &Registrations,
    originals: Option<(&Value, Option<&crate::collection::ArtifactTransport>)>,
    live: bool,
    store: &mut RetainedInputs<'_>,
) -> Result<Value, String> {
    if new.filemap()["schema_version"] == 1
        && old.config()["schema_version"] == 1
        && new.config()["schema_version"] == 1
        && old.config()["environment"]["inputs"]
            .as_array()
            .unwrap()
            .is_empty()
        && new.config()["environment"]["inputs"]
            .as_array()
            .unwrap()
            .is_empty()
        && req.observations["retained"].is_null()
    {
        // Historical registration-only profiles have no retained-input success claim.
        return Ok(Value::Null);
    }
    let mut endpoints = serde_json::Map::new();
    for (name, endpoint, r) in [("base", &req.base, old), ("candidate", &req.candidate, new)] {
        let cfg = r.config();
        let required = endpoint_required_files(name, old, new, req.scope.as_ref())?;
        let retained = &req.observations["retained"][name];
        if matches!(cfg["schema_version"].as_u64(), Some(2 | 3 | 4))
            && retained["schema"] != snapshot_schema_for(r)
        {
            return Err(if r.selection().is_some() {
                format!("{name}: selected config requires snapshot v3")
            } else {
                format!("{name}: config v2 requires snapshot v2")
            });
        }
        if retained.get("schema").is_some() {
            snapshot_shape(retained)?;
            if retained["schema"] != snapshot_schema_for(r)
                || retained["config_path"] != req.config_path
                || retained["config_digest"] != wire::digest(cfg)?
            {
                return Err(format!("{name}: snapshot configuration binding mismatch"));
            }
            if snapshot_uses_selector(retained["schema"].as_str())
                && (retained["effective_config_path"] != r.effective_config_path()
                    || retained["selection"] != r.selection().cloned().unwrap_or(Value::Null))
            {
                return Err(format!("{name}: snapshot selection binding mismatch"));
            }
        }
        let files = cfg["environment"]["inputs"]
            .as_array()
            .ok_or("inputs missing")?;
        let strict_environment = new.filemap()["schema_version"] == 2;
        if !crate::verified_readiness::endpoint_snapshot_matches(
            retained.get("schema").is_some(),
            files.len(),
            strict_environment,
            cfg["environment"]["inherit"].as_array().unwrap().len(),
            retained["commit"].as_str(),
            &endpoint.commit,
        ) {
            return Err(format!(
                "{name}: retained input snapshot absent or wrong endpoint"
            ));
        }
        let inherited = if retained.is_object() {
            &retained["environment"]
        } else if name == "candidate" {
            &req.observations["environment"]["inherited"]
        } else {
            &Value::Null
        };
        let effective = environment(cfg, inherited);
        if strict_environment && effective.is_err() {
            return Err(format!("{name}: {}", effective.unwrap_err()));
        }
        if name == "candidate"
            && strict_environment
            && serde_json::to_value(effective.as_ref().unwrap()).unwrap()
                != req.observations["environment"]["effective"]
        {
            return Err("candidate environment differs from entry observation".into());
        }
        let mut evidence = BTreeMap::new();
        if let Some(retained_files) = retained["files"].as_object() {
            for id in retained_files.keys() {
                if !files.iter().any(|i| i["id"] == *id) {
                    return Err(format!("{name}: unregistered retained input {id}"));
                }
            }
        }
        for input in files {
            let id = input["id"].as_str().unwrap();
            if !required.contains(id) && retained["files"].get(id).is_none() {
                continue;
            }
            let value = retained["files"]
                .get(id)
                .ok_or_else(|| format!("{name}: missing retained input {id}"))?;
            let identity = store
                .identity(value, originals)
                .map_err(|e| format!("{name}: retained input {id}: {e}"))?;
            match &identity {
                None if matches!(cfg["schema_version"].as_u64(), Some(2 | 3 | 4))
                    && input["presence"] == "absent" => {}
                Some((digest, _))
                    if input["presence"] != "absent"
                        && input["sha256"].as_str() == Some(digest) => {}
                _ => {
                    return Err(format!(
                        "{name}: retained input digest/presence mismatch: {id}"
                    ));
                }
            }
            if name == "candidate" && live && required.contains(id) {
                let path = Path::new(input["location"].as_str().unwrap());
                let path = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    req.candidate.root.join(path)
                };
                let actual = if matches!(cfg["schema_version"].as_u64(), Some(2 | 3 | 4)) {
                    observe_file(&path)
                } else {
                    file_identity(&path).map(Some)
                };
                if actual.map_err(|e| format!("candidate input {id}: {e}"))? != identity {
                    return Err(format!("candidate input changed: {id}"));
                }
            }
            let mut fact = match identity {
                None => json!({"presence":"absent","location":input["location"]}),
                Some((digest, length)) => {
                    json!({"sha256":digest,"length":length,"location":input["location"]})
                }
            };
            if matches!(cfg["schema_version"].as_u64(), Some(2 | 3 | 4))
                && fact.get("presence").is_none()
            {
                fact["presence"] = json!("present");
            }
            if required.contains(id)
                || matches!(req.scope, Some(crate::wire::Scope::Collect { .. }))
            {
                evidence.insert(id.to_string(), fact);
            }
        }
        endpoints.insert(name.into(),json!({"commit":endpoint.commit,"environment":effective.ok(),"inherited":inherited,"files":evidence}));
    }
    let endpoints = Value::Object(endpoints);
    Ok(
        json!({"schema":effective_schema(old,new),"identity":wire::digest(&endpoints)?,"endpoints":endpoints,"completeness_proven":false}),
    )
}

#[cfg(not(verus_keep_ghost))]
fn effective_schema(old: &Registrations, new: &Registrations) -> &'static str {
    if matches!(old.config()["schema_version"].as_u64(), Some(2 | 3 | 4))
        || matches!(new.config()["schema_version"].as_u64(), Some(2 | 3 | 4))
    {
        "chrono-effective-inputs/v2"
    } else {
        "chrono-effective-inputs/v1"
    }
}

/// Consume retained facts through the same environment semantics as validation.
/// Null is supported only by the historical registration-only v1 contract.
#[cfg(not(verus_keep_ghost))]
pub fn effective_environments(
    evidence: &Value,
    old: &Registrations,
    new: &Registrations,
) -> Result<Option<[BTreeMap<String, String>; 2]>, String> {
    if evidence.is_null()
        && new.filemap()["schema_version"] == 1
        && old.config()["schema_version"] == 1
        && new.config()["schema_version"] == 1
    {
        return Ok(None);
    }
    if evidence["schema"] != effective_schema(old, new)
        || evidence["identity"] != wire::digest(&evidence["endpoints"])?
    {
        return Err("effective input identity missing/mismatched".into());
    }
    let mut pair = [BTreeMap::new(), BTreeMap::new()];
    for (index, name, r) in [(0, "base", old), (1, "candidate", new)] {
        let endpoint = &evidence["endpoints"][name];
        let effective = environment(r.config(), &endpoint["inherited"])?;
        if serde_json::to_value(&effective).unwrap() != endpoint["environment"] {
            return Err(format!(
                "{name}: effective environment differs from declared retained inputs"
            ));
        }
        pair[index] = effective;
    }
    Ok(Some(pair))
}
