# Studio login entry and writing desk

Issue [#131](https://github.com/loveoverflowcom/cantos/issues/131), based on
`develop@84ebda4f30fbba271ca8bbc68839f69e37afa646`. This bounded Web presentation slice
preserves Studio's cookie-confirmed authority, mounted drafts and existing Catalog/Content/
Brief/private-audio controllers. It introduces no backend, API, schema, permission or token
change. Studio remains Web-only. Live identity-provider and capability acceptance stay with
[the identity handoff](../archive/handoffs/identity-authentication-handoff.md); this slice does not
close #66, #67, #8 or #9.

## Review boundary and component lookup

The login-only entry distinguishes checking, signed out, unavailable, explicit denied and
retained-owner states. Only `SessionState::projects_owner` permits the private projection.
A pure `Entry` projection chooses presentation; it grants no capability. Session cancellation
invalidates the local epoch, clears the accepted HTTP fence and releases held checks without
abandoning ambiguous writes. Cookie confirmation, UUID response correlation and server-owned
OIDC/logout behavior remain in their existing owners.

The writing desk is a Studio-owned composition inside `ContentWorkspace`, using the same
`Facts`, `Dispatch`, request tickets, bounded Work page and selectors as its existing forms.
Initial reads are once per confirmed owner observation; failure/cancellation requires explicit
refresh. A page is never called a portfolio total. Current-Work continuation bypasses selection
and fetching, retaining the exact selected unit/revisions. Other-Work selection uses the
existing selector after an explicit dirty-draft confirmation. Import/editor/audio/new-Work
intents open real existing controls; no successful placeholder modal is used.

Lookup reused qualified `Button` (primary/tonal/text/auth-primary), `FilterChip`, `TextField`,
`Select`, `Surface` and official `LogoVariant::CantosStudio`. Shared status/loading/error/empty
components lack both-renderer qualification for new adoption. The app-owned `CatalogRead`
projection therefore renders native semantic headings/paragraphs within its admitted Surface,
with a live status region for pending/failure facts; it introduces no generic feedback control,
new palette or shared API. An initial preflight rejected those unqualified imports, which were
removed before final admission.
The auth variants and Logo retain the shared catalog's Leptos/CMP qualification records and
unchanged source/token digests. Entry/desk geometry is Web-owned; no shared API or preview/CMP
source is changed. No ADR is needed: the slice selects no new library, data rule or permission.

The Drive [handoff](https://drive.google.com/file/d/1ETmdL9w1ywC10OqjPIrhV9ERy2l59tvJ/view)
and [Studio board](https://drive.google.com/drive/folders/1ljDPKHZ6SvNM5tFvaHjyU87u_NNXBONE)
were read; login, provider-reference and dashboard PNGs were opened. The source HTML identifies
itself as `design-v2-proposal`. Reference HTML SHA-256:
`dcc7e32bd9a9900a586bb0e15fe88d0b4772ef2af3de4d0d3db40373562cfd14`;
board: `8274e00035430fc1a9711b3799506dcaf564f75512351b28db2e9a31397dcc0f`.
The implementation uses official symbolic brand artwork, no downloaded reference raster,
illustrative Work, 04/02/01 count or inferred QC state. Exact illustration fidelity remains
outside this implementation claim.

## Claim and evidence ledger

| Claim / failure mode | Owner / cheapest oracle | Evidence held |
| --- | --- | --- |
| Checking/canceled/denied/other-account entry cannot project private data | `Entry`, existing `SessionState` allow/deny and delayed-reply tests | Native tests and browser evidence recorded below |
| Late A responses cannot replace B facts or regain canceled authority | Existing session UUID/epoch and live request tickets | Existing regressions retained; browser observations recorded below |
| Dashboard page is bounded; failed/unread is not empty | `DashboardQuery` and controlled SSR presentation | New deterministic state and rendering regressions |
| Dashboard navigation retains exact Work/unit and unresolved intent | Existing selector/Dispatch; direct Continue callback | Native blocked/recovery rendering checks and browser observations |
| Ambiguous writes retain A without blocking reauthentication | Separate busy/retention projections, including delegation | New real RequestState/SessionState composition regression |
| Pane return retains native text/caret/selection/scroll | Mounted form subtree and browser-only `FieldReturn` | Browser observation; navigation stores no manuscript bytes |
| No new private or production API | Existing Studio-only command routes and ports | Source review; no API/contract/persistence changes |

## Verification and provenance

Full output, capture driver, browser report and opened PNGs live under the ignored
`artifacts/agent-runs/issue-131/`. Native/SSR checks exercise actual pure owners and presentation;
SSR string output is not DOM or visual acceptance. Browser observations use the real debug CSR
Studio WASM and production HTTP adapters with Playwright-intercepted synthetic same-origin
responses. They are not real cookie, PostgreSQL, OIDC-provider or public-delivery acceptance.

| Command / observation | Result and scope |
| --- | --- |
| `cargo xtask verify web` | PASS at the Web checkpoint: Studio/Theatre WASM lint/build and shared preview build. Final Studio source is rechecked by changed-only preflight. |
| `cargo xtask ui-preview leptos verify` | PASS: native shared-component tests, WASM lint and preview build; shared source/tokens unchanged. |
| `INVALID=1 node artifacts/agent-runs/issue-131/browser.cjs` | PASS: nine real-browser scenarios against intercepted synthetic HTTP; 19 captures. |
| `cargo xtask preflight --changed-only` | Final handoff command; the PR records its actual final gate summary. |

Chrome `153.0.8010.36`, headless, `--no-sandbox`, Playwright 1.61.1; debug CSR served through
`cargo xtask app cantos-studio --web`. A temporary loopback synthetic API boundary satisfied
that command's backend-listener prerequisite; all browser API requests were intercepted by
owned fixtures. No database, identity account, grant, provider or real session was provisioned.
Both task-owned server processes were stopped after the browser run.

The final browser pass drove trusted Enter/selection input and pointer actions: production-mode
login with no private read/token/registration; synthetic callback followed by actual current-
session observation; one bounded Work page; Work/unit selection and Continue without a read or
mutation; new-Work/import/editor/audio destinations; invalid advanced JSON and exact native
selection retained across desk return; canceled observation followed by delayed A success;
explicit A recheck; failed list versus successful empty retry; explicit 403 denial; development
login and ambiguous-write A→B→A isolation with no automatic repeated POST. No page errors
occurred in that final run. These are `DOM-tested` and `interaction-tested` within the fixture
boundary, with focused keyboard/semantics assertions; no complete screen-reader/Tab tour is
claimed.

Opened final images (agent review, vi-VN, DPR 1, fonts ready):

- `login-{320,768,1024,1280}-{light,dark}.png` — login-only composition and readable actions.
- `dashboard-{320,768,1024,1280}-{light,dark}.png` — bounded page, long Vietnamese title,
  wide rail and narrower navigation, intact labels without horizontal overflow.
- `dashboard-320-text200.png` — 200% root text scaling, reduced motion; native selected text
  abbreviates but the full destination heading is adjacent. This is not browser page zoom.
- `dashboard-empty.png`, `login-denied.png` — explanatory empty/denied states and recovery.

The three first-pass visual findings were rechecked on the same fixtures: the empty login status
strip is hidden, irrelevant Continue/Next controls are hidden instead of reporting a false
session blocker, and compact native navigation removes the oversized narrow rail. Final opened
captures support `cross-viewport-inspected` and `cross-theme-inspected`; hashes are retained in
`capture-hashes.json`. No exact pixel, contrast-measurement or motion-recording claim follows.

The browser pass also exposed retired structured-field attributes reading disposed signals
when advanced JSON became invalid on blur. A blur-only reproduction isolated that existing
component path from pane navigation; clean-develop runtime was not established. `BufferedText`
and its error projection now use teardown-safe reads, fail closed for retired readonly inputs,
and skip value synchronization when the source has retired. The original invalid-text navigation
and same-owner session return passed afterward. No Script IR validity or save rule changed.

Initial failures remain in the run record. The first broad Rust run could not bind fixture
sockets in the sandbox. With loopback allowed, the API unit suite passed (479 tests), and the
broad run stopped at a new dashboard test whose supposedly different Works had duplicate IDs.
That test fixture now has distinct identities and an explicit inequality assertion; final
Studio preflight rechecks it. Early Web lint/compile failures in the new navigation helper were
corrected before the passing checkpoint. No failed run is relabeled as PASS.

## UI scope and remaining acceptance

Surface / permission context: Studio only; existing `/api/v1/studio/*` adapters. Dashboard
intensity is Medium, editor remains Low, auth uses admitted expressive primary controls.
Canonical semantic roles supply colors, shapes, focus, typography and spacing. Local reading
column widths/breakpoints are documented renderer geometry. Both vi-VN and en resources are
exhaustive. No motion is added; existing shared button press behavior and reduced-motion policy
remain shared-owned.

The registered Quint catalog was inspected. Its current accepted capability is private audio
admission, not Studio login/navigation; no model or conformance claim is made here. Native
session/request tests remain the applicable deterministic oracle.

Live wrong-group acceptance is still pending: the existing BFF maps inactive/unbound OIDC
identity to 401, and current-session observation presents that as anonymous. Explicit 403
presentation does not prove the real provider returns that denial to Leptos. No self-service
role chooser, Theatre credential fallback or provider password form is introduced.

Residual risk: real provider/cookie/issuer/group and cross-tab integration, OS Vietnamese IME,
screen-reader/complete Tab walkthrough, English runtime layouts, actual browser page zoom, Firefox/WebKit and exact reference artwork
fidelity require separate acceptance. Backend/persistence behavior, API contracts, audio bytes,
rights, review/publication rules and native clients are unchanged and receive no new acceptance
claim. Production gate owners remain authoritative; this PR is part of #131.

## Compact writing desk — 28 October redesign delta (#131)

Source: `develop@7b132b49` after the 28 October owner review (screenshot
`12-recovery-sign-in-again-reaches-workspace.png`: oversized resume card, one-row list inside a
large card, long audio explanation with no chapter selected). The delta is presentation only:
`live/dashboard.rs`, `studio.css`, four dashboard copy entries in the localization catalog (and the
digest headers every generated consumer carries). No API, session, draft, permission or selector
change; the existing callbacks, ids and blocked/pending projections are unchanged.

| Claim / failure mode | Oracle | Evidence held |
| --- | --- | --- |
| The Work list follows the heading and actions and precedes resume, audio and tools | `work_list_follows_the_heading_and_precedes_resume_and_tools` (SSR order) | native SSR |
| No resume card without a selection; a short hint appears instead; every entrypoint stays rendered | `without_a_selection_there_is_a_short_hint_and_no_resume_block` | native SSR |
| Audio lines, explanation and "not read" line follow facts actually read | `audio_notes` table test (pure core) | native unit |
| Pagination shows only existing directions; none on a single page | `pagination_offers_only_the_directions_that_exist` | native SSR |
| Failed/unread/loading never render a successful empty page; blocked recovery stays reachable | existing dashboard tests, updated | native SSR |

**Layout observation (harness, not the app).** The real `Dashboard` and `WorkspaceNavigation`
markup (native SSR) was loaded in Chromium with the real `tokens.css`, `foundation.css` and
`studio.css`, before (`develop@89c5964c`) and after, one selected Work with a long Vietnamese title,
CSS px, DPR 1, light theme. The harness has no topbar, session, cookies, network or WASM, and is
not Studio runtime acceptance. Top of the Work list: 1280 400 → 160, 768 562 → 302, 390 745 → 299,
320 745 → 329; no horizontal page overflow at 1280/1024/768/390/320 in either state. Dark theme
was opened at 1024 (list, selected marker, resume and tools legible).

**Real app (WASM) on an authorized local session.** A temporary, uncommitted test fixture started
the real `cantos-identity` executable (real Argon2id, run-owned loopback SMTP sink, file-backed
Turso), signed one synthetic account up through the mailed link, and hosted the production Studio
API router on `127.0.0.1:18080` with that account's subject as the Studio roster; `trunk serve`
on `localhost:8083` served the debug CSR Studio and proxied `/api/`. This is the #565 fixture
pattern, not production provisioning, HTTPS, a deployed origin or a real mailbox. Chromium in the
app browser pane (version not exposed), vi-VN, DPR 1. Real form login reached the writing desk;
the Work was created through the real create form (one Work, long Vietnamese title).

| Observation (this branch unless stated) | Result |
| --- | --- |
| Empty list, login → desk | List card with the existing empty copy; Import primary, New work secondary; audio and tools as a compact secondary column |
| Top of Work list, one Work, none selected, 1280 | `develop@7b132b49`: 438; this branch: 271 (CSS px) |
| Top of Work list, Work selected, 1280 / 768 / 390 / 320 | 238 / 380 / 433 / 463; no horizontal page overflow at any width (topbar included) |
| 320 CSS px with root text 200% (not browser page zoom), dark | no horizontal overflow; list starts at 1300 because the stacked topbar (#558) and nav strip precede it |
| Dark theme at 1280 | list, selected marker, resume and tools legible |
| Keyboard: focus on the Work row | 3 px solid focus ring visible on the row |
| Resume → Content → Back (Enter) → Resume | selection stayed marked (`aria-current`); an unsaved title edit and its selection range (4–9) survived the round trip; focus moved to the desk heading on Back and back into the edited field on return; the unsaved-changes notice stayed |

An earlier Back attempt on this branch left focus on `<body>`; it was a keypress delivered while
the Content pane was still settling, and the identical steps repeated cleanly (clean `develop`
behaves the same in the stable case). Not observed: English layout, screen reader, Firefox/WebKit,
real page zoom, session expiry/account-switch during a fetch on the new layout, and a populated
multi-page list (one Work only), so the pagination controls were exercised by SSR tests only.
