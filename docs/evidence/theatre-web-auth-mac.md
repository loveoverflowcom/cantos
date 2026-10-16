# Theatre Web auth and spacing — Mac acceptance, 16 October 2026

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
