# Wire codec: JSON and MessagePack representations

> **Scope.** Decide whether a route, surface or client may carry a contract DTO as MessagePack
> beside JSON, and change, test, benchmark, roll back and review that safely. Use when touching
> `Content-Type`/`Accept` handling, `cantos_contracts::wire`, the server's `http/wire.rs`, a
> client codec switch, serde attributes of a DTO on a negotiated route, the MessagePack goldens,
> or when extending the codec to more routes, Cantos Theatre or the CMP app.

The protocol itself (scope, media types, negotiation tables, limits, error policy, client rules,
rollback) is owned by [docs/architecture/wire-codec.md](../../../../docs/architecture/wire-codec.md),
the decision by [ADR 0013](../../../../docs/decisions/0013-optional-messagepack-wire.md) and the
measurements by the [evidence record](../../../../docs/evidence/messagepack-wire.md). Link them;
never restate the tables. Handler and DTO shape stay with
[`http-api-boundary.md`](http-api-boundary.md). This reference owns the working method.

## 1. JSON or MessagePack

| Situation | Representation | Why |
|---|---|---|
| any route, client or tool by default | JSON | every client, proxy, script and person can read it |
| errors, on every route | JSON `ApiError`, existing codes | a gateway answers JSON anyway; one error path |
| a route whose payload is large and text-dominated, with a client that measured a real cost | MessagePack negotiated beside JSON | it stops JSON from escaping embedded text a second time |
| small metadata, session, uploads, downloads, Python/CLI tools, listener routes today | JSON | gains are within noise or a contract change would be needed |
| persistence, object storage, Script IR files, digests, logs | never MessagePack | canonical bytes and stored records are not a transport choice |
| positional/tuple encodings | never | an omitted or added field silently shifts every later field |

A binary-looking field does not justify MessagePack by itself: `original_bytes` is a JSON number
array, and carrying it as MessagePack `bin` would change the contract for both representations.

## 2. Rules

| Rule | Failure it prevents | Good | Counterexample | Oracle | Status / exception |
|---|---|---|---|---|---|
| **W1 One DTO, two representations.** DTOs stay in `cantos-contracts`; no MessagePack-only struct, field or default | the representations drift and a client decodes a value the other never sends | `RevisionResponse` used by both paths | a `RevisionPack` struct with shortened keys | JSON ↔ MessagePack equivalence over every fixture of the route | example-tested in `msgpack_wire.rs`; no exception |
| **W2 Named, representation-neutral serde.** Named maps only; no type whose serde form depends on `is_human_readable`; no float, `bin` or extension value; tagged enums only with an equivalence test | positional shifts; a value that is a string in JSON and bytes in MessagePack | `rmp_serde::to_vec_named` through `wire::encode` | `rmp_serde::to_vec` (positional); a `chrono`/`uuid` type in a DTO | goldens from the independent Python encoder equal Rust bytes | differentially-tested; a new DTO type needs its own golden or equivalence row |
| **W3 Contract changes stay contract changes.** Adding MessagePack to a route is additive negotiation; any DTO change still follows its contract's versioning, updates the JSON fixture and regenerates goldens deliberately in the same change | goldens silently re-blessed to match a broken encoder | edit fixture → `python3 scripts/reference_msgpack.py --write-goldens` → review the diff | editing `msgpack_hex` by hand or regenerating from Rust output | `scripts/test_reference_msgpack.py` and the Rust golden test both fail on drift | example-tested; changing a media type, encoding or error rule needs an ADR |
| **W4 Bounded decoding only through `wire::decode`.** Buffered slice, nesting bound, exactly one value, top-level map | stack exhaustion, huge preallocation, smuggled trailing data | `wire::decode(WireFormat::MessagePack, &bytes)` after the body limit | `rmp_serde::from_read(stream)`; `rmp_serde::from_slice` without a depth bound | malformed, every-prefix truncation, trailing, invalid UTF-8, depth and huge-length tests | example-tested; no exception |
| **W5 Negotiate in the shell, before the store.** `Accepts` and `WireBody` extractors settle the representation; `respond` is the only MessagePack response path; failures stay JSON `ApiError` | a `406` after a committed save; a MessagePack error a client cannot read | extractor rejection before the handler body | encoding after `store.save` and returning `406` on failure | probe-router call counter; PostgreSQL row counts unchanged after `406`/`415`/`400` | example-tested and integration-tested (CI) |
| **W6 The codec never touches authority.** Origin, session and authorization run first and decide identically; the idempotency key ignores the representation | a MessagePack path that skips a guard; a retry in the other codec creating a second revision | same middleware; replay of a lost MessagePack response through JSON returns the first revision | a handler variant per codec | parity tests per decision; cross-representation replay test | integration-tested (CI) and DB-free router test |
| **W7 No codec fallback, one switch per client.** A request is sent once; an undecodable success is the ambiguous `unavailable`; only a constant changes the codec, and its default is JSON until evidence exists | a timed-out save retried as JSON and applied twice | `REVISION_CODEC` in the client's `wire.rs` | `on error → resend with Content-Type: application/json` | client host tests on `decode_response`; review | example-tested; rollback is flipping the constant |
| **W8 Decode responses by their own `Content-Type`.** Requests declare; responses are sniffed; a JSON build never links the MessagePack decoder | a JSON error or proxy page read as MessagePack garbage | `decode_response(REVISION_CODEC, ok, content_type, body)` | decoding by what was requested | host tests; bundle measurement for the JSON build | example-tested; bundle check is manual |

## 3. Test matrix for any codec change

Every row is required unless the change cannot affect it; state the reason when skipping one.

| Claim | Test | Evidence level | Runs in |
|---|---|---|---|
| each route DTO is equal through JSON and MessagePack | fixture round trips | example-tested | `cargo xtask verify contracts` |
| bytes match an implementation that is not `rmp-serde` | goldens from `scripts/reference_msgpack.py` | differentially-tested | `verify repository` (Python) and `verify contracts` (Rust) |
| hostile bodies are refused, bounded | malformed, every strict prefix, trailing, invalid UTF-8, depth, huge length | example-tested (prefixes: bounded exhaustive) | `verify contracts` |
| headers select the codec | pure `Content-Type`/`Accept` table incl. quality, specificity, `406`, `415` | example-tested | `verify rust` |
| refusals never reach the handler; JSON responses are byte-identical | probe router with a call counter | example-tested | `verify rust` |
| Origin and session refusals are the same JSON for both codecs | real router, lazy store | example-tested | `verify contracts` |
| authorization, stale and `operation_reused` are identical | PostgreSQL harness | integration-tested | `verify postgres` (CI) |
| a lost response replays in either codec without a second write | PostgreSQL harness and the live socket after a host kill and database restart | integration-tested, fault-injected | `verify postgres` (CI) |
| the client decodes by `Content-Type` and treats an undecodable success as ambiguous | host tests on the client `wire.rs` | example-tested | `verify rust` |
| the built WASM client sends and decodes the bytes in a browser | a driven browser run against a real or stub server | interaction-tested; name the backend | manual; `NOT_RUN` unless it ran |

A fake backend is never `integration-tested`. A PostgreSQL gate that only compiled or was skipped
is `NOT_RUN`/`UNSUPPORTED`, not passing.

## 4. Benchmark before changing a default

1. **Payloads.** Real contract DTOs: small metadata, a bounded history page and Script IR at
   about 1, 8, 64 and 512 KiB, with Vietnamese text, curly quotes, `"`, `\`, emoji and decomposed
   marks. Reuse `services/cantos-api/tests/wire_bench.rs`; extend it for a new route.
2. **Measure.** Raw and gzip bytes; native encode/decode as a median with spread; the same
   payloads inside a WASM build in a browser (`CANTOS_WIRE_PAYLOADS` exports them); release
   bundle raw and compressed for the base branch, the JSON switch and the MessagePack switch;
   end-to-end latency only when a real backend and network path exist.
3. **Label.** One machine, its CPU, toolchain, browser and run count. No confidence interval
   unless you computed one.
4. **Decide.** Turn a client default on only when the gain is material on target payloads after
   compression, the bundle cost is acceptable, the full matrix ran including PostgreSQL, and the
   rollback was executed once. Otherwise keep JSON and record why. Never claim MessagePack is
   faster without these numbers.

## 5. Extending to Theatre and CMP

1. Name the consumer, the route and the measured problem. No route gains MessagePack for symmetry.
2. Keep the DTOs in `cantos-contracts`; add JSON fixtures and add each DTO to
   `reference_msgpack.py`'s cases; regenerate and review the goldens.
3. Server: give the route `Accepts`, `WireBody` and `respond`; add its rows to the parity and
   PostgreSQL tests; update the scope table in the architecture document.
4. Theatre Web: reuse `cantos_contracts::wire` and the Studio client pattern (one constant,
   decode by `Content-Type`) inside the Theatre package; never import Studio code. Listener rules
   (progress revisions, idempotency, offline outbox) stay with
   [`cantos-listening`](../../cantos-listening/SKILL.md); stored offline data is not a wire format.
5. CMP: choose the Kotlin MessagePack library by ADR; decode by response `Content-Type`; write
   named maps; pass the committed goldens and the malformed-input table in `commonTest`; keep a
   JSON default switch. Do not generate bindings or add a fake client before a CMP consumer exists.
6. Run the matrix and the benchmark on that platform (a named device, not a desktop host), then
   update the evidence record.

## Review questions

1. Does any path decode MessagePack outside `wire::decode`, or encode outside `wire::encode`/`respond`?
2. Can a representation refusal or encoding failure happen after the store call?
3. Is every error still a JSON `ApiError` with an existing code?
4. Did a DTO change regenerate goldens from the Python oracle, with the JSON fixture in the same diff?
5. Does any client retry, fall back or switch codec after a failure?
6. Is the client default still JSON, and does the evidence record justify any change to it?
7. Are PostgreSQL and browser results reported as they actually ran?
