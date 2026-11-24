use chrono_harness::{facts, wire::Request};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, Write},
    path::PathBuf,
    process::ExitCode,
};

// The fixture runs against the modeled Git on its registered test PATH.
// Only the operations consumed by registration belong in this protocol double.
struct FactsFixture {
    root: PathBuf,
    base: Option<String>,
    candidate: String,
    config: String,
    git: facts::Reader,
}

fn field<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("missing fixture field {key}"))
}

impl FactsFixture {
    fn open(header: Value) -> Result<Self, String> {
        match field(&header, "mode")? {
            "request" => {
                let request: Request =
                    serde_json::from_value(header["request"].clone()).map_err(|e| e.to_string())?;
                let git = facts::Reader::for_request(&request)?;
                Ok(Self {
                    root: request.candidate.root,
                    base: Some(request.base.commit),
                    candidate: request.candidate.commit,
                    config: request.config_path,
                    git,
                })
            }
            "observed" => {
                let root = std::env::current_dir().map_err(|e| e.to_string())?;
                let config = field(&header, "config")?.to_owned();
                let candidate = field(&header, "candidate")?.to_owned();
                if !header["base"].is_null() {
                    return Err("initial fixture received a baseline".into());
                }
                let git = facts::Reader::from_observations(
                    &root,
                    &config,
                    &candidate,
                    &header["observations"],
                )?;
                Ok(Self {
                    root,
                    base: None,
                    candidate,
                    config,
                    git,
                })
            }
            mode => Err(format!("unexpected registration fixture mode {mode}")),
        }
    }
    fn operation(&self, query: &Value) -> Result<Value, String> {
        let endpoint = || match field(query, "endpoint")? {
            "base" => self
                .base
                .as_deref()
                .ok_or_else(|| "initial fixture has no base".into()),
            "candidate" => Ok(self.candidate.as_str()),
            other => Err(format!("unexpected fixture endpoint {other}")),
        };
        let root = &self.root;
        let git = &self.git;
        match field(query, "op")? {
            "observation" => Ok(git.observation()),
            "checkout-root" => {
                let top = facts::utf8(git.git(root, &["rev-parse", "--show-toplevel"])?)?;
                Ok(json!(
                    std::fs::canonicalize(top.trim()).map_err(|e| e.to_string())?
                ))
            }
            "verify-tree" => Ok(json!(git.verify_oid(root, endpoint()?)?)),
            "verify-commit" => Ok(json!(git.verify_oid(root, field(query, "commit")?)?)),
            "parents" => Ok(json!(git.parents(root, field(query, "commit")?)?)),
            "blob-bytes" => Ok(json!(git.blob(
                root,
                field(query, "commit")?,
                field(query, "path")?
            )?)),
            "checkout-excluding" => {
                let artifacts: Vec<&str> = serde::Deserialize::deserialize(&query["artifacts"])
                    .map_err(|e| e.to_string())?;
                Ok(json!(git.checkout_excluding(
                    root,
                    &self.candidate,
                    &artifacts
                )?))
            }
            "candidate-inventory" => {
                let candidate = git.tree(root, &self.candidate)?;
                let delta = self
                    .base
                    .as_ref()
                    .map(|base| {
                        git.tree(root, base)
                            .map(|old| facts::delta(&old, &candidate))
                    })
                    .transpose()?;
                let entries: BTreeMap<_, _> = candidate
                    .into_iter()
                    .map(|(path, file)| (path.text, json!({"kind":file.kind, "mode":file.mode})))
                    .collect();
                Ok(json!({"entries":entries, "delta":delta}))
            }
            "registry-snapshot" => {
                let mut snap = git.registry_snapshot(root, endpoint()?, &self.config)?;
                if root
                    .join(".chrono-harness/state/mock-inconsistent-registry")
                    .exists()
                {
                    snap.values.get_mut(&self.config).unwrap()["status"] = json!("forged");
                }
                let originals: BTreeMap<_, _> = snap
                    .bytes
                    .into_iter()
                    .map(|(path, bytes)| String::from_utf8(bytes).map(|text| (path, text)))
                    .collect::<Result<_, _>>()
                    .map_err(|e| e.to_string())?;
                Ok(
                    json!({"values":snap.values,"originals":originals,"effective_path":snap.effective_path,"selection":snap.selection}),
                )
            }
            op => Err(format!("unexpected registration fixture operation {op}")),
        }
    }
}

fn reply(out: &mut impl Write, value: Result<Value, String>) -> std::io::Result<()> {
    serde_json::to_writer(&mut *out, &value)?;
    out.write_all(b"\n")?;
    out.flush()
}

fn facts_session() -> Result<(), Box<dyn std::error::Error>> {
    let mut frames = std::io::stdin().lock().lines();
    let header = frames.next().ok_or("missing fixture header")??;
    let mut out = std::io::stdout().lock();
    let fixture = match FactsFixture::open(serde_json::from_str(&header)?) {
        Ok(fixture) => fixture,
        Err(error) => {
            reply(&mut out, Err(error))?;
            return Ok(());
        }
    };
    reply(&mut out, Ok(json!({"bound":fixture.git.is_bound()})))?;
    for frame in frames {
        let query = serde_json::from_str(&frame?)?;
        reply(&mut out, fixture.operation(&query))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    if std::env::args_os()
        .skip(1)
        .eq([std::ffi::OsString::from("git-facts-v1")])
    {
        return match facts_session() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("registration fixture: {error}");
                ExitCode::FAILURE
            }
        };
    }
    chrono_harness::cli_main()
}
