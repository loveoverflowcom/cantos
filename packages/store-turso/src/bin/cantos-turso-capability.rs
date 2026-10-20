//! `cantos-turso-capability`: print the capability report for the pinned engine on this host.
//!
//! `report [DIR]` (default) runs the engine probes plus the process-level probes that need a
//! real second process: SIGKILL after acknowledged commits, a second owner, and a file-size
//! limit standing in for a full disk. The other modes are the children those probes start.
//! Exit status is non-zero when any probe differs from its recorded expectation.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::Arc;

use cantos_store_turso::capability::{self, Outcome, Probe, Report};
use cantos_store_turso::clock::SystemClock;
use cantos_store_turso::error::OpenError;
use cantos_store_turso::registry::{DatabaseKind, Intent};
use cantos_store_turso::testing::{ledger_registry, TestRun};
use cantos_store_turso::{OpenRequest, OwnedDatabase, Value};

#[tokio::main]
async fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("report") => report(args.next().map(PathBuf::from)).await,
        Some("crash-writer") => writer(args.next(), false).await,
        Some("limit-writer") => writer(args.next(), true).await,
        Some("try-open") => try_open(args.next()).await,
        Some(_) => {
            eprintln!("usage: cantos-turso-capability [report [DIR]]");
            ExitCode::from(64)
        }
    }
}

fn open_request(dir: &Path, intent: Intent) -> OpenRequest {
    OpenRequest::new(
        dir.join("server.db"),
        ledger_registry(DatabaseKind::Server),
        intent,
        Arc::new(SystemClock),
    )
}

/// Commit one ledger row at a time, printing the id only after the commit was acknowledged.
async fn writer(dir: Option<String>, tolerate_faults: bool) -> ExitCode {
    let Some(dir) = dir else {
        return ExitCode::from(64);
    };
    let db = match OwnedDatabase::open(open_request(Path::new(&dir), Intent::Migrate)).await {
        Ok(db) => db,
        Err(error) => {
            println!("open-failed {error}");
            return ExitCode::from(2);
        }
    };
    let out = std::io::stdout();
    for id in 1_i64.. {
        let step: Result<(), String> = async {
            let tx = db.begin_write().await.map_err(|f| format!("begin {f:?}"))?;
            tx.execute(
                "INSERT INTO ledger(id, payload, at) VALUES (?1, ?2, ?3)",
                cantos_store_turso::params_from_iter([
                    Value::Integer(id),
                    Value::Blob(vec![0xAB; 3000]),
                    Value::Integer(tx.now().get()),
                ]),
            )
            .await
            .map_err(|f| format!("insert {f:?}"))?;
            tx.commit().await.map_err(|f| format!("commit {f:?}"))
        }
        .await;
        match step {
            Ok(()) => {
                let mut lock = out.lock();
                let _ = writeln!(lock, "ack {id}");
                let _ = lock.flush();
            }
            Err(fault) => {
                println!("fault {fault}");
                return if tolerate_faults {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(3)
                };
            }
        }
    }
    ExitCode::SUCCESS
}

/// Exit status 10 when the file is already owned, 0 when ownership was taken.
async fn try_open(dir: Option<String>) -> ExitCode {
    let Some(dir) = dir else {
        return ExitCode::from(64);
    };
    match OwnedDatabase::open(open_request(Path::new(&dir), Intent::Serve)).await {
        Ok(_) => ExitCode::SUCCESS,
        Err(OpenError::AlreadyOwned) => ExitCode::from(10),
        Err(error) => {
            println!("other {error}");
            ExitCode::from(11)
        }
    }
}

async fn report(dir: Option<PathBuf>) -> ExitCode {
    let owned_run;
    let dir = match dir {
        Some(dir) => dir,
        None => {
            owned_run = TestRun::new("capability");
            owned_run.dir().to_path_buf()
        }
    };
    let mut report: Report = capability::run_all(&dir).await;
    let exe = std::env::current_exe().expect("the current executable is known");
    report.probes.push(kill9(&exe, &dir).await);
    report.probes.push(second_owner(&exe, &dir).await);
    report.probes.push(file_size_limit(&exe, &dir).await);
    print!("{}", report.render());
    if report.unexpected().is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn process_probe(id: &'static str, requirement: &'static str, pass: bool, detail: String) -> Probe {
    Probe {
        id,
        group: "process",
        requirement,
        expected: Outcome::Pass,
        substitute: None,
        outcome: if pass { Outcome::Pass } else { Outcome::Fail },
        detail,
    }
}

async fn ledger_state(dir: &Path) -> Result<(i64, String), String> {
    let db = OwnedDatabase::open(open_request(dir, Intent::Serve))
        .await
        .map_err(|error| format!("reopen refused: {error}"))?;
    let reader = db.reader().await.map_err(|f| format!("{f:?}"))?;
    let scalar = |sql: &'static str| {
        let reader = &reader;
        async move {
            let mut rows = reader.query(sql, ()).await.map_err(|f| format!("{f:?}"))?;
            let row = rows
                .next()
                .await
                .map_err(|e| e.to_string())?
                .ok_or("no row")?;
            let value = row.get_value(0).map_err(|e| e.to_string())?;
            while rows.next().await.map_err(|e| e.to_string())?.is_some() {}
            Ok::<Value, String>(value)
        }
    };
    let Value::Integer(max) = scalar("SELECT coalesce(max(id), 0) FROM ledger").await? else {
        return Err("max(id) is not an integer".into());
    };
    let Value::Integer(count) = scalar("SELECT count(*) FROM ledger").await? else {
        return Err("count is not an integer".into());
    };
    let Value::Text(check) = scalar("PRAGMA integrity_check").await? else {
        return Err("integrity_check is not text".into());
    };
    if count != max {
        return Err(format!("gap in the ledger: {count} rows, max id {max}"));
    }
    Ok((max, check))
}

async fn kill9(exe: &Path, dir: &Path) -> Probe {
    const ID: &str = "process.kill9_after_ack";
    const NEED: &str =
        "every acknowledged commit survives SIGKILL; the owner lock dies with the process";
    let run = dir.join("kill9");
    if std::fs::create_dir(&run).is_err() {
        return process_probe(ID, NEED, false, "could not create the run directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o700));
    }
    let mut child = match Command::new(exe)
        .args(["crash-writer", &run.to_string_lossy()])
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return process_probe(ID, NEED, false, format!("spawn failed: {error}")),
    };
    let mut acknowledged = 0_i64;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(id) = line.strip_prefix("ack ").and_then(|n| n.parse().ok()) {
                acknowledged = id;
            } else {
                let _ = child.kill();
                return process_probe(ID, NEED, false, format!("child said: {line}"));
            }
            if acknowledged >= 60 {
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    match ledger_state(&run).await {
        Ok((max, check)) if check == "ok" && max >= acknowledged => process_probe(
            ID,
            NEED,
            true,
            format!("killed after {acknowledged} acknowledged commits; reopened with {max} rows, integrity_check {check}, lock re-acquired"),
        ),
        Ok((max, check)) => process_probe(ID, NEED, false, format!("acknowledged {acknowledged}, found {max}, integrity {check}")),
        Err(error) => process_probe(ID, NEED, false, error),
    }
}

async fn second_owner(exe: &Path, dir: &Path) -> Probe {
    const ID: &str = "process.second_owner_refused";
    const NEED: &str = "a second process cannot open a file that already has an owner";
    let run = dir.join("second_owner");
    if std::fs::create_dir(&run).is_err() {
        return process_probe(ID, NEED, false, "could not create the run directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o700));
    }
    let owner = match OwnedDatabase::open(open_request(&run, Intent::Migrate)).await {
        Ok(owner) => owner,
        Err(error) => {
            return process_probe(ID, NEED, false, format!("owner could not open: {error}"))
        }
    };
    let refused = Command::new(exe)
        .args(["try-open", &run.to_string_lossy()])
        .status();
    drop(owner);
    let after = Command::new(exe)
        .args(["try-open", &run.to_string_lossy()])
        .status();
    match (refused.map(|s| s.code()), after.map(|s| s.code())) {
        (Ok(Some(10)), Ok(Some(0))) => process_probe(
            ID,
            NEED,
            true,
            "refused while owned (exit 10); accepted after the owner closed".into(),
        ),
        other => process_probe(ID, NEED, false, format!("{other:?}")),
    }
}

async fn file_size_limit(exe: &Path, dir: &Path) -> Probe {
    const ID: &str = "process.file_size_limit";
    const NEED: &str = "a write refused for lack of space (simulated by a file-size limit, not ENOSPC) leaves a consistent file";
    let run = dir.join("size_limit");
    if std::fs::create_dir(&run).is_err() {
        return process_probe(ID, NEED, false, "could not create the run directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o700));
    }
    // SIGXFSZ is ignored (and stays ignored across exec) so the write returns EFBIG instead of
    // terminating the child. `ulimit -f` counts 512- or 1024-byte blocks depending on the shell.
    let output = Command::new("sh")
        .args([
            "-c",
            "trap '' XFSZ; ulimit -f 96; exec \"$0\" limit-writer \"$1\"",
            &exe.to_string_lossy(),
            &run.to_string_lossy(),
        ])
        .output();
    let Ok(output) = output else {
        return process_probe(ID, NEED, false, "could not start the limited child".into());
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let acked = stdout.lines().filter(|l| l.starts_with("ack ")).count() as i64;
    let fault = stdout
        .lines()
        .find(|l| l.starts_with("fault "))
        .unwrap_or("no fault reported")
        .to_owned();
    if !stdout.lines().any(|l| l.starts_with("fault ")) {
        return process_probe(
            ID,
            NEED,
            false,
            format!("the limit never triggered after {acked} commits: {stdout}"),
        );
    }
    match ledger_state(&run).await {
        Ok((max, check)) if check == "ok" && max >= acked => process_probe(
            ID,
            NEED,
            true,
            format!("{acked} commits acknowledged, then `{fault}`; reopened with {max} rows, integrity_check {check}"),
        ),
        Ok((max, check)) => process_probe(ID, NEED, false, format!("acknowledged {acked}, found {max}, integrity {check}; {fault}")),
        Err(error) => process_probe(ID, NEED, false, format!("{error}; after `{fault}`")),
    }
}
