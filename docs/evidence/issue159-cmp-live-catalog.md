# CMP production catalog and server search

Source-bound implementation record for part of [#159](https://github.com/loveoverflowcom/cantos/issues/159),
based on `develop@e4d6ca23a51d3f1c1465c7c2da5c2a198f1c3e14`.
The issue and #14/#180 were re-read before implementation. The only open PR at selection was
#246, an unrelated database cutover. This record accepts no native release or phase exit.

## Review boundary

The production `PublicCatalogScreen` now composes discovery/server search, Work detail and
chapter/episode metadata through the existing `PublicCatalogController` and strict Discovery v1
adapter. Any validated server UUID works; no fixture-ID mapping or implicit sandbox fallback is
used. One catalog owner holds the committed query and cursor walk. Local editing, scroll, tab
and selected-unit metadata remain presentation state. UI locale never changes content language.

Search uses the existing `/api/v1/theatre/search` contract (Work title/description). The validated
query admits at most 512 UTF-8 bytes and no control characters before trimming; normalization
and matching remain server-owned. UTF-8 query encoding cannot inject a cursor/filter/limit.
Changed queries clear the old walk; repeated queries refresh it. Every request carries the
committed query and a fresh token. Cancellation and token fencing cover late A→B→A responses.
Retry, empty intermediate pages and bounded invalid-cursor recovery reuse the existing walker.
Invalid query input offers correction instead of repeated network retry.

The shell retains only Khám phá/Của tôi. Library explicitly explains its unconnected capability;
no Save, playback, progress, identity, download, transcript or membership state is fabricated.
Unit selection reveals its metadata and does not request Play. Exact immutable publication IDs
are retained; rendition selection/playback-manifest handoff remains #14/#170. Missing public
covers are labeled honestly (#171); the API's positive duration is never replaced with a fixture.

Android release can use an explicitly configured HTTPS origin. INTERNET is a deliberate new
catalog capability, with an exact permission allowlist and unchanged sandbox/test exclusion.
Only debuggable builds admit the existing loopback HTTP exception. The separate iOS production
project has bundle ID `com.cantos.theatre`, links shared-only `CantosTheatre`, and retains one
catalog owner. Its ephemeral URLSession bridge bounds body accumulation, rejects redirects,
disables cookie/cache/credential storage and cancels owned tasks. Platform TLS/ATS stays enabled.
Missing or rejected origin configuration renders the existing unavailable presentation.

No ADR, dependency, API DTO/schema, backend query, migration, publication rule or provider call
was added. The accepted [ADR 0035](../decisions/0035-theatre-catalog-unit-availability.md) and
[Discovery v1](../../packages/contracts/theatre-discovery-v1.md) remain the owning contracts.

## Local evidence and provenance

Reproduce with installed Rust 1.98.1, Gradle 8.14.3, JDK 17+, and Android platform/build-tools 35.
Full command output stays in ignored `artifacts/agent-runs/cmp-live-catalog-159/`.

| Layer / claim | Evidence and boundary |
| --- | --- |
| L1 architecture | Separate read-only review of core, adapter, presentation, Android and iOS hosts; no actionable findings remain. A copied demo iOS bundle ID was corrected and source-tested. |
| L2–L3 domain/reducer | `theatre-demo verify`: PASS, 217 shared + 219 sandbox tests, zero failed/skipped in the opt-in live run. Validated query, arbitrary server identities, precise failures, committed query/cursor retention, duplicate/empty-page recovery and stale result fencing; common tests executed on JVM. |
| L4 orchestration | Production controller/HTTP adapter/JVM transport against a JDK loopback HTTP fixture, including unloaded search results, page failure/retry and exact Work navigation. This is mock-integration evidence, not the Rust API. |
| CMP renderer | JVM Compose semantics/interaction tests submit Search/IME, clear, switch tabs, select units and change vi/en while preserving creator text and draft. Existing shared Button/TextField/NavTabs/BackButton/Badge APIs are reused; shared qualification remains a separate gate. |
| Anonymous live API | Documented HTTPS origin returned one empty `/works` page and an empty `Cantos` search through the real JVM adapter. Zero Works/units means no live detail, pagination or nonempty acceptance. No credentials, writes or media requests were used. |
| Android packaging | PASS: canonical unsigned release build and APK/AAB inspector. Both artifacts define 26,262 classes and exclude the 70-class sandbox inventory and three forbidden resources. No install/distribution is implied. |
| iOS | `theatre-host ios-build` reports `UNSUPPORTED`: installed Kotlin/Native is not the exact required 2.2.20. Swift/Xcode application compilation, launch and native HTTP behavior remain unverified. |
| Handoff checks | Preflight FAIL at repository; downstream gates NOT_RUN in that attempt. The untouched base reproduces 22 UI qualification rows. This diff still has 22, with the catalog row additionally naming newly consumed NavTabs/TextField. Their two-renderer admission is unresolved; the failure is retained. The final PR records independent available gate results separately. |

The supplemental `preflight --changed-only --skip repository` completed Rust format,
Script IR reference, Protobuf reference/schema oracle and workspace Clippy commands. It was
stopped during workspace test compilation when disk fell below 1 GiB; it has no completed
preflight summary and is not a passing handoff. Full Rust tests and later Forge/DBSP/WAV/audio/
Web/preview/mutation/PostgreSQL gates remain incomplete or not reached. No cache, source,
worktree, log, evidence or runtime data was removed. Independently, `cargo test -p xtask
--locked` passed 440 tests with two ignored, and the repository's existing Python regression
primitive passed 648 cases with eight skipped. These do not turn the repository gate green.
`adb devices -l` found no connected target, so Android runtime acceptance could not run.

Commands:

```sh
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-demo verify
CANTOS_THEATRE_LIVE_ORIGIN=https://api.cantoss.duckdns.org cargo xtask theatre-demo verify
cargo xtask theatre-host android-release --api-origin https://api.cantoss.duckdns.org
cargo xtask theatre-host ios-build --kotlin-native-home /absolute/installed/native-2.2.20
TMPDIR=/private/tmp cargo xtask preflight --changed-only
```

JVM captures and semantics are under `shared/build/reports/theatre-demo/catalog-*` in the mobile
Gradle root. Eight list/detail captures cover light/dark, vi-VN, 390×844 at 1× and 320×568 at 2×
text, density 1. Noto Sans/Serif and existing semantic tokens are unchanged. Opened captures
showed the refresh action squeezing the large heading; moving it into the wrapping action row
was rechecked on the same scenarios. Long text remains vertically scrollable. English copy and
locale switching have semantic tests; English pixels and native OS text scaling are unobserved.
These files are evidence-local-only, not committed or attached to GitHub. TalkBack/VoiceOver,
keyboard/focus walkthrough, reduced-motion signal and native insets/IME remain unobserved.

## Deferred acceptance

Before native acceptance, an owner must run the production Android host and the separate iOS
project against an authorized API containing representative released Works. Exercise arbitrary
UUIDs, novel/story chapters, podcast episodes, later pages, query races, page retry, detail/Back,
empty/revoked states, vi/en, 200% text, OS Back/IME/insets and screen readers independently on
both targets. A production catalog must never fall back to fixtures. Use the disposable
[seeded-catalog lane](../deployment/theatre-seeded-catalog.md) for synthetic data; never seed a
production database. Retain fixture evidence separately from real API/device observations.

Install/provision no native compiler, device, credentials or services as part of this record.
Once Native 2.2.20 and Xcode/simulator prerequisites are already available, link the shared
framework with the command above, then compile/run `iosApp/CantosProduction.xcodeproj` with
scheme `CantosProduction` and an explicit `CANTOS_API_ORIGIN`. Framework linking alone does not
compile the Swift host. Native runtime and Library/player follow-ups remain #14/#35/#170/#191;
no owner is assigned by this PR. No deployment, Firebase upload, merge or issue closure is
authorized by this evidence.
