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
