# Cantos Theatre Web

The existing `cantos-theatre` target remains **Rust 2021 / Leptos 0.7.8 CSR/WASM**. It implements
a public discovery Home, [live guest playback](#live-guest-playback) of published units, a
separate listener account/auth boundary and an explicitly isolated session-only mock; users see
**Cantos**. The [PR 02 validation record](../../../docs/evidence/theatre-web-pr02.md)
names the local build, reducer/fake-port tests and integrated-browser observations. No production
listening issue or UI-system qualification is completed by this foundation.
The [library MVP record](../../../docs/evidence/theatre-web-library-mvp.md) scopes the
[#35](https://github.com/loveoverflowcom/cantos/issues/35) follow-up to this Web mock; CMP and
acceptance against real listener data remain separate.

A static release bundle of this app can be published to Firebase Hosting with the explicit, manual
`cargo xtask deploy firebase cantos-theatre --web --project ID --channel NAME`
([runbook](../../../docs/deployment/firebase-hosting.md)). Hosting serves static files; clients call
the explicit Cantos API origin. Catalog and authentication require separate API deployment and
browser acceptance; [#295](https://github.com/loveoverflowcom/cantos/issues/295) selects API-issued
HttpOnly cookies on the custom domains, and this source now consumes the
[Identity v1 contract](../../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) that way (no Bearer/OIDC
fallback). That consumer has run against no backend or browser yet; static delivery establishes
no playback.
The preferred shorter site `cantos.web.app` is conditional on verified allocation; the technical
module stays `cantos-theatre`. Its [site transition](../../../docs/deployment/firebase-hosting.md#shorter-site-names-controlled-transition-237)
follows ADR 0041 direct Hosting/API addressing and exact-origin review. Cross-site authentication
needs its own declared preview policy and browser acceptance; it does not inherit #295
production custom-domain cookie qualification.

The [performance pattern audit](../../../docs/evidence/web-performance-patterns.md) measures
release assets with `cargo xtask web-performance cantos-theatre` and narrows mock-player/library
reactive projections. Deterministic work counters and bundle bytes are separate from browser
latency, actual audio and production static-cache acceptance.

The [player interaction record](../../../docs/evidence/theatre-web-player-interaction.md)
describes the isolated mock's local scrub draft, one final seek, root-owned pending
acknowledgement and narrow transport projection. It includes the A/B/C decision and deferred
browser/audio acceptance; no browser latency improvement is claimed.

## Live guest playback

Live mode lets a guest hear a published unit ([#269](https://github.com/loveoverflowcom/cantos/issues/269);
[evidence and limits](../../../docs/evidence/theatre-web-guest-playback.md)). Choosing a unit (or
**Nghe** on a Work, which starts its first listed unit) resolves that exact release's manifest
(`GET /publications/{id}`), pins `work / unit / publication / rendition`, manifest digest, SHA-256
and the verified duration, requests `POST /publications/{id}/playback-authorizations` with an empty
body and hands `PlaybackAuthorization.url` to the audio element exactly as returned, so the browser
streams it with `Range` requests. The URL is a bearer capability: it is never parsed for identity,
logged, persisted or fetched into a `Blob`.

- **One owner.** `LiveBoundary` creates one `PlayerSession` and one `<audio id="theatre-audio-engine">`
  for the page. Home, Work, `#player` and the mini-player read the same session; browsing never
  replaces the source. A new source drops the old source's listeners, requests and timers, and every
  callback carries the identity it belongs to, so a cancelled source or intent cannot change the
  current state.
- **No autoplay.** Only an explicit press resolves or plays anything. Opening a Work, `#player`,
  Back/Forward, reload and sign-in start nothing; `#player` without a source shows an empty state.
- **Truthful status.** The indicator says playing only after the engine acknowledged it. A refused
  `play()`, a withdrawn release, a changed release, a transport failure and an undecodable format
  each have their own message. A pause the page did not ask for (media key, headset) becomes a
  pause intent. Pressing Play again is the explicit retry.
- **Bounded recovery.** The grant is refreshed 30 s before `expires_at_ms` for the **same**
  publication and compared with the pin; a different publication/rendition/SHA stops playback
  instead of switching. A `404` stops at once; `503` backs off 1/2/4 s over four attempts; an
  element failure is recovered at the last position at most three times without progress.
- **Not covered here:** search and routing (#270), offline,
  queue, transcript, Safari/iOS and deployed CDN/CORS behavior.

## Discover layout

Discover follows the Home anatomy of the design ([#282](https://github.com/loveoverflowcom/cantos/issues/282),
[record and gaps](../../../docs/evidence/theatre-web-discover-parity.md)): intro, a hero for the
first Work of the answered search, then the search/filters and a grid of Work cards. The hero
and cards use a neutral cover fallback because the public contract carries no cover, and the
hero's eyebrow never claims an editorial pick. The content language is chosen by name; the
request still carries its tag. The design reference was not re-rendered for this change.

## Scope and honest behavior

Account has no manual session refresh while Guest or Signed-in. Automatic session checks,
renewal and cookie/session fences remain with the runtime. A failed session read offers one
contextual **Thử lại / Try again**; unresolved sign-out retains the separate **Finish signing out**
action and cleanup barrier. Read recovery never repeats a sign-in or signup mutation.

Auth/account/signup feedback projects typed facts through the shared `StatusTone` vocabulary:
normal guests are neutral, loading and accepted signup receipts informational, rate limits and
unknown outcomes warnings, and unavailable services or refused proofs errors. Invalid email
uses the existing `TextField` field error once. Register owns its signup feedback and shows
session feedback only for unresolved sign-out. Existing status/focus IDs and one caller-owned
announcement per feedback region are retained.

These are Web-owned live regions using canonical semantic roles from token version 0.6.0.
The shared `Status`/`ErrorState` APIs lack both-renderer qualification for new Theatre import
sites, so this composition does not import them or alter the admission baseline. Their future
migration needs the existing qualification process; source registration is not runtime approval.

The default entrypoint opens public Home without a login wall. It reads actual published work
summaries anonymously, independently of session availability. Account opens through the avatar;
Login and Create account are explicit secondary routes. The two primary destinations remain
**Khám phá** and **Của tôi**, following [#35](https://github.com/loveoverflowcom/cantos/issues/35).
The Auth–Account v2 reference's third primary Account destination is intentionally not adopted,
as required by [#129](https://github.com/loveoverflowcom/cantos/issues/129).

The live Account/Login surface offers email/password sign-in to every guest behind an
acknowledged session-cleanup barrier; there is no provider capability to read first, so an API
without Identity answers the attempt with a clear `404`-based message instead of a gate. The
browser keeps the API-issued HttpOnly cookie; this app only holds nonsecret session facts
(actor, session id, deadlines) in RAM under [the shared owner](../../../packages/web-auth/README.md).

The live **Của tôi** reads Continue, Saved and History with `presentation=1`, limit20 and independent
opaque cursors. It renders released Work/unit titles and canonical UUID Work links directly from
`Presented*Page` DTOs, retaining exact audio identities, progress revisions, history state and
resume status. Each section owns loading/empty/page-error/retry/load-more; one Library action refreshes all three lists. Failed next
pages retain loaded rows; expired cursors require a fresh walk of that section. Null metadata
and replaced/unavailable releases retain their records without borrowing another release's title.
Logout, expiry and session change hide all private lists before new admission; late results are
fenced by account/session and page generation. Reading/rendering/navigating never writes progress,
Saved or history and never starts media.

A terminal empty section (`Ready`, no rows, no next cursor) uses its existing Theatre Outlined
icon and contextual guidance. When all three sections are terminally empty, one invitation
links to Discover and the section explanations form a compact responsive group. Partial empty
sections keep their own Discover link. An empty page with a cursor still offers Load more;
loading, errors and retained rows never become empty success. The shared `EmptyState` remains
unchanged: its fixed book glyph is not a contextual Library contract. Page-wide refresh keeps
focus by retaining a native focusable trigger with `aria-disabled`/`aria-busy` and an associated
pending status. Its current-state intent guard rejects repeats; no completion effect restores or
steals focus moved by the listener. Section retries/paging keep the existing status focus target.

For a signed-in listener ([#272](https://github.com/loveoverflowcom/cantos/issues/272)), live
Work detail, **Của tôi** and `#player` synchronize Saved Works, exact-release progress, history
starts/clear and paused exact-release resume against `/api/v1/theatre`:

- **Saved Works (`GET/PUT /library/saved/{work}`).** Work detail shows a Save/Unsave control and
  each Saved row in **Của tôi** offers Unsave. Guests pressing Save on a Work are routed to Login
  with a contextual Back return to that Work; guest state is never auto-merged on sign-in.
  Concurrent Save/Unsave intentions on the same Work coalesce into a single desired boolean
  queued behind the in-flight write, advancing `expected_revision` and `device_sequence` from
  accepted or conflict receipts. Unsaving removes the Work from **Đã lưu** while preserving
  progress and history.
- **Exact-release progress (`GET/PUT /progress/{publication}`).** Only one mutation is in flight
  per publication. Periodic `Checkpoint` writes throttle to ≥10 s (without a movement threshold); pause,
  route leave, `visibilitychange -> hidden`, confirmed `ExplicitSeek`, `MarkCompleted` and
  `Restart` flush immediately. Tab close (`visibilitychange -> hidden`) triggers a best-effort
  async flush and persists the pending operation to the account-scoped recovery journal; because
  the browser cannot guarantee an authenticated `fetch` completes during document teardown, any
  unacknowledged checkpoint is recovered on the next sign-in via receipt lookup.
  Server reconciliation never uses `max(position)`: `Resolution::KeepServer` adopts the server
  record, while `Resolution::ListenerChooses` exposes an explicit choice between the local
  position and the server position on `#player` and **Của tôi**.
- **History starts and clear (`POST /library/history/playback-starts`, `POST /library/history/clear`).**
  A history start is emitted only after `HTMLAudioElement` (`EngineStarted`) acknowledges
  playback for the current signed-in session, deduplicated per `(engine_identity, publication_id)`
  so pause/resume or buffering on the same session never re-emits. Clearing history requires an
  explicit confirmation step bound to the observed `HistoryState { epoch, revision }`; it clears
  only **Lịch sử** and its cursor while leaving **Nghe tiếp**, **Đã lưu** and the active player
  untouched.
- **Account-scoped recovery journal (`localStorage`).** Pending mutations and the per-account
  `device_id` / `next_device_sequence` counter are stored under `cantos.theatre.sync.v1.<actor_id>`.
  The journal never stores credentials, the (unreadable) session cookie or signed media URLs. After an
  unknown outcome (`503`/timeout/reload), recovery queries `GET /operations/{operation_id}`
  first, reuses the exact immutable `OperationEnvelope` if `404 NotFound`, and triggers a
  point/section reread after a recovered receipt. Sign-out or account switch clears in-memory
  sync state before any new account is admitted and never replays account A's journal under
  account B.
- **Paused exact-release resume.** Choosing Resume from **Nghe tiếp**, **Lịch sử** or a Work's
  unit row checks `ResumeStatus`: `ReleaseReplaced` and `AccessUnavailable` refuse immediately
  with explicit localized status without substituting another release. `ExactAvailable` loads
  `GET /publications/{publication_id}`, verifies `PinnedRelease::admit` and exact
  `work_id / unit_id / publication_id / rendition_id / duration_ms / sha256` equality, and
  restores `PlayerSession` in `Paused(ColdRestore)` at the saved position without auto-playing.

Native tests consume the shared presentation goldens. No browser fixture drives the cookie
sign-in, renewal, signup completion or account-switch journeys yet: they are covered only by host
tests of the pure state machines, status projections and render compositions (see
[Verification boundary](#identity-v1-cookie-session)). Browser, backend and end-to-end acceptance
are `NOT_RUN`.

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
  localized speed/duration matching CMP `ShellCopy` (`1,5×`, `1 phút 1 giây`), native accessible
  links/buttons/search/range controls. Messages interpolate once, so braces inside story titles
  stay literal; each root mirrors its locale onto `<html lang>`
  ([evidence](../../../docs/evidence/leptos-localization-spike.md)).
- Loading/empty/error catalog scenarios use a deterministic fake port. Loading remains pending
  until Show sample data; Retry requests local samples only. Only the newest issued pending
  request can settle; unissued, older/future and duplicate terminal callbacks are ignored. Refresh
  advances the checked generation. Recovery returns focus to main; clearing search returns it to
  the search field.

Within the explicit mock, no API, authentication, network client, real audio/download, timer,
sync, disk/account persistence, billing or provider code is integrated. Unsupported actions stay disabled
with associated reasons. The fake adapter is not browser/native audio integration, and the
in-memory cold-restore exercise is not reload/session persistence.

## Membership interest preview

Open `?mode=membership-preview` through the existing Web command below, or use the labeled
Membership preview link in the mock Account screen. This separate diagnostic mounts no player,
account adapter or network client. The default form explains unavailable submission. The live
Account surface opens separate Membership information with an honest unavailable state; it
does not mount this fixture form, submit interest, activate entitlement or expose payment.

The preview controls can explicitly start an empty fixture session. Fill name/email, submit,
then manually select confirmed receipt, known failure or outcome-unknown timeout. No timer
creates success. Unknown retry retains the same operation/contact payload and a fresh callback
fence. Plans are a separate screen; their illustrative monthly/annual structure says **Chưa
công bố**, with no price, plan selection or payment. Account-registration completion is another
explicit fixture, independent of Membership submission.

Back uses closed internal destinations and the browser hash history; direct plans entry falls
back to Account. Reload clears the entire preview. Real identity returns, durable receipts and
provider signup remain with their owners. See the [preview contract](../../../docs/product/membership-interest.md)
and [acceptance ledger](../../../docs/evidence/theatre-membership-preview.md); source and test
declarations do not establish browser or native acceptance.

## Implemented ownership and dependency direction

| Module | Concrete responsibility |
| --- | --- |
| `src/app/context.rs` | One root composition: independent catalog/player/library/progress/queue handles; explicit user commands; locale/theme/navigation |
| `src/app/routes.rs` | Typed hash routes; no player construction. `#work/<slug>` is the mock fixture route; `#work/<canonical uuid>` is the live public route, which the mock shell treats as unknown |
| `src/core/public_ids.rs` | Canonical lower-case public Work UUID admission; never a slug, title or position |
| `src/core/ids.rs`, `catalog.rs` | Synthetic typed keys and catalog read models; no publication authority |
| `src/core/playback/` | Framework-free immutable state, events, ordered effects and `reduce(state, event) -> Transition` |
| `src/core/listening/` | Separate saved-work/bookmark, progress, simulated history and queue facts/rules |
| `src/application/player_session.rs` | Commit pure transitions, then interpret effects through `MediaPort`; derived status/view `Memo`s and playback observation callbacks |
| `src/application/catalog.rs`, `ports.rs` | Catalog request/result identity and concrete catalog/media consumer boundaries |
| `src/application/account.rs`, `account_view.rs` | Deterministic session generation with shell-injected device IDs, fail-closed exhaustion, logout acknowledgement, private read projection and fenced sync state lifecycle |
| `src/application/account_writer.rs` | Host-compilable Leptos writer for typed account/sync capabilities; journal snapshot and persist-before-dispatch orchestration, without browser imports |
| `src/application/library.rs`, `library_sync.rs` | Pure presented Library projection, epoch-checked history paging, Save/progress/history write outbox, receipt reconciliation, account-scoped recovery journal and exact-release resume admission |
| `src/application/auth_navigation.rs` | Closed internal auth return destinations (Login/Register) and checked request epochs; routes never establish a session |
| `src/application/signup.rs` | RAM-only mailbox-proof link (`ProofLink`, strict `#operation&proof&secret` parser, scrub predicate), chosen password, explicit confirmation, immutable retained request snapshot, same-request retry with Retry-After hold, a signup-start 429 hold that survives `clear`, and epoch-fenced results; redacted `Debug` |
| `src/application/public_discovery.rs` | Pure Work-list walk: normalized `DiscoveryQuery` (q/kind/language), ticket-fenced requests, opaque cursor bound to its query, bounded follow-up of empty pages, same-cursor retry, `invalid_cursor` → fresh walk (never merged), one-time row identity |
| `src/application/public_work.rs` | Pure live Work detail and unit-walk state machine: route generation and page-request fencing, empty-continuation follow-up budget, same-cursor page retry, `invalid_cursor` restart with `unit_id` de-duplication, terminal empty list |
| `src/live/account.rs`, `library.rs`, `runtime.rs` | Cookie-session account lookup/sign-in/logout and Library transport (`Lease::apply`: `credentials: include` plus the session fence), bounded actual Library reads, point reads, idempotent mutations, receipt lookup recovery and mounted browser lifecycle; no mock fallback |
| `src/live/mounted.rs` | Pure decision for the mounted listeners/timer given the live network gate and the owner's 429 hold |
| `src/live/failure.rs`, `transport.rs` | Pure projection of the shared owner's failures onto account failures (401 signed out, 409 session change, 429 bounded wait, 503 unknown, 404 clear fault) and the per-request-class credential policy with source-level regression guards |
| `src/live/signup.rs`, `src/platform/signup_browser.rs` | Anonymous `POST identity/signup` and `identity/signup/complete` (`credentials: omit`), strict bounded decoders, and the synchronous capture/scrub of the mailed fragment with its popstate/hashchange guard |
| `src/live/discovery.rs` | Anonymous `GET /works?[q&kind&language&]limit&presentation=1[&cursor]` (encoded; cursor only form-encoded), `PresentedWorkPage` decoding and exact `ListenerError` classification; an empty page with a cursor is valid |
| `src/live/public_work.rs` | Anonymous `GET /works/{id}?presentation=1` and `/works/{id}/units?…&presentation=1` reads; `PresentedWork*`/`PresentedUnit*` decoding, route identity, unit kind and exact `ListenerError` classification |
| `src/core/public_track.rs` | Pure pinned identity of one public release (`PublicTrack`: Work/unit/publication/rendition IDs, manifest digest, SHA-256) admitted only from a manifest that names the requested release, plus strict grant admission (`AdmittedGrant`) and the redacted `MediaUrl` capability |
| `src/application/public_listen.rs` | Pure listen intent: ticket-fenced manifest resolution, one display label per intent, `NowPlaying` facts that describe only the track they were captured for |
| `src/application/media_driver.rs` | Pure engine decisions: grant lifecycle (authorize, refresh 30 s before expiry for the same publication, bounded backoff, bounded recovery, withdrawn → stop), and the mapping from audio-element facts to player events. Performs no I/O |
| `src/platform/audio.rs` | The one `HTMLAudioElement` and its listeners, requests and timers; executes the driver's commands and reports back (browser only) |
| `src/live/public_media.rs` | Anonymous manifest `GET` and empty-body authorization `POST`; strict shared-DTO decoding and exact `ListenerError` classification. Media bytes are streamed by the element, never fetched here |
| `src/features/live_player.rs` | Live full player (`#player`) and mini-player over the one root session; reuses the shared timeline control |
| `src/infrastructure/` | Mock `MockCatalog` and deterministic `FakeMedia` (isolated `?mode=mock`, **`preview` feature only**) and an inert native port; no empty future adapters |
| `src/features/` | App-private discovery/search/work/library/player/queue HTML compositions; screen-owned search/filter and confirmation state |
| `src/view.rs`, `src/view/live_shell.rs` | Separate mock/live route compositions; Home remains available when identity fails |
| `src/app/live_navigation.rs`, `src/platform/auth_browser.rs` | Browser history, contextual focus/scroll restoration, explicit provider return intent and current-session confirmation |
| `src/messages.rs`, `src/features/auth_copy.rs` | Typed Vietnamese/English locale resources, single-pass interpolation and CLDR plural category |
| `src/platform.rs` | Fallible browser navigation/title/document-language/focus helpers |
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
[`theatre-mock-playback@1`](../conformance/playback/v1/vectors/README.md).
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

Paper/purple semantic roles, tonal grouping, responsive rail and existing focus/reduced-motion/
forced-color styling are retained. The Auth–Account v2 composition reuses shared `Button` auth
variants, `Surface` profile/auth/Membership/account-group variants, `BackButton` and the official
`Logo`. The [preview catalog](../../../packages/ui-system/shared/preview-catalog.json) records
their renderer qualifications; the repository gate checks freshness before new imports are
admitted. Existing compatibility uses retain their path-specific boundary. App layouts consume
canonical token roles and introduce no shared primitive, token copy or admission exemption.
The [v2 handoff](https://drive.google.com/file/d/1ETmdL9w1ywC10OqjPIrhV9ERy2l59tvJ/view)
and actual Home/Login/guest/signed-account reference PNGs were read and opened for #129.
Reference pixels are prototype HTML, not changed-app interaction evidence. The live auth brand
panel uses the official symbol; the mock paintings retain their existing unverified-rights boundary.
PR 02 adds inspected app captures and keyboard/focus observations at named viewports/themes.
The library MVP has its own [observations and limitations](../../../docs/evidence/theatre-web-library-mvp.md);
earlier captures do not establish its changed navigation or library behavior.

## Build, run and checks

Use the existing [xtask commands](../../../xtask/README.md), Rust 1.98.1,
`wasm32-unknown-unknown` and Trunk 0.21.14:

```sh
cargo xtask doctor
cargo xtask build cantos-theatre --web
# Native run owns private Server + Identity files; business catalog readiness remains incomplete.
cargo xtask run cantos-theatre --web
cargo xtask verify repository
cargo xtask verify rust
cargo xtask verify contracts
cargo xtask verify web
cargo xtask preflight --changed-only
```

`run cantos-theatre --web` builds/starts the real API on `127.0.0.1:8080`, sets
`CANTOS_WEB_ORIGIN=http://127.0.0.1:8082`, then starts Trunk on `127.0.0.1:8082`. Trunk forwards
`/api/` to 8080. Readiness requires an anonymous JSON catalog envelope directly from 8080 and
through 8082, plus the frontend HTML. An empty published catalog is a legitimate success;
connection failure, non-JSON or an unavailable response cannot become an empty-success page.
The optional `/session` route is not the anonymous Home readiness gate.

**Compatibility proposal — pending review:** this is option A from
[#183](https://github.com/loveoverflowcom/cantos/issues/183), changing the former frontend-only
Theatre `run` behavior to match Studio. Use `app cantos-theatre --web` for the previous
frontend-only workflow or for explicit offline previews. The
[decision/evidence ledger](../../../docs/evidence/theatre-local-run.md) separates the proposed
compatibility choice, implemented behavior and actual verification; the Draft PR claims no
production or playback acceptance.

Current `cargo xtask run cantos-theatre --web` owns Server + Identity with private native
files and explicit migrations; `app` starts only Trunk against an existing API. Business schema
porting is incomplete, so catalog readiness can fail. The old retained-PG `--test-cluster` lane
is a retired refusal. `seed theatre` also refuses while the native `catalog_http` fixture is
absent; no fake publication or preserved/user database substitutes for that prerequisite.
See the [current seed boundary](../../../docs/deployment/theatre-seeded-catalog.md).

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

| URL at `http://127.0.0.1:8082/` | Backend use | What it demonstrates |
| --- | --- | --- |
| `/` (default live mode) | Real anonymous JSON catalog; independent optional Theatre session/account APIs | Public discovery and explicit live errors; no fixture fallback |
| `/?mode=mock` | None | **`preview` build only.** Session-only sample catalog/library and simulated player; no audio, persistence or publication evidence |
| `/?mode=membership-preview` | None | **`preview` build only.** Explicit Membership interest/receipt/plan fixtures; no durable submission, entitlement or payment |
| `/?mode=auth-preview` | None | **`preview` build only.** Explicit synthetic Home/Auth/Account states; no provider login or registration proof |

### Production and preview build lanes

The crate has one opt-in Cargo feature, `preview`, **off by default**. It compiles the three
preview entrypoints above, the sample catalog (`MockCatalog`), the deterministic `FakeMedia`
player port, the mock shell and the diagnostic fixture controls and copy. A production lane
(`trunk build`, `--release`, `cargo xtask deploy`, `release-build`, `web-performance`) never
enables it, so there `?mode=mock`, `?mode=auth-preview` and `?mode=membership-preview` are
ordinary live URLs: `Mode::from_query` has no preview variant to return and the bundle contains
neither the data nor the entrypoints. Unit tests and lint cover the preview code, so `cargo
xtask verify` enables the feature for them and lints the production lane separately
(`cargo clippy -p cantos-theatre --target wasm32-unknown-unknown`). A plain `cargo test -p
cantos-theatre` stops with a message asking for `--features preview`.

To open the offline previews in a development build, opt in explicitly through Trunk's own
setting, which every `cargo xtask build|run|app cantos-theatre --web` invocation inherits:

```sh
TRUNK_BUILD_FEATURES=preview cargo xtask app cantos-theatre --web
```

The shipped-bundle commands cannot take a feature from the ambient environment: `deploy`
removes `TRUNK_BUILD_FEATURES`, `TRUNK_BUILD_ALL_FEATURES` and `TRUNK_BUILD_NO_DEFAULT_FEATURES`
from the Trunk child (its plan shows `env -u …`), while `web-performance` and `release-build`
refuse any inherited `TRUNK_*` override, so an ambient setting cannot ship a preview. Unknown mode values remain live; a network refusal never selects a preview. Full-stack startup verifies only the
bounded discovery path, while browser interaction, live Identity, playback delivery and native
acceptance retain the owners in [#179](https://github.com/loveoverflowcom/cantos/issues/179),
[#180](https://github.com/loveoverflowcom/cantos/issues/180),
[#13](https://github.com/loveoverflowcom/cantos/issues/13) and
[#170](https://github.com/loveoverflowcom/cantos/issues/170).

Open `?mode=auth-preview` for labeled, no-network Home/Auth/Account diagnostics using the same
production composition. Its controls explicitly select Guest or synthetic A/B, hold and deliver
an old session result, fail or acknowledge logout cleanup, and cancel an auth intent. Fixture
completion exercises the existing account owner; it never proves provider login or registration.
No timer or unavailable live response selects these fixtures. Unknown mode values stay live.
The diagnostic also shows the candidate Material action-icon mapping for
[#224](https://github.com/loveoverflowcom/cantos/issues/224); live icon adoption awaits shared
admission. The [phase/action audit](../../../docs/evidence/theatre-web-auth-actions-224.md)
records the contextual Back, quiet Home escape and proof-specific cancellation behavior.
The [issue 129 record](../../../docs/evidence/theatre-web-auth-account.md) separates browser
observations, pure tests, signed-provider mocks and real disposable PostgreSQL evidence.

The default entrypoint opens **Khám phá** (`#discover`, also `#explore`) and sends anonymous
`GET /api/v1/theatre/works?limit=20&presentation=1` reads with credentials omitted. Loading,
empty success, connection failure and invalid response remain distinct; failure offers Retry and
never substitutes a mock catalog. Published work IDs/titles/descriptions and each Work's public
`unit_count` come from the `PresentedWorkPage`. A session 401 or provider outage does not
redirect Home to authentication.

**Search and filters** use the same endpoint (`q`, `kind` = novel/story/podcast — not a genre —
and the exact released `language` tag such as `vi-VN`), never a filter over the loaded 20 rows.
Text and language apply on Enter or **Tìm** (no debounce timer; Enter during IME composition is
ignored); the type chips apply at once. The applied search lives in the link
(`#discover?q=…&kind=…&language=…`), one history entry per distinct search, so Back/Forward and
returning from a Work restore it; an answered search is kept with its rows and cursor, another
search starts a fresh walk and fences the old response. Invalid input (over 512 bytes, control
characters, a language tag outside `[A-Za-z0-9-]{1,63}`) is shown inline and sends nothing.

**Pagination.** The cursor is opaque and only form-encoded; it is bound to its search, so
changing the search drops it. **Tải thêm tác phẩm** keeps the rows when a later page fails and
retries the same cursor; a repeated identity is kept once; an empty page that still has a cursor
is followed automatically within a small budget and, if the budget runs out, shows "keep
searching" rather than "no results"; `invalid_cursor` keeps the rows shown but offers a fresh walk
instead of mixing snapshots. The list ends only at `next_cursor: null`.

**Routes.** `#explore` renders the same screen. An unknown route (including a mock fixture slug
such as `#work/son-ha-ky`) shows a recovery view with a way back, and `#queue`, which has no live
capability, says so instead of borrowing Home.

Each Home card title links to `#work/<work_id>`, keyed by the server's canonical UUID. The live
detail reads `GET /api/v1/theatre/works/{id}` and then `/works/{id}/units?limit=20` with
credentials omitted, the shared deadline and bounded bodies. It distinguishes loading, the merged
`not_found` answer (no reason is guessed), retryable unavailable and protocol failures, a terminal
empty list and a partial list with Load more. An empty page with a cursor is followed
automatically within a small budget; a failed page keeps its rows and retries the same cursor;
`invalid_cursor` restarts from page one without duplicating rows. Both reads send
`presentation=1` and decode the `Presented*` DTOs: the Work shows the server's public
`unit_count` ("Chương đã công bố: N") and each row its `ordinal` as "Vị trí N" — the current rank
in the public list, never an identity or an author-assigned chapter number. No cover, genre or
loaded-page-length total is shown. Listening is described under [Live guest playback](#live-guest-playback);
opening a Work plays nothing. Back returns to the opening card's focus and the search it came from.

### Identity v1 cookie session

The account owner trusts only the API's answer. Startup, reload, window focus and a once-a-minute
visible timer call `GET /api/v1/theatre/identity/session` with `credentials: include`: a `200`
admits the session, only a `401` means guest, and an empty page state never decides signed-out.
The HttpOnly cookie is never read, stored or forwarded by script; the browser attaches `Origin`
itself and this app never forges it. Every protected request goes through `Lease::apply`
(ambient cookie, redirects refused, `x-cantos-theatre-session` fence); public catalog, publication
and media requests and the two signup routes stay anonymous (`credentials: omit`), and no cookie is
ever attached to a media URL or any URL outside the API's Theatre surface.

| Request | Transport |
| --- | --- |
| `GET identity/session`, `POST identity/session/renew`, `POST identity/login`, `DELETE identity/session` | shared owner (`cantos-web-auth`), `credentials: include`, fence on renew/logout |
| Library `GET`, Saved/progress `GET/PUT`, history `POST`, `GET operations/{id}` | `Lease::apply` (cookie + fence), JSON, bounded bodies, session echo checked before any private projection |
| `POST identity/signup`, `POST identity/signup/complete` | `credentials: omit`, JSON, bounded response, no fence |
| `GET works…`, `publications…`, media authorization | anonymous (`credentials: omit`) |

**Sign-in.** The Login page is an email and password form (`username` / `current-password`
autocomplete). Enter submits except during an IME composition. The password signal is cleared the
moment the typed intent is built; the value moves into the single request and is never stored,
logged, placed in an error notice or persisted. A `401` returns to the form with one generic message
that names no factor; `400` names only the format; `403`/`404` show clear configuration faults; a
`429` shows a bounded wait (the shared owner also refuses to send before `Retry-After` passes); a
`503` or lost response is an unknown effect, never "signed out". The reply is confirmed by an
authoritative read, so a browser that did not retain the cookie reports a fault.

**Rate limits and refusals.** The shared owner reads `Retry-After` for **every** protected answer
(`Lease::receive`: point reads, writes, receipt lookups and Library pages) and holds the whole
Theatre surface of this document until it passes: a request is not dispatched inside the hold, even
with a still-valid session, and the 60 s timer, window focus and manual refresh wait instead of
blanking a valid account with a failed session read. The hold survives a view refresh, a manual retry
and a durable-journal replay (they all re-enter through the owner). A write held by a `429` keeps its
operation id and recovers receipt-first; it is never resent early, never dropped as a failure and
never given a new id. Only a real `401` is a lost session: a `403` is a refusal (wrong Origin,
no permission) that runs no expiry cleanup, a `409` or a differing session echo drops the local
session so it is re-read (a business `409` keeps its own receipt/error meaning), and a `404` is a
configuration fault. The Server's CORS layer
(`services/cantos-server/src/adapters/inbound/http/cors.rs`) exposes `Retry-After` and the session
echo to the configured Theatre origin, so the hold is the wait the Server asked for; a hop that
drops the header falls back to a conservative 30 s. No browser has observed this cross-origin.

**Renewal and expiry.** A due idle deadline (≤90 s left) triggers one shared `POST …/session/renew`
from the same lookup that runs every minute; the absolute deadline cannot be extended. Local clocks
only decide *when to ask*: a passed deadline re-reads the session instead of concluding anything.
**Another tab.** A `409` (or a stamped session id that differs from the one verified) means another
tab changed the account: the session is re-read and every account/session fence applies before any
data from the old account can appear. **Logout** closes the local intent first; the API `DELETE`
retries until acknowledged and a delayed response never revives or re-labels a session.

**Signup.** A `429` on `POST identity/signup {email}` also holds *new* starts until the wait passes
(the button stays focusable with the reason; pressing Enter, editing the email or leaving and
reopening the page cannot bypass it, and a late `429` still arms it). `POST identity/signup {email}` always shows the same accepted notice. The mail opens
`<web>#operation=<32 hex>&proof=<32 hex>&secret=<64 hex>`; the page captures it once before any
other effect, scrubs it from the address bar and history (an unreadable link is scrubbed too) and
holds it only in RAM. Opening, prefetching or reloading the link sends nothing. The owner chooses a
password (8–128 characters, hint only; the server decides), repeats it, ticks the explicit
confirmation and presses **Tạo tài khoản**: only then `POST identity/signup/complete` with
`explicit_confirmation: true`. `200` (`completed` or `replayed`) moves to Login with a RAM-only
notice and **no session**. A `503`, `429` or lost response keeps the exact request snapshot and
offers only "send the same request again" (no new operation, password locked, Retry-After
respected); `401` (spent/expired link) and `403`/`404` end the attempt, `400` allows a new
explicit password with the intact proof. Leaving the page drops everything, which cannot undo an
account the API may already have created.

**Mounted listeners.** The window-focus, visibility, 60 s timer and credential-loss handlers are
mounted once but consult the shell's *live* network gate on every trigger (`live::mounted::decide`):
if a same-tab email link later fails its history scrub, none of them issues a session read, cleanup
or checkpoint write while the proof is still in the URL. (They are mounted only when the network was
allowed at mount time; a shell that starts blocked does not mount them until reload, which is the
existing behaviour.)

**Verification boundary.** Host tests cover the proof parser and scrub predicate, explicit
confirmation, same-request retry and fencing, the status projection, the per-class credential
policy (including source guards against a Bearer header or a cookie on the anonymous adapters) and
render compositions in both locales. The [native Mac browser record](../../../docs/evidence/web-identity-browser-mac.md)
adds named Chromium journeys through the real local Server/Identity cookie path, including reload,
peer login/logout and a delayed A response after B login. It retains separate HTTPS, Studio roster,
other-engine, IME and mail-link UI gaps. The removed OIDC popup, PKCE callback, `ca1_`/`cr1_` handling,
registration capability read and Firebase/idm setup flow have no fallback.

The account view uses Vietnamese/English resources and distinguishes pending, guest, authenticated,
unavailable/protocol failure and unacknowledged logout. An expired session hides private data
immediately and requires acknowledged session revocation before another login. Cleanup is stale-only;
explicit logout pins the observed session. A rotated token or a different account is never invalidated by a delayed old denial or cleanup.

The authenticated session reads the first 20 Continue, Saved and History items through their actual
Library endpoints, using JSON, the cookie session and the session fence. The UI shows accepted progress positions
and bounded page information; the Account profile uses a truthful signed-in fallback without
exposing an actor UUID as a name, inventing totals or asserting Membership status. No Library
mutation, pagination UI, real playback, history-start event, automatic
guest import, durable outbox or full synchronization is introduced. Network/body bounds and
the session-correlated response header are checked before any private projection is displayed.
Account/session generation fences both success and failure; switching accounts never relabels an
old intent. Session checks repeat on window focus and once per minute while an authenticated page
is visible; request batches have a 15 second deadline and refresh refuses duplicate pending reads.
The timer/listener are removed on unmount. A peer cookie change supersedes a pending session lookup,
clears private state and confirms the current account with the Server after pending cleanup settles;
it cannot cancel our own outstanding logout. Browser evidence is scoped by the record above;
device and production acceptance remain separate.

Open `?mode=mock` explicitly for the existing isolated UI preview. An unknown mode never falls
back to mock success. No Studio session, producer token, private submission or draft is available
to either listener mode. The live account projection is not live Theatre playback integration.

## Explicit API origin for static Hosting

Build with nonsecret `CANTOS_API_ORIGIN=https://api.cantoss.duckdns.org` through the existing
`cargo xtask build|app|deploy` command. Unset retains the local Trunk proxy. All live HTTP,
XHR/private reads and login navigation resolve within this app's own API group; invalid origins
fail closed. The client never takes an identity-provider URL or token as configuration.
Protected requests send the API-issued HttpOnly cookie (`credentials: include`) and the session
fence; anonymous discovery and signup send no cookie. The server needs the exact Web origin with
credentialed CORS and the cookie's host-only `Path=/api/v1/theatre`
([#295](https://github.com/loveoverflowcom/cantos/issues/295); the cookie is `SameSite=Strict`, so
the Web and API origins must be same-site). Cross-site browser acceptance remains separate from
host tests. The
[current production runbook](../../../docs/deployment/vps-production.md) owns destinations;
a successful local build is not deployed-user acceptance.

## Default API and error recovery (#298)

Theatre Web follows [client error presentation v1](../conformance/client-error-presentation-v1.md):
an omitted `CANTOS_API_ORIGIN` selects `https://api.cantos.cc`; explicit invalid/empty input fails
before network. Local `xtask run|app` injects a loopback API origin explicitly. Build commands keep
the production default unless an explicit override is supplied. This does not change Studio.

The shell owns one ErrorCenter and composes the shared Dialog/Button for actionable failures.
Catalog/detail reads preserve typed network/timeout/401/403/protocol failures and current tickets;
retained rows remain inline. No request failure becomes a mock or an empty success. Identity
failures remain scoped to the explicit account flow, keeping public Home usable. Account and
route changes retire old notices; dismiss suppresses repeated reports until resolution/new intent.

Durable journal access and parsing failures are visible, do not overwrite unread bytes, and block
mutation sends until persistence/reconciliation succeeds. The storage action reuses the existing
receipt/session owners and never retries a mutation generically. Source/host tests and local API
checks do not establish browser or native acceptance; use the PR's deferred acceptance checklist.


### Account transition boundary

Session completion receives its device-ID generator from the shell. Refused duplicate/stale
requests preserve the whole state. Security invalidation at `u64::MAX` intentionally clears
private scope, Library and sync memory, cancels cleanup and enters `Failed(Unavailable)` without
wrapping or issuing another generation; reload is required before fresh admission. This boundary
is effectively unreachable in an ordinary page lifetime and is not a production incident claim.
The durable actor-scoped journal is untouched and restores the same operation ID/payload with
receipt-first recovery after reload. The shell commits this fail-closed transition even when the
owner returns `Exhausted`.

`generate_uuid_string` remains a shell utility: the browser first tries `crypto.randomUUID`,
then falls through to the existing process-local counter if no valid UUID was returned. This
change does not evaluate browser exception/fallback reachability or change that utility.


Sync consumers use `AccountViewState` capabilities for Saved, progress, History, read/receipt
completions, acknowledged playback and ticket-fenced Resume. Admission returns
`Result<Admitted<T>, IntentRefusal>`: current intents distinguish `SignedOut`, `RestoreBlocked`
and `NoSync`; fenced completions distinguish `StaleFence` and `NoSync`. Completion recovery
remains allowed while journal restoration is blocked, matching the existing protocol. Admission
refusals preserve the whole aggregate and do not call ID generators. Mutable sync/projection
closures are private to the owner; fixture seeding exists only under `cfg(test)`.

The host-compilable `AccountWriter` publishes owner results, releases its guards, snapshots only
the journal and persists before constructing the network effect. Persistence failure retains the
pending operation as an unknown outcome. Receipt-first recovery keeps its original ID/payload;
the browser shell rechecks its fence after awaited I/O. Resume completions recheck both fence and
ticket at the owner. The writer returns `Write::Disposed` or `Write::Done { value, published }`;
publication is independent of admission and returned effects. Cold writes compare owned reader
facts; playback compares only its bounded source/presentation entries and History command, without
cloning the aggregate or journal. A changed cleanup flag publishes even when logout returns false.
Transient progress coalescing and other explicitly classified non-presentation bookkeeping may
change without publication; returned effects still run after guard release. ErrorWindow writes
likewise publish only when the active notice changes, preserving queued and dismissed episodes.
See [the publication boundary and finite oracle](../../../docs/evidence/theatre-account-publication-362.md).
`AccountState`'s byte-projection/outbox API has test-only callers; production outbox evidence
comes from `LibrarySyncState` and `AccountWriter`, not that legacy test API.

See [the source-bound capability record](../../../docs/evidence/theatre-account-capabilities-361.md)
for the local evidence boundary and remaining runtime acceptance.
