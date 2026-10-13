//! The non-interactive command line: arguments in, one response out.
//!
//! `run` parses, executes against the given [`Runtime`] and renders; `main` supplies the
//! filesystem runtime and writes the response. Results go to stdout, diagnostics to stderr,
//! and `--json` output is one envelope per invocation with a stable schema name.

use std::ffi::OsString;

use clap::{error::ErrorKind, Parser};

use crate::application::ports::Runtime;

use super::{args::Cli, commands, reply::Reply};

pub use super::{
    render::COMMANDS,
    reply::{ExitStatus, Response, SCHEMA},
};

pub fn run<I, T, R>(args: I, runtime: &R) -> Response
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    R: Runtime,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    match Cli::try_parse_from(&args) {
        Ok(cli) => commands::execute(cli.command, runtime).render(cli.json),
        Err(error) => match error.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => Response {
                stdout: error.render().to_string(),
                stderr: String::new(),
                status: ExitStatus::Success,
            },
            // A usage error is reported as JSON when the raw arguments asked for JSON, so that
            // a caller parsing stdout never has to scrape a help message.
            _ => Reply::Usage {
                message: error.render().to_string(),
            }
            .render(requests_json(&args)),
        },
    }
}

fn requests_json(args: &[OsString]) -> bool {
    args.iter().skip(1).any(|arg| arg == "--json")
}
