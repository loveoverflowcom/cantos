//! Runs the built `cantos-forge` binary: exit codes, the stdout/stderr split, and the whole
//! register → validate → export workflow on a real temporary directory.

use std::{
    ffi::OsStr,
    fs,
    path::Path,
    process::{Command, Output},
};

use serde_json::Value;
use tempfile::TempDir;

#[path = "support/ogg_fixture.rs"]
mod ogg_fixture;

fn forge<S: AsRef<OsStr>>(args: &[S]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cantos-forge"))
        .args(args)
        .output()
        .expect("the cantos-forge binary starts")
}

/// One JSON envelope on stdout, nothing on stderr.
fn envelope(output: &Output) -> Value {
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("UTF-8 stdout");
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    serde_json::from_str(&stdout).expect("JSON envelope")
}

fn json_ok(output: &Output) -> Value {
    let envelope = envelope(output);
    assert_eq!(output.status.code(), Some(0), "{envelope}");
    assert_eq!(envelope["outcome"], "ok", "{envelope}");
    envelope["data"].clone()
}

#[test]
fn capabilities_json_exits_0_with_one_envelope_on_stdout() {
    let output = forge(&["capabilities", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    let envelope = envelope(&output);
    assert_eq!(envelope["schema"], "cantos-forge.cli/v1");
    assert_eq!(envelope["outcome"], "ok");
    let commands: Vec<&str> = envelope["data"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command.as_str().unwrap())
        .collect();
    assert!(commands.contains(&"handoff export"), "{commands:?}");
    assert_eq!(envelope["data"]["handoff"], "unsupported");
    assert_eq!(
        envelope["data"]["workflows"]["provider_managed"],
        "unsupported"
    );
}

#[test]
fn an_unknown_command_exits_2_with_a_diagnostic_on_stderr_only() {
    let output = forge(&["run", "plan-1"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand 'run'"));
}

#[test]
fn a_missing_workspace_is_blocked_with_exit_1_and_the_command_that_fixes_it() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("none");
    let output = forge(&[
        OsStr::new("artifact"),
        OsStr::new("list"),
        OsStr::new("--json"),
        OsStr::new("--workspace"),
        missing.as_os_str(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let envelope = envelope(&output);
    assert_eq!(envelope["outcome"], "blocked");
    assert_eq!(envelope["error"]["code"], "workspace_missing");
    assert!(envelope["error"]["action"]
        .as_str()
        .unwrap()
        .starts_with("cantos-forge workspace init --path "));
}

/// The workflow's files, and a runner that appends `--workspace` and `--json` to each call.
struct Session {
    dir: TempDir,
}

impl Session {
    fn workspace(&self) -> std::path::PathBuf {
        self.dir.path().join("forge")
    }

    fn run(&self, args: &[&str], extra: &[&OsStr]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cantos-forge"));
        command.args(args).arg("--json");
        command.arg("--workspace").arg(self.workspace());
        command.args(extra);
        command.output().expect("the cantos-forge binary starts")
    }

    fn add(&self, kind: &str, file: &Path, inputs: &[String]) -> Value {
        let mut extra: Vec<&OsStr> = vec![
            OsStr::new("--kind"),
            OsStr::new(kind),
            OsStr::new("--producer"),
            OsStr::new("AI host session"),
            OsStr::new("--rights"),
            OsStr::new("original-work"),
            OsStr::new("--file"),
            file.as_os_str(),
        ];
        for input in inputs {
            extra.push(OsStr::new("--input"));
            extra.push(OsStr::new(input));
        }
        json_ok(&self.run(&["artifact", "add"], &extra))
    }
}

#[test]
fn a_story_and_its_audio_go_from_files_to_a_verified_local_bundle() {
    let session = Session {
        dir: TempDir::new().unwrap(),
    };
    let root = session.dir.path();
    let story = root.join("story.txt");
    let audio = root.join("line.ogg");
    let out = root.join("bundles").join("line-1");
    fs::write(
        &story,
        "Chiều cuối mùa mưa, Lan đứng trước hiên nhà cũ.".as_bytes(),
    )
    .unwrap();
    let ogg = ogg_fixture::Fixture::mono(vec![30; 100]).build();
    fs::write(&audio, &ogg).unwrap();

    let init = json_ok(&forge(&[
        OsStr::new("workspace"),
        OsStr::new("init"),
        OsStr::new("--json"),
        OsStr::new("--path"),
        session.workspace().as_os_str(),
    ]));
    assert_eq!(init["created"], true);

    let story_added = session.add("manuscript-original", &story, &[]);
    let story_id = story_added["artifact"].as_str().unwrap().to_owned();
    assert_eq!(story_added["revision"], 1);
    let audio_added = session.add("audio", &audio, &[format!("{story_id}@1")]);
    let audio_id = audio_added["artifact"].as_str().unwrap().to_owned();

    let validated = json_ok(&session.run(
        &["validate"],
        &[
            OsStr::new("--artifact"),
            OsStr::new(&audio_id),
            OsStr::new("--profile"),
            OsStr::new("opus-mono-24-v1"),
        ],
    ));
    assert_eq!(validated["outcome"], "passed");
    // 100 packets of 30 bytes over 1.99 s is about 12 kbit/s: inside the profile's ceiling.
    assert!(validated["detail"]["audio_bitrate_bps"].as_u64().unwrap() < 24_000);

    let export_args = |out: &Path| -> Vec<std::ffi::OsString> {
        vec![
            "--artifact".into(),
            audio_id.clone().into(),
            "--profile".into(),
            "opus-mono-24-v1".into(),
            "--out".into(),
            out.as_os_str().to_owned(),
        ]
    };
    let export = |out: &Path| {
        let args = export_args(out);
        let refs: Vec<&OsStr> = args.iter().map(|arg| arg.as_os_str()).collect();
        session.run(&["handoff", "export"], &refs)
    };
    let exported = json_ok(&export(&out));
    assert_eq!(exported["submission"], "not_run");

    assert_eq!(fs::read(out.join("audio.ogg")).unwrap(), ogg, "exact bytes");
    let manifest: Value =
        serde_json::from_slice(&fs::read(out.join("bundle.json")).unwrap()).unwrap();
    assert_eq!(manifest["schema"], "cantos-forge.handoff-bundle/v1");
    assert_eq!(manifest["inputs"][0]["artifact"], story_id);
    assert_eq!(manifest["evaluation"]["human_review"]["status"], "not_run");
    assert_eq!(manifest["submission"]["status"], "not_run");

    // Tampering with the stored bytes is detected, not exported.
    let blobs: Vec<_> = fs::read_dir(session.workspace().join("blobs"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(blobs.len(), 2);
    let audio_blob = blobs
        .iter()
        .find(|path| fs::metadata(path).unwrap().len() == ogg.len() as u64)
        .expect("the audio blob");
    let mut damaged = fs::read(audio_blob).unwrap();
    damaged[40] ^= 0xff;
    fs::write(audio_blob, damaged).unwrap();

    let out_two = root.join("bundles").join("line-2");
    let output = export(&out_two);
    assert_eq!(output.status.code(), Some(1));
    let envelope = envelope(&output);
    assert_eq!(envelope["outcome"], "failed");
    assert_eq!(envelope["error"]["code"], "workspace_corrupt");
    assert_eq!(envelope["error"]["details"]["kind"], "blob_digest_mismatch");
    assert!(
        !out_two.exists(),
        "nothing is written for a corrupt workspace"
    );
}
