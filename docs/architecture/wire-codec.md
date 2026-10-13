# Wire codec: JSON, versioned Protobuf and legacy MessagePack

Cantos carries existing application DTOs as JSON by default. The four Studio revision/validation
routes additionally negotiate versioned Protobuf or legacy named MessagePack. This changes only
the HTTP representation: Script IR remains the exact JSON string, and authorization, revisions,
idempotency, digests, PostgreSQL rows and object/audio storage retain their existing authority.

[ADR 0023](../decisions/0023-versioned-protobuf-wire.md) transitions the binary preference from
[ADR 0013](../decisions/0013-optional-messagepack-wire.md). The latter and its
[historical measurements](../evidence/messagepack-wire.md) remain intact. The versioned
[protocol policy](../../packages/contracts/protobuf/README.md) owns the exact field/presence,
media-type/parameter, limits and admission tables; do not infer an additional route or DTO.
The [integration evidence](../evidence/protobuf-wire-migration.md) separates implemented,
executed and deferred acceptance.

## Owning boundaries

- Existing Serde DTOs: [cantos-contracts](../../packages/contracts/api/src/lib.rs).
- Protobuf authority: [studio_v1.proto](../../packages/contracts/protobuf/studio_v1.proto),
  namespace `cantos.studio.wire.v1`, proto2 optional wire scalars with required application admission.
- Private generated prost values, exhaustive fallible adapters and bounded prescan:
  [protobuf](../../packages/contracts/api/src/protobuf/mod.rs).
- JSON/MessagePack helpers and media parsing: [wire](../../packages/contracts/api/src/wire.rs).
  Generic Serde encode/decode refuse Protobuf; only the four `ProtobufDto` roots use pilot helpers.
- HTTP negotiation/extractors: [http/wire.rs](../../services/cantos-api/src/interfaces/http/wire.rs).
- Studio request/actual-response decoding:
  [client wire](../../apps/cantos-studio/web/src/wire.rs),
  [fetch adapter](../../apps/cantos-studio/web/src/api.rs).
- Portable fixture-only Kotlin adapter:
  [compatibility record](../../apps/cantos-theatre/mobile/docs/protobuf-compatibility.md).
  It sends no Theatre request and adds no native media capability.

The pilot remains `GET scripts/{id}/head`, `GET scripts/{id}/revisions/{revision}`,
`POST scripts/{id}/revisions` and `POST validation`, mounted under `/api/v1/studio` and the
existing deprecated `/api/v1` alias. Other implemented Studio/listener routes remain JSON.
No auth, storage, audio, Admin, Theatre or Studio Mobile transport is migrated.

## Negotiation and errors

Canonical Protobuf output is `application/protobuf`; deprecated `application/x-protobuf` is
accepted under the same strict version/binary parameter policy. ProtoJSON is refused.
JSON and legacy MessagePack keep their existing media parsing. `Content-Type` chooses request
decode; `Accept` chooses success encode. Omitted/ambiguous preferences and JSON ties choose JSON;
a strict binary preference chooses Protobuf, including a binary-only tie with MessagePack.
Most-specific exclusions cannot be resurrected by a wildcard. Invalid Protobuf parameters are
explicit exclusions even if a malformed quality accompanies them; baseline malformed-quality
tolerance for the other representations remains unchanged.

Origin/session decisions precede negotiation, then Accept, Content-Type and body admission
complete before handler/store effects. Every failure remains the existing JSON `ApiError`.
Success and 406 carry `Vary: Accept`; API responses retain `Cache-Control: no-store` and
`X-Content-Type-Options: nosniff`. Encoding can fail after a committed write, which remains
ambiguous JSON `503 unavailable`, never a definitive no-write claim or codec retry.

The Studio sends once and decodes success by actual response Content-Type. An unexpected or
undecodable success, timeout or cancellation retains the pending operation/snapshot for explicit
same-operation reconciliation. JSON remains the shipped default. `revision-protobuf` is an
explicit build feature for only these four routes; JSON success at lower Accept quality is
server negotiation within the same request, not a second write.

## Compatibility, rollout and rollback

1. Keep the JSON default until independent schema/goldens, real disposable PostgreSQL replay,
   real browser save/reload/proxy/cancellation, target-platform and measurement gates are accepted.
2. Roll back by rebuilding without `revision-protobuf`; no data conversion or migration exists.
   Host JSON compatibility tests have run; deployment/browser rollback has not been observed.
3. Retain legacy server MessagePack read/write and its existing tests. Consumers have not been
   proven absent; add no new MessagePack client.
4. Retire legacy support only after consumer inventory, owner-approved zero-request observation
   period, published notice and executed JSON rollback. This change satisfies none of those
   retirement prerequisites and removes no historical evidence.
5. Any field addition/type change requires schema/version review, adapters, both-language fixtures
   and negotiation. Never reuse field numbers/names; removed numbers and names must be reserved.

## Verification through the existing command surface

`cargo xtask verify contracts` runs the independent Python golden encoder, pinned isolated
prost/protoc schema drift oracle, contract/runtime tests, Script IR/WAV references and real
owned Opus inspection. `protobuf-reference` is also in full preflight and the existing Rust CI
job; no paid/new CI lane is introduced. Cargo tracks the external `.proto` before generation,
so cached builds cannot silently retain a prior schema.

Changed schema, Protobuf fixtures or Rust adapters select both `protobuf-reference` and
`theatre-cmp-demo`; the latter embeds unchanged corpus bytes and executes commonTest on JVM.
Web verification remains independent of Gradle. Android/iOS compile/runtime remain separately
reported; shared source availability and JVM output cannot establish Native acceptance.

The [working method](../../.agents/skills/cantos-engineering/references/wire-codec.md) owns the
review/test workflow. Payload bytes, compression, CPU, allocation, bundle and network latency
are independent measurements. Format choice establishes no blanket speedup.
