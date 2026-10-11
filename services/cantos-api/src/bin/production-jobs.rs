//! Authenticated development CLI for the durable local-worker foundation.
use std::{env, process::ExitCode};

use cantos_api::{
    jobs::{ExecutionMode, RetryPolicy, RunEvent},
    postgres::{local_config, Store},
};

const USAGE: &str = "usage: production-jobs enqueue <script-id> <snapshot-id> <operation-id> (--local-espeak|--test-fake) | status <run-id> | (pause|resume|cancel|retry) <run-id> <operation-id> | resolve-not-executed <run-id> <attempt-id> <evidence-reference>; requires CANTOS_ENV=development, CANTOS_DATABASE_URL and CANTOS_SESSION_TOKEN";

enum Command<'a> {
    Enqueue {
        script: &'a str,
        snapshot: &'a str,
        operation: &'a str,
        mode: ExecutionMode,
    },
    Status {
        run: &'a str,
    },
    Control {
        run: &'a str,
        operation: &'a str,
        event: RunEvent,
    },
    ResolveNotExecuted {
        run: &'a str,
        attempt: &'a str,
        evidence: &'a str,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.as_slice() == ["--help"] || args.as_slice() == ["-h"] {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match execute(&args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

async fn execute(args: &[String]) -> Result<(), String> {
    let values: Vec<_> = args.iter().map(String::as_str).collect();
    let command = parse_command(&values)?;
    if env::var("CANTOS_ENV").ok().as_deref() != Some("development") {
        return Err("production jobs support CANTOS_ENV=development only".into());
    }
    let url = env::var("CANTOS_DATABASE_URL").map_err(|_| "CANTOS_DATABASE_URL is required")?;
    let config = local_config(&url).map_err(|_| "invalid local PostgreSQL configuration")?;
    let store = Store::new(config).map_err(|_| "database pool unavailable")?;
    let token = env::var("CANTOS_SESSION_TOKEN").map_err(|_| "CANTOS_SESSION_TOKEN is required")?;
    // Token and database URL are never argv fields or included in reports.
    let result = match command {
        Command::Enqueue {
            script,
            snapshot,
            operation,
            mode,
        } => {
            let policy = RetryPolicy::new(3, 1, 30).map_err(|_| "invalid retry policy")?;
            store
                .enqueue_job(&token, script, snapshot, operation, mode, policy)
                .await
        }
        Command::Status { run } => store.job_status(&token, run).await,
        Command::Control {
            run,
            operation,
            event,
        } => store.control_job(&token, run, operation, event).await,
        Command::ResolveNotExecuted {
            run,
            attempt,
            evidence,
        } => {
            store
                .resolve_not_executed(&token, run, attempt, evidence)
                .await
        }
    }
    .map_err(|error| format!("production job command rejected: {error:?}"))?;
    let report = serde_json::to_string(&result).map_err(|_| "job report cannot be serialized")?;
    println!("{report}");
    Ok(())
}

fn parse_command<'a>(args: &[&'a str]) -> Result<Command<'a>, String> {
    let command = match args {
        ["enqueue", script, snapshot, operation, "--local-espeak"] => Command::Enqueue {
            script,
            snapshot,
            operation,
            mode: ExecutionMode::LocalEspeak,
        },
        ["enqueue", script, snapshot, operation, "--test-fake"] => Command::Enqueue {
            script,
            snapshot,
            operation,
            mode: ExecutionMode::SyntheticFake,
        },
        ["status", run] => Command::Status { run },
        [action @ ("pause" | "resume" | "cancel" | "retry"), run, operation] => Command::Control {
            run,
            operation,
            event: match *action {
                "pause" => RunEvent::Pause,
                "resume" => RunEvent::Resume,
                "cancel" => RunEvent::Cancel,
                "retry" => RunEvent::Retry,
                _ => return Err(USAGE.into()),
            },
        },
        ["resolve-not-executed", run, attempt, evidence] => Command::ResolveNotExecuted {
            run,
            attempt,
            evidence,
        },
        _ => return Err(USAGE.into()),
    };
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_implicit_or_commercial_provider_can_be_enqueued() {
        assert!(matches!(
            parse_command(&["enqueue", "script", "snapshot", "operation", "--test-fake"]),
            Ok(Command::Enqueue {
                mode: ExecutionMode::SyntheticFake,
                ..
            })
        ));
        for values in [
            vec!["enqueue", "script", "snapshot", "operation"],
            vec!["enqueue", "script", "snapshot", "operation", "--google"],
            vec!["enqueue", "script", "snapshot", "operation", "--vieneu"],
        ] {
            assert_eq!(parse_command(&values).err().as_deref(), Some(USAGE));
        }
    }
}
