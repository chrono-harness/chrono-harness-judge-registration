//! Supply fixed request and Git facts to the registration executable.
use super::*;
use chrono_judge_registration::wire::{self, digest};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    process::Stdio,
};

fn raw(root: &Path, arguments: &[&str]) -> Vec<u8> {
    let result = Command::new(fixture_git())
        .arg("--no-replace-objects")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{arguments:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}
pub(super) fn blob(root: &Path, commit: &str, path: &str) -> Vec<u8> {
    raw(root, &["show", &format!("{commit}:{path}")])
}
fn entries(root: &Path, commit: &str) -> BTreeMap<String, Value> {
    let bytes = raw(root, &["ls-tree", "-rz", "--full-tree", commit]);
    std::str::from_utf8(&bytes).unwrap().split_terminator('\0').map(|record|{
        let (header,path)=record.split_once('\t').unwrap();
        let mut fields=header.split(' ');
        (path.to_owned(),json!({"mode":fields.next().unwrap(),"kind":fields.next().unwrap(),"blob":fields.next().unwrap()}))
    }).collect()
}
fn changes(base: &BTreeMap<String, Value>, candidate: &BTreeMap<String, Value>) -> Vec<Value> {
    let paths: BTreeSet<_> = base.keys().chain(candidate.keys()).collect();
    paths.into_iter().filter_map(|path|{
        let before=base.get(path); let after=candidate.get(path);
        if before==after {return None;}
        let kind=match (before,after) {(None,Some(_))=>"A",(Some(_),None)=>"D",_=>"M"};
        Some(json!({"kind":kind,"path":path,"old_blob":before.map(|e|&e["blob"]),"new_blob":after.map(|e|&e["blob"]),"old_mode":before.map(|e|&e["mode"]),"new_mode":after.map(|e|&e["mode"])}))
    }).collect()
}

fn snapshot(root: &Path, commit: &str, entry: &str) -> Value {
    let mut originals = BTreeMap::new();
    let mut values = BTreeMap::new();
    let mut read = |path: &str| {
        let original = String::from_utf8(blob(root, commit, path)).unwrap();
        let value: Value = serde_json::from_str(&original).unwrap();
        originals.insert(path.to_owned(), original);
        values.insert(path.to_owned(), value.clone());
        value
    };
    let start = read(entry);
    let (effective, config, selection) = if start["schema"] == "chrono-git-configs/v1" {
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let target = start["platforms"][&platform].as_str().unwrap();
        (
            target,
            read(target),
            json!({"path":entry,"platform":platform,"config_path":target}),
        )
    } else {
        (entry, start.clone(), Value::Null)
    };
    for path in config["registries"].as_object().unwrap().values() {
        read(path.as_str().unwrap());
    }
    json!({"effective_path":effective,"selection":selection,"values":values,"originals":originals})
}

pub(super) fn recorded_delta(host: &Host) -> Value {
    let facts: Value = serde_json::from_slice(
        &fs::read(host.root().join(".chrono-harness/state/fixture-facts.json")).unwrap(),
    )
    .unwrap();
    facts["delta"].clone()
}

pub(super) fn checkout_case(host: &Host, case: &str) {
    let mut checkout = json!({"head":host.candidate,"tracked":[],"untracked":[],"index_flags":[]});
    match case {
        "ignored" => checkout["untracked"] = json!(["ignored.txt"]),
        "untracked" => checkout["untracked"] = json!(["new.txt"]),
        "tracked" | "unstaged" | "staged" => checkout["tracked"] = json!(["document.txt"]),
        "head" => checkout["head"] = json!(host.base),
        "assume-unchanged" | "--assume-unchanged" => {
            checkout["index_flags"] = json!([{"tag":"h","path":"document.txt"}])
        }
        "--skip-worktree" => checkout["index_flags"] = json!([{"tag":"S","path":"document.txt"}]),
        _ => panic!("unexpected checkout case {case}"),
    }
    write(
        host.root(),
        ".chrono-harness/state/fixture-checkout.json",
        &checkout,
    );
}

pub(super) fn invoke(host: &Host, context: Value, initial: bool) -> (i32, Value) {
    let root = fs::canonicalize(host.root()).unwrap();
    let state = root.join(".chrono-harness/state");
    fs::create_dir_all(&state).unwrap();
    let profile = ".chrono-harness/initial.json";
    let config = if initial {
        let profile: Value =
            serde_json::from_slice(&fs::read(root.join(profile)).unwrap()).unwrap();
        profile["host_config"].as_str().unwrap().to_owned()
    } else {
        ".chrono-harness/config.json".to_owned()
    };
    let after = snapshot(&root, &host.candidate, &config);
    let tree = |oid: &str| git(&root, &["rev-parse", &format!("{oid}^{{tree}}")]);
    let candidate_tree = entries(&root, &host.candidate);
    let runner_path = root.join(".chrono-harness/bin/chrono-harness");
    let runner = json!({"path":runner_path,"sha256":wire::sha256(&fs::read(&runner_path).unwrap()),"version":"0.1.0"});
    let checkout=fs::read(state.join("fixture-checkout.json")).map(|b|serde_json::from_slice(&b).unwrap()).unwrap_or_else(|_|json!({"head":git(&root,&["rev-parse","HEAD"]),"tracked":[],"untracked":[],"index_flags":[]}));
    let candidate = json!({"commit":host.candidate,"tree":tree(&host.candidate),"root":root});
    let effective = &after["values"][after["effective_path"].as_str().unwrap()];
    let mut environment = effective["environment"]["values"].clone();
    let mut inherited = serde_json::Map::new();
    for name in effective["environment"]["inherit"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        let val = std::env::var(name).ok();
        if let Some(value) = &val {
            environment[name] = json!(value);
        }
        inherited.insert(name.into(), json!(val));
    }
    let mut obs = json!({"environment":{"inherited":inherited,"effective":environment}});
    if let Some(relative) = context["retained_inputs"].as_str() {
        obs["retained"] = serde_json::from_slice(&fs::read(root.join(relative)).unwrap()).unwrap();
    }
    let mut facts = json!({"candidate":candidate,"after":after,"entries":candidate_tree,"checkout":checkout,"delta":null,"observation":null});
    let mut request = if initial {
        obs["registry_binding"] = json!({"entry_path":config,"effective_path":after["effective_path"],"selection":after["selection"]});
        json!({"protocol":wire::INITIAL_PROTOCOL,"judge_id":"initial-registration","mode":"inventory","candidate":candidate,
          "profile_path":profile,"profile_sha256":wire::sha256(&fs::read(root.join(profile)).unwrap()),"config_path":config,
          "registry_digest":digest(&after["values"]).unwrap(),"checkout":checkout,"runner":runner,"observations":obs,"prior_results":[]})
    } else {
        let before = snapshot(&root, &host.base, &config);
        let base_root = state.join("base");
        for (path, text) in before["originals"].as_object().unwrap() {
            let p = base_root.join(path);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text.as_str().unwrap()).unwrap();
        }
        let base = json!({"commit":host.base,"tree":tree(&host.base),"root":base_root});
        let delta = changes(&entries(&root, &host.base), &candidate_tree);
        facts["base"] = base.clone();
        facts["before"] = before.clone();
        facts["delta"] = json!(delta);
        write(&root, ".chrono-harness/state/context.json", &context);
        json!({"protocol":wire::PROTOCOL,"judge_id":"registration","mode":"evaluate","base":base,"candidate":candidate,"delta":delta,
         "registries":{"base":base_root.join(".chrono-harness"),"candidate":root.join(".chrono-harness"),"digest":digest(&json!({"base":before["values"],"candidate":after["values"]})).unwrap()},
         "context":{"path":state.join("context.json"),"sha256":digest(&context).unwrap()},"impact":{},"prior_results":[],"config_path":config,
         "checkout":checkout,"runner":runner,"observations":obs})
    };
    request["request_id"] = json!(digest(&request).unwrap());
    write(&root, ".chrono-harness/state/fixture-facts.json", &facts);
    let mut child = Command::new(root.join(".chrono-harness/bin/chrono-judge-registration"))
        .current_dir(&root)
        .args(["--protocol", request["protocol"].as_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&request).unwrap())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let response = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
        panic!(
            "registration response: {}; {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out.status.code().unwrap(), response)
}
