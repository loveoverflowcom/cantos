# Live client boundary evidence

Review boundary: Studio Catalog/Content/Brief/private Audio Lifecycle forms, plus explicit
unsupported Theatre Web/common-JVM transport seams. [API matrix and acceptance](../work-plan/live-client-api-matrix.md)
record the available routes. This completes no public release, listener API or broad #51/#64/#66
acceptance. User end-to-end acceptance remains separate.

## Source and ownership

BASE `7396ede88466ef56dc9e7759956a4072d6672a5b` (fresh develop, PR72).
Implementation checkpoint `d2d784fda70a9c1b72ae7e0903b3766a309ee388`, tree
`64c3ac71796db2db80fbc36eb61ede64ec9deee0`; completion records after that pin are documentation.
Worktree `/private/tmp/cantos-live-clients`, branch `feat/live-client-boundaries`.
The original checkout and all other worktrees were preserved; PR69 was not modified.

Backend routes, migrations, domain policy, publication gates and media storage are unchanged.
Apps consume `cantos-contracts` over HTTP, never service/app internals. Native Theatre retains
one existing mock root player; its live probe is a separate authority with no media engine.
Studio remains Web-only. No new framework, provider, CI lane, generation, mixer or encoder was added.
Kotlin promotes its existing serialization dependency for the concrete error-envelope consumer.
Theatre Web consumes only shared `ApiError`/route constants from the DTO crate; the preflight
package-graph fixtures now include that dependency.

| Claim / owner | Failure mode | Actual oracle / level |
| --- | --- | --- |
| exact write ownership / Studio request core | old/new actor or late response reports a saved draft | pending/cancel/newer/session exchange and acknowledgement actor regressions; example-tested |
| immutable replay snapshot / Studio request core | retry changes operation, revision or payload | original JSON retained through transport loss, authentication denial and owner→other→owner; example-tested |
| owner reconciliation / Studio request core | revoked producer bearer makes committed bytes irrecoverable | exact owner receipt after unknown upload, full binding/checksum/length matching, mismatch keeps unknown state; example-tested |
| exact review consent / Studio core | confirmation for rendition A approves B or changed evidence | per-subject/kind/decision/notes/evidence negatives; example-tested |
| parent/unit facts / Studio projection | new work retains another work's unit or file | pure parent-reset regression, selected-unit initialization and file reset; example-tested/source-reviewed |
| raw response compatibility / contracts + client edge | successful PUT parsed as a status wrapper | canonical `AudioRenditionDocument` fixture,204revocation, status/revision/error negatives; differentially-tested |
| cookie/bearer separation / Fetch shell | producer request carries Studio cookie | explicit SameOrigin/Omit plans and separate redacted upload credential; example-tested/source-reviewed; browser interaction NOT_RUN |
| listener boundary / Theatre ports | private Studio API or mock data supplies playback | fixed reserved-group GET, explicit mock query, no credential input,200→Protocol; example-tested |
| Rust/Kotlin envelope compatibility / fixture owner | clients classify real404differently | six shared literal cases in Rust and generated Kotlin test source; differentially-tested |
| cancellation/session fences / Kotlin owner | noncooperative old result overwrites newer/session state | fake transport with NonCancellable late response, cancellation propagation; mock-integration-tested |
| actual JVM HTTP / adapter | cookies, redirects or unbounded response reach a private surface | fresh loopback HTTP server; empty Cookie/no Authorization, ambient-cookie refusal, redirect and8KiB bound; mock-integration-tested |
| bounded local verification / runner | disk exhaustion aborts tests or cleanup destroys recovery evidence | tested fresh-cluster admin URL/name guards and all five retained fixture paths; complete real PG/restart pass; example/integration-tested |

The PostgreSQL runner releases only completed suite databases in its newly initialized cluster,
after the child suite succeeds. It keeps revision, editor, direct Catalog, Catalog HTTP and Audio
restart databases until all existing recovery assertions finish. No inherited database URL,
production DB, force-disconnect, relaxed fsync or weakened assertion is introduced.
`CANTOS_TEST_CONTEXT=audio` selects only the retained AudioLifecycle fixture database/root through
the existing xtask-serving primitive; editorial remains the default, catalog is also selectable.

## Local checks

Mac ARM64, Rust1.87.0, PostgreSQL17.4, Trunk0.21.14, installed Gradle8.14.3/JDK24, Android SDK35.
Trusted existing FFmpeg/ffprobe binaries from the preceding authorized test environment were reused.
Synthetic Ogg test bytes stay outside Git. Shared Cargo artifacts were reused under Cargo locking;
the worktree's ignored `target/debug` links to that cache for the existing WAV/PG primitives.

Command prefix, in this worktree:

```sh
PATH="/Users/manhblue/.local/share/gradle/gradle-8.14.3/bin:/private/tmp/cantos-audio-tools:/opt/homebrew/opt/postgresql@17/bin:$PATH" \
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.87.0 RUST_TEST_THREADS=1 \
CARGO_TARGET_DIR=/private/tmp/cantos-production-brief/target cargo xtask <operation>
```

| Operation | Result | Attributable local output |
| --- | --- | --- |
| `verify rust` | PASS format/lint/workspace tests;148Studio tests | `/tmp/cantos-live-rust-final.log` |
| `verify repository` | PASS;225Python tests and whitespace | `/tmp/cantos-live-repository-final.log` |
| `verify web` | PASS all4gates: WASM lint, Studio/Theatre builds, shared preview build | `/tmp/cantos-live-web-final.log` |
| `theatre-demo verify` | PASS common/JVM compile/tests; new5common +2JVM HTTP, no failures/skips | `/tmp/cantos-live-cmp.log` and ignored Gradle XML |
| `verify postgres --keep` | PASS complete disposable PG suites plus HTTP/server/PG restarts, receipts/replays/private-byte integrity | `/tmp/cantos-live-postgres.log` |
| `theatre-demo android-build` | PASS debug APK, affected shared HTTP source compiled | `/tmp/cantos-live-android.log` |

Final preflight and explicit contract-suite results are recorded in the completion addendum below.
No remote CI was checked or awaited. Earlier diagnostic failures are not passing evidence:
unimplemented test scaffold, corrected navigation/dependency graph expectations, missing cache
path, restricted process tests (rerun outside sandbox) and disk exhaustion. The stopped failed
cluster's data and this task's temporary doctor build were reclaimed; raw logs remain. The complete
fresh PostgreSQL rerun with between-suite cleanup passed, preserving the restart coverage.

## Kani boundary

`RUSTUP_TOOLCHAIN=1.87.0 cargo kani --version` returned `no such command: kani`.
Kani execution is **NOT_RUN**; no toolchain was installed and no bounded-model-checked result is claimed.
Two `cfg(kani)` harnesses call actual `ProbeState` production methods in Theatre Web:

- `completion_requires_current_pending_generation`: all `u64` current/answered generations,
  four representative phase classes, one terminal reply; no assumptions or stubs, unwind2.
- `cancellation_never_wraps_or_accepts_previous_completion`: all `u64` generations, pending phase;
  checked exhaustion and rejection of the prior completion; no assumptions/stubs, unwind2.

Ordinary tests execute the concrete guard/exhaustion cases. The proposed proof boundary covers
pure Rust generation guards only, never Kotlin, Fetch, network, credentials, media or a full app.
On an approved configured Kani environment, run `cargo kani -p cantos-theatre --harness <name>`;
retain verifier version/output and rerun ordinary tests if the harness changes.

## Runtime, design and user acceptance

`CANTOS_TEST_CONTEXT=audio cargo xtask run cantos-studio --web --test-cluster` built and started
only the owned local API and Trunk processes against the retained disposable fixture. Trunk
served the Studio title and proxied `/api/`; the API remained an intake-enabled API-only host.
Both owned process groups were stopped by xtask afterward; the retained test cluster was stopped
and only its task-owned data reclaimed. No production identity or database was used.

CUA attempts found no in-app/Chrome browser and an empty enabled surface inventory, reporting:
“The Mac is locked and automatic unlock could not unlock it.” No workaround or security bypass
was used. Browser interaction, focus/accessibility, screenshots, viewport/theme/text-scale
inspection and user end-to-end acceptance are **NOT_RUN**. Android APK is **compiled**, not
launched/device-tested. iOS concrete HTTP transport, iOS compilation/runtime, native playback,
background/offline/progress, screen-reader and listening quality are NOT_RUN/not implemented.

Screen: Studio Content & audio, Low–Medium; Theatre live boundary, calm unavailable state.
Tokens: existing surface/foreground/spacing/shape/focus roles; no token source or shared component
API changed. App-local forms reuse established Studio actions and semantic native elements;
no new unqualified shared import site was added. Localization: vi-VN/en resources and mapped
status/error copy; actual font rendering not observed. Motion: no new motion; runtime not observed.
Visual/reference fidelity and accessibility findings remain pending the unlocked browser workflow.

Independent review at the implementation checkpoint reports `no-actionable-findings`; its ledger
and scope are in the [review record](live-client-review.md). All grounded findings were fixed and
covered before that assessment; the reviewer read the named logs without rerunning heavy gates.

Residual risk: user/browser acceptance, Kani, production identity and secure producer credential
handoff, iOS transport, object-store retention and public publication/delivery remain unavailable.
Browser reload loses in-memory intents/credentials; reconcile immutable IDs before new preparation.
New Catalog source/Script IR binding controls, work archive/reorder and Brief read-grant management
are UI follow-ups; typed Catalog adapter coverage alone does not prove those controls. There is
**no live Theatre integration** until its own API and immutable public delivery exist.

## Completion addendum

`cargo xtask preflight --changed-only` on the frozen source plus handoff/evidence documents
returned **PASS all16gates**, no skips. Attributable output:
`/tmp/cantos-live-preflight-final.log`. The mutation gate killed all9Script IR mutants; it is
not a client-state/Kani campaign. Kotlin preflight tasks reused current Gradle outputs; the
new common/JVM suites were executed earlier with the recorded XML.

```text
PASS         repository
  PASS         rust-format
  PASS         script-ir-reference
  PASS         rust-lint (workspace)
  PASS         rust-tests (workspace)
  PASS         dbsp
  PASS         wav-reference
  PASS         audio-inspection
  PASS         web-lint (cantos-studio, cantos-theatre)
  PASS         web-build (cantos-studio)
  PASS         web-build (cantos-theatre)
  PASS         ui-preview
  PASS         cmp-ui-preview
  PASS         theatre-cmp-demo
  PASS         mutation
  PASS         postgres
preflight: PASS
```

Explicit `cargo xtask verify contracts` also returned PASS script-ir-reference, contract-tests,
wav-reference and audio-inspection; `/tmp/cantos-live-contracts-final.log`. Final completion
records change documentation only; repository verification is rerun for those records.
No production or user end-to-end acceptance is inferred from these local gates.
