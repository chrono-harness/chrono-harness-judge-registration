use chrono_judge_registration::wire;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};
fn field<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("missing fixture field {key}"))
}

fn reply(out: &mut impl Write, value: Result<Value, String>) -> std::io::Result<()> {
    serde_json::to_writer(&mut *out, &value)?;
    out.write_all(b"\n")?;
    out.flush()
}

fn checked(root: &Path, path: &str) -> Result<PathBuf, String> {
    let mut current = root.to_path_buf();
    for component in Path::new(path).components() {
        let std::path::Component::Normal(piece) = component else {
            return Err("invalid fixture path".into());
        };
        current.push(piece);
        if fs::symlink_metadata(&current).is_ok_and(|meta| meta.is_symlink()) {
            return Err("symlink in fixture path".into());
        }
    }
    Ok(current)
}

fn facts_session() -> Result<(), Box<dyn std::error::Error>> {
    let mut lines = std::io::stdin().lock().lines();
    let open: Value = serde_json::from_str(&lines.next().ok_or("missing facts header")??)?;
    let root = if open["mode"] == "request" {
        PathBuf::from(field(&open["request"]["candidate"], "root")?)
    } else {
        std::env::current_dir()?
    };
    let facts: Value = serde_json::from_slice(&fs::read(
        root.join(".chrono-harness/state/fixture-facts.json"),
    )?)?;
    let bound = !facts["observation"].is_null();
    let mut out = std::io::stdout().lock();
    reply(&mut out, Ok(json!({"bound":bound})))?;
    for line in lines {
        let q: Value = serde_json::from_str(&line?)?;
        let result = (|| {
            let endpoint = || match field(&q, "endpoint")? {
                "base" => Ok("base"),
                "candidate" => Ok("candidate"),
                _ => Err("invalid fixture endpoint".to_owned()),
            };
            match field(&q, "op")? {
                "checkout-root" => Ok(json!(root)),
                "candidate-inventory" => {
                    Ok(json!({"entries":facts["entries"],"delta":facts["delta"]}))
                }
                "observation" => Ok(facts["observation"].clone()),
                "verify-tree" => Ok(facts[endpoint()?]["tree"].clone()),
                "verify-commit" => {
                    let oid = field(&q, "commit")?;
                    let ep = ["base", "candidate"]
                        .into_iter()
                        .find(|k| facts[*k]["commit"] == oid)
                        .ok_or("unknown fixture commit")?;
                    Ok(facts[ep]["tree"].clone())
                }
                "verify-config" => Ok(Value::Null),
                "parents" => {
                    let data = git(&root, &["cat-file", "commit", field(&q, "commit")?])?;
                    Ok(json!(
                        std::str::from_utf8(&data)
                            .map_err(|e| e.to_string())?
                            .lines()
                            .take_while(|l| !l.is_empty())
                            .filter_map(|l| l.strip_prefix("parent "))
                            .collect::<Vec<_>>()
                    ))
                }
                "blob-bytes" => Ok(json!(git(
                    &root,
                    &[
                        "show",
                        &format!("{}:{}", field(&q, "commit")?, field(&q, "path")?)
                    ]
                )?)),
                "checkout-excluding" => Ok(facts["checkout"].clone()),
                "registry-snapshot" => {
                    let key = if endpoint()? == "base" {
                        "before"
                    } else {
                        "after"
                    };
                    let mut snapshot = facts[key].clone();
                    if root
                        .join(".chrono-harness/state/mock-inconsistent-registry")
                        .exists()
                    {
                        let effective = field(&snapshot, "effective_path")?.to_owned();
                        snapshot["values"][effective]["status"] = json!("forged");
                    }
                    Ok(snapshot)
                }
                other => Err(format!("unexpected fixture Git operation {other}")),
            }
        })();
        reply(&mut out, result)?;
    }
    Ok(())
}
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let result = Command::new(env!("CHRONO_TEST_GIT"))
        .arg("--no-replace-objects")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if result.status.success() {
        Ok(result.stdout)
    } else {
        Err(String::from_utf8_lossy(&result.stderr).into_owned())
    }
}
fn retained_session() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::current_dir()?;
    let mut out = std::io::stdout().lock();
    for frame in std::io::stdin().lock().lines() {
        let query: Value = serde_json::from_str(&frame?)?;
        let result = (|| {
            let path = || field(&query, "path");
            match field(&query, "operation")? {
                "read-bounded-text" => {
                    let limit = query["limit"]
                        .as_u64()
                        .ok_or("missing fixture size limit")?;
                    let text =
                        std::fs::read_to_string(root.join(path()?)).map_err(|e| e.to_string())?;
                    if text.len() as u64 > limit {
                        return Err("fixture input exceeds collection limit".into());
                    }
                    Ok(Value::String(text))
                }
                "resolve-original" => {
                    let original = field(&query["original"], "path")?;
                    let relocated = if query["transport"].is_null() {
                        original.to_owned()
                    } else {
                        let transport = &query["transport"];
                        let suffix = original
                            .strip_prefix(field(transport, "source_directory")?)
                            .ok_or("fixture path outside source directory")?;
                        format!("{}{suffix}", field(transport, "directory")?)
                    };
                    Ok(Value::String(relocated))
                }
                "retain-read" => {
                    checked(&root, path()?)?;
                    Ok(Value::Null)
                }
                "checked-path" => Ok(json!(checked(&root, path()?)?)),
                "artifact-identity" => {
                    let transport = &query["transport"];
                    let relative = path()?;
                    let target = if transport.is_null() {
                        relative.to_owned()
                    } else {
                        let suffix = relative
                            .strip_prefix(field(transport, "source_directory")?)
                            .ok_or("fixture source mismatch")?;
                        format!("{}{suffix}", field(transport, "directory")?)
                    };
                    let bytes =
                        std::fs::read(checked(&root, &target)?).map_err(|e| e.to_string())?;
                    let digest = wire::sha256(&bytes);
                    let original = &query["artifacts"][relative];
                    if original["sha256"] != digest || original["length"] != bytes.len() {
                        return Err("fixture original digest/length mismatch".into());
                    }
                    Ok(json!({"sha256":digest,"length":bytes.len()}))
                }
                op => Err(format!("unexpected fixture custody operation {op}")),
            }
        })();
        reply(&mut out, result)?;
    }
    Ok(())
}

fn compiler_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let query: Value = serde_json::from_reader(std::io::stdin().lock())?;
    let reply = match query["operation"].as_str() {
        Some("source-declarations")
            if query["files"] == json!([]) && query["operations"] == json!([]) =>
        {
            let paths = query["reference_paths"]
                .as_array()
                .ok_or("missing reference paths")?;
            let values = paths
                .iter()
                .map(|path| match path.as_str() {
                    Some("x") => Ok(true),
                    Some("./x") => Ok(false),
                    _ => Err("unexpected reference fixture path"),
                })
                .collect::<Result<Vec<_>, _>>()?;
            json!(values)
        }
        Some("source-declarations")
            if query["operations"] == json!([]) && query["reference_paths"] == json!([]) =>
        {
            let files = &query["files"];
            if files
                == &json!([{"path":"declaration.rs","verification":{"role":"data","roots":[]}}])
            {
                json!([])
            } else if files
                == &json!([{"path":"declaration.rs","verification":{"role":"program","roots":[]}}])
            {
                return Err("declaration.rs: source declaration rejected by fixture".into());
            } else {
                return Err("unexpected source declaration fixture".into());
            }
        }
        Some("validate-contract")
            if query
                == json!({"operation":"validate-contract","contract":{"version":"fixture","commit":"valid"}}) =>
        {
            Value::Null
        }
        Some("validate-contract")
            if query
                == json!({"operation":"validate-contract","contract":{"version":"fixture","commit":""}}) =>
        {
            return Err("output contract rejected by fixture".into());
        }
        _ => return Err(format!("unexpected compiler fixture request: {query}").into()),
    };
    serde_json::to_writer(std::io::stdout().lock(), &reply)?;
    Ok(())
}

fn evidence_session() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let query: Value = serde_json::from_str(&line?)?;
        let result = (|| match field(&query, "operation")? {
            "validate-request" | "validate-initial-request" => {
                let mut request = query["request"].clone();
                let id = request
                    .as_object_mut()
                    .ok_or("request object")?
                    .remove("request_id")
                    .ok_or("request id")?;
                if wire::digest(&request)? != id {
                    return Err("fixture: request identity mismatch".into());
                }
                Ok(Value::Null)
            }
            "bound-judge-path" => {
                let binding = &query["binding"];
                let path = std::env::current_dir()
                    .map_err(|e| e.to_string())?
                    .join(field(binding, "executable")?);
                let image = fs::canonicalize(path).map_err(|e| e.to_string())?;
                if binding["sha256"] != wire::sha256(&fs::read(&image).map_err(|e| e.to_string())?)
                {
                    return Err("fixture: judge digest differs".into());
                }
                Ok(json!(image))
            }
            "report-paths" => {
                let block = &query["profile"]["execution_units"];
                let collection = field(block, "report_path")?;
                let one = field(&block["units"]["one"], "report_path")?;
                let two = field(&block["units"]["two"], "report_path")?;
                let paths =
                    [collection, one, two].map(|p| p.strip_prefix(".chrono-harness/state/"));
                match paths {
                    [Some("report.json"), _, _]
                    | [_, Some("report.json"), _]
                    | [Some("one.json"), Some("one.json"), _]
                    | [Some("reports"), Some("reports/one.json"), _]
                    | [Some("reports/all.json"), Some("reports"), _]
                    | [_, Some("one.json"), Some("one.json")]
                    | [_, Some("reports"), Some("reports/two.json")]
                    | [_, Some("reports/one.json"), Some("reports")] => {
                        Err("fixture rejects overlapping unit report paths".into())
                    }
                    [Some(_), Some(_), Some(_)] => Ok(Value::Null),
                    _ => Err("unrecognized fixture report path scope".into()),
                }
            }
            "credential-environment" => {
                match &query["config"]["environment"]["credential_environment"] {
                    names if names == &json!(["TOKEN"]) => Ok(names.clone()),
                    names if names == &json!(["UNDECLARED"]) => {
                        Err("invalid acquisition credential environment binding".into())
                    }
                    names if names.is_null() => Ok(json!([])),
                    _ => Err("unexpected credentials fixture".into()),
                }
            }
            "validate-schedule" if query["bindings"].is_array() => Ok(Value::Null),
            "validate-git-declaration" if query["config"]["facts_git"]["tool"].is_string() => {
                Ok(Value::Null)
            }
            "validate-preparation-declaration" if query["config"]["schema_version"] == 4 => {
                Ok(Value::Null)
            }
            "validate-initial-profile"
                if query["profile"]["schema"] == "chrono-initial-check/v1" =>
            {
                Ok(Value::Null)
            }
            "full-execution-units" if query["profile"]["execution_units"].is_object() => {
                Ok(query["profile"]["execution_units"].clone())
            }
            "registry-identity" if query["entry"] == ".chrono-harness/platforms.json" => {
                let entry = &query["entry"];
                let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
                let target = &query["documents"][entry.as_str().unwrap()]["platforms"][&platform];
                Ok(
                    json!({"entry_path":entry,"effective_path":target,"selection":{"path":entry,"platform":platform,"config_path":target}}),
                )
            }
            "artifact-directories" => Ok(json!(
                query["config"]["artifacts"]
                    .as_array()
                    .ok_or("artifacts array")?
                    .iter()
                    .map(|r| r["path"].clone())
                    .collect::<Vec<_>>()
            )),
            "observe-record" => {
                let record = &query["record"];
                let process = &record["process"];
                if process["exit_code"] != 0 || !process["failure"].is_null() {
                    return Err("fixture process failed".into());
                }
                Ok(
                    json!({"record":record,"stdout_matches":process["stdout_sha256"]==wire::digest(&record["response"])?}),
                )
            }
            op => Err(format!("unexpected fixture evidence operation {op}")),
        })();
        reply(&mut out, result)?;
    }
    Ok(())
}
// Emulate the runner boundary and return exactly the preregistered selector reply.
fn record_classification() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;
    let mut input = std::io::stdin().lock();
    let mut line = String::new();
    input.read_line(&mut line)?;
    let header: Value = serde_json::from_str(&line)?;
    let mut payload = Vec::new();
    input.read_to_end(&mut payload)?;
    let message: Value = serde_json::from_slice(&payload)?;
    let state = Path::new(".chrono-harness/state");
    fs::write(
        state.join("selector-observed.json"),
        serde_json::to_vec(&json!({"header":header, "payload":message}))?,
    )?;
    let plan = fs::read(state.join("selector-plan.json"))?;
    let process = json!({
        "exit_code":0, "failure":null, "executable":header["program"], "sha256":header["expected_sha256"],
        "cwd":std::env::current_dir()?, "argv":header["args"], "environment":header["env"],
        "stdin_sha256":wire::sha256(&payload), "stdout_bytes":plan,
        "stdout_sha256":wire::sha256(&plan), "stdout":std::str::from_utf8(&plan)?, "stderr":""
    });
    serde_json::to_writer(std::io::stdout().lock(), &process)?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match args.as_slice() {
        [op] if op == "record-command-v1" => record_classification(),
        [op] if op == "compiler-output-v1" => compiler_fixture(),
        [op] if op == "git-facts-v1" => facts_session(),
        [op] if op == "judge-evidence-v1" => evidence_session(),
        [op] if op == "retained-artifacts-v1" => retained_session(),
        _ => Err("unexpected runner fixture argv".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("registration fixture: {error}");
            ExitCode::FAILURE
        }
    }
}
