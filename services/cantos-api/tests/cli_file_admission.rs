//! Exercise the actual read-only CLIs under a bounded writerless-FIFO fault.
#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::symlink,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct OwnedChild(Option<Child>);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn bounded_output(mut command: Command) -> Output {
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut owned = OwnedChild(Some(child));
    let deadline = Instant::now() + Duration::from_secs(2);
    while owned.0.as_mut().unwrap().try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "CLI blocked on a writerless FIFO"
        );
        thread::sleep(Duration::from_millis(10));
    }
    owned.0.take().unwrap().wait_with_output().unwrap()
}

#[test]
fn writerless_fifo_and_its_symlink_are_rejected_before_reading_by_both_clis() {
    let scratch = Scratch(std::env::temp_dir().join(format!(
        "cantos-cli-file-admission-{}",
        uuid::Uuid::new_v4()
    )));
    fs::create_dir(&scratch.0).unwrap();
    let fifo = scratch.0.join("input.fifo");
    assert!(Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    let link = scratch.0.join("linked.fifo");
    symlink(&fifo, &link).unwrap();
    let good = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
    for path in [&fifo, &link] {
        let mut wav = Command::new(env!("CARGO_BIN_EXE_inspect-wav"));
        wav.arg(path);
        let mut before = Command::new(env!("CARGO_BIN_EXE_diff-script"));
        before.arg(path).arg(&good);
        let mut after = Command::new(env!("CARGO_BIN_EXE_diff-script"));
        after.arg(&good).arg(path);
        for command in [wav, before, after] {
            let output = bounded_output(command);
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8(output.stderr)
                .unwrap()
                .contains("input is not a regular file"));
        }
    }
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 2);
}
