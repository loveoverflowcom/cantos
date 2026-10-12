//! Authenticated development CLI for the durable local-worker foundation.
use std::{env, path::PathBuf, process::ExitCode, time::Duration};

use cantos_api::{
    incremental::ReadModel,
    jobs::{ExecutionMode, RetryPolicy, RunEvent},
    postgres::{durable::DurableError, local_config, Store},
};
use serde::Serialize;

const USAGE: &str = "usage: production-jobs enqueue <script-id> <snapshot-id> <operation-id> (--local-espeak|--test-fake) | status <run-id> | watch <run-id> [--samples <1-10000>] [--interval-ms <10-60000>] | (pause|resume|cancel|retry) <run-id> <operation-id> | resolve-not-executed <run-id> <attempt-id> <evidence-reference> | plan <script-id> <next-snapshot-id> (--local-espeak|--test-fake) [--previous <snapshot-id>] [--output-root <absolute-private-directory>]; requires CANTOS_ENV=development, CANTOS_DATABASE_URL and CANTOS_SESSION_TOKEN; optional CANTOS_JOB_SUMMARY_READ_MODEL=postgres|dbsp-shadow|dbsp-serve (DBSP needs the dbsp-experimental build)";

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
    /// Re-reads one run's summary on an interval: the long-lived read-model consumer.
    Watch {
        run: &'a str,
        samples: u32,
        interval: Duration,
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
    let read_model = match env::var(ReadModel::VARIABLE) {
        Ok(value) => value.parse::<ReadModel>().map_err(|_| {
            format!(
                "{} must be postgres, dbsp-shadow or dbsp-serve",
                ReadModel::VARIABLE
            )
        })?,
        Err(_) => ReadModel::Postgres,
    };
    let url = env::var("CANTOS_DATABASE_URL").map_err(|_| "CANTOS_DATABASE_URL is required")?;
    let config = local_config(&url).map_err(|_| "invalid local PostgreSQL configuration")?;
    let store = Store::new(config.clone()).map_err(|_| "database pool unavailable")?;
    let (store, view) = attach_read_model(store, config, read_model).await?;
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
        Command::Watch {
            run,
            samples,
            interval,
        } => {
            let mut last = Ok(String::new());
            for sample in 0..samples {
                if sample > 0 {
                    tokio::time::sleep(interval).await;
                }
                last = store
                    .job_status(&token, run)
                    .await
                    .and_then(|summary| report(&summary));
                match &last {
                    // Every sample but the last is printed here; the last follows below.
                    Ok(line) if sample + 1 < samples => println!("{line}"),
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            last
        }
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
    .map_err(|error| format!("production job command rejected: {error:?}"));
    // Counters only: no identifiers, counts, token or database URL.
    if let Some(status) = view_status(view.as_ref()) {
        eprintln!("job-summary-view: {status}");
    }
    println!("{}", result?);
    Ok(())
}

#[cfg(feature = "dbsp-experimental")]
type View = std::sync::Arc<cantos_api::postgres::durable::summary_view::SummaryView>;
#[cfg(not(feature = "dbsp-experimental"))]
type View = std::convert::Infallible;

/// Starts and attaches the experimental view for a DBSP read model. A build without the
/// feature refuses it rather than silently answering from PostgreSQL.
#[cfg(feature = "dbsp-experimental")]
async fn attach_read_model(
    store: Store,
    config: tokio_postgres::Config,
    model: ReadModel,
) -> Result<(Store, Option<View>), String> {
    use cantos_api::postgres::durable::summary_view::{AttachedView, SummaryView, ViewConfig};
    if !model.uses_view() {
        return Ok((store, None));
    }
    let view = std::sync::Arc::new(
        SummaryView::start(config, ViewConfig::default())
            .map_err(|_| "job summary view could not start")?,
    );
    // A view that is not ready in time is still attached: reads fall back to PostgreSQL.
    view.wait_ready(Duration::from_secs(10)).await;
    let attached = AttachedView::new(view.clone(), model, 4, Duration::from_secs(2));
    Ok((
        store.with_job_summary_view(std::sync::Arc::new(attached)),
        Some(view),
    ))
}

#[cfg(not(feature = "dbsp-experimental"))]
async fn attach_read_model(
    store: Store,
    _config: tokio_postgres::Config,
    model: ReadModel,
) -> Result<(Store, Option<View>), String> {
    if model.uses_view() {
        return Err(format!(
            "{} requires a build with the dbsp-experimental feature",
            model.as_str()
        ));
    }
    Ok((store, None))
}

#[cfg(feature = "dbsp-experimental")]
fn view_status(view: Option<&View>) -> Option<String> {
    view.and_then(|view| serde_json::to_string(&view.status()).ok())
}

#[cfg(not(feature = "dbsp-experimental"))]
fn view_status(view: Option<&View>) -> Option<String> {
    view.map(|never| match *never {})
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
        ["watch", run, options @ ..] => parse_watch(run, options)?,
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

/// Bounded sampling: each option at most once, within its documented range.
fn parse_watch<'a>(run: &'a str, options: &[&'a str]) -> Result<Command<'a>, String> {
    let (mut samples, mut interval) = (None, None);
    let mut index = 0;
    while index < options.len() {
        let value = options.get(index + 1).ok_or(USAGE)?;
        match options[index] {
            "--samples" if samples.is_none() => {
                samples = Some(
                    value
                        .parse::<u32>()
                        .ok()
                        .filter(|n| (1..=10_000).contains(n))
                        .ok_or(USAGE)?,
                );
            }
            "--interval-ms" if interval.is_none() => {
                interval = Some(
                    value
                        .parse::<u64>()
                        .ok()
                        .filter(|n| (10..=60_000).contains(n))
                        .ok_or(USAGE)?,
                );
            }
            _ => return Err(USAGE.into()),
        }
        index += 2;
    }
    Ok(Command::Watch {
        run,
        samples: samples.unwrap_or(10),
        interval: Duration::from_millis(interval.unwrap_or(500)),
    })
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
    fn watch_samples_and_interval_are_bounded_and_given_at_most_once() {
        assert!(matches!(
            parse_command(&["watch", "run"]),
            Ok(Command::Watch {
                run: "run",
                samples: 10,
                interval
            }) if interval == Duration::from_millis(500)
        ));
        assert!(matches!(
            parse_command(&["watch", "run", "--interval-ms", "10", "--samples", "10000"]),
            Ok(Command::Watch {
                samples: 10_000,
                interval,
                ..
            }) if interval == Duration::from_millis(10)
        ));
        for values in [
            vec!["watch"],
            vec!["watch", "run", "--samples"],
            vec!["watch", "run", "--samples", "0"],
            vec!["watch", "run", "--samples", "10001"],
            vec!["watch", "run", "--interval-ms", "9"],
            vec!["watch", "run", "--interval-ms", "60001"],
            vec!["watch", "run", "--samples", "2", "--samples", "3"],
            vec!["watch", "run", "--follow"],
        ] {
            assert_eq!(
                parse_command(&values).err().as_deref(),
                Some(USAGE),
                "{values:?}"
            );
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
