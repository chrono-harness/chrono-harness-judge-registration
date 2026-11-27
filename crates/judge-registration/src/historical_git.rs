//! Fixed Git inputs used while interpreting historical registries.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// Preserve the operation's outcome and publish the observation produced by a successful close.
pub(crate) fn complete_read<T, O, P, E, F>(
    action: core::result::Result<T, E>, closed: core::result::Result<O, E>, publish: F,
) -> (out: (core::result::Result<T, E>, Option<P>))
    where F: FnOnce(O) -> Option<P>,
    requires forall|observation: O| closed == Ok::<O, E>(observation) ==> #[trigger] publish.requires((observation,)),
    ensures match closed {
        Ok(observation) => out.0 == action && publish.ensures((observation,), out.1),
        Err(close_error) => out.1.is_none() && out.0 == match action {
            Err(action_error) => Err::<T, E>(action_error),
            Ok(_) => Err::<T, E>(close_error),
        },
    },
{
    match closed {
        Ok(observation) => (action, publish(observation)),
        Err(close_error) => match action {
            Err(action_error) => (Err(action_error), None),
            Ok(_) => (Err(close_error), None),
        },
    }
}
}

#[cfg(not(verus_keep_ghost))]
use crate::runner_session::Stream;
#[cfg(not(verus_keep_ghost))]
use crate::wire::{Checkout, Delta, Request, Response};
#[cfg(not(verus_keep_ghost))]
use serde::de::DeserializeOwned;
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::{cell::RefCell, collections::BTreeMap, path::Path};

#[cfg(not(verus_keep_ghost))]
type Result<T> = std::result::Result<T, String>;

#[cfg(not(verus_keep_ghost))]
pub(crate) struct InstalledGit<'a> {
    root: &'a Path,
    base: Option<&'a str>,
    candidate: &'a str,
    config: &'a str,
    stream: RefCell<Stream>,
}
#[cfg(not(verus_keep_ghost))]
#[derive(serde::Deserialize)]
pub(crate) struct Entry {
    pub(crate) kind: String,
    pub(crate) mode: String,
}
#[cfg(not(verus_keep_ghost))]
#[derive(serde::Deserialize)]
pub(crate) struct Inventory {
    pub(crate) entries: BTreeMap<String, Entry>,
    pub(crate) delta: Option<Vec<Delta>>,
}

#[cfg(not(verus_keep_ghost))]
impl<'a> InstalledGit<'a> {
    pub(crate) fn open(request: &'a Request) -> Result<Self> {
        request.validate()?;
        Self::start(
            &request.candidate.root,
            Some(&request.base.commit),
            &request.candidate.commit,
            &request.config_path,
            Some(!request.observations["git_facts"].is_null()),
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
            Some(!observations["git_facts"].is_null()),
            Some(root),
            json!({"mode":"observed","config":config,"base":base,"candidate":candidate,"observations":observations}),
        )
    }
    pub(crate) fn history(root: &'a Path, base: &'a str, candidate: &'a str) -> Result<Self> {
        Self::start(
            root,
            Some(base),
            candidate,
            "",
            Some(false),
            Some(root),
            json!({"mode":"history"}),
        )
    }
    pub(crate) fn configured(
        root: &'a Path,
        base: &'a str,
        candidate: &'a str,
        config: &'a str,
    ) -> Result<Self> {
        Self::start(
            root,
            Some(base),
            candidate,
            config,
            None,
            Some(root),
            json!({"mode":"config","config":config,"base":base,"candidate":candidate}),
        )
    }
    fn start(
        root: &'a Path,
        base: Option<&'a str>,
        candidate: &'a str,
        config: &'a str,
        expected_bound: Option<bool>,
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
        if expected_bound.is_some_and(|expected| expected != open.bound) {
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
        complete_read(result, reader.finish(), |observation| {
            if observation.is_null() {
                None
            } else {
                Some(observation)
            }
        })
    }
    pub(crate) fn inventory(&self) -> Result<Inventory> {
        self.query(json!({"op":"candidate-inventory"}))
    }
    pub(crate) fn snapshot(&self, endpoint: &str) -> Result<crate::facts::RegistrySnapshot> {
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
#[cfg(not(verus_keep_ghost))]
impl InstalledGit<'_> {
    pub(crate) fn blob(&self, root: &Path, commit: &str, path: &str) -> Result<Vec<u8>> {
        self.endpoint(root, commit)?;
        self.stream
            .borrow_mut()
            .exchange(&json!({"op":"blob-bytes","commit":commit,"path":path}))
    }
    pub(crate) fn checkout_excluding(
        &self,
        root: &Path,
        commit: &str,
        artifacts: &[String],
    ) -> Result<Checkout> {
        if self.endpoint(root, commit)? != "candidate" {
            return Err("expected candidate checkout".into());
        }
        self.stream
            .borrow_mut()
            .exchange(&json!({"op":"checkout-excluding","artifacts":artifacts}))
    }
}
