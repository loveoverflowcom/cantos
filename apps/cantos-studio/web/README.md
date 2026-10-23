# Cantos Studio Web

> [ADR 0018](../../../docs/decisions/0018-external-audio-production.md): Studio is
> **Web-only catalog and management** for novels/stories/podcasts, chapters/episodes and their
> externally produced audio. Existing provider/casting controls are legacy **reference planning**;
> Studio does not dispatch TTS and the Work/ContentUnit/Audio handoff UI is not implemented yet
> ([#66](https://github.com/loveoverflowcom/cantos/issues/66), [#8](https://github.com/loveoverflowcom/cantos/issues/8)).

`cantos-studio` is a Leptos 0.7.8 CSR/WASM consumer of the native Axum/Turso (business port incomplete)
revision API. Its text-first workspace has scene/act/character navigation, native dialogue,
narration, emotion, pronunciation and cue controls, and a source/validation/change inspector.
Import, proposal review and accepted-revision editing keep their state mounted when switching
workspace sections. Advanced complete JSON is optional; unsupported documents stay available
there without projection or silent field deletion. Casting and immutable production-input
review use that same authority; audio production and Cantos Theatre remain planned. Cantos
provides a data/tool pipeline: generation belongs to the user's chosen Gemini, ChatGPT or Codex
host under their control. Cantos has no AI API client or generation runtime. Fixture and live host
evidence are separately recorded in the
[adaptation evidence](../../../docs/evidence/ai-script-adaptation.md).

## Build and run

Development-only Forge mock: in the existing debug Studio runtime, open
`http://127.0.0.1:8081/?preview=studio-forge` after `cargo xtask app cantos-studio --web`.
The normal launcher retains its local-API prerequisite; this explicit composition returns
before live bootstrap and makes no API/provider/storage requests. It is absent from release
builds and is never a production fallback. Recipe v1, immutable session revisions, separate
run/storage/take/sync states and remaining acceptance are documented in the
[Forge mock record](../../../docs/evidence/studio-forge-mock-ui.md). Reload/reset discards mock
changes. No real keys or audio are accepted by this preview.

The owner-selected Forge workspace is a future section of this app, backed by a separately
configured producer client; it is not implemented by the relocated producer/worker foundation.
See [ADR 0022](../../../docs/decisions/0022-cantos-forge-producer-cli.md) and the
[worker acceptance boundary](../../../docs/deployment/forge-worker.md). Forge downtime must
not disable existing content/manual-audio workflows. No browser-held provider/OAuth secret,
automatic draft ingestion or publication is permitted.

Requires Rust 1.98.1, `wasm32-unknown-unknown`, Trunk 0.21.14 and matching wasm-bindgen 0.2.100.
From the repository root:

```sh
rustup target add wasm32-unknown-unknown --toolchain 1.98.1
cargo xtask build cantos-studio --web   # NO_COLOR=true trunk build --locked in this directory
cargo xtask verify web                  # WASM clippy plus Trunk builds of Studio and Theatre
cargo xtask web-performance cantos-studio # release asset hashes and offline compression sizes
cargo xtask deploy firebase cantos-studio --web --project ID --channel NAME --dry-run # plan a Hosting deploy
```

The explicit, manual `deploy firebase` can publish this app's static bundle to Firebase Hosting
([runbook](../../../docs/deployment/firebase-hosting.md)); Hosting serves static files and clients
use the explicit Cantos API origin. [#295](https://github.com/loveoverflowcom/cantos/issues/295)
selects Web cookies on the custom domains; this client now consumes the
[Identity auth v1](../../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) cookie session. Direct-origin
sign-in remains gated by backend/browser acceptance (none ran). Never upload private Studio content to a preview, which is public.
Studio keeps its own Hosting site and browser origin. The preferred `cantos-studio.web.app`
name is conditional on verified allocation; `studio.cantos.web.app` is not supported. The
[site transition](../../../docs/deployment/firebase-hosting.md#shorter-site-names-controlled-transition-237)
requires exact API-origin/CORS/return/callback review under ADR 0041. Short names alone establish
no authenticated session or same-site domain.

Trunk fetches its matching wasm-bindgen helper on its first build. An offline build requires
that helper on `PATH` as well as the Cargo cache; the Mac run used
`~/Library/Caches/dev.trunkrs.trunk/wasm-bindgen-0.2.100`. Generated `dist/` is ignored.
The [performance audit](../../../docs/evidence/web-performance-patterns.md) records measurement
limits and deferred editor/SSR/splitting experiments. The inventory does not measure browser
startup or change the existing private/session cache boundary.
`cargo xtask run cantos-studio --web` starts the native Server + Identity with private files,
explicit migrations, distinct surface origins and Trunk on 8081. `app cantos-studio --web`
starts only Trunk against an API already listening. Retained-PG `--test-cluster`/`verify postgres`
commands are retired; prior browser walkthroughs remain source-bound history. Business porting
and actual browser/login acceptance are separate from build/source checks.

## Login entry and writing desk

Studio opens a login-only entry while the API has not confirmed the browser's session. Sign-in is
**email and password** (Identity auth v1); there is no public sign-up, passkey/MFA, recovery,
change-password, phone or Google sign-in, and no provider or OIDC popup. Checking, unavailable,
explicit permission-denied, sign-in-unavailable (404) and retained-owner states keep every private
workspace hidden/inert. Cancel returns to Studio sign-in and invalidates that observation, so a
delayed response cannot reopen the workspace. There is no Theatre Back, registration,
access-request, invitation or Membership flow.

| API answer at sign-in / session read | Studio shows (vi-VN and en copy) |
| --- | --- |
| 200 session | confirmed by an authoritative `GET identity/session`; only then the writing desk |
| 401 at login | one generic "could not sign in" message; never names the password or the account |
| 403 at login | "the request was refused: this account may not access Cantos Studio"; the Denied entry. A 403 is a wrong `Origin` **or** a roster denial and the response cannot tell which, so no copy claims the password was right. Login and email never grant Studio authority |
| 404 | "Studio sign-in is not available on this server"; no fallback sign-in, form hidden |
| 429 | the bounded `Retry-After` seconds; the identity client refuses every request of this surface (login, renew, reads, writes, downloads, uploads) until then, even with a still-valid session |
| 503, network, timeout | unknown effect, shown as unavailable with Retry; never as signed out; drafts kept |
| 400 | invalid input message |
| refused account (retained-draft owner mismatch) | message that the new session was revoked; the other account is never adopted |

A confirmed matching owner opens **Writing desk / Bàn viết**. Its bounded Work page, current
Work/unit and exact private-audio facts share the existing Content workspace's `Facts` and
request authority. No summary endpoint, portfolio total, QC count or demo record is added.
Import, new Work, current Work, audio and script-editor actions open the existing mounted
workspaces. Returning to the desk preserves authoring text and request snapshots; pane return
retains native caret/selection/scroll without copying manuscript text into navigation state.
In-flight requests block sign-in, refresh and reauthentication, but **never logout**: logout
(`session::admits`) closes local authority synchronously, before its first await. The session
owner fences every lease (a late reply of the account being left is an unknown outcome, never an
acknowledgement), the page-wide fence is withdrawn so nothing new is dispatched and a late denial
cannot touch the next account, each panel aborts its request best effort (an abort is not proof
that a write did not commit), and the private workspace is hidden. Unsaved drafts and unresolved
operations are kept for their owner; ambiguous writes allow explicit reauthentication as the same
owner and same-operation reconciliation, including delegation.

The [scoped evidence](../../../docs/evidence/studio-login-dashboard.md) separates native/SSR,
WASM and synthetic-HTTP browser observations from live identity-provider acceptance. The
reference's raster illustration and aggregate metrics are not imported as product data.

## Interaction and ownership

On mount, idle browser-history restoration and **Check current session**, Studio asks the API
(`GET /api/v1/studio/identity/session`) who the browser is; an empty page proves nothing. The
credential is the API-issued HttpOnly cookie, which Studio never reads, stores or forwards;
only nonsecret facts (actor, session id, idle/absolute deadlines) are kept in RAM by
[`cantos-web-auth`](../../../packages/web-auth/README.md). Every owner request is sent with
`credentials: include` and the immutable session-id fence in `x-cantos-studio-session`, never an
`Authorization` header; the browser adds `Origin` itself. A request renews the idle deadline
once (shared flight) before it is near, and re-reads the session instead of concluding "signed
out" when local deadlines pass; the absolute deadline cannot be extended, so the person signs in
again after it. A 409 `unauthenticated` is a session-fence mismatch (another tab changed
account): the local session is dropped, the denial re-reads the session, and nothing is
accepted for the old one. Business writes keep their existing operation receipt and are never
blindly replayed.

| Request | Transport |
| --- | --- |
| Owner JSON/Protobuf reads and writes, delegations, catalog/content/briefs/audio metadata | `credentials: include` + session fence; per-route codec negotiation unchanged |
| Original download, private artwork preview | same, then SHA-256/identity checks before an ephemeral Blob |
| Artwork upload | XHR with `withCredentials=true` + fence (progress for same- and cross-origin); fetch fallback with the same policy and no invented percentage. The target must be this surface of the configured API origin |
| 429 on any owner request | `Lease::receive` (XHR: `Lease::response(status, Retry-After, echo)`) arms the shared hold; commands surface `ClientError::RateLimited { seconds }` (an unknown outcome for a write, reconciled by its operation and never replayed before the hold ends); the `ApiError`-returning paths keep `Unavailable` (shared contract) and expose the wait through `api::rate_wait()` |
| Producer status/upload (work capability) | its own `Authorization: Bearer`, `credentials: omit`, no cookie, no fence |
| Public/anonymous | `credentials: omit` |

Sign-in is one typed intent built from an explicit form submission: the password leaves the form
with the intent (the field is empty before the request) and is never logged, persisted or kept
for a retry. A retained draft (or the account menu's reauthentication) pins its owner: the
identity client refuses and revokes any other account, so the owner's private drafts are never
projected to it. Cancel/logout invalidate intent before asynchronous cleanup (the previous
session is revoked with a fenced `DELETE identity/session`; an unacknowledged revoke blocks new
admission until retried); late success/401/body results cannot reopen an old account. Pane IDs
survive in history, while confirmed reload/close discards RAM drafts under the existing
unsaved-work warning; no manuscript backup is silently persisted. A session that expires while a
draft or upload is pending keeps the draft in RAM; the sign-in form re-authenticates as the same
owner and nothing is auto-replayed.

The authority is documented in [Identity auth v1](../../../services/cantos-server/contracts/auth/identity-auth/v1/README.md)
and the shared [Web boundary](../../../packages/web-auth/README.md). **Not verified:** no
backend, browser, cookie (`SameSite`/`Secure`/third-party blocking), CORS, multi-tab or E2E run
exists for this migration; only host unit tests and the WASM lint/build. Real Identity roster
and clock-skew behavior are untested.

The pure `authoring.rs` patches known leaves of the complete JSON value, preserving stable IDs,
unknown metadata, provenance, rights and selected asset fields. It rejects duplicate keys,
unsupported versions and numbers it cannot preserve exactly; backend admission still rejects
unsupported Script IR fields. Local structural undo/redo retains 40 immutable snapshots.

The pure `editor.rs` owns immutable save snapshots, actor-bound drafts, request tickets and error
transitions. Its fields are private; one closed phase (idle, reading, saving, reconciliation
required, checking session) owns at most one request, and the shell changes it only through named
transitions such as `checking_session` and `release_session_check`. A read abandoned when a
session settles can never settle a later save. A clean read opens the stored head; a dirty read preserves the local draft. A stale save requires a read/compare and explicit
"keep text, use this head as base" action. A timeout preserves the original actor, operation
key, base and snapshot; retries reconcile that same operation before another save. Session
denial after an ambiguous attempt does not discard it. A different actor cannot retry it.
Editing during a request survives its acknowledgement.

The component obtains the concrete HTTP adapter from Studio context; URLs and wire decoding
stay in `api.rs`. Head, pinned revision, save and validation go through the pure `wire.rs`: one
constant, `REVISION_CODEC`, picks JSON (shipped) or the explicitly configured Protobuf pilot.
Responses are decoded by their own `Content-Type`, and a request is never retried in the other
representation. The [pilot build and rollback](#protobuf-pilot-build-and-rollback) section records
the activation boundary and commands. The browser does not decide backend permissions or Script IR validity.
`services/cantos-server/src/contract/` is shared with Axum; no server crate enters the WASM dependency graph.

Vietnamese/English resources cover every async/error state. The root owns a typed `Locale`
(`vi-VN` first render) and mirrors it onto `<html lang>`; namespaces not yet migrated read a
read-only `english` adapter until the [localization decision](../../../docs/evidence/leptos-localization-spike.md)
is recorded. The switch writes only that locale signal, never draft or session state. Semantic labels, polite status,
visible focus and keyboard-operable controls use native HTML. Guarded aria-disabled actions
remain focusable with a reason. No animation or audio is implemented. Page-return handling
reapplies controlled values if Safari restores WASM state but clears autocomplete-off fields.
Native unload confirmation protects dirty, composing, buffered and unresolved states on back,
reload or close. Explicit discard requires confirmation and cannot abandon a pending mutation.
Drafts and retry snapshots remain in session memory; confirmed reload/close discards them.

The import section selects a file and explicit format, records its source and permission
evidence, then saves original bytes and the parse outcome. The receipt ID reopens the saved
source; original download returns the byte-exact upload. Failures retain the immutable request
for a same-operation retry by the same actor. A changed selection is a new operation.
Prose labels and scene suggestions remain review findings, unknown speakers remain visible,
and publication permission stays unverified. The UI renders manuscript markup as inert text.
It never assigns voices or silently submits an imported source as an accepted script.

Adaptation tools prepare a context pinned to the imported source ID/checksum, extraction version,
target script and input revision, with a recorded source export permission claim. The host generates
under the user's control and submits a bounded structured result through the tools. Studio opens
the context or proposal ID through `GET /api/v1/studio/adaptations/{id}/review`; it never requests AI
generation. The tagged response admits only the current caller context workflow; retired
server-generated runs have no compatibility review path.

The review shows preserved source text, paged extraction and warnings, the pinned input revision,
coverage claims and unsupported pacing/prosody beside the structured proposal editor and optional advanced JSON. Caller-declared host,
provider, model, settings, usage and cost are explicitly unverified; absent values stay unknown.
A valid submitted result remains a proposal. Its creator acknowledges review of the current text
and explicitly accepts a validated immutable revision through the existing guarded acceptance path.
The exact accepted revision can be reopened read-only. Ordinary editor text stays separate.
Unresolved acceptance retains the original actor, operation ID, base and JSON snapshot for exact
retry; later typing and IME composition survive responses. A changed base requires a new context.
Rights claims do not establish legal eligibility, production approval or publication permission.

An uncertain mutation retains its exact actor/operation/snapshot for reconciliation. A different
actor, stale response or changed input cannot silently replace the visible proposal or accepted
receipt. Revision conflicts require a new reviewed run against the current base. The structured editor is covered by the [issue #4 evidence](../../../docs/evidence/studio-script-editor.md).

## Tokens and evidence

[`packages/ui-system/shared/tokens.json`](../../../packages/ui-system/shared/tokens.json) is the
canonical v0.2.0 light/dark source. Studio links generated shared token CSS directly and uses
`cantos_ui::Theme` under `.studio-shell.cantos-ui`; system/light/dark selection stays scoped.
Generate with `python3 scripts/ui_tokens.py`; append `--check` for drift. The old
`scripts/studio_tokens.py` CLI forwards to it. The [migration](../../../docs/architecture/ui-system-migration.md)
records retained role names and intentional value changes. Noto family names keep system/local
fallbacks; no downloaded font is bundled. CMP mapping remains deferred. Shared component
integration follows in the [UI sequence](../../../docs/work-plan/015-leptos-ui-system.md).

[Browser observations](../../../docs/evidence/script-revision-persistence.md#browser-observations)
separate real keyboard/read/save and inspected captures from pure reducer tests and compilation.
VoiceOver, OS Telex/VNI composition, exact instrumented viewport coverage and native devices
remain unverified; no complete accessibility or full Studio acceptance claim is made.

## Validation, history and review

`POST /api/v1/studio/validation` previews structural and semantic Script IR findings without writing
records or proving source ownership. The preview and review acknowledgement bind the exact
actor/text snapshot and become unavailable after edits; save/acceptance revalidate current access,
evidence and base inside the existing transaction. Field findings retain server paths/rule codes.

`inspector.rs` consumes the existing bounded history, exact revision and script-scoped source
APIs. Historical versions and sources remain read only. The owner review checkbox names a saved
head; the backend distinguishes owner review, editor save and reader access. An uncertain review
retains its exact operation/actor/revision and remains retryable after reauthentication. History
opens never replace the draft. Stale-write resolution explicitly keeps local text while adopting
the compared head as the next base; there is no automatic merge.

Native fields buffer typing, use browser text undo and preserve composition. Pane changes,
structural actions and structural undo wait for buffered input. Escape restores an uncommitted
field; explicit removal confirms and returns focus on cancel. Source and validation are never
inserted into spoken text. The casting section plans inputs; no voice audio preview, TTS,
mixing or listener control is provided.

## Casting and production-input review

The [production v1 contract](../../../services/cantos-server/contracts/studio/production/v1/README.md) provides a static
capability catalog and authenticated settings, rights, preview, freeze, exact snapshot and owner
approval routes. Studio binds existing accepted script characters to catalog provider/model/voice
profiles (synthetic reference, eSpeak NG, VieNeu Nano research and Google Cloud TTS) with
versioned performance/pronunciation and an explicit private-planning budget. Controls follow the
selected profile: voice, language and emotion choices, rate/pitch bounds, and, where a profile has
no intensity control, a choice between each line's setting and an explicit `0`. Switching provider
changes only the provider and model (a sole model is adopted; several require a choice). Kept
voice, language, performance or pronunciation values the profile cannot honor stay unchanged and
are marked unsupported with `aria-invalid` and a described note; the backend still decides
admission and preview.
It displays recorded claim status/term/scope, estimate and typed blockers. Missing rates remain
unknown; a caller-recorded estimate never appears as a reservation or settled charge.

Freeze preserves the exact accepted revision, complete settings, required rights versions and
resolved inputs. A blocked candidate can be inspected, but only an eligible exact owner-reviewed
candidate can receive separate production-input approval. Historical bytes stay immutable;
accepted script/settings/required-rights edits, expiry/revocation and current permission changes
reevaluate eligibility. Browser typing alone changes no backend record. The reference catalog
always reports billable dispatch unavailable, including for an approved planning candidate.

The pure `production.rs` state and `production/browser.rs` adapter remain consumers of shared
DTOs and the existing Studio context; server modules do not enter WASM. Explicit mutations
retain their actor/operation/payload for reconciliation after an uncertain response; one private
closed request (idle, reading, sending, unresolved) prevents a read and a command from overlapping. Native
fields and localized finding/status labels use the Studio's shared focus/theme/token behavior.
[Casting evidence](../../../docs/evidence/casting-production-inputs.md) separates reducer/build
checks from actual browser interaction and inspected viewport/zoom captures.

Recorded rights assertions are not legal verification. This interface creates no real provider
account, terms acceptance, payment, generation request or audio; no cloning/real-person voice is
implied by synthetic labels. The former local generation prerequisite is superseded.

## Shared UI consumer

The shell uses `cantos-ui` root/container, native fields, pane selection, feedback and revision/
casting actions. Studio's [`ui.rs`](src/ui.rs) owns presentation adapters and localized reasons;
it infers no permissions. FormSubmit keeps native Enter/form submission while each form retains
its original guard. Workspace panes remain mounted, preserving session drafts. Explicit revision
discard uses the controlled native Dialog; Escape/cancel preserve the draft. Other existing
navigation/discard paths keep their browser confirmations.

`ActionButton` forwards an optional typed `IconName` into the existing shared Button.
`FormSubmit` accepts an optional caller-owned decorative `ViewFn` slot, wraps it in
`aria-hidden` icon anatomy and keeps one visible label in its native submit button. The slot
adds no handler, form guard or submit effect; no interactive view belongs there. New shared
Icon imports at a consumer site still require admission. The paired VI/EN login fixture
checks adapter anatomy; it does not establish real browser Enter or auth acceptance.

Buffered authoring/casting fields retain native NodeRefs, IME and commit/undo behavior, using
shared FieldLabel/FieldError and `cui-field-control` anatomy. Casting Select preserves PR29's
memoized options, provider/model switching, reactive bounds and full unsupported values. Neutral
0/inherit remain separate. The default comfortable density selects canonical 48px control targets;
app panes retain compact spacing. See the [consumer map and evidence](../../../docs/evidence/ui-system-integration.md).

Native consumer anatomy tests run with `cargo test -p cantos-studio -p cantos-theatre -p cantos-ui --locked`.
They serialize real presentation adapters to HTML; they do not exercise browser events or IME.
SSR is a native test feature only. Browser builds remain CSR.

## Catalog, immutable content and private audio workspace

The **Content & audio** pane calls the actual JSON Studio owner APIs with the Studio cookie
session, its session fence and normal browser Origin enforcement. It lists bounded work/unit pages, creates work/chapter/
episode metadata, edits the selected unit, reads current/exact/history content and appends
text or recording revisions. It creates/reads exact narration/recording briefs with an explicit
Opus mono20/24/32/40 profile, then prepares a private submission, reads bounded file bytes and
SHA-256 in-browser and PUTs octet-stream using the separate upload bearer with
`credentials: omit` (the producer capability, unchanged by the cookie migration). The token stays in browser memory and is never put in result JSON or
storage. The current prepared file must match the expected size/hash; server byte inspection
remains authoritative. No encoding or playback preview runs here.

Owner status, upload revocation, exact QC/rights/editorial review and private candidate
creation/read show real lifecycle facts/blockers. Review consent is bound to immutable
rendition/checksum plus the submitted decision/notes/evidence. A successful candidate is never
publication; `public_delivery_not_implemented` remains visible. Review records do not prove
listening or rights quality by themselves.

Every write keeps one immutable request/operation snapshot. Network/body loss or cancellation
is ambiguous; exact retry retains its identity, including across authentication refusal and
same-actor reauthentication. A different actor cannot retry it. Pending exchanges block new
commands; late or wrong-actor acknowledgements cannot update the workspace. Failed saves retain
browser inputs. Actor switches clear private forms after the shell's dirty-input confirmation.
Selecting another unit clears file/credential/review subject state. Reloading the browser loses
in-memory credentials/intents; reconcile known immutable IDs before preparing a new upload.

The typed adapter also covers Catalog PATCH/archive/reorder; the first UI exposes the narrower
flow above. Source/Script IR content bindings retain their existing typed contracts and editorial
UI, but new Catalog binding controls, work archive/reorder controls and Brief read-grant
management/export are separate UI follow-ups. This slice completes no broader #51/#64/#66
acceptance. [Capability matrix](../../../docs/work-plan/live-client-api-matrix.md).

## Work delegation (owner only)

A selected Work in **Content & audio** shows **Ủy quyền theo tác phẩm** when the signed-in actor
is its `owner_id`; members see a short explanation and no request is sent. The panel calls only
the [delegation v1](../../../services/cantos-server/contracts/studio/delegation/v1/README.md) owner routes, built from the
shared `routes::STUDIO_WORK_DELEGATIONS`/`STUDIO_WORK_DELEGATION` templates with the Studio owner cookie session,
session fence and Origin: it pages grants by `after` (20 per page), issues a grant for a
registered delegate actor with any of `work:read`, `content:read`, `content:write` and a 1/7/14/30
day lifetime, and revokes after a native confirmation dialog. The browser decides no admission
rule: `422` issues (`invalid_actor`, `self_delegation`, `unavailable_actor`, `empty`,
`duplicate_action`, `invalid_expiry`) become field errors, and `403` hides the controls.

The `cantos_dlg_…` secret from a `201` is shown once in a read-only field with an explicit copy
action (clipboard, or a selected field when the clipboard is unavailable) and a warning that it
cannot be shown again. It lives only in memory: no storage, URL, log, `Debug` or JSON dump; it is
dropped on dismissal, revocation, Work or actor change and whenever the session stops
confirming its owner (the shell then hides private workspaces); a visible secret arms the unload
prompt. A lost response keeps the exact operation for **Retry**; a `200` replay returns
`token: null`, so the panel explains that the secret is unrecoverable and offers to revoke and
reissue. An unresolved write keeps the Work selected until it is retried or explicitly abandoned
(the list is then reloaded, because every grant, including one created by the abandoned
attempt, is listed). Grant status text never relies on color; "expired" compares the server's
`expires_at` with the device clock for display only. Evidence:
[Studio delegation UI](../../../docs/evidence/work-scoped-delegation-studio-ui.md).

## Protobuf pilot build and rollback

The four revision/validation routes have a typed Protobuf transport configured by the
`revision-protobuf` Cargo feature. No feature is enabled by default: the shipped build remains
JSON while [ADR 0023](../../../docs/decisions/0023-versioned-protobuf-wire.md)'s compatibility,
PostgreSQL, browser and performance activation gates remain open. The opt-in build is for
explicit pilot validation, not evidence that those gates passed.

From the repository root, using the same installed tools/profile for both builds:

```sh
# Shipped JSON build / rollback (no revision-protobuf feature).
cargo xtask build cantos-studio --web --release
# Explicit Protobuf pilot build; Trunk passes this feature to the Studio package.
TRUNK_BUILD_FEATURES=revision-protobuf cargo xtask build cantos-studio --web --release
# Both pure transport/editor paths on the host.
cargo test --locked -p cantos-studio wire::
cargo test --locked -p cantos-studio --features revision-protobuf wire::
```

Pilot requests offer `application/protobuf, application/json;q=0.5`. A successful response is
read by its actual Content-Type; JSON success/errors remain accepted. Malformed, mislabeled or
unoffered binary successes remain ambiguous `unavailable`. Errors remain JSON ApiError. There
is one fetch per call and no automatic codec fallback, retry or reauthentication. Existing
request deadlines, cookie credentials/Origin, immutable operation identity, draft retention and
actor fences are unchanged. All other APIs remain on their existing JSON helpers; the client
continues to call Studio routes rather than introducing a legacy-alias consumer.

Host tests exercise the real pure wire adapter against independent Python goldens and the real
editor reducer. They do not establish actual browser network counts, AbortController behavior,
proxy behavior or PostgreSQL acceptance. Those are independent runtime gates, including save,
lost body, timeout/navigation cancellation, exact reload and JSON rollback.

## Private Work artwork

The owner-only `studio-artwork` composition uses the existing [artwork-v1](../../../services/cantos-server/contracts/studio/artwork/v1/README.md)
Studio routes through `live::Command` and its session-fenced dispatcher. Select an original
PNG/JPEG, declare provenance and rights, prepare the 30-minute intent, upload raw bytes, then
explicitly select the admitted asset with alt text and the displayed Work revision. The
current cover and local replacement are separate. Upload admission never announces a saved
cover or public delivery. Clearing requires confirmation and sends explicit `artwork: null`;
source bytes and history remain intact.

Client checks are hints: 1 byte–8 MiB, 1–4096 px per dimension, 16,777,216 pixels, provenance
through 4096 UTF-8 bytes, 1–64 distinct valid rights IDs, and alt through 1024 UTF-8 bytes
without control/line-separator characters. The server owns admission, ownership and rights.
XHR progress displays observed transferred bytes only. Private preview reads verify the exact
asset digest/hash/length/type and use transient Blob URLs, never credential-bearing image URLs.

A failed request retains inputs. A revision conflict exposes submitted/current comparison after
an explicit reload; it never silently rebases. An unknown prepare/PUT can read the exact intent;
an unknown selection/clear replays the original operation and CAS request. New mutations and
local discard are blocked while an operation is unresolved. Discarding a local replacement
cannot revoke a committed upload. Artwork activity participates in existing workspace/session
and unload guards; object URLs and listeners are released on replacement/disposal.

Development composition preview: build with `cargo xtask build cantos-studio --web`, then use
`?preview=studio-artwork` in the existing debug Studio runtime. It injects synthetic facts into
the same form and makes no API/storage call. Its v1 scenarios are `empty`, `current`,
`replacement`, `validating`, `preparing`, `prepared`, `uploading`, `awaiting-acknowledgement`,
`admitted`, `failed`, `expired`, `unknown`, and `conflict`. Scenario selection explicitly resets
projection; theme/locale retain draft text. This entry and its fixture module are excluded by
`cfg(debug_assertions)` in release builds. It is a Studio-owned composition using established
UI primitives, not a new shared component or a native Studio surface.

See the [implementation/evidence record](../../../docs/evidence/studio-private-artwork-ui.md)
for browser fixtures, commands and unrun API/storage acceptance. The normal app launcher still
requires its documented local API; mock-browser observations do not waive that prerequisite.

## Explicit API origin for static Hosting

Build with nonsecret `CANTOS_API_ORIGIN=https://api.cantoss.duckdns.org` through the existing
`cargo xtask build|app|deploy` command. Unset retains the local Trunk proxy. All live HTTP,
XHR/private reads and sign-in resolve within this app's own API group; invalid origins
fail closed, and a credentialed upload/read target must be this surface of that origin. The
client never takes an identity-provider URL or provider token as configuration. Owner requests
carry the API's HttpOnly cookie (`credentials: include`); producer capability and anonymous
requests retain their separate authority and omit cookies. The server needs exact per-surface Web
CORS with credentials, CSRF/Origin enforcement and cookie scope (`SameSite=Strict`, host-only,
`Path=/api/v1/studio`). [ADR 0041](../../../docs/decisions/0041-direct-hosting-api-boundary.md#owner-selected-web-cookies-native-bearer-retained-295)
owns the topology; previews need their own declared topology. Native Bearer is unchanged.
The [current production runbook](../../../docs/deployment/vps-production.md) owns destinations;
a successful local build or mocked browser run is not deployed-user acceptance.

**Retry-After and cross-origin reads.** The wait comes from the `Retry-After` header, which a
cross-origin page can read only when the API exposes it (`Access-Control-Expose-Headers`). At this
base `services/cantos-server/src/adapters/inbound/http/cors.rs` does not, so the header reads as
absent and the owner holds for its conservative 30 s default instead of the Server's value (API
owner follow-up; same-origin proxies are unaffected). Cross-tab cookie ordering and logout-while-busy
have host tests only: no browser, Web Locks or real Server/Identity acceptance exists.
