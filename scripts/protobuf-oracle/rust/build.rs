fn main() {
    // Cargo's default package-directory tracking does not include this external authority.
    // Without this edge, a cached oracle can silently validate a stale generated schema.
    println!("cargo:rerun-if-changed=../../../packages/contracts/protobuf/studio_v1.proto");
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("pinned protoc");
    std::env::set_var("PROTOC", protoc);
    prost_build::compile_protos(
        &["../../../packages/contracts/protobuf/studio_v1.proto"],
        &["../../../packages/contracts/protobuf"],
    )
    .expect("reviewed schema");
}
