//! Effects are executed only after configuration and exact resource ownership are established.
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;

use super::{
    config::{self, Plan, IMAGE, SIGNATURE},
    podman::{success, Podman},
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

fn call(podman: &mut impl Podman, args: &[String]) -> Result<String> {
    success(podman.invoke(args, COMMAND_TIMEOUT)?, &args[0])
}

pub fn available(podman: &mut impl Podman) -> Result<()> {
    ensure!(
        matches!(std::env::consts::OS, "linux" | "macos"),
        "UNSUPPORTED: services require Linux or macOS"
    );
    let output = podman.invoke(&["info".into(), "--format=json".into()], Duration::from_secs(10)).map_err(|error| {
        if error.to_string().starts_with("UNSUPPORTED:") { return error; }
        if error.chain().any(|cause| cause.downcast_ref::<std::io::Error>().is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)) {
            anyhow::anyhow!("UNSUPPORTED: Podman is unavailable; install/configure it separately with consent; no machine was initialized or started")
        } else { error.context("Podman probe failed") }
    })?;
    ensure!(output.code == 0, "UNSUPPORTED: Podman connection unavailable; use an existing running rootless connection; no machine was initialized or started");
    let info: serde_json::Value =
        serde_json::from_str(&output.stdout).context("invalid Podman info JSON")?;
    ensure!(info.pointer("/host/security/rootless").and_then(serde_json::Value::as_bool) == Some(true), "UNSUPPORTED: selected Podman connection is not observed rootless; do not change it automatically");
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ContainerState {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Config")]
    config: ContainerConfig,
    #[serde(rename = "State")]
    pub state: State,
    #[serde(rename = "Mounts")]
    mounts: Vec<Mount>,
}

#[derive(Debug, Deserialize)]
struct Mount {
    #[serde(rename = "Type")]
    kind: String,
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "Destination")]
    destination: String,
}

#[derive(Debug, Deserialize)]
struct ContainerConfig {
    #[serde(rename = "Labels")]
    labels: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct State {
    #[serde(rename = "Status")]
    pub status: String,
    #[serde(rename = "Running")]
    pub running: bool,
    #[serde(rename = "ExitCode")]
    pub exit_code: i32,
    #[serde(rename = "Health")]
    health: Option<Health>,
    #[serde(rename = "Healthcheck")]
    healthcheck: Option<Health>,
}

#[derive(Debug, Deserialize)]
struct Health {
    #[serde(rename = "Status")]
    status: String,
}

impl ContainerState {
    pub fn description(&self) -> String {
        let health = self
            .state
            .health
            .as_ref()
            .or(self.state.healthcheck.as_ref())
            .map(|health| health.status.as_str())
            .unwrap_or("not reported");
        format!(
            "observed {} running={} exit={} health={health} id={}",
            self.state.status, self.state.running, self.state.exit_code, self.id
        )
    }

    fn compatible(&self, plan: &Plan) -> Result<()> {
        ensure!(
            self.mounts.iter().any(|mount| mount.kind == "volume"
                && mount.name.as_deref() == Some(&plan.volume)
                && mount.destination
                    == if plan.service == "postgresql" {
                        "/var/lib/postgresql/data"
                    } else {
                        "/data"
                    }),
            "{}: retained container does not reference the expected persistent volume",
            plan.service
        );
        ensure!(self.config.labels.get(SIGNATURE) == Some(&plan.signature), "{}: configuration drift; retained container/volume unchanged; review recreation explicitly", plan.service);
        Ok(())
    }
}

pub fn inspect(
    podman: &mut impl Podman,
    instance: &str,
    service: &str,
) -> Result<Option<ContainerState>> {
    let name = format!("cantos-{instance}-{service}");
    let output = podman.invoke(
        &["container".into(), "exists".into(), name.clone()],
        COMMAND_TIMEOUT,
    )?;
    match output.code {
        1 => return Ok(None),
        0 => {}
        _ => bail!(
            "container exists failed with {}; absence was not established",
            output.code
        ),
    }
    let text = call(
        podman,
        &["container".into(), "inspect".into(), name.clone()],
    )?;
    let mut records: Vec<ContainerState> =
        serde_json::from_str(&text).context("invalid Podman container inspection")?;
    ensure!(
        records.len() == 1,
        "expected exactly one inspected container"
    );
    let observed = records.remove(0);
    ensure!(
        observed.name.trim_start_matches('/') == name
            && config::owned(&observed.config.labels, instance, service),
        "{name}: refusing foreign/unlabelled container; no mutation or logs allowed"
    );
    ensure!(
        observed.id.len() == 64 && observed.id.bytes().all(|c| c.is_ascii_hexdigit()),
        "invalid inspected container ID"
    );
    Ok(Some(observed))
}

fn volume(podman: &mut impl Podman, plan: &Plan, allow_create: bool) -> Result<()> {
    let output = podman.invoke(
        &["volume".into(), "exists".into(), plan.volume.clone()],
        COMMAND_TIMEOUT,
    )?;
    match output.code {
        0 => {}
        1 => {
            ensure!(
                allow_create,
                "retained container has a missing volume; refusing to replace persistent data"
            );
            let mut args = vec!["volume".into(), "create".into()];
            config::append_labels(&mut args, &config::labels(&plan.instance, &plan.service));
            args.push(format!("--label={IMAGE}={}", plan.image));
            args.push(plan.volume.clone());
            call(podman, &args)?;
        }
        _ => bail!(
            "volume exists failed with {}; absence was not established",
            output.code
        ),
    }
    #[derive(Deserialize)]
    struct Volume {
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Labels")]
        labels: BTreeMap<String, String>,
    }
    let text = call(
        podman,
        &["volume".into(), "inspect".into(), plan.volume.clone()],
    )?;
    let records: Vec<Volume> =
        serde_json::from_str(&text).context("invalid Podman volume inspection")?;
    ensure!(records.len() == 1, "expected exactly one inspected volume");
    let observed = &records[0];
    ensure!(
        observed.name == plan.volume
            && config::owned(&observed.labels, &plan.instance, &plan.service)
            && observed.labels.get(IMAGE) == Some(&plan.image),
        "{}: refusing foreign/unlabelled or image-incompatible persistent volume",
        plan.volume
    );
    Ok(())
}

pub fn up(podman: &mut impl Podman, plan: &Plan, timeout: Duration) -> Result<String> {
    let existing = inspect(podman, &plan.instance, &plan.service)?;
    if let Some(observed) = &existing {
        observed.compatible(plan)?;
    }
    // Recheck retained volume labels even when restarting an existing stopped container.
    volume(podman, plan, existing.is_none())?;
    let id = match existing {
        Some(observed) if observed.state.running => observed.id,
        Some(observed) => {
            if let Some(check) = &plan.configtest {
                call(podman, check)?;
            }
            call(podman, &["start".into(), observed.id.clone()])?;
            observed.id
        }
        None => {
            if let Some(check) = &plan.configtest {
                call(podman, check)?;
            }
            let output = podman.invoke(&plan.create, COMMAND_TIMEOUT)?;
            if output.code != 0
                && (output.stderr.contains("invalid volume option nocreate")
                    || output.stderr.contains("invalid option type \"nocreate\""))
            {
                bail!("UNSUPPORTED: Podman server must support named-volume nocreate; update only with separate consent; owned volume retained");
            }
            let id = success(output, "create")?.trim().to_owned();
            ensure!(id.len() == 64 && id.bytes().all(|c| c.is_ascii_hexdigit()), "create did not return a valid immutable container ID; inspect retained state before retry");
            // A stopped container reference prevents normal volume removal. Reinspect its
            // actual mount and the volume labels before any server process can write data.
            let created = inspect(podman, &plan.instance, &plan.service)?
                .context("created container disappeared; nothing started")?;
            ensure!(
                created.id == id && !created.state.running,
                "created container identity/state changed; refusing start"
            );
            created.compatible(plan)?;
            volume(podman, plan, false)?;
            call(podman, &["start".into(), id.clone()])?;
            id
        }
    };
    ready(podman, plan, &id, timeout)
}

fn ready(podman: &mut impl Podman, plan: &Plan, id: &str, timeout: Duration) -> Result<String> {
    let start = Instant::now();
    struct Deadline<'a, P> {
        podman: &'a mut P,
        start: Instant,
        timeout: Duration,
    }
    impl<P: Podman> Podman for Deadline<'_, P> {
        fn invoke(&mut self, args: &[String], timeout: Duration) -> Result<super::podman::Output> {
            let remaining = self
                .timeout
                .checked_sub(self.start.elapsed())
                .context("readiness deadline elapsed; container/volume retained")?;
            self.podman.invoke(args, timeout.min(remaining))
        }
    }
    let mut deadline = Deadline {
        podman,
        start,
        timeout,
    };
    let podman = &mut deadline;
    loop {
        let remaining = timeout.checked_sub(start.elapsed()).context("readiness deadline elapsed; container and volume retained; inspect status/logs before retry")?;
        let observed = inspect(podman, &plan.instance, &plan.service)?
            .context("container disappeared during readiness")?;
        observed.compatible(plan)?;
        ensure!(
            observed.id == id,
            "container identity changed during readiness; refusing further effects"
        );
        ensure!(
            observed.state.running,
            "{} exited before readiness (state={}, exit={}); container/volume retained",
            plan.service,
            observed.state.status,
            observed.state.exit_code
        );
        let probe = podman.invoke(
            &["healthcheck".into(), "run".into(), id.into()],
            remaining.min(Duration::from_secs(10)),
        )?;
        match probe.code {
            0 => {
                // Read observed health again; never infer healthy from the requested state.
                let observed = inspect(podman, &plan.instance, &plan.service)?
                    .context("container disappeared after health probe")?;
                ensure!(
                    observed.id == id && observed.state.running,
                    "container changed/exited after health probe"
                );
                return Ok(observed.description());
            }
            1 => {}
            _ => bail!(
                "{} health command failed with {}: {}; container/volume retained",
                plan.service,
                probe.code,
                probe.stderr.trim()
            ),
        }
        ensure!(
            start.elapsed() < timeout,
            "{} readiness timed out; container/volume retained; inspect status/logs before retry",
            plan.service
        );
        std::thread::sleep(Duration::from_millis(200).min(timeout.saturating_sub(start.elapsed())));
    }
}

pub fn down(podman: &mut impl Podman, instance: &str, service: &str) -> Result<String> {
    let Some(observed) = inspect(podman, instance, service)? else {
        return Ok("observed ABSENT; no resource changed".into());
    };
    if observed.state.running {
        call(
            podman,
            &["stop".into(), "--time=10".into(), observed.id.clone()],
        )?;
    }
    let after = inspect(podman, instance, service)?
        .context("container disappeared after stop; volume was not removed by xtask")?;
    ensure!(
        after.id == observed.id && !after.state.running,
        "stop did not establish the same stopped container"
    );
    Ok(format!(
        "{}; container and persistent volume retained",
        after.description()
    ))
}

pub fn logs(podman: &mut impl Podman, instance: &str, service: &str) -> Result<String> {
    let observed =
        inspect(podman, instance, service)?.context("container is absent; no logs available")?;
    let output = podman.invoke(
        &["logs".into(), "--tail=200".into(), observed.id],
        COMMAND_TIMEOUT,
    )?;
    ensure!(output.code == 0, "logs command failed with {}", output.code);
    Ok(format!("{}{}", output.stdout, output.stderr))
}
