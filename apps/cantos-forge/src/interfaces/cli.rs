//! The non-interactive command line: arguments in, one response out.
//!
//! `run` performs no I/O; `main` writes the response. Results go to stdout, diagnostics to
//! stderr, and `--json` output is one envelope per invocation with a stable schema name.

use std::ffi::OsString;

use clap::{error::ErrorKind, Parser, Subcommand};
use serde::Serialize;

/// Version of the JSON envelope. A breaking change to any field needs a new name.
pub const SCHEMA: &str = "cantos-forge.cli/v1";

/// Every implemented command. `capabilities` reports exactly this list.
const COMMANDS: &[&str] = &["capabilities"];
const UNSUPPORTED: &str = "unsupported";

#[derive(Debug, Parser)]
#[command(
    name = "cantos-forge",
    version,
    disable_help_subcommand = true,
    about = "Producer-side Cantos Forge. Runs outside the Cantos API; this build only reports its capabilities."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Report what this build implements. Anything not listed is unsupported.
    Capabilities {
        /// Write one JSON envelope to stdout.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitStatus {
    Success,
    /// The command ran and failed, or its result could not be written.
    Failure,
    /// The arguments were invalid; nothing ran.
    Usage,
}

impl ExitStatus {
    pub fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
            Self::Usage => 2,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    pub stdout: String,
    pub stderr: String,
    pub status: ExitStatus,
}

pub fn run<I, T>(args: I) -> Response
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    match Cli::try_parse_from(&args) {
        Ok(cli) => execute(cli.command),
        Err(error) => match error.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => respond(
                error.render().to_string(),
                String::new(),
                ExitStatus::Success,
            ),
            _ if requests_json(&args) => usage_envelope(error.render().to_string()),
            _ => respond(String::new(), error.render().to_string(), ExitStatus::Usage),
        },
    }
}

fn execute(command: Command) -> Response {
    match command {
        Command::Capabilities { json: true } => envelope("capabilities", &capabilities()),
        Command::Capabilities { json: false } => {
            respond(capabilities().to_text(), String::new(), ExitStatus::Success)
        }
    }
}

/// A usage error is reported as JSON when the raw arguments asked for JSON, so that a caller
/// parsing stdout never has to scrape a help message.
fn requests_json(args: &[OsString]) -> bool {
    args.iter().skip(1).any(|arg| arg == "--json")
}

fn respond(stdout: String, stderr: String, status: ExitStatus) -> Response {
    Response {
        stdout,
        stderr,
        status,
    }
}

#[derive(Serialize)]
struct Envelope<D> {
    schema: &'static str,
    command: Option<&'static str>,
    #[serde(flatten)]
    outcome: Outcome<D>,
}

#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum Outcome<D> {
    Ok { data: D },
    UsageError { error: Problem },
}

#[derive(Serialize)]
struct Problem {
    code: ErrorCode,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    Usage,
}

fn envelope<D: Serialize>(command: &'static str, data: &D) -> Response {
    let body = Envelope {
        schema: SCHEMA,
        command: Some(command),
        outcome: Outcome::Ok { data },
    };
    encode(&body, ExitStatus::Success)
}

fn usage_envelope(message: String) -> Response {
    let body: Envelope<()> = Envelope {
        schema: SCHEMA,
        command: None,
        outcome: Outcome::UsageError {
            error: Problem {
                code: ErrorCode::Usage,
                message,
            },
        },
    };
    encode(&body, ExitStatus::Usage)
}

fn encode<B: Serialize>(body: &B, status: ExitStatus) -> Response {
    match serde_json::to_string(body) {
        Ok(json) => respond(json + "\n", String::new(), status),
        Err(_) => respond(
            String::new(),
            "cantos-forge: internal error: could not encode the JSON response\n".to_owned(),
            ExitStatus::Failure,
        ),
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Capabilities {
    forge_version: &'static str,
    commands: &'static [&'static str],
    workflows: Workflows,
    providers: &'static [&'static str],
    handoff: &'static str,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Workflows {
    host_assisted: &'static str,
    provider_managed: &'static str,
}

fn capabilities() -> Capabilities {
    Capabilities {
        forge_version: env!("CARGO_PKG_VERSION"),
        commands: COMMANDS,
        workflows: Workflows {
            host_assisted: UNSUPPORTED,
            provider_managed: UNSUPPORTED,
        },
        providers: &[],
        handoff: UNSUPPORTED,
    }
}

impl Capabilities {
    fn to_text(&self) -> String {
        let providers = if self.providers.is_empty() {
            "none".to_owned()
        } else {
            self.providers.join(", ")
        };
        format!(
            "cantos-forge {}\ncommands: {}\nhost-assisted workflow: {}\nprovider-managed workflow: {}\nproviders: {}\nhandoff: {}\n",
            self.forge_version,
            self.commands.join(", "),
            self.workflows.host_assisted,
            self.workflows.provider_managed,
            providers,
            self.handoff,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn invoke(args: &[&str]) -> Response {
        run(std::iter::once("cantos-forge").chain(args.iter().copied()))
    }

    #[test]
    fn capabilities_json_is_one_stable_envelope_on_stdout() {
        let response = invoke(&["capabilities", "--json"]);
        assert_eq!(
            response,
            Response {
                stdout: format!(
                    "{{\"schema\":\"cantos-forge.cli/v1\",\"command\":\"capabilities\",\"outcome\":\"ok\",\
                     \"data\":{{\"forge_version\":\"{}\",\"commands\":[\"capabilities\"],\
                     \"workflows\":{{\"host_assisted\":\"unsupported\",\"provider_managed\":\"unsupported\"}},\
                     \"providers\":[],\"handoff\":\"unsupported\"}}}}\n",
                    env!("CARGO_PKG_VERSION")
                ),
                stderr: String::new(),
                status: ExitStatus::Success,
            }
        );
    }

    #[test]
    fn capabilities_text_lists_the_same_facts() {
        let response = invoke(&["capabilities"]);
        assert_eq!(response.status, ExitStatus::Success);
        assert_eq!(response.stderr, "");
        assert_eq!(
            response.stdout,
            format!(
                "cantos-forge {}\ncommands: capabilities\nhost-assisted workflow: unsupported\n\
                 provider-managed workflow: unsupported\nproviders: none\nhandoff: unsupported\n",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn the_reported_commands_are_exactly_the_implemented_subcommands() {
        let implemented: Vec<String> = Cli::command()
            .get_subcommands()
            .map(|command| command.get_name().to_owned())
            .collect();
        assert_eq!(implemented, COMMANDS);
    }

    #[test]
    fn usage_errors_exit_2_and_never_write_a_result() {
        for args in [
            &["publish"][..],
            &["help"][..],
            &["capabilities", "--yaml"][..],
        ] {
            let response = invoke(args);
            assert_eq!(response.status, ExitStatus::Usage, "{args:?}");
            assert_eq!(response.stdout, "", "{args:?}");
            assert!(
                response.stderr.starts_with("error: "),
                "{args:?}: {}",
                response.stderr
            );
        }
    }

    #[test]
    fn a_missing_command_prints_usage_to_stderr_and_exits_2() {
        let response = invoke(&[]);
        assert_eq!(response.status, ExitStatus::Usage);
        assert_eq!(response.stdout, "");
        assert!(response.stderr.contains("Usage: cantos-forge <COMMAND>"));
    }

    #[test]
    fn a_json_usage_error_is_a_typed_envelope() {
        let response = invoke(&["submit", "--json"]);
        assert_eq!(response.status, ExitStatus::Usage);
        assert_eq!(response.stderr, "");
        let value: serde_json::Value = serde_json::from_str(&response.stdout).unwrap();
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["command"], serde_json::Value::Null);
        assert_eq!(value["outcome"], "usage_error");
        assert_eq!(value["error"]["code"], "usage");
        assert!(value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unrecognized subcommand 'submit'"));
        assert_eq!(value.as_object().unwrap().len(), 4);
    }

    #[test]
    fn help_and_version_succeed_on_stdout() {
        for flag in ["--help", "--version"] {
            let response = invoke(&[flag]);
            assert_eq!(response.status, ExitStatus::Success, "{flag}");
            assert!(response.stdout.contains("cantos-forge"), "{flag}");
            assert_eq!(response.stderr, "", "{flag}");
        }
    }

    #[test]
    fn exit_codes_are_stable() {
        let codes =
            [ExitStatus::Success, ExitStatus::Failure, ExitStatus::Usage].map(ExitStatus::code);
        assert_eq!(codes, [0, 1, 2]);
    }
}
