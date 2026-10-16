//! The result of a command and how it becomes stdout, stderr and an exit status.
//!
//! Outcomes are typed so a caller never scrapes prose:
//!
//! | outcome        | meaning                                                          | exit |
//! |----------------|------------------------------------------------------------------|------|
//! | `ok`           | the command did what was asked (including a safe no-op replay)    | 0    |
//! | `check_failed` | `validate` ran and recorded a failing result                      | 1    |
//! | `refused`      | a rule said no; nothing was written                               | 1    |
//! | `blocked`      | an operator action is needed first; the error names it            | 1    |
//! | `failed`       | the environment or a stored record could not be trusted           | 1    |
//! | `usage_error`  | the arguments were invalid; nothing ran                           | 2    |

use serde::Serialize;
use serde_json::Value;

/// Version of the JSON envelope. A breaking change to any field needs a new name.
pub const SCHEMA: &str = "cantos-forge.cli/v1";

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub code: &'static str,
    pub message: String,
    /// What the operator can do next, when there is a command for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl Problem {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            action: None,
            details: None,
        }
    }

    pub fn action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    pub fn details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    Ok {
        command: &'static str,
        data: Value,
        text: String,
    },
    CheckFailed {
        command: &'static str,
        data: Value,
        text: String,
    },
    Refused {
        command: &'static str,
        problem: Problem,
    },
    Blocked {
        command: &'static str,
        problem: Problem,
    },
    Failed {
        command: &'static str,
        problem: Problem,
    },
    Usage {
        message: String,
    },
}

#[derive(Serialize)]
struct Envelope {
    schema: &'static str,
    command: Option<&'static str>,
    outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Problem>,
}

impl Reply {
    pub fn render(self, json: bool) -> Response {
        let (command, outcome, status) = self.classify();
        if json {
            return self.into_json(command, outcome, status);
        }
        match self {
            Self::Ok { text, .. } => respond(text, String::new(), status),
            Self::CheckFailed { text, .. } => respond(text, String::new(), status),
            Self::Refused { problem, .. }
            | Self::Blocked { problem, .. }
            | Self::Failed { problem, .. } => {
                respond(String::new(), problem_text(&problem), status)
            }
            Self::Usage { message } => respond(String::new(), message, status),
        }
    }

    fn classify(&self) -> (Option<&'static str>, &'static str, ExitStatus) {
        match self {
            Self::Ok { command, .. } => (Some(command), "ok", ExitStatus::Success),
            Self::CheckFailed { command, .. } => {
                (Some(command), "check_failed", ExitStatus::Failure)
            }
            Self::Refused { command, .. } => (Some(command), "refused", ExitStatus::Failure),
            Self::Blocked { command, .. } => (Some(command), "blocked", ExitStatus::Failure),
            Self::Failed { command, .. } => (Some(command), "failed", ExitStatus::Failure),
            Self::Usage { .. } => (None, "usage_error", ExitStatus::Usage),
        }
    }

    fn into_json(
        self,
        command: Option<&'static str>,
        outcome: &'static str,
        status: ExitStatus,
    ) -> Response {
        let (data, error) = match self {
            Self::Ok { data, .. } | Self::CheckFailed { data, .. } => (Some(data), None),
            Self::Refused { problem, .. }
            | Self::Blocked { problem, .. }
            | Self::Failed { problem, .. } => (None, Some(problem)),
            Self::Usage { message } => (None, Some(Problem::new("usage", message))),
        };
        encode(
            &Envelope {
                schema: SCHEMA,
                command,
                outcome,
                data,
                error,
            },
            status,
        )
    }
}

fn problem_text(problem: &Problem) -> String {
    let mut text = format!("cantos-forge: {}\n", problem.message);
    if let Some(Value::Array(reasons)) = &problem.details {
        for reason in reasons {
            text.push_str(&format!("  - {}\n", reason_line(reason)));
        }
    }
    if let Some(action) = &problem.action {
        text.push_str(&format!("action: {action}\n"));
    }
    text
}

/// One reason as `code: note or action`, falling back to the compact JSON.
fn reason_line(reason: &Value) -> String {
    let field = |name: &str| reason.get(name).and_then(Value::as_str);
    match (field("code"), field("note").or(field("action"))) {
        (Some(code), Some(hint)) => format!("{code}: {hint}"),
        (Some(code), None) => code.to_owned(),
        _ => reason.to_string(),
    }
}

pub fn respond(stdout: String, stderr: String, status: ExitStatus) -> Response {
    Response {
        stdout,
        stderr,
        status,
    }
}

fn encode(body: &Envelope, status: ExitStatus) -> Response {
    match serde_json::to_string(body) {
        Ok(json) => respond(json + "\n", String::new(), status),
        Err(_) => respond(
            String::new(),
            "cantos-forge: internal error: could not encode the JSON response\n".to_owned(),
            ExitStatus::Failure,
        ),
    }
}
