//! `cantos-forge` binary: the imperative shell around `interfaces::cli::run`.

use std::{
    io::{self, Write},
    process::ExitCode,
};

use cantos_forge::interfaces::cli::{self, ExitStatus, Response};

fn main() -> ExitCode {
    let response = cli::run(std::env::args_os());
    match write_response(&response) {
        Ok(()) => ExitCode::from(response.status.code()),
        // stdout is gone (for example a closed pipe); the result was not delivered.
        Err(_) => ExitCode::from(ExitStatus::Failure.code()),
    }
}

fn write_response(response: &Response) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(response.stdout.as_bytes())?;
    stdout.flush()?;
    let mut stderr = io::stderr().lock();
    stderr.write_all(response.stderr.as_bytes())?;
    stderr.flush()
}
