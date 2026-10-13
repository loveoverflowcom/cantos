use super::*;
use super::{
    config::{labels, Plan, IMAGE, SIGNATURE},
    manifest::{Kind, Runtime},
    podman::{Output, Podman},
};
use serde_json::json;
use std::{collections::VecDeque, fs, path::PathBuf, time::Duration};

fn root() -> PathBuf {
    Workspace::discover().unwrap().root().to_owned()
}
fn inventory() -> Manifest {
    validate(&root()).unwrap()
}

struct PrivateFixture(PathBuf);
impl PrivateFixture {
    fn new() -> Self {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "cantos-service-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        for (name, contents) in [
            ("password", "test fixture secret"),
            ("chain.pem", "fixture only"),
            ("key.pem", "fixture only"),
        ] {
            fs::write(path.join(name), contents).unwrap();
            fs::set_permissions(path.join(name), fs::Permissions::from_mode(0o600)).unwrap();
        }
        fs::copy(
            root().join("services/kanidm/server.toml.example"),
            path.join("server.toml"),
        )
        .unwrap();
        fs::set_permissions(path.join("server.toml"), fs::Permissions::from_mode(0o600)).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn env(&self) -> BTreeMap<String, String> {
        [
            (
                "CANTOS_POSTGRES_PASSWORD_FILE",
                self.0.join("password").display().to_string(),
            ),
            ("CANTOS_KANIDM_CONFIG_DIR", self.0.display().to_string()),
        ]
        .into_iter()
        .map(|(key, value)| (key.into(), value))
        .collect()
    }
    fn plan(&self, service: &str) -> Plan {
        Plan::resolve(
            &root(),
            "fixture",
            service,
            inventory()
                .service(service)
                .unwrap()
                .container
                .as_ref()
                .unwrap(),
            &self.env(),
        )
        .unwrap()
    }
}
impl Drop for PrivateFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn out(code: i32, stdout: impl Into<String>) -> Output {
    Output {
        code,
        stdout: stdout.into(),
        stderr: String::new(),
    }
}

struct ScriptedPodman {
    results: VecDeque<Output>,
    calls: Vec<Vec<String>>,
    health_delay: Duration,
    limits: Vec<Duration>,
}
impl ScriptedPodman {
    fn new(results: Vec<Output>) -> Self {
        Self {
            results: results.into(),
            calls: Vec::new(),
            health_delay: Duration::ZERO,
            limits: Vec::new(),
        }
    }
}
impl Podman for ScriptedPodman {
    fn invoke(&mut self, args: &[String], timeout: Duration) -> Result<Output> {
        self.calls.push(args.to_vec());
        self.limits.push(timeout);
        if args[0] == "healthcheck" {
            std::thread::sleep(self.health_delay);
        }
        Ok(self
            .results
            .pop_front()
            .unwrap_or_else(|| panic!("unexpected call {args:?}")))
    }
}
const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn observed(plan: &Plan, running: bool) -> Output {
    let mut owned = labels("fixture", &plan.service);
    owned.insert(SIGNATURE.into(), plan.signature.clone());
    out(0, json!([{"Id":ID,"Name":format!("cantos-fixture-{}", plan.service),"Config":{"Labels":owned},"Mounts":[{"Type":"volume","Name":plan.volume,"Destination":if plan.service == "postgresql" {"/var/lib/postgresql/data"} else {"/data"}}],"State":{"Status": if running {"running"} else {"exited"},"Running":running,"ExitCode": if running {0} else {7},"Health":{"Status": if running {"healthy"} else {"unhealthy"}}}}]).to_string())
}
fn volume(plan: &Plan) -> Output {
    let mut owned = labels("fixture", &plan.service);
    owned.insert(IMAGE.into(), plan.image.clone());
    out(0, json!([{"Name":plan.volume,"Labels":owned}]).to_string())
}

#[test]
fn inventory_records_real_native_configured_container_and_planned_external_boundaries() {
    let m = inventory();
    assert_eq!(m.service("cantos-api").unwrap().runtime, Runtime::Native);
    assert_eq!(
        m.service("postgresql")
            .unwrap()
            .container
            .as_ref()
            .unwrap()
            .kind,
        Kind::Postgresql
    );
    assert_eq!(
        m.service("kanidm")
            .unwrap()
            .container
            .as_ref()
            .unwrap()
            .kind,
        Kind::Kanidm
    );
    assert_eq!(
        m.service("object-storage").unwrap().runtime,
        Runtime::External
    );
    assert_eq!(
        Target {
            service: None,
            all: true
        }
        .names(true),
        ["postgresql"]
    );
    assert_eq!(
        Target {
            service: None,
            all: true
        }
        .names(false),
        ["kanidm", "postgresql"]
    );
}

#[test]
fn schema_rejects_unknown_fields_runtime_mismatch_bad_pins_and_dependency_cycles() {
    let source = fs::read_to_string(root().join("services/manifest.toml")).unwrap();
    for (before, after, message) in [
        (
            "version = 1",
            "version = 2",
            "unsupported service manifest version",
        ),
        (
            "runtime = \"native\"",
            "runtime = \"external\"",
            "cantos-api: runtime differs from the reviewed boundary",
        ),
        (
            "17.11-bookworm",
            "latest",
            "postgresql: image needs an explicit patch version",
        ),
        (
            "1.10.5",
            "1.11.0",
            "Kanidm config/health contract is pinned to 1.10.5",
        ),
        (
            "optional_dependencies = [\"kanidm\", \"object-storage\"]",
            "optional_dependencies = [\"postgresql\"]",
            "cantos-api: duplicate dependency postgresql",
        ),
        (
            "required_dependencies = []",
            "required_dependencies = [\"cantos-api\"]",
            "required dependency cycle at cantos-api",
        ),
    ] {
        assert_eq!(
            Manifest::parse(&source.replacen(before, after, 1))
                .unwrap_err()
                .to_string(),
            message
        );
    }
    assert_eq!(
        Manifest::parse(&(source + "\n[typo]\nfoo = 1\n"))
            .unwrap_err()
            .to_string(),
        "invalid services/manifest.toml"
    );
}

#[test]
fn cli_requires_explicit_single_or_all_target_and_bounds_readiness() {
    use clap::Parser;
    for (line, kind) in [
        (
            "services up",
            clap::error::ErrorKind::MissingRequiredArgument,
        ),
        (
            "services down",
            clap::error::ErrorKind::MissingRequiredArgument,
        ),
        (
            "services logs",
            clap::error::ErrorKind::MissingRequiredArgument,
        ),
        (
            "services up kanidm --all",
            clap::error::ErrorKind::ArgumentConflict,
        ),
        (
            "services up postgresql --timeout-seconds 0",
            clap::error::ErrorKind::ValueValidation,
        ),
        (
            "services up postgresql --timeout-seconds 301",
            clap::error::ErrorKind::ValueValidation,
        ),
    ] {
        assert_eq!(
            crate::cli::Cli::try_parse_from(
                std::iter::once("xtask").chain(line.split_whitespace())
            )
            .unwrap_err()
            .kind(),
            kind,
            "{line}"
        );
    }
    assert!(matches!(
        crate::cli::Cli::try_parse_from(["xtask", "services", "up", "--all"])
            .unwrap()
            .command,
        crate::cli::Task::Services(ServicesArgs {
            action: Action::Up { .. },
            ..
        })
    ));
}

#[test]
fn generated_argv_binds_only_loopback_and_keeps_secrets_out_of_values() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    assert!(plan
        .create
        .contains(&"--publish=127.0.0.1:5432:5432".into()));
    assert!(plan.create.contains(
        &"--volume=cantos-fixture-postgresql-data:/var/lib/postgresql/data:nocreate".into()
    ));
    assert!(plan
        .create
        .contains(&"--env=POSTGRES_PASSWORD_FILE=/run/secrets/postgres-password".into()));
    assert!(!plan.create.join(" ").contains("test fixture secret"));
    let health = plan
        .create
        .iter()
        .find(|arg| arg.starts_with("--health-cmd="))
        .unwrap();
    let decoded: Vec<String> =
        serde_json::from_str(health.trim_start_matches("--health-cmd=")).unwrap();
    assert_eq!(
        decoded,
        [
            "pg_isready",
            "-h",
            "127.0.0.1",
            "-p",
            "5432",
            "-U",
            "postgres",
            "-d",
            "postgres"
        ]
    );
    assert!(!plan.create.iter().any(
        |arg| ["--privileged", "--network=host", "--rm", "--replace"].contains(&arg.as_str())
    ));
}

#[test]
fn private_input_permissions_symlinks_and_malformed_ports_fail_before_effects() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let fixture = PrivateFixture::new();
    let m = inventory();
    let c = m.service("postgresql").unwrap().container.as_ref().unwrap();
    let mut env = fixture.env();
    env.insert("CANTOS_POSTGRES_PORT".into(), "80".into());
    assert_eq!(
        Plan::resolve(&root(), "fixture", "postgresql", c, &env)
            .unwrap_err()
            .to_string(),
        "port must be in 1024..65535"
    );
    env.remove("CANTOS_POSTGRES_PORT");
    symlink(fixture.0.join("password"), fixture.0.join("link")).unwrap();
    env.insert(
        "CANTOS_POSTGRES_PASSWORD_FILE".into(),
        fixture.0.join("link").display().to_string(),
    );
    assert_eq!(
        Plan::resolve(&root(), "fixture", "postgresql", c, &env)
            .unwrap_err()
            .to_string(),
        "private input must be a regular file (no symlinks)"
    );
    env = fixture.env();
    fs::set_permissions(
        fixture.0.join("password"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_eq!(
        Plan::resolve(&root(), "fixture", "postgresql", c, &env)
            .unwrap_err()
            .to_string(),
        "private input must be caller-owned with no group/other permissions"
    );
    assert!(!config::instance_valid("--all"));
    assert!(!config::instance_valid("../foreign"));
    assert!(!config::instance_valid("LOCAL"));
    assert!(config::instance_valid("task-77"));
}

#[test]
fn kanidm_configtest_uses_private_readonly_config_and_temporary_data_without_network() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("kanidm");
    let check = plan.configtest.unwrap();
    assert!(check.contains(&"--network=none".into()));
    assert!(check.contains(&"--tmpfs=/data".into()));
    assert!(!check.iter().any(|arg| arg.contains("kanidm-data")));
    assert_eq!(
        &check[check.len() - 4..],
        [
            "docker.io/kanidm/server:1.10.5",
            "configtest",
            "-c",
            "/config/server.toml"
        ]
    );
    let source = fs::read_to_string(fixture.0.join("server.toml")).unwrap();
    assert_eq!(
        config::validate_kanidm(&source, Some(8444))
            .unwrap_err()
            .to_string(),
        "Kanidm origin must match the configured unprivileged host port"
    );
    assert_eq!(
        config::validate_kanidm(&(source + "unknown_secret = 'redacted'\n"), None)
            .unwrap_err()
            .to_string(),
        "invalid narrow Kanidm evaluation configuration"
    );
}

#[test]
fn changed_effective_ports_or_config_invalidate_restart_but_password_rotation_is_not_claimed() {
    let fixture = PrivateFixture::new();
    let before = fixture.plan("postgresql");
    fs::write(fixture.0.join("password"), "a different private secret").unwrap();
    assert_eq!(before.signature, fixture.plan("postgresql").signature);
    let mut env = fixture.env();
    env.insert("CANTOS_POSTGRES_PORT".into(), "55432".into());
    let m = inventory();
    let after = Plan::resolve(
        &root(),
        "fixture",
        "postgresql",
        m.service("postgresql").unwrap().container.as_ref().unwrap(),
        &env,
    )
    .unwrap();
    assert_ne!(before.signature, after.signature);
    let before = fixture.plan("kanidm");
    let source = fs::read_to_string(fixture.0.join("server.toml")).unwrap();
    fs::write(
        fixture.0.join("server.toml"),
        source.replace("idm.cantos.localhost", "identity.cantos.localhost"),
    )
    .unwrap();
    assert_ne!(before.signature, fixture.plan("kanidm").signature);
}

#[test]
fn foreign_container_blocks_down_logs_and_health_without_mutation() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    for action in ["down", "logs", "up"] {
        let mut record: serde_json::Value =
            serde_json::from_str(&observed(&plan, true).stdout).unwrap();
        record[0]["Config"]["Labels"][config::INSTANCE] = json!("another-task");
        let mut fake = ScriptedPodman::new(vec![out(0, ""), out(0, record.to_string())]);
        let error = match action {
            "down" => lifecycle::down(&mut fake, "fixture", "postgresql"),
            "logs" => lifecycle::logs(&mut fake, "fixture", "postgresql"),
            _ => lifecycle::up(&mut fake, &plan, Duration::from_secs(1)),
        }
        .unwrap_err();
        assert_eq!(error.to_string(), "cantos-fixture-postgresql: refusing foreign/unlabelled container; no mutation or logs allowed");
        assert_eq!(fake.calls.len(), 2);
    }
}

#[test]
fn stop_targets_inspected_id_and_preserves_container_and_volume() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    let mut fake = ScriptedPodman::new(vec![
        out(0, ""),
        observed(&plan, true),
        out(0, ""),
        out(0, ""),
        observed(&plan, false),
    ]);
    let message = lifecycle::down(&mut fake, "fixture", "postgresql").unwrap();
    assert!(message.ends_with("container and persistent volume retained"));
    assert_eq!(fake.calls[2], ["stop", "--time=10", ID]);
    assert!(!fake.calls.iter().any(|args| args
        .iter()
        .any(|arg| ["rm", "prune", "--volumes"].contains(&arg.as_str()))));
    assert!(fake.results.is_empty());
}

#[test]
fn absence_is_observed_only_on_the_documented_exists_exit_code() {
    let mut absent = ScriptedPodman::new(vec![out(1, "")]);
    assert_eq!(
        lifecycle::down(&mut absent, "fixture", "postgresql").unwrap(),
        "observed ABSENT; no resource changed"
    );
    let mut error = ScriptedPodman::new(vec![out(125, "")]);
    assert_eq!(
        lifecycle::down(&mut error, "fixture", "postgresql")
            .unwrap_err()
            .to_string(),
        "container exists failed with 125; absence was not established"
    );
}

#[test]
fn drift_and_missing_retained_volume_abort_restart_before_any_start() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    let mut record: serde_json::Value =
        serde_json::from_str(&observed(&plan, false).stdout).unwrap();
    record[0]["Config"]["Labels"][SIGNATURE] = json!("old-inputs");
    let mut fake = ScriptedPodman::new(vec![out(0, ""), out(0, record.to_string())]);
    assert_eq!(lifecycle::up(&mut fake,&plan,Duration::from_secs(1)).unwrap_err().to_string(),"postgresql: configuration drift; retained container/volume unchanged; review recreation explicitly");
    assert_eq!(fake.calls.len(), 2);
    let mut missing = ScriptedPodman::new(vec![out(0, ""), observed(&plan, false), out(1, "")]);
    assert_eq!(
        lifecycle::up(&mut missing, &plan, Duration::from_secs(1))
            .unwrap_err()
            .to_string(),
        "retained container has a missing volume; refusing to replace persistent data"
    );
    assert_eq!(missing.calls.len(), 3);
}

#[test]
fn foreign_or_image_incompatible_volume_is_never_attached() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    for field in [config::INSTANCE, IMAGE] {
        let mut record: serde_json::Value = serde_json::from_str(&volume(&plan).stdout).unwrap();
        record[0]["Labels"][field] = json!("foreign");
        let mut fake =
            ScriptedPodman::new(vec![out(1, ""), out(0, ""), out(0, record.to_string())]);
        assert_eq!(lifecycle::up(&mut fake,&plan,Duration::from_secs(1)).unwrap_err().to_string(),"cantos-fixture-postgresql-data: refusing foreign/unlabelled or image-incompatible persistent volume");
        assert_eq!(fake.calls.len(), 3);
    }
}

#[test]
fn healthy_restart_reuses_the_same_inspected_container_id_and_volume() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    let mut fake = ScriptedPodman::new(vec![
        out(0, ""),
        observed(&plan, false),
        out(0, ""),
        volume(&plan),
        out(0, ""),
        out(0, ""),
        observed(&plan, true),
        out(0, ""),
        out(0, ""),
        observed(&plan, true),
    ]);
    let message = lifecycle::up(&mut fake, &plan, Duration::from_secs(1)).unwrap();
    assert!(message.contains("observed running running=true"));
    assert_eq!(fake.calls[4], ["start", ID]);
    assert_eq!(fake.calls[7], ["healthcheck", "run", ID]);
    assert!(!fake
        .calls
        .iter()
        .any(|args| args[0] == "run" || args.iter().any(|arg| arg == "create")));
    assert!(fake.results.is_empty());
}

#[test]
fn an_exit_before_readiness_reports_the_real_exit_and_retains_resources() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    let mut fake = ScriptedPodman::new(vec![
        out(0, ""),
        observed(&plan, true),
        out(0, ""),
        volume(&plan),
        out(0, ""),
        observed(&plan, false),
    ]);
    assert_eq!(
        lifecycle::up(&mut fake, &plan, Duration::from_secs(1))
            .unwrap_err()
            .to_string(),
        "postgresql exited before readiness (state=exited, exit=7); container/volume retained"
    );
    assert!(!fake
        .calls
        .iter()
        .any(|args| ["stop", "rm", "healthcheck"].contains(&args[0].as_str())));
}

#[test]
fn readiness_caps_all_observation_and_probe_calls_by_the_remaining_budget() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    let mut fake = ScriptedPodman::new(vec![
        out(0, ""),
        observed(&plan, true),
        out(0, ""),
        volume(&plan),
        out(0, ""),
        observed(&plan, true),
        out(1, ""),
    ]);
    fake.health_delay = Duration::from_millis(30);
    assert_eq!(lifecycle::up(&mut fake,&plan,Duration::from_millis(20)).unwrap_err().to_string(),"postgresql readiness timed out; container/volume retained; inspect status/logs before retry");
    assert!(fake.limits[4..]
        .iter()
        .all(|duration| *duration <= Duration::from_millis(20)));
    assert_eq!(fake.calls.last().unwrap(), &["healthcheck", "run", ID]);
}

#[test]
fn status_uses_inspected_state_and_logs_include_both_streams_from_owned_id() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    let mut fake = ScriptedPodman::new(vec![out(0, ""), observed(&plan, false)]);
    let state = lifecycle::inspect(&mut fake, "fixture", "postgresql")
        .unwrap()
        .unwrap();
    assert!(state
        .description()
        .contains("exited running=false exit=7 health=unhealthy"));
    let mut fake = ScriptedPodman::new(vec![
        out(0, ""),
        observed(&plan, true),
        Output {
            code: 0,
            stdout: "stdout\n".into(),
            stderr: "stderr\n".into(),
        },
    ]);
    assert_eq!(
        lifecycle::logs(&mut fake, "fixture", "postgresql").unwrap(),
        "stdout\nstderr\n"
    );
    assert_eq!(fake.calls[2], ["logs", "--tail=200", ID]);
}

#[test]
fn runtime_probe_refuses_missing_connection_and_rootful_engine_without_provisioning() {
    for (output, expected) in [(out(125,""),"UNSUPPORTED: Podman connection unavailable; use an existing running rootless connection; no machine was initialized or started"),(out(0,r#"{"host":{"security":{"rootless":false}}}"#),"UNSUPPORTED: selected Podman connection is not observed rootless; do not change it automatically")] {
        let mut fake = ScriptedPodman::new(vec![output]);
        assert_eq!(lifecycle::available(&mut fake).unwrap_err().to_string(),expected);
        assert_eq!(fake.calls,[vec!["info","--format=json"]]);
    }
}

#[cfg(unix)]
#[test]
fn command_boundary_passes_metacharacters_as_literal_argv_and_bounds_stuck_children() {
    let args = vec![
        "-c".into(),
        "printf '%s' \"$1\"".into(),
        "fixture".into(),
        "$(touch /tmp/should-not-exist); `id`".into(),
    ];
    let result = podman::bounded_command(
        "sh",
        &args,
        Duration::from_secs(2),
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(result.code, 0);
    assert_eq!(result.stdout, args[3]);
    let error = podman::bounded_command(
        "sh",
        &["-c".into(), "sleep 5".into()],
        Duration::from_millis(40),
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "service command timed out after 0s; inspect retained state before retry"
    );
}

#[test]
fn first_creation_requires_nocreate_and_remains_bound_to_the_returned_id() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    let mut fake = ScriptedPodman::new(vec![
        out(1, ""),
        out(1, ""),
        out(0, plan.volume.clone()),
        volume(&plan),
        out(0, ID),
        out(0, ""),
        observed(&plan, false),
        out(0, ""),
        volume(&plan),
        out(0, ""),
        out(0, ""),
        observed(&plan, true),
        out(0, ""),
        out(0, ""),
        observed(&plan, true),
    ]);
    assert!(lifecycle::up(&mut fake, &plan, Duration::from_secs(1))
        .unwrap()
        .contains("observed running"));
    assert_eq!(fake.calls[4], plan.create);
    assert!(fake.calls[4].iter().any(|arg| arg.ends_with(":nocreate")));
    let mut replacement: serde_json::Value =
        serde_json::from_str(&observed(&plan, true).stdout).unwrap();
    replacement[0]["Id"] =
        json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let mut fake = ScriptedPodman::new(vec![
        out(1, ""),
        out(0, ""),
        volume(&plan),
        out(0, ID),
        out(0, ""),
        out(0, replacement.to_string()),
    ]);
    assert_eq!(
        lifecycle::up(&mut fake, &plan, Duration::from_secs(1))
            .unwrap_err()
            .to_string(),
        "created container identity/state changed; refusing start"
    );
    assert!(!fake.calls.iter().any(|call| call[0] == "healthcheck"));
}

#[test]
fn unsupported_nocreate_and_configtest_failure_leave_owned_volume_for_recovery() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    let mut fake = ScriptedPodman::new(vec![
        out(1, ""),
        out(0, ""),
        volume(&plan),
        Output {
            code: 125,
            stdout: String::new(),
            stderr: "invalid volume option nocreate".into(),
        },
    ]);
    assert_eq!(lifecycle::up(&mut fake,&plan,Duration::from_secs(1)).unwrap_err().to_string(),"UNSUPPORTED: Podman server must support named-volume nocreate; update only with separate consent; owned volume retained");
    let plan = fixture.plan("kanidm");
    let mut fake = ScriptedPodman::new(vec![
        out(1, ""),
        out(0, ""),
        volume(&plan),
        Output {
            code: 1,
            stdout: String::new(),
            stderr: "invalid TLS fixture".into(),
        },
    ]);
    assert_eq!(
        lifecycle::up(&mut fake, &plan, Duration::from_secs(1))
            .unwrap_err()
            .to_string(),
        "run exited with 1: invalid TLS fixture"
    );
    assert_eq!(fake.calls[3], *plan.configtest.as_ref().unwrap());
    assert_eq!(fake.calls.len(), 4);
    assert!(!fake
        .calls
        .iter()
        .any(|call| call.iter().any(|arg| arg == "stop" || arg == "prune")));
}

#[test]
fn read_only_private_mounts_never_relabel_operator_inputs() {
    let fixture = PrivateFixture::new();
    for name in ["postgresql", "kanidm"] {
        let plan = fixture.plan(name);
        for args in std::iter::once(&plan.create).chain(plan.configtest.as_ref()) {
            assert!(!args.iter().any(|arg| arg.ends_with(":ro,Z")
                || arg.ends_with(":ro,z")
                || arg.contains("label=disable")));
            assert!(args
                .iter()
                .any(|arg| arg.starts_with("--volume=/") && arg.ends_with(":ro")));
        }
    }
}

#[cfg(unix)]
#[test]
fn cancellation_persists_between_calls_and_prevents_the_next_spawn() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let interrupted = Arc::new(AtomicBool::new(false));
    assert_eq!(
        podman::bounded_command(
            "sh",
            &["-c".into(), "exit 0".into()],
            Duration::from_secs(1),
            &interrupted
        )
        .unwrap()
        .code,
        0
    );
    interrupted.store(true, Ordering::Relaxed);
    assert_eq!(
        podman::bounded_command(
            "cantos-must-not-spawn",
            &[],
            Duration::from_secs(1),
            &interrupted
        )
        .unwrap_err()
        .to_string(),
        "service command interrupted; inspect retained container state before retry"
    );
    interrupted.store(false, Ordering::Relaxed);
    let signal = Arc::clone(&interrupted);
    let cancel = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        signal.store(true, Ordering::Relaxed);
    });
    assert_eq!(
        podman::bounded_command(
            "sh",
            &["-c".into(), "sleep 5".into()],
            Duration::from_secs(1),
            &interrupted
        )
        .unwrap_err()
        .to_string(),
        "service command interrupted; inspect retained container state before retry"
    );
    cancel.join().unwrap();
}

#[test]
fn foreign_volume_replacement_after_create_is_refused_before_server_start() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    let mut replaced: serde_json::Value = serde_json::from_str(&volume(&plan).stdout).unwrap();
    replaced[0]["Labels"][config::INSTANCE] = json!("foreign");
    let mut fake = ScriptedPodman::new(vec![
        out(1, ""),
        out(0, ""),
        volume(&plan),
        out(0, ID),
        out(0, ""),
        observed(&plan, false),
        out(0, ""),
        out(0, replaced.to_string()),
    ]);
    assert_eq!(lifecycle::up(&mut fake,&plan,Duration::from_secs(1)).unwrap_err().to_string(),"cantos-fixture-postgresql-data: refusing foreign/unlabelled or image-incompatible persistent volume");
    assert!(!fake.calls.iter().any(|call| call[0] == "start"));
}

#[test]
fn supported_nocreate_refusal_is_a_failure_instead_of_an_upgrade_instruction() {
    let fixture = PrivateFixture::new();
    let plan = fixture.plan("postgresql");
    let mut fake = ScriptedPodman::new(vec![
        out(1, ""),
        out(0, ""),
        volume(&plan),
        Output {
            code: 125,
            stdout: String::new(),
            stderr: "volume missing does not exist and nocreate option is set".into(),
        },
    ]);
    assert_eq!(
        lifecycle::up(&mut fake, &plan, Duration::from_secs(1))
            .unwrap_err()
            .to_string(),
        "create exited with 125: volume missing does not exist and nocreate option is set"
    );
}

#[test]
fn machine_selection_freezes_existing_loopback_rootless_connection_and_refuses_cloud() {
    assert_eq!(podman::local_machine_connection(r#"[{"Name":"podman-machine-default","URI":"ssh://core@127.0.0.1:55111/run/user/1000/podman/podman.sock","Default":true}]"#).unwrap(),"podman-machine-default");
    for uri in [
        "ssh://root@127.0.0.1:55111/run/podman/podman.sock",
        "ssh://core@cloud.example:22/run/user/1000/podman/podman.sock",
    ] {
        let source = json!([{"Name":"remote","URI":uri,"Default":true}]).to_string();
        assert_eq!(podman::local_machine_connection(&source).unwrap_err().to_string(), "UNSUPPORTED: default connection must be existing loopback SSH to rootless Podman Machine; remote/cloud targets are refused");
    }
    assert_eq!(podman::local_machine_connection("[]").unwrap_err().to_string(), "UNSUPPORTED: macOS needs one existing default Podman Machine connection; no attachment was changed");
}
