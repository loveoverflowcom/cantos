//! One request per process. This binary has no provider or filesystem adapter.

use std::{io, process::ExitCode};

use cantos_forge::interfaces::worker;

fn main() -> ExitCode {
    match worker::serve(io::stdin().lock(), io::stdout().lock()) {
        Ok(status) => ExitCode::from(status.code()),
        Err(_) => ExitCode::from(70),
    }
}
