//! Fixed Git inputs used while interpreting historical registries.
use crate::runner_session::Stream;
use chrono_harness::{
    facts::Reader,
    wire::{Checkout, Delta, Request, Response},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, path::Path};

type Result<T> = std::result::Result<T, String>;

pub(crate) trait HistoricalGit {
    fn blob(&self, root: &Path, commit: &str, path: &str) -> Result<Vec<u8>>;
    fn checkout_excluding(&self, root: &Path, commit: &str, artifacts: &[&str])
    -> Result<Checkout>;
}

impl HistoricalGit for Reader {
    fn blob(&self, root: &Path, commit: &str, path: &str) -> Result<Vec<u8>> {
        Reader::blob(self, root, commit, path)
    }
    fn checkout_excluding(
        &self,
        root: &Path,
        commit: &str,
        artifacts: &[&str],
    ) -> Result<Checkout> {
        Reader::checkout_excluding(self, root, commit, artifacts)
    }
}

pub(crate) struct InstalledGit<'a> {
    root: &'a Path,
    base: Option<&'a str>,
    candidate: &'a str,
    config: &'a str,
    stream: RefCell<Stream>,
}
#[derive(serde::Deserialize)]
pub(crate) struct Entry {
    pub(crate) kind: String,
    pub(crate) mode: String,
}
#[derive(serde::Deserialize)]
pub(crate) struct Inventory {
    pub(crate) entries: BTreeMap<String, Entry>,
    pub(crate) delta: Option<Vec<Delta>>,
}

impl<'a> InstalledGit<'a> {
    pub(crate) fn open(request: &'a Request) -> Result<Self> {
        request.validate()?;
        Self::start(
            &request.candidate.root,
            Some(&request.base.commit),
            &request.candidate.commit,
            &request.config_path,
            &request.observations,
            None,
            json!({"mode":"request","request":request}),
        )
    }
    pub(crate) fn observed(
        root: &'a Path,
        base: Option<&'a str>,
        candidate: &'a str,
        config: &'a str,
        observations: &Value,
    ) -> Result<Self> {
        Self::start(
            root,
            base,
            candidate,
            config,
            observations,
            Some(root),
            json!({"mode":"observed","config":config,"base":base,"candidate":candidate,"observations":observations}),
        )
    }
    fn start(
        root: &'a Path,
        base: Option<&'a str>,
        candidate: &'a str,
        config: &'a str,
        observations: &Value,
        cwd: Option<&Path>,
        opening: Value,
    ) -> Result<Self> {
        let mut stream = Stream::start("git-facts-v1", cwd)?;
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Open {
            bound: bool,
        }
        let open: Open = stream.exchange(&opening)?;
        if open.bound == observations["git_facts"].is_null() {
            return Err("E_GIT_FACTS: session binding differs from request".into());
        }
        Ok(Self {
            root,
            base,
            candidate,
            config,
            stream: RefCell::new(stream),
        })
    }
    fn endpoint(&self, root: &Path, commit: &str) -> Result<&'static str> {
        if root != self.root {
            return Err("Git facts checkout differs from request".into());
        }
        if commit == self.candidate {
            return Ok("candidate");
        }
        if Some(commit) == self.base {
            return Ok("base");
        }
        Err("Git facts commit differs from request".into())
    }
    pub(crate) fn query<T: DeserializeOwned>(&self, op: Value) -> Result<T> {
        self.stream.borrow_mut().exchange(&op)
    }
    pub(crate) fn with_request<T>(
        request: &'a Request,
        action: impl FnOnce(&Self) -> Result<T>,
    ) -> (Result<T>, Option<Value>) {
        let reader = match Self::open(request) {
            Ok(reader) => reader,
            Err(error) => return (Err(error), None),
        };
        let result = action(&reader);
        match reader.finish() {
            Ok(observation) => (result, (!observation.is_null()).then_some(observation)),
            Err(error) => (result.and(Err(error)), None),
        }
    }
    pub(crate) fn inventory(&self) -> Result<Inventory> {
        self.query(json!({"op":"candidate-inventory"}))
    }
    pub(crate) fn snapshot(
        &self,
        endpoint: &str,
    ) -> Result<chrono_harness::facts::RegistrySnapshot> {
        #[derive(serde::Deserialize)]
        struct Snapshot {
            values: BTreeMap<String, Value>,
            originals: BTreeMap<String, String>,
            effective_path: String,
            selection: Option<Value>,
        }
        let row: Snapshot = self.query(json!({"op":"registry-snapshot","endpoint":endpoint}))?;
        let snapshot = crate::impact_views::snapshot(self.config, row.originals)?;
        if snapshot.values != row.values
            || snapshot.effective_path != row.effective_path
            || snapshot.selection != row.selection
        {
            return Err(
                "E_SNAPSHOT_IDENTITY: original registry and decoded snapshot differ".into(),
            );
        }
        Ok(snapshot)
    }
    pub(crate) fn record(self, response: &mut Response) -> Result<()> {
        let observation = self.finish()?;
        if !observation.is_null() {
            response.outputs.insert("git_facts".into(), observation);
        }
        Ok(())
    }
    pub(crate) fn finish(self) -> Result<Value> {
        let mut stream = self.stream.into_inner();
        let observation = stream.exchange(&json!({"op":"observation"}))?;
        stream.finish()?;
        Ok(observation)
    }
}
impl HistoricalGit for InstalledGit<'_> {
    fn blob(&self, root: &Path, commit: &str, path: &str) -> Result<Vec<u8>> {
        self.endpoint(root, commit)?;
        self.stream
            .borrow_mut()
            .exchange(&json!({"op":"blob-bytes","commit":commit,"path":path}))
    }
    fn checkout_excluding(
        &self,
        root: &Path,
        commit: &str,
        artifacts: &[&str],
    ) -> Result<Checkout> {
        if self.endpoint(root, commit)? != "candidate" {
            return Err("expected candidate checkout".into());
        }
        self.stream
            .borrow_mut()
            .exchange(&json!({"op":"checkout-excluding","artifacts":artifacts}))
    }
}
