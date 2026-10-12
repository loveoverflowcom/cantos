//! Explicit local development worker process; commercial and neural dispatch are unavailable.
use std::{env, path::PathBuf, process::ExitCode};

use cantos_api::{
    contexts::synthesis::{
        jobs::LeaseDuration,
        worker::{run_worker, Failpoint, WorkerConfig, WorkerProvider},
    },
    platform::postgres::{local_config, Store},
};

const USAGE: &str = "usage: cantos-worker --output-root <absolute-private-directory> --worker-id <id> [--once] [--lease-seconds <1..300>] (--local-espeak <absolute-runtime> | --test-fake [--failpoint <after_claim|before_reuse|after_intent|after_provider|after_observation|before_accept|after_accept>]); requires CANTOS_ENV=development and CANTOS_DATABASE_URL";

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
    let config = parse_config(args)?;
    if env::var("CANTOS_ENV").ok().as_deref() != Some("development") {
        return Err("worker supports CANTOS_ENV=development only".into());
    }
    let url = env::var("CANTOS_DATABASE_URL").map_err(|_| "CANTOS_DATABASE_URL is required")?;
    let database = local_config(&url).map_err(|_| "invalid local PostgreSQL configuration")?;
    let store = Store::new(database).map_err(|_| "database pool unavailable")?;
    run_worker(store, config)
        .await
        .map_err(|error| error.to_string())
}

fn parse_config(args: &[String]) -> Result<WorkerConfig, String> {
    let mut output_root = None;
    let mut worker_id = None;
    let mut provider = None;
    let mut once = false;
    let mut seconds = 30;
    let mut failpoint = None;
    let mut offset = 0;
    while let Some(argument) = args.get(offset).map(String::as_str) {
        match argument {
            "--output-root" if output_root.is_none() => {
                output_root = Some(PathBuf::from(value(args, &mut offset)?));
            }
            "--worker-id" if worker_id.is_none() => {
                worker_id = Some(value(args, &mut offset)?.to_owned());
            }
            "--once" if !once => once = true,
            "--lease-seconds" => {
                seconds = value(args, &mut offset)?.parse().map_err(|_| USAGE)?;
            }
            "--local-espeak" if provider.is_none() => {
                provider = Some(WorkerProvider::LocalEspeak {
                    runtime: PathBuf::from(value(args, &mut offset)?),
                });
            }
            "--test-fake" if provider.is_none() => provider = Some(WorkerProvider::SyntheticFake),
            "--failpoint" if failpoint.is_none() => {
                failpoint = Some(Failpoint::parse(value(args, &mut offset)?).ok_or(USAGE)?);
            }
            _ => return Err(USAGE.into()),
        }
        offset += 1;
    }
    let provider = provider
        .ok_or("provider execution disabled; explicitly select --local-espeak or --test-fake")?;
    if failpoint.is_some() && !matches!(provider, WorkerProvider::SyntheticFake) {
        return Err("failpoints require the explicit --test-fake provider".into());
    }
    let lease_duration = LeaseDuration::new(seconds).map_err(|_| "invalid lease duration")?;
    Ok(WorkerConfig {
        provider,
        output_root: output_root.ok_or(USAGE)?,
        worker_id: worker_id.ok_or(USAGE)?,
        lease_duration,
        once,
        failpoint,
    })
}

fn value<'a>(args: &'a [String], offset: &mut usize) -> Result<&'a str, String> {
    *offset += 1;
    args.get(*offset)
        .map(String::as_str)
        .ok_or_else(|| USAGE.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn dispatch_requires_explicit_provider_and_failpoints_require_fake() {
        let base = ["--output-root", "/tmp/private", "--worker-id", "worker-01"];
        assert_eq!(
            parse_config(&arguments(&base)).err().as_deref(),
            Some("provider execution disabled; explicitly select --local-espeak or --test-fake")
        );
        let mut local = base.to_vec();
        local.extend([
            "--local-espeak",
            "/opt/local/espeak-ng",
            "--failpoint",
            "after_claim",
        ]);
        assert_eq!(
            parse_config(&arguments(&local)).err().as_deref(),
            Some("failpoints require the explicit --test-fake provider")
        );
        let mut fake = base.to_vec();
        fake.extend(["--test-fake", "--failpoint", "after_intent", "--once"]);
        let parsed = parse_config(&arguments(&fake));
        assert!(matches!(
            parsed,
            Ok(WorkerConfig {
                provider: WorkerProvider::SyntheticFake,
                failpoint: Some(Failpoint::AfterIntent),
                once: true,
                ..
            })
        ));
    }
}
