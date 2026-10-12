# Theatre Web session-only mock evidence

Date: 2026-10-12. Requester-authorized scope: initial Leptos mock plus the architecture follow-up,
on the assigned dot cloud computer. Base `develop`: `8eb7f495fd07936295bda328b7392e83ed97ffa5`.
Existing draft [PR61](https://github.com/loveoverflowcom/cantos/pull/61), same branch; no merge or
listening-issue closure. Source is implemented; compilation/runtime verification is **incomplete**.

## Invariant and ownership ledger

| Invariant | Owner / cheapest oracle | Evidence |
| --- | --- | --- |
| No real API/auth/audio/storage/publication behavior | app imports, fake adapters, shell negative assertions | source reviewed; browser/build NOT_RUN |
| Pure immutable playback decisions, ordered effect data | `core/playback/reducer.rs`; full-state targeted/sequence tests | tests authored, NOT_RUN |
| Play is intention; only matching fake Started confirms simulated running | reducer; fake-port and deferred-port tests | source reviewed; tests NOT_RUN |
| Old source/generation callbacks never override new intent | checked identity; stale/preparing-race vectors and deferred callback tests | tests authored, NOT_RUN |
| New commands during prepare establish viable current-generation effects | full facts in Prepare/Start effects; preparing-races vector | source reviewed; tests NOT_RUN |
| Cold restore never starts; Reset never reuses identity | restore transition; PlayerSession/context tests | tests authored, NOT_RUN |
| One root player survives routes | `TheatreContext::new`, root composition, route/string-render tests | source reviewed; tests NOT_RUN |
| Catalog stale response cannot replace newer state | CatalogState begin/accept; old ready/error result tests | tests authored, NOT_RUN |
| Saved work, bookmark, progress and queue are distinct | separate core/listening modules and explicit application commands | source reviewed; tests NOT_RUN |
| Bounds/rejection/repeated actions are transactional | source constructor, reducer/capability tests, invalid-controls vector | tests authored, NOT_RUN |
| Common corpus is exact, not generated from a reducer | v1 schema/index; 7 authored vectors, strict Rust test-only consumer | JSON/schema/SHA-256 diagnostic passed; reducer replay NOT_RUN |
| UI resources and component admission remain honest | resource parity test; existing preview/adoption primitive | supplemental diagnostics passed; no renderer qualification |

The architecture is an app-local mock core, not a production listener contract or shared runtime.
Core depends on values only; application commits a transition before invoking the fake MediaPort.
`Memo` derives status/view/search results. No reactive effect copies player facts into another store.
Local progress recording occurs only in explicit user commands and grants no completed-listen or
sync fact. There are no empty production adapters, FFI, new state framework, HTTP DTOs or backend
changes. Backend publication authorization remains authoritative; draft/TTS preview is not release.

## Shared fixture handoff

Canonical contract: `theatre-mock-playback@1`, vector format 1. Fixture-only commit:
[`56bb39bcd769befd8385e7cb963b7ff4930f3c89`](https://github.com/loveoverflowcom/cantos/commit/56bb39bcd769befd8385e7cb963b7ff4930f3c89),
tree `dac6fa1b6a8d6bb6e3543a5fb82b6677b9fbaccf`. It can be reused by CMP independently of Web
source. [Canonical rules/schema/index](../../packages/contracts/fixtures/theatre/playback/v1/README.md)
are shared as data, not a language binding or playback acceptance.

- Index SHA-256: `108503bdcceb36fe6351b1d320075d974615f7dc0ea560582bbe19c2da2b0485`.
- Schema SHA-256: `0c6130a831dd4486b07736888ed0f6df0f43d1faee68ce3a70fb5a9351c02000`.
- Seven vectors, 51 steps: play-pause, buffering, interruption, stale-response, restore-session,
  preparing-races and invalid-controls. Each compares full state, derived status/pause cause and
  ordered effects. Expected values were authored independently, never generated from the reducer.
- The native Rust consumer strictly rejects unknown version/contract/tag/field, missing values and
  bad integer bounds; its embedded inventory matches the index. Index SHA values and a labeled
  non-cryptographic byte-drift sentinel are pinned; separate Python checked actual SHA-256 bytes.
- Rust/WASM and Kotlin/JVM/Android/iOS results must be reported separately. This record makes
  **no claim that either or both platform reducers passed**. Kotlin evidence belongs to its PR.

## Execution record

Python 3 and Git are available. Cargo, rustc, rustfmt, rustup and Trunk are absent on PATH; the
previous task-local toolchain path is absent. No download/install, TLS workaround, environment
switch or CI dispatch/polling was attempted. No compiled app or changed-app capture exists.

| Command / check | Actual result | Boundary |
| --- | --- | --- |
| `cargo xtask verify repository` | blocked before xtask: `cargo: command not found` | no xtask summary |
| `cargo xtask verify rust` | blocked before xtask: `cargo: command not found` | format/Clippy/native tests NOT_RUN |
| `cargo xtask verify contracts` | blocked before xtask: `cargo: command not found` | contracts gate NOT_RUN |
| `cargo xtask verify web` | blocked before xtask: `cargo: command not found` | WASM Clippy/Trunk builds NOT_RUN |
| `cargo xtask preflight --changed-only` | blocked before xtask: `cargo: command not found` | aggregate/impact selection NOT_RUN |
| `python3 scripts/ui_preview_catalog.py --check` | diagnostic printed PASS | structural drift/adoption only; not renderer acceptance |
| `python3 scripts/check_repository.py` | links/JSON/skill diagnostic passed | supplemental, not docs-only fallback for code |
| `git diff --check` | passed | whitespace only, not rustfmt |
| JSON Schema 2020-12, exact corpus inventory/hash/bounds/NFC | supplemental diagnostic passed | no reducer or platform test execution |
| Resource/placeholder (132 keys), SVG XML/hash, CSS roles (48 canonical) and dependency-boundary scans | supplemental diagnostics passed | not Rust parsing, compilation or live accessibility |
| Native/Web/WASM tests, app build/run and browser/device/media acceptance | NOT_RUN | no supported toolchain/runtime artifact |
| Remote CI | not checked | no polling, rerun or dispatch |

Tests are in the existing Cargo workspace, not a competing test orchestrator. Coverage includes
pure reducer rejection/immutability/idempotence, bounded event sequences, exact shared-vector
replay, strict decoding/inventory, fake/deferred media ports, stale catalog completion, root player
route persistence, repeated Continue, unavailable sample refusal, resource parity and Leptos native
string-render compositions. None executed. Test-source presence is not `example-tested`,
`DOM-tested`, `mock-integration-tested` or `interaction-tested` evidence. Even passing native
string-render tests would not prove browser focus/history/keyboard/reflow or screen-reader behavior.
The existing package lock changes only the `cantos-theatre` dependency list for already-locked
web-sys 0.3.77 and test-only serde_json 1.0.145; no package/version update is made.

## Independent review and fixes

An independent read-only architecture reviewer inspected the pure reducer, canonical corpus,
application/fake ports, capability wiring and UI boundaries using the relevant repository skills.
Confirmed findings were corrected:

- Added the pinned test-only serde_json dependency; transitive dependencies are not usable imports.
- Continuing an older recorded position initially made the outgoing position the newest progress
  fact, allowing repeated Continue to alternate sources. Successful selection/restore now records
  its explicit position as newest; a repeated-Continue regression test was authored.
- Removed broad copied feature imports, obsolete catalog bypass helpers and the global DemoState.
- Seeking back from an observed ended phase now labels the action Play, not Replay-from-zero; a
  regression string-render/session test was authored.
- Manual range input now uses a checked seconds-to-ms conversion. UI status/rejection, simulated
  buffering/end/interruption/restore and disabled explanations remain explicitly mock-labeled.

Source/API inspection includes official tagged Leptos 0.7.8 `StoredValue`/LocalStorage construction
and local value access; that is API inspection, not compilation. Original source-level fixes remain:
skip-link focus without route change, focus fallback after last-row deletion, stable save/queue
labels, associated disabled reasons, canonical on-primary-container pairing and type roles,
localized speed/plurals, document title and elapsed/total spoken position, keyed typed IDs.
These have not been interaction-tested. Reviewer assurance remains limited by the missing compiler
and browser app; a draft is not ready-to-merge or production acceptance.

## UI and design evidence

- The initial phase retrieved/read [Web HTML](https://drive.google.com/file/d/1UEMLvfNdorz4P9qZgfoVkp2HwQV_Ipwt/view),
  [package](https://drive.google.com/file/d/1XasS1_1Qi5gQrICwp7fb1kzqBzegEGZb/view) and
  [handoff](https://drive.google.com/file/d/1d2LA4MxQmcW02gUJElz9RseBWV0sAAah/view) through Drive.
  The design board and three original paintings were opened and inspected. They are reference
  pixels, not changed-app screenshots. Reference paintings/fonts were not committed.
- [Canonical tokens v0.2.0](../../packages/ui-system/shared/tokens.json) and
  [official Material icon provenance](../../apps/cantos-theatre/web/assets/icons/manifest.json)
  are unchanged. Typographic covers/system fonts are intentional mock substitutions; no CDN.
- Existing shared imports stay at `src/view.rs`; app-private feature HTML is not a new shared UI
  system. Catalog/admission baseline remains unchanged and **0/29 qualified**. No new reusable
  component is registered, accepted or secretly granted an exemption.
- Flows: discovery/search/work → root full/compact player; library/bookmarks/progress, queue,
  settings/catalog recovery and unsupported account. Surface intensity: discovery/library Medium,
  mock player Medium-High, mini-player Medium-Low; canonical paper/purple tonal grouping retained.
- vi-VN/en resources and `vi-VN` sample content. Actual narrow/wide/light/dark/zoomed pixels,
  contrast over live content, focus/text shaping/assistive technology, motion recordings, hash
  Back/Forward, input and async callback runtime observations remain NOT_RUN.
- Media path: deterministic fake acknowledgements only; no browser/native audio or streaming.

Next: run every required xtask gate on an authorized supported toolchain and fix any resulting
compile/format/lint/test failures; then perform the [manual browser scenarios](../../apps/cantos-theatre/web/README.md#manual-browser-scenarios-still-required).
New shared components still need both-preview/runtime/accessibility admission. Real listener API,
publication rights/release pinning, playback, reload persistence, CDN/progress synchronization and
native/background/device gates remain separate. No production issue is closed by this mock.
