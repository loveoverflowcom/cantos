# ADR 0023: Versioned Protobuf wire pilot with JSON rollback

- Status: proposed; issue #97 implementation contract frozen for parallel review
- Date: 2026-10-13
- Decision owner: repository owner (final PR review/acceptance)
- Related issue: [#97](https://github.com/loveoverflowcom/cantos/issues/97)
- Supersedes: [ADR 0013](0013-optional-messagepack-wire.md) for new preferred binary clients;
  its historical evidence and temporary server compatibility remain intact

## Context

At inspected develop `b09c180`, the only negotiated Studio routes are the two script reads,
revision save and validation. Existing Rust Serde DTOs remain application/API values and the
unchanged JSON contract. The Studio's shipped codec is JSON. There is no evidence that all
external/deployed MessagePack clients are absent. Publication, identity, storage and listener
branches do not justify expanding this pilot or changing their contracts.

Protobuf is selected for an explicit, cross-language schema at the transport edge, not because
a binary format is inherently faster. Actual raw/compressed bytes, encode/decode CPU, allocation,
WASM bundle and request latency must be measured separately before enabling the pilot default.
Minimal independent Rust/Kotlin schema evidence is recorded in
[the freeze record](../evidence/protobuf-wire-contract-freeze.md). This is not HTTP, DB or Native acceptance.

## Decision

The one wire authority is [studio_v1.proto](../../packages/contracts/protobuf/studio_v1.proto),
namespace `cantos.studio.wire.v1`, **proto2**. Every scalar is optional on the wire solely to expose
presence to both implementations. Exhaustive, fallible edge adapters require every scalar in the
current application DTOs; missing/null cannot manufacture zero, empty, validation or identity.

Rust uses pinned `prost = 0.13.5`, generated messages and explicit adapters. Kotlin uses compiler
and serialization plugin **2.2.20**, pinned `kotlinx-serialization-protobuf/core = 1.9.0`, explicit
`@ProtoNumber`, nullable scalar defaults and empty repeated defaults. These are portable common
sources, without generated JVM Google runtime classes or Rust FFI. JVM interoperability was
executed; Android/iOS builds/tests remain independent gates. Full uint64 range maps to Kotlin
Long raw bits (`toULong`/`toLong`); negative raw Long is not a negative application revision.

See the [versioned policy](../../packages/contracts/protobuf/README.md) for exact field and
conversion tables, HTTP negotiation, hostile decode and frozen worker interfaces.

### Compatibility, rollout and rollback

- JSON success and every JSON `ApiError` retain their exact existing shape and codes. No ProtoJSON.
- Protobuf is the only new binary client path. Legacy MessagePack server read/write stays
  temporarily supported; existing compatibility clients/tests may remain, but add no new client.
- Remove legacy support only after identifying deployed/external consumers, measuring zero
  legacy requests over an owner-approved release/observation period, publishing retirement
  notice and executing JSON rollback. No such evidence is held by this PR.
- Client sends each operation once. A timeout, cancellation or undecodable success remains
  ambiguous `unavailable`. Never resend/fallback a write with another codec. Existing explicit
  operation reconciliation may replay the same operation, preserving server authority.
- Keep `REVISION_CODEC = Json` until C0/C1/C2 compatibility and the reviewed DB/browser/measurement
  activation gates pass. Rebuild with JSON for rollback; no storage migration exists.
- Origin/session/ACL, idempotency, Script IR parsing, digest inputs, persisted revisions,
  business rules, audio and object storage are unchanged. Script IR is the exact JSON string.

## Alternatives

| Option | Benefit | Limitation / decision |
| --- | --- | --- |
| JSON only | universal tools, lowest rollout risk | remains baseline and rollback; no explicit binary schema |
| Existing named MessagePack | implemented and independently measured | retained server compatibility; no new clients |
| Proto3 defaults | wide tool ecosystem | defaults obscure required scalar presence; optional mappings require more care |
| Proto2 optional + admission | shared nullable presence mapping, full uint64 fixture agreement | selected; strict policy deliberately rejects unknown/duplicate tags |
| Generated Google JVM classes | straightforward JVM generation | cannot demonstrate commonMain/Native; rejected |
| Rust FFI or new network framework | reusable Rust engine | unnecessary coupling for portable fixture harness; rejected |

## Consequences

Versioned wire messages add generated artifacts, schema drift checks and explicit adapters, without
replacing Serde DTOs. Unknown fields are rejected, intentionally matching current strict JSON
admission; incompatible additive rollout needs a new negotiated schema version rather than a
server silently interpreting unknown facts. Repeated absence and empty cannot be distinguished
by protobuf and are intentionally one empty-list value. No application optional nullable scalar,
enum, packed scalar list or oneof is present; those need a new reviewed mapping and independent
fixtures before use. There is no API error protobuf message.

## Validation and revisit conditions

C0 independent Python wire-spec goldens must agree byte/value-wise with generated prost and
Kotlin serializers. C1/C4 must enforce the same bounded hostile pre-scan before runtime decoders.
C2 checks refusal before store and JSON/persistence/auth/replay parity. C3 checks actual response
Content-Type, single-send behavior and WASM costs. Final acceptance requires separate honest
Android/iOS, browser, real PostgreSQL and performance ledgers. Revisit on a field/type addition,
consumer/version change, measurable regression, failed rollback or proven legacy retirement.
