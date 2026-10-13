# Cantos Theatre Web

The existing `cantos-theatre` target remains **Rust 2021 / Leptos 0.7.8 CSR/WASM**. It implements
a same-origin listener account boundary and an explicitly isolated session-only mock; users see
**Cantos**. The [PR 02 validation record](../../../docs/evidence/theatre-web-pr02.md)
names the local build, reducer/fake-port tests and integrated-browser observations. No production
listening issue or UI-system qualification is completed by this foundation.
The [library MVP record](../../../docs/evidence/theatre-web-library-mvp.md) scopes the
[#35](https://github.com/loveoverflowcom/cantos/issues/35) follow-up to this Web mock; CMP and
acceptance against real listener data remain separate.

## Scope and honest behavior

The discovery/player/library preview below remains isolated at `?mode=mock`. Its sample
state is never imported into the authenticated account, and its controls never call live APIs.

- Two primary navigation destinations: **Khám phá** (Discover) and **Của tôi** (My library).
  Search and filters belong to Khám phá; account opens through the avatar. Work details,
  full/compact mock player, queue, settings and unknown-route recovery remain hash-routed
  secondary views. The account view shows a guest identity, states that sign-in and sync are not integrated and that data lives only in this session, and links to listening settings and Của tôi; it draws no conflict choice because the mock has no conflict model.
- Của tôi presents **Nghe tiếp**, **Đã lưu** and **Lịch sử** directly, with no nested library tabs.
  Save/unsave acts on a work and is independent of playback position. There is no separate
  favorite/star list in this MVP. History previews three recent works and links to a secondary
  all-history view. Point bookmarks remain secondary mock-player tools.
- Deterministic Vietnamese titles/synopses, a long title and an unavailable sample episode.
  Sample availability is not publication, rights or backend authorization.
- Separate saved works, point bookmarks, explicit session progress and fake playback history.
  History records a matching accepted fake start, keeping the latest sample episode per work
  in recency order; page visits, saves and manual seeks alone do not add history. It is not a
  real audio or completion record.
  Clearing history requires confirmation and preserves progress, saved works, the root player
  and queue. The existing separate demo Reset/clear-library controls still clear their broader
  local sample state. Reload loses every session fact.
- A single root-owned `PlayerSession` persists across route changes. Play dispatches intention;
  only a matching **fake-engine** `EngineStarted` confirms **simulated** running. There is no
  audio, media element or playback clock. Seeking/skipping changes manual position, preserving
  intent; a fake end event, rather than seeking to the end, labels the sample ended.
- The full player's clearly labeled demo controls inject fake buffering, acknowledgement,
  interruption, no-auto-resume and in-memory restore events. Restore offers a position while
  paused. Reset advances generation instead of reusing an old callback identity.
- Vietnamese/English typed resources, `vi-VN` content, system/light/dark presentation,
  localized speed/duration, native accessible links/buttons/search/range controls.
- Loading/empty/error catalog scenarios use a deterministic fake port. Loading remains pending
  until Show sample data; Retry requests local samples only. Only the newest issued pending
  request can settle; unissued, older/future and duplicate terminal callbacks are ignored. Refresh
  advances the checked generation. Recovery returns focus to main; clearing search returns it to
  the search field.

Within the explicit mock, no API, authentication, network client, real audio/download, timer,
sync, disk/account persistence, billing or provider code is integrated. Unsupported actions stay disabled
with associated reasons. The fake adapter is not browser/native audio integration, and the
in-memory cold-restore exercise is not reload/session persistence.

## Implemented ownership and dependency direction

| Module | Concrete responsibility |
| --- | --- |
| `src/app/context.rs` | One root composition: independent catalog/player/library/progress/queue handles; explicit user commands; locale/theme/navigation |
| `src/app/routes.rs` | Typed hash routes; no player construction |
| `src/core/ids.rs`, `catalog.rs` | Synthetic typed keys and catalog read models; no publication authority |
| `src/core/playback/` | Framework-free immutable state, events, ordered effects and `reduce(state, event) -> Transition` |
| `src/core/listening/` | Separate saved-work/bookmark, progress, simulated history and queue facts/rules |
| `src/application/player_session.rs` | Commit pure transitions, then interpret effects through `MediaPort`; derived status/view `Memo`s |
| `src/application/catalog.rs`, `ports.rs` | Catalog request/result identity and concrete catalog/media consumer boundaries |
| `src/application/account.rs`, `account_view.rs` | Pure session generation, logout acknowledgement, immutable intent ownership and private read projection lifecycle |
| `src/live/account.rs`, `library.rs`, `runtime.rs` | Same-origin Theatre BFF transport, bounded actual Library reads and mounted browser lifecycle; no mock fallback |
| `src/infrastructure/` | Only implemented `MockCatalog` and deterministic `FakeMedia`; no empty future adapters |
| `src/features/` | App-private discovery/search/work/library/player/queue HTML compositions; screen-owned search/filter and confirmation state |
| `src/view.rs` | Root route composition plus existing shared-component compatibility import site for settings/recovery |
| `src/messages.rs`, `platform.rs` | Typed locale resources; fallible browser navigation/title/focus only |
| `src/styles/theatre.css`, `assets/icons/` | Canonical-token layout; pinned licensed official Material Icons |

The core imports no Leptos, browser, I/O or adapter. The application owns effect execution; features
receive capability handles and never import infrastructure. Player status and view are derived
with `Memo`, not maintained by an effect copying state. Local progress is recorded only by explicit
user commands; it makes no completed-listen or synchronization claim. No global `AppStore`, shared
runtime, FFI, state-management framework, speculative crate or empty production adapter is added.

Raw `ResumeSnapshot` events remain able to carry invalid controls for explicit rejection.
`ValidatedResumeSnapshot` admits position within the source duration and the five supported
mock speeds; reducer loading and retained `ProgressState` consume that immutable value.
The player shell exports admitted checkpoints and progress cannot retain a raw event value.
This is session-only validation, with synthetic episode identity and no publication authority.

### Shared mock semantics

The independent Rust and Kotlin reducers share **data**, not runtime code:
[`theatre-mock-playback@1`](../../../packages/contracts/fixtures/theatre/playback/v1/README.md).
Its schema/index and seven hand-authored vectors specify full state, derived status and ordered
effects for play/pause, buffering, interruption, stale responses, restore, preparation races and
invalid controls. The native Rust harness embeds the exact corpus, rejects unknown versions,
fields/tags and bounds, and compares every step to the authored expectations. Rust native replay
passed in PR 02. The WASM app compiled and ran in the browser; the vector test harness was not run
on WASM, and Kotlin was not tested in this PR.

Every callback must match source identity and monotonically checked generation. Play, pause,
seek, source change, restore and interruption boundaries invalidate old work. Prepare/Start effects
carry complete source/position/speed, so a new intent during preparation establishes a viable
current-generation path rather than waiting on discarded readiness. Generation exhaustion rejects
transactionally, without wrapping. These are pinned **mock** rules, not production OS autoplay,
media-version or listener synchronization policy.

Production implementation still follows [Web listening](../../../docs/work-plan/040-theatre-web-listening.md),
[listening rules](../../../.agents/skills/cantos-listening/SKILL.md) and
[API boundaries](../../../docs/decisions/0015-api-surface-boundaries.md). Future Theatre adapters
must consume only backend-authorized Theatre data. Private drafts and TTS previews never become
public releases through a frontend reducer; backend publication authorization remains authoritative.

## Design and component admission

The [Web reference](https://drive.google.com/file/d/1UEMLvfNdorz4P9qZgfoVkp2HwQV_Ipwt/view)
and [handoff](https://drive.google.com/file/d/1d2LA4MxQmcW02gUJElz9RseBWV0sAAah/view) were read in
the initial mock phase; their board and original artwork were visually inspected. They are design
references, not changed-app browser screenshots. Noto Sans/Serif (OFL, Vietnamese subset) and the two mock paintings are bundled as Web-sized derivatives with
[provenance](assets/provenance.json); works without a painting keep the typographic cover. Artwork usage rights are
unverified. There is no font/icon CDN. See the [library/account v2 record](../../../docs/evidence/theatre-web-library-account-v2.md).

Paper/purple v0.2.0 roles, tonal grouping, responsive rail and existing focus/reduced-motion/forced-
color styling are retained. Shared imports `UiRoot`, `Container`, `Surface`, `Stack`, `Select` and
`EmptyState` stay at the **same `src/view.rs` compatibility site**. Feature compositions use native
HTML and existing canonical tokens, not new reusable shared primitives. No token, preview catalog,
qualification, admission baseline or exemption is changed. The shared catalog remains **0/29
qualified**; new reusable adoption still requires [both-preview admission](../../../packages/ui-system/shared/README.md#preview-registration-and-adoption).
PR 02 adds inspected app captures and keyboard/focus observations at named viewports/themes.
The library MVP has its own [observations and limitations](../../../docs/evidence/theatre-web-library-mvp.md);
earlier captures do not establish its changed navigation or library behavior.

## Build, run and checks

Use the existing [xtask commands](../../../xtask/README.md), Rust 1.87,
`wasm32-unknown-unknown` and Trunk 0.21.14:

```sh
cargo xtask doctor
cargo xtask build cantos-theatre --web
cargo xtask run cantos-theatre --web
cargo xtask verify repository
cargo xtask verify rust
cargo xtask verify contracts
cargo xtask verify web
cargo xtask preflight --changed-only
```

Run serves `127.0.0.1:8082`; its Trunk proxy forwards `/api/` to the existing API on port8080.
The frontend-only `run cantos-theatre --web` does not start or provision a backend. For the live
account boundary, use the existing API/app commands in separate terminals:

```sh
cargo xtask run cantos-api --web-origin http://127.0.0.1:8082
cargo xtask app cantos-theatre --web
```

The API needs an operator-supplied migrated local app-role `DATABASE_URL` (or its documented
retained disposable test-cluster option). Theatre OIDC remains opt-in through the existing
`CANTOS_THEATRE_OIDC_ISSUER` and separate `CANTOS_THEATRE_OIDC_SECRET`; approved client/callback,
scope, credentials and TLS/provider acceptance are operator prerequisites, never provisioned by
these commands. The current development host has one `CANTOS_WEB_ORIGIN`, so this recipe is a
Theatre-origin profile. Simultaneous Studio on8081 and Theatre on8082 authentication is unverified.
Without an available/configured identity backend the live view reports failure; it does not
create a guest account or substitute mock data. The explicit mock works without an API.

Tests belong to the existing Cargo workspace and `verify rust`: pure reducers, exact
shared-vector replay, fake ports, stale callbacks, account fences, actual Library fixture decoding,
a single mock player across routes, resources and Leptos native string-render compositions.
`serde_json` remains at already-locked 1.0.145. No alternative test/run orchestrator is introduced.

The [initial cloud execution record](../../../docs/evidence/theatre-web-mock.md) retains its
historical tool limitations and later pre-merge run. PR 02 uses the existing local Rust 1.87.0 and
Trunk 0.21.14 toolchains; see its [current evidence and remaining limits](../../../docs/evidence/theatre-web-pr02.md).
The historical PR02 full contracts suite was `UNSUPPORTED` because ffmpeg/ffprobe were absent; no
listener API, fixture corpus or media-inspection code is changed.

## Browser scenarios

Repeat these scenarios when the affected behavior changes. PR 02 exercised the subset recorded
in its evidence at 1440×1000, 1024×768, 390×844 and 320×800, light/dark and both locales.
200% text zoom and assistive technology still require separate observations:

1. Khám phá → work → sample → Player → Của tôi → Back/Forward. The same root player identity
   and position survive navigation. Opening a different sample records progress and selects it paused.
2. Use Skip to main content; confirm focus without a route change. Check focus rings and that the
   mini-player does not hide focused controls at narrow/zoomed widths.
3. Play/pause, manual seek and clamped skips; fake-buffer and confirm recovery. Seek to the end
   without claiming a completed listen; inject a fake end, then explicitly replay from zero. No sound
   or clock movement is expected.
4. Start an interruption; pause during it; end with no automatic resume. Restore the current
   in-memory snapshot; it stays paused until explicit Play. Repeated restore/Reset advances identity.
5. Save/unsave a work from its detail and Của tôi; verify both surfaces agree. Simulate playback,
   then open Của tôi and inspect Nghe tiếp, Đã lưu and Lịch sử without changing sub-tabs.
   Repeat Continue without alternating to an older progress record. Add the same bookmark/queue
   item twice; remove focused and last rows; verify focus/recovery and exact bookmark resume.
6. Change catalog scenarios, retry and clear filters. Player/library remain intact. Open history
   clear confirmation, cancel once and confirm once; verify only history disappears and progress, saved works,
   player and queue survive. Search/filter and confirmation state belongs to its screen.
7. After Reset, stale bookmark clicks and older engine acknowledgements must not reopen a removed
   bookmark, navigate on a rejected action or revive old progress. Continue uses the current seed;
   an empty Continue/Next remains a no-op. These Reset paths have synchronous fake ports.
8. Check all disabled reasons, selects, hash deep links/unknown routes, Vietnamese long titles,
   reduced motion and forced colors. Verify the demo labels remain visible on every player state.

Real audio, release/media-version pinning, reload persistence, CDN delivery, progress reconciliation
and Android/iOS/background/native-engine acceptance remain future work. This mock closes no
production or UI-system acceptance gate.

## Explicit live and mock modes

The default entrypoint mounts the actual Theatre Web BFF account consumer. It bootstraps with
`GET /api/v1/theatre/session`, redirects sign-in through `/api/v1/theatre/auth/login` and logs out
with `DELETE /api/v1/theatre/session`. Provider tokens never reach JavaScript or browser storage.
The account view uses Vietnamese/English resources and distinguishes pending, guest, authenticated,
unavailable/protocol failure and unacknowledged logout. An expired session hides private data
immediately and requires acknowledged session revocation before another login. Cleanup is stale-only;
explicit logout pins the observed session. A cookie rotated in another tab is rechecked rather
than revoked by a delayed request.

The authenticated session reads the first 20 Continue, Saved and History items through their actual
Library endpoints, using JSON and same-origin cookies. The UI shows accepted progress positions
and page counts; no Library mutation, pagination UI, real playback, history-start event, automatic
guest import, durable outbox or full synchronization is introduced. Network/body bounds and
the session-correlated response header are checked before any private projection is displayed.
Account/session generation fences both success and failure; switching accounts never relabels an
old intent. Session checks repeat on window focus and once per minute while an authenticated page
is visible; request batches have a 15 second deadline and refresh refuses duplicate pending reads.
The timer/listener are removed on unmount. These implemented paths still require actual browser,
provider/device and production acceptance; source/fixture tests alone do not establish them.

Open `?mode=mock` explicitly for the existing isolated UI preview. An unknown mode never falls
back to mock success. No Studio session, producer token, private submission or draft is available
to either listener mode. The live account projection is not live Theatre playback integration.
