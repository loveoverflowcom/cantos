# Theatre Web auth and spacing — Mac acceptance, 16 October 2026

The [same-day account-policy follow-up](#account-policy-and-server-admission-follow-up) below
supersedes this earlier run's open signup-policy decision. Earlier observations remain historical.

Part of [#191](https://github.com/loveoverflowcom/cantos/issues/191), delivered on the existing
Draft [PR #196](https://github.com/loveoverflowcom/cantos/pull/196), branch
`fix/theatre-auth-activation`. Starting HEAD and remote were
`e8e9a17a0d9b430dfd01e196d2c3d91f5ba0bd90`; fetched `origin/develop` remained
`f3619bff1d69246f22be2906570a3933ea922f53`. This supplements the
[native activation record](theatre-auth-activation.md); it does not accept native identity.

## Implemented repairs and boundaries

**Invariant:** a returned login without a current session must allow a fresh login after
acknowledged cookie cleanup, while outbound requests and unacknowledged cleanup stay fenced.
The existing `LiveNavigation` observer now distinguishes `ConfirmingReturn` from `Outbound`.
Previously, session 401 followed by cleanup 204 produced `Guest` with the callback request
still pending: the enabled login button silently refused the next request as a duplicate.
Only the completed callback fence is invalidated on Guest. Session generation, auth epoch,
same-origin transport, server admission and provider protocol retain their existing owners.

**UI invariant:** the Library status panel and its Account entry have visible separation.
Actual Chrome pixels showed their boundaries touching at both wide and small viewports.
The app-owned CTA now consumes `space.lg` (`1.5rem`, 24 CSS px at the tested base size) as its
block-start margin. No shared token, component API, new primitive or CMP consumer changed.
Existing panel/link semantics and focus styles are reused. This CSS-only repair uses the
foundation's inspected-visual regression exemption.

No registration API, credentials, provider/grant/TLS configuration, persistent account,
Studio behavior, payment, Membership entitlement or automatic guest import was added.

## Environment and reproducible commands

macOS, headed Google Chrome **155.0.8059.80**, official arm64 build; Leptos 0.7.8 CSR/WASM,
Rust **1.98.1**, Trunk **0.21.14**, debug development assets. Launch:

```sh
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask app cantos-theatre --web
```

This starts only the frontend at `http://127.0.0.1:8082`; the existing `/api/` proxy targets
8080. No API was listening there, and no `DATABASE_URL` or Theatre issuer/secret pair was
exported in this task's process environment. No credential file or old chat secret was used.
The initial sandboxed launch could not bind; the authorized unsandboxed launch succeeded.
The frontend-only warning and actual connection-refused/proxy-500 errors are preserved in
ignored `artifacts/agent-runs/issue191-web-mac/app.log`.

Live mode and the exact `?mode=auth-preview` diagnostic were tested separately. The diagnostic
uses production account/navigation state with explicit fixture A/B/cleanup responses; it mounts
no provider or HTTP account effects. An unknown combined mode query stayed live/unavailable;
it did not fall back to fixture success. Browser DevTools was used through native Mac UI only
to capture viewport PNGs, read provenance and seed the documented nonsecret callback-return
context. No token/cookie/password or private content was entered or exported.

## Verification ledger

| Layer / evidence | Result and actual scope |
| --- | --- |
| L1 architecture / independent source review | No actionable finding in the two-file implementation diff; navigation, state owner, browser helper and lifecycle callers reviewed |
| L2 types/state | Private pending enum preserves outbound vs return-confirmation meaning; Guest still requires successful account cleanup |
| L3 regression | **FAIL-before**, then **PASS**: 3 new production-observer/account tests; 12 existing auth-navigation and 7 account-view tests; affected native suite passed 191 unit tests and 3 doctests |
| L4 browser fixture | **PASS**, interaction-tested and accessibility-checked for the named scenarios below; no mocked HTTP or real-provider claim |
| L5 real API/provider | **NOT_RUN / BLOCKED**, no running API, configured issuer/client or supplied test account |
| Runtime build | **PASS**, actual Theatre debug WASM compilation and Trunk serve; no workspace/release/mobile/iOS build |
| Required preflight | **FAIL**, inherited repository skill-length failure; independently attempted format and native/WASM lint also fail on unchanged baseline inputs, as recorded below |

Focused DEV commands used the existing rust-tests gate primitive:

```sh
RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-theatre app::live_navigation::tests --locked
RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-theatre application::auth_navigation::tests --locked
RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-theatre application::account_view::tests --locked
```

The first regression failed behaviorally after extracting the unchanged observer for direct
testing. The repair then passed. It covers cleanup failure/success, exactly one new retry,
duplicate submission, outbound preservation, cancellation, old session rejection and current
session completion. Host tests directly call the production observer; browser fixture acceptance
below separately exercises mounted effects. Neither crosses real HTTP/database/provider IO.

Full outputs are in ignored `artifacts/agent-runs/issue191-web-mac/`, including
`auth-callback-{fail-before,pass-after}.log`, `auth-navigation-existing.log` and
`auth-account-view-existing.log`. The independent implementation review found no actionable
regression; screenshot inspection was performed separately by the implementing agent.

Preflight was attempted normally, then the already observed blockers were explicitly skipped
to reach the next independent gate, without changing their original results:

| Gate | Actual result / cause |
| --- | --- |
| `repository` | **FAIL**, unchanged `cantos-cmp-mobile/SKILL.md` has 307 lines, limit 300; typed static services validation **PASS** |
| `rust-format` | **FAIL**, unchanged Studio artwork preview/tests, Theatre `player_timeline.rs` and xtask distribution host formatting |
| `rust-lint (cantos-theatre)` | **FAIL**, unchanged `player_timeline.rs:314` test type triggers `clippy::type_complexity` |
| `rust-tests (cantos-theatre)` | **PASS**, 191 unit tests and 3 doctests |
| `web-lint (cantos-theatre)` | **FAIL**, the same unchanged `player_timeline.rs:314` test type on WASM |
| `web-build` inside stopped preflight | **NOT_RUN** after lint failure; separate `cargo xtask build cantos-theatre --web` **PASS**, debug only |
| `theatre-cmp-demo` | **NOT_RUN**, explicit skip; no mobile build/test requested in this Web turn |

All five failing input paths were compared to `origin/develop` and are unchanged. They are
actual baseline failures, not missing-tool results or newly repaired scope. No lint suppression,
test expectation relaxation or unrelated formatting repair was applied. Retain the original
failed summaries in `preflight.log`, `preflight-available.log`, `preflight-web-layers.log` and
`preflight-tests-web.log`; skipped follow-up rows do not turn these gates into PASS. Focused
`rustfmt --edition 2021 --check` on changed `live_navigation.rs` and `git diff --check` passed.

Reproduction (all commands prefixed with `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp`):

```sh
cargo xtask preflight --changed-only --skip theatre-cmp-demo
cargo xtask preflight --changed-only --skip repository --skip theatre-cmp-demo
cargo xtask preflight --changed-only --skip repository --skip rust-format --skip theatre-cmp-demo
cargo xtask preflight --changed-only --skip repository --skip rust-format --skip rust-lint --skip theatre-cmp-demo
cargo xtask build cantos-theatre --web
```

## Browser observations

| Scenario | Observed result |
| --- | --- |
| Live API unavailable | Public Home remains navigable with catalog error/retry; Login shows session error, disabled auth CTAs and refresh; Account does not invent signed-in data |
| Registration route | Disabled Create Account has a visible reason; no email/password fields; Back and Home recovery remain available |
| Reconstructed registration-complete URL | Explicitly unconfirmed; no receipt, account success, plans or entitlement |
| Fixture guest Account → Login, repeated submit → explicit A response | Returned to Account only after fixture session confirmation; Signed-in label/logout rendered |
| Matching callback + Guest | Seeded one canonical nonsecret return intent, navigated/reloaded through `#auth-return`; returned to Login as Guest; retried Login + explicit A response returned to Account |
| Callback without stored/matching intent | Replaced callback with Home; did not establish a signed-in session |
| Logout, cleanup failure, retry and acknowledgement | Private/signed-in projection hidden immediately; login unavailable until acknowledgement, then guest actions restored |
| Held A, cancel + Escape, explicit B, deliver old A | Fixture reported old A ignored; signed-in projection retained |
| Reload fixture signed-in Account | Returned to Guest; preview creates no durable session |
| Back / Forward / Escape | Internal return destinations reconstructed; Escape returned Home/Account; browser Back restored the triggering Home Create Account button focus; Forward reopened Login |
| Keyboard Library entry | Tab exposed skip link with visible focus; Enter focused main; Tab focused separated Account entry; Enter opened Account and focused main |
| Text/reflow | Vietnamese labels/reasons wrap; 320 CSS px and 200% Chrome zoom inspected, recovery controls reachable by scroll, no clipped action or horizontal overflow observed |

Fixture controls and results are diagnostic, not a registration receipt or an actual authenticated
Library read. Provider form validation, provider cancel, real callback/exchange, session reload,
expiry/revocation, real logout refusal and verification email/duplicate registration remain unrun.
The fixture cancel control cancels generation; Escape was used separately to leave the auth route.

## Opened visual evidence

Ignored `artifacts/ui-evidence/issue191-web-mac/` contains original Chrome DevTools screenshots
and `images.json` with exact physical dimensions and SHA-256. All six PNGs were opened.
The before/after pairs use the same live Library state, vi-VN, light theme and 100% text size.
Wide **1280×720** and small **390×844** are CSS viewports in Chrome responsive emulation
(DPR2/mobile input and UA emulation, not a native mobile device). Actual Mac desktop input,
focus and 200% zoom were also inspected outside emulation. Additional **320×844** light fixture
and dark live captures show the separated CTA and wrapped Vietnamese text.

| Original image | SHA-256 |
| --- | --- |
| `library-before-desktop.png` | `2e86cfc3aa97933520a351373312d31d976a6bb59767a4bcfcd9206a68cdbccc` |
| `library-after-desktop.png` | `67fc713481ec0f178572e259ff9327a700f2ad035ac95cc6b56df8d0af703c35` |
| `library-before-small.png` | `7f08d6ad373331439401de15637aff74b15b9ce1306c23f72604d3e5981e2e9c` |
| `library-after-small.png` | `a025aad3917832e6e3eaf0cfe5eb23646ed85f38334c2f114c1e89b49eb2c7f3` |

Inspection: before, the panel and CTA touch; after, a 24 CSS px gap separates them. Medium
Theatre intensity, unchanged lavender hierarchy, readable label and focus visibility retained.
Light/dark reflow is screenshot-inspected and cross-viewport-inspected for these named states.
No screenshot comparison metric, screen-reader, motion/reduced-motion, OS IME, performance,
real audio or device acceptance is claimed. No new localized text was introduced.

## Remaining decisions and acceptance

Both [#174](https://github.com/loveoverflowcom/cantos/issues/174) and #191 were fetched with
their full comments on 16 October: neither had comments or a newer signup policy decision.
Signup is **BLOCKED / pending product decision** between issuer-hosted self-signup,
invitation/activation, or defer. The first two additionally need capability/config evidence from
the chosen real issuer. Existing OIDC login does not prove either signup capability.

Existing-account live login is **BLOCKED / pending environment**, independently of that policy:
an operator-prepared migrated local API, exact Theatre BFF issuer/client/callback, securely supplied
server secret and existing test account are required. Then run guest → issuer → callback → API
session → authorized Library read → reload/logout/refusal, including error/cancel/expiry cases.
Use supported secure handoff for human sign-in; do not paste credentials into chat or provision
new accounts/grants/TLS as an acceptance shortcut. Acceptance owner: unassigned.

No denied action or uncertain remote mutation occurred in this Web slice. Keep PR Draft;
no merge, issue closure or GitHub CI inspection. Stash `cb8f104`, 11 auxiliary worktrees and
earlier evidence remain untouched. Baseline hygiene/format failures are not repaired here.

## Account policy and server admission follow-up

**Review boundary:** implement the selected audience/admission policy and truthful Theatre Web
capability handling on the same Draft PR #196, starting from clean local/remote
`0d481097524b21a6fd9b47c3dd155fe4c1908b3c`. Remote `develop` was read-only verified at
`159f57ce66948fa5e42f47a3aec98bbc4857209b` only for branch comparison; the existing branch was
preserved without rebase/reset.

### Accepted decision and actual implementation

[ADR 0034](../decisions/0034-theatre-signup-and-provisioned-studio.md) accepts listener
self-registration and only one or two securely provisioned Studio administrators. The earlier
self-signup/invitation/defer choice is settled. Pinned Kanidm **1.10.5** source has no accepted
public signup operation: `/ui/enrol` handles an existing authenticated account's credential
enrollment, while person creation needs authenticated management authority. The source audit
and exact links are in the ADR and [contract](../../services/cantos-server/contracts/theatre/identity/v1/README.md).
No live issuer settings were inspected or changed; this limitation is not live-provider evidence.

Implemented invariant / owner:

- **Studio admission:** private pure roster accepts exactly one or two distinct exact subjects
  under the Studio issuer. The development host requires `CANTOS_STUDIO_ADMIN_SUBJECTS` with
  the existing issuer/secret. Callback refuses unlisted signed identities before registry and
  session issuance; private requests recheck the stored issuer/subject. Applying a roster change
  requires host restart. Existing ownership/action permissions and separate Admin capability
  checks remain. New tests cover Theatre client/issuer refusal, forged groups/email, count and
  duplicate boundaries, and default deny. No account or credential is hardcoded.
- **Theatre capability:** public read-only `GET /api/v1/theatre/auth/configuration` has strict
  schema/DTO/fixtures and no account, database or provider effect. It distinguishes
  `identity_unconfigured` from `provider_unsupported`; its enum has no supported/signup-success
  variant. Native-only configuration does not imply a Web login capability.
- **Web:** a separate pure state owns capability loading, exact failure, duplicate retry and
  stale response refusal. A bounded same-origin JSON adapter supplies server facts. Registration
  shows the typed unavailable reason or explicit retry; it never collects credentials or creates
  a receipt. The existing Button variants, bilingual copy owner and spacing tokens are reused.
  A persistent polite live region announces capability result changes. Account/session and
  registration facts do not authorize each other or merge guest data.

No schema migration, credential-format change, provider switch, live provisioning, membership,
audio, shared tokens/components or mobile implementation changed. This is a partial delivery of
#174/#191: actual public registration remains blocked by issuer capability.

### Verification and independent evidence

All commands ran locally on Rust 1.98.1. Logs are ignored under
`artifacts/agent-runs/issue191-signup-mac/` and `artifacts/agent-runs/studio-admin-admission/`.
The four contract and three router test results are retained in this task's tool outputs only;
no local log file is claimed for those two focused invocations.

| Layer / exact scope | Result / evidence |
| --- | --- |
| L1/L2 admission and purpose boundaries | Source reviewed independently against ADR 0034; exact issuer+subject policy is separate from verification, SQL issuance and resource rights |
| L3 Studio pure/signed mock HTTP | **PASS**, 12 tests: `RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-api --lib platform::studio_oidc::tests --locked`; real local synthetic HTTP issuer, no live accounts. First sandbox run had 4 loopback bind failures; the same authorized escalated command passed unchanged |
| L3 configuration contract | **PASS**, 4 tests: `RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-contracts --locked --lib theatre_identity::tests`; literal fixtures, schema and strict unknown/credential rejection |
| L3 Theatre application/presentation | **PASS**, 195 unit tests + 3 doctests: `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo test -p cantos-theatre --locked`; includes retry/stale/protocol partitions and locale × registration-state semantics |
| L4 actual Theatre router, lazy database/issuer | **PASS**, 3 tests: `RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-api --locked --test theatre_registration_routes`; no database/provider connection, cookies or signup/password handler |
| Web compilation | **PASS**, `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask build cantos-theatre --web`, debug CSR/WASM; repeated once after the review fix |
| PostgreSQL test compilation | **PASS**, `RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-api --test studio_oidc_postgres --no-run --locked`; adds zero-issuance, consumed retry and roster-removal HTTP/SQL regressions; execution is **NOT_RUN** |
| Required PostgreSQL execution | **UNSUPPORTED**, `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask verify postgres`: missing trusted `ffmpeg`/`ffprobe` on PATH; prerequisite check stopped before any cluster was created |
| Affected native lint | **FAIL**, `RUSTUP_TOOLCHAIN=1.98.1 cargo clippy -p cantos-api -p cantos-contracts -p cantos-theatre --all-targets --locked -- -D warnings`: inherited unchanged `player_timeline.rs:314` type-complexity finding |
| Standard changed-only preflight | **UNSUPPORTED** before execution: missing `ffmpeg`/`ffprobe` and Gradle on PATH; printed `audio-inspection`, `postgres`, `theatre-cmp-demo` UNSUPPORTED and all other rows NOT_RUN. No mobile build ran |
| Available preflight attempt | **FAIL**, `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask preflight --changed-only --skip audio-inspection --skip theatre-cmp-demo --skip postgres`: repository fails on unchanged 307-line CMP skill; subsequent gates NOT_RUN, three explicit skips NOT_RUN |

Changed Rust files passed scoped rustfmt; whitespace passed. The previous four-file global
format failure and 307-line CMP skill are unchanged. Repository hygiene is attempted separately
when unavailable preflight prerequisites prevent it; its actual result remains in the run log.
No full workspace/release build or GitHub CI inspection was performed.

An independent read-only review covered the new code/docs, private routing, legacy paths,
developer-token gate, identity construction/issuance and Web async state. Its one P2 finding
(the async registration result lacked persistent live announcement semantics) was fixed and
re-read; no actionable source finding remained. Tests and Web build passed after that fix.
This does not certify screen-reader speech or live provider behavior.

### Mac browser observations

Continued the existing task-owned Trunk host on `127.0.0.1:8082`, previously launched through
`cargo xtask app cantos-theatre --web`; current debug build passed above. Headed Mac Chrome
155.0.8059.80, Vietnamese, actual native pointer/keyboard, explicit fixtures separately labeled.

| Scenario | Observed result |
| --- | --- |
| Live Register without API | Create Account disabled, service-check error and Check again visible; retry returns the error with no fake account or fallback |
| Exact auth-preview Register | Provider-unsupported reason shown; no password/input form or success; contextual Back returns Login |
| Browser Back/Forward + Escape | Back restored the prior live Register document; Forward restored fixture Login, Escape restored its Account origin; no authenticated state appeared |
| Synthetic A + normal Logout | Signed-in presentation appears only after explicit fixture selection; Logout hides private state and waits for cleanup |
| Failed cleanup / retry / acknowledgement | Login remains unavailable after failure; retry returns to pending; acknowledgement permits Guest and reload stays Guest |
| Text/focus/layout | Wide light/dark inspected; 390×844 light error and 320×844 dark unsupported reason wrap; scrolling reaches Back/Home; actual Chrome 200% zoom shows readable controls and a visible keyboard focus ring on Back |

Both ignored original PNGs were opened: `artifacts/ui-evidence/issue191-signup-mac/` with
`images.json` dimensions/SHA-256 inventory. The 390px image predates the nonvisual live-region
wrapper; final 320px image and subsequent 200% focus inspection include it. Responsive emulation
uses DPR2, not a native mobile device. No visual comparator metric is claimed. Observations are
interaction-tested, accessibility-checked for those keyboard paths, screenshot-inspected and
cross-viewport/theme-inspected. They are not live identity/API/Studio browser or screen-reader
acceptance. Performance, cache, motion and OS IME were not measured; no relevant animation changed.

### Deferred acceptance, blockers and outward actions

1. **Signup BLOCKED:** owner must approve an audited enrollment-capable issuer integration and
   its real verification/return semantics. Self-registration policy is accepted; no request to
   choose invitation/defer is outstanding. No speculative signup URL or privileged provisioning
   bridge was added.
2. **Live login and Studio provisioning BLOCKED:** require the exact environment, issuer/client,
   existing authorized test identity, and explicit approval of any new account/credential/key or
   standing grant. The secure provisioning runbook is in `services/kanidm/README.md`; do not send
   passwords in chat. No such live operation has been attempted.
3. **SQL acceptance NOT_RUN:** make trusted media prerequisites available, then run
   `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask verify postgres`. Required evidence:
   unlisted callback creates zero actor/binding/session/receipt/cookie; retry never re-exchanges;
   roster removal denies existing sessions on current/legacy private routes while history remains.
4. **Runtime acceptance NOT_RUN:** operator-prepared API and issuer, then Web guest → existing
   account provider → callback → confirmed session/Library → reload/logout/refusal, and separate
   denied Theatre/Studio identity checks. Real registration needs its enrollment adapter first.
   Native/iOS and VoiceOver remain separate. Acceptance owner is unassigned.

The existing Draft PR stays open; no merge, issue closure, new PR, live identity/credential/grant,
provider settings or history rewrite. Stash `cb8f104ec2285a36fecab85388623cd09c112950` and all
11 auxiliary worktrees remain. No remaining approval denial or uncertain remote mutation is
being treated as a successful identity operation.

## Email registration bridge follow-up

This follow-up supersedes the earlier statement that no onboarding bridge was implemented.
Direction A in ADR0034 is approved for code and isolated tests: retain Kanidm, compose an
email-verification onboarding module in the existing monolith, and keep live activation off.
The earlier login/Studio roster and Library spacing evidence remains attributable to its
recorded inputs; this section adds no mobile, live identity or inbox acceptance.

### Implemented boundary

- Anonymous Theatre routes start email verification, explicitly consume proof, or cancel an
  unconsumed proof. They never issue a session, choose an existing person, collect a password,
  add a group, admit Studio or grant Admin capability. Exact Origin, strict JSON/2KiB bodies,
  direct socket peer and bounded attempts/concurrency guard the transport. Forwarded headers
  cannot select a quota identity. Public start/cancel responses are generic 202 receipts.
- Random purpose-bound proof hashes, exact issuer namespace, DB-clock expiry and atomic
  claim/CAS phases fence creation, setup intent and setup-mail delivery before external I/O.
  Unknown create/setup/send outcomes require operator reconciliation; public retry cannot
  reset a discovered account or repeat a potentially accepted effect. Verification-mail
  resend after cooldown rotates proof. No transaction contains issuer/SMTP I/O.
- Additive migration0027 preserves prior migration bytes and records immutable mail-budget
  receipts. PII/quota records currently have no age-based cleanup; expiry is not retention.
  Process-local rolling attempt limits supplement durable global/peer mail budgets. Neither
  process-local limits nor direct-peer HMAC imply reviewed multi-host/proxy protection.
- The pinned Kanidm1.10.5 adapter sends one create for a preselected UUID/name/mail. Only a
  positive create acknowledgement constructs the matching created-person witness used for
  a single setup intent. It does not discover existing people or mutate group/client access.
  The built-in onboarding role can reset existing non-high-privilege people: application
  witnesses do not constrain a stolen bearer. Effective issuer protection remains unaccepted.
- Vendor-neutral SMTP uses pinned lettre0.11.23, required STARTTLS or implicit TLS, normal
  certificate validation, bounded deadlines and no automatic retry. Plaintext is restricted
  to unauthenticated literal-loopback development fixtures. Final DATA250 means relay
  acceptance, not inbox delivery. Setup email includes the exact generated nonsecret login;
  Kanidm does not use mail as a login alias. Raw proofs/setup links/passwords are not stored
  in the registration tables or returned by its public DTOs.
- Web captures and scrubs a fragment proof before application navigation/session/config I/O,
  retains it in RAM only and requires explicit confirmation. Leaving fences late results;
  ambiguous verification is not automatically retried. Shared email field/auth buttons,
  bilingual copy and existing spacing tokens are reused. Explicit auth-preview controls
  remain isolated synthetic data, never a live transport fallback.
- Composition requires every policy/transport input explicitly; absent configuration is off,
  partial/orphan inputs fail startup. Approval declarations are operator assertions, not
  effective permissions, mail readiness or production acceptance. The issuer and origin must
  match the mounted Theatre Web identity. No live account, service principal, bearer/key,
  standing grant, SMTP setting, provider security setting or credential format was changed.

### Checkpoint evidence

Mac Rust1.98.1; debug builds; Leptos0.7.8, Trunk0.21.14, headed Chrome155.0.8059.80 arm64.
Logs are ignored under `artifacts/agent-runs/issue174-email-enrollment/`.

| Layer | Status | Command / evidence | Scope |
| --- | --- | --- | --- |
| Core, adapters and mounted HTTP | PASS | `RUSTUP_TOOLCHAIN=1.98.1 cargo test --locked -p cantos-api --lib theatre_registration --no-fail-fast`; `core-adapters.log`, `.provenance.txt` | 52 tests: domain9/application13/Kanidm8/HTTP8/SMTP5/mail-config5/host-config3/runtime1. Isolated ports/loopback only; 10 SQL tests ignored. |
| Contract | PASS | `RUSTUP_TOOLCHAIN=1.98.1 cargo test --locked -p cantos-contracts --lib theatre_`; `contracts-final.log` | 8 tests; independent request/capability fixtures and schema, no caller-selected role/person/password, exact receipt semantics. Empty struct enum variant rejects extra fields that Serde's unit variant ignored. |
| Metadata router | PASS | `RUSTUP_TOOLCHAIN=1.98.1 cargo test --locked -p cantos-api --test theatre_registration_routes`; `api-route-capability.log` | 3 actual router cases, lazy DB; absence/native-only/OIDC without registration and cross-surface denial. |
| SQL targets compile | PASS | `cargo test --locked -p cantos-api --test catalog_postgres --test revisions_postgres --test private_artwork_storage_postgres --test theatre_identity_postgres --test studio_oidc_postgres --no-run`; `sql-target-compile.log` | Existing migration assertions updated to27; compilation is not SQL execution. New10 registration SQL tests compiled in the library test run. |
| API debug build | PASS | `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask build cantos-api`; `api-build-final.log` | Real development host composition, no service startup. |
| API lint | PASS | `RUSTUP_TOOLCHAIN=1.98.1 cargo clippy --locked -p cantos-api --lib --bins -- -D warnings`; `api-clippy.log` | Affected API library and binaries only. |

The first adapter test run could not bind sandbox loopback; the specifically authorized isolated
command was rerun with loopback access. A separate SMTP fixture assertion initially failed
because lettre encoded Vietnamese mail as MIME Base64; the oracle now decodes actual transfer
encoding before asserting link/login content. Neither correction changes a live service.
A Trunk build raced the existing task-owned watcher on `dist/.stage`; its failed asset-copy log
is retained as `web-build-stage-race.log`, and the same final source built after writes settled.

### Review and remaining acceptance

Independent source review covered durable phase/binding rules, actual routes, configuration,
provider/SMTP adapters and Web. It found and the implementation repaired paired browser
history events cancelling a newly scrubbed proof, and later history-scrub failure being hidden
by route cleanup. Error recovery scrubs then reloads, reinstating normal account lifecycle on
a fresh owner. Browser testing also identified a conditional field signal-lifetime failure;
its final disposition and renderer observations are recorded below.

Live activation remains BLOCKED. It needs an agreed external existing-email policy (current
adapter never looks up/reuses/resets a pre-existing identity), accepted naming/canonicalization,
TTL/cooldown/quota and retention/recovery policies, effective least-privilege target protection,
the exact authorized environment/issuer/client/service principal and SMTP plan, secure secret
handoff and specific approval for each live account/credential/key/grant/security change.
No mailbox, real login, Studio browser admission/refusal or account completion is claimed from
these fixture results. VoiceOver speech, injected browser-history failure, deployed timing/
proxy/rate behavior, real PostgreSQL semantics and native/iOS acceptance remain separate.

### Final Web checkpoint and observations

| Layer | Status | Evidence | Practical limit |
| --- | --- | --- | --- |
| Theatre host tests | PASS | `RUSTUP_TOOLCHAIN=1.98.1 cargo test --locked -p cantos-theatre`; `web-tests.log`: 211 unit +3 doctests | Includes exact proof grammar, epoch/single-submit fences, renderer copy, paired-event guard, persistent scrub-failure safety and navigation-close notification regression. Host rendering alone did not establish DOM lifecycle correctness. |
| Debug WASM build | PASS | `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask build cantos-theatre --web`; `web-build-final.log` | Final functional sources with temporary panic hook and console feature removed. No release build. |
| Native Web library lint | PASS | `RUSTUP_TOOLCHAIN=1.98.1 cargo clippy --locked -p cantos-theatre --lib -- -D warnings`; `web-clippy.log` | Library only; does not supersede the previously recorded unchanged test-target type-complexity failure in player_timeline.rs. |
| Chrome fixture interaction | PASS | Actual Mac pointer/keyboard via CUA, exact `?mode=auth-preview` | Synthetic results only; no API/issuer/SMTP account or session was created. |
| Viewport/theme and keyboard | PASS | Full-page PNGs from verified390×844 light and320×844 dark device viewports; actual browser200% zoom and keyboard Tab | Both PNGs opened/inspected; input→Submit focus ring visible at200%. Not VoiceOver or native/mobile evidence. |
| Real PostgreSQL / issuer / inbox / Studio identity | NOT_RUN | Deferred activation matrix above | Mock and browser-fixture passes do not accept these layers. |

Chrome observations against the repaired interaction:

- Empty email produces localized status/error; a synthetic invalid-email API response returns
  the editable field. Pending Submit/Confirm controls disable duplicate submission. Generic
  acceptance clears the email form and does not claim account creation or login.
- A real same-tab synthetic `#register-verify?token=...` fragment is replaced by
  `#register?from=home`; explicit confirmation stays visible through the paired history
  events. Actual Back clears proof, including a repeated sanitized route, and Forward does
  not restore the discarded proof. Reload returns to an empty request; Escape/Cancel leave
  the flow. No real token was used.
- Confirm→lost-response fixture displays uncertainty without a second Confirm control.
  Confirm→unavailable explains expired/used proof. Setup-email fixture reports relay
  acceptance and explicitly leaves inbox/account completion unconfirmed; returning to Login
  does not create a session. Earlier signed-in/Logout fixture evidence remains separate.
- The previously failing full chain was rerun: proof→Confirm→unavailable→NewRequest→email→
  Submit→accepted→Cancel→Create. It now opens an empty working form. The short accepted→
  Cancel→Create chain also passes, including after explicit reload of final assets without
  the temporary diagnostic hook. Settings/theme navigation remains responsive.
- The lifetime repair separates same-page reset from navigation leave. Leave clears private
  state and advances its fence without notifying the form being disposed; it cannot schedule
  a transient email-field remount inside the old route owner. Shared field props/callbacks
  have stable panel ownership. The initial hoist-only candidate failed browser acceptance;
  it was not treated as a pass. Final Chrome DevTools reports zero runtime errors and one
  existing preload-integrity warning, not a silent ignored panic.
- Real provider validation, duplicate external email behavior, SMTP/inbox delivery, password/
  passkey setup, authenticated session persistence/logout and denied Studio identity remain
  untested live. Browser history API failure has deterministic boundary/render tests and
  independent source review, but no injected CUA runtime-failure acceptance.

Ignored screenshot inventory `artifacts/ui-evidence/issue174-email-enrollment/inventory.json`
records device-pixel ratio2 full-page exports (not viewport-height crops):

| File | PNG pixels | SHA256 |
| --- | --- | --- |
| signup-390-light.png | 780×3492 | 6c7ba9821dc96ce92fcaca8c27dca19a900d6f736e39d253053917864f1b7f44 |
| signup-320-dark.png | 640×3992 | 5cc3d7f0e8b0e7774603fc0f45f8884074e89f5636bbc48cc7d1e89e24e84b89 |

The fixture controls occupy their own labelled region before the product form. At both widths,
Vietnamese field/help/action text wraps without horizontal clipping or button collision; existing
spacing tokens are retained. Full-page exports alone establish no interaction or motion result.

Handoff logs are `preflight.log` and `repository.log` under the same ignored run directory.
Required gates remain incomplete. The repository gate still finds the unchanged307-line CMP
skill (limit300). This change also adds pinned SMTP dependencies to Cargo.lock, which is a
Leptos qualification input: existing button/text-field/surface/badge/back-button/logo records
are now stale, and current/new consumer sites (including the signup composition) fail admission.
These are actual qualification failures, not inherited skill failures or proof of a visual
regression. The new UI stays unaccepted for activation. The bounded task reuses production
components and inspected geometry but does not renew the two-renderer qualification campaign;
Gradle is unavailable and the user explicitly excluded mobile/iOS redo. Do not edit hashes or
compatibility baselines to bypass this gate. Fresh qualified records remain required before
production adoption. A new HTTP test initially constructed the PostgreSQL driver config inside
the HTTP module; fixture construction was moved to its platform test-support owner and the
mounted tests rerun. Two new module-order format differences were corrected locally.
Global format/test-target lint findings and unavailable ffmpeg/ffprobe/Gradle prerequisites
remain separate. No required gate was suppressed,
no full workspace/release/mobile/iOS build or GitHub CI was requested, and fixture results do
not authorize activation. SQL requires the existing disposable-cluster xtask gate after its
trusted prerequisites are available; do not substitute an inherited database or fake binaries.

The existing PR was observed Ready during metadata reconciliation and was restored to Draft
as requested. It remains the same PR196/branch targeting develop. Preserve stash cb8f104 and
all11 auxiliary worktrees/history; packaging performs no reset, rebase, force push, merge or
issue closure. Final commit/remote identity is supplied by the PR handoff, rather than embedding
a self-referential commit SHA in this evidence file.
