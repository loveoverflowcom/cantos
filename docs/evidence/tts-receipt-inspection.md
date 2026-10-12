# Local TTS and receipt inspection evidence — issue #6

> **Historical (ADR 0018).** This evidence describes the in-process text-to-speech runtime that
> [ADR 0018](../decisions/0018-external-audio-production.md) removed on 2026-10-12. The code, tests
> and contracts it names no longer exist; links to them are pinned to commit `2de8c69`, the last
> commit that contained them. Nothing here describes current behavior.

This focused slice is **part of [#6](https://github.com/loveoverflowcom/cantos/issues/6)**.
The user explicitly authorized local TTS installation and synthesis. It implements one compact
local engine, scoped receipts and actual bounded WAV inspection. Commercial provider selection,
terms/account/voice permissions, budget and live samples remain pending; #6 is not complete.
[ADR 0008](../decisions/0008-local-tts-inspection.md) accepts local inspection selection only;
production adoption remains proposed. [Provider prerequisites](tts-provider-prerequisites.md)
separate official declarations from installed observations.

## Invariant, owner and boundary

**Invariant:** a local sample must retain exact frozen effective text/voice/control identity and
an actual validated output digest; uncertain execution or unknown money cannot become a retry
permission, free charge, accepted artifact or publication approval.

**Owner / boundary:** existing production rules reconstruct frozen documents; the pure
`tts::local_contract` owns one concrete local capability and text predicate. `tts::attempt`
decides recovery from typed facts, `tts::receipt` audits caller records, and `tts::audio` reuses
the WAV inspector. `tts::local` owns bounded child/filesystem effects. The explicit
`probe-local-tts` CLI writes private sample records; HTTP exposes only the planning descriptor.
No new service, generic provider framework, schema migration or production worker was added.
Database read inventory remains the existing production catalog/state/preview/snapshot/eligibility
routes. No SQL query/index/read-limit change accompanies local catalog/rights validation; the
existing bounded PostgreSQL authority and [#5 read assessment](casting-production-inputs.md)
remain the baseline. No measured optimization need or new consumer justifies DBSP here.
The exact already-locked Unix `libc` dependency supports nonblocking safe file/pipe reads and
child file-size limits. Domain/Script IR rules do not depend on Unix, HTTP or a provider SDK.

## Assurance ledger

| Claim / invariant | Failure mode | Oracle / evidence level |
| --- | --- | --- |
| Requested local voice/version and supported controls are explicit; no fallback | Native eSpeak silently changes voice or delivery | Domain/request examples, installed voice preflight; `example-tested`, local `provider-live-tested` |
| The same pure predicate validates resolved pronunciation text and execution | A frozen candidate is rejected only after dispatch | U+2028/U+2029/phoneme and cross-replacement regressions; `example-tested` |
| Full frozen document and receipt identities match exactly | A forged digest/voice/export attaches output to another scope | Reconstruction and scoped mutation examples; `example-tested` |
| Actual PCM bytes are bounded, complete, nonempty and checksum matched | Claimed format admits empty/truncated/foreign/oversized audio | Existing inspector plus truncation/format/frame-bound examples; `example-tested` |
| Ambiguous delivery never automatically resends, even with an idempotency assertion | Lost response causes repeat synthesis or unknown charges disappear | Typed recovery matrix and strict wire constructors; `example-tested` |
| Intent exists before any child; observations never overwrite old records | A crash leaves false nonexecution or a repeat loses history | Fake executable checks intent on first invocation, exact failure receipt and destination refusal; `example-tested` |
| Child timeout, pipes and output files stay bounded | Hang, unbounded bytes, FIFO/device/symlink or leaked child | Owned-PID reaping, timeout/pipe/file-size and unsafe-output injection; `fault-injected` against process fixtures |
| Local settings/voice rights use existing scoped snapshot approval | New local casting bypasses persisted rights or stale approval | Fresh PostgreSQL freeze/approval/reopen/change-denial test; `integration-tested` |
| Historical reference snapshots retain their identity/output semantics | Local catalog addition mutates old frozen candidates | Existing reference digest/oracle and explicit reference text regressions; scoped examples |

Receipts use `caller_reported` or `synthetic_fixture`, never a caller-controlled verified/live
flag. Their audit is `offline-inspection-only-no-dispatch-or-acceptance`: it checks identity and
bytes, not current owner/rights approval. Local attempt files are inspection records, not a
durable worker ledger or power-loss-safe transaction. No cache/reuse is implemented. The local
model ID names the adapter/version contract, not a complete immutable executable/data pin.

## Actual installed local synthesis

The official Homebrew installation command was
`HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_CLEANUP=1 brew install espeak-ng`.
eSpeak NG 1.52.0 occupies approximately
26 MB, with pcaudiolib 1.3 as a small dependency. Execution used macOS arm64 with 24 GiB RAM;
limited disk space motivated this formant engine instead of a neural-weight environment.
There were **zero commercial/cloud synthesis calls**, no provider account/credential changes
and no private source transmission. Local resource cost is unmeasured; charge stays unknown.

The installed `/opt/homebrew/bin/espeak-ng` reported exactly 1.52.0 and advertised `vi`,
`vi-vn-x-central` and `vi-vn-x-south`. The live test consumed the original nonprivate
`two-scenes.json` repository fixture, with explicit neutral/zero-intensity local casting and
distinct voice-rights reference IDs. This is a sample-only document, deliberately missing
production rights/approval; recorded fixture assertions are not legal clearance.

**Command:**

```sh
CANTOS_LOCAL_TTS_RUNTIME=/opt/homebrew/bin/espeak-ng cargo test -p cantos-server --test tts_receipt installed_local_runtime --locked -- --ignored --nocapture
```

**Evidence:** PASS, three real local invocations with actual inspected native signed PCM WAV,
mono 16-bit, 22,050 Hz. The current sample evidence is in ignored
`target/tts-evidence/ede50ae7-2aa7-4588-bbad-cafe487cde1b/evidence.json` with per-line intent,
frozen document, receipt, audit and `output.wav` files. Audio and run JSON stay outside Git.
The final-source run passed one selected test in 0.58 seconds. An earlier successful run at
`target/tts-evidence/c219aca8-f695-48bc-bd65-53fe26ca3968/` produced the same three hashes;
there were six real local synthesis invocations across these two runs. An initial fixture
setup reused a rights-reference ID across distinct voices and was rejected before synthesis;
the corrected sample uses distinct IDs without changing the original script or rights records.

| Dialogue / installed voice | Frames / file bytes | WAV SHA-256 |
| --- | --- | --- |
| `dialogue-01` / `vi` | 52,814 / 105,672 | `c7c4db634144aae58bd454f4b7097d111f8e3da14414db0fcb01451ed9f5a14f` |
| `dialogue-02` / `vi-vn-x-central` | 45,260 / 90,564 | `ae22cb18903642b9cb927e998cac1fa011168216077dc9d7bdf55bb7756ee60b` |
| `dialogue-03` / `vi-vn-x-south` | 51,843 / 103,730 | `ed513a745f66bf8da0f1bd47e97a784a2006ea03478476870dcc9ce56a637cd9` |

The frozen input digest is
`production-p1:sha256:26a1cccd0e08ae703e1fd6c6c4aedd8136164689460663ee67d82336e2a41e80`;
the original export digest is
`sir-e1:sha256:4873857d25942c5c158bfaf30e53662d4f1fe9b8b95db72ecb2639b2ddf4ebf0`.
Installation fingerprints are labeled as observations, assuming this trusted
installation remained stable during each sample run. Production identity must address runtime
or data replacement and dependencies before cache/acceptance use. No pin guarantee is claimed.

The private post-run `installation-evidence.json` records 513 regular data files
(24,152,227 bytes, no symlinks), with sorted relative-path/file-hash manifest SHA-256
`3bf76f3e5adbf0964740e5021890c1c18c7688cfa4457fd8a96968842d3f9b1e`.
Executable SHA-256 is
`8958785a5f9e97a183a7224a1abde1dd89d871eec8c634284e34fb382dc5f377`;
the installed voice-table stdout SHA-256 is
`54e28f8bace9ca52d4b642feea39c15283b896f5f5fad77abc8d3b1e77e6fb39`.
Traversal was capped at 4,096 files / 64 MiB with descriptor/type/stability checks. Linked
libraries and the execution environment were not hashed. These are observations, not a
production immutable-runtime contract.
The final sample directory also contains a post-final-run installation observation at
2026-10-10 19:48:12 UTC: binary, data-tree, voice-table and all 513 per-file records match
the earlier observation made before the final sample run. Its record SHA-256 is
`a2e1206ef8b8168a2ad943ab7ea24874e5e1beb08f29c8ffb5846b7b09b2fdfa`.
Matching observations cannot exclude an intervening change/restoration and do not hash shared
libraries or the inherited environment; the trusted stable-installation assumption still applies.

**NOT_RUN:** audio audition by a named Vietnamese listener, expressive quality, true peak,
loudness, mobile playback, commercial synthesis/billing and end-to-end publication. Valid PCM
and a sample peak do not prove listening quality. eSpeak is formant speech, not a natural acting
or creator-ready voice selection.

## Reproducible checks and provenance

Base is `develop` at `e3d047c862915e5bf1d11a11b06d771f33982435`; the feature branch is
`feat/tts-provider-boundary`. Initial and final checks are recorded separately; synthetic
process fixtures never satisfy a real provider gate. Relevant logs stay under ignored `target/`.
The independently checked 18-file compiled source/test/manifest record
`target/tts-source-fingerprint.txt` has SHA-256
`e1a1193c44867b614fd5f730f2daf81829b6c07736055eb98636677099ebeb35`.
Subsequent edits only completed documentation; compiled files match that record.

| Command | Result / scope |
| --- | --- |
| `cargo fmt --all -- --check` | PASS on final compiled source |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS, `target/tts-clippy-native.log` |
| `cargo test --workspace --locked` | PASS: 311 passed / 52 ignored; ignored PostgreSQL/live checks executed separately |
| `cargo test -p cantos-server --test production --test production_postgres --test tts_attempt --test tts_audio --test tts_local --test tts_receipt --locked` | PASS: 17 domain, 15 recovery, 14 audio, 13 process and 8 receipt/CLI tests; database/live tests ignored here |
| `python3 scripts/check_repository.py` | PASS: 249 text files / 15 skills, plus final documentation recheck |
| `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS: 80 tests |
| `python3 scripts/reference_script_ir.py` | PASS: both independent c1 goldens |
| `CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 TMPDIR=/private/tmp python3 scripts/mutate_script_ir.py` | PASS: 15-test baseline, 9 killed / 0 survived / 0 timeout / 0 unviable |
| `cargo build --locked --bin inspect-wav` then `python3 scripts/reference_wav_inspection.py` | PASS: Python wave/hashlib agrees on five PCM fixtures |
| `PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH" TMPDIR=/private/tmp python3 scripts/test_postgres.py` | PASS: 51 database tests including 9 production tests, plus actual CLI/HTTP/host-kill/database-restart/replay journeys |
| `cargo clippy -p cantos-studio --target wasm32-unknown-unknown --locked -- -D warnings` | PASS, browser target static gate |
| `NO_COLOR=true trunk build --locked` from `apps/web` | PASS, Trunk 0.21.14; no UI source changes |
| `git diff --check` | PASS |

One initial parallel successful-process fixture exceeded its two-second startup deadline.
The successful fixture budget is now ten seconds; the separate short timeout test still checks
exact owned-child reaping. The focused and workspace reruns passed. PostgreSQL and mutation
initially overlapped and exhausted free disk: database error E53100 and mutation ENOSPC are
retained in `target/tts-postgres-final.log` / `target/tts-mutation-final.log`. Neither failed run
is counted as passing. Sequential reruns passed in `target/tts-postgres-retry.log` and
`target/tts-mutation-retry.log`, with bounded mutation compilation using the listed profile
environment. No production/TTS mutation-testing claim is made by the existing Script IR check.

The failed disposable cluster `cluster-1791661330964106000` and fresh successful cluster
`cluster-1791661522971963000` were created under `target/revision-evidence/` by this task.
Both were stopped by the runner; `pg_ctl status` independently reported no server and the PID
file was absent before only their generated `pgdata` directories were removed. Cluster/test
logs remain. Generated server/WASM incremental compiler caches were also removed for space;
unrelated data, previous clusters and all TTS sample records were preserved. No inherited or
production database URL was used. Final native, mutation, PostgreSQL, WAV and Trunk log paths
are listed above or use the `target/tts-*.log` prefix.

## Review and residual risk

Independent read-only review found and corrected FIFO blocking in receipt file admission,
the local text predicate mismatch and implicit phoneme syntax admission. Review also narrowed
runtime fingerprint claims and requested the concrete intent/failure CLI regression. Fresh
read-only review assessed `no-actionable-findings`, `complete-for-declared-scope` for backend,
CLI, contracts and docs; all 18 fingerprinted compiled files matched. The reviewer inspected
final native/PG/mutation/WAV/WASM results and independently executed focused built binaries;
the root confirmed final Trunk/live/repository results. This is local review, not a formal
GitHub approval or deployed result. No CI workflow or gate was added.

**Edge classes covered:** rate limit/timeout/permanent/unsupported voice/control/malformed
audio/cancellation facts; declared versus verified lookup/idempotency contracts; absent lookup
IDs; unknown usage/charges; forged scope; complete/truncated/empty/oversized/wrong-profile PCM;
unsafe output files; child deadline/reaping/output caps; intent ordering and exact retry refusal;
local casting/rights and stale snapshot approval.

**Not covered / residual risk:** commercial provider prerequisites/live evidence, production
credential/identity deployment, durable dispatch/leases/retry/charge settlement, complete runtime
fingerprinting and publication authorization remain open under #6 and the owning later pipeline
boundaries. No exactly-once generation, provider lookup contract, power-loss recovery or current
rights proof is asserted. Studio interaction/accessibility and native mobile were not exercised
for this backend/CLI slice. Shutdown belongs to the parent task after remote verification and
protection of unsaved data.
