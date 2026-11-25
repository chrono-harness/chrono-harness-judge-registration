//! Finite candidate-owned decoders for explicitly named historical profiles.
use crate::facts;
use crate::runner_session;
use crate::wire::json;
use crate::wire::sha256;
use crate::wire::{self, Request};
use crate::{Registrations, execution, historical_git::InstalledGit};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json as value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
type Values = BTreeMap<String, Value>;
/// Fields registration checks against the fixed migration declaration. The runner
/// retains ownership of the rest of the record, including original launch evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Process {
    pub(crate) exit_code: i32,
    pub(crate) failure: Option<String>,
    executable: std::path::PathBuf,
    sha256: String,
    cwd: std::path::PathBuf,
    argv: Vec<String>,
    environment: BTreeMap<String, String>,
    stdin_sha256: String,
    pub(crate) stdout_bytes: Vec<u8>,
    stdout_sha256: String,
    stdout: String,
    pub(crate) stderr: String,
    #[serde(flatten)]
    original: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    program: String,
    path: std::path::PathBuf,
    basename: String,
    sha256: String,
    version: Process,
}

enum Conversion<'a> {
    Original(&'a Value),
    Acquire(&'a dyn Fn(&Registrations, &Value) -> Result<Option<Value>, String>),
}
fn config_binding(base: &Values, candidate: &Values, entry: &str) -> Result<Value, String> {
    Ok(value!({"base":facts::registry_identity(base, entry)?,
        "candidate":facts::registry_identity(candidate, entry)?}))
}
pub(crate) fn load_view(
    view: &Value,
    config: &str,
) -> Result<(Registrations, Registrations), String> {
    let a: Values = serde_json::from_value(view["base"].clone()).map_err(|e| e.to_string())?;
    let b: Values = serde_json::from_value(view["candidate"].clone()).map_err(|e| e.to_string())?;
    let identities = config_binding(&a, &b, config)?;
    if identities["base"]["selection"].is_object()
        || identities["candidate"]["selection"].is_object()
    {
        if view["schema"] != "chrono-registration-view/v2" || view["config_bindings"] != identities
        {
            return Err("registration view selection binding mismatch".into());
        }
        if !view["conversion"].is_null() {
            let input = &view["conversion"]["input"];
            if input["schema"] != "chrono-historical-decode/v3"
                || input["config_path"] != config
                || input["config_bindings"] != identities
            {
                return Err("historical decoder selection binding mismatch".into());
            }
        }
    }
    let mut old = Registrations::load(&a, config)?;
    old.historical =
        serde_json::from_value(view["historical"].clone()).map_err(|e| e.to_string())?;
    if let Some(raw) = view["conversion"]["input"].get("original") {
        let raw: Values = serde_json::from_value(raw.clone()).map_err(|e| e.to_string())?;
        old.original_schemas = Registrations::schemas(&raw, config)?;
        // Supported config versions already have readers. Historical snapshots
        // bind their original config, never a decoder's replacement semantics.
        let original = facts::registry_identity(&raw, config)?;
        if facts::registry_identity(&a, config)? != original
            || a.get(config) != raw.get(config)
            || a.get(&original.effective_path) != raw.get(&original.effective_path)
        {
            return Err(
                "E_MIGRATION_INPUT_SEMANTICS: historical config must remain original".into(),
            );
        }
    }
    Ok((old, Registrations::load(&b, config)?))
}
pub fn views(req: &Request) -> Result<(Registrations, Registrations, Value), String> {
    InstalledGit::with_request(req, |reader| installed_views(req, reader, None)).0
}
fn view_binding(
    req: &Request,
    base: &facts::RegistrySnapshot,
    candidate: &facts::RegistrySnapshot,
) -> Value {
    let mut binding = value!({"base":req.base.commit,"candidate":req.candidate.commit,"registry":req.registries.digest,"context":req.context.sha256});
    if base.selection.is_some() || candidate.selection.is_some() {
        binding["entry_path"] = value!(req.config_path);
        binding["base_config"] = value!({"entry_path":base.entry_path,"effective_path":base.effective_path,"selection":base.selection});
        binding["candidate_config"] = value!({"entry_path":candidate.entry_path,"effective_path":candidate.effective_path,"selection":candidate.selection});
    }
    binding
}
/// Rebuild the original registration view from current immutable endpoint
/// data and retained conversion observations. This never invokes the decoder.
pub(crate) fn validate_retained_snapshots(
    req: &Request,
    reader: &InstalledGit<'_>,
    declaration_root: &Path,
    supplied: &Value,
    base: facts::RegistrySnapshot,
    candidate: facts::RegistrySnapshot,
) -> Result<(), String> {
    if facts::registry_digest(&base.values, &candidate.values)? != req.registries.digest {
        return Err("E_COLLECTION_INPUT: original registration endpoint digest".into());
    }
    let binding = view_binding(req, &base, &candidate);
    let conversion = &supplied["conversion"];
    if !conversion.is_null() && conversion["process"]["cwd"] != value!(req.candidate.root) {
        return Err("E_COLLECTION_INPUT: original conversion coordinates differ".into());
    }
    let env = serde_json::from_value(req.observations["environment"]["effective"].clone())
        .map_err(|e| e.to_string())?;
    let (_, _, mut expected) = interpret_mode(
        || Ok(reader),
        declaration_root,
        &req.base.commit,
        &req.candidate.commit,
        &req.config_path,
        base.values,
        candidate.values,
        &env,
        Some("registration"),
        Some(Conversion::Original(conversion)),
        true,
    )?;
    expected["binding"] = binding;
    if *supplied != expected {
        return Err("E_COLLECTION_INPUT: original registration view differs".into());
    }
    Ok(())
}
pub(crate) fn installed_views(
    request: &Request,
    reader: &InstalledGit<'_>,
    reports: Option<&dyn Fn(&[String]) -> Result<crate::collection::FullManifest, String>>,
) -> Result<(Registrations, Registrations, Value), String> {
    views_from_snapshots(
        request,
        reader.snapshot("base")?,
        reader.snapshot("candidate")?,
        reports,
        || Ok(reader),
        true,
    )
}
/// Resolve the same sealed request as an in-process registration consumer.
pub fn filemap_views(req: &Request) -> Value {
    endpoint_reply(req, |endpoint| endpoint.filemap_view())
}

/// Interpreted endpoint inputs for the mixed judge's native protocol.
pub fn classification_views(req: &Request) -> Value {
    if let Err(error) = req.validate() {
        return value!({"request_id":req.request_id,"error":error});
    }
    let changed = req
        .delta
        .iter()
        .map(|change| change.path.as_str())
        .collect();
    endpoint_reply(req, |endpoint| endpoint.classification_view(&changed))
}

fn endpoint_reply(req: &Request, project: impl Fn(&Registrations) -> Value) -> Value {
    let (result, observation) = InstalledGit::with_request(req, |reader| {
        let (old, current, _) = installed_views(req, reader, None)?;
        Ok(
            value!({ "request_id":req.request_id, "base":project(&old), "candidate":project(&current) }),
        )
    });
    let mut reply =
        result.unwrap_or_else(|error| value!({"request_id":req.request_id,"error":error}));
    if let Some(observation) = observation {
        reply["git_facts"] = observation;
    }
    reply
}

pub(crate) fn views_from_snapshots<'a>(
    req: &Request,
    base_snapshot: facts::RegistrySnapshot,
    candidate_snapshot: facts::RegistrySnapshot,
    reports: Option<&dyn Fn(&[String]) -> Result<crate::collection::FullManifest, String>>,
    reader: impl Fn() -> Result<&'a InstalledGit<'a>, String>,
    check_binding: bool,
) -> Result<(Registrations, Registrations, Value), String> {
    let selector_required =
        base_snapshot.selection.is_some() || candidate_snapshot.selection.is_some();
    let binding = view_binding(req, &base_snapshot, &candidate_snapshot);
    let prior: Vec<_> = req
        .prior_results
        .iter()
        .filter_map(|r| r.outputs.get("registration_view"))
        .collect();
    if let Some(v) = prior.first() {
        if prior.iter().any(|p| *p != *v) || v["binding"] != binding {
            return Err("registration view binding mismatch".into());
        }
        if selector_required {
            let expected = config_binding(
                &base_snapshot.values,
                &candidate_snapshot.values,
                &req.config_path,
            )?;
            if v["schema"] != "chrono-registration-view/v2"
                || v["config_bindings"] != expected
                || v["candidate"] != value!(candidate_snapshot.values)
            {
                return Err("registration view selection binding mismatch".into());
            }
            let original = if v["conversion"].is_null() {
                &v["base"]
            } else {
                &v["conversion"]["input"]["original"]
            };
            if *original != value!(base_snapshot.values)
                || (!v["conversion"].is_null()
                    && v["conversion"]["input"]["original_bytes"] != value!(base_snapshot.bytes))
            {
                return Err("registration view original endpoint mismatch".into());
            }
        }
        let (a, b) = load_view(v, &req.config_path)?;
        return Ok((a, b, (*v).clone()));
    }
    let a = base_snapshot.values;
    let b = candidate_snapshot.values;
    if facts::registry_digest(&a, &b)? != req.registries.digest {
        return Err("registry digest mismatch".into());
    }
    let env = serde_json::from_value(req.observations["environment"]["effective"].clone())
        .unwrap_or_default();
    let acquire = |r: &Registrations, profile: &Value| {
        let originals = reports
            .map(|load| {
                let block = crate::collection::execution_units(r.config())?
                    .ok_or("full collection units missing")?;
                let test = format!("test:{}", profile["test"].as_str().ok_or("migration test")?);
                let required =
                    crate::collection::required(&block["units"], &[test].into_iter().collect())?;
                if required.is_empty() {
                    return Err("E_COLLECTION_INPUT: migration test has no registered unit".into());
                }
                load(&required)
            })
            .transpose()?;
        retained_conversion(req, r, originals.as_ref())
    };
    // A full CI collection prepared the sealed request with its own bound reader.
    // Other consumers verify their reader here, even for document-only transitions.
    if check_binding {
        reader()?;
    }
    let (a, b, mut view) = interpret_mode(
        reader,
        &req.candidate.root,
        &req.base.commit,
        &req.candidate.commit,
        &req.config_path,
        a,
        b,
        &env,
        Some(&req.judge_id),
        Some(Conversion::Acquire(&acquire)),
        true,
    )?;
    view["binding"] = binding;
    Ok((a, b, view))
}
pub fn interpret(
    root: &Path,
    base: &str,
    candidate_oid: &str,
    config: &str,
    raw: Values,
    candidate: Values,
    env: &BTreeMap<String, String>,
) -> Result<(Registrations, Registrations, Value), String> {
    let reader = InstalledGit::configured(root, base, candidate_oid, config)?;
    reader.query::<()>(value!({"op":"verify-config","candidate":candidate_oid}))?;
    let result = interpret_ci_documents(
        || Ok(&reader),
        root,
        base,
        candidate_oid,
        config,
        raw,
        candidate,
        Some(env),
    );
    let closed = reader.finish();
    result.and_then(|value| closed.map(|_| value))
}
fn retained_conversion(
    req: &Request,
    r: &Registrations,
    reports: Option<&crate::collection::FullManifest>,
) -> Result<Option<Value>, String> {
    let Some(crate::wire::Scope::Collect { manifest }) = &req.scope else {
        return Ok(None);
    };
    crate::runner_session::Stream::with(
        "retained-artifacts-v1",
        Some(&req.candidate.root),
        |originals| read_retained_conversion(req, r, reports, manifest, originals),
    )
}

fn read_retained_conversion(
    req: &Request,
    r: &Registrations,
    reports: Option<&crate::collection::FullManifest>,
    manifest_path: &str,
    originals: &mut crate::runner_session::Stream,
) -> Result<Option<Value>, String> {
    let limits = &r.config()["execution_units"]["collection_limits"];
    let manifest = match reports {
        Some(reports) => reports.clone(),
        None => {
            let text: String = originals.exchange(&value!({
                "operation":"read-bounded-text", "path":manifest_path,
                "limit":limits["manifest_bytes"].as_u64().ok_or("manifest bound")?
            }))?;
            crate::wire::decode(text.as_bytes())?
        }
    };
    if !matches!(
        manifest.schema.as_str(),
        "chrono-full-collection/v1" | "chrono-ci-collection/v1"
    ) {
        return Err("E_COLLECTION_INPUT: manifest schema".into());
    }
    let mut seen = BTreeSet::new();
    let mut retained = None;
    for input in manifest.reports {
        let source = r.config()["execution_units"]["units"][&input.unit]["report_path"]
            .as_str()
            .ok_or("E_COLLECTION_INPUT: unregistered original unit")?;
        let transported: String = originals.exchange(&value!({
            "operation":"resolve-original", "original":{"path":source, "sha256":input.sha256},
            "transport":input.artifacts,
        }))?;
        if !seen.insert(input.unit.clone()) || transported != input.path {
            return Err("E_COLLECTION_INPUT: original unit registration".into());
        }
        let text: String = originals.exchange(&value!({
            "operation":"read-bounded-text", "path":input.path,
            "limit":limits["report_bytes"].as_u64().ok_or("report bound")?
        }))?;
        if sha256(text.as_bytes()) != input.sha256 {
            return Err("E_COLLECTION_INPUT: original report digest".into());
        }
        let report = json(text.as_bytes())?;
        let record = report["judges"]
            .as_array()
            .ok_or("original judges")?
            .iter()
            .find(|j| j["id"] == "registration")
            .ok_or("original registration")?;
        let c = &record["response"]["outputs"]["registration_view"]["conversion"];
        if !c.is_null() {
            let original: Request =
                serde_json::from_value(report["request"].clone()).map_err(|e| e.to_string())?;
            original.validate()?;
            #[derive(serde::Deserialize)]
            struct Observed {
                record: ObservedRecord,
                stdout_matches: bool,
            }
            #[derive(serde::Deserialize)]
            struct ObservedRecord {
                process: ProcessBinding,
            }
            #[derive(serde::Deserialize)]
            struct ProcessBinding {
                stdin_sha256: String,
                sha256: String,
            }
            let observed: Observed =
                crate::runner_session::Stream::with("judge-evidence-v1", None, |stream| {
                    stream.exchange(&value!({"operation":"observe-record","record":record}))
                })?;
            let process = observed.record.process;
            if original.base.commit != req.base.commit
                || original.base.tree != req.base.tree
                || original.candidate.commit != req.candidate.commit
                || original.candidate.tree != req.candidate.tree
                || original.registries.digest != req.registries.digest
                || original.context.sha256 != req.context.sha256
                || original.runner.sha256 != req.runner.sha256
                || original.scope
                    != Some(crate::wire::Scope::Unit {
                        unit: input.unit.clone(),
                    })
                || original.observations["registry_bindings"]
                    != req.observations["registry_bindings"]
                || process.stdin_sha256 != wire::digest(&original)?
                || process.sha256
                    != r.judges()["judges"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|j| j["id"] == "registration")
                        .ok_or("registration binding")?["sha256"]
                        .as_str()
                        .ok_or("registration pin")?
                || !observed.stdout_matches
                || c["process"]["cwd"] != serde_json::to_value(&original.candidate.root).unwrap()
            {
                return Err("E_COLLECTION_INPUT: original conversion bootstrap binding".into());
            }
            // Different roots carry different process coordinates. The conversion's declared input/output must agree.
            if let Some(previous) = &retained {
                let previous: &Value = previous;
                if previous["input"] != c["input"]
                    || previous["output"] != c["output"]
                    || previous["script_sha256"] != c["script_sha256"]
                {
                    return Err("E_COLLECTION_INPUT: conflicting original conversion".into());
                }
            }
            if retained.is_none() {
                retained = Some(c.clone());
            }
        }
    }
    Ok(Some(retained.unwrap_or(Value::Null)))
}
pub(crate) fn interpret_ci_documents<'a>(
    reader: impl Fn() -> Result<&'a InstalledGit<'a>, String>,
    root: &Path,
    base: &str,
    candidate_oid: &str,
    config: &str,
    raw: Values,
    candidate: Values,
    environment: Option<&BTreeMap<String, String>>,
) -> Result<(Registrations, Registrations, Value), String> {
    interpret_mode(
        reader,
        root,
        base,
        candidate_oid,
        config,
        raw,
        candidate,
        environment.unwrap_or(&BTreeMap::new()),
        None,
        None,
        environment.is_some(),
    )
}

fn interpret_mode<'a>(
    get_reader: impl Fn() -> Result<&'a InstalledGit<'a>, String>,
    root: &Path,
    base: &str,
    candidate_oid: &str,
    config: &str,
    raw: Values,
    candidate: Values,
    env: &BTreeMap<String, String>,
    consumer: Option<&str>,
    retained: Option<Conversion<'_>>,
    allow_conversion: bool,
) -> Result<(Registrations, Registrations, Value), String> {
    let new = Registrations::load(&candidate, config).map_err(|e| format!("candidate: {e}"))?;
    let deferred = consumer.is_some_and(|id| downstream_validator(&new, id));
    let raw_config = facts::registry_identity(&raw, config)?.effective_path;
    let filemap = raw[&raw_config]["registries"]["filemap"]
        .as_str()
        .ok_or("historical filemap path")?;
    let mut view = value!({"base":raw,"candidate":candidate,"historical":{},"conversion":null});
    let bindings = config_binding(&raw, &candidate, config)?;
    let selected =
        bindings["base"]["selection"].is_object() || bindings["candidate"]["selection"].is_object();
    if selected {
        view["schema"] = value!("chrono-registration-view/v2");
        view["config_bindings"] = bindings.clone();
    }
    let versions = |values: &Values| -> Result<Value, String> {
        Ok(value!(
            Registrations::schemas(values, config)?
                .into_iter()
                .map(|(key, (_, version))| (key, version))
                .collect::<BTreeMap<_, _>>()
        ))
    };
    let from = versions(&raw)?;
    let to = versions(&candidate)?;
    let mut matching = vec![];
    let mut legacy_required = false;
    for profile in new.workflow()["historical_profiles"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if profile.get("from_versions").is_some() {
            if profile["from_versions"] == from && profile["to_versions"] == to {
                matching.push((profile, Value::Null, Value::Null, Value::Null));
            }
        } else if profile["filemap_version"] == raw[filemap]["schema_version"] {
            legacy_required = true;
            let path = profile["profile_path"].as_str().unwrap();
            let original = get_reader()?.blob(root, base, path);
            if let Err(e) = &original {
                if e.starts_with("E_GIT_FACTS:") {
                    return Err(e.clone());
                }
            }
            if let Ok(bytes) = original {
                let value = json(&bytes)?;
                if value["schema"] == profile["id"] {
                    matching.push((profile, value!(path), value!(bytes), value));
                }
            }
        }
    }
    if matching.is_empty() && !legacy_required {
        let old = Registrations::load(&raw, config).map_err(|e| format!("base: {e}"))?;
        return Ok((old, new, view));
    }
    if matching.len() != 1 {
        return Err("E_MIGRATION_PROFILE: missing/ambiguous historical profile".into());
    }
    if !allow_conversion {
        return Err("E_SCHEDULING_CONVERSION: historical registries require a decoder; scheduling cannot acquire its interpreter or execute conversion".into());
    }
    let (profile, path, profile_bytes, profile_value) = matching.remove(0);
    let acquired;
    let retained = match retained {
        Some(Conversion::Original(value)) => Some(value),
        Some(Conversion::Acquire(load)) => {
            acquired = load(&new, profile)?;
            acquired.as_ref()
        }
        None => None,
    };
    if retained.is_some_and(Value::is_null) {
        return Err("E_MIGRATION: collection lacks original conversion evidence".into());
    }
    let scripts = new.projects()["scripts"].as_array().unwrap();
    let script = scripts
        .iter()
        .find(|s| s["id"] == profile["script"])
        .ok_or("missing registered migration script")?;
    let test = scripts
        .iter()
        .find(|s| s["id"] == profile["test"])
        .ok_or("missing dedicated migration test")?;
    if script["test_script"] != test["id"] || test["tests_for"] != script["id"] {
        return Err("E_TEST_PAIR: migration script".into());
    }
    let op = script["actions"]["execute"]["operation"]
        .as_str()
        .ok_or("migration execute action")?;
    let all = execution::methods(new.projects())?;
    let methods = all.get(op).ok_or("migration method missing")?;
    if methods.len() != 1 {
        return Err("E_ROUTE_AMBIGUOUS: migration".into());
    }
    let method = &methods[0];
    let t = new.config()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == method.tool)
        .ok_or("migration tool missing")?;
    let argv: Vec<String> =
        serde_json::from_value(t["version_argv"].clone()).map_err(|e| e.to_string())?;
    let reader = get_reader()?;
    let raw_bytes: BTreeMap<_, _> = raw
        .keys()
        .map(|path| Ok((path.clone(), reader.blob(root, base, path)?)))
        .collect::<Result<_, String>>()?;
    let mut input = if profile.get("from_versions").is_some() {
        value!({"schema":"chrono-historical-decode/v2","config_path":config,"original":raw,"original_bytes":raw_bytes,"candidate":candidate,"profile":profile})
    } else {
        value!({"schema":"chrono-historical-decode/v1","config_path":config,"original":raw,"original_bytes":raw_bytes,"profile":profile,"profile_bytes":profile_bytes,"profile_value":profile_value})
    };
    if selected {
        input["schema"] = value!("chrono-historical-decode/v3");
        input["config_bindings"] = bindings;
        input["candidate"] = value!(candidate);
    }
    let script_path = script["path"].as_str().unwrap();
    let script_bytes = reader.blob(root, candidate_oid, script_path)?;
    let (tool, receipt) = if let Some(c) = retained {
        let tool: Tool = serde_json::from_value(c["tool"].clone()).map_err(|e| e.to_string())?;
        let receipt: Process =
            serde_json::from_value(c["process"].clone()).map_err(|e| e.to_string())?;
        for process in [&tool.version, &receipt] {
            wire::evidence::<()>(value!({"operation":"check-process","process":process}))?;
        }
        let mut version_argv = vec![tool.path.to_str().ok_or("migration tool path")?.to_owned()];
        version_argv.extend(argv.clone());
        let mut invocation = vec![tool.path.to_str().ok_or("migration tool path")?.to_owned()];
        invocation.extend(method.argv.clone());
        let declared_input = new.config()["environment"]["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| {
                Path::new(i["location"].as_str().unwrap()).is_absolute()
                    && i["location"].as_str() == tool.path.to_str()
                    && i["sha256"] == tool.sha256
            });
        let candidate_file = tool
            .path
            .strip_prefix(&receipt.cwd)
            .ok()
            .and_then(|p| p.to_str())
            .is_some_and(|p| {
                new.filemap()["files"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f["path"] == p)
                    && reader
                        .blob(root, candidate_oid, p)
                        .is_ok_and(|b| sha256(&b) == tool.sha256)
            });
        if c["input"] != input
            || c["input_digest"] != wire::digest(&input)?
            || c["output_digest"] != receipt.stdout_sha256
            || c["script"] != script_path
            || c["script_sha256"] != sha256(&script_bytes)
            || c["tool_id"] != method.tool
            || tool.program != t["program"].as_str().ok_or("migration program")?
            || tool.path.file_name().and_then(|s| s.to_str()) != Some(&tool.basename)
            || tool.version.executable != tool.path
            || tool.version.sha256 != tool.sha256
            || tool.version.argv != version_argv
            || tool.version.cwd != receipt.cwd
            || tool.version.environment != *env
            || tool.version.stdin_sha256 != sha256(&[])
            || t["expected_version"].as_str() != Some(tool.version.stdout.trim_end())
            || receipt.executable != tool.path
            || receipt.sha256 != tool.sha256
            || receipt.argv != invocation
            || receipt.environment != *env
            || receipt.stdin_sha256 != wire::digest(&input)?
            || c["output"] != json(&receipt.stdout_bytes)?
            || (!declared_input && !candidate_file)
        {
            return Err(
                "E_MIGRATION: retained conversion differs from declared original contract".into(),
            );
        }
        (tool, receipt)
    } else {
        let artifacts = facts::artifact_directories(new.config())?;
        let before = reader.checkout_excluding(root, candidate_oid, &artifacts)?;
        let tool: Tool = runner_session::query(
            "chrono-harness",
            "observed-tool-v1",
            Some(root),
            &value!({"root":root,"program":t["program"],"version_argv":argv,"environment":env,
                "timeout_seconds":30,"output_limit_bytes":1048576}),
        )?;
        if tool.version.exit_code != 0
            || tool.version.failure.is_some()
            || t["expected_version"].as_str() != Some(tool.version.stdout.trim_end())
        {
            return Err(format!(
                "E_TOOL_BINDING: migration version mismatch; observation={}",
                value!({"tool_id":method.tool,"expected_version":t["expected_version"],"tool":tool})
            ));
        }
        if fs::read(root.join(script_path)).map_err(|e| e.to_string())? != script_bytes {
            return Err("E_MIGRATION: decoder differs from fixed candidate".into());
        }
        let receipt: Process = runner_session::record(
            root,
            tool.path.to_str().ok_or("migration tool path")?,
            &method.argv,
            env,
            30,
            16 * 1024 * 1024,
            &tool.sha256,
            &wire::canonical(&input)?,
        )?;
        if receipt.exit_code != 0 || receipt.failure.is_some() {
            return Err(format!(
                "E_MIGRATION: exit {}: {}",
                receipt.exit_code, receipt.stderr
            ));
        }
        if before != reader.checkout_excluding(root, candidate_oid, &artifacts)? {
            return Err("E_SNAPSHOT_DIRTY: migration changed candidate inputs".into());
        }
        (tool, receipt)
    };
    let output = json(&receipt.stdout_bytes)?;
    if output["mappings"] != profile["mappings"] {
        return Err("E_MIGRATION: mapping mismatch".into());
    }
    view["base"] = output["values"].clone();
    view["historical"] = output["historical"].clone();
    view["conversion"] = if let Some(c) = retained {
        c.clone()
    } else {
        value!({"input":input,"input_digest":receipt.stdin_sha256,"output":output,"output_digest":receipt.stdout_sha256,"profile_path":path,"script":script_path,"script_sha256":sha256(&script_bytes),"tool_id":method.tool,"tool":tool,"process":receipt,"certification":"unresolved: workflow producer required"})
    };
    let (old, _) = load_view(&view, config)?;
    // Every explicitly removed malformed definition must survive as its original bytes/value.
    for record in profile["legacy_records"].as_array().unwrap() {
        let raw_config = facts::registry_identity(&raw, config)?.effective_path;
        let original = raw[raw[&raw_config]["registries"]["projects"].as_str().unwrap()]
            [record["collection"].as_str().unwrap()]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == record["id"]);
        if original != Some(&record["definition"]) {
            return Err("E_MIGRATION: historical definition mismatch".into());
        }
        for node in record["nodes"].as_array().unwrap() {
            if !old
                .historical
                .get(node.as_str().unwrap())
                .is_some_and(|defs| defs.iter().any(|(_, v)| Some(v) == original))
            {
                return Err("E_MIGRATION: lost historical definition".into());
            }
        }
    }
    let nodes = new.node_data();
    let mappings = profile["mappings"].as_array().unwrap();
    // A replacement is an exact, same-operation declaration, not an inference
    // from a renamed path or a waiver of the original execution obligation.
    let mut replacements = BTreeSet::new();
    for replacement in profile["method_replacements"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let from: execution::Method = serde_json::from_value(replacement["from"].clone())
            .map_err(|e| format!("E_MIGRATION: {e}"))?;
        let to: execution::Method = serde_json::from_value(replacement["to"].clone())
            .map_err(|e| format!("E_MIGRATION: {e}"))?;
        let originals: Vec<_> = profile["legacy_records"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|record| {
                record["definition"]["actions"]
                    .as_object()
                    .into_iter()
                    .flat_map(|actions| actions.values())
                    .map(move |action| (record, action))
            })
            .filter(|(_, action)| action["operation"] == from.operation)
            .collect();
        let original_matches = originals.len() == 1 && {
            let (record, action) = originals[0];
            from.owner == format!("script:{}", record["id"].as_str().unwrap())
                && action["tool"] == from.tool
                && action["argv"] == value!(from.argv)
        };
        let target = all.get(&to.operation);
        let owner_mapped = from.owner == to.owner
            || mappings
                .iter()
                .any(|m| m["from"] == from.owner && m["to"] == to.owner);
        if !original_matches || !owner_mapped || target != Some(&vec![to]) {
            return Err("E_MIGRATION: method replacement must bind original and unique candidate methods with an explicit owner mapping".into());
        }
        replacements.insert(from.operation);
    }
    for record in profile["legacy_records"].as_array().unwrap() {
        for node in record["nodes"].as_array().unwrap() {
            let matches: Vec<_> = mappings.iter().filter(|m| m["from"] == *node).collect();
            if matches.len() != 1
                || !nodes
                    .get(matches[0]["to"].as_str().unwrap())
                    .is_some_and(|n| n.unique().is_some())
            {
                return Err(
                    "E_MIGRATION: missing/ambiguous mapping to unique candidate identity".into(),
                );
            }
        }
        let owner = format!("owner:{}", record["id"].as_str().unwrap());
        if !new.projects()["owners"]
            .as_array()
            .unwrap()
            .contains(&record["id"])
        {
            let matches: Vec<_> = mappings.iter().filter(|m| m["from"] == owner).collect();
            if matches.len() != 1
                || !matches[0]["to"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("owner:"))
                    .is_some_and(|id| {
                        new.projects()["owners"]
                            .as_array()
                            .unwrap()
                            .contains(&value!(id))
                    })
            {
                return Err("E_MIGRATION: missing retired owner mapping".into());
            }
        }
        for action in record["definition"]["actions"]
            .as_object()
            .unwrap()
            .values()
        {
            if deferred || replacements.contains(action["operation"].as_str().unwrap()) {
                continue;
            }
            let current = all
                .get(action["operation"].as_str().unwrap())
                .ok_or("E_MIGRATION: historical method lost")?;
            if current.len() != 1
                || current[0].tool != action["tool"]
                || value!(current[0].argv) != action["argv"]
            {
                return Err(
                    "E_MIGRATION: historical method changed without supported mapping".into(),
                );
            }
        }
    }
    Ok((old, new, view))
}
pub fn reused_tools(view: &Value) -> Result<BTreeMap<String, Tool>, String> {
    let mut out = BTreeMap::new();
    let c = &view["conversion"];
    if let Some(id) = c["tool_id"].as_str() {
        out.insert(
            id.into(),
            serde_json::from_value(c["tool"].clone()).map_err(|e| e.to_string())?,
        );
    }
    Ok(out)
}
pub fn replacements(r: &Registrations) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for row in r.workflow()["retirements"].as_array().unwrap() {
        if row["kind"] != "test" {
            continue;
        }
        if let Some(to) = row["replacement"].as_str() {
            let from = format!("test:{}", row["id"].as_str().unwrap());
            let to = if to.starts_with("test:") {
                to.to_string()
            } else {
                format!("test:{to}")
            };
            if out.insert(from, to).is_some() {
                return Err("E_REQUIRED_TEST_REMOVED: ambiguous replacement".into());
            }
        }
    }
    Ok(out)
}
pub fn ambiguity_repaired(r: &Registrations, node: &str, definitions: &[String]) -> bool {
    let repairs: Vec<_> = r.workflow()["historical_profiles"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|p| p["ambiguities"].as_array().into_iter().flatten())
        .filter(|p| p["node"] == node)
        .collect();
    if repairs.len() != 1 {
        return false;
    }
    let repair = repairs[0];
    let declared: BTreeSet<_> = repair["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let actual: BTreeSet<_> = definitions.iter().map(String::as_str).collect();
    let Some(to) = repair["replacement"].as_str() else {
        return false;
    };
    declared == actual
        && r.node_data().get(to).is_some_and(|n| n.unique().is_some())
        && (to == node
            || replacements(r)
                .ok()
                .is_some_and(|m| m.get(node).is_some_and(|v| v == to)))
}

/// Explicit requests only: null is a requested retirement, never its approval.
pub fn retirement_requests(r: &Registrations) -> Result<BTreeMap<String, Option<String>>, String> {
    let mut out = BTreeMap::new();
    for row in r.workflow()["retirements"].as_array().unwrap() {
        if row["kind"] != "test" {
            continue;
        }
        let from = format!("test:{}", row["id"].as_str().unwrap());
        let to = row["replacement"].as_str().map(|s| {
            if s.starts_with("test:") {
                s.into()
            } else {
                format!("test:{s}")
            }
        });
        if out.insert(from, to).is_some() {
            return Err("E_REQUIRED_TEST_REMOVED: ambiguous retirement request".into());
        }
    }
    Ok(out)
}
pub fn retirement_declaration<'a>(
    r: &'a Registrations,
    kind: &str,
    id: &str,
) -> Result<&'a Value, String> {
    let rows: Vec<_> = r.workflow()["retirements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["kind"] == kind && row["id"] == id)
        .collect();
    if rows.len() != 1 {
        return Err(format!("E_RETIREMENT: missing/ambiguous {kind}:{id}"));
    }
    Ok(rows[0])
}

/// Validate the registration structure of a removed test, never its execution.
/// Callers must retain its old costs and require a replacement's actual success.
pub fn validate_test_retirement(
    a: &Registrations,
    b: &Registrations,
    node: &str,
) -> Result<Option<String>, String> {
    let old = a.node_data();
    let new = b.node_data();
    let defs = old
        .get(node)
        .filter(|_| node.starts_with("test:") && !new.contains_key(node))
        .ok_or_else(|| format!("E_RETIREMENT: test is not a removed baseline identity: {node}"))?;
    let requests = retirement_requests(b)?;
    let request = requests
        .get(node)
        .ok_or_else(|| format!("E_REQUIRED_TEST_REMOVED: {node}"))?;
    if let Some(to) = request {
        if !new.get(to).is_some_and(|n| n.unique().is_some()) {
            return Err(format!(
                "E_REQUIRED_TEST_REMOVED: replacement missing/ambiguous: {node} -> {to}"
            ));
        }
    } else {
        for definition in &defs.definitions {
            let d = &definition.value;
            // Group aliases are opaque; the retained definition identifies the owner.
            let (collection, kind, pair, defining_id) =
                if let Some(id) = definition.identity.strip_prefix("project:") {
                    ("projects", "project", "test_project", id)
                } else if let Some(id) = definition.identity.strip_prefix("script:") {
                    ("scripts", "script", "test_script", id)
                } else {
                    return Err(format!(
                        "E_RETIREMENT: retired test has invalid defining owner: {node}"
                    ));
                };
            let owner = d["tests_for"]
                .as_str()
                .ok_or("E_RETIREMENT: retired test has no producer")?;
            if d["id"].as_str() != Some(defining_id) {
                return Err(format!(
                    "E_RETIREMENT: defining owner identity/value mismatch: {node}"
                ));
            }
            let originals: Vec<_> = a.projects()[collection]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p["id"] == owner && p[pair] == defining_id)
                .collect();
            if originals.len() != 1
                || b.projects()[collection]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["id"] == owner || p["id"] == defining_id)
                || !retirement_declaration(b, kind, owner)?["replacement"].is_null()
            {
                return Err(format!(
                    "E_RETIREMENT: joint producer/test removal required: {node}"
                ));
            }
        }
    }
    Ok(request.clone())
}
/// A declared later validator must consume this judge before a pending obligation
/// can leave planning/execution. The validator, not this graph query, approves it.
pub fn downstream_validator(r: &Registrations, id: &str) -> bool {
    let judges = r.judges()["judges"].as_array().unwrap();
    let Some(validator) = r.judges()["migration_validator"].as_str() else {
        return false;
    };
    let mut indices = BTreeMap::new();
    for (index, judge) in judges.iter().enumerate() {
        if let Some(name) = judge["id"].as_str() {
            indices.entry(name).or_insert(index);
        }
    }
    let target = *indices.entry(id).or_insert(judges.len());
    let start = indices.get(validator).copied();
    let unknown = judges.len() + 1;
    let graph: Vec<Vec<_>> = judges
        .iter()
        .map(|judge| {
            judge["after"]
                .as_array()
                .unwrap()
                .iter()
                .map(|parent| {
                    parent
                        .as_str()
                        .map(|name| *indices.get(name).unwrap_or(&unknown))
                })
                .collect()
        })
        .collect();
    // Every registered row is expanded at most once. Only the initial entry
    // and the dependencies in those rows can ever be popped.
    let steps = 1 + graph.iter().map(Vec::len).sum::<usize>();
    crate::verified_readiness::validator_reaches(&graph, start, Some(target), steps)
}
