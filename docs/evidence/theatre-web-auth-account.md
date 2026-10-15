# Theatre Web Home, contextual auth and Account — issue 129

## Scope and invariant

[Issue 129](https://github.com/loveoverflowcom/cantos/issues/129) applies Auth–Account design v2
to Cantos Theatre Web. Public Home remains usable without a listener session; authentication is
an explicit user intent. Back, browser history, provider cancellation and delayed responses must
not reopen a closed auth intent, relabel another account's private projection or change playback.
A route, fixture, provider callback hash or rights claim is not authentication or registration
success. The existing account/session owner remains authoritative.

The change includes the inherited [Shared Back presentation repair](shared-back-v3-alignment.md),
anonymous published-work discovery, typed auth return context, Login and unavailable-registration
presentation, guest/signed-in Account, compact appearance preferences, a separate unavailable
Membership information route, and a bounded provider-cancellation HTTP/SQL repair. It introduces
no password collection, registration API, entitlement or payment operation.

The owner boundaries are the [Web implementation guide](../../apps/cantos-theatre/web/README.md),
[Theatre identity architecture](../architecture/theatre-identity.md),
[identity handoff](../work-plan/theatre-identity-handoff.md),
[API surface decision](../decisions/0015-api-surface-boundaries.md) and
[Membership interest contract](../product/membership-interest.md). Theatre calls only its own
`/api/v1/theatre/*` routes. Studio handlers, sessions and permission contexts are not reused.

## Source and design provenance

- Working branch: `codex/issue-129-theatre-auth-account`; inspected starting HEAD:
  `1854b3497a149654d3ac246e882f09fc81cd960a`. Before handoff the branch was fast-forwarded to
  `develop@ff5a86aa4f1c3e67901026dfa31ed284493dd3b5` and the task changes restored without conflict.
  The delivered source is the implementation tree in this PR; its changed source-file SHA-256
  inventory is retained locally in `artifacts/agent-runs/shared-back-pr129/source-manifest.json`.
- The actual [design v2 handoff](https://drive.google.com/file/d/1ETmdL9w1ywC10OqjPIrhV9ERy2l59tvJ/view)
  was read through the connected Google Drive tools. Its [Web reference folder](https://drive.google.com/drive/folders/1ZpqcXLcPRUT6bP0_ygyDISGAq98TemUH)
  was listed, and the original PNG bytes below were downloaded and opened. The design's HTML
  timer/demo state was not imported as authentication.
- Local opened references: `/private/tmp/cantos-issue129-design/home.png`, `login.png`,
  `account-guest.png`, `account-signed.png`. They depict prototype Chromium pixels, not Leptos
  runtime acceptance. Original Drive names are `theatre-web-home.png`, `theatre-web-login.png`,
  `theatre-web-account-guest.png` and `theatre-web-account-signed.png`.
- The shared official Logo Kit is reused. The live auth panel deliberately uses the official
  symbol as decoration; it excludes the landscape paintings whose usage rights remain
  unverified. This is a recorded visual difference, not a full design-pixel parity claim.
- Final verification logs belong to ignored `artifacts/agent-runs/shared-back-pr129/`.
  Product browser artifacts belong to ignored `artifacts/ui-evidence/issue129/`.
  Codex root opened the screenshots listed below. The browser was Codex's visible in-app browser
  on macOS; its engine/version and underlying launch flags were not observed. DOM facts are
  recorded in `browser-facts.json`; they are observations, not verifier output.

## Implemented ownership and guarantees

| Boundary | Implemented behavior | Failure prevented |
| --- | --- | --- |
| Public discovery application/adapter | Independent checked read generation; duplicate loading requests are inert; only the current pending request settles. One fixed-JSON `GET /api/v1/theatre/works?limit=20` omits credentials. The decoder validates the public DTO, unique work IDs and a 2 MiB response bound, including legal maximum-length catalog records. | A session 401/provider outage creates a login wall; an error becomes an empty catalog; stale or malformed data replaces current public content. |
| Public Home composition | Actual published titles/descriptions/kinds, explicit loading/empty/unavailable/protocol states and real Retry. No synthetic works, artwork, detail links or playback are substituted for unavailable live data. | Fixture data is presented as an actual publication or an unusable control pretends to play. |
| Pure auth navigation | Closed Home/Account/Membership return destinations, including Membership opened from Account. Canonical hash reconstruction preserves context; unknown/URL/duplicate-query context falls back internally. Checked epochs, one pending request and synchronous invalidation fence Back/cancel/logout/navigation. | Open redirects, lost Membership return intent, duplicate submission and late completion reopening closed auth. |
| Browser shell | One live composition owns route/history and injected locale/theme/account state. Back uses a shell-created history entry or replaces a deep link with its typed fallback. Contextual entry focus/scroll is restored, including bounded nonsecret provider-return snapshots. | History traps, duplicate same-route entries, focus returning to an unrelated entry, automatic provider redirect after cancel. |
| Existing account owner | Checking, Guest, SignedIn, Failed and LogoutPending remain distinct. Private projections clear before logout I/O. Stale-only cleanup, expected-session logout, session mismatch, retry and generation/session fences remain intact. Login stays blocked until required cleanup is acknowledged. | Stale account A data appears under B; a delayed cleanup revokes B; a second login starts over unresolved credentials. |
| Account presentation | Guest profile and vertical auth actions; signed-in fallback identity and Library link; separate Membership information and appearance settings; actual logout/retry callbacks. No actor UUID used as a display name, invented total counts, guest conflict choices or fabricated Membership status. | Design fixtures become production profile/entitlement facts. |
| Registration boundary | The pinned Kanidm configuration, Theatre router and identity handoff expose no verified supported self-registration entry/callback capability. The app renders an explained unavailable state and does not collect a password or show successful registration from a URL. | Invented `/register`, credentials in BFF/browser storage, fake success or bypass of provider verification/MFA. |

### Exact provider return and cancellation

The browser creates a 32-character lowercase-hex **nonsecret UI correlation intent** and stores
only `<intent>|<canonical login hash>` in session storage. Login passes that intent to the Theatre
BFF. The BFF places it in a separate HttpOnly, SameSite=Lax, auth-path cookie named for the exact
random OAuth state; HTTPS uses the secure cookie namespace. This companion does not replace the
existing private state/browser binder, PKCE, nonce, issuer/audience verification or current session.

The callback validates exclusive code/error shape, the exact state inventory and matching private
binder. Only fixed `/#auth-return?intent=…` or `/#auth-cancelled?intent=…` destinations are emitted;
provider error text and caller redirect URLs are ignored. The pure browser parser accepts exactly
one matching intent and canonical Login context. Old callback A does not consume newer stored
context B. A matching completion still waits for the actual current `/session` result before
navigating; a callback hash alone grants no authority.

Provider cancellation durably marks only the matching unexpired flow consumed/cancelled under
its state, binder, issuer, client and redirect. The flow row locks before its issued session,
matching issuance's flow/session order. Cancellation either fences a paused exchange's final
issuance or locally revokes the session already issued by that exact flow. Time is rechecked after
lock/trigger waits. Local revocation commits before bounded best-effort provider revocation;
pending remote cleanup remains durable. Another flow/account B is not selected. Only A's companion
return cookie is expired; the response emits no shared session-cookie expiry.

The new PostgreSQL scenarios exercise forged/wrong binders, cancellation without code exchange,
late code after cancel, cancellation during a paused exchange, exact per-flow UI intents and
completed A cancellation preserving B. They use the existing signed provider mock and actual
disposable PostgreSQL when the gate runs; they do not establish live Kanidm acceptance.

## Production and diagnostic rendering

The default mode uses actual anonymous discovery and the separate Theatre account boundary.
`?mode=auth-preview` explicitly injects diagnostic states into the **same production
Home/Auth/Account shell and components**. Its visible controls can select Guest or synthetic A/B,
hold/deliver an old session completion, fail/acknowledge cleanup and cancel an intent. No network,
provider, password or timer creates success in this mode. Fixture completion exercises the real
account transition owner; it is never a live login or registration claim. Unknown query modes
retain the live boundary rather than selecting fixtures.

The pre-existing `?mode=mock` listening demo and `?mode=membership-preview` remain separately
explicit. Membership form/receipt/plans are owned by #128. The production unavailable information
route does not mount the Membership form or grant an entitlement. No Library domain, player,
release identity, seek/play behavior or guest-progress merge is rewritten.

The two primary destinations remain **Khám phá** and **Của tôi**, following #35; Account is an avatar
entry. The v2 PNG's third primary Account tab is intentionally not adopted. This reconciliation is
recorded in the [owning Web guide](../../apps/cantos-theatre/web/README.md#scope-and-honest-behavior).

## Component lookup and admission

The inherited Back repair keeps the label background quiet in disabled/loading states, preserving
the tonal icon circle, focus, native disabled/busy attributes and visible reason. Its own before/
after comparisons, six-component recheck and historical gates remain in the
[separate ledger](shared-back-v3-alignment.md); they do not establish the new product journeys.

The [preview catalog](../../packages/ui-system/shared/preview-catalog.json) records qualifications
for `Button`, `TextField`, `Surface`, `Badge`, `BackButton` and `Logo`. The new compositions reuse
the applicable admitted Button auth variants, Surface auth/profile/Membership/account-group
variants, BackButton and official Logo. Existing UiRoot/Container compatibility imports retain
their original owner site. No shared primitive, token copy or baseline-consumer exemption is
introduced. Fresh source/token qualification and product observations remain separate gates.

## Layered evidence and command ledger

This record separates implemented claims, browser observations and command results. Final
verification uses `PATH="/opt/homebrew/bin:$PATH" RUSTUP_TOOLCHAIN=1.98.1 RUST_TEST_THREADS=1
TMPDIR=/private/tmp`. No command PASS is inferred from source or browser observations. The full
PostgreSQL gate failed at the pre-existing artwork test described below; passing Theatre tests
do not waive that gate.

| Layer | Claim/oracle | Current evidence boundary |
| --- | --- | --- |
| L1 — Abstraction & Architecture | Independent source review of inward imports, separate Theatre routes, anonymous discovery/session separation, exact cancellation scope and no provider calls from components. | `documented`: review completed with no actionable backend correctness defect identified. This is not compiler or runtime evidence. |
| L2 — Domain, Types & Error Handling | Closed return routes; canonical provider-context constructor/parser; exact nonce correlation; checked auth/discovery generations and typed failures. | `type-enforced`, `statically-checked`: native lint/tests passed in the current preflight. |
| L3 — Unit & Property Verification | Semantic tests for each origin, browser-stack-independent fallback, hostile URL/query context, A/B mismatch, duplicate/late completion, epoch exhaustion, cleanup barrier and maximum legal discovery response. | `example-tested`: current Theatre native suite 164 passed; API native suite 481 passed, 68 PostgreSQL/tool cases ignored for their owning gates. No new Quint/Kani/property/fuzz result is claimed. |
| L4 — Application & I/O Adapters | Actual component composition and native string-render tests; explicit no-network auth-preview fixtures; anonymous fixed-JSON adapter/error mapping. | Native component/adapter tests passed. The same production shell's diagnostic interactions were driven in the browser, as scoped below. Native string rendering is not `DOM-tested`; fixture interaction is not real-provider acceptance. |
| L5 — DB, API & Integration | Signed-mock provider plus real disposable PostgreSQL callback/issuance/cancel/session tests; real browser Home/Account/Auth/history/focus observations. | `integration-tested` on PostgreSQL 17.11 with the signed provider mock: Theatre identity 19/19 passed. Full PostgreSQL gate: **FAIL** at an unrelated baseline test; later stages **NOT_RUN**. Browser evidence is scoped below. Live Kanidm, deployed TLS and production acceptance remain unrun. |

| Exact command / obligation | Final result | Evidence / scope |
| --- | --- | --- |
| `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify repository` | PASS | `repository-current.log` and current preflight: repository policy, generated drift/import admission and document checks. On the updated base: 707 text files, 19 skills, 442 Python tests with 5 optional Pillow cases skipped. The inherited comparator self-test separately ran those adapter cases. |
| `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask verify rust` | PASS | `rust-serial.log`: full workspace format/lint/tests passed serially before the final frontend refinements. Current affected-package native checks were rerun by preflight, including the new Library entry regression. |
| `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify contracts` | PASS | `contracts-final.log`: script-ir-reference, protobuf-reference, contract-tests, wav-reference and audio-inspection. No new registration/membership wire contract. |
| `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify web` | PASS | `web-recheck.log` passed before the final navigation refinements; current `preflight-complete.log` reran WASM lint, both Trunk app builds and gallery build. No browser acceptance is inferred from builds. |
| `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview leptos verify` | PASS | `preview-final.log`: native tests/doctests, WASM lint and gallery build. New product journeys remain separate. |
| `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask verify postgres` | FAIL | `postgres.log`: disposable PostgreSQL 17.11; Theatre identity 19/19 passed before the pre-existing artwork inventory assertion failed. Later stages were not reached. |
| `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask preflight --changed-only --skip postgres` | PASS with 1 explicitly skipped (NOT_RUN) step | `preflight-complete.log`: repository, rust-format, affected-package rust-lint/tests, dbsp, wav-reference, audio-inspection, web-lint, both web-builds and ui-preview PASS. PostgreSQL is explicitly NOT_RUN here because the standalone run recorded the known failure. Its required merge gate remains unmet. |
| `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask run cantos-theatre --web` | Observed ready and stopped | `browser-serve.log`, `browser-final-serve.log`: task-owned local CSR host on 127.0.0.1:8082; no infrastructure provisioning or live provider activation. |
| Fresh product browser matrix and opened captures | Observed within the scope below | `artifacts/ui-evidence/issue129/browser-facts.json`, named DOM snapshots and opened product captures; inherited Shared Back images remain separate. |

No registered behavior model changes. Catalog lookup found only media admission/publishing draft
capabilities and listening-progress `needs-decision`; no accepted auth/navigation capability exists.
No model simulation is presented as implementation conformance. Script IR/validator/mutation scope
is unchanged; applicable required gates remain owned by the final preflight selection.

### Actual failures and independent recovery

The full PostgreSQL run failed in
`contexts::media::postgres::artwork::tests::additive_upgrade_preserves_exact_catalog_snapshot_operation_bytes_and_pinned_checksums`
at `services/cantos-api/tests/support/private_artwork_postgres.rs:674`: actual migrations 1–26,
expected 1–25. That fixture, migration 26, migration loader and runner have identical Git blobs
in the original task base and `develop@ff5a86a`; this feature changes none of them. This is a
confirmed pre-existing test expectation defect, rather than an environment limitation or a
passing full gate. The artwork suite finished 8 passed/1 failed. Later artwork repository/storage,
listener-library, audio-intake, host and coordinated restart stages are NOT_RUN. The disposable
server was stopped. `postgres-diagnosis.md` retains the local source-object comparison and logs.

Earlier feature failures remain in the run directory: a missing required Back ID, WASM needless
return, test-only issuer fixtures and an imprecise string selector were corrected. The first
final preflight also caught a test-only import after the Library CTA repair; it was moved into
the test before rerunning. The existing xtask port-ownership test failed in parallel native runs,
then passed both its isolated serial diagnosis and the full serial Rust verification. Its source
was not changed. Earlier FAIL logs remain FAIL; later results have their own named logs.

## Browser matrix and UI/Web report

Codex root drove the local CSR app through the in-app browser, read the live DOM and opened all
listed screenshots. The default Home was observed while services were unavailable; the auth-state
journeys used the explicitly labeled no-network `?mode=auth-preview` boundary. There was no live
issuer, authenticated external provider or real listening media in these browser observations.

| Scenario | Required observation | Result/artifact |
| --- | --- | --- |
| Anonymous default Home with unavailable services | Public content/read states remain accessible; no implicit login request or redirect loop. | Observed public Home and recoverable outage while account/identity were unavailable. `home-live.jpg` and `home-live-dom.txt`; the DOM snapshot records the initial public-loading state. This does not establish a successful published-work network read or live 401 cleanup. |
| Home → Account → Login → Back; Home → Login and Register → Login → origin | Correct typed destination, browser Back/Forward/reload, entry scroll/focus and Escape; no duplicate-entry trap. | Contextual Home/Account Back, Escape, browser history and reload were driven. Direct Login without context returned to `#discover`; `browser-facts.json` records `direct-login-fallback`. Back restored the relevant Home/Account entry focus in the observed journeys. |
| Account → Membership → Login → Back | Membership and its original Account return intent retained. | The initial Membership focus-return issue was repaired and the same journey repeated: Membership return and its original Account intent/focus were retained. This is an interaction recheck; there is no reference-pixel comparison for the repair. |
| Guest/checking/failed/signed-in/logout-pending; late A after cancel/B | Distinct states, acknowledged cleanup before Login, private projections fenced; fixture/live scopes named separately. | Explicit synthetic A/B, held session completion, cleanup failure/acknowledgement and cancel controls were driven. Old A after B stayed fenced (`account-stale-a-after-b-dark.jpg`); cleanup failure kept Login blocked until acknowledgement. `late-after-back-dom.txt` and facts record Account retained after late completion following Back. |
| Current and cancelled auth presentation intents | Current completion returns to its origin; an old completion cannot reopen auth after Back/cancel. | `browser-facts.json` records current completion returning to Account, late completion retaining Account after Back, and cancel/late-response retaining guest Login without redirect. `cancel-late-response-dom.txt` reports the old A completion was ignored. These are diagnostic presentation/account-owner observations. Exact provider nonce A/B matching and durable cancellation remain separately scoped to pure/SQL tests; real provider login/cancel was not run. |
| Direct unconfirmed registration-completion route | A route alone cannot display successful signup or Membership activation. | Observed an unavailable/unconfirmed registration presentation, with no password collection or fabricated success. `register-unavailable-dark-390.jpg` is the opened narrow unavailable-registration capture. |
| Requested widths 320/390/768/1280, light/dark, Vietnamese text | Open each attributed capture; inspect reflow, hierarchy, clipping and Back/action layout. | All eight Login width/theme captures were opened. Fonts were loaded and DOM horizontal overflow was false in every case. Wide Login had two columns; narrower layouts collapsed to the auth content. Exact measured widths and captures follow. |
| Native browser zoom 200% | Repeat the relevant layouts and interactions at actual 200% zoom. | **NOT_RUN.** The attempted keyboard shortcut did not change measured browser metrics; native Chrome access was unavailable without Computer Use permission. These width/theme captures do not satisfy the 200% zoom criterion. |

The width/height facts are the values recorded by the browser observation, not inferred from an
image's file dimensions. Each requested frame was 900 px high. The recorded layout widths were
1265/753/375/305 for requested 1280/768/390/320 respectively; the discrepancy is retained rather
than silently reporting a different measured viewport. All rows use Vietnamese, ordinary text
scale and the named explicit theme.

| Requested width | Recorded layout width | Opened Light capture | Opened Dark capture | DOM observation |
| --- | --- | --- | --- | --- |
| 1280 | 1265 | `login-guest-light-1280.jpg` | `login-guest-dark-1280.jpg` | Fonts loaded; no horizontal overflow; wide two-column composition. |
| 768 | 753 | `login-guest-light-768.jpg` | `login-guest-dark-768.jpg` | Fonts loaded; no horizontal overflow; collapsed auth layout. |
| 390 | 375 | `login-guest-light-390.jpg` | `login-guest-dark-390.jpg` | Fonts loaded; no horizontal overflow; readable stacked auth actions. |
| 320 | 305 | `login-guest-light-320.jpg` | `login-guest-dark-320.jpg` | Fonts loaded; no horizontal overflow; readable stacked auth actions. |

Other opened product captures include `home-live.jpg`, `login-live-light-1280.jpg`,
`account-guest-light-1280.jpg`, `account-guest-system-dark-1280.jpg`,
`account-stale-a-after-b-dark.jpg` and `register-unavailable-dark-390.jpg`. The intermediate
`login-preview-wide-resize-transition.jpg` is not a settled-layout acceptance image. Screenshots
are qualitative JPEG observations, not lossless approved design baselines or pixel-diff results.
The reviewer was Codex root; engine/version, capture timestamps and underlying browser flags
were not recorded in `browser-facts.json`.

After the final source review, Codex root repeated Account → Login → current fixture completion
against the current build. The old Guest Login entry no longer existed on SignedIn Account;
focus fell back to `theatre-main` with `tabindex=-1`, scroll Y 0 and zero audio/video elements.
Opened `account-signed-focus-fallback.jpg` and `final-focus-facts.json` record that repair.
The guest Library entry now explicitly opens Account. Library → Account → Login → Back was
repeated: `#account`, active `theatre-account-login`, restored scroll Y 615 and zero media elements
(`final-library-return-facts.json`, `final-library-return-dom.txt`). Existing entry focus/scroll
and absent-entry fallback therefore have separate observed outcomes. These remain no-network
diagnostic journeys, not a live provider roundtrip. The final task-owned browser tab/server were
closed/stopped.

| Report field | Recorded scope / outstanding evidence |
| --- | --- |
| Invariant / owner | Public Home and explicit contextual auth; application owns typed navigation/session/read facts, browser/HTTP/SQL shells own effects. |
| Evidence level / command / provenance | Browser evidence is `DOM-tested`, `interaction-tested`, `screenshot-inspected`, `cross-viewport-inspected` and `cross-theme-inspected` for the named scenarios above. Source review is `documented`; source hashes, named test results and independent command statuses are recorded in the ledger. |
| Edge classes | Guest/401/outage/protocol/empty/maximal data; duplicate/stale/mismatched callbacks; Back/Forward/deep links; cleanup failure/retry/session switch; cancellation before and after issuance. |
| Screens / flow / consumers | Theatre Web Home, Login, unavailable registration, Account, appearance preferences and unavailable Membership information. Studio/CMP consumers unchanged. |
| Surface / permission context | Theatre only; anonymous `/works` and separate authenticated `/session`/readonly Library; auth routes use the Theatre BFF. |
| Component lookup / admission | Applicable members of the six qualified shared APIs above; existing compatibility owner retained. Current repository gate passed admission/freshness checks after the base update. |
| Surface intensity | Medium auth/account; calm public discovery, quiet Back and tonal profile/Membership grouping. Opened Login/Account captures retain the vertical primary/secondary hierarchy and distinct tonal groups at the named widths/themes. |
| Renderer / target / build | Leptos 0.7.8 CSR/WASM, existing Trunk command, local debug build; current preflight Web builds PASS. Codex in-app browser on macOS; engine/version and launch flags unobserved. Source hashes and task-owned serving logs retain local provenance. |
| Backend for evidence | Default Home was observed with services unavailable. Auth preview used explicit no-network synthetic A/B and an empty public catalog. No live issuer/provider login was exercised. SQL tests use the signed provider mock plus disposable PostgreSQL; their final gate result is separate. |
| Tokens / style review | Existing canonical v0.6.0 roles and scoped shared variants; renderer-local app geometry only. No shared token change. Source review found valid hyphenated token names. Opened Light/Dark pixels support qualitative hierarchy/readability; no new quantitative computed-color/contrast result is claimed. |
| Localization | Typed Vietnamese/English Auth/Account/Home/diagnostic resources; listener-facing brand remains Cantos. Opened Vietnamese Login/Account/unavailable-registration captures preserve marks and wrapping, with loaded fonts and no horizontal overflow in the Login matrix. English runtime and scaled long-text acceptance are not established by those captures. |
| Accessibility | Native names/reasons/statuses were read from the live DOM; keyboard Escape and contextual entry-focus restoration were driven, including the repaired Membership journey. This is scoped `accessibility-checked` evidence. A complete Tab-order audit and named screen-reader walkthrough were not recorded. |
| Visual / same-scenario recheck | Codex root opened the actual v2 references and the named Leptos captures above. The eight settled Login captures cover ordinary-scale Light/Dark widths; the Membership focus fix was rechecked by repeating the journey. No full design-pixel parity or 200% zoom claim. |
| Motion | Existing shared control feedback/reduced-motion policy retained. Product interruption/reduced-motion runtime observation was not recorded; still images do not establish motion. |
| Async races | Diagnostic held A after B/cancel/Back and current auth completion were driven as described above. Pure nonce/epoch and SQL cancelled-flow issuance tests remain separately attributed to the final gate results. Runtime unmount was not observed. |
| Input methods | Pointer input, keyboard Escape and browser history actions were driven in the IAB. No password form is mounted. OS IME, touch and assistive-technology walkthroughs were not observed; native 200% zoom is NOT_RUN for the reason above. |
| Media path | None in the default/auth-preview shell; no audio element, Range request or storage/CDN playback exercised by auth/navigation. Existing explicit mock remains isolated. |
| Performance / cache scope | Public HTTP response, static assets, loaded WASM/code and root reactive-state lifetimes are separate scopes. No startup/cache-hit/reactive-cost measurements or speedup claim; no cache policy change. |

## Deferred acceptance and residual risk

Passed core, Web and preview checks support the requested scoped PR; PostgreSQL acceptance remains
incomplete because its full gate FAILs at the confirmed baseline expectation defect above. Its later
stages are NOT_RUN. Correct that test in its owning change and rerun `cargo xtask verify postgres`
before accepting the required merge gate. No unrelated migration/test repair is included here.
Native 200% zoom is NOT_RUN because the shortcut left metrics unchanged and the native Chrome
fallback lacked Computer Use permission.
Engine/version provenance, actual 200% reflow, full Tab order, named screen-reader and product
motion/reduced-motion acceptance remain incomplete.

Live Kanidm login/cancel/signup, provider grants/client configuration, TLS/credentials and
production activation remain outside this UI change. Verified self-registration capability and a
real registration receipt are required before any success/plans action becomes production behavior.
Membership form/receipt/plans (#128), Library/player rewrites, native lifecycle/audio, guest merge,
real playback/CDN and named screen-reader/device acceptance are excluded. Prototype HTML pixels,
fixture A/B and signed-mock SQL tests cannot accept those boundaries.
