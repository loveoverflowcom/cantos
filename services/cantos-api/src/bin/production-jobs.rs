//! Authenticated development CLI for the durable local-worker foundation.
use std::{env, path::PathBuf, process::ExitCode};

use cantos_api::{
    jobs::{ExecutionMode, RetryPolicy, RunEvent},
    postgres::{durable::DurableError, local_config, Store},
};
use serde::Serialize;

const USAGE: &str = "usage: production-jobs enqueue <script-id> <snapshot-id> <operation-id> (--local-espeak|--test-fake) | status <run-id> | (pause|resume|cancel|retry) <run-id> <operation-id> | resolve-not-executed <run-id> <attempt-id> <evidence-reference> | plan <script-id> <next-snapshot-id> (--local-espeak|--test-fake) [--previous <snapshot-id>] [--output-root <absolute-private-directory>]; requires CANTOS_ENV=development, CANTOS_DATABASE_URL and CANTOS_SESSION_TOKEN";

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
    /// Read-only: forecast which dialogue would be synthesized, reused or blocked.
    Plan {
        script: &'a str,
        next: &'a str,
        previous: Option<&'a str>,
        mode: ExecutionMode,
        output_root: Option<PathBuf>,
    },
}

fn report(value: &impl Serialize) -> Result<String, DurableError> {
    serde_json::to_string(value).map_err(|_| DurableError::InvalidRequest)
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
            match store
                .enqueue_job(&token, script, snapshot, operation, mode, policy)
                .await
            {
                Ok(summary) => report(&summary),
                Err(error) => Err(error),
            }
        }
        Command::Status { run } => match store.job_status(&token, run).await {
            Ok(summary) => report(&summary),
            Err(error) => Err(error),
        },
        Command::Control {
            run,
            operation,
            event,
        } => match store.control_job(&token, run, operation, event).await {
            Ok(summary) => report(&summary),
            Err(error) => Err(error),
        },
        Command::ResolveNotExecuted {
            run,
            attempt,
            evidence,
        } => match store
            .resolve_not_executed(&token, run, attempt, evidence)
            .await
        {
            Ok(summary) => report(&summary),
            Err(error) => Err(error),
        },
        Command::Plan {
            script,
            next,
            previous,
            mode,
            output_root,
        } => match store
            .regeneration_plan(&token, script, previous, next, mode, output_root.as_deref())
            .await
        {
            Ok(plan) => report(&plan),
            Err(error) => Err(error),
        },
    }
    .map_err(|error| format!("production job command rejected: {error:?}"))?;
    println!("{result}");
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
        ["plan", script, next, options @ ..] => parse_plan(script, next, options)?,
        _ => return Err(USAGE.into()),
    };
    Ok(command)
}

/// Exactly one execution mode, and each of `--previous` and `--output-root` at most once.
/// The output root is the only place object probes read, and it must be absolute.
fn parse_plan<'a>(
    script: &'a str,
    next: &'a str,
    options: &[&'a str],
) -> Result<Command<'a>, String> {
    let (mut mode, mut previous, mut output_root) = (None, None, None);
    let mut index = 0;
    while index < options.len() {
        match options[index] {
            "--local-espeak" if mode.is_none() => mode = Some(ExecutionMode::LocalEspeak),
            "--test-fake" if mode.is_none() => mode = Some(ExecutionMode::SyntheticFake),
            "--previous" if previous.is_none() => {
                index += 1;
                previous = Some(*options.get(index).ok_or(USAGE)?);
            }
            "--output-root" if output_root.is_none() => {
                index += 1;
                let path = PathBuf::from(*options.get(index).ok_or(USAGE)?);
                if !path.is_absolute() {
                    return Err("--output-root must be an absolute path".into());
                }
                output_root = Some(path);
            }
            _ => return Err(USAGE.into()),
        }
        index += 1;
    }
    Ok(Command::Plan {
        script,
        next,
        previous,
        mode: mode.ok_or(USAGE)?,
        output_root,
    })
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

    #[test]
    fn plan_requires_one_explicit_mode_and_an_absolute_output_root() {
        let parsed = parse_command(&[
            "plan",
            "script",
            "next",
            "--test-fake",
            "--previous",
            "old",
            "--output-root",
            "/private/root",
        ]);
        assert!(matches!(
            parsed,
            Ok(Command::Plan {
                previous: Some("old"),
                mode: ExecutionMode::SyntheticFake,
                output_root: Some(_),
                ..
            })
        ));
        assert!(matches!(
            parse_command(&["plan", "script", "next", "--local-espeak"]),
            Ok(Command::Plan {
                previous: None,
                output_root: None,
                mode: ExecutionMode::LocalEspeak,
                ..
            })
        ));
        for values in [
            vec!["plan", "script", "next"],
            vec!["plan", "script", "next", "--google"],
            vec!["plan", "script", "next", "--test-fake", "--local-espeak"],
            vec!["plan", "script", "next", "--test-fake", "--previous"],
            vec![
                "plan",
                "script",
                "next",
                "--test-fake",
                "--previous",
                "a",
                "--previous",
                "b",
            ],
            vec!["plan", "script", "next", "--test-fake", "--unknown"],
        ] {
            assert_eq!(
                parse_command(&values).err().as_deref(),
                Some(USAGE),
                "{values:?}"
            );
        }
        assert_eq!(
            parse_command(&["plan", "s", "n", "--test-fake", "--output-root", "relative"])
                .err()
                .as_deref(),
            Some("--output-root must be an absolute path")
        );
    }
}
