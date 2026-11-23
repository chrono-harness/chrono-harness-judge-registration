//! FILEMAP consumes these registration responses at the process boundary.
use chrono_judge_registration::impact_views;
use chrono_registration_fixture::{CONFIG, FM, Values, load, script_pair, values};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

fn originals(values: &Values) -> BTreeMap<&str, String> {
    values
        .iter()
        .map(|(path, value)| (path.as_str(), value.to_string()))
        .collect()
}

fn fixed(base: &Values, candidate: &Values) -> Result<Value, String> {
    let query = json!({
        "operation":{"mode":"endpoints","base":"1".repeat(40),"candidate":"2".repeat(40),"config":CONFIG},
        "originals":[originals(base),originals(candidate)]
    });
    impact_views::query(Path::new("."), query.to_string().as_bytes())
}

#[test]
fn invalid_registration_cannot_be_interpreted_as_empty_impact() {
    let valid = values();
    let mut absent = valid.clone();
    absent.remove(FM);
    assert!(fixed(&absent, &valid).is_err());
    let mut wrong_files = valid.clone();
    wrong_files.get_mut(FM).unwrap()["files"] = json!({});
    assert!(fixed(&valid, &wrong_files).is_err());
    let mut guessed = valid.clone();
    guessed.get_mut(FM).unwrap()["files"][0]["edges"][0]["kind"] = json!("guessed");
    assert!(fixed(&guessed, &valid).is_err());
}

#[test]
fn projection_preserves_colliding_and_unique_test_definitions() {
    let mut base = values();
    script_pair(&mut base, "t");
    let mut candidate = base.clone();
    script_pair(&mut candidate, "st2");
    let data = fixed(&base, &candidate).unwrap();
    let old = &data["base"];
    let new = &data["candidate"];
    assert_eq!(old["active_test_aliases"]["project:t"], json!(["test:t"]));
    assert_eq!(old["active_test_aliases"]["script:t"], json!(["test:t"]));
    assert_eq!(new["active_test_aliases"]["project:t"], json!(["test:t"]));
    assert_eq!(
        new["active_test_aliases"]["script:st2"],
        json!(["test:st2"])
    );
    assert_eq!(old["nodes"]["test:t"]["kind"], "test");
    let actions = |view: &Value, id: &str| {
        view["nodes"][id]["definitions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|definition| {
                (
                    definition["identity"].as_str().unwrap().to_owned(),
                    definition["value"]["actions"]["execute"]["operation"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        actions(old, "test:t"),
        [
            ("project:t".into(), "execute.t".into()),
            ("script:t".into(), "script.st".into())
        ]
    );
    assert_eq!(
        actions(new, "test:t"),
        [("project:t".into(), "execute.t".into())]
    );
    assert_eq!(
        actions(new, "test:st2"),
        [("script:st2".into(), "script.st".into())]
    );
    for (registrations, encoded) in [(load(&base), old), (load(&candidate), new)] {
        for (id, view) in registrations.node_data() {
            let wire = &encoded["nodes"][&id]["definitions"];
            assert_eq!(
                wire.as_array().unwrap().len(),
                view.definitions.len(),
                "{id}"
            );
            for (n, definition) in view.definitions.iter().enumerate() {
                assert_eq!(wire[n]["identity"], definition.identity);
                assert_eq!(&wire[n]["value"], definition.value);
            }
            assert_eq!(wire.as_array().unwrap().len() == 1, view.unique().is_some());
        }
    }
    assert_eq!(data["repaired"], json!([]));
    assert!(data.get("git_facts").is_none());
}

#[test]
fn retained_environment_excludes_undeclared_entries_even_with_a_matching_digest() {
    let mut before = values();
    before.get_mut(CONFIG).unwrap()["environment"]["inherit"] = json!(["MODE"]);
    let mut after = before.clone();
    after.get_mut(CONFIG).unwrap()["environment"]["inherit"] = json!([]);
    let (old, new) = (load(&before), load(&after));
    let mut endpoints = json!({"base":{"inherited":{"MODE":"old"},"environment":{"MODE":"old"}},
        "candidate":{"inherited":{},"environment":{}}});
    let environments = |endpoints: &Value| {
        let evidence = json!({"schema":"chrono-effective-inputs/v1",
            "identity":chrono_harness::wire::digest(endpoints).unwrap(), "endpoints":endpoints});
        chrono_judge_registration::inputs::effective_environments(&evidence, &old, &new)
    };
    let [base, candidate] = environments(&endpoints).unwrap().unwrap();
    assert_eq!(base["MODE"], "old");
    assert!(candidate.is_empty());
    endpoints["candidate"]["environment"]["UNKNOWN"] = json!("unrepresented");
    assert!(
        environments(&endpoints)
            .unwrap_err()
            .contains("candidate: effective environment differs")
    );
}

#[test]
fn workflow_views_bind_originals_and_preserve_retirement_failures_until_demanded() {
    use chrono_harness::wire::{Request, Status};
    use chrono_registration_fixture::{PROJECTS, WORKFLOW};
    let base = values();
    let mut current = base.clone();
    current.get_mut(PROJECTS).unwrap()["projects"] = json!([]);
    current.get_mut(WORKFLOW).unwrap()["retirements"] = json!([
        {"kind":"test","id":"t","replacement":"missing","reason":"exercise an unresolved replacement"}
    ]);
    let base_id = "1".repeat(40);
    let head_id = "2".repeat(40);
    let context = "a".repeat(64);
    let digest = chrono_harness::facts::registry_digest(&base, &current).unwrap();
    let mut request: Request = serde_json::from_value(json!({
        "protocol":"chrono-judge/v1","request_id":"","judge_id":"workflow","mode":"evaluate",
        "base":{"commit":base_id,"tree":base_id,"root":"."},
        "candidate":{"commit":head_id,"tree":head_id,"root":"."},
        "delta":[],"registries":{"base":"before","candidate":"after","digest":digest},
        "context":{"path":"context","sha256":context},"impact":{},"prior_results":[],
        "config_path":CONFIG,"checkout":{"head":head_id,"tracked":[],"untracked":[],"index_flags":[]},
        "runner":{"path":"runner","sha256":context,"version":"fixture"}
    })).unwrap();
    let mut previous = request.response(Status::Pass);
    previous.judge_id = "registration".into();
    previous.outputs.insert(
        "registration_view".into(),
        json!({
            "binding":{"base":base_id,"candidate":head_id,"registry":digest,"context":context},
            "base":base,"candidate":current,"historical":{}
        }),
    );
    request.prior_results.push(previous);
    request.seal().unwrap();
    let run = |request: &Request, replay| {
        let body = json!({"request":request,"replay":replay,
            "originals":[originals(&base),originals(&current)]});
        chrono_judge_registration::workflow_views::query(
            Path::new("."),
            body.to_string().as_bytes(),
        )
    };
    let full = run(&request, false).unwrap();
    assert_eq!(full["request_id"], request.request_id);
    assert_eq!(full["commits"], json!([base_id, head_id]));
    assert_eq!(full["changes"]["requests"]["Ok"]["test:t"], "test:missing");
    assert!(
        full["changes"]["removed_tests"]["test:t"]["Err"]
            .as_str()
            .unwrap()
            .contains("replacement missing/ambiguous")
    );
    assert_eq!(full["changes"]["removed_owners"][0]["id"], "p");
    assert!(
        full["changes"]["removed_owners"][0]["replacement"]["Err"]
            .as_str()
            .unwrap()
            .contains("missing/ambiguous project:p")
    );
    assert_eq!(full["effective_inputs"], json!({"Ok":null}));
    assert_eq!(run(&request, true).unwrap()["replayed"], true);
    request.scope = Some(chrono_harness::units::Scope::Unit {
        unit: "example".into(),
    });
    request.seal().unwrap();
    assert!(run(&request, false).unwrap()["changes"].is_null());
    request.judge_id = "another".into();
    assert!(run(&request, true).unwrap_err().contains("identity"));
}

#[test]
fn route_inputs_replay_the_checked_view_at_the_sealed_endpoints() {
    use chrono_harness::wire::Request;
    let docs = values();
    let registry = chrono_harness::facts::registry_digest(&docs, &docs).unwrap();
    let endpoint = json!({"commit":"1".repeat(40),"tree":"2".repeat(40),"root":"."});
    let context = "3".repeat(64);
    let mut request: Request=serde_json::from_value(json!({
        "request_id":"","protocol":"chrono-judge/v1","judge_id":"routes","mode":"evaluate",
        "base":endpoint,"candidate":endpoint,"delta":[],"registries":{"base":"before","candidate":"after","digest":registry},
        "context":{"path":"context","sha256":context},"impact":{},"prior_results":[],"config_path":CONFIG,
        "checkout":{"head":"1".repeat(40),"tracked":[],"untracked":[],"index_flags":[]},
        "runner":{"path":"runner","sha256":context,"version":"fixture"}
    })).unwrap();
    request.seal().unwrap();
    let view = json!({"binding":{"base":endpoint["commit"],"candidate":endpoint["commit"],"context":context,"registry":registry},
        "base":docs,"candidate":docs,"historical":{}});
    let mut body = json!({"request":request,"view":view,"selected":[]});
    let read = |q: &Value| {
        chrono_judge_registration::route_views::effective_inputs(q.to_string().as_bytes())
    };
    assert_eq!(
        read(&body).unwrap(),
        json!({"request_id":request.request_id,"effective_inputs":null})
    );
    for path in [".", "relative/checkout", "/original/checkout"] {
        let mut located = request.clone();
        located.candidate.root = path.into();
        located.seal().unwrap();
        body["request"] = json!(located);
        assert_eq!(read(&body).unwrap()["request_id"], located.request_id);
    }
    body["request"] = json!(request);
    body["view"]["binding"]["registry"] = json!("different");
    assert!(read(&body).unwrap_err().contains("binding mismatch"));
    body["view"] = view.clone();
    body["request"]["scope"] = json!({"kind":"collect","manifest":"reports.json"});
    assert!(read(&body).unwrap_err().contains("request identity"));
    request.scope = Some(chrono_harness::units::Scope::Collect {
        manifest: "reports.json".into(),
    });
    request.seal().unwrap();
    body["request"] = json!(request);
    assert!(
        read(&body)
            .unwrap_err()
            .contains("collection execution units missing")
    );
    let retirement = json!({"endpoints":{"mode":"view","view":view,"config":CONFIG},"judge":"routes","retired_tests":["test:t"]});
    let response = chrono_judge_registration::test_retirements::query(
        Path::new("."),
        retirement.to_string().as_bytes(),
    )
    .unwrap();
    assert_eq!(
        response["retirements"]["test:t"]["Err"],
        "E_RETIREMENT: test is not a removed baseline identity: test:t"
    );
}
