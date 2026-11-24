//! Fixed Git inputs used while interpreting historical registries.
use chrono_harness::{
    facts::Reader,
    wire::{Checkout, Request},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

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
    request: &'a Request,
    stream: RefCell<Stream>,
}
struct Stream {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}
impl Stream {
    fn exchange<T: DeserializeOwned>(&mut self, operation: &impl Serialize) -> Result<T> {
        let input = self.input.as_mut().ok_or("closed Git facts session")?;
        serde_json::to_writer(&mut *input, operation).map_err(|error| error.to_string())?;
        input
            .write_all(b"\n")
            .and_then(|()| input.flush())
            .map_err(|error| error.to_string())?;
        let mut line = Vec::new();
        self.output
            .read_until(b'\n', &mut line)
            .map_err(|error| error.to_string())?;
        if line.pop() != Some(b'\n') {
            return Err("E_PROTOCOL: incomplete Git facts response".into());
        }
        serde_json::from_slice::<Result<T>>(&line).map_err(|error| error.to_string())?
    }
    fn finish(&mut self) -> Result<()> {
        self.input.take();
        let trailing = std::io::copy(&mut self.output, &mut std::io::sink());
        let status = self.child.wait().map_err(|error| error.to_string())?;
        if trailing.map_err(|error| error.to_string())? != 0 {
            return Err("E_PROTOCOL: trailing Git facts response".into());
        }
        if status.success() {
            Ok(())
        } else {
            Err(format!("git-facts-v1 {status}"))
        }
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        if self.input.is_some() {
            let _ = self.finish();
        }
    }
}
impl<'a> InstalledGit<'a> {
    pub(crate) fn open(request: &'a Request) -> Result<Self> {
        let mut child = Command::new("chrono-harness")
            .arg("git-facts-v1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| format!("start chrono-harness git-facts-v1: {error}"))?;
        let mut stream = Stream {
            input: child.stdin.take(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
        };
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Open {
            bound: bool,
        }
        let open: Open = stream.exchange(&json!({"mode":"request","request":request}))?;
        if open.bound == request.observations["git_facts"].is_null() {
            return Err("E_GIT_FACTS: session binding differs from request".into());
        }
        Ok(Self {
            request,
            stream: RefCell::new(stream),
        })
    }
    fn endpoint(&self, root: &Path, commit: &str) -> Result<&'static str> {
        if root != self.request.candidate.root {
            return Err("Git facts checkout differs from request".into());
        }
        if commit == self.request.candidate.commit {
            return Ok("candidate");
        }
        if commit == self.request.base.commit {
            return Ok("base");
        }
        Err("Git facts commit differs from request".into())
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
