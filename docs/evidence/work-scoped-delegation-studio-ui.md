# Work-scoped delegation — Studio Web owner UI evidence

Date: 15 October 2026. This record separates verified local UI behavior from deferred global
gates and platform acceptance. It extends the
[API implementation evidence](work-scoped-delegation-implementation.md) for
[ADR 0028](../decisions/0028-work-scoped-delegation.md) and the immutable
[delegation v1 contract](../../services/cantos-server/contracts/studio/delegation/v1/README.md).

## Scope, invariant and ownership

On a Work owned by the signed-in actor, Cantos Studio lists, issues and revokes delegation
grants through the existing owner API. A creating `201` may expose its token once; a replayed
`200` never recovers the secret. A token must not enter local storage, URLs or logs, and a late
response must not reveal a secret in another actor or Work context. If these invariants fail,
owners could lose the only copy of a credential, misunderstand an unresolved operation, or
see a credential belonging to a stale context.

The Catalog backend remains the authority for ownership, actor availability, action admission,
expiry and revocation. The client uses shared DTOs in
[`api/src/delegation.rs`](../../services/cantos-server/src/contract/studio/delegation.rs) and shared route
constants; it makes no API contract change. Studio uses only `/api/v1/studio/*` cookie routes,
never a delegate bearer to manage grants. Theatre and Studio Mobile are outside this change.

The pure client module owns request shaping, exact acknowledgement correlation, field-error
projection and immutable grant-list projection. The existing request state owns tickets,
context fencing and exact-operation retry. The Leptos shell owns fetch, abort, clipboard and
focus effects; presentation components render acknowledged facts.

The final presentation deliberately uses app-local native HTML inputs, select, checkboxes,
status regions and empty-state text, styled with the existing semantic CSS roles. It composes
the established `ActionButton`/`FormSubmit` adapters. The Work-specific revoke dialog is
composed inside Studio's existing `ui.rs` Dialog adapter site. Initial direct imports of
additional shared UI components failed the repository admission check because those adoption
sites had no qualifications. The implementation removed that unqualified adoption; it did not
change the shared component catalog, admission baselines, qualifications or reexports.

The delegation panel now has the workspace's stable lifetime. Selecting a Work or adopting
another actor resets fenced facts while native `hidden`/`inert` attributes control visibility;
the owner body stays mounted, and issuance also checks the owner/denial guard. This avoids
disposing its local signals while queued control rendering still refers to them.

| Claim | Failure mode | Oracle | Evidence held |
| --- | --- | --- | --- |
| Shared owner routes, DTOs and closed three-action set | contract drift or accidental broader access | route/body/decode fixture tests and source review | `type-enforced`, `example-tested` |
| `201` token and `200` null are distinct outcomes | an old secret is redisplayed or a replay looks newly issued | status/response admission, retry tests and dropped-after-commit browser journey | `example-tested`, `fault-injected`, `interaction-tested`, `integration-tested` |
| Actor, Work and request ticket fence completion | a stale response reveals another context's token | cancellation/context-change regression tests | `example-tested` |
| Show-once token has no readable Debug or persistence path | credential appears in diagnostics, storage or navigation | redaction tests and source review of all consumers | `example-tested`; source-reviewed, no heap-erasure claim |
| Requested `422` issues reach their fields | correction is hidden in a generic failure | issue mapping tests and real API refusal flows | `example-tested`, `interaction-tested`, `integration-tested` for observed cases below |
| Paged grants retain identity and acknowledged revocation | duplicate rows, skipped projection or false active state | page/projection tests and real 20+1 page walk | `example-tested`, `interaction-tested`, `integration-tested` |
| Copy and revoke work through the real browser and API | unusable copy control or false revocation acknowledgement | clipboard equality and issue/copy/revoke journey | `interaction-tested`, `integration-tested` |

## Provenance and local setup

| Field | Value |
| --- | --- |
| Source | `feat/issue113-delegation-studio-ui`, HEAD `545bd04b438350a8f78365230ceebaa57e664516` plus working changes; based on PR 136 API revision `082cd27` |
| Toolchain | Rust 1.98.1 (`RUSTUP_TOOLCHAIN=1.98.1`) |
| Renderer / build | Leptos CSR/WASM; final stable-lifetime/native-form development build passes UI-only preflight and the exact `verify web` command |
| Browser | Codex in-app browser and Google Chrome on macOS; precise engine/version strings not recorded |
| Backend | Real local Cantos API at `http://127.0.0.1:8093`, disposable PostgreSQL 17 fixture at loopback port 55432 |
| API executable | Reused task-local executable, SHA-256 `4353e0379cc4ddf1dacd9010c7f530643cda1deb200669da7940e27547ec94b1` |
| Fault lane | Loopback proxy at port 8094 in front of the same real API; creating response dropped after commit, then exact retry observed |
| Text-scale lane | Ignored local HTML test proxy at port 8095, serving the exact compiled UI/API with root font size 32 px; this is font scaling, not browser zoom |
| Synthetic identities | owner `mai`, member `nam`, registered delegate `agent-editor` |
| Main Work | `Ánh đèn cuối sân khấu`, ID `3a70f28e-69ca-44bb-9e4e-cbb4015e1552` |
| Data and secrets | Synthetic content only; no token value, session cookie, private manuscript or generated audio belongs in this record or Git |
| Artifacts | Ignored captures under `artifacts/ui-evidence/delegation-20261015/`, opened by the root agent; no token visible in the recorded images; gate logs listed below |

The fixture is a real PostgreSQL/API boundary, not an in-memory backend. The executable was
reused rather than claiming a new API build or API change. Live identity-provider acceptance,
production role provisioning and deployment are outside this local UI acceptance.

## Commands and gate ledger

The focused core run used this primitive command; it is not a new repository entrypoint:

```sh
RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-studio --locked live::delegation::tests::
```

Result: **19 passed**. This is `example-tested` client core and rendering-fixture evidence,
not real-DOM or browser acceptance. Source review also checked command, browser, state,
workspace, panel and view consumers without changing the API contract.

The final stable-lifetime/native-form source then passed the complete Studio test suite:
**213 tests and 2 doctests** in UI-only preflight. The final source was rebuilt, and the
issue/copy/dismiss/revoke, lost-response retry, member/account-switch and visual scenarios
were rerun as recorded below. The focused shared-contract suite also passed:
`RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-contracts --locked`, **94 tests**, log
`/private/tmp/cantos-delegation-contract-tests.log`. This focused suite does not replace the
full contracts gate.

| Repository command | Observed status | Scope / remaining work |
| --- | --- | --- |
| `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify web` | `PASS` | Final-source `web-lint` for Studio/Theatre, both `web-build` steps, and `ui-preview` all `PASS`; log `/private/tmp/cantos-delegation-web-final-source.log` |
| `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify rust` | `FAIL` (`rust-tests`) | `rust-format` and workspace `rust-lint` `PASS`; workspace tests failed while linking unrelated `catalog_http_postgres`: `ld: write() failed, errno=28 (No space left on device)`. This is an environment failure, not a passing test suite; log `/private/tmp/cantos-delegation-verify-rust-isolated.log` |
| Repository gate in final UI-only preflight | `PASS` | 401 Python tests plus document, architecture and diff hygiene checks; initial unqualified component adoption failure resolved through app-local presentation, without changing admission policy |
| `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify contracts` | `UNSUPPORTED` | `audio-inspection` missing `ffmpeg`, `ffprobe`; `script-ir-reference`, `protobuf-reference`, `contract-tests`, `wav-reference` all `NOT_RUN`; log `/private/tmp/cantos-delegation-contracts.log` |
| `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask preflight --changed-only` (default base `origin/develop`) | `UNSUPPORTED` | Includes the PR 136 API changes: `audio-inspection` and `postgres` lack `ffmpeg`, `ffprobe`; every other selected step `NOT_RUN`; log `/private/tmp/cantos-delegation-preflight-base-develop.log` |
| `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask preflight --changed-only --base HEAD` | `PASS` | All selected steps `PASS`: repository, rust-format, Studio rust-lint/tests, Studio WASM web-lint/build; log `/private/tmp/cantos-delegation-preflight-ui-acceptance.log` |

The first Web attempt failed because the filesystem was full. The run recovered disk space by
removing only this task's rebuildable incremental cache, then restarted its disposable
PostgreSQL fixture. The isolated full Rust gate later hit the same disk-capacity limit while
linking a backend test. These environment failures remain recorded despite final passing UI
tests and Web gates; no full workspace Rust test acceptance is claimed.

The initial sandbox UI-only preflight also encountered process/socket permission errors and
macOS temporary-path failures. Its final rerun uses `TMPDIR=/private/tmp`, a Python 3.14 shim,
`CARGO_INCREMENTAL=0`, two build jobs and approved local fixture permissions. It invokes the
same `cargo xtask preflight --changed-only --base HEAD`; `HEAD` isolates this UI diff from the
inherited PR 136 API changes. The final rerun passed every selected step; the default
develop-base preflight retains its independent `UNSUPPORTED` result above.

| Layer | Status / evidence | Verified scope and limits |
| --- | --- | --- |
| L1 architecture | Repository gate `PASS`; source-reviewed, `documented`, `type-enforced` boundaries | Studio-only shared DTO/routes; backend remains authority; native app-local presentation and established adapters; no catalog/admission policy change, server dependency or provider call introduced |
| L2 domain/types | `example-tested` | Exact response correlation, show-once/replay distinction, closed actions, redacted token wrapper and pure field projection |
| L3 unit/static | Final UI-only preflight and exact Web gate `PASS`; focused contracts suite passed | 213 Studio tests + 2 doctests, 94 contract tests; full workspace Rust gate retains `FAIL` from disk exhaustion; no generated property or mutation campaign claimed |
| L4 browser adapters | `interaction-tested`, scoped `accessibility-checked` | Final real fetch, clipboard, dialog/focus and exact retry flows; owner/member switch passes without new panic; additional source-only focus edges listed below |
| L5 DB/API | `integration-tested`, `fault-injected` for named local flows | Real local API and PostgreSQL issue/refusal/revoke and dropped response/retry; real paginated 20+1 grant journey; no production identity or deployment acceptance |

## Browser flow observations

The following were driven against the real local API through the Codex in-app browser and
Chrome. The final source reran the main flow and lost-response recovery. Outcomes were
inspected without recording the returned credential value; the sanitized metadata log records
statuses and contains zero credential prefixes.

| Scenario | Observed result | Evidence |
| --- | --- | --- |
| Delegate equals owner | Final-source `422 self_delegation` appears at the delegate field with `aria-invalid` and first-field focus | `interaction-tested`, `integration-tested` |
| Delegate actor unavailable | Earlier-source `unavailable_actor` appeared at the delegate field with first-field focus; final pure mapping/render coverage passes | Earlier runtime `interaction-tested`, `integration-tested`; final `example-tested` |
| No actions selected | Earlier-source `empty` appeared at the action group with first-action focus; final pure mapping/render coverage passes | Earlier runtime `interaction-tested`, `integration-tested`; final `example-tested` |
| Issue to `agent-editor`, all three actions, 30-day option | Creating response exposes one token field; issuing another grant is disabled while that secret is visible | `interaction-tested`, `integration-tested` |
| Copy token | Clipboard contents equal the current token field (`true`); no credential value retained in the evidence | `interaction-tested` |
| Dismiss saved token | Token field disappears and focus returns to the issue heading | `interaction-tested` |
| Open revoke dialog, press Escape | Native dialog closes and restores focus to its opener | `interaction-tested` |
| Confirm revoke with Enter | Real revoke succeeds; any matching reveal disappears, focus moves to the grant row, and the row shows revoked | `interaction-tested`, `integration-tested` |
| Lose creating `201`, retry exact operation | Proxy 8094 records `dropped_after_commit`; UI becomes Ambiguous with focus on exact retry. Work selection, session refresh/logout and issue fieldset are all disabled (`true`). Enter retries the same operation; real `200` with `token: null` gives the unrecoverable heading focus and zero token fields | `fault-injected`, `interaction-tested`, `integration-tested` |
| Revoke the unrecoverable grant | Notice action receives real `204`, notice disappears, and focus moves to the grant row | `interaction-tested`, `integration-tested` |
| Load 20 grants then the remaining grant with `after` | Chrome shows 20 then 21 unique grant rows, removes Load more, and focuses the list heading | `interaction-tested`, `integration-tested` |
| Loaded owner Work, then switch `mai` → `nam` | Final Chrome session summary settles to `nam`, with zero new RuntimeError panics. Member owner-controls remain in the mounted inert/hidden body but are not visible or enabled; token field count is zero | `interaction-tested`, `DOM-tested`, `integration-tested` |
| English locale | Final-source title is “Work delegation” and field label is “Delegate actor ID”; no new panic | `DOM-tested`, `interaction-tested` |
| `invalid_expiry` and `duplicate_action` field issues | Pure mapping coverage; no real browser refusal observed yet | `example-tested` only |

## UI, accessibility and visual report

| Required field | Evidence / limitation |
| --- | --- |
| Screen / flow / consumers | Cantos Studio Web, Work content workspace, owner Delegation panel: list → issue → reveal/copy → dismiss/revoke |
| Intent and primary action | Give a registered collaborator bounded access to the selected Work; issuing a grant is the primary action, list/revoke are secondary tools, credential recovery warning is explicit |
| Surface intensity | Low–Medium, consistent with Studio trust/review controls; root agent inspected quiet form/list surfaces and explicit unrecoverable warning in the captures below |
| Tokens | Existing semantic surface, text, warning, focus, spacing, typography and reading-shape CSS roles; no shared token source or CMP mapping changed; no new contrast measurement claimed |
| Localization | Delegation resources supplied for vi-VN and English; final Vietnamese flow uses `Ánh đèn cuối sân khấu`; English title and delegate field label observed on final source without panic; no exhaustive English runtime campaign |
| Accessibility | Scoped `accessibility-checked`: final native field labels and invalid/error associations, form Tab order ending at issue submit, Enter/Escape, visible focus and deliberate outcome focus. Dialog Tab visits Close, Keep, Confirm; next Tab reaches browser focus (`body`), never a background control; Escape restores its opener. No full-application or screen-reader acceptance claimed |
| Viewports / reflow | Final captures opened at 320×800, 768×900 and 1280×900 CSS px. Scroll width equals client width: 305 px narrow, 753 px compact, 1265 px desktop; no page horizontal scroll observed |
| Themes | Final light/dark captures opened at 320 px; desktop dark and compact light/dark captures inspected |
| Text scaling | Final root-font 200% fixture (32 px) inspected at 768×900 after reload: choices and labels wrap, focus stays visible, submit remains reachable and no page horizontal scroll observed. Actual 200% browser zoom remains deferred |
| Visual | Images and their precise fixture conditions listed below; root agent opened every listed capture; no secret visible |
| Motion | Not observed; no new animated interaction, and no reduced-motion runtime acceptance claimed |
| Screen reader | `NOT_RUN`; no VoiceOver, NVDA or other reader acceptance claimed |
| Input methods | Keyboard and pointer in the observed flows; no named OS Vietnamese IME walkthrough |

### Opened visual artifacts

All paths are relative to ignored `artifacts/ui-evidence/delegation-20261015/`. Reviewer: root
agent. Locale: vi-VN. These are fresh final-source captures, all opened. The files remain
outside Git and contain no visible token.

| File | State | Viewport | Theme | Text scale |
| --- | --- | --- | --- | --- |
| `final-retry-1280-dark-vi-100.jpg` | Exact-operation replay; unrecoverable notice | 1280×900 | Dark | 100% |
| `final-retry-320-dark-vi-100.jpg` | Exact-operation replay; unrecoverable notice | 320×800 | Dark | 100% |
| `final-retry-320-light-vi-100.jpg` | Exact-operation replay; unrecoverable notice | 320×800 | Light | 100% |
| `final-form-320-light-vi-100.jpg` | Owner form | 320×800 | Light | 100% |
| `final-form-768-light-vi-100.jpg` | Owner form | 768×900 | Light | 100% |
| `final-form-768-dark-vi-font200.jpg` | Owner form | 768×900 | Dark | Root font 32 px (200% fixture) |

The text-scale fixture uses an ignored loopback HTML proxy with the same compiled app and real
API. It changes only the root font size for observation. It does not establish page zoom:
in-app browser Command-plus had no effect, native Chrome app access was denied, and browser
policy blocked the Chrome settings path. Those boundaries were respected; no access-policy
bypass was attempted. Actual 200% browser zoom remains a named acceptance gap.

### Findings and same-scenario recheck

Read-only focused source review found these issues; implementation now contains the minimal
repairs. Source correction alone does not establish browser acceptance of the repair.

| Finding | Repair present | Recheck status |
| --- | --- | --- |
| Delegation request activity did not reach shell session-operation guards | Workspace activity now includes unresolved delegation state | Dropped-after-commit journey observed Work/session refresh/logout/fieldset all disabled, then exact retry recovery |
| A retained keyed row could remain Active after its expiry crossed during refresh | Row key tracks expiry crossing against the refreshed clock | Source-corrected; same-row refresh after expiry not runtime-tested |
| Revoke from an unrecoverable notice could target a grant row absent from the current page | Focus falls back to the grant-list heading when the row is absent | Source-corrected; absent-from-page revoke focus edge not runtime-tested; ordinary notice revoke passes |
| Cancel removed its focused button without moving focus | Cancel moves focus to exact retry for an ambiguous write, otherwise status | Source-corrected; pending-read/write Cancel focus edges not runtime-tested; transport-loss retry focus passes |
| Server `403` could leave focus targeting hidden owner content | Persistent delegation heading is the denial focus target | Source-corrected; server-denial focus edge not runtime-tested; member hiding/disabling passes |
| Chrome owner-to-member switch panicked while queued visibility rendering read disposed `Grants`, followed by `Signal<ButtonState>` panics | Panel mounts outside selected-Work Show at workspace lifetime; persistent owner body uses native hidden/inert, and issue dispatch checks ownership/denial | Final same Chrome loaded-Work owner → member reproduction passes: session settles to `nam`, controls invisible/disabled, zero token fields and zero new RuntimeError panics |
| New shared UI import sites lacked admission qualifications | Native app-local presentation, established action/submit adapters and Work-specific dialog composition in the existing `ui.rs` adapter | Final repository/preflight `PASS`; admission policy and qualification baselines unchanged |

## Web-specific acceptance boundaries

- **Surface / permission context:** Studio cookie owner routes only; content workspace and
  delegation adapter/state/view touched. Membership is not inferred as grant management power.
- **Rendering mode / build:** Leptos CSR/WASM development build; final UI-only preflight and
  exact `cargo xtask verify web` passed. Browser evidence reuses the task-local API executable
  and fixture identified above; no new API build is claimed.
- **Browser / engine:** Codex in-app browser and Google Chrome, macOS; precise versions not
  recorded. Evidence does not establish Firefox, Safari or other browsers by inference.
- **Backend for evidence:** Named real local API and disposable PostgreSQL fixture; fault proxy
  retains the real service boundary. Proxy 8094 drops an issuance acknowledgement; proxy 8095
  supplies the root-font-size observation fixture, not a fake API or browser-zoom setting.
- **Async races covered:** Pure tests cover stale ticket/context completion, duplicate-submit
  exclusion and immutable exact retry. Real final lost-`201`, guarded-session and account-switch
  checks passed. The explicit Cancel focus edge and retry-exhaustion campaigns were not run.
- **Media path:** None; no audio, storage/CDN, provider or production publication flow involved.
- **Style review:** Semantic token use and focus styles reviewed in source; rendered hierarchy,
  both themes, viewports and the root-font-size fixture inspected as listed above. Actual page
  zoom and reduced motion remain unobserved.

## Deferred acceptance and residual risk

Local UI gates and the named final-source browser flows pass, including the repaired account
switch; no unresolved feature bug is identified by this run. Deferred global gates remain
explicit: full workspace Rust tests retain `FAIL` from disk exhaustion, and full contracts plus
the default develop-base preflight retain `UNSUPPORTED` because `ffmpeg`/`ffprobe` are missing.
The passing UI-only preflight and focused contracts suite do not replace those results.

Actual 200% browser page zoom is deferred; the inspected root-font 200% fixture is separate
text-scaling evidence. Explicit Cancel focus, server-`403` focus, refresh across expiry and
revocation when its row is outside the current page are source-corrected but not runtime-tested.
Precise browser engine/version identification, screen-reader walking, reduced motion and
other-browser acceptance remain unobserved. These are bounded residual gaps, not claims of
passed platform coverage.

Production identity/role provisioning, deployment, actor lookup UI, grant last-used/audit/rate
limits and additional delegated actions remain outside this change. No contract gap was found
in the reviewed first-slice owner flows. Tokens remain session-memory values; this record does
not claim JavaScript/Rust heap zeroization or erase an owner's intentional clipboard copy.
