# Protobuf v1 corpus and Rust codec evidence

`goldens.json` contains 13 independent wire-spec fixtures, authored from existing exact JSON
application fixtures plus explicit presence/uint64/Unicode/repeated values. Python spec encoder,
generated Rust prost and actual Kotlin serializers agree byte/value-wise. Never regenerate the
expectations from the implementation under test. `hostile.json` adds 26 reviewed strict-admission
vectors with exact error categories for both Rust and Kotlin; null error means successful admission.

## C1 actual results, 2026-10-13

At frozen C0 `9f0368e18de867c58da31619b6be171478779e9e` plus C1 source:
- Rust 1.87.0 / prost 0.13.5; `cargo test --locked -p cantos-contracts`: **70 tests PASS**,
  including all baseline JSON/MessagePack tests and 9 new Protobuf tests
- `cargo clippy --locked -p cantos-contracts --all-targets -- -D warnings`: PASS
- Crate/oracle rustfmt checks: PASS
- Isolated pinned prost-build/protoc oracle recompiles the authoritative `.proto`, compares
  its exact generated Rust text to the checked-in module and checks all 13 vectors: PASS
- Python reviewed wire-spec/schema-drift/golden tests run in repository verification
- C0 actual Kotlin 2.2.20 + matching serialization plugin/core/protobuf 1.9.0 JVM execution:
  13 fixture vectors PASS; Native/Android and Kotlin hostile admission belong to C4 separately

Rust admission covers absent/null/explicit empty, all required scalar presence, wrong-wire,
unknown/reserved/appended tags, duplicate nested/scalar tags, trailing garbage, invalid UTF-8,
truncation, shortest varints, u64/tag/length overflow, reordered fields and repeated-list order.
It tests the exact 8 MiB byte boundary and excess, 65,536 issues and excess, and all **65,536**
two-byte inputs through **four** production root decoders (bounded exhaustive robustness,
not fuzzing or a proof). The strict scan performs no heap allocation and precedes prost allocation;
this is source-reviewed plus exact hostile example-tested evidence, not an allocator profiler.

Actual raw Protobuf fixture bytes: validate-script 3,852; save-revision 2,019; revision 2,215;
empty validation 0; required present empty script 2; unsigned-max save 51; two Unicode issues 31.
These are exact corpus lengths, not a compression, latency or speed claim. CPU, heap profiler,
WASM, target-native and end-to-end benchmarks remain for the serial integration measurement.

Aggregate `cargo xtask preflight --changed-only` initially reports UNSUPPORTED (missing installed
Gradle 8.14.3 for broadly selected CMP gates), other selected gates NOT_RUN; no preflight PASS.
Server/Web compilation awaits their sibling implementation commits and root Cargo.lock
reconciliation in integration. The independent crate result does not prove DB/auth/HTTP/browser,
Kotlin hostile handling, Android or iOS acceptance. No production default was enabled in C1.
