const EDITORIAL: &str = "../../../services/cantos-server/contracts/studio/editorial/v1";
const ARTWORK: &str = "../../../services/cantos-server/contracts/studio/artwork/v1";

fn main() {
    // Cargo's default package-directory tracking does not include this external authority.
    // Without this edge, a cached oracle can silently validate a stale generated schema.
    println!("cargo:rerun-if-changed={EDITORIAL}/editorial.proto");
    println!("cargo:rerun-if-changed={ARTWORK}/artwork.proto");
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("pinned protoc");
    std::env::set_var("PROTOC", protoc);
    prost_build::compile_protos(
        &[
            format!("{EDITORIAL}/editorial.proto"),
            format!("{ARTWORK}/artwork.proto"),
        ],
        &[EDITORIAL, ARTWORK],
    )
    .expect("reviewed schema");
}
