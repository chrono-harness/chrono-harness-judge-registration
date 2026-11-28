//! Immutable execution declarations. This module neither selects nor launches work.
#[cfg(not(verus_keep_ghost))]
use crate::verified_scheduling;
#[cfg(verus_keep_ghost)]
#[path = "verified_scheduling.rs"]
pub mod verified_scheduling;
use vstd::hash_set::StringHashSet;
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(not(verus_keep_ghost))]
use serde::{Deserialize, Serialize};
#[cfg(not(verus_keep_ghost))]
use serde_json::Value;
#[cfg(not(verus_keep_ghost))]
use std::collections::{BTreeMap, BTreeSet};

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub operations: Vec<String>,
    #[serde(deserialize_with = "crate::execution::declared_timeout")]
    pub timeout_seconds: u64,
    pub output_limit_bytes: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub operation_bounds: BTreeMap<String, Bounds>,
}

pub use self::verified_scheduling::Bounds;

/// FILEMAP can persist a wide timeout as a canonical decimal string so its
/// original bytes also work with JCS-based registry identity producers.
#[cfg(not(verus_keep_ghost))]
pub(crate) fn declared_timeout<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Timeout {
        Seconds(u64),
        Decimal(String),
    }
    match Timeout::deserialize(d)? {
        Timeout::Seconds(seconds) => Ok(seconds),
        Timeout::Decimal(text) => text
            .parse::<u64>()
            .ok()
            .filter(|n| verified_scheduling::decimal_timeout(*n) && n.to_string() == text)
            .ok_or_else(|| serde::de::Error::custom("invalid exact timeout")),
    }
}

#[cfg(not(verus_keep_ghost))]
impl Plan {
    pub fn validate(&self) -> Result<(), String> {
        let global = Bounds {
            timeout_seconds: self.timeout_seconds,
            output_limit_bytes: self.output_limit_bytes,
        };
        if !verified_scheduling::valid_plan(
            &self.operations,
            global,
            &self.operation_bounds.iter().collect::<Vec<_>>(),
            crate::PROJECTS_INPUT_LIMIT,
        ) {
            return Err("E_EXECUTION_PLAN: invalid operations or bounds".into());
        }
        Ok(())
    }
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    pub resources: Vec<String>,
    pub outputs: Vec<String>,
}
#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scheduling {
    pub max_running: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority: Vec<String>,
    pub resources: Vec<String>,
    pub claims: BTreeMap<String, Claims>,
}
#[cfg(not(verus_keep_ghost))]
impl Scheduling {
    pub fn validate(&self) -> Result<(), String> {
        let unique = |items: &[String]| {
            items.iter().all(|v| !v.is_empty() && !v.contains('\0'))
                && items.iter().collect::<BTreeSet<_>>().len() == items.len()
        };
        if self.max_running == 0 || !unique(&self.resources) {
            return Err(
                "E_SCHEDULING: positive max_running and unique resource names required".into(),
            );
        }
        if !unique(&self.priority) || self.priority.iter().any(|id| !self.claims.contains_key(id)) {
            return Err(
                "E_SCHEDULING: priority must list distinct registered operation IDs".into(),
            );
        }
        for (op, claims) in &self.claims {
            if op.is_empty()
                || !unique(&claims.resources)
                || !unique(&claims.outputs)
                || claims
                    .outputs
                    .iter()
                    .map(|p| std::path::Path::new(p))
                    .collect::<BTreeSet<_>>()
                    .len()
                    != claims.outputs.len()
                || claims.resources.iter().any(|r| !self.resources.contains(r))
                || claims
                    .outputs
                    .iter()
                    .any(|p| crate::schema::relative_path(p.trim_end_matches('/')).is_err())
            {
                return Err(format!("E_SCHEDULING: invalid claims for {op}"));
            }
        }
        Ok(())
    }
    pub fn conflicts(&self, a: &str, b: &str) -> bool {
        let a = &self.claims[a];
        let b = &self.claims[b];
        a.resources.iter().any(|r| b.resources.contains(r))
            || a.outputs.iter().any(|x| {
                b.outputs.iter().any(|y| {
                    std::path::Path::new(x).starts_with(y) || std::path::Path::new(y).starts_with(x)
                })
            })
    }
}
verus! {

pub open spec fn claim_named(claims: Seq<&String>, name: Seq<char>) -> bool {
    exists|j: int| 0 <= j < claims.len() && #[trigger] claims[j]@ == name
}

/// Build the lookup from every declared name. Operations from different plans may repeat.
pub fn claims_cover_operations(operations: &[&String], claims: &[&String]) -> (covered: bool)
    ensures covered == (forall|i: int| 0 <= i < operations.len() ==>
        claim_named(claims@, #[trigger] operations@[i]@)),
{
    let mut registered = StringHashSet::new();
    let mut index = 0;
    while index < claims.len()
        invariant
            registered.well_formed(), 0 <= index <= claims.len(),
            forall|name: Seq<char>| #[trigger] registered@.contains(name)
                <==> exists|j: int| 0 <= j < index && #[trigger] claims@[j]@ == name,
        decreases claims.len() - index,
    {
        registered.insert(claims[index].clone());
        index += 1;
    }
    let mut pos = 0;
    while pos < operations.len()
        invariant
            registered.well_formed(), 0 <= pos <= operations.len(),
            forall|name: Seq<char>| #[trigger] registered@.contains(name)
                <==> exists|j: int| 0 <= j < claims.len() && #[trigger] claims@[j]@ == name,
            forall|i: int| 0 <= i < pos ==> #[trigger] registered@.contains(operations@[i]@),
        decreases operations.len() - pos,
    {
        if !registered.contains(operations[pos].as_str()) {
            assert(!exists|j: int| 0 <= j < claims.len() && #[trigger] claims@[j]@ == operations@[pos as int]@);
            return false;
        }
        pos += 1;
    }
    assert forall|i: int| 0 <= i < operations.len() implies
        claim_named(claims@, #[trigger] operations@[i]@) by {
        assert(registered@.contains(operations@[i]@));
    }
    true
}

}

/// Interpret the complete host policy before tool observation or operation effects.
#[cfg(not(verus_keep_ghost))]
pub fn scheduling(
    filemap: &Value,
    projects: &Value,
    artifacts: &Value,
) -> Result<Option<Scheduling>, String> {
    let Some(value) = filemap.get("execution_scheduling") else {
        return Ok(None);
    };
    let policy: Scheduling =
        serde_json::from_value(value.clone()).map_err(|e| format!("E_SCHEDULING: {e}"))?;
    policy.validate()?;
    let plans = plans(filemap)?;
    let participating: Vec<_> = plans.values().flat_map(|plan| &plan.operations).collect();
    let claims: Vec<_> = policy.claims.keys().collect();
    if !claims_cover_operations(&participating, &claims) {
        return Err(
            "E_SCHEDULING: claims must name every participating operation, including empty claims"
                .into(),
        );
    }
    let methods = methods(projects)?;
    for (operation, claims) in &policy.claims {
        if methods.get(operation).is_none_or(|m| m.len() != 1) {
            return Err(format!(
                "E_SCHEDULING: unknown or ambiguous operation {operation}"
            ));
        }
        for output in &claims.outputs {
            let matches: Vec<_> = artifacts
                .as_array()
                .ok_or("E_SCHEDULING: artifact declarations missing")?
                .iter()
                .filter(|a| {
                    a["path"]
                        .as_str()
                        .is_some_and(|p| std::path::Path::new(p) == std::path::Path::new(output))
                })
                .collect();
            if matches.len() != 1 || matches[0]["path"] != *output {
                return Err(format!(
                    "E_SCHEDULING: output must name one registered artifact path: {operation}: {output}"
                ));
            }
        }
    }
    Ok(Some(policy))
}
#[cfg(not(verus_keep_ghost))]
pub fn plans(filemap: &Value) -> Result<BTreeMap<String, Plan>, String> {
    let plans: BTreeMap<String, Plan> = serde_json::from_value(filemap["execution_plans"].clone())
        .map_err(|e| format!("E_EXECUTION_PLAN: {e}"))?;
    for (test, plan) in &plans {
        if !test.starts_with("test:") || test.len() == 5 {
            return Err(format!("E_EXECUTION_PLAN: invalid plan {test}"));
        }
        plan.validate().map_err(|e| format!("{e}: {test}"))?;
    }
    Ok(plans)
}
#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Method {
    pub owner: String,
    pub operation: String,
    pub tool: String,
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<Value>,
}
/// A test identity is opaque; its owner and terminal action are explicit registration.
#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TestBinding {
    pub owner: String,
    pub action: String,
    pub operation: String,
}

verus! {
pub fn default_test_action(
    execute: bool, project: bool, test_kind: bool, kind_missing: bool,
    tests_for_declared: bool, test_script_missing: bool,
) -> (bind: bool)
    ensures bind == (execute && if project {
        test_kind || kind_missing
    } else {
        tests_for_declared || test_script_missing
    }),
{
    execute && if project {
        test_kind || kind_missing
    } else {
        tests_for_declared || test_script_missing
    }
}

pub fn explicit_groups_allowed(project: bool, test_kind: bool) -> (allowed: bool)
    ensures allowed == (project && test_kind),
{
    project && test_kind
}

pub fn includes_default_group(count: usize, legacy: bool) -> (valid: bool)
    ensures valid == (count != 0 && legacy),
{
    count != 0 && legacy
}

pub fn explicit_action_key(key: Option<&str>) -> (action: Result<&str, ()>)
    ensures match key {
        Some(text) => action == Ok(text),
        None => action == Err(()),
    },
{
    key.ok_or(())
}

pub enum TestActionError {
    EmptyOrRepeated,
    Missing,
}

pub fn test_action<T>(test: &str, key: &str, new_key: bool, action: Option<T>)
    -> (result: Result<T, TestActionError>)
    ensures match result {
        Ok(value) => test@.len() != 0 && key@.len() != 0 && new_key && action == Some(value),
        Err(TestActionError::EmptyOrRepeated) => test@.len() == 0 || key@.len() == 0 || !new_key,
        Err(TestActionError::Missing) => test@.len() != 0 && key@.len() != 0 && new_key && action is None,
    },
{
    if test.is_empty() || key.is_empty() || !new_key {
        Err(TestActionError::EmptyOrRepeated)
    } else {
        action.ok_or(TestActionError::Missing)
    }
}

pub fn duplicate_test_identity(existing: bool, explicit: bool, earlier_explicit: bool)
    -> (reject: bool)
    ensures reject == (existing && (explicit || earlier_explicit)),
{
    existing && (explicit || earlier_explicit)
}

pub fn append_declaration<T>(values: &mut Vec<T>, declaration: T)
    ensures final(values)@ == old(values)@.push(declaration),
{
    values.push(declaration);
}
}

/// Shared by full and scoped consumers. Legacy scripts retain their single execute alias.
#[cfg(not(verus_keep_ghost))]
pub fn test_bindings(projects: &Value) -> Result<BTreeMap<String, Vec<TestBinding>>, String> {
    let mut out: BTreeMap<String, Vec<TestBinding>> = BTreeMap::new();
    let mut explicit = BTreeSet::new();
    for (collection, prefix) in [("projects", "project"), ("scripts", "script")] {
        for row in projects[collection]
            .as_array()
            .ok_or("projects collection missing")?
        {
            let id = row["id"].as_str().ok_or("project ID missing")?;
            let owner = format!("{prefix}:{id}");
            let actions = row["actions"].as_object().ok_or("actions missing")?;
            let project = collection == "projects";
            let test_kind = row["kind"] == "test";
            let is_explicit = row.get("test_groups").is_some();
            let groups = if let Some(groups) = row.get("test_groups") {
                if !explicit_groups_allowed(project, test_kind) {
                    return Err(format!(
                        "E_TEST_BINDING: test_groups requires a test project: {owner}"
                    ));
                }
                let groups = groups
                    .as_object()
                    .ok_or("E_TEST_BINDING: test_groups must be a mapping")?;
                if !includes_default_group(groups.len(), groups.contains_key(id)) {
                    return Err(format!(
                        "E_TEST_BINDING: test_groups must include legacy identity {id}"
                    ));
                }
                groups
                    .iter()
                    .map(|(test, action)| {
                        Ok((
                            test.as_str(),
                            explicit_action_key(action.as_str())
                                .map_err(|()| "E_TEST_BINDING: action key must be a string")?,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?
            } else if default_test_action(
                actions.contains_key("execute"),
                project,
                test_kind,
                row.get("kind").is_none(),
                row.get("tests_for").is_some(),
                row.get("test_script").is_none(),
            ) {
                vec![(id, "execute")]
            } else {
                vec![]
            };
            let mut selected = BTreeSet::new();
            for (test, key) in groups {
                let action = test_action(test, key, selected.insert(key), actions.get(key))
                    .map_err(|error| match error {
                        TestActionError::EmptyOrRepeated => {
                            format!("E_TEST_BINDING: empty or repeated action binding: {owner}")
                        }
                        TestActionError::Missing => {
                            format!("E_TEST_BINDING: {owner} has no action {key}")
                        }
                    })?;
                let operation = action["operation"]
                    .as_str()
                    .ok_or("E_TEST_BINDING: operation missing")?;
                let binding = TestBinding {
                    owner: owner.clone(),
                    action: key.into(),
                    operation: operation.into(),
                };
                let identity = format!("test:{test}");
                if duplicate_test_identity(
                    out.contains_key(&identity),
                    is_explicit,
                    explicit.contains(&identity),
                ) {
                    return Err(format!(
                        "E_TEST_BINDING: duplicate test identity {identity}"
                    ));
                }
                if is_explicit {
                    explicit.insert(identity.clone());
                }
                // Legacy ambiguity stays visible for DELTA-local adjudication, never a winner.
                append_declaration(out.entry(identity).or_default(), binding);
            }
        }
    }
    Ok(out)
}
/// Retain duplicates for the routes policy to reject; never overwrite a method.
#[cfg(not(verus_keep_ghost))]
pub fn methods(projects: &Value) -> Result<BTreeMap<String, Vec<Method>>, String> {
    let mut out: BTreeMap<String, Vec<Method>> = BTreeMap::new();
    for (key, prefix) in [("projects", "project"), ("scripts", "script")] {
        for row in projects[key]
            .as_array()
            .ok_or("projects collection missing")?
        {
            for action in row["actions"]
                .as_object()
                .ok_or("actions missing")?
                .values()
            {
                let operation = action["operation"].as_str().ok_or("operation missing")?;
                crate::schema::output_contract(&action["output_contract"])?;
                let method = Method {
                    owner: format!("{prefix}:{}", row["id"].as_str().ok_or("owner missing")?),
                    operation: operation.into(),
                    tool: action["tool"].as_str().ok_or("tool missing")?.into(),
                    argv: serde_json::from_value(action["argv"].clone())
                        .map_err(|e| e.to_string())?,
                    output_contract: serde_json::from_value(action["output_contract"].clone())
                        .map_err(|e| e.to_string())?,
                };
                append_declaration(out.entry(operation.into()).or_default(), method);
            }
        }
    }
    Ok(out)
}

#[cfg(not(verus_keep_ghost))]
fn scoped_bindings(projects: &Value) -> Result<BTreeMap<String, Vec<TestBinding>>, String> {
    if projects["schema_version"] == 2 {
        crate::schema::project_bindings(projects)
    } else {
        test_bindings(projects)
    }
}

/// Check the execution declarations from one scoped endpoint before returning its bindings.
/// The caller already owns the original plans, scheduling policy and reference declarations.
#[cfg(not(verus_keep_ghost))]
pub fn scoped_execution(input: impl std::io::Read) -> Result<Value, String> {
    scoped_endpoint(input, false)
}

#[cfg(not(verus_keep_ghost))]
pub fn scoped_startup(input: impl std::io::Read) -> Result<Value, String> {
    scoped_endpoint(input, true)
}

#[cfg(not(verus_keep_ghost))]
fn scoped_endpoint(input: impl std::io::Read, startup: bool) -> Result<Value, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Endpoint {
        #[serde(deserialize_with = "crate::wire::strict_value")]
        filemap: Value,
        projects: Box<serde_json::value::RawValue>,
        #[serde(deserialize_with = "crate::wire::strict_value")]
        artifacts: Value,
    }
    let Endpoint {
        filemap,
        projects,
        artifacts,
    } = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    // The existing bindings interface limits the project document alone.
    if projects.get().len() > crate::PROJECTS_INPUT_LIMIT {
        return Err("E_PROTOCOL: request read/bound".into());
    }
    let projects = crate::wire::json(projects.get().as_bytes())?;
    if [&filemap, &projects]
        .iter()
        .any(|doc| !matches!(doc["schema_version"].as_u64(), Some(1 | 2)))
    {
        return Err("unsupported registry version".into());
    }
    let bindings = scoped_bindings(&projects)?;
    // Scheduling validates its participating plans against the operation and artifact registrations.
    if scheduling(&filemap, &projects, &artifacts)?.is_none() && filemap["schema_version"] == 2 {
        plans(&filemap)?;
    }
    if let Some(declaration) = filemap.get("reference_syntax") {
        crate::validate_reference_syntax(declaration)?;
    }
    if !startup {
        return serde_json::to_value(bindings).map_err(|error| error.to_string());
    }
    // Consumption errors stay deferred until CI needs the corresponding declaration.
    let methods = methods(&projects).map(|methods| {
        methods
            .into_iter()
            .map(|(id, rows)| {
                let entries: Vec<_> = rows
                    .into_iter()
                    .map(|method| {
                        serde_json::json!({
                            "owner":method.owner, "tool":method.tool, "argv":method.argv
                        })
                    })
                    .collect();
                (id, entries)
            })
            .collect::<BTreeMap<_, _>>()
    });
    let operations = plans(&filemap).map(|plans| {
        plans
            .into_iter()
            .map(|(test, plan)| (test, plan.operations))
            .collect::<BTreeMap<_, _>>()
    });
    let owners = test_bindings(&projects).map(|tests| {
        tests
            .into_iter()
            .map(|(test, rows)| {
                (
                    test,
                    rows.into_iter().map(|row| row.owner).collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>()
    });
    Ok(
        serde_json::json!({"bindings":bindings,"startup":{"methods":methods,"plans":operations,"owners":owners}}),
    )
}

/// Read-only views of execution registrations consumed by release recipes and previews.
#[cfg(not(verus_keep_ghost))]
pub enum ExecutionView {
    Methods,
    TestOwners,
    TestBindings,
    ScopedTestBindings,
}

#[cfg(not(verus_keep_ghost))]
impl ExecutionView {
    pub fn write(&self, input: &[u8], mut output: impl std::io::Write) -> Result<(), String> {
        let projects = crate::wire::json(input)?;
        match self {
            Self::Methods => serde_json::to_writer(&mut output, &methods(&projects)?),
            Self::TestBindings => serde_json::to_writer(&mut output, &test_bindings(&projects)?),
            Self::ScopedTestBindings => {
                serde_json::to_writer(&mut output, &scoped_bindings(&projects)?)
            }
            Self::TestOwners => {
                let mut tests_by_owner: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
                for (test, bindings) in test_bindings(&projects)? {
                    for binding in bindings {
                        tests_by_owner
                            .entry(binding.owner)
                            .or_default()
                            .insert(test.clone());
                    }
                }
                serde_json::to_writer(&mut output, &tests_by_owner)
            }
        }
        .map_err(|error| error.to_string())?;
        output.flush().map_err(|error| error.to_string())
    }
}

/// Interpret the declarations a route planner consumes before constructing its operation graph.
#[cfg(not(verus_keep_ghost))]
pub fn plan_declarations(input: impl std::io::Read) -> Result<Value, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Query {
        filemap: Value,
        projects: Value,
    }
    let query: Query = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    let plans = plans(&query.filemap)?;
    // The caller must check unit ownership after decoding plans and before applying methods.
    Ok(serde_json::json!({"plans":plans, "methods":methods(&query.projects)}))
}

verus! {
/// The adapter observes registered resource names and native output path components.
pub trait ClaimConflicts {
    spec fn overlap(&self, left: &String, right: &String) -> bool;
    fn overlaps(&self, left: &String, right: &String) -> (conflict: bool)
        ensures conflict == self.overlap(left, right);
}

pub open spec fn affected_claim<P: ClaimConflicts>(
    op: &String, changed: &String, before: &P, after: &P,
) -> bool {
    before.overlap(op, changed) || after.overlap(op, changed)
}

pub open spec fn affected_operation<P: ClaimConflicts>(
    op: &String, changed: Seq<&String>, before: &P, after: &P,
) -> bool {
    exists|j: int| 0 <= j < changed.len() && affected_claim(op, #[trigger] changed[j], before, after)
}

fn operation_changed<P: ClaimConflicts>(
    op: &String, changed: &[&String], before: &P, after: &P,
) -> (found: bool)
    ensures found == affected_operation(op, changed@, before, after),
{
    for j in 0..changed.len()
        invariant forall|k: int| 0 <= k < j ==> !affected_claim(op, #[trigger] changed@[k], before, after),
    {
        let other = changed[j];
        if before.overlaps(op, other) || after.overlaps(op, other) {
            assert(affected_claim(op, changed@[j as int], before, after));
            return true;
        }
    }
    false
}

/// Each endpoint's plan can be affected by claims observed at either endpoint.
pub fn affected_plan<P: ClaimConflicts>(
    global: bool, direct: bool, operations: &[String], changed: &[&String], before: &P, after: &P,
) -> (selected: bool)
    ensures selected == (global || direct || exists|i: int| 0 <= i < operations.len()
        && affected_operation(&#[trigger] operations@[i], changed@, before, after)),
{
    if global || direct { return true; }
    for i in 0..operations.len()
        invariant forall|k: int| 0 <= k < i ==>
            !affected_operation(&#[trigger] operations@[k], changed@, before, after),
    {
        if operation_changed(&operations[i], changed, before, after) {
            assert(affected_operation(&operations@[i as int], changed@, before, after));
            return true;
        }
    }
    false
}
}

#[cfg(not(verus_keep_ghost))]
impl ClaimConflicts for Option<&Scheduling> {
    fn overlaps(&self, left: &String, right: &String) -> bool {
        self.is_some_and(|policy| {
            policy.claims.contains_key(left)
                && policy.claims.contains_key(right)
                && policy.conflicts(left, right)
        })
    }
}

/// Select tests whose admitted scheduling declarations differ across fixed endpoints.
#[cfg(not(verus_keep_ghost))]
pub fn scheduling_tests(input: impl std::io::Read) -> Result<BTreeSet<String>, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Endpoint {
        scheduling: Option<Scheduling>,
        operations: BTreeMap<String, Vec<String>>,
    }
    let [old, new]: [Endpoint; 2] = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    fn global(policy: &Scheduling) -> (usize, &[String], &[String]) {
        (policy.max_running, &policy.resources, &policy.priority)
    }
    let all = old.scheduling.as_ref().map(global) != new.scheduling.as_ref().map(global);
    let mut selected = BTreeSet::new();
    let before = old.scheduling.as_ref();
    let after = new.scheduling.as_ref();
    let empty = BTreeMap::new();
    let old_claims = before.map(|p| &p.claims).unwrap_or(&empty);
    let new_claims = after.map(|p| &p.claims).unwrap_or(&empty);
    let changed: BTreeSet<_> = if all {
        BTreeSet::new()
    } else {
        old_claims
            .keys()
            .chain(new_claims.keys())
            .filter(|op| old_claims.get(*op) != new_claims.get(*op))
            .collect()
    };
    if all || !changed.is_empty() {
        let names: Vec<_> = changed.iter().copied().collect();
        for (test, operations) in old.operations.iter().chain(&new.operations) {
            let direct = !all && operations.iter().any(|op| changed.contains(op));
            if affected_plan(all, direct, operations, &names, &before, &after) {
                selected.insert(test.clone());
            }
        }
    }
    Ok(selected)
}
