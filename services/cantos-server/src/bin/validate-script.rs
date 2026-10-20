use std::{env, fs, process::ExitCode};

use cantos_server::script_ir::read_script;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: validate-script <script.json> [--export | --content]");
        return ExitCode::from(2);
    };
    let mode = args.next();
    if args.next().is_some() || !matches!(mode.as_deref(), None | Some("--export" | "--content")) {
        eprintln!("usage: validate-script <script.json> [--export | --content]");
        return ExitCode::from(2);
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("input could not be read: {error}");
            return ExitCode::from(2);
        }
    };
    match read_script(&bytes) {
        Ok(script) => {
            use std::io::{self, Write};
            let output = match mode.as_deref() {
                Some("--export") => script.export_bytes(),
                Some("--content") => script.canonical_bytes(),
                _ => format!("{}\n", script.content_digest()).into_bytes(),
            };
            match io::stdout().write_all(&output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("output could not be written: {error}");
                    ExitCode::from(2)
                }
            }
        }
        Err(error) => {
            eprintln!("Script IR rejected: {error:?}");
            ExitCode::from(1)
        }
    }
}
