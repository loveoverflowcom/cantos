# Wire codec: JSON and versioned binary representations

Use for Content-Type/Accept negotiation, contract codec helpers, generated wire adapters,
client codec switches, schema/goldens or a proposed extension to another route/platform.
[Wire architecture](../../../../docs/architecture/wire-codec.md) and the frozen
[versioned policy](../../../../packages/contracts/protobuf/README.md) own protocol rules.
[ADR 0023](../../../../docs/decisions/0023-versioned-protobuf-wire.md) owns the new binary choice;
[ADR 0013](../../../../docs/decisions/0013-optional-messagepack-wire.md) and
[MessagePack evidence](../../../../docs/evidence/messagepack-wire.md) remain historical authority.
Do not rewrite historical results or duplicate the protocol tables here.

## Working method

1. Name the actual consumer, route, DTO and measured cost. A binary format is not a product API
   expansion. Default JSON; errors are the unchanged JSON ApiError. No binary storage/digest/auth
   migration, invented Theatre client or Studio Mobile.
2. Keep existing Serde DTOs as application values. Protobuf has one versioned `.proto` authority,
   private generated edge messages and exhaustive fallible adapters. Generic wire helpers refuse
   arbitrary Serde-to-Protobuf; only reviewed roots implement ProtobufDto. Legacy MessagePack
   retains named, representation-neutral Serde maps and independent historical goldens.
3. Review required scalar presence, unknown/reserved/duplicate fields, canonical varints, uint64,
   UTF-8, size/depth/item/field budgets and checked arithmetic before runtime decoding allocates
   message/string/list data. Missing scalars must never manufacture authenticated or validated facts.
4. Derive golden bytes from the independent Python wire-spec encoder; compare generated prost
   and Kotlin against those bytes/values. Never bless output from production. Schema generation
   must track the external authority on warm builds as well as compare exact generated text.
5. Settle Origin/session, Accept, Content-Type and body admission before store effects. Inspect
   wildcard specificity, q=0 exclusions, aliases and every approved/refused parameter. Probe
   counters and real SQL row counts establish different boundaries.
6. Send each client operation once. Decode by actual response Content-Type; undecodable success,
   timeout and cancellation remain ambiguous. Preserve actor/base/operation/snapshot/draft fences
   and reconcile the same operation explicitly. Never change codec and resend a failed write.
7. Run the cheapest focused production-code regression first, then applicable xtask gates.
   `verify contracts` and `protobuf-reference` establish Rust/Python schema and codec evidence;
   `theatre-cmp-preview` runs embedded common/JVM vectors. Source/fixture changes select both.
8. Record Android and iOS compile/test/runtime independently. Kotlin/JVM, published KMP artifacts,
   a host render or a linked Apple framework cannot substitute for Native/device execution.
9. Measure raw/actual gzip bytes and native encode/decode with spread for 1/8/64/512 KiB DTOs;
   distinguish allocation/heap, matched WASM bundle and browser CPU from actual request latency.
   Never infer CMP speedup from Rust, compression in a fixture from HTTP compression, or zero
   allocation from a bounded prescan. Keep JSON default until activation gates and rollback pass.
10. Retain legacy server MessagePack until consumer inventory, owner-approved zero-use period,
    published retirement notice and executed JSON rollback are evidenced. Add no new legacy client.

<a id="3-test-matrix-for-any-codec-change"></a>
<a id="5-extending-to-theatre-and-cmp"></a>

## Required test/review matrix

| Claim | Cheapest evidence | Additional acceptance |
| --- | --- | --- |
| actual four pilot DTOs agree | independent Python/prost/Kotlin byte/value goldens | every shipping Kotlin target |
| generated types match schema | exact isolated generated-text comparison, warm-schema-change refusal | version evolution review |
| hostile input/presence is bounded | exact-category corpus, prefixes, finite byte/boundary loops | named fuzzing only if actually run |
| headers refuse before effects | pure negotiation + production extractor/router counter | real PostgreSQL row counts |
| JSON/auth/Origin/cookies preserved | exact JSON and lazy-store router tests | real authorized SQL/HTTP journey |
| replay ignores representation | both-direction same-operation tests | lost-response, host kill and database restart |
| actual Content-Type and draft survive ambiguity | production client wire and reducer tests | driven browser fetch/cancel/proxy/save/reload |
| rollout cost/rollback acceptable | attributable per-format/platform measurements | real browser/deployment rollback |

Use the foundation evidence vocabulary and exact xtask PASS/FAIL/NOT_RUN/UNSUPPORTED outcomes.
An ignored database test that merely compiled is NOT_RUN. A finite byte loop is not fuzzing or
formal proof. Preserve fail-before evidence and explain baseline/environment/flaky failures.
The [migration ledger](../../../../docs/evidence/protobuf-wire-migration.md) records current scope
and deferred acceptance. A new consumer needs its own reviewed versioned contract, fixtures,
measurement and activation decision; the fixture-only CMP harness grants no network authority.
