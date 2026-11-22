use std::io::{Read, Write};
fn read_request() -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > 64 * 1024 * 1024
    {
        return Err("E_PROTOCOL: request read/bound".into());
    }
    Ok(bytes)
}

fn run(args: &[String]) -> u8 {
    if args == ["--version"] {
        println!("chrono-judge-registration {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    let initial = args == ["--protocol", chrono_harness::initial::PROTOCOL];
    if !initial && args != ["--protocol", "chrono-judge/v1"] {
        chrono_harness::log_cli_error(&format!(
            "E_USAGE: execution-methods, test-owners or --protocol chrono-judge/v1 or chrono-initial-judge/v1"
        ));
        eprintln!(
            "E_USAGE: execution-methods, test-owners or --protocol chrono-judge/v1 or chrono-initial-judge/v1"
        );
        return 2;
    }
    let bytes = match read_request() {
        Ok(bytes) => bytes,
        Err(error) => {
            chrono_harness::log_cli_error(&error);
            eprintln!("{error}");
            return 2;
        }
    };
    let decoded = if initial {
        chrono_harness::decode(&bytes).map(|r| chrono_judge_registration::initial::judge(&r))
    } else {
        chrono_harness::decode(&bytes).map(|r| chrono_judge_registration::judge(&r))
    };
    let response = match decoded {
        Ok(r) => r,
        Err(e) => {
            chrono_harness::log_cli_error(&format!("E_PROTOCOL: {e}"));
            eprintln!("E_PROTOCOL: {e}");
            return 2;
        }
    };
    match chrono_harness::wire::canonical(&response) {
        Ok(bytes) => {
            if std::io::stdout().write_all(&bytes).is_err() {
                return 2;
            }
        }
        Err(e) => {
            chrono_harness::log_cli_error(&format!("E_PROTOCOL: {e}"));
            eprintln!("E_PROTOCOL: {e}");
            return 2;
        }
    }
    response.status.exit_code() as u8
}

fn main() -> std::process::ExitCode {
    use chrono_judge_registration::execution::ExecutionView;
    let mut args = std::env::args_os().skip(1);
    let operation = args.next();
    if args.next().is_none() {
        let operation = operation.as_deref().and_then(std::ffi::OsStr::to_str);
        if operation == Some("worktree-inputs-v1") {
            let result = chrono_judge_registration::worktree::query(std::io::stdin().lock())
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
            Some("execution-methods" | "test-owners" | "filemap-views" | "classification-views-v1")
        ) {
            let result = read_request().and_then(|bytes| {
                let mut out = std::io::stdout().lock();
                match operation {
                    Some("execution-methods") => ExecutionView::Methods.write(&bytes, &mut out)?,
                    Some("test-owners") => ExecutionView::TestOwners.write(&bytes, &mut out)?,
                    _ => {
                        let request = chrono_harness::decode(&bytes)?;
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
    std::process::ExitCode::from(chrono_harness::logged_text_main(
        "chrono-judge-registration",
        run,
    ))
}
