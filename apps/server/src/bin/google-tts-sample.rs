//! Plan one Google Cloud TTS line, and synthesize it only on an explicit, capped `--live` run.
//!
//! Without `--live` nothing is read from the environment and nothing is sent. A live run sends
//! exactly one request, never retries, writes the WAV only after validation and prints the
//! attempt record. It reads the line from a file the operator provides; use original sample
//! text, never a private manuscript.
use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
};

use cantos_server::synthesis::{
    google::{self, plan_line, GooglePlan, SpeechLine},
    google_http::{
        synthesize_once, AccessToken, GoogleCredential, GoogleHttpTransport, QuotaProject,
    },
    AttemptOutcome,
};
use serde_json::{json, Value};

const USAGE: &str = "usage: google-tts-sample --line <line.json> \
[--live --output <new.wav> --max-estimate-cents <n>]";
const TOKEN_VARIABLE: &str = "CANTOS_GOOGLE_TTS_ACCESS_TOKEN";
const PROJECT_VARIABLE: &str = "CANTOS_GOOGLE_TTS_QUOTA_PROJECT";
const MAX_LINE_FILE_BYTES: u64 = 64 * 1024;

struct Live {
    output: PathBuf,
    max_estimate_cents: u64,
}

struct Request {
    line: PathBuf,
    live: Option<Live>,
}

enum Command {
    Help,
    Run(Request),
}

fn main() -> ExitCode {
    let request = match parse_args(env::args_os().skip(1)) {
        Ok(Command::Run(request)) => request,
        Ok(Command::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(request) {
        Ok((report, succeeded)) => match print_json(&report) {
            Ok(()) if succeeded => ExitCode::SUCCESS,
            Ok(()) => ExitCode::from(1),
            Err(error) => {
                eprintln!("report could not be written: {error}");
                ExitCode::from(2)
            }
        },
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn run(request: Request) -> Result<(Value, bool), String> {
    let line = read_line(&request.line)?;
    let plan = match plan_line(&line) {
        Ok(plan) => plan,
        Err(findings) => return Ok((json!({ "status": "blocked", "findings": findings }), false)),
    };
    let Some(live) = request.live else {
        return Ok((planned(&plan), true));
    };
    if plan.estimate().amount_minor > live.max_estimate_cents {
        let report = json!({
            "status": "blocked",
            "findings": [{
                "code": "estimate_exceeds_cap",
                "estimate_cents": plan.estimate().amount_minor,
                "cap_cents": live.max_estimate_cents,
            }],
        });
        return Ok((report, false));
    }
    let transport = GoogleHttpTransport::new(credential()?)
        .map_err(|_| "the HTTPS client could not be initialized".to_owned())?;
    // Claim the output before paying for audio; create_new never overwrites a file.
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&live.output)
        .map_err(|error| format!("output could not be created: {error}"))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("runtime could not start: {error}"))?;
    let outcome = runtime.block_on(synthesize_once(&transport, &plan));
    let record = serde_json::to_value(google::record(&plan, &outcome))
        .map_err(|error| format!("record could not be serialized: {error}"))?;
    let report = json!({ "status": "attempted", "record": record });
    match outcome {
        AttemptOutcome::Succeeded(audio) => {
            output
                .write_all(audio.bytes())
                .and_then(|()| output.sync_all())
                .map_err(|error| {
                    format!("audio was synthesized but could not be written: {error}; {report}")
                })?;
            Ok((report, true))
        }
        AttemptOutcome::Failed(_) => {
            drop(output);
            // Only the empty file this run created is removed.
            let _ = fs::remove_file(&live.output);
            Ok((report, false))
        }
    }
}

fn planned(plan: &GooglePlan) -> Value {
    json!({
        "status": "planned",
        "sent": false,
        "provider_id": google::PROVIDER_ID,
        "adapter_version": google::ADAPTER_VERSION,
        "model_id": plan.voice().family.model_id(),
        "voice_id": plan.voice().name,
        "request_digest": plan.request_digest(),
        "estimate": plan.estimate(),
        "limitations": google::LIMITATIONS,
    })
}

fn credential() -> Result<GoogleCredential, String> {
    let token = env::var(TOKEN_VARIABLE)
        .map_err(|_| format!("{TOKEN_VARIABLE} must hold an OAuth access token for --live"))?;
    let access_token =
        AccessToken::new(token.trim()).map_err(|_| format!("{TOKEN_VARIABLE} is malformed"))?;
    let quota_project = match env::var(PROJECT_VARIABLE) {
        Ok(project) => Some(
            QuotaProject::new(project.trim())
                .map_err(|_| format!("{PROJECT_VARIABLE} is not a project ID"))?,
        ),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!("{PROJECT_VARIABLE} is not a project ID"))
        }
    };
    Ok(GoogleCredential {
        access_token,
        quota_project,
    })
}

fn read_line(path: &PathBuf) -> Result<SpeechLine, String> {
    let unreadable = |error: io::Error| format!("line file could not be read: {error}");
    let file = File::open(path).map_err(unreadable)?;
    let mut bytes = Vec::new();
    file.take(MAX_LINE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > MAX_LINE_FILE_BYTES {
        return Err(format!("line file exceeds {MAX_LINE_FILE_BYTES} bytes"));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("line file is not a speech line: {error}"))
}

fn print_json(value: &Value) -> io::Result<()> {
    let mut json = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    json.push(b'\n');
    io::stdout().lock().write_all(&json)
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Command, String> {
    let mut args = args;
    let (mut line, mut output, mut cap, mut live) = (None, None, None, false);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => return Ok(Command::Help),
            Some("--live") => live = true,
            Some("--line") => line = Some(PathBuf::from(args.next().ok_or("--line needs a path")?)),
            Some("--output") => {
                output = Some(PathBuf::from(args.next().ok_or("--output needs a path")?));
            }
            Some("--max-estimate-cents") => {
                let value = args.next().and_then(|value| value.into_string().ok());
                let parsed = value.and_then(|value| value.parse::<u64>().ok());
                cap = Some(parsed.ok_or("--max-estimate-cents expects a whole number of cents")?);
            }
            _ => return Err(format!("unexpected argument: {}", arg.to_string_lossy())),
        }
    }
    let line = line.ok_or("missing --line")?;
    let live = match (live, output, cap) {
        (false, None, None) => None,
        (true, Some(output), Some(max_estimate_cents)) => Some(Live {
            output,
            max_estimate_cents,
        }),
        (true, _, _) => return Err("--live requires --output and --max-estimate-cents".into()),
        (false, _, _) => return Err("--output and --max-estimate-cents require --live".into()),
    };
    Ok(Command::Run(Request { line, live }))
}
