# Studio Protobuf wire migration: integration and acceptance ledger

Date: 2026-10-13. Scope: [issue #97](https://github.com/loveoverflowcom/cantos/issues/97),
[ADR 0023](../decisions/0023-versioned-protobuf-wire.md),
[frozen policy](../../packages/contracts/protobuf/README.md).
This is a partial acceptance record. JSON remains default, legacy MessagePack is retained,
and no storage, Script IR, audio, auth, business-rule or Theatre endpoint migration is claimed.

## Provenance and logical commits

C0 froze from fresh develop `b09c180` at `9f0368e18de867c58da31619b6be171478779e9e`.
Four isolated workers completed before serial integration. Exact own commits, in dependency order:

| Scope | Worker source | Integration cherry-pick |
| --- | --- | --- |
| C1 contracts/oracle | `cb3721d23d248726e23a43d429eb08c6e756578f` | `6c59319` |
| C2 HTTP/replay tests | `0b750ab07c4ad3d660518bd9be3f59beed08fc63` | `ada39e3` |
| C3 Studio Web | `45221436f19db8de18f10ab874e11647df28c2ee` | `b4f5580` |
| C4 portable CMP harness | `6874a88f4b41457d69fab6d53c477f8106415e07` | `085feb7` |

Fresh develop `00eab6d456e2fb306e0b5f2009b3f01da7ef75d1` was merged cleanly before C5;
external PR93/95 listener and PR99/100 tooling/account changes are preserved. Only the integration
branch reconciles the root lock: prost/prost-derive 0.13.5, no global toolchain replacement.
Kotlin/plugin stays 2.2.20; core/JSON/Protobuf stay aligned at 1.9.0. No Gradle dependency lock is
configured; exact source pins and the existing distribution checksum are retained.

Golden SHA-256: `f9370eed5574a6a4066dfccbe50a37be6033bf05d76a4973371fa4d48f02e829`.
Hostile SHA-256: `bb843b118aba3acdd03029fb3ce6058ca6e01dc49b08290f89866c690ce6d821`.
The [freeze record](protobuf-wire-contract-freeze.md) remains historical C0 evidence.

## Independent critical review and confirmed corrections

1. **P2 negotiation/exclusion defect.** An early malformed-q return discarded an explicitly
   unsupported/duplicate Protobuf range, allowing a wildcard to select Protobuf. A new production
   unit regression failed first: `version=2;q=bad` plus all non-Protobuf exclusions returned
   `Ok(Protobuf)` instead of `Err(NotAcceptable)`. Parsing now completes Protobuf parameter
   inspection before applying baseline malformed-q tolerance. Unsupported version/encoding,
   unknown/duplicate parameters, duplicate quality and ProtoJSON retain explicit exclusion.
   A Router counter regression checks JSON 406/Vary and zero handler calls. JSON/MessagePack
   malformed-quality handling and lone bad-q tolerance are preserved.
2. **P2 stale-generation evidence defect.** The oracle build script did not register its external
   `.proto` with Cargo. In a disposable copy, adding a field after a warm run incorrectly retained
   PASS in 0.03 seconds. Explicit `cargo:rerun-if-changed` now forces regeneration; the same
   altered-schema copy fails E0063 rather than reusing stale generated types. The committed schema
   is unchanged. A static regression retains that tracking edge, while the xtask oracle compares
   generated text and all 13 independent vectors on the real reviewed schema.
3. **Isolated oracle lint correction.** The exact generated-text check compared two include_str
   expansions directly; Clippy classified the equal frozen literals as eq_op. A fresh isolated
   all-targets lint failed first. Named generated/reviewed inputs retain the same exact equality
   assertion without a lint suppression; all-targets Clippy then passed. The drift gate now runs
   the isolated oracle's Clippy so workspace lint cannot omit this package.
4. **Assurance accuracy correction.** Nullable Kotlin ULong scan returns can box on JVM.
   Wording is narrowed to bounded admission before message/string/list construction; zero
   allocation, Native heap or speedup is not inferred. No functional codec defect was found in
   independent Rust/Kotlin bounds, UTF-8, required-presence or full-uint64 review.

Review also traced real client actor/ticket/text/operation fences and immutable pending-save
state. Host ambiguity tests preserve later drafts and same-operation reconciliation. This source
review and host evidence do not substitute for driven browser fetch/AbortController behavior.

## Claim → oracle → evidence and layers

| Layer / invariant | Owner / oracle | Evidence and boundary |
| --- | --- | --- |
| L1 transport-only, one authority | ADR0023, inward contracts dependency, route inventory | documented/source-reviewed; no product/auth/store dependency introduced |
| L2 required presence and exact values | private wire Option/nullable fields → fallible adapters | type-enforced and exact-category tests; zero/empty/full uint64 and exact Script IR preserved |
| L3 bounded hostile admission | independent 26-case corpus, prefixes, finite byte loops, inclusive limits | C1 9 Protobuf methods within 70 contract tests; C4 14 common/JVM methods, 262,144 two-byte + 40,000 longer checks; not fuzzing/formal proof |
| L3 schema/byte drift | independent Python wire-spec, generated prost, Kotlin serializer | 13 vectors; exact generated-text gate, external-schema warm-build correction; source pins/hashes above |
| L4 media/error/no effect on refusal | production extractors, Router counter and lazy PostgreSQL router | C2 11 header tests + 4 contract tests + 2 size/roundtrip tests; JSON/Origin/session/alias parity, no actual SQL connection |
| L4 client single-send/ambiguity | actual client wire and editor reducers | C3 174 host tests + 1 doctest in each feature configuration; 13 wire cases; no real browser observation |
| L5 SQL/host restart/cross-codec replay | real disposable PostgreSQL + independent Python encoder and row oracle | authored/compiled tests; runtime NOT_RUN after denied Unix-socket setup; no mock PASS |
| Platforms | actual JVM/Android/Apple runners | whole JVM suite and independent focused common/JVM execution; Android/iOS compile UNSUPPORTED, runtime NOT_RUN |

Prior worker counts are attributable to their source commits, not blanket integrated acceptance.
Final integrated command outcomes are recorded below. Repository checks establish hygiene only.
The known Rust aggregate failure does not change the independently passed core/adapters.

## Verification integration and executable gates

- `cargo xtask verify contracts`: adds `protobuf-reference` before runtime contract tests.
  It runs the independent Python encoder and isolated pinned prost/protoc oracle, checking exact
  generated text and lints that isolated package. Runtime/WASM builds do not need protoc.
- Existing Rust CI and full preflight include the same gate. No new paid CI or Gradle CI lane.
- Changed schema/Protobuf fixtures/Rust adapter inputs select both the drift gate and
  `theatre-cmp-demo`. Oracle-only inputs select the drift gate. Selector/command-plan unit tests
  assert these paths; Web checks remain independent of Kotlin.
- The PostgreSQL restart primitive now encodes SaveRevisionRequest via the independent Python
  encoder and compares raw RevisionResponse bytes after API kill + PostgreSQL restart, with
  canonical Content-Type/no-store/nosniff/Vary and JSON failure assertions. JSON and legacy
  MessagePack replay of the same Protobuf-seeded operation remain present. The reference is
  encode-only; no production decoder is used to bless response bytes.
- Mocked primitive regression checks request bytes, exact response bytes, mismatches, actual type
  and JSON refusal. Passing those unit tests is not SQL/HTTP runtime acceptance.

## Integrated command outcomes

| Integrated command/check | Result | Exact observed scope |
| --- | --- | --- |
| `cargo xtask verify repository` | PASS | 319 Python tests, typed/static/link/diff checks |
| `cargo xtask verify contracts` | PASS | Script IR reference, 13 Python/prost goldens + exact drift, Rust/API contract targets, independent WAV/ffprobe and 2 actual Opus tests |
| `cargo xtask verify rust` | FAIL | rust-format and workspace lint PASS; workspace tests fail in 17 unchanged xtask private-input fixtures |
| API wire focused test module | PASS | 13 methods, including both fail-before negotiation regressions |
| xtask gate/selector/CI focused tests | PASS | 12 / 16 / 4 methods; new command plans/selectors exercised |
| Isolated schema oracle clippy | PASS | all targets, -D warnings; original eq_op FAIL retained |
| Python reference / restart primitive unit tests | PASS | 4 / 13 methods; no SQL runtime inference |
| `cargo xtask verify web` | PASS | host-independent WASM lint, Studio and Theatre builds, UI preview |
| Studio `revision-protobuf` host tests / WASM clippy | PASS | 174 tests + 1 doctest, actual feature build lint |
| `cargo xtask theatre-demo verify` | PASS | 179 integrated JVM tests, zero failures/errors/skips; all 14 Protobuf methods executed |
| Android / iOS build commands | UNSUPPORTED | SDK absent / installed Native executable absent; Linux also lacks macOS/Xcode |
| Android / iOS runtime | NOT_RUN | no device/emulator/simulator execution |
| real PostgreSQL runtime | NOT_RUN | prior denied Unix-socket setup, no retry/bypass |
| first broad changed-only preflight | FAIL | all Rust/DBSP/WAV/Opus/Web stages PASS; untouched CMP preview could not fetch existing serialization-core1.7.3; later stages NOT_RUN |
| standalone shared CMP preview retry | PASS | 87 executed host tests, zero failures/errors/skips; verified offline Maven metadata and process-local bundled Noto fonts |
| mutation | FAIL | cold disposable baseline exceeded existing 60s subprocess timeout before any mutant; no assertion/timeout/runner change |
| final broad changed-only preflight | PASS with 3 explicit NOT_RUN skips | 14 executed gates PASS; rust-tests, postgres and mutation gaps retained; both JVM suites forced to execute again |

The full Rust failure is retained: this execution sandbox exposes a read-only `/tmp/.git`
sentinel, so the existing private-input guard correctly rejects every private fixture under it.
The earlier 17 failures are pre-existing; no guard, fixture or assertion was weakened. Whole
workspace runtime test success is not claimed. The preflight skip does not erase the full gate's
FAIL. Repository process-timing failures in prior workers remain historical evidence; the current
independent repository run passed. No Kani/formal claim is made from mock adapter test output.
PostgreSQL is explicitly skipped/NOT_RUN rather than retrying denied socket setup.
Shared CMP preview network failures and the later default-font Skiko SIGSEGV are preserved.
A supported execution-only local repository contains unchanged official 1.7.3 metadata/artifacts;
jar SHA-256 values match publisher module metadata. Process-local Fontconfig uses the existing
repo Noto fonts and writable cache, without an OS font/toolchain change or test filtering.
The final preflight forced both Gradle suites to re-execute with `--rerun-tasks`: shared CMP
preview 87 tests and Theatre 179 tests, each with zero failures/errors/skips. Fresh JVM XML
timestamps span 15:52–15:55 UTC. That named host configuration passed; stock-system-font
rendering remains a separate limitation. Its existing 1.7.3 transitive runtime is outside the Theatre 1.9.0 pin; no version was changed to manufacture a pass. The
supported network permission route can reach official Maven with ordinary TLS.
The mutation runner and all four Script IR source hashes match develop; its cold build timeout
is recorded as FAIL, with mutant execution NOT_RUN, not a killed-mutant claim.

## Performance: measurements are separate claims

C2 native release worker: Rust1.87, Linux x86_64, Intel Xeon Platinum8573C, 12 actual DTO payloads
(Revision/Save/Validate × 1/8/64/512 KiB target Script IR), three codecs, actual fixture gzip,
7 timed rounds with median/min/max. At 512 KiB Revision:

| Format | Raw bytes | gzip bytes | Native decode median µs |
| --- | --- | --- | --- |
| JSON | 591836 | 24500 | 1511.84 |
| legacy MessagePack | 525967 | 23733 | 341.52 |
| strict Protobuf | 525884 | 23656 | 652.72 |

The small compressed saving over MessagePack does not imply a speedup: strict Protobuf decode
was slower than MessagePack on this host. Actual HTTP compression, native heap/allocation and
network/request latency were NOT_RUN. The [serial integration rerun](protobuf-wire-native-rerun.json) passed one exact benchmark method
and retained all 12 raw/gzip rows and seven-round CPU spread. Concurrent builds were active; its
CPU timings are noisy and not a controlled speed comparison. Raw/gzip byte counts agree with the
worker. The initial integration command also executed both benchmark methods concurrently;
that output is retained locally but not used as a controlled CPU measurement.

C3 matched release Studio bundles (Trunk0.21.14, wasm-bindgen0.2.100, wasm-opt123, default release,
Python gzip level9/mtime0) are retained in [measurement data](protobuf-wire-bundles.json):

| Build | WASM raw | WASM gzip |
| --- | --- | --- |
| C0 JSON baseline | 4316981 | 1310913 |
| C3 JSON | 4318528 | 1311300 |
| C3 explicit Protobuf | 4337228 | 1319117 |

Protobuf adds 18,700 raw / 7,817 gzip WASM bytes versus C3 JSON. JS is 40,337 raw / 7,453 gzip
for every measured stage. These are worker-stage matched bundles; no current-develop performance
identity or browser CPU/network claim is inferred.

[C4 JVM measurements](../../apps/cantos-theatre/mobile/docs/protobuf-compatibility.md#payload-measurements-jvm-only)
cover 1/8/64/512 KiB Validate payloads, Protobuf CPU and JDK current-thread allocated bytes.
They compare neither Kotlin JSON/MessagePack nor Native, compression, peak heap or transport.
The independent review confirmed boxing is possible before runtime decode. No blanket speedup
or zero-allocation claim is made.

## Deferred acceptance, rollout and stopping condition

Keep issue #97 open and use **Part of #97**. To finish declared acceptance/activate the default,
the integration/release owner must provide:

1. Supported disposable PostgreSQL execution of `cargo xtask verify postgres`: origin/session/ACL,
   exact row/digest snapshots, both-direction replay, host kill/restart and no partial writes.
   Existing denied Unix-socket setup was not retried or bypassed.
2. Driven real-browser Studio Protobuf save/reload, proxy/actual Content-Type, delayed stale
   response, cancellation, preserved draft and explicit JSON deployment rollback evidence.
3. Android compile/test and supported macOS/Xcode/Kotlin-Native2.2.20 Apple compile/test/runtime;
   separate per-target uint64/codec evidence. JVM never settles Native acceptance.
4. Missing WASM/browser CPU/allocation, native heap and actual request latency/compression,
   plus target-specific comparisons where a performance claim is proposed.
5. Owner review of activation results, measured tradeoffs and rollback. Until then JSON default.

Legacy MessagePack retirement additionally needs identified deployed/external consumers,
owner-approved zero-use observation period and notice; consumer absence is unproven.
Broader publication/listener/account/native-audio/rights acceptance is outside this wire change.
Residual risk: real PostgreSQL, browser/network and Native acceptance remain open; baseline/flaky
failures are retained explicitly; independently passed layers do not complete the declared issue.
