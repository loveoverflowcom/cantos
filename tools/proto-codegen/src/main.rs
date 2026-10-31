//! Writes every consumer-owned, checked-in Protobuf binding from its service-owned `.proto`, or
//! with `--check` compares each checked-in file with a fresh generation. Each job names the
//! consumer that owns the output and the side it needs; no consumer imports another's output.
//! Run from this directory with the pinned lockfile: `cargo run --locked -- [--check]`.
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
enum Kind {
    /// prost messages only (the Studio Protobuf pilot representation).
    Messages,
    /// Messages plus the tonic client a caller of the service needs.
    Client,
    /// Messages plus the tonic server trait and router glue the service implements.
    Server,
}

struct Job {
    consumer: &'static str,
    proto: &'static str,
    out: &'static str,
    kind: Kind,
    /// Messages whose Debug the consumer writes by hand (redacted).
    redacted: &'static [&'static str],
}

const STUDIO_EDITORIAL: &str =
    "services/cantos-server/contracts/studio/editorial/v1/editorial.proto";
const STUDIO_ARTWORK: &str = "services/cantos-server/contracts/studio/artwork/v1/artwork.proto";
const IDENTITY: &str = "services/cantos-identity/contracts/identity/v1/identity.proto";
/// Requests that carry an email, password or proof secret.
const IDENTITY_SECRETS: &[&str] = &[
    "cantos.identity.v1.StartTheatreSignupRequest",
    "cantos.identity.v1.CompleteTheatreSignupRequest",
    "cantos.identity.v1.AuthenticatePasswordRequest",
    "cantos.identity.v1.CanonicalizeMailboxRequest",
    "cantos.identity.v1.CanonicalizeMailboxResponse",
    "cantos.identity.v1.VerifiedMailboxFact",
];

const JOBS: &[Job] = &[
    Job {
        consumer: "cantos-server Studio pilot codec",
        proto: STUDIO_EDITORIAL,
        out: "services/cantos-server/src/contract/studio/protobuf/generated.rs",
        kind: Kind::Messages,
        redacted: &[],
    },
    Job {
        consumer: "cantos-server Studio artwork codec",
        proto: STUDIO_ARTWORK,
        out: "services/cantos-server/src/contract/studio/protobuf/generated_artwork.rs",
        kind: Kind::Messages,
        redacted: &[],
    },
    Job {
        consumer: "cantos-studio Web pilot codec",
        proto: STUDIO_EDITORIAL,
        out: "apps/cantos-studio/web/src/contract/protobuf/generated.rs",
        kind: Kind::Messages,
        redacted: &[],
    },
    Job {
        consumer: "cantos-identity RPC server",
        proto: IDENTITY,
        out: "services/cantos-identity/src/rpc/generated.rs",
        kind: Kind::Server,
        redacted: IDENTITY_SECRETS,
    },
    Job {
        consumer: "cantos-identity process tests (RPC client)",
        proto: IDENTITY,
        out: "services/cantos-identity/tests/support/rpc_client.rs",
        kind: Kind::Client,
        redacted: &[],
    },
    Job {
        consumer: "cantos-server Identity RPC client",
        proto: IDENTITY,
        out: "services/cantos-server/src/platform/identity_auth/rpc/generated.rs",
        kind: Kind::Client,
        redacted: IDENTITY_SECRETS,
    },
    Job {
        consumer: "cantos-server tests (scripted Identity peer)",
        proto: IDENTITY,
        out: "services/cantos-server/tests/support/identity_rpc_peer.rs",
        kind: Kind::Server,
        redacted: &[],
    },
];

fn generate(root: &Path, job: &Job, scratch: &Path) -> String {
    let proto = root.join(job.proto);
    let include = proto.parent().expect("proto directory").to_owned();
    fs::create_dir_all(scratch).expect("scratch directory");
    let mut config = prost_build::Config::new();
    config.skip_debug(job.redacted.iter().copied());
    match job.kind {
        Kind::Messages => {
            config
                .out_dir(scratch)
                .compile_protos(&[&proto], &[&include])
                .expect("generate messages");
        }
        Kind::Client | Kind::Server => {
            tonic_prost_build::configure()
                .build_client(matches!(job.kind, Kind::Client))
                .build_server(matches!(job.kind, Kind::Server))
                .out_dir(scratch)
                .compile_with_config(config, &[&proto], &[&include])
                .expect("generate RPC");
        }
    }
    let mut outputs: Vec<PathBuf> = fs::read_dir(scratch)
        .expect("generated output")
        .map(|entry| entry.expect("generated entry").path())
        .collect();
    assert_eq!(outputs.len(), 1, "{}: one package per proto", job.proto);
    let generated = fs::read_to_string(outputs.remove(0)).expect("generated text");
    fs::remove_dir_all(scratch).ok();
    generated
}

fn main() {
    let check = std::env::args().nth(1).as_deref() == Some("--check");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::env::set_var(
        "PROTOC",
        protoc_bin_vendored::protoc_bin_path().expect("vendored protoc"),
    );
    let base = std::env::temp_dir().join(format!("cantos-proto-codegen-{}", std::process::id()));
    let mut drift = Vec::new();
    for (index, job) in JOBS.iter().enumerate() {
        let fresh = generate(&root, job, &base.join(index.to_string()));
        let target = root.join(job.out);
        if check {
            let current = fs::read_to_string(&target).unwrap_or_default();
            if current != fresh {
                drift.push(format!("{} ({})", job.out, job.consumer));
            }
        } else {
            fs::create_dir_all(target.parent().expect("output directory")).expect("output dir");
            fs::write(&target, fresh).expect("write generated binding");
            println!("generated {} for {}", job.out, job.consumer);
        }
    }
    fs::remove_dir_all(&base).ok();
    assert!(
        drift.is_empty(),
        "generated Protobuf drift: regenerate from the reviewed service-owned proto: {drift:?}"
    );
    if check {
        println!(
            "{} consumer-owned Protobuf bindings match their protos",
            JOBS.len()
        );
    }
}
