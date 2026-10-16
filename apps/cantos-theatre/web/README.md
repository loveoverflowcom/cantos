# Cantos Theatre Web

The existing `cantos-theatre` target remains **Rust 2021 / Leptos 0.7.8 CSR/WASM**. It implements
a public discovery Home, a same-origin listener account/auth boundary and an explicitly isolated
session-only mock; users see
**Cantos**. The [PR 02 validation record](../../../docs/evidence/theatre-web-pr02.md)
names the local build, reducer/fake-port tests and integrated-browser observations. No production
listening issue or UI-system qualification is completed by this foundation.
The [library MVP record](../../../docs/evidence/theatre-web-library-mvp.md) scopes the
[#35](https://github.com/loveoverflowcom/cantos/issues/35) follow-up to this Web mock; CMP and
acceptance against real listener data remain separate.

The [performance pattern audit](../../../docs/evidence/web-performance-patterns.md) measures
release assets with `cargo xtask web-performance cantos-theatre` and narrows mock-player/library
reactive projections. Deterministic work counters and bundle bytes are separate from browser
latency, actual audio and production static-cache acceptance.

The [player interaction record](../../../docs/evidence/theatre-web-player-interaction.md)
describes the isolated mock's local scrub draft, one final seek, root-owned pending
acknowledgement and narrow transport projection. It includes the A/B/C decision and deferred
browser/audio acceptance; no browser latency improvement is claimed.

## Scope and honest behavior

The default entrypoint opens public Home without a login wall. It reads actual published work
summaries anonymously, independently of session availability. Account opens through the avatar;
Login and Create account are explicit secondary routes. The two primary destinations remain
**Khám phá** and **Của tôi**, following [#35](https://github.com/loveoverflowcom/cantos/issues/35).
The Auth–Account v2 reference's third primary Account destination is intentionally not adopted,
as required by [#129](https://github.com/loveoverflowcom/cantos/issues/129).

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
| `src/app/routes.rs` | Typed hash routes; no player construction |
| `src/core/ids.rs`, `catalog.rs` | Synthetic typed keys and catalog read models; no publication authority |
| `src/core/playback/` | Framework-free immutable state, events, ordered effects and `reduce(state, event) -> Transition` |
| `src/core/listening/` | Separate saved-work/bookmark, progress, simulated history and queue facts/rules |
| `src/application/player_session.rs` | Commit pure transitions, then interpret effects through `MediaPort`; derived status/view `Memo`s |
| `src/application/catalog.rs`, `ports.rs` | Catalog request/result identity and concrete catalog/media consumer boundaries |
| `src/application/account.rs`, `account_view.rs` | Pure session generation, logout acknowledgement, immutable intent ownership and private read projection lifecycle |
| `src/application/auth_navigation.rs` | Closed internal auth return destinations and checked request epochs; routes never establish a session or registration receipt |
| `src/application/public_discovery.rs` | Independent anonymous read generation, duplicate admission and terminal response fencing |
| `src/live/account.rs`, `library.rs`, `runtime.rs` | Same-origin Theatre BFF transport, bounded actual Library reads and mounted browser lifecycle; no mock fallback |
| `src/live/discovery.rs` | Anonymous fixed-JSON published-work read, bounded response decoding and explicit unavailable/protocol failures |
| `src/infrastructure/` | Only implemented `MockCatalog` and deterministic `FakeMedia`; no empty future adapters |
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
The [issue 129 record](../../../docs/evidence/theatre-web-auth-account.md) separates browser
observations, pure tests, signed-provider mocks and real disposable PostgreSQL evidence.

The default entrypoint opens Home and sends one anonymous fixed-JSON
`GET /api/v1/theatre/works?limit=20` with credentials omitted. Loading, empty success, connection
failure and invalid response remain distinct; failure offers Retry and never substitutes a mock
catalog. Published work IDs/titles/descriptions come from the actual DTO. This bounded read does
not implement work-detail navigation or real playback. A session 401 or provider outage does not
redirect Home to authentication.

The account owner independently bootstraps with `GET /api/v1/theatre/session`. The explicit Login
route starts `/api/v1/theatre/auth/login` only after the existing cookie-cleanup barrier allows it;
logout uses `DELETE /api/v1/theatre/session`. Provider tokens never reach JavaScript or browser
storage. Provider return context stores a canonical internal route and a nonsecret correlation
intent in session storage; only its matching callback consumes it. The fixed callback return hash
grants no authority: the mounted owner must confirm the current session before completing
navigation. Contextual Back/cancel invalidates old presentation requests and restores the original
entry's focus/scroll within the mounted session. Deep links without accepted context fall back to
Home.

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
explicit logout pins the observed session. A cookie rotated in another tab is rechecked rather
than revoked by a delayed request.

The authenticated session reads the first 20 Continue, Saved and History items through their actual
Library endpoints, using JSON and same-origin cookies. The UI shows accepted progress positions
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
