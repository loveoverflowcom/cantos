# Studio Protobuf wire v1: frozen protocol and ownership

Authority: [editorial.proto](editorial.proto), `cantos.studio.wire.v1`, proto2;
[ADR 0023](../../../../../../docs/decisions/0023-versioned-protobuf-wire.md).
Existing Serde DTOs in `api/src/lib.rs` remain the application/JSON contract.

## Inventory and conversion table

Inspected develop `b09c180`; `services/cantos-server/src/interfaces/http/studio/mod.rs` mounts the
same handlers at `/api/v1/studio` and legacy `/api/v1` (legacy adds deprecation headers).

| Existing route (both mounts) | Request message | Success message |
| --- | --- | --- |
| GET scripts/{script}/head | none | RevisionResponse |
| GET scripts/{script}/revisions/{revision} | none | RevisionResponse |
| POST scripts/{script}/revisions | SaveRevisionRequest | RevisionResponse |
| POST validation | ValidateScriptRequest | ScriptValidationResponse |

All other routes stay JSON. Head/pinned/save share one response type. There is no Theatre or
Studio Mobile endpoint/harness client, no auth/session protobuf, no error message in the schema.

| Message | Field numbers (fixed) | Application admission |
| --- | --- | --- |
| ValidateScriptRequest | 1 script_json string | presence required, empty preserved |
| SaveRevisionRequest | 1 expected_revision uint64; 2 operation_id string; 3 script_json string | all present, full u64 and empty strings preserved; existing business validation unchanged |
| RevisionResponse | 1 script_id string; 2 revision uint64; 3 accepted_by string; 4 accepted_at string; 5 content_digest string; 6 export_digest string; 7 script_json string | all present; values copied exactly, no normalization |
| FieldIssue | 1 path string; 2 rule string | both present, including empty |
| ScriptValidationResponse | 1 issues repeated FieldIssue | absence and empty mean empty; ordered elements retained |

Reserved unused low numbers are explicitly in the schema. Never renumber/reuse a field; removed
numbers **and names** must be reserved. No removed names exist at initial v1. Future fields require
schema review, adapters, oracle fixtures and version negotiation. No custom defaults. Optional
wire scalars map to Rust Option and Kotlin nullable/default null; application conversion rejects
None/null. JSON null for a required field stays invalid. Repeated absent and empty normalize to
empty (JSON always emits issues []). No enums/oneof exist in these actual DTOs; adding either needs
reviewed valid/missing/unknown/zero variants, exhaustive fallible adapters and independent goldens.
No packed numeric collections, maps, polymorphic serializers or floating point exist.

## Strict bounded decode profile (both Rust and Kotlin)

Before allocating message/string/list data, scan the already-buffered envelope:
- Maximum envelope 8 MiB (same HTTP baseline); maximum nested message depth 4
- Maximum repeated issues 65,536; total fields at most 196,616; no capacity from hostile lengths
- Only declared tags and exact wire types 0 (uint64), 2 (string/message); unknown/reserved tags
  rejected, including unknown fields appended after an otherwise valid message
- Duplicate scalar tags rejected at every message level, regardless of value; only issues repeats
- Varints must be shortest canonical encoding, at most 10 bytes, tenth byte <= 1 for u64;
  tags must fit protobuf's 29-bit field number and cannot be zero
- Checked length/cursor arithmetic; claimed length beyond remaining bytes is Truncated;
  invalid UTF-8 rejected, never replaced or normalized; no groups/fixed32/fixed64 accepted
- Every known string present before application conversion; no implicit required default
- Exactly one unframed message. Protobuf has no outer message terminator: concatenation of
  singular messages fails duplicate/unknown tags; concatenated repeated-only validation messages
  are indistinguishable from one ordered list and are intentionally that list, subject to bounds
- Empty validation response is valid empty issues; empty scalar-bearing message fails Shape;
  arbitrary trailing garbage fails Malformed/Truncated/Shape, never ignored

Typed existing DecodeError categories remain Truncated, TooDeep, TrailingBytes, InvalidUtf8,
Shape, Malformed; add TooLarge for bounded envelope/field/item limits. Duplicate/unknown/wrong
wire type/missing presence are Shape; overflow/noncanonical varints/zero tags are Malformed.
Fallible encoding checks size/item limits and returns existing EncodeError without bytes.
Script IR's independent 2 MiB semantic limit remains unchanged and is not reimplemented here.

## HTTP media type and order

Canonical binary type: `application/protobuf` ([RFC 9996](https://www.rfc-editor.org/rfc/rfc9996.html)).
Accept deprecated `application/x-protobuf` as input/Accept alias; always emit canonical spelling.
No x-protobuffer, suffix inference or ProtoJSON. ASCII case/outer whitespace ignored. For protobuf,
allow only absent parameters or `encoding=binary`, `version=1` (case-insensitive names/value
binary, optional quoted values); reject duplicate/unknown parameters, charset, nonbinary encoding
or unsupported version. q is allowed in Accept only. JSON and MessagePack keep baseline parsing.
`application/protobuf+json` must be refused rather than fall through generic +json classification.

Request Content-Type drives decoding. Omitted/empty/non-ASCII header follows baseline JSON;
unknown positive type gives existing 415 invalid_request. Accept quality/specificity stays baseline:
strictly higher protobuf wins; JSON wins equal/ambiguous ties when allowed; legacy MessagePack
wins only its strict binary preference, with equal binary-only qualities favoring protobuf.
Omitted/unparseable/wildcards default JSON. No allowed representation gives existing 406.
Protobuf parameters are validated before store admission; q=0 cannot be resurrected by wildcards.

Order: Origin -> session -> Accept -> Content-Type -> buffered body + strict scan -> existing
handler/store. Every failure is JSON ApiError, unchanged taxonomy. Successful/406 responses use
Vary: Accept, all responses retain no-store/nosniff. Response decoder uses actual Content-Type;
unexpected/invalid success is ambiguous unavailable. No codec fallback/retry of failed writes.

## Frozen Rust interfaces (C1 supplies; B/C consume)

Each consumer's `contract::codec` (Server and Studio Web; formerly `cantos_contracts::wire`):
- WireFormat adds Protobuf; media_type returns application/protobuf
- PROTOBUF_MEDIA_TYPE constant; from_content_type implements the protobuf-specific policy
- Existing generic encode/decode remain baseline JSON/MessagePack; Protobuf fails closed there
  (EncodeError / DecodeError::Shape), because arbitrary Serde DTOs have no protobuf schema
- New encode_pilot<T: Serialize + protobuf::ProtobufDto>(WireFormat, &T) -> Result<Vec<u8>, EncodeError>
- New decode_pilot<T: DeserializeOwned + protobuf::ProtobufDto>(WireFormat, &[u8]) -> Result<T, DecodeError>

Each consumer's Studio Protobuf edge codec (Server `contract::studio::protobuf`, Studio Web
`contract::protobuf`; formerly `cantos_contracts::protobuf`):
- Public ProtobufDto trait: fn encode_protobuf(&self) -> Result<Vec<u8>, wire::EncodeError>;
  fn decode_protobuf(bytes: &[u8]) -> Result<Self, wire::DecodeError> where Self: Sized
- Implemented ONLY for ValidateScriptRequest, SaveRevisionRequest, RevisionResponse,
  ScriptValidationResponse (FieldIssue conversion nested, not a pilot root)
- encode<T: ProtobufDto + ?Sized>(&T), decode<T: ProtobufDto>(&[u8]) convenience functions
- MAX_PROTOBUF_BYTES = 8 * 1024 * 1024; MAX_PROTOBUF_DEPTH = 4;
  MAX_PROTOBUF_ISSUES = 65_536; MAX_PROTOBUF_FIELDS = 196_616
- Checked-in prost generated messages live in private edge module, not DTO replacement

B adds ProtobufDto bounds only to pilot extractors/respond, keeping JSON-only helpers untouched;
C adds bounds only to negotiated functions. Existing generic wire tests/tools use baseline helpers.
Test-only unencodable DTOs must explicitly implement ProtobufDto if using pilot respond.

## Parallel path ownership after C0

| Commit / worker | Exclusive edits |
| --- | --- |
| C1 A | packages/contracts/api/** (now each consumer's `contract`); packages/contracts/fixtures/protobuf/** (now `fixtures/protobuf/**` here); scripts/reference_protobuf.py; scripts/test_reference_protobuf.py; scripts/protobuf-oracle/** |
| C2 B | services/cantos-server/src/interfaces/http/**; services/cantos-server/tests/** |
| C3 C | apps/cantos-studio/web/** |
| C4 D | apps/cantos-theatre/mobile/** |
| C5 integrator | shared root Cargo.lock, Gradle lock reconciliation, xtask/CI selectors, architecture/skills/work queue/evidence/rollout documentation |

Only integration worktree owns root Cargo.lock. Workers may locally resolve ignored lock changes
for tests and supply a dependency list, but never commit/shared-edit that lock. C0 proto/ADR/policy
are frozen; coordinate any interface change with parent before changing them. Standalone oracle
has its own reviewed pinned lock; it never changes root workspace dependency resolution.

## Deterministic oracle and generation

Reviewed independent fixtures: [goldens.json](fixtures/protobuf/goldens.json). Derive bytes
only via the Python wire-spec encoder, compare independently using Rust generated prost and
Kotlin serializers. Never bless expected bytes from production output.
The oracle is an xtask primitive; C5 wires its drift checks into existing verification selectors.

Reference commands, from repository root (local installed tools, no global replacement):

```sh
python3 scripts/reference_protobuf.py
cargo run --locked --manifest-path scripts/protobuf-oracle/rust/Cargo.toml -- services/cantos-server/contracts/studio/editorial/v1/fixtures/protobuf/goldens.json
python3 scripts/protobuf-oracle/generate_kotlin_vectors.py services/cantos-server/contracts/studio/editorial/v1/fixtures/protobuf/goldens.json target/protobuf-vectors.kt
# Kotlin 2.2.20 + matching serialization plugin, core/protobuf JVM 1.9.0 in $TOOLS
CP=$TOOLS/kotlinx-serialization-core-jvm-1.9.0.jar:$TOOLS/kotlinx-serialization-protobuf-jvm-1.9.0.jar
$TOOLS/kotlinc/bin/kotlinc scripts/protobuf-oracle/Oracle.kt target/protobuf-vectors.kt -cp "$CP" -Xplugin=$TOOLS/kotlinc/lib/kotlinx-serialization-compiler-plugin.jar -d target/protobuf-oracle.jar
$TOOLS/kotlinc/bin/kotlin -cp target/protobuf-oracle.jar:$CP Protobuf_vectorsKt
```

Pinned prost-build 0.14.4 / protoc-bin-vendored 3.2.0 compile the same authority in the isolated
oracle, which compares its output with both consumers' checked-in bindings; `tools/proto-codegen`
writes each consumer's private module (Server, Studio Web) and its `--check` compares exact
generated text ([ADR 0046](../../../../../../docs/decisions/0046-service-owned-contracts.md); the
0.13.5 → 0.14.4 move added only `Eq, Hash` derives). Runtime/WASM builds never install/run protoc.
Kotlin vectors are test inputs from reviewed values; no production Kotlin source generation.
