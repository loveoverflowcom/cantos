> Historical adapter-2 evidence. The current adapter and fresh local checks are recorded in
> [provider verification](tts-provider-verification.md); this record is not current live evidence.

# Google Cloud TTS adapter evidence

Review boundary: part of [#6](https://github.com/loveoverflowcom/cantos/issues/6). One commercial
provider, Google Cloud Text-to-Speech `v1/text:synthesize` for vi-VN, on the shared provider
pattern of [ADR 0008](../decisions/0008-local-tts-inspection.md): Google voices in the casting
catalog, an exact adapter on each frozen line, a pure planning/classification/admission core,
a one-request HTTPS shell, an opt-in probe that consumes a frozen snapshot, and the existing
`cantos-tts-receipt-1` audit. Provider choice and the mocked-only scope were decided by the
repository owner on 2026-10-11; [ADR 0011](../decisions/0011-google-cloud-tts-adapter.md) is
proposed. Base: `83eab60` on `develop`, with the first adapter slice `ea8d4d0`.

Delivers: four Google capabilities in the catalog; casting resolution that pins
`google-cloud-tts-synthesize-2` / `wav-pcm-s16le-mono-24000-v1` and requires an explicit neutral
delivery and admissible text; the planner and its exact request; classification into receipt-v1
classes and delivery states with private exact evidence; the `cantos-google-tts-pcm-1` audio
profile and its audit selection; the production v1 JSON Schema covering the eSpeak and Google
identities; and `probe-google-tts`. The first slice's standalone `synthesis` module and
`google-tts-sample` command were replaced. Does not deliver: a live Google call or audio, recorded
provider envelopes, a Studio way to enter the explicit intensity 0, PostgreSQL attempts,
reservations, settlement, leases, retry loops, speech-cache fingerprints, storage or any
publication path. Script IR, migrations, routes and the Studio code are unchanged.

## Invariant and ownership

Nothing reaches Google unless the line was frozen for this exact adapter with an explicit
neutral delivery, re-admitted by the production authority and re-checked against every declared
capability; and every observation after a connection opened is `may_have_executed`, so the
shared planner never advises a resend. A body becomes audio only through the shared inspector's
Google profile. No record contains a credential, dialogue text or a provider error message.

[`tts/google_contract.rs`](../../services/cantos-api/src/tts/google_contract.rs) is pure identity used by
casting and the audit. [`tts/google.rs`](../../services/cantos-api/src/tts/google.rs) is pure: no
network, clock, credential or file access. [`tts/google_http.rs`](../../services/cantos-api/src/tts/google_http.rs)
is the shell and sends exactly once per call. [`probe-google-tts`](../../services/cantos-api/src/bin/probe-google-tts.rs)
is the only caller. Dependency edges point inward: `production.rs` and the receipt audit depend on
the contract module only; the core depends on `cantos-api`, `tts::attempt` and `tts::audio`,
never on reqwest. The transport port has two implementations: the reqwest shell and the
test-only `ScriptedTransport`.

## Ledger

Levels use the [foundation vocabulary](../../.agents/skills/cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified).
Everything below is **mocked**: synthetic bodies, a scripted transport or a scripted loopback
server. Nothing touched Google.

| Claim | Source | Failure mode | Oracle | Level held |
| --- | --- | --- | --- | --- |
| Declared voices are the documented 40 vi-VN voices, unique, each in the family its name states | Google voice list, 2026-10-11 | casting a nonexistent voice or wrong family | name-segment family table; counts 30/2/4/4 | example-tested (declaration: documented) |
| The catalog lists four Google capabilities that never claim reference status or dispatch | ADR 0011 | a planning catalog read as billable | exact provider/model/voice-count tuples; flags | example-tested |
| Inherited Script IR emotion is refused; only an explicit neutral/0 override freezes | business rule 8 | silent performance substitution | four exact `unsupported_control` issues, then a frozen document whose every line pins the Google adapter | example-tested |
| Rate, pitch, voice, model and language follow each family's declared range | Google AudioConfig, voice list | Chirp3-HD sent controls it lacks | settings issues at both bounds and per family | example-tested |
| Google's 5,000-byte limit and text admission are enforced before freezing and planning | Google quotas | rejection after a paid send | `google_text_too_large` on 5,001 bytes; planner `TextTooLong`/`InvalidText` incl. U+2028, control, non-NFC; 5,000 bytes plans | example-tested |
| Request body and digest are exact and deterministic | ADR 0011 | identity drift; unsupported fields sent | golden bytes and digests computed independently with Python `json`/`hashlib`; five single-field mutations change the digest | differentially-tested (body, digest); example-tested (mutations) |
| A line frozen for another adapter or adapter version is refused | ADR 0011 | reinterpreting reference or v1 inputs | exact findings for reference identity and `google-cloud-tts-synthesize-1` | example-tested |
| Estimate is list price, rounded up per request, never a charge | Google pricing; business rule 17 | estimate shown as settled cost | 41/1,667/2,500/2,501/5,000-character cases; receipt `charge: unknown` | example-tested |
| Each observation maps to one receipt class and delivery state with exact evidence | ADR 0011; receipt v1 | lost error detail or a false not-sent claim | 13-row table: not sent, timeout, reset, 302/400/401/403/429/500/502/503/504, 2xx defect | example-tested |
| No failure after a connection opened permits a resend; a connection that never opened needs fresh authorization | business rule 19; receipt v1 planner | duplicate billing | scripted fake with its own execution ledger; shared `audit_receipt` recovery for lost, 429, 503, 400, 2xx defect and not-sent | fault-injected (fake transport, mocked) |
| Malformed audio is rejected with an exact defect | issue #6 criterion 3 | accepting empty, truncated, stereo, wrong-rate, 8-bit, float, frameless or over-120-second audio | code-built WAV headers; exact `AudioDefect`/`AudioError` per case | example-tested |
| The shared audit admits Google output only for the exact Google identity and profile | receipt v1 | eSpeak-rate or other-voice bytes admitted | `audit_receipt` accepts 24 kHz, rejects 22,050 Hz and a mismatched voice; `inspect-tts-receipt` CLI accepts a Python `wave`-written 24 kHz file and rejects 22,050 Hz | example-tested; differentially-tested (independent WAV writer) |
| The published schema covers every implemented catalog identity | contracts | clients rejecting valid server responses | real catalog, Google settings request, preview and frozen document validate; the same test fails against the previous schema | example-tested |
| Connect failure is `NotSent`; timeout or reset after the request was written is lost | provider-adapters § 3 | an ambiguous call treated as free to repeat | real reqwest client against a scripted loopback TCP server | example-tested (real HTTP client, mocked server) |
| The transport sends the exact body and credential headers, follows no redirect and bounds the body | ADR 0011 | wrong bytes, auth to another host, memory blow-up | loopback capture; 302 reported; declared and chunked oversize; truncated body | example-tested (mocked server) |
| An HTTPS-only transport opens no connection to a plain-HTTP URL; credentials never appear in `Debug` | business rule 12 | token in clear text or logs | loopback listener accepts nothing; sentinel token absent | example-tested |
| Observations never contain dialogue text, provider messages or audio payload | business rule 12 | private text leaked into records | sentinel text and message absent from serialized observations | example-tested |
| The probe sends nothing without `--live`, checks the cap before reading a token, and refuses a tampered snapshot | ADR 0011 | accidental paid call | CLI runs below; no directory created | example-tested (manual CLI runs) |
| Google returns playable vi-VN audio that this admission accepts | issue #6 criterion 2 | header or format mismatch only visible live | opt-in `probe-google-tts --live` | NOT_RUN by owner decision |
| Real Google error envelopes classify as declared | provider-adapters § 7 | mapping drift from documentation | recorded envelopes with capture date | NOT_RUN (no live session) |

## Commands that ran

Local toolchain `rustc 1.96.1`; the CI-pinned `1.87.0` was used for the workspace gates.

| Command | Result |
| --- | --- |
| `cargo +1.87.0 fmt --all -- --check` | pass |
| `cargo +1.87.0 clippy --workspace --all-targets --locked -- -D warnings` | pass |
| `cargo +1.87.0 test --workspace --locked` | pass: 341 passed, 52 ignored (PostgreSQL and opt-in tests, as by default) |
| `cargo +1.87.0 test -p cantos-server --locked --test google_tts` | pass: 16 |
| `cargo +1.87.0 test -p cantos-server --locked --lib tts::google_http` | pass: 12 |
| `cargo +1.87.0 test -p cantos-server --locked --test production` | pass: 18, including the new Google casting test |
| `cargo +1.87.0 test -p cantos-server --locked --test production_contract` | pass: 3; the new schema test fails against the previous schema |
| `cargo +1.87.0 test -p cantos-server --locked --test tts_receipt` / `--test tts_audio` | pass: 8 (1 ignored) / 14, unchanged |
| `probe-google-tts` on a Google frozen document: dry run, unknown dialogue, cap 0, `--live` without a token, tampered resolved text, two usage errors | exit 0, 2, 1, 2, 1 (`scope_rejected`/`invalid_document`), 2, 2; the dry-run digest equals the golden value; no private directory created |
| `inspect-tts-receipt` on that document with synthetic receipts and Python-written WAVs | 24 kHz admitted as `cantos-google-tts-pcm-1` (exit 0); 22,050 Hz `unsupported_profile` (exit 1) |
| `python3 scripts/check_repository.py`; `python3 -m unittest discover -s scripts -p 'test_*.py'`; `python3 scripts/reference_script_ir.py`; `git diff --check` | pass; 80 Python tests |
| `cargo clippy -p cantos-studio --target wasm32-unknown-unknown --locked -- -D warnings` (stable 1.96.1; 1.87.0 has no wasm32 target here); `NO_COLOR=true trunk build --locked` from `apps/web` | pass (compile evidence only; Studio code unchanged) |

The frozen document for the CLI runs was produced by a temporary local test that was deleted
and not committed. `cargo clippy --workspace --all-targets` on the local 1.96.1 toolchain reports
two `cloned_ref_to_slice_refs` lints in the pre-existing `tests/wav_inspection.rs`; the pinned
1.87.0 gate passes.

## Not run

- Any call to Google, live audio, `provider-live-tested` or `audio-listened` evidence. The owner
  chose mocked-only; the probe is ready for an operator-paid run.
- PostgreSQL suites (`python3 scripts/test_postgres.py`): no PostgreSQL binaries on this host's
  `PATH`, and this slice changes no SQL, migration or route. The ignored database tests did not
  run; one of them counts reference capabilities, which are unchanged.
- Studio runtime: Studio code is unchanged, and no browser walkthrough of the larger catalog
  ran.
- Mutation testing and fuzzing of the new decoder path.

## Residual risk

- The declaration, prices and error mapping are documentation-tested only. Google may answer
  with a WAV header the inspector rejects, bill rejected or rate-limited requests, or change
  voices or prices without notice.
- Superseded: Studio could not record the explicit intensity `0` for capabilities without
  intensity when this record was written. The [Studio casting evidence](studio-provider-casting.md)
  records the correction.
- Receipt v1 records a refused connection as `timeout` and 503 as `permanent`; the exact cause
  is only in the private `observation.json`.
- The 5,000-byte check covers the text only. If Google counts other request bytes against the
  limit, a line near it is rejected after a send.
- Billable characters are counted as Unicode scalar values of NFC text; Google does not define
  "character" for Vietnamese combining sequences, so the estimate may differ from the bill.
- No frozen line can be dispatched in production: #7 must add durable intent, reservation and a
  current approval check before any send.
- Draft [#26](https://github.com/loveoverflowcom/cantos/pull/26) also extends the catalog,
  resolution, audio profiles and receipt audit, and also uses ADR number 0009; whichever merges
  second must renumber and rebase.
