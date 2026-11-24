use std::io::{Read, Write};
fn read_request() -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take(chrono_judge_registration::PROJECTS_INPUT_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > chrono_judge_registration::PROJECTS_INPUT_LIMIT
    {
        return Err("E_PROTOCOL: request read/bound".into());
    }
    Ok(bytes)
}

fn run(args: &[String]) -> Result<u8, String> {
    if args == ["--version"] {
        println!("chrono-judge-registration {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    let initial = args
        == [
            "--protocol",
            chrono_judge_registration::wire::INITIAL_PROTOCOL,
        ];
    if !initial && args != ["--protocol", "chrono-judge/v1"] {
        return Err("E_USAGE: execution-methods, test-owners or --protocol chrono-judge/v1 or chrono-initial-judge/v1".into());
    }
    let bytes = read_request()?;
    let decoded = if initial {
        chrono_judge_registration::wire::decode(&bytes)
            .map(|r| chrono_judge_registration::initial::judge(&r))
    } else {
        chrono_judge_registration::wire::decode(&bytes)
            .map(|r| chrono_judge_registration::judge(&r))
    };
    let response = decoded.map_err(|e| format!("E_PROTOCOL: {e}"))?;
    let bytes = chrono_judge_registration::wire::canonical(&response)
        .map_err(|e| format!("E_PROTOCOL: {e}"))?;
    std::io::stdout()
        .write_all(&bytes)
        .map_err(|e| e.to_string())?;
    Ok(response.status.exit_code() as u8)
}

fn protocol_cli() -> Result<u8, String> {
    use serde_json::{Value, json};
    use std::{
        io::{BufRead, BufReader},
        path::PathBuf,
        process::{Command, Stdio},
    };

    let argv: Vec<_> = std::env::args_os().collect();
    let args = argv
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            arg.to_str()
                .map(str::to_owned)
                .ok_or((index, arg.as_encoded_bytes()))
        })
        .collect::<Result<Vec<_>, _>>();
    let root = argv
        .iter()
        .skip(1)
        .zip(argv.iter().skip(2))
        .find(|(key, _)| *key == "--host-root")
        .map(|(_, value)| PathBuf::from(value))
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| e.to_string())?;
    let policy = root.join(".chrono-harness/logging.json");
    if let Ok(args) = &args {
        if policy
            .symlink_metadata()
            .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            return run(&args[1..]);
        }
    }
    let operation = argv.get(1).and_then(|arg| arg.to_str()).unwrap_or("");
    let mut child = Command::new("chrono-diagnostics")
        .arg("cli-events-v1")
        .arg(&root)
        .args(["chrono-judge-registration", operation])
        .arg(std::process::id().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("chrono-diagnostics: {e}"))?;
    let mut channel = child.stdin.take().unwrap();
    let mut reply = BufReader::new(child.stdout.take().unwrap());
    let business = (|| -> Result<u8, String> {
        let mut line = String::new();
        reply.read_line(&mut line).map_err(|e| e.to_string())?;
        if serde_json::from_str::<Value>(&line).map_err(|e| e.to_string())? != json!({"ready":true})
        {
            return Err("chrono-diagnostics: invalid ready reply".into());
        }
        let mut send = |event: Value| -> Result<(), String> {
            serde_json::to_writer(&mut channel, &event).map_err(|e| e.to_string())?;
            channel
                .write_all(b"\n")
                .and_then(|()| channel.flush())
                .map_err(|e| e.to_string())
        };
        let exit = match args {
            Err((index, bytes)) => {
                send(json!({"invalid_argument":{"argv_index":index,"bytes":bytes}}))?;
                2
            }
            Ok(args) => match run(&args[1..]) {
                Ok(exit) => exit,
                Err(message) => {
                    eprintln!("{message}");
                    send(json!({"diagnostic":message}))?;
                    2
                }
            },
        };
        send(json!({"exit_code":exit}))?;
        Ok(exit)
    })();
    drop(channel);
    let result = child.wait().map_err(|e| e.to_string())?;
    let exit = business?;
    if result.code() == Some(i32::from(exit)) || result.code() == Some(2) {
        Ok(result.code().unwrap() as u8)
    } else {
        Err(format!(
            "chrono-diagnostics exited {result} after business exit {exit}"
        ))
    }
}

fn main() -> std::process::ExitCode {
    use chrono_judge_registration::execution::ExecutionView;
    let mut args = std::env::args_os().skip(1);
    let operation = args.next();
    let argument = args.next();
    if operation.as_deref() == Some(std::ffi::OsStr::new("classification-inputs-v1")) {
        let result = (|| {
            let selector = argument
                .filter(|_| args.next().is_none())
                .ok_or("E_USAGE: classification-inputs-v1 SELECTOR")?;
            let request = chrono_judge_registration::wire::decode(&read_request()?)?;
            let output = chrono_judge_registration::classification_inputs::query(
                &request,
                std::path::Path::new(&selector),
            );
            let mut out = std::io::stdout().lock();
            serde_json::to_writer(&mut out, &output).map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())
        })();
        return match result {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                2.into()
            }
        };
    }
    if argument.is_none() {
        let operation = operation.as_deref().and_then(std::ffi::OsStr::to_str);
        if matches!(
            operation,
            Some(
                "worktree-inputs-v1"
                    | "capture-plan-v1"
                    | "input-profile-v1"
                    | "collection-inputs-v1"
                    | "project-views-v1"
                    | "project-evidence-v1"
                    | "check-project-registry-v1"
                    | "check-documents-v1"
                    | "ci-transition-v1"
                    | "route-views-v1"
                    | "route-inputs-v1"
                    | "execution-declarations-v1"
                    | "configuration-edges-v1"
                    | "scoped-execution-v1"
                    | "scheduling-tests-v1"
                    | "scoped-startup-v1"
                    | "test-retirements-v1"
                    | "impact-views-v1"
                    | "workflow-views-v1"
            )
        ) {
            let input = std::io::stdin().lock();
            let result = match operation {
                Some("check-documents-v1") => {
                    chrono_judge_registration::check_documents(input).map(serde_json::Value::String)
                }
                Some("project-views-v1") => chrono_judge_registration::project_views::views(input),
                Some("check-project-registry-v1") => {
                    chrono_judge_registration::project_views::check_projects(input)
                }
                Some("project-evidence-v1") => std::env::current_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|root| {
                        chrono_judge_registration::project_views::evidence(&root, input)
                    }),
                Some("ci-transition-v1") => std::env::current_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|root| chrono_judge_registration::ci_transition::query(&root, input)),
                Some("route-views-v1") => chrono_judge_registration::route_views::views(input),
                Some("route-inputs-v1") => {
                    chrono_judge_registration::route_views::effective_inputs(input)
                }
                Some("configuration-edges-v1") => {
                    chrono_judge_registration::query_configuration_edges(input)
                }
                Some("scoped-startup-v1") => {
                    chrono_judge_registration::execution::scoped_startup(input)
                }
                Some("scheduling-tests-v1") => {
                    chrono_judge_registration::execution::scheduling_tests(input)
                        .map(|tests| serde_json::json!(tests))
                }
                Some("scoped-execution-v1") => {
                    chrono_judge_registration::execution::scoped_execution(input)
                }
                Some("execution-declarations-v1") => {
                    chrono_judge_registration::execution::plan_declarations(input)
                }
                Some("test-retirements-v1") => std::env::current_dir()
                    .map_err(|error| error.to_string())
                    .and_then(|root| {
                        chrono_judge_registration::test_retirements::query(&root, input)
                    }),
                Some("impact-views-v1") => std::env::current_dir()
                    .map_err(|error| error.to_string())
                    .and_then(|root| chrono_judge_registration::impact_views::query(&root, input)),
                Some("workflow-views-v1") => std::env::current_dir()
                    .map_err(|error| error.to_string())
                    .and_then(|root| {
                        chrono_judge_registration::workflow_views::query(&root, input)
                    }),
                Some("input-profile-v1") => chrono_judge_registration::inputs::input_profile(input),
                Some("capture-plan-v1") => chrono_judge_registration::inputs::capture_plan(input),
                Some("collection-inputs-v1") => {
                    chrono_judge_registration::snapshot_queries::composition(input)
                }
                _ => chrono_judge_registration::worktree::query(input),
            }
            .and_then(|reply| {
                let mut out = std::io::stdout().lock();
                serde_json::to_writer(&mut out, &reply).map_err(|e| e.to_string())?;
                out.flush().map_err(|e| e.to_string())
            });
            return match result {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    2.into()
                }
            };
        }
        if matches!(
            operation,
            Some(
                "execution-methods"
                    | "test-owners"
                    | "test-bindings-v1"
                    | "scoped-test-bindings-v1"
                    | "filemap-views"
                    | "classification-views-v1"
            )
        ) {
            let result = read_request().and_then(|bytes| {
                let mut out = std::io::stdout().lock();
                match operation {
                    Some("execution-methods") => ExecutionView::Methods.write(&bytes, &mut out)?,
                    Some("test-bindings-v1") => {
                        ExecutionView::TestBindings.write(&bytes, &mut out)?
                    }
                    Some("scoped-test-bindings-v1") => {
                        ExecutionView::ScopedTestBindings.write(&bytes, &mut out)?
                    }
                    Some("test-owners") => ExecutionView::TestOwners.write(&bytes, &mut out)?,
                    _ => {
                        let request = chrono_judge_registration::wire::decode(&bytes)?;
                        let views = if operation == Some("classification-views-v1") {
                            chrono_judge_registration::classification_views(&request)
                        } else {
                            chrono_judge_registration::filemap_views(&request)
                        };
                        serde_json::to_writer(&mut out, &views)
                            .map_err(|error| error.to_string())?;
                    }
                }
                out.flush().map_err(|error| error.to_string())
            });
            return match result {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    2.into()
                }
            };
        }
    }
    protocol_cli()
        .unwrap_or_else(|message| {
            eprintln!("{message}");
            2
        })
        .into()
}
