//! Regenerates `../../src/generated.rs` from `identity_v1.proto`, or with `--check` compares the
//! checked-in file with a fresh generation. Run from this directory with the pinned lockfile:
//! `cargo run --locked -- [--check]`.
use std::{fs, path::PathBuf};

const REDACTED: [&str; 3] = [
    "cantos.identity.v1.StartTheatreSignupRequest",
    "cantos.identity.v1.CompleteTheatreSignupRequest",
    "cantos.identity.v1.AuthenticatePasswordRequest",
];

fn main() {
    let check = std::env::args().nth(1).as_deref() == Some("--check");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let out = std::env::temp_dir().join(format!("cantos-identity-rpc-{}", std::process::id()));
    fs::create_dir_all(&out).expect("scratch directory");
    std::env::set_var(
        "PROTOC",
        protoc_bin_vendored::protoc_bin_path().expect("vendored protoc"),
    );
    let mut config = tonic_prost_build::Config::new();
    // Debug for messages that carry a secret is hand-written (redacted) in the contract crate.
    config.skip_debug(REDACTED);
    tonic_prost_build::configure()
        .out_dir(&out)
        .compile_with_config(
            config,
            &[root.join("identity_v1.proto")],
            std::slice::from_ref(&root),
        )
        .expect("generate identity RPC");
    let fresh = fs::read_to_string(out.join("cantos.identity.v1.rs")).expect("generated output");
    let target = root.join("src/generated.rs");
    fs::remove_dir_all(&out).ok();
    if check {
        let current = fs::read_to_string(&target).expect("checked-in generated.rs");
        assert_eq!(
            current, fresh,
            "generated identity RPC drift: regenerate from the reviewed proto"
        );
    } else {
        fs::write(&target, fresh).expect("write generated.rs");
    }
}
