# ADR 0013: Optional MessagePack representation for Studio revision routes

- Status: accepted legacy foundation; superseded by ADR 0023 for new preferred binary clients.
  Existing MessagePack route compatibility is retained; this is not a protocol removal.
- Date: 2026-10-11
- Decision owner: Cantos maintainers
- Related issue or PR: branch `feat/studio-messagepack-wire`
- Superseded by: [ADR 0023](0023-versioned-protobuf-wire.md) for binary preference only;
  [current wire policy](../architecture/wire-codec.md) owns negotiation and rollout.

The decisions and measurements below describe the original MessagePack foundation. Its rejected
Protobuf alternative and proposed client switch are historical reasoning, not today's binary roadmap.

## Context

Studio revision traffic carries a complete Script IR document as the JSON string `script_json`.
JSON therefore escapes that text twice: every quote and backslash inside the document is escaped
again in the envelope. Save, head and pinned-revision bodies grow with the script (up to the 2 MiB
Script IR bound), and the browser encodes and decodes them on the main thread.

VOT Workspace already negotiates MessagePack beside JSON for its document API. Cantos wanted the
same capability as a reusable, tested foundation without copying VOT's error envelope, entity
tags or defaults, and without moving any route to a binary protocol mechanically.

Known facts: the API has no HTTP compression layer today; production edge compression is not
decided; no PostgreSQL binaries are installed on the authoring machine, so database-backed checks
run in CI only; no end-to-end latency environment exists.

## Decision

1. The DTOs in `cantos-contracts` remain the single source of truth. `cantos_contracts::wire`
   defines the byte representations: JSON, and MessagePack as named maps with bounded,
   exactly-one-value decoding. No domain, Script IR or persistence module imports it.
2. The server negotiates JSON or MessagePack, per request, on four Studio routes only: head,
   pinned revision, save revision and validation. `Content-Type` selects the decoder, `Accept`
   the encoder, `application/msgpack` is the only MessagePack media type, and JSON wins every
   ambiguous case. Errors stay JSON `ApiError` with existing codes; unsupported types are `415`/
   `406` with the existing `invalid_request` code. Negotiation completes before the store is
   called. The full protocol is in [the wire codec architecture](../architecture/wire-codec.md).
3. The Studio client has one switch, `REVISION_CODEC`, and it ships as **JSON**. Responses are
   decoded by their own `Content-Type`; nothing is retried in the other representation.
4. Validation was evaluated with the pilot routes: its payload is the same `script_json` text and
   its measured codec costs match save, and it precedes every editor save, so it is negotiated
   with the same switch.
5. Changing a client default, or adding a route, surface or client, requires the benchmark and
   test matrix in the engineering skill's
   [wire codec reference](../../.agents/skills/cantos-engineering/references/wire-codec.md).

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| JSON only (status quo) | nothing new to test or ship | double escaping and main-thread codec cost stay; no path to measure an alternative | rejected: the foundation is cheap to keep and makes a later, evidence-based switch a one-line change |
| Negotiated MessagePack on four routes, JSON default (chosen) | additive, reversible, same DTOs; server ready for any client | a second representation to test; +10 KB gzip on the JSON Studio build for the negotiated transport path | selected |
| Turn MessagePack on by default now | measured codec CPU savings in WASM | +25.6 KB gzip bundle; ~3 % size gain after gzip; no end-to-end or PostgreSQL evidence locally | rejected until the revisit conditions below are met |
| MessagePack for every route | one representation everywhere | uploads, tools, listener routes and small metadata gain nothing or need contract changes | rejected: mechanical migration is explicitly out of scope |
| HTTP compression only | shrinks JSON without a second representation | no CPU benefit; edge configuration undecided | complementary, not a substitute; compression is measured in the evidence |
| CBOR, Protobuf or positional MessagePack | standard alternatives, smaller positional bytes | new schema tooling or field-shift risk; no consumer asked for them | rejected |

## Consequences

- Product and user impact: none by default; the Studio still speaks JSON.
- Compatibility and migrations: additive. Old clients send no `Accept` and get identical JSON
  bytes; JSON responses on the four routes add only `Vary: Accept`. A non-JSON, non-MessagePack
  `Content-Type` on those routes is now `415` instead of `400` (both `invalid_request`). Nothing
  is stored in MessagePack; no migration.
- Operations, storage, and production cost: `rmp`/`rmp-serde` join the dependency graph; server
  CPU per request is unchanged or lower; no new infrastructure.
- Implementation and recovery requirements: rollback is the client constant; server support is
  permanent and inert without a MessagePack `Accept`/`Content-Type`.
- Deferred questions: production edge compression; an end-to-end latency measurement against
  PostgreSQL; a Kotlin MessagePack library for CMP; whether Theatre listener routes ever need it.

## Validation and revisit conditions

Evidence is recorded in [MessagePack wire evidence](../evidence/messagepack-wire.md): goldens from
an independent encoder, DTO equivalence, hostile-input, negotiation and parity tests, native and
in-browser WASM measurements, bundle sizes and two driven browser runs against a stub backend.
PostgreSQL-backed parity tests are implemented but did not run on the authoring machine.

Revisit the client default when all of these hold: PostgreSQL parity and replay tests pass in CI;
an end-to-end measurement with the production compression setting shows a material save or load
improvement on realistic script sizes; the bundle cost is accepted for that gain; and the
rollback has been executed against a real backend.
