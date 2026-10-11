# Google Cloud TTS adapter evidence

Review boundary: part of [#6](https://github.com/loveoverflowcom/cantos/issues/6). One commercial
provider adapter, Google Cloud Text-to-Speech `v1/text:synthesize` for vi-VN, with a pure
capability, mapping, classification and audio-validation core, a one-request HTTPS shell and an
opt-in operator sample command. Base: `e3d047c` on `develop`. Provider choice and the
mocked-only scope were decided by the repository owner on 2026-10-11;
[ADR 0009](../decisions/0009-google-cloud-tts-adapter.md) is proposed.

Delivers: dated vi-VN capability declaration, blocking pre-send findings, exact request body and
digest, list-price estimate, send-boundary classification with charge exposure, retry decision
and recovery action, returned-audio validation, redacted credentials, an HTTPS-only bounded
transport and `google-tts-sample`. Does not deliver: a live Google call or audio, recorded
provider envelopes, Studio or casting-catalog integration, an emotion/intensity mapping,
PostgreSQL attempts, reservations, settlement, leases, retry loops, speech-cache fingerprints,
storage, a local/open-source provider or any publication path. Script IR, migrations, routes,
contracts and the Studio are unchanged.

## Invariant and ownership

Nothing reaches Google unless every declared capability check passes, and every observation
after a send is classified so that a possibly billed outcome is never repeated automatically. A
returned body becomes audio only if it is mono 16-bit integer PCM at 24 kHz with at least one
frame. No record contains a credential, dialogue text or a provider error message.

[`synthesis/mod.rs`](../../apps/server/src/synthesis/mod.rs) holds the provider-neutral
vocabulary. [`synthesis/google.rs`](../../apps/server/src/synthesis/google.rs) is pure: no
network, clock, credential or file access. [`synthesis/google_http.rs`](../../apps/server/src/synthesis/google_http.rs)
is the shell and sends exactly once per call. [`google-tts-sample`](../../apps/server/src/bin/google-tts-sample.rs)
is the only caller. Dependency edges point inward: the shell and the command depend on the
core; the core depends on `cantos-api` wire types and `wav_inspection`, never on reqwest. The
transport port has two implementations: the reqwest shell and the test-only `ScriptedTransport`.

## Ledger

Levels use the [foundation vocabulary](../../.agents/skills/cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified).
Everything below is **mocked**: synthetic bodies, a scripted transport or a scripted loopback
server. Nothing touched Google.

| Claim | Source | Failure mode | Oracle | Level held |
| --- | --- | --- | --- | --- |
| Declared voices are the documented 40 vi-VN voices, unique, each in the family its name states | Google voice list, 2026-10-11 | casting a nonexistent voice or the wrong model family | name-segment family table; counts 30/2/4/4 | example-tested (declaration reviewed against docs: documented) |
| Every unsupported control is an exact blocking finding, never dropped or substituted | business rule 8; ADR 0009 | silent neutral delivery, rate/pitch on Chirp3-HD, unknown voice | table over language, voice, model, rate, pitch, emotion (including `neutral`), intensity, text | example-tested |
| Lines frozen for the #5 reference catalog cannot be dispatched to Google | ADR 0007/0009 | reinterpreting reference inputs as billable | `plan_resolved` exact findings; Google-tagged frozen line blocks on Script IR emotion | example-tested |
| Request body and digest are exact and deterministic | ADR 0009 | cache/attempt identity drift; unsupported fields sent | golden bytes and digest computed independently with Python `json`/`hashlib`; equal lines plan equal; five single-field mutations change the digest | differentially-tested (body, digest); example-tested (mutations) |
| 5,000-byte text limit is exact | Google quotas | provider rejection after a send | 5,000 bytes plans, 5,001 bytes blocks | example-tested |
| Estimate is list price, rounded up per request, never a charge | Google pricing; business rule 17 | estimate shown as settled cost | 41/1,667/2,500/2,501/5,000-character cases across families | example-tested |
| Each observation maps to one class, charge, retry and recovery | ADR 0009; AIP-193/194 | blind retry, wrong recovery, lost error class | 15-row table incl. not sent, timeout, reset, 400/401/403/404/429/500/502/503/504/302, interrupted error body, 2xx defect | example-tested |
| No class with an unknown charge permits an automatic repeat | business rule 19; PL5 | duplicate billing | exhaustive loop over all eight classes | example-tested (exhaustive over the finite domain) |
| A response lost after execution is recorded once and never resent | business-rule invariant "provider response is lost" | second paid call | scripted fake with its own execution ledger: one execution, `ambiguous`, `resolve_before_retry`, `charge: unknown` | fault-injected (fake transport, mocked) |
| Malformed audio is rejected with an exact defect | PL12; issue #6 criterion 3 | accepting empty, truncated, stereo, wrong-rate, 8-bit, float or frameless audio | code-built WAV headers; exact `AudioDefect` per case | example-tested |
| Valid audio facts are exact | `wav_inspection` contract | wrong checksum or frame count recorded | SHA-256 of the generated bytes; frames 2,400 | example-tested |
| Connect failure is `not_sent`; timeout or reset after the request was written is lost, not unsent | provider-adapters § 3 | an ambiguous call treated as free to repeat | real reqwest client against a scripted loopback TCP server: refused port, stall past the deadline, close after request | example-tested (real HTTP client, mocked server) |
| The transport sends the exact body with the credential and project headers once, and follows no redirect | ADR 0009 | wrong bytes or auth leaked to another host | loopback server captures the request; 302 is reported, not followed | example-tested (mocked server) |
| Oversized or cut-off bodies are not buffered without bound | PL12 | memory blow-up on hostile responses | declared and chunked oversize; truncated body | example-tested (mocked server) |
| An HTTPS-only transport opens no connection to a plain-HTTP URL | ADR 0009 | token sent in clear text | loopback listener accepts nothing; observation `not_sent` | example-tested |
| Credentials and provider messages never reach a record or `Debug` output | business rule 12; PL10 | leaked token or private text | sentinel token and sentinel message absent from `Debug` and serialized records | example-tested |
| The sample command sends nothing without `--live`, and checks the cap before reading a token | ADR 0009 | accidental paid call | dry run, blocked line, missing token and cap 0 runs; no output file created | example-tested (manual CLI runs, below) |
| Google returns playable vi-VN audio that this validation accepts | issue #6 criterion 2 | header or format mismatch only visible live | opt-in `google-tts-sample --live` | NOT_RUN by owner decision |
| Real Google error envelopes classify as declared | provider-adapters § 7 | mapping drift from documentation | recorded envelopes with capture date | NOT_RUN (no live session) |

## Commands that ran

Local toolchain `rustc 1.96.1`; the CI-pinned `1.87.0` was used for the workspace gates.

| Command | Result |
| --- | --- |
| `cargo +1.87.0 fmt --all -- --check` | pass |
| `cargo +1.87.0 clippy --workspace --all-targets --locked -- -D warnings` | pass |
| `cargo +1.87.0 test --workspace --locked` | pass: 287 passed, 50 PostgreSQL tests ignored as by default (257 before this slice; 12 new transport unit tests and 18 new adapter tests) |
| `cargo test -p cantos-server --locked --test google_tts` | pass: 18 |
| `cargo test -p cantos-server --locked --lib synthesis` | pass: 12 |
| `python3 scripts/check_repository.py` | pass |
| `python3 -m unittest discover -s scripts -p 'test_*.py'` | pass: 80 |
| `python3 scripts/reference_script_ir.py` | pass |
| `git diff --check` | pass |
| `cargo clippy -p cantos-studio --target wasm32-unknown-unknown --locked -- -D warnings` (stable 1.96.1; 1.87.0 has no wasm32 target installed here) | pass; the Studio is unchanged, run because `Cargo.lock` changed |
| `NO_COLOR=true trunk build --locked` from `apps/web` | pass |
| `google-tts-sample --line <sample>` and the blocked, missing-token, cap-0 and usage-error cases | outputs and exit codes 0, 1, 2, 1, 2 as documented; no output file created |

`cargo clippy --workspace --all-targets` on the local 1.96.1 toolchain reports two
`cloned_ref_to_slice_refs` lints in the pre-existing `tests/wav_inspection.rs`, unchanged by this
slice; the pinned 1.87.0 gate passes.

## Not run

- Any call to Google, live audio, `provider-live-tested` or `audio-listened` evidence. The owner
  chose mocked-only; the command above is ready for an operator-paid run.
- PostgreSQL suites (`python3 scripts/test_postgres.py`): no PostgreSQL binaries on this host's
  `PATH`, and this slice changes no SQL, migration or route. The ignored database tests did not
  run.
- Mutation testing and fuzzing of the new decoder path; `wav_inspection` itself was not changed.
- Studio, mobile, screen-reader or browser runs: no UI change.

## Residual risk

- The declaration, prices and error mapping are documentation-tested only. Google may answer
  with a WAV header the inspector rejects (for example an extensible or streaming-size header),
  bill rejected or rate-limited requests, or change voices or prices without notice.
- The 5,000-byte check covers the text only. If Google counts other request bytes against the
  limit, a line near it is rejected after a send as `invalid_input` with an unknown charge.
- Billable characters are counted as Unicode scalar values of NFC text; Google does not define
  "character" for Vietnamese combining sequences, so the estimate may differ from the bill.
- Every 429 and 5xx requires an explicit resolution before a repeat; at volume this needs the
  Studio resolution flow from #7/#8.
- No frozen production line can dispatch yet: casting needs Google capabilities and an explicit
  emotion/intensity mapping, and #7 must persist intent and reserve budget before any send.
- The local/open-source provider category in #6's original acceptance text is superseded in the
  work plan, but the issue text still lists it; the owner must reconcile the issue.
