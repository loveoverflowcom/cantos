# Protobuf C0 contract freeze evidence

Date: 2026-10-13. Inspected develop `b09c180` (fresh fetch; includes PR91 publication core).
Scope: issue #97 C0 only. Python wire-spec bytes, actual generated Rust prost messages and
actual Kotlin kotlinx serializers agree for **13 reviewed vectors** of all four pilot DTOs:
existing exact JSON fixtures, repeated issues, absent/null/empty presence, Vietnamese/emoji/
decomposed Unicode and uint64 0, 2^63-1, 2^63, 2^64-1. Missing/null vectors prove optional
wire presence, not valid application admission (C1/C4 reject those).

## Actual executed evidence

- `python3 scripts/reference_protobuf.py --write-goldens`: PASS independent protobuf goldens: 13
- `cargo run --manifest-path scripts/protobuf-oracle/rust/Cargo.toml -- packages/contracts/fixtures/protobuf/v1/goldens.json`:
  PASS prost 0.13.5 schema oracle: 13 vectors; generated directly from authority using pinned
  prost-build 0.13.5 / protoc-bin-vendored 3.2.0, Rust 1.87.0
- Kotlin compiler 2.2.20 with matching bundled serialization compiler plugin, kotlinx core/protobuf
  JVM 1.9.0: PASS Kotlin 2.2.20 / kotlinx protobuf 1.9.0 schema oracle: 13 vectors
- JVM: OpenJDK 21.0.12.1 Debian; Linux x86_64. Standalone common-compatible source executed on JVM;
  this is differentially-tested cross-implementation compatibility, not Android/iOS execution.

Tool artifacts obtained from official Kotlin GitHub release and Maven Central, installed in an
isolated task directory. No global tool replacement, account, secret, paid tool or network bypass.
SHA-256:

| Artifact | SHA-256 |
| --- | --- |
| kotlin-compiler-2.2.20.zip | 81f0264c9073b5cbbdb3ff8418cf2c5dac076879fc156fa1a6462f5a5acc4420 |
| kotlinx-serialization-core-jvm-1.9.0.jar | 1f0afa172110e45a7231ef1b44ae8fd84c1ebaff96f3fc3ad68ef8c48120b59c |
| kotlinx-serialization-protobuf-jvm-1.9.0.jar | 555035205ad1a21254b49fd6b5ddbe6ad1abb5c7baba244258bd758f80687bca |

Official checked references: [RFC 9996](https://www.rfc-editor.org/rfc/rfc9996.html),
[Kotlin ProtoBuf API](https://kotlinlang.org/api/kotlinx.serialization/kotlinx-serialization-protobuf/kotlinx.serialization.protobuf/-proto-buf/),
[kotlinx 1.9.0 setup](https://github.com/Kotlin/kotlinx.serialization/blob/v1.9.0/README.md),
[prost 0.13.5](https://docs.rs/prost/0.13.5/prost/).

## Explicit remaining gates

C0 proves no production codec, hostile admission, HTTP, database, browser, performance or Native
behavior. Those belong to C1–C5. Android SDK/CLI, iOS host/Native compiler and installed Gradle
were absent at initial doctor; independent JVM oracle succeeded without them. No iOS PASS claim.
No performance inference: payload/bundle/CPU/allocation/latency are NOT_RUN at C0.

Legacy MessagePack consumers were not proved absent; retention is mandatory. Pending branches
for discovery/library/publication do not add negotiated routes. Wire transport has no auth edit;
identity work must retain ownership of session/OIDC paths and merge reconciliation remains serial.

C0 local verification: `cargo xtask verify repository` PASS (including all discovered Python
unit tests and diff checks); standalone oracle `cargo clippy --locked --manifest-path
scripts/protobuf-oracle/rust/Cargo.toml --all-targets -- -D warnings` PASS; standalone rustfmt PASS.
`cargo xtask preflight --changed-only` UNSUPPORTED because missing Gradle broadly selected CMP
preview/demo; all other selected gates NOT_RUN at that attempt. No aggregate preflight PASS claimed.
