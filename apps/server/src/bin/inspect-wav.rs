//! Print the structural report of one WAV file as JSON. Reads only; never converts or uploads.
use std::{
    env,
    ffi::OsString,
    fs::OpenOptions,
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
};

use cantos_server::wav_inspection::{inspect_wav, Structure, MAX_RIFF_FILE_BYTES};

const USAGE: &str = "usage: inspect-wav <file.wav> [--max-bytes <n>]";
/// Fits about one hour of 48 kHz stereo 24-bit PCM; raise it explicitly for longer masters.
const DEFAULT_MAX_BYTES: u64 = 1 << 30;

struct Request {
    path: PathBuf,
    max_bytes: u64,
}

enum Command {
    Help,
    Inspect(Request),
}

fn main() -> ExitCode {
    let request = match parse_args(env::args_os().skip(1)) {
        Ok(Command::Inspect(request)) => request,
        Ok(Command::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let bytes = match read_bounded(&request) {
        Ok(bytes) => bytes,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let report = inspect_wav(&bytes);
    let written = serde_json::to_vec_pretty(&report)
        .map_err(io::Error::other)
        .and_then(|mut json| {
            json.push(b'\n');
            io::stdout().lock().write_all(&json)
        });
    match (written, report.structure()) {
        (Err(error), _) => {
            eprintln!("report could not be written: {error}");
            ExitCode::from(2)
        }
        (Ok(()), Structure::Valid) => ExitCode::SUCCESS,
        (Ok(()), Structure::Invalid) => ExitCode::from(1),
    }
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Command, String> {
    let mut args = args;
    let mut path = None;
    let mut max_bytes = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => return Ok(Command::Help),
            Some("--max-bytes") => {
                if max_bytes.replace(parse_max_bytes(args.next())?).is_some() {
                    return Err("--max-bytes given more than once".into());
                }
            }
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unexpected option: {flag}"));
            }
            _ if path.is_none() => path = Some(PathBuf::from(arg)),
            _ => return Err("expected exactly one input file".into()),
        }
    }
    let path = path.ok_or("missing input file")?;
    Ok(Command::Inspect(Request {
        path,
        max_bytes: max_bytes.unwrap_or(DEFAULT_MAX_BYTES),
    }))
}

fn parse_max_bytes(value: Option<OsString>) -> Result<u64, String> {
    let limit = value
        .as_deref()
        .and_then(|value| value.to_str())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|limit| (1..=MAX_RIFF_FILE_BYTES).contains(limit));
    limit.ok_or(format!(
        "--max-bytes expects an integer from 1 to {MAX_RIFF_FILE_BYTES}"
    ))
}

/// Refuses oversized input before reading it, and again if the file grows while it is read.
fn read_bounded(request: &Request) -> Result<Vec<u8>, String> {
    let unreadable = |error: io::Error| format!("input could not be read: {error}");
    let mut options = OpenOptions::new();
    options.read(true);
    // Opening a writerless FIFO must not block before descriptor admission.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(&request.path).map_err(unreadable)?;
    let metadata = file.metadata().map_err(unreadable)?;
    if !metadata.is_file() {
        return Err("input is not a regular file".into());
    }
    let too_large = || {
        format!(
            "input exceeds the {}-byte limit; pass --max-bytes to raise it",
            request.max_bytes
        )
    };
    if metadata.len() > request.max_bytes {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    file.take(request.max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > request.max_bytes {
        return Err(too_large());
    }
    Ok(bytes)
}
