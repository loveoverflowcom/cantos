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
Bearer/gateway/issuer acceptance; static delivery establishes no playback.
The preferred shorter site `cantos.web.app` is conditional on verified allocation; the technical
module stays `cantos-theatre`. Its [site transition](../../../docs/deployment/firebase-hosting.md#shorter-site-names-controlled-transition-237)
follows ADR 0041 direct Hosting/API addressing and exact-origin review. Cross-site authentication
remains unavailable until the independent Bearer/gateway/issuer acceptance.

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
- **Not covered here:** save/progress/history (#272), search and routing (#270), offline,
  queue, transcript, Safari/iOS and deployed CDN/CORS behavior.

## Scope and honest behavior

The default entrypoint opens public Home without a login wall. It reads actual published work
summaries anonymously, independently of session availability. Account opens through the avatar;
Login and Create account are explicit secondary routes. The two primary destinations remain
**Khám phá** and **Của tôi**, following [#35](https://github.com/loveoverflowcom/cantos/issues/35).
The Auth–Account v2 reference's third primary Account destination is intentionally not adopted,
as required by [#129](https://github.com/loveoverflowcom/cantos/issues/129).

The live Account/Login surface reads both capabilities from `/auth/configuration`. Login requires
an explicitly configured OIDC capability and an acknowledged session-cleanup barrier before
reserving the trusted-click popup. Registration availability is independent; configuration
transport/protocol failures expose a retry rather than claiming the service is unsupported.
Bearer credentials remain document-RAM only under [the shared owner](../../../packages/web-auth/README.md).

The live **Của tôi** reads Continue, Saved and History with `presentation=1`, limit20 and independent
opaque cursors. It renders released Work/unit titles and canonical UUID Work links directly from
`Presented*Page` DTOs, retaining exact audio identities, progress revisions, history state and
resume status. Each section owns loading/empty/page-error/retry/reload/load-more. Failed next
pages retain loaded rows; expired cursors require a fresh walk of that section. Null metadata
and replaced/unavailable releases retain their records without borrowing another release's title.
Logout, expiry and session change hide all private lists before new admission; late results are
fenced by account/session and page generation. Reading/rendering/navigating never writes progress,
Saved or history and never starts media. Connecting the retained exact position to a paused
production player depends on [#269](https://github.com/loveoverflowcom/cantos/issues/269); progress
writes/reconciliation remain [#272](https://github.com/loveoverflowcom/cantos/issues/272).

Native tests consume the shared presentation goldens. The existing explicit
`cargo xtask verify web-bearer` lane also exercises these production Web modules with a loopback
mock API/issuer and synthetic paged Library records; its browser captures are local evidence,
not real Kanidm/PostgreSQL/signup or production acceptance. The separate
[enrollment lane and remaining Web fixture](../../../docs/deployment/firebase-enrollment-proof.md#isolated-local-enrollment-testing)
retain their existing platform/tool requirements.

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
| `src/application/player_session.rs` | Commit pure transitions, then interpret effects through `MediaPort`; derived status/view `Memo`s |
| `src/application/catalog.rs`, `ports.rs` | Catalog request/result identity and concrete catalog/media consumer boundaries |
| `src/application/account.rs`, `account_view.rs` | Pure session generation, logout acknowledgement, immutable intent ownership and private read projection lifecycle |
| `src/application/auth_navigation.rs` | Closed internal auth return destinations and checked request epochs; routes never establish a session or registration receipt |
| `src/application/public_discovery.rs` | Independent anonymous read generation, duplicate admission and terminal response fencing |
| `src/application/public_work.rs` | Pure live Work detail and unit-walk state machine: route generation and page-request fencing, empty-continuation follow-up budget, same-cursor page retry, `invalid_cursor` restart with `unit_id` de-duplication, terminal empty list |
| `src/live/account.rs`, `library.rs`, `runtime.rs` | Same-origin Theatre BFF transport, bounded actual Library reads and mounted browser lifecycle; no mock fallback |
| `src/live/discovery.rs` | Anonymous fixed-JSON published-work read, bounded response decoding and explicit unavailable/protocol failures |
| `src/live/public_work.rs` | Anonymous `GET /works/{id}` and `/works/{id}/units` reads; strict shared-DTO decoding, route identity, unit kind and exact `ListenerError` classification |
| `src/core/public_track.rs` | Pure pinned identity of one public release (`PublicTrack`: Work/unit/publication/rendition IDs, manifest digest, SHA-256) admitted only from a manifest that names the requested release, plus strict grant admission (`AdmittedGrant`) and the redacted `MediaUrl` capability |
| `src/application/public_listen.rs` | Pure listen intent: ticket-fenced manifest resolution, one display label per intent, `NowPlaying` facts that describe only the track they were captured for |
| `src/application/media_driver.rs` | Pure engine decisions: grant lifecycle (authorize, refresh 30 s before expiry for the same publication, bounded backoff, bounded recovery, withdrawn → stop), and the mapping from audio-element facts to player events. Performs no I/O |
| `src/platform/audio.rs` | The one `HTMLAudioElement` and its listeners, requests and timers; executes the driver's commands and reports back (browser only) |
| `src/live/public_media.rs` | Anonymous manifest `GET` and empty-body authorization `POST`; strict shared-DTO decoding and exact `ListenerError` classification. Media bytes are streamed by the element, never fetched here |
| `src/features/live_player.rs` | Live full player (`#player`) and mini-player over the one root session; reuses the shared timeline control |
| `src/infrastructure/` | Mock `MockCatalog` and deterministic `FakeMedia` (isolated `?mode=mock` only) and an inert native port; no empty future adapters |
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
# DATABASE_URL must already name an operator-prepared, migrated local app-role database.
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

For a disposable cluster already retained by a successful explicit
`cargo xtask verify postgres --keep`, the full-stack alternative is:

```sh
cargo xtask run cantos-theatre --web --test-cluster
```

It selects the retained Theatre fixture (`CANTOS_TEST_CONTEXT=theatre`, `theatre_app_url`),
which has bounded discovery read grants and no invented publication. It refuses another DB
or a record without that field; explicitly rerun disposable verification if the record is too
old. `run` starts no PostgreSQL service and applies no role grants or migrations.

To see published Works, run `cargo xtask seed theatre` once against that retained cluster before
`run`. It publishes synthetic novel, story and podcast Works through the real Studio flows; `run
--test-cluster` then serves that seed database and says so on its first line. The
[seeded catalog runbook](../../../docs/deployment/theatre-seeded-catalog.md) lists the dataset,
stable IDs, a live withdrawal, reset and troubleshooting. Hearing a unit in the browser
additionally needs its [local TLS front end](../../../docs/deployment/theatre-seeded-catalog.md#local-tls-front-end-for-browser-playback),
because the API admits only an HTTPS media origin.

To reuse an already-running API, use separate terminals. The API terminal requires the same
operator-prepared `DATABASE_URL` (or its retained test-cluster option):

```sh
cargo xtask run cantos-api --web-origin http://127.0.0.1:8082
cargo xtask app cantos-theatre --web
```

`app` starts only Trunk, probes the existing public catalog and prints a prominent `WARNING`
when the dependency is absent or invalid. Its final readiness message always means
**frontend-only**, even if that advisory probe succeeded. Open the URL with the desired
explicit preview query to work without an API; live mode never switches itself to fixtures.

The API needs an operator-supplied migrated local app-role `DATABASE_URL` (or its documented
retained disposable test-cluster option). Theatre OIDC remains opt-in through the existing
`CANTOS_THEATRE_OIDC_ISSUER` and separate `CANTOS_THEATRE_OIDC_SECRET`; approved client/callback,
scope, credentials and TLS/provider acceptance are operator prerequisites, never provisioned by
these commands. The current development host has one `CANTOS_WEB_ORIGIN`, so this recipe is a
Theatre-origin profile. An explicit `--web-origin` on the full-stack Theatre command must be
exactly `http://127.0.0.1:8082`; Studio similarly requires its actual `http://127.0.0.1:8081`.
`localhost` is a different host identity and is rejected for those profiles. Standalone API
origin selection remains configurable within its loopback contract. Simultaneous Studio
on 8081 and Theatre on 8082 authentication is unverified. Identity failure does not hide a
working anonymous catalog, create an account or substitute mock data.

Smoke the public route at both origins without cookies or bearer credentials:

```sh
curl -i -H 'Accept: application/json' 'http://127.0.0.1:8080/api/v1/theatre/works?limit=20'
curl -i -H 'Accept: application/json' 'http://127.0.0.1:8082/api/v1/theatre/works?limit=20'
```

Expect HTTP 200 and the [WorkPage contract](../../../packages/contracts/theatre-discovery-v1.md),
including a possible empty `items` list. A typed error keeps its status and `code`/`retryable`
meaning; 404 is not evidence of a disconnected frontend and unavailable is not an empty catalog.
To see actual works, an operator-approved published Work must already exist; startup does not
manufacture one or make delivery available.

If startup fails, inspect the named process and readiness route. Check that the operator's
database is reachable, migrated and readable by the app role; repair issuer configuration only
when identity is explicitly enabled. A port collision is actionable: stop only a process you
own, or use `app` to reuse an appropriate API. The API's 60-second and Trunk's 15-minute readiness
budgets are bounded. Ctrl-C stops only groups started by this invocation. The commands never
start Podman, Kanidm or S3, create credentials, or print `DATABASE_URL`/password values. Keep
secret-bearing environment/configuration and retained local evidence outside Git; share only
redacted diagnostics.

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

| URL at `http://127.0.0.1:8082/` | Backend use | What it demonstrates |
| --- | --- | --- |
| `/` (default live mode) | Real anonymous JSON catalog; independent optional Theatre session/account APIs | Public discovery and explicit live errors; no fixture fallback or live player |
| `/?mode=mock` | None | Session-only sample catalog/library and simulated player; no audio, persistence or publication evidence |
| `/?mode=membership-preview` | None | Explicit Membership interest/receipt/plan fixtures; no durable submission, entitlement or payment |
| `/?mode=auth-preview` | None | Explicit synthetic Home/Auth/Account states; no provider login or registration proof |

Use `cargo xtask app cantos-theatre --web` for these offline previews. Unknown mode values
remain live; a network refusal never selects a preview. Full-stack startup verifies only the
bounded discovery path, while browser interaction, real issuer, playback delivery and native
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

The default entrypoint opens Home and sends one anonymous fixed-JSON
`GET /api/v1/theatre/works?limit=20` with credentials omitted. Loading, empty success, connection
failure and invalid response remain distinct; failure offers Retry and never substitutes a mock
catalog. Published work IDs/titles/descriptions come from the actual DTO. A session 401 or provider
outage does not redirect Home to authentication.

Each Home card title links to `#work/<work_id>`, keyed by the server's canonical UUID. The live
detail reads `GET /api/v1/theatre/works/{id}` and then `/works/{id}/units?limit=20` with
credentials omitted, the shared deadline and bounded bodies. It distinguishes loading, the merged
`not_found` answer (no reason is guessed), retryable unavailable and protocol failures, a terminal
empty list and a partial list with Load more. An empty page with a cursor is followed
automatically within a small budget; a failed page keeps its rows and retries the same cursor;
`invalid_cursor` restarts from page one without duplicating rows. No unit count, cover, genre or
position number is shown. Playback is not integrated: the Listen control is disabled with a visible
reason, and nothing autoplays or is marked completed. Back returns to the opening card's focus.

The account owner admits only the RAM credential through
`GET /api/v1/theatre/auth/bearer/session`. Explicit Login reserves a popup before network work,
begins the JSON Bearer flow, carries TTL/consume-once PKCE protocol state through issuer page
navigation and exchanges its handoff only in the original opener. The callback query is scrubbed
before product effects; exact message origin, WindowProxy and flow/intent are checked. Provider
tokens never reach JavaScript. Access/refresh tokens never enter persistent storage or URLs.

Refresh is single-flight, rotating and bounded by original server authority. Unknown outcomes
revoke the original family or cancel the exact flow; no spent token/code is replayed. Explicit
logout clears local admission before the server call. Cleanup and late results cannot bind A to
B. Reload/new tab requires controlled authorization/SSO or login; guests never auto-loop. The
mounted shell keeps canonical return/focus context, including entry from Library; a callback or
cached route is never session evidence. See the [shared Web owner](../../../packages/web-auth/README.md).

[ADR 0034](../../../docs/decisions/0034-theatre-signup-and-provisioned-studio.md) accepts
listener self-registration and provisioned-only Studio administrators. Web independently reads
`GET /api/v1/theatre/auth/configuration` on mount. Registration distinguishes loading,
`identity_unconfigured`, `provider_unsupported`, transport failure and invalid response; failed
checks expose a bounded, duplicate-fenced retry. The strict contract currently has no supported
registration variant: pinned Kanidm 1.10.5 has no verified public enrollment operation. This is
an issuer capability blocker, not an undecided product policy. Cantos collects no password and
shows no account success from a route or capability read. Auth preview explicitly fixtures
`provider_unsupported` without network access. Membership form/receipt/plans remain with #128.
The account view uses Vietnamese/English resources and distinguishes pending, guest, authenticated,
unavailable/protocol failure and unacknowledged logout. An expired session hides private data
immediately and requires acknowledged session revocation before another login. Cleanup is stale-only;
explicit logout pins the observed session. A rotated token or a different account is never invalidated by a delayed old denial or cleanup.

The authenticated session reads the first 20 Continue, Saved and History items through their actual
Library endpoints, using JSON and scoped Bearer with cookies omitted. The UI shows accepted progress positions
and bounded page information; the Account profile uses a truthful signed-in fallback without
exposing an actor UUID as a name, inventing totals or asserting Membership status. No Library
mutation, pagination UI, real playback, history-start event, automatic
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

## Explicit API origin for static Hosting

Build with nonsecret `CANTOS_API_ORIGIN=https://api.cantoss.duckdns.org` through the existing
`cargo xtask build|app|deploy` command. Unset retains the local Trunk proxy. All live HTTP,
XHR/private reads and login navigation resolve within this app's own API group; invalid origins
fail closed. The client never takes an identity-provider URL or provider token as configuration.
All requests omit cookies. Protected effects use scoped Bearer and existing session fences;
anonymous discovery and enrollment keep their separate contracts. The server needs matching exact
Web-origin/CORS and Bearer callbacks. [ADR 0041](../../../docs/decisions/0041-direct-hosting-api-boundary.md)
removes the cookie/same-site prerequisite. Gateway, callback reconciliation and real issuer/
cross-site browser acceptance remain separate from mocked client journeys. The
[current production runbook](../../../docs/deployment/vps-production.md) owns destinations;
a successful local build is not deployed-user acceptance.
