# Existing TTS provider execution verification

Status: bounded backend verification for issue #6, 2026-10-11. Base `develop` is
`8a9e140cb7198f3a53beca23f71490f180fa0c28`; branch `feat/tts-provider-verification`.
This slice adds no second commercial adapter, production dispatcher, durable job, reservation,
cache acceptance or publication route. Google live and listening are **NOT_RUN**. Issue #6
remains open because its full local/commercial acceptance criteria are not met.

## Invariant and owner

A sample sends only the exact admitted provider/voice/control scope; an outcome that may have
executed preserves unknown charge and requires reconciliation before repetition. Untrusted
response/process bytes are bounded and admitted by the shared WAV/checksum inspector.
The existing `tts` modules own provider planning, HTTP/process execution and offline receipt
inspection. The existing production authority owns frozen inputs, rights and approval; the
[production pipeline](../product/production-pipeline.md) and
[business rules](../product/business-rules.md) remain authoritative.

The functional core stays in `google::plan`, `classify`, `decode_audio`, production admission
and receipt recovery. The thin probe composes those decisions before credentials and I/O.
No generic provider port or new crate is justified by this verification work. Regression
fixtures were derived from the existing request/WAV contract and source-identified gaps;
new failure cases were not executed against BASE before implementation. The final named
regressions exercise production modules, not a fabricated evidence report.

## Implemented boundaries

- Google adapter `google-cloud-tts-synthesize-3` requires explicit neutral/zero delivery,
  validates voice/family/model/locale/prosody and exact output/version identity. Google `-2` candidates remain readable/auditable with their original bytes and digest,
  but current eligibility and dispatch fail closed until a new candidate is frozen and approved.
  A narrow complete-document reconstruction admits history; it creates no authorization witness. Reference and eSpeak identities are unchanged.
- The pure decoder enforces the 24 MiB response bound before parsing and 16 MiB decoded audio
  bound before hashing, then rejects empty, malformed, truncated, wrong-profile WAVs. Receipt
  inspection independently rejects checksum corruption. Only canonical Google RPC status names
  survive sanitization; arbitrary text and credentials are not recorded as observations.
- The production HTTP client is HTTPS-only with no redirects, explicit HTTP/1.1, no idle pool
  and no automatic decompression. A counted loopback server remains available after the first
  reply to detect any resend. Rate limits, 503, resets, header/body timeout, interrupted body and
  empty audio produce exactly one recorded request in the mock. After possible execution,
  delivery remains `may_have_executed`, charge remains `unknown` and recovery is reconciliation.
- `probe-google-tts --live` checks the line estimate against both an explicit cap and the frozen
  USD budget, and checks pinned rights/budget eligibility at the current time before reading
  credentials or creating output. Missing, revoked, expired, wrong-voice claims and budget
  mismatches block. These are offline caller assertions, not current backend approval or legal
  clearance. A cap bounds a dated estimate, not actual account spending; production dispatch
  stays unavailable until #7 supplies the durable/current authority.
- eSpeak subprocesses have a cleared environment, fixed PATH/locale and private HOME/TMPDIR.
  Exact built-in Vietnamese language/file rows must be unique; inherited data or loader settings
  cannot silently select a different package. Output descriptors reject symlinks, FIFOs,
  hardlinks and an inode replaced between metadata and open, under existing deadline/pipe/file
  bounds. Request/control/output mapping remains `espeak-ng-local-v1`.
- Canonical production skills now distinguish implemented samples from proposed workers and
  require reconciliation of potentially executed malformed-audio attempts. No unknown charge
  is converted into zero. Neural VieNeu production refusal is unchanged.

## Fresh local eSpeak evidence

Command: `CANTOS_LOCAL_TTS_RUNTIME=/opt/homebrew/bin/espeak-ng cargo test -p cantos-api
--test tts_receipt installed_local_runtime --locked -- --ignored --nocapture`.
Result: **PASS**, one selected test, three real sample invocations on the final source; repeated destinations were
refused without replacing output. Evidence level: `provider-live-tested` (local eSpeak only),
`audio-measured` (structural PCM/frame/sample inspection only).

Private records and audio are in ignored
`target/tts-evidence/b82f1180-1106-438a-966d-84442ade9ac2/`; the run log is
`target/tts-provider-verification/espeak-final-live.log`. An earlier run under
`target/tts-evidence/995e2410-998a-4635-8b7f-c5c80c478fbe/` produced the same measured bytes:
six local invocations across both runs. Each line has frozen document, conservative
intent, receipt, audit and actual output. The original nonprivate `two-scenes.json` fixture is
used with explicit neutral/zero local casting, without production rights/approval.

| Voice | Frames / bytes | Actual WAV SHA-256 |
| --- | --- | --- |
| `vi` | 52,814 / 105,672 | `c7c4db634144aae58bd454f4b7097d111f8e3da14414db0fcb01451ed9f5a14f` |
| `vi-vn-x-central` | 45,260 / 90,564 | `ae22cb18903642b9cb927e998cac1fa011168216077dc9d7bdf55bb7756ee60b` |
| `vi-vn-x-south` | 51,843 / 103,730 | `ed513a745f66bf8da0f1bd47e97a784a2006ea03478476870dcc9ce56a637cd9` |

All actual bytes are complete native 22,050 Hz mono signed 16-bit PCM; no conversion occurred.
Frozen digest is `production-p1:sha256:26a1cccd0e08ae703e1fd6c6c4aedd8136164689460663ee67d82336e2a41e80`.
The private post-run `target/tts-provider-verification/installation-evidence.json` records
513 data files, 24,152,227 bytes, manifest SHA-256
`0984d0a55db99885c1aa9a0262f0e86a35c74173992085d2103f5963abe6288a`
(canonical sorted JSON of relative path, length and file hash), binary SHA-256
`8958785a5f9e97a183a7224a1abde1dd89d871eec8c634284e34fb382dc5f377` and voice-table SHA-256
`54e28f8bace9ca52d4b642feea39c15283b896f5f5fad77abc8d3b1e77e6fb39`.
Traversal was bounded at 4,096 files / 64 MiB. This is a trusted installation observation,
not an immutable runtime pin; linked libraries, memory and latency were not measured.

## Rights and commercial status

Primary documentation was re-read on 2026-10-11. Google's
[REST contract](https://docs.cloud.google.com/text-to-speech/docs/reference/rest/v1/text/synthesize)
returns base64 WAV for LINEAR16 and documents no request identity/lookup/deduplication or billing
settlement field. The dated [voice list](https://docs.cloud.google.com/text-to-speech/docs/list-voices-and-types)
still lists the selected 40 vi-VN voices, and [pricing](https://cloud.google.com/text-to-speech/pricing)
still lists the existing family's tariffs. These declarations do not prove account availability,
remaining free allowance, error billing or a pinned model version. No account/credential was
created, inspected, uploaded or changed; billing was not enabled and no paid request occurred.
The [quota documentation](https://docs.cloud.google.com/text-to-speech/quotas) permits generated
files in applications/media subject to the Cloud terms and applicable law. That statement is
not a newly accepted agreement or a claim of source/voice production clearance. The actual
account agreement and applicable scope still require the operator's determination.

The [versioned eSpeak license](https://raw.githubusercontent.com/espeak-ng/espeak-ng/1.52.0/COPYING)
remains GPLv3 for the separately installed engine/data. This PR does not bundle it or approve
new source/voice/output distribution rights. Original sample inspection is the authorized use.
[VieNeu lineage](local-neural-tts-research.md) remains unresolved; research-only **NO_GO** for
production, even with a planning approval. No neural generation ran in this slice.

## Verification and assurance ledger

Toolchain: actual Rust/Cargo 1.87.0 with `RUSTUP_TOOLCHAIN=1.87.0`; PATH points to the existing
stable-named installation's direct binaries, avoiding a duplicate download. PostgreSQL 17.4,
Trunk 0.21.14, Python 3.13.5 are available. Each heavy gate runs sequentially; final
verification uses `RUST_TEST_THREADS=1` after a pre-existing xtask ephemeral-port race under
parallel execution. The failed workspace log is retained as
`target/tts-provider-verification/verify-rust-parallel-failure.log`; the exact port test passed
when isolated. No assertion or xtask source was weakened. Initial socket
tests were blocked by sandbox; they were rerun with authorized loopback access. A startup-sensitive
local timeout fixture was corrected to retain synthesis timeout, bounded elapsed time and exact
owned-child reaping. An initial fixture lint and a copied malformed-WAV expected error were corrected and rerun.
The first preflight reached PostgreSQL but failed with `E53100` / `No space left on device`.
Its log is retained as `preflight-disk-failure.log`. Only the two stopped disposable `pgdata`
directories created by this task were removed (clusters `1791728476041130000` and
`1791728619761497000`); their logs, all audio/evidence, other caches and source were retained.
This recovered approximately 1.8 GiB. No code or assertion changed. Only final reruns below count as PASS.

| Command | Final result / evidence |
| --- | --- |
| Focused `probe-google-tts`, `google_tts`, `tts_local`, `tts_receipt` tests | PASS: 2 / 19 / 16 / 8 tests before historical fix; final compatibility suite: Google 20, production 20, schema 4, local 16; one opt-in live test separately run |
| `cargo test -p cantos-api --lib tts::google_http --locked` | PASS: 14 tests; counted loopback mock, no Google |
| `cargo xtask verify rust` | PASS: rust-format, rust-lint (workspace), rust-tests (workspace) |
| `cargo xtask verify contracts` | PASS: script-ir-reference, contract-tests, wav-reference |
| `cargo xtask verify postgres` | PASS: disposable PostgreSQL, including 11 production tests and actual CLI/HTTP/restart journeys |
| `cargo xtask verify repository` | PASS: repository checks, 145 Python tests, diff whitespace |
| `cargo xtask preflight --changed-only` | PASS: repository, rust-format, rust-lint, rust-tests, wav-reference, web-lint, web-build, postgres; complete rerun after the retained disk FAIL |
| Google live / billing / listening / publication / mobile playback | NOT_RUN; outside authorization or unavailable acceptance evidence |

Working directory is the repository root. All listed gates ran on the named BASE plus this
dirty implementation diff, with the following environment (the direct Rust binaries report
1.87.0):

```sh
PATH="/Users/manhblue/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/opt/homebrew/opt/postgresql@17/bin:$PATH" \
RUSTUP_TOOLCHAIN=1.87.0 RUST_TEST_THREADS=1 TMPDIR=/private/tmp cargo xtask <command above>
```

Logs are in `target/tts-provider-verification/`. Mocked envelopes and synthetic process/WAV
fixtures establish `mock-integration-tested` and `fault-injected` behavior; they are not captured
Google envelopes. Existing production tests cover exact owner approval, stale revision/settings/
rights, term expiry and budget denial. PostgreSQL results are recorded separately above. The affected database reads remain the
existing authorized point/snapshot/eligibility paths: no SQL, index, cardinality or permission
filter changes. Historical reconstruction replaces current-only CPU admission on the same
bounded input; no performance benefit is claimed. The existing
[production read assessment](casting-production-inputs.md) remains the PostgreSQL baseline;
there is no measured reason or new consumer for DBSP.

| Claim | Owner / oracle | Evidence level / disposition |
| --- | --- | --- |
| Unsupported identity/control never dispatches | Google planner + production/CLI regressions | `example-tested`; held |
| Possible paid execution is never replayed or called free | Counted HTTP mock + recovery/receipt tests | `mock-integration-tested`, `fault-injected`; held for mocked boundary |
| Rights/estimate blockers precede credentials/output | Actual probe composed with production authority | `mock-integration-tested`; held for pinned sample policy |
| WAV type/size/completeness/checksum fail closed | Synthetic WAVs, independent hash literals and real eSpeak bytes | `example-tested`, `provider-live-tested` local only; held |
| Exact local package voice and private bounded output | Fake child/output faults, cleared-env real eSpeak | `fault-injected`, `provider-live-tested` local only; held |
| Historical Google snapshots retain exact bytes without current authorization | Exact pinned reconstruction, old receipt audit, real PostgreSQL read/HTTP/replay/immutability/denial regression | `example-tested`, `integration-tested` (local PostgreSQL); held |
| Current approval cannot be inferred from sample receipt | No conversion to dispatcher/accepted artifact; existing authority tests | `type-enforced` boundary, production tests; durable dispatch deferred |

## Residual risk and next handoff

Independent review identified and fixed the historical `-2` snapshot corruption regression
introduced by the version bump. PostgreSQL readers and offline receipt audits now preserve old
identity, and outdated-adapter findings invalidate current eligibility. Independent re-review
found no unresolved actionable defect in the declared backend/contracts/tests/docs/skills scope.
The code/contracts diff fingerprint is SHA-256
`55bcb9f437e286710125e98ff91a9477b983d815fcc66defdd1c12991e5ed6bb`
(`git diff --binary -- services/cantos-api packages/contracts` against the named BASE).
Final gate-result documentation does not change that implementation snapshot and receives
repository and formatting checks before commit. Remote CI is not checked or polled. Web build
PASS does not establish Studio interaction/accessibility; no UI behavior changed here. No listening quality,
pronunciation, naturalness, true peak or loudness acceptance is claimed. Installed files may
change during execution; current revocation and production approval cannot be proved offline.
Google model revision and bill settlement are unknown. Bounded decoding is regression-tested,
not fuzz-tested. An interrupted probe can leave partial private records; those records are not a
power-loss-safe transaction or a job store.

The next requester-authorized PR is issue #7 after this PR merges, from fresh `develop`.
It must persist frozen scope/current approval, reservations, immutable intent/observations,
leases/fencing/cancellation and ambiguous-attempt reconciliation before any production send.
Use local/fake execution and preserve Google live **NOT_RUN**; a successful mock or snapshot
approval does not authorize spending. Preserve VieNeu research-only refusal. Issue #6's separate
commercial/live/listening/rights gaps remain open rather than being silently absorbed into #7.
