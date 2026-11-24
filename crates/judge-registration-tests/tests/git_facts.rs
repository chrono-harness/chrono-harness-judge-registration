use super::*;

#[path = "input_coverage.rs"]
mod input_coverage;

const CONFIG: &str = ".chrono-harness/config.json";

fn retained_input(root: &Path, path: &Path) -> Value {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return json!({"absent":true}),
        Err(e) => panic!("snapshot input: {e}"),
    };
    // Preserve the complete original without expanding an executable into JSON
    // values. Bounded inline/blob equivalence is covered by registration_contract.
    let digest = sha256(&bytes);
    let blob = format!(".chrono-harness/state/fixture-{digest}");
    fs::write(root.join(&blob), &bytes).unwrap();
    json!({"blob":blob,"sha256":digest,"length":bytes.len()})
}

struct BoundHost {
    host: Host,
}
impl BoundHost {
    fn new() -> Self {
        let mut h = Host::new();
        let root = fs::canonicalize(h.root()).unwrap();
        let state = root.join(".chrono-harness/state");
        fs::create_dir_all(&state).unwrap();
        let program = state.join("chosen Git λ");
        let trace = state.join("chosen-git.trace");
        let real = super::fixture_git();
        let version = Command::new(&real).arg("--version").output().unwrap();
        assert!(version.status.success());
        fs::copy(&real, &program).unwrap();
        let mut config: Value =
            serde_json::from_slice(&fs::read(root.join(CONFIG)).unwrap()).unwrap();
        config["schema_version"] = json!(3);
        config["facts_git"] = json!({"tool":"facts-git","input":"facts-git-bytes"});
        config["tools"] = json!([{"id":"facts-git","program":program,"resolution":"PATH-once",
            "version_argv":["--version"],"expected_version":String::from_utf8(version.stdout).unwrap().trim()}]);
        config["environment"] = json!({"inherit":[],"values":{"PATH":std::env::var("PATH").unwrap(),
            "CHRONO_FACTS_TRACE":trace,"BOUND_FACTS_VALUE":"declared",
            "CHRONO_TEST_GIT_PROFILE":"registration","FACTS_GIT_MODE":"pass",
            "GIT_CONFIG_NOSYSTEM":"1","GIT_CONFIG_GLOBAL":"/dev/null"},
            "inputs":[{"id":"facts-git-bytes","location":program,"presence":"present","sha256":sha256(&fs::read(&program).unwrap())}]});
        config["input_closure"] = json!({
            "status": "declared-complete",
            "unresolved": [],
            "bindings": [{
                "id": "registration-git-facts",
                "consumer": "judge:registration",
                "kind": "git-facts",
                "inputs": [
                    "tool:facts-git",
                    "input:facts-git-bytes",
                    "environment:PATH",
                    "environment:CHRONO_FACTS_TRACE",
                    "environment:BOUND_FACTS_VALUE",
                    "environment:CHRONO_TEST_GIT_PROFILE",
                    "environment:FACTS_GIT_MODE",
                    "environment:GIT_CONFIG_NOSYSTEM",
                    "environment:GIT_CONFIG_GLOBAL"
                ]
            }]
        });
        write(&root, CONFIG, &config);
        let path = ".chrono-harness/FILEMAP.json";
        let mut map: Value = serde_json::from_slice(&fs::read(root.join(path)).unwrap()).unwrap();
        map["project_edges"] = json!([
            {"from":"tool:facts-git","kind":"runtime-input","to":"judge:registration"},
            {"from":"input:facts-git-bytes","kind":"judge-trigger","to":"judge:registration"},
            {"from":"environment:PATH","kind":"runtime-input","to":"judge:registration"},
            {"from":"environment:CHRONO_FACTS_TRACE","kind":"runtime-input","to":"judge:registration"},
            {"from":"environment:BOUND_FACTS_VALUE","kind":"runtime-input","to":"judge:registration"},
            {"from":"environment:CHRONO_TEST_GIT_PROFILE","kind":"runtime-input","to":"judge:registration"},
            {"from":"environment:FACTS_GIT_MODE","kind":"runtime-input","to":"judge:registration"},
            {"from":"environment:GIT_CONFIG_NOSYSTEM","kind":"runtime-input","to":"judge:registration"},
            {"from":"environment:GIT_CONFIG_GLOBAL","kind":"runtime-input","to":"judge:registration"}
        ]);
        write(&root, path, &map);
        h.base = commit(&root);
        fs::write(root.join("document.txt"), "new candidate using bound facts").unwrap();
        h.candidate = commit(&root);
        Self { host: h }
    }
    fn run(&self) -> (i32, Value) {
        run_with_endpoint_inputs(self)
    }
}

// Retain each endpoint's own configuration. Adopting coverage must not rewrite
// the old v3 config or require executing the previous judge binary.
fn run_with_endpoint_inputs(h: &BoundHost) -> (i32, Value) {
    let snapshot = |oid: &str| {
        let config: Value =
            serde_json::from_slice(&request_fixture::blob(h.host.root(), oid, CONFIG)).unwrap();
        let mut files = serde_json::Map::new();
        for input in config["environment"]["inputs"].as_array().unwrap() {
            let observed = retained_input(
                h.host.root(),
                &h.host.root().join(input["location"].as_str().unwrap()),
            );
            files.insert(input["id"].as_str().unwrap().to_owned(), observed);
        }
        json!({"schema":"chrono-input-snapshot/v2","commit":oid,"config_path":CONFIG,
            "config_digest":wire::digest(&config).unwrap(),"environment":{},"files":files})
    };
    write(
        h.host.root(),
        ".chrono-harness/state/coverage-inputs.json",
        &json!({"base":snapshot(&h.host.base),"candidate":snapshot(&h.host.candidate)}),
    );
    let mut context = h.host.context();
    context["retained_inputs"] = json!(".chrono-harness/state/coverage-inputs.json");
    h.host.run_context(context)
}

#[test]
fn required_candidate_snapshot_is_rejected_by_registration() {
    let h = BoundHost::new();
    let (exit, complete) = h.run();
    assert_eq!(exit, 0, "{complete}");

    let path = ".chrono-harness/state/coverage-inputs.json";
    let mut pair: Value =
        serde_json::from_slice(&fs::read(h.host.root().join(path)).unwrap()).unwrap();
    pair.as_object_mut().unwrap().remove("candidate");
    write(h.host.root(), path, &pair);
    let mut context = h.host.context();
    context["retained_inputs"] = json!(path);
    let (exit, response) = h.host.run_context(context);
    assert_ne!(exit, 0, "{response}");
    assert!(
        response["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|result| {
                result["message"].as_str().is_some_and(|cause| {
                    cause.contains("candidate: config v2 requires snapshot v2")
                })
            }),
        "{response}"
    );
}

#[test]
fn input_closure_bindings_accept_explicit_external_nodes() {
    let mut h = BoundHost::new();
    h.host.edit(CONFIG, |config| {
        config["input_closure"]["bindings"] = json!([{
            "id": "registration-git-facts",
            "consumer": "judge:registration",
            "kind": "git-facts-explicit",
            "inputs": [
                "tool:facts-git",
                "input:facts-git-bytes",
                "environment:PATH",
                "environment:CHRONO_FACTS_TRACE",
                "environment:BOUND_FACTS_VALUE",
                "environment:CHRONO_TEST_GIT_PROFILE",
                "environment:FACTS_GIT_MODE",
                "environment:GIT_CONFIG_NOSYSTEM",
                "environment:GIT_CONFIG_GLOBAL"
            ]
        }]);
    });
    h.host.base = h.host.candidate.clone();
    fs::write(
        h.host.root().join("document.txt"),
        "candidate after closure binding",
    )
    .unwrap();
    h.host.candidate = commit(h.host.root());
    let (exit, report) = h.run();
    assert_eq!(exit, 0, "explicit closure binding must pass: {report}");
    assert_eq!(report["status"], "pass");
}

#[test]
fn declared_complete_v3_closure_requires_all_external_nodes() {
    let mut h = BoundHost::new();
    h.host.edit(CONFIG, |config| {
        config["input_closure"] = json!({
            "status": "declared-complete",
            "unresolved": [],
            "bindings": [{
                "id": "registration-git-facts",
                "consumer": "judge:registration",
                "kind": "git-facts",
                "inputs": ["input:facts-git-bytes"]
            }]
        });
    });
    h.host.base = h.host.candidate.clone();
    fs::write(
        h.host.root().join("document.txt"),
        "candidate after incomplete declared closure",
    )
    .unwrap();
    h.host.candidate = commit(h.host.root());
    let (exit, report) = h.run();
    assert_ne!(exit, 0, "omitted external nodes must fail: {report}");
    assert!(finding(&report, "E_INPUT_UNDECLARED"), "{report}");
    assert!(format!("{report}").contains("tool:facts-git"));
}

#[test]
fn declared_complete_v3_closure_accepts_explicit_external_inventory() {
    let mut h = BoundHost::new();
    // BoundHost already installs this complete v3 inventory. Retain the
    // fixture's candidate transition below so the test exercises the reader
    // without creating a no-op commit.
    h.host.base = h.host.candidate.clone();
    fs::write(
        h.host.root().join("document.txt"),
        "candidate after complete declared closure",
    )
    .unwrap();
    h.host.candidate = commit(h.host.root());
    let (exit, report) = h.run();
    assert_eq!(exit, 0, "explicit external inventory must pass: {report}");
    assert_eq!(report["status"], "pass");
}

#[test]
fn declared_complete_v3_input_closure_requires_bindings_field() {
    let mut h = BoundHost::new();
    h.host.edit(CONFIG, |config| {
        config["input_closure"] = json!({
            "status": "declared-complete",
            "unresolved": []
        });
    });
    h.host.base = h.host.candidate.clone();
    fs::write(
        h.host.root().join("document.txt"),
        "candidate after missing bindings field",
    )
    .unwrap();
    h.host.candidate = commit(h.host.root());
    let (exit, report) = h.run();
    assert_ne!(
        exit, 0,
        "missing bindings must fail schema validation: {report}"
    );
    assert!(finding(&report, "E_SCHEMA"), "{report}");
}

#[test]
fn input_closure_bindings_reject_unknown_nodes_without_discovery() {
    for (field, value) in [
        ("consumer", json!("project:unknown")),
        ("inputs", json!(["input:unknown"])),
    ] {
        let mut h = BoundHost::new();
        h.host.edit(CONFIG, |config| {
            config["input_closure"]["bindings"] = json!([{
                "id": "invalid-closure",
                "consumer": "judge:registration",
                "kind": "git-facts",
                "inputs": ["tool:facts-git"]
            }]);
            config["input_closure"]["bindings"][0][field] = value.clone();
        });
        h.host.base = h.host.candidate.clone();
        fs::write(
            h.host.root().join("document.txt"),
            "candidate after invalid closure binding",
        )
        .unwrap();
        h.host.candidate = commit(h.host.root());
        let (exit, report) = h.run();
        assert_ne!(exit, 0, "{field}: {report}");
        assert!(finding(&report, "E_REFERENCE"), "{field}: {report}");
    }
}

#[test]
fn input_closure_bindings_require_explicit_project_edges() {
    let mut h = BoundHost::new();
    h.host.edit(CONFIG, |config| {
        config["input_closure"]["bindings"] = json!([{
            "id": "registration-git-facts",
            "consumer": "judge:registration",
            "kind": "git-facts",
            "inputs": [
                "tool:facts-git",
                "input:facts-git-bytes",
                "environment:PATH"
            ]
        }]);
    });
    h.host.edit(".chrono-harness/FILEMAP.json", |filemap| {
        filemap["project_edges"] = json!([
            {"from":"input:facts-git-bytes","kind":"judge-trigger","to":"judge:registration"},
            {"from":"environment:PATH","kind":"runtime-input","to":"judge:registration"}
        ]);
    });
    h.host.base = h.host.candidate.clone();
    fs::write(
        h.host.root().join("document.txt"),
        "candidate after missing closure edge",
    )
    .unwrap();
    h.host.candidate = commit(h.host.root());
    let (exit, report) = h.run();
    assert_ne!(exit, 0, "missing input edge must fail: {report}");
    assert!(finding(&report, "E_REFERENCE"), "{report}");
    assert!(format!("{report}").contains("tool:facts-git"));
}
