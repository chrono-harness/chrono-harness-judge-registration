use chrono_judge_registration::Registrations;
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub const CONFIG: &str = ".chrono-harness/config.json";
pub const FM: &str = ".chrono-harness/FILEMAP.json";
pub const PROJECTS: &str = ".chrono-harness/projects.json";
pub const JUDGES: &str = ".chrono-harness/judges.json";
pub const WORKFLOW: &str = ".chrono-harness/workflow.json";
pub type Values = BTreeMap<String, Value>;
pub fn file(path: &str, edges: Value) -> Value {
    json!({"path":path,"owner":"host","surface":"product","cost":"unknown","edges":edges})
}
pub fn edge(from: &str, kind: &str, to: &str) -> Value {
    json!({"from":from,"kind":kind,"to":to})
}
pub fn project(id: &str, kind: &str, pair: &str) -> Value {
    let mut p = json!({"id":id,"kind":kind,"manifest":format!("{id}/Cargo.toml"),"lockfile":format!("{id}/Cargo.lock"),"root":id,"actions":{"execute":{"operation":format!("execute.{id}"),"tool":"sh","argv":["-c","exit 0"]}}});
    p[if kind == "production" {
        "test_project"
    } else {
        "tests_for"
    }] = pair.into();
    p
}
pub fn values() -> Values {
    let mut files = vec![
        file("src.bin", json!([{"kind":"compile","to":"project:p"}])),
        file("doc.txt", json!([])),
    ];
    for p in [
        CONFIG,
        FM,
        PROJECTS,
        JUDGES,
        WORKFLOW,
        ".gitignore",
        "p/Cargo.toml",
        "p/Cargo.lock",
        "t/Cargo.toml",
        "t/Cargo.lock",
    ] {
        files.push(file(p, json!([])));
    }
    BTreeMap::from([
        (
            CONFIG.into(),
            json!({"schema_version":1,"status":"active","enforcement":"enabled","runner":{"path":".chrono-harness/bin/chrono-harness","version":"0.1.0","sha256":null},"registries":{"filemap":FM,"projects":PROJECTS,"judges":JUDGES,"workflow":WORKFLOW},"canonical_check":{"operation":"validate.delta","argv":[".chrono-harness/bin/chrono-harness","check","--config",CONFIG,"--base","{base}","--candidate","{candidate}","--context",".chrono-harness/state/context.json"]},"tools":[{"id":"sh","program":"/bin/sh","resolution":"PATH-once","version_argv":["--version"],"expected_version":"fixture-declared"}],"environment":{"inherit":["PATH"],"values":{},"inputs":[]},"input_closure":{"status":"declared-complete","unresolved":[]},"protocol":{"id":"chrono-judge/v1","timeout_seconds":30,"stdout_limit_bytes":8388608,"encoding":"UTF-8"},"semantic_fields":[],"artifacts":[{"path":".chrono-harness/bin/","owner":"host","kind":"executable","tracked":false},{"path":".chrono-harness/state/","owner":"host","kind":"evidence","tracked":false}]}),
        ),
        (
            FM.into(),
            json!({"schema_version":1,"status":"active","files":files,"project_edges":[edge("project:p","test-execution","test:t")],"cost_models":{"unknown":{"cpu_ms":null,"wall_ms":null,"peak_rss_bytes":null,"io_bytes":null,"basis":"unmeasured fixture"}},"test_costs":[{"test":"t","cost":"unknown"}]}),
        ),
        (
            PROJECTS.into(),
            json!({"schema_version":1,"status":"active","owners":["host","p","t"],"projects":[project("p","production","t"),project("t","test","p")],"scripts":[]}),
        ),
        (
            JUDGES.into(),
            json!({"schema_version":1,"status":"active","migration_validator":"registration","judges":[{"id":"registration","executable":".chrono-harness/bin/chrono-judge-registration","version":"0.1.1","sha256":null,"argv":["--protocol","chrono-judge/v1"],"selector":"every-delta","after":[],"modes":["evaluate"]},{"id":"filemap","executable":".chrono-harness/bin/chrono-judge-filemap","version":"0.1.0","sha256":null,"argv":["--protocol","chrono-judge/v1"],"selector":"every-delta","after":["registration"],"modes":["evaluate"]}]}),
        ),
        (
            WORKFLOW.into(),
            json!({"schema_version":1,"status":"active","target_branch":"dev","feature_prefix":"feature/","integration_prefix":"integration/","staleness":{"max_behind_commits":3,"max_age_hours":24,"combine":"any-exceeded"},"stability":[],"semantic_changes_require_integration":true,"mixed_change":{"level":"warning","code":"W_MIXED","requires_acknowledgement":false},"integration":{"tests":[],"bind":[],"evidence":".chrono-harness/state/integration.json"},"retirements":[],"migrations":[]}),
        ),
    ])
}
pub fn load(v: &Values) -> Registrations {
    Registrations::load(v, CONFIG).unwrap()
}
pub fn script_pair(v: &mut Values, test_id: &str) {
    v.get_mut(PROJECTS).unwrap()["scripts"] = json!([
        {"id":"s","path":"s.sh","test_script":test_id,"actions":{"execute":{"operation":"script.s","tool":"sh","argv":["s.sh"]}}},
        {"id":test_id,"path":"st.sh","tests_for":"s","actions":{"execute":{"operation":"script.st","tool":"sh","argv":["st.sh"]}}}
    ]);
    let owners = v.get_mut(PROJECTS).unwrap()["owners"]
        .as_array_mut()
        .unwrap();
    owners.retain(|o| o != "st" && o != "st2");
    for id in ["s", test_id] {
        if !owners.contains(&json!(id)) {
            owners.push(json!(id));
        }
    }
    let fm = v.get_mut(FM).unwrap();
    let files = fm["files"].as_array_mut().unwrap();
    for path in ["s.sh", "st.sh"] {
        if !files.iter().any(|f| f["path"] == path) {
            files.push(file(path, json!([])));
        }
    }
    let edges = fm["project_edges"].as_array_mut().unwrap();
    edges.retain(|e| e["from"] != "script:s");
    edges.push(edge(
        "script:s",
        "test-execution",
        &format!("test:{test_id}"),
    ));
    fm["test_costs"] = json!([{"test":"t","cost":"unknown"}]);
    if test_id != "t" {
        fm["test_costs"]
            .as_array_mut()
            .unwrap()
            .push(json!({"test":test_id,"cost":"unknown"}));
    }
}
