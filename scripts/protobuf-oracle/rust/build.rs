fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("pinned protoc");
    std::env::set_var("PROTOC", protoc);
    prost_build::compile_protos(
        &["../../../packages/contracts/protobuf/studio_v1.proto"],
        &["../../../packages/contracts/protobuf"],
    )
    .expect("reviewed schema");
}
