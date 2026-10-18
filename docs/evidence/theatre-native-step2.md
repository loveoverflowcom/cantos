# Theatre native integration: step 2

This record covers the requested native player, account/Library wiring, SQLDelight outbox and
production Android artifact path. It depends on **unmerged PR #247**, pinned at
`cc1e56b121c95e86f01028813df6b9f9c26408b5`, from `develop@e4d6ca2`.
The [step-1 handoff](theatre-live-catalog-step1.md) owns its local dataset and media evidence.
This change targets `develop`; the dependency must land first. Its historical server/storage
results are not new native acceptance. The independently proposed #248 Work-ID guard is
preserved in the touched reducer, including the new Library ID-only entry path.

## Implementation and boundaries

| Capability | Production path | Scope / owner |
| --- | --- | --- |
| Public catalog | Existing Discovery v1 adapter/controller; validated account-local terminal-page/detail cache, explicit refresh and account fencing | #159/#14; guest reads remain public, with no durable guest import |
| Native playback | `NativeSessionRuntime` → `NativeListeningController` → existing reducer/`MediaEngine` → authorizing shell → native decoder | #14; one process owner, shared mini/full controls, no simulation adapter |
| Playback identity | Exact Work/Unit/Publication/Rendition/SHA/duration in `AudioIdentity`; order is never media identity | Discovery v1; URL only inside ephemeral redacted grant/location values |
| Android | SDK MediaPlayer, MediaSession and mediaPlayback foreground service | Min SDK23/target35 retained; platform support and actual device behavior remain acceptance work |
| iOS | Swift AVPlayer/KVO/NowPlaying/remote commands behind the plain Kotlin driver bridge; production `Cantos` scheme | Source only on this Linux host. Current Ogg/Opus compatibility is unaccepted; no substitute/transcode is invented |
| Account | Existing native flow/client/secure-store/browser adapters, production panel, API-confirmed session StateFlow | #191; configuration absent fails closed; no provider credentials or Web Origin impersonation |
| Library | Continue, Saved, History pages/opaque cursors; saved/progress reads and PUTs; acknowledged starts, explicit clear, receipts | #35/#17; independent Library client replaces first-page count as the product read model |
| Local state | SQLDelight2.1.0, actual SQLite, one account-scoped metadata/checkpoint/outbox database | #17; SQLDelight adoption and native driver gaps in [ADR0026](../decisions/0026-theatre-mobile-local-storage-sandbox.md) |
| Distribution | Distinct production channel and `androidProductionApp` debug artifact, package/environment/signature/version and actual-byte exclusion checks | #184; demo remains a different application. Remote distribution requires explicit app/recipient authorization |

Library endpoints consumed beneath `/api/v1/theatre` are:
`GET /library/continue`, `GET /library/saved`, `GET/PUT /library/saved/{work}`,
`GET /library/history`, `POST /library/history/playback-starts`,
`POST /library/history/clear`, `GET/PUT /progress/{publication}` and
`GET /operations/{operation}`. Only bounded JSON metadata uses the identity HTTP transport;
media goes directly to the decoder. PUT is admitted on both native transports.

No bookmark or download endpoint is implemented by this slice. Saved membership is distinct
from timestamp bookmarks (#12), downloaded bytes and download authorization (#16).
Registration (#174) remains unavailable in the native product: existing Web enrollment
mutations require the reviewed Web Origin; email proof is not a native session.

The existing native authorization response names the configured provider browser UI. #191
still owns browser-facing ingress/issuer acceptance; this change does not create an API
browser facade, proxy privileged Kanidm APIs or fabricate an Origin. The default artifact
omits the native issuer and therefore exposes the honest unavailable account capability.
An explicit issuer setting is not evidence of successful login.

## Claim, oracle and evidence ledger

| Invariant / failure | Owner / cheapest independent oracle | Evidence boundary |
| --- | --- | --- |
| Playing and history require current engine acknowledgement | Existing pure reducer, streaming tests, same production runtime through JVM Compose with an explicit fake decoder | Example/semantics/interaction tests; no audible-output claim |
| Seek position stays confirmed until native position acknowledgement | Reducer pending-position test; adapter source/item/generation gates | Pure tests and Android compile; real seek accuracy pending |
| Cold restore and remote progress never autoplay/seek | SQLite reopen → paused checkpoint; controlled delayed manifest after Continue; explicit conflict choice has no engine dependency | Local SQLite and deterministic controller evidence |
| Back/navigation never creates a second engine | Process roots, production Compose test counts factory calls across mini/full/navigation | JVM interaction evidence; OS lifecycle pending |
| Expiry/retry preserves exact release and position | Manifest/grant identity validation; bounded refresh, expired grants, delayed cancellation and redaction tests | API doubles; actual expiry/withdrawal/native retry pending |
| Final authorization refusal stops buffered playback | Authorizing adapter pauses native before terminal failed observation | Deterministic port evidence; device observation pending |
| Logout/account switch hides old state and fences late work | Verified authority clears synchronously; Library leave cancels/fences old owner; native clear releases metadata/notification | Account/controller tests; native logout acceptance pending |
| Local user mutation and outbox append are atomic | Real file-backed SQLite rollback/reopen tests | Integration-tested SQLite/JDBC; native force-stop pending |
| Lost response never invents a new operation | Persist exact sent body/operation/device/sequence/base; receipt read precedes replay | Fault-injected response over API double plus real local SQLite |
| Reads cannot rebase unseen offline intent | Separate own-accepted-ack watermark; explicit seek/resolution bases stay pinned; remote revision conflict regression | Deterministic/JDBC; real two-device API concurrency pending |
| Pending desire survives refresh; Saved/History remain independent | Pending overlays, FIFO/receipt tests, explicit clear confirmation | Production controller tests; server/cross-client journey pending |
| Incomplete/invalid network results never become demo content | Strict schema/fixture decoding, errors/empty states, release graph and actual APK/AAB inspection | No mock or test classes/resources may enter production bytes |

An operation is created once at user/checkpoint intent time and committed with its local
candidate. Its request is frozen before first transmission. Only a checkpoint or Saved desire
may advance through this device's own already-accepted FIFO predecessor before its first send;
GETs and conflict receipts never grant that permission. Sent/unknown requests are immutable.
An explicit listener conflict choice creates a new operation against the displayed revision;
it changes saved progress, never the active playhead. Known failed desires can be superseded
only by an explicit new user action, retaining original body/receipt records.

Checkpoint/sync is bounded to 30s plus pause/acknowledged seek/speed/background boundaries;
no per-second server writes. Ordinary decoder samples read actual native position and do not
advance playback from a timer. Retry backoff is finite; successful batches are bounded and
continued separately. There is no native connectivity observer in this slice: after retry
exhaustion, reopening the verified account or explicitly pressing Retry resumes the durable
outbox. Automatic reconnect wakeup remains with #17. Cursors are process/session walk
references, never durable identity.
Only terminal public pages/details are cached; a nonterminal page cannot be cached as a
truncated successful end. Cached catalog copy explains that Refresh obtains current data.

`cantos.listening.progress` was looked up in the behavior catalog: it remains `needs-decision`,
with no executable model. No model was invented or promoted, and no Quint conformance is
claimed. The existing `theatre-mock-playback@1` corpus keeps its original mock meaning; streaming
adds separate typed regressions without changing its expected outputs.

## Reproduce and install

Use Rust 1.98.1, Kotlin 2.2.20, CMP 1.9.0, Gradle 8.14.3 and the configured Android SDK/JDK.
Commands run from the repository root, never the preview/demo application's packaging task.

```sh
cargo xtask theatre-demo verify
cargo xtask theatre-host android-debug --api-origin https://api.cantoss.duckdns.org
cargo xtask theatre-host android-release --api-origin https://api.cantoss.duckdns.org
cargo xtask theatre-host ios-build --kotlin-native-home /absolute/installed/native-2.2.20
cargo xtask preflight --changed-only
```

The debug artifact is the production composition, signed with the local debug key for QA;
it is not a signed production release. Its expected location is
`apps/cantos-theatre/mobile/androidProductionApp/build/outputs/apk/debug/androidProductionApp-debug.apk`.
Install that exact APK using the Android package installer or the operator's device tooling.
The command does not install, upload or notify testers. The configured remote API's step-1
probe was empty and its deployed revision differs from the dependency; do not claim that a
new native artifact has played the retained local synthetic release through advertised HTTPS.
The HTTP loopback catalog option does not weaken native credential/media HTTPS requirements.

An optional, read-only distribution plan is:

```sh
cargo xtask distribute firebase cantos-theatre --channel production --artifact apk \
  --api-origin https://api.cantoss.duckdns.org --groups cantos-internal --dry-run
```

The example group is only a plan value. Real distribution requires the explicit Firebase app,
expected signing certificate and actual authorized recipient set; no real distribution ran.
The signing/environment mapping is separate from decoder, account and API acceptance.

## Verification and residual acceptance

Full command output is retained under ignored `artifacts/agent-runs/step2/`. Earlier failing
runs remain attributable and are not renamed to PASS. The source is the dirty step-2 tree
over the pinned dependency; the handoff commit records the final tracked source. Commands use
`RUSTUP_TOOLCHAIN=1.98.1` and the existing shared Cargo target cache. No remote CI was awaited.

| Final check | Result | Attributable log / scope |
| --- | --- | --- |
| `cargo xtask verify rust` | PASS | `rust-final.log`: format, workspace Clippy and workspace tests |
| `cargo xtask verify contracts` | PASS | `contracts-accepted.log`: Script IR, Protobuf, contract tests, independent WAV oracle and actual codec/byte checks |
| `cargo xtask theatre-demo verify` | PASS | `cmp-final-verified.log`: shared 271 selected, 269 passed/two skipped; sandbox 219 passed. Native 20 and the SQLite History-clear regression passed; `jvm-final-summary.json` preserves suite attribution |
| Focused existing Python primitives | PASS, 26 tests | `python-accepted.log`: Android inspector 13, production hosts 5, iOS identity 5, source routing/codegen 3 |
| `cargo xtask theatre-host android-debug --api-origin https://api.cantoss.duckdns.org` | PASS | `android-debug-ui-final.log`: production build, actual-byte exclusion, package/version and signing readback |
| `cargo xtask theatre-host android-release --api-origin https://api.cantoss.duckdns.org` | PASS | `android-release-ui-final.log`: unsigned release APK/AAB and matching class inventories; no sandbox/mock/demo/test bytes |
| Production Firebase distribution with `--dry-run` as above | NOT_RUN, plan emitted | `distribution-dry-run.log`: no probe/build/upload/write; app ID and expected cert unresolved; example group is not an authorized recipient set |
| `cargo xtask theatre-host ios-build --kotlin-native-home /home/manhpd/.konan/kotlin-native-prebuilt-linux-x86_64-2.4.20` | UNSUPPORTED | `ios-accepted.log`: actual discovered compiler is 2.4.20; command requires installed 2.2.20. No framework/app compile started |
| `cargo xtask preflight --changed-only` | UNSUPPORTED | `preflight-final.log`: missing PostgreSQL `initdb`, `pg_ctl`, `psql`; every other selected gate NOT_RUN at prerequisite check |
| `cargo xtask preflight --changed-only --skip postgres` | FAIL | `preflight-final-available.log`: repository's same 22 qualification findings; PostgreSQL explicitly NOT_RUN; remaining selected gates NOT_RUN after repository failure |

The two shared skips are the opt-in live catalog presentation and complete-page walk: both
require `CANTOS_THEATRE_LIVE_ORIGIN`, which was unset. They supply no live acceptance here.

Final Android artifacts use package `com.cantos.theatre`, version `0.1.0` / code `1` and API origin
`https://api.cantoss.duckdns.org`. The native issuer is empty. Both byte-inspection runs reject
the 70 sandbox classes and three sandbox resources. Debug contains 26,773 defined classes;
release APK/AAB each contain 26,778. These counts do not establish decoder execution.

| Artifact / identity | SHA256 |
| --- | --- |
| Installable production-composition debug APK | `c5b4e3ce664857ea4aa16d1d2a58eed182df560b8aa38be85c4243854ef0dcc6` |
| Local debug signing certificate | `b0f5604fde71614bd8b9578d5022f2498571245ee1f4e16e290071836c934c0e` |
| Unsigned release APK | `f56ee94f83c9f92644fdd9a5a57a87febb781f366e3111d72c637586b4d25a82` |
| Unsigned release AAB | `2dd3f89a19b61d28d6894879903e2fdde5c44cfae63c03114384817abd3a7cf4` |
| Bounded Android production input inventory, 154 files | `7f077aa50ce31fa6c3760a9a7a15f49fb6a290b7bdccb28e6e29c0d7ce8bb2af` |

`android-source-ui-final.json` records the dirty source over `cc1e56b`, build files, shared UI,
schema and production sources, excluding tests/docs/build outputs. Production inputs remained
frozen throughout both final builds. Earlier artifacts and source inventories are retained as superseded candidates.
Read-only probes of the configured deployed API returned HTTP200 with an empty Work page and
registration `unavailable/registration_unconfigured` (`deployed-works.json` and
`deployed-auth-configuration.json`). They are HTTP probes, not requests observed from the APK.

The first contracts run built `inspect-wav` into the shared `CARGO_TARGET_DIR`, while the
existing Python primitive expected the worktree's `target/debug/inspect-wav`. That run is FAIL
in `contracts-final.log`. An ignored symlink to the just-built binary repaired only the local
run environment; the complete gate then passed. No checker requirement was changed.

Repository qualification was compared independently with a clean checkout of the exact
dependency SHA. Both checker outputs contain the identical 22 findings (six stale shared
qualifications and 16 adoption rows), byte for byte. Logs and comparison are
`repository-parent-baseline.log`, `repository-step2-current-checker.log` and
`repository-qualification-comparison.json`. Both renderer digests are unchanged by step2.
This attribution is against #247, not clean `develop`: #247 itself added a catalog TextField.
No evidence hash was rewritten to manufacture admission. The final preflight retains that
required failure.

Preflight broadened to all gates. Rust/contracts and the Theatre JVM target above have
independent PASS results; Forge, DBSP, Web builds/previews, shared CMP preview and mutation
were not reached by preflight. No step-2 server SQL or Script IR encoder changed. Full required
preflight remains unaccepted until the PostgreSQL tool prerequisite and existing qualification
failure are resolved; the latter needs an assigned owner before acceptance.

Initial development check: Library/source/JVM baseline passed before the final host wiring.
The first integrated checkpoint compiled common code and selected250 shared tests: two failed,
two skipped; the failures were a test-only cursor grammar error and an in-flight build that
still contained the subsequently removed automatic device-remint branch. The final run must
recompile current sources and execute both regressions. The existing sandbox219 XML result
was from a previous run, not reached after that integrated failure.

The next compiled candidate (`cmp-accepted.log`) exposed two obsolete Loading expectations
after grant acceptance now emits Buffering, plus a race in the inherited catalog test's
synchronized-list iterator. Test oracles now expect Buffering until the real Started callback;
the HTTP request recorder uses snapshot iteration. Production behavior was not weakened to
satisfy those tests. The final run also adds a real SQLite/controller regression proving an
acknowledged History clear retains confirmed Saved/progress and pending exact requests.

Production Compose tests use fake catalog/access/decoder ports, with four constraints:
390×844dp at text scale 1 and 320×568dp at text scale 2, light/dark, Vietnamese. Mini controls and two-item
navigation remain visible; the full player is scrollable at large text. Final assertions
measure all ten full-player controls and both mini controls at least 48×48 in every matrix cell.
Inspection found 40px full buttons and a 44px seek pointer node before repair. Full buttons now
have 48dp minimum height; the standard Slider thumb is also at least 48dp, so the actual inner
seek semantics/pointer node grows. Merely sizing the outer slider did not pass the assertion;
that failed attempt remains `cmp-ui-final.log`. The focused DEV primitive and the complete
final xtask run passed after the real thumb correction.

The [sanitized capture package](theatre-native-step2/README.md) retains eight final PNGs with
semantics/hash sidecars and two before pairs. This is JVM desktop evidence, not native safe-area,
TalkBack, VoiceOver, screen-reader or device/audio acceptance. Full run records remain ignored.

| Acceptance gap | Status / reason | Next acceptance / owner |
| --- | --- | --- |
| Android install/native audio/force-stop/background/lock screen/focus/seek | NOT_RUN: `adb devices` has no attached target | Install exact APK; run requested journey and fault cases on named target; #14/#15/#17/#179 |
| iOS framework and app build | UNSUPPORTED: Linux has no Xcode/pinned Native toolchain | macOS `theatre-host ios-build`, Xcode scheme `Cantos`; #14 |
| iOS current Ogg/Opus decoder | NOT_RUN / compatibility unaccepted | Representative actual bytes on supported iOS; review rendition/decoder decision if unsupported. No on-play transcode; #14/#120 |
| Native login and registration | NOT_RUN / browser boundary and native registration capability unresolved | API-confirmed session through reviewed browser-facing issuer; mailbox proof remains separate; #191/#174 |
| Real Library/progress receipt and cross-client sync | NOT_RUN | Actual native session + listener API, offline/restart/reconnect, lost response and second-client conflicts; #17/#35/#179 |
| Authenticated Library UI state matrix, focus and native screen readers | NOT_RUN | Production account/Library UI with real rows on relevant viewports and platforms; current inspected matrix covers the player and unavailable storage branch; #35/#179 |
| Advertised HTTPS media path and dataset | NOT_RUN by this native slice | Qualify step-1 release through installed native decoder, real grants/Range/expiry/withdrawal; #170/#14 |
| Remote tester distribution | NOT_RUN, no named authorized Firebase app/recipients supplied | Exact package, app mapping, cert, version/origin and approved recipients; #184 |
| Shared component qualification | Existing repository admission failure remains | Renew attributed qualification through existing UI workflow; never edit evidence hashes alone; #179, owner unassigned |

No provider cost, synthesis, private media upload, publication, deployed infrastructure change,
real manuscript, signed URL or credential is introduced by this work. Performance/capacity,
native backup/file protection and screen readers were not measured. These gaps remain open in
#14/#17/#35/#191/#174/#184/#179/#180; the full multi-platform task is not accepted by compilation.
