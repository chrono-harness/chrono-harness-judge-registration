//! A single invocation of an installed runner JSON-lines protocol.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// The installed runner decodes one JSON header line followed by the unchanged stdin bytes.
pub(crate) fn command_input(mut header: Vec<u8>, input: &[u8]) -> (frame: Vec<u8>)
    ensures frame@ == header@ + seq![b'\n'] + input@,
{
    header.push(b'\n');
    header.extend_from_slice(input);
    header
}
}

#[cfg(not(verus_keep_ghost))]
use serde::{Serialize, de::DeserializeOwned};
#[cfg(not(verus_keep_ghost))]
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};
#[cfg(not(verus_keep_ghost))]
type Result<T> = std::result::Result<T, String>;

#[cfg(not(verus_keep_ghost))]
pub(crate) struct Stream {
    protocol: &'static str,
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}
#[cfg(not(verus_keep_ghost))]
impl Stream {
    pub(crate) fn with<T>(
        protocol: &'static str,
        cwd: Option<&Path>,
        action: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let mut stream = Self::start(protocol, cwd)?;
        let result = action(&mut stream);
        let finished = stream.finish();
        result.and_then(|value| finished.map(|()| value))
    }

    pub(crate) fn start(protocol: &'static str, cwd: Option<&Path>) -> Result<Self> {
        let mut command = Command::new("chrono-harness");
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let mut child = command
            .arg(protocol)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| format!("start chrono-harness {protocol}: {error}"))?;
        Ok(Self {
            protocol,
            input: child.stdin.take(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
        })
    }
    pub(crate) fn exchange<T: DeserializeOwned>(
        &mut self,
        operation: &impl Serialize,
    ) -> Result<T> {
        let input = self.input.as_mut().ok_or("closed runner session")?;
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
            return Err("E_PROTOCOL: incomplete runner response".into());
        }
        serde_json::from_slice::<Result<T>>(&line).map_err(|error| error.to_string())?
    }
    pub(crate) fn finish(&mut self) -> Result<()> {
        self.input.take();
        let trailing = std::io::copy(&mut self.output, &mut std::io::sink());
        let status = self.child.wait().map_err(|error| error.to_string())?;
        if trailing.map_err(|error| error.to_string())? != 0 {
            return Err("E_PROTOCOL: trailing runner response".into());
        }
        if status.success() {
            Ok(())
        } else {
            Err(format!("{} {status}", self.protocol))
        }
    }
}
#[cfg(not(verus_keep_ghost))]
impl Drop for Stream {
    fn drop(&mut self) {
        if self.input.is_some() {
            let _ = self.finish();
        }
    }
}

#[cfg(not(verus_keep_ghost))]
/// Invoke a one-request installed tool and collect the child's original outcome.
pub(crate) fn query<T: DeserializeOwned>(
    program: &str,
    operation: &str,
    cwd: Option<&Path>,
    request: &impl Serialize,
) -> Result<T> {
    let bytes = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    query_bytes(program, operation, cwd, &bytes)
}

#[cfg(not(verus_keep_ghost))]
fn query_bytes<T: DeserializeOwned>(
    program: &str,
    operation: &str,
    cwd: Option<&Path>,
    bytes: &[u8],
) -> Result<T> {
    let mut command = Command::new(program);
    if let Some(root) = cwd {
        command.current_dir(root);
    }
    let mut child = command
        .arg(operation)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start {program} {operation}: {error}"))?;
    let mut stdin = child.stdin.take().unwrap();
    let (output, written) = std::thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(bytes));
        let output = child.wait_with_output();
        (output, writer.join().expect("JSON input writer panicked"))
    });
    let output = output.map_err(|error| error.to_string())?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(if message.trim().is_empty() {
            format!("{program} {operation}: {}", output.status)
        } else {
            message.trim_end().to_owned()
        });
    }
    written.map_err(|error| error.to_string())?;
    serde_json::from_slice(&output.stdout).map_err(|e| format!("{program} {operation}: {e}"))
}

#[cfg(not(verus_keep_ghost))]
/// Ask the runner which launched the child for the complete original receipt.
pub(crate) fn record<T: DeserializeOwned>(
    root: &Path,
    program: &str,
    args: &[String],
    env: &std::collections::BTreeMap<String, String>,
    timeout_seconds: u64,
    output_limit_bytes: usize,
    digest: &str,
    input: &[u8],
) -> Result<T> {
    let header = serde_json::json!({"program":program,"args":args,"env":env,
        "timeout_seconds":timeout_seconds,"output_limit_bytes":output_limit_bytes,"expected_sha256":digest});
    let header = serde_json::to_vec(&header).map_err(|e| e.to_string())?;
    let bytes = command_input(header, input);
    query_bytes("chrono-harness", "record-command-v1", Some(root), &bytes)
}
