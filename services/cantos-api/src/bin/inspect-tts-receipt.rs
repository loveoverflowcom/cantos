//! Inspect local evidence; never calls a provider, accepts an artifact or changes a snapshot.
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};

use cantos_api::production::MAX_PRODUCTION_DOCUMENT_BYTES;
use cantos_api::tts::audio::MAX_OUTPUT_BYTES;
use cantos_api::tts::{audit_receipt, decode_document, decode_receipt, MAX_RECEIPT_BYTES};

const USAGE: &str = "usage: inspect-tts-receipt <frozen-document.json> <receipt.json> [output.wav]";

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if !(2..=3).contains(&args.len()) {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let inputs = (|| {
        let document = read_bounded(Path::new(&args[0]), MAX_PRODUCTION_DOCUMENT_BYTES)?;
        let receipt = read_bounded(Path::new(&args[1]), MAX_RECEIPT_BYTES)?;
        let audio = args
            .get(2)
            .map(|path| read_bounded(Path::new(path), MAX_OUTPUT_BYTES))
            .transpose()?;
        Ok::<_, &'static str>((document, receipt, audio))
    })();
    let (document, receipt, audio) = match inputs {
        Ok(inputs) => inputs,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let result = (|| {
        let document = decode_document(&document)?;
        let receipt = decode_receipt(&receipt)?;
        audit_receipt(&document, &receipt, audio.as_deref())
    })();
    let (report, code) = match result {
        Ok(audit) => (serde_json::to_vec_pretty(&audit), 0),
        Err(error) => (serde_json::to_vec_pretty(&error), 1),
    };
    match report.map_err(io::Error::other).and_then(|mut json| {
        json.push(b'\n');
        io::stdout().lock().write_all(&json)
    }) {
        Ok(()) => ExitCode::from(code),
        Err(_) => {
            eprintln!("inspection report could not be written");
            ExitCode::from(2)
        }
    }
}

/// Bound both metadata and actual reads; paths and parser fragments stay out of diagnostics.
fn read_bounded(path: &Path, max_bytes: usize) -> Result<Vec<u8>, &'static str> {
    if !fs::metadata(path)
        .map_err(|_| "input could not be inspected")?
        .is_file()
    {
        return Err("input is not a regular file");
    }
    let mut options = OpenOptions::new();
    options.read(true);
    // A path can be swapped for a FIFO after the first check. Opening nonblocking lets the
    // descriptor check reject it without hanging. Regular-file reads retain their behavior.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|_| "input could not be opened")?;
    let metadata = file
        .metadata()
        .map_err(|_| "input could not be inspected")?;
    if !metadata.is_file() {
        return Err("input is not a regular file");
    }
    if metadata.len() > max_bytes as u64 {
        return Err("input exceeds its byte limit");
    }
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "input could not be read")?;
    if bytes.len() > max_bytes {
        return Err("input exceeds its byte limit");
    }
    Ok(bytes)
}
