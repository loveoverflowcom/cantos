//! `cantos-identity migrate` initializes or upgrades the database and exits.
//! `cantos-identity serve` opens it in serve mode (never creating it) and listens on the
//! private Unix socket until SIGTERM or SIGINT. All configuration is in `CANTOS_IDENTITY_*`
//! environment variables; see the service README.

use std::{io::Write, process::ExitCode};

use cantos_identity::{
    config::{MigrateConfig, ServeConfig},
    server::{run_check, run_migrate, run_serve, Readiness, RunError},
};

fn lookup(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn fail(code: u8, message: &dyn std::fmt::Display) -> ExitCode {
    eprintln!("cantos-identity: {message}");
    ExitCode::from(code)
}

async fn stop_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let (Ok(mut term), Ok(mut int)) = (
        signal(SignalKind::terminate()),
        signal(SignalKind::interrupt()),
    ) else {
        // Without signal handlers the process cannot be drained: stop immediately.
        return;
    };
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
}

fn announce(readiness: Readiness) {
    let mail = if readiness.mail_configured {
        "configured"
    } else {
        "missing"
    };
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{{\"event\":\"ready\",\"mail\":\"{mail}\"}}");
    let _ = out.flush();
}

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let (Some(command), None) = (arguments.next(), arguments.next()) else {
        return fail(2, &"usage: cantos-identity <migrate|check|serve>");
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return fail(1, &"async runtime unavailable"),
    };
    match command.as_str() {
        "migrate" | "check" => {
            let config = match MigrateConfig::from_lookup(lookup) {
                Ok(config) => config,
                Err(error) => return fail(2, &error),
            };
            let result = if command == "check" {
                runtime.block_on(run_check(&config))
            } else {
                runtime.block_on(run_migrate(&config))
            };
            match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => fail(1, &error),
            }
        }
        "serve" => {
            let config = match ServeConfig::from_lookup(lookup) {
                Ok(config) => config,
                Err(error) => return fail(2, &error),
            };
            match runtime.block_on(run_serve(config, announce, stop_signal())) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error @ RunError::Configuration) => fail(2, &error),
                Err(error) => fail(1, &error),
            }
        }
        _ => fail(2, &"usage: cantos-identity <migrate|check|serve>"),
    }
}
