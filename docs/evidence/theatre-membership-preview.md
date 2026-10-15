# Theatre Membership preview — issue 128

## Current handoff status — acceptance deferred by owner

On 15 October 2026 the owner explicitly requested that the review stop remaining acceptance,
document unchecked work, and push the scoped source for their own review. No further Chrome
attempt, CI polling or verification gate is required by that revised task. This does not
establish acceptance: the implementation-session PASS results below belong to input head
`c0cf32ec3ea1b0845a777929ca9949aeaecd252a`, before the follow-up repairs described at the end.
They are **not final-head results**. Independent review is partial; browser/native/live and
remaining final-head gates are NOT_RUN/deferred. The PR remains a draft and issue 128 stays open.

## Boundary and provenance

Base: `develop@f05b09bf24243bb99bc869f5009628294ebb15d8`; branch
`feat/issue128-membership-interest`; Mac mini checkout
`/Users/manhblue/projects/loveoverflow/cantos`. The initial clean splash branch and all other
worktrees/commits were preserved; develop was fetched and fast-forwarded before branching.
The final commit/head is supplied by the PR and handoff.

[Issue 128](https://github.com/loveoverflowcom/cantos/issues/128), its empty comments and
#127/#12/#34/#129/#130 dependencies were read. The [owning contract](../product/membership-interest.md)
separates account registration, interest receipt, plans and paid entitlement. This delivers the
issue-permitted **pure-state/preview slice**, not the production Membership capability.
No HTTP/SQL, signup/provider, payment/entitlement, native host or shared-component admission
change is included. PR132's source is merged; auth wrappers/Logo remain unqualified.
App-owned semantic/native Material controls reuse canonical tokens, Noto and local artwork.

The [Drive handoff](https://drive.google.com/file/d/1ETmdL9w1ywC10OqjPIrhV9ERy2l59tvJ/view)
and `auth-runtime.mjs` were read; Web plans/mobile form reference PNGs were opened. Large linked
[Web HTML](https://drive.google.com/file/d/17qt_BRSuG0iV3PBX0g4FG2uH9w4GOcw4/view) and
[Mobile HTML](https://drive.google.com/file/d/1dYf5Ww16naGGU0RGTluVoGhNg--Id0_b/view)
reads hit transport/DNS failures; no HTML runtime acceptance is inferred. Actual renderer
observations below use the implemented app, not the design HTML.

Raw logs, reference images, DOM/AX, JVM captures and cleanup receipts remain ignored under
`artifacts/agent-runs/issue128/`. Final app-byte manifest: `source-manifest.json`, SHA256
`f66476b0d2d470bf9bc6f77ac3946eab87df348d79368cfa97feeee60cb4c152`.

## Invariant and independent layers

Invariant: only a matching confirmed fixture receipt reaches received; unknown outcomes,
navigation and session changes cannot manufacture confirmation or paid access.
Owner/boundary: pure Theatre client cores → controlled presentation shells; no IO adapter.

Validation accumulates field findings and binds trimmed contact to an opaque intent. Pending
prevents duplicate sends/edits. Unknown retry preserves operation/body with a new request fence;
old callbacks cannot settle it. Back keeps the session draft and fences pending results;
clear-session removes private projections without reusing request IDs. Plan data status is
independent of receipt and remembers a closed internal origin. Default submission is unavailable;
receipt/account-complete fixtures require explicit opt-in. Neither renderer projects real
identity into this diagnostic or sends a playback effect.

The hand-authored [shared corpus](../../apps/cantos-theatre/web/fixtures/membership-interest-v1.json)
has 12 validation vectors/7 transition traces, executed by Rust and Kotlin. No Membership
Quint model/observer exists; no model or conformance claim is introduced.

| Layer / claim | Status | Evidence / limits |
| --- | --- | --- |
| L1 ownership/dependency guards | PASS | `statically-checked`: repository architecture/admission/link checks; no new shared import/baseline exemption |
| L2–L3 Rust types/errors/core | PASS | `example-tested`: workspace gate plus 127 Theatre unit tests and 3 doc tests; exact refusals, replay/fences, shared vectors and 6 SSR/history/projection tests |
| L2–L3 Kotlin core/corpus | PASS | `example-tested`: common/JVM suite; 10 Membership core + 2 vector methods |
| L4 Web | PASS | `DOM-tested`, `interaction-tested`: actual Chrome trusted pointer/key/text input, AX, validation focus, unknown retry, receipt/Back/history/account fixture/reset/fallback; no API responses or runtime exceptions observed |
| L4 CMP | PASS | `semantics-tested`, `interaction-tested`: JVM Compose host, 5 Membership flows; full suite 270, failures 0/errors 0/skipped 0; not Android/iOS evidence |
| L5 Membership HTTP/SQL | NOT_RUN | No accepted production contract/persistence; authentication, durable idempotency and restart acceptance remain open |
| L5 native/provider | NOT_RUN | No device/OS Back/secure store/signup/payment or screen-reader acceptance |
| Independent review | NOT_RUN (completion deferred) | Partial independent source inspection and follow-up repairs are recorded below; the owner will review remaining acceptance |

## Executed commands

Use `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp`; builds used `CARGO_BUILD_JOBS=1
CARGO_INCREMENTAL=0`. Existing installed tools were discovered and reused, not installed:
Gradle 8.14.3 at `/private/tmp/cantos-gradle-8.14.3/gradle-8.14.3/bin`, and local ffmpeg/ffprobe
at `/private/tmp/cantos-audio-tools`. Prefix those directories to `PATH`. Ordinary sandbox
process-group/native-cache failures were diagnosed; authorized normal-access runs resolved them.

| Command | Implementation-session result, before review repairs | Raw output |
| --- | --- | --- |
| `cargo xtask verify repository` | PASS; 401 Python tests | `repository-authorized.log` |
| `cargo xtask verify rust` | PASS; format, workspace lint/tests | `verify-rust.log` |
| `cargo xtask verify contracts` | PASS; all five reference/contract/audio gates | `verify-contracts.log` |
| `cargo xtask verify web` | PASS; both app lint/builds and UI preview build | `web-after-cleanup.log` |
| `cargo xtask theatre-demo verify` | PASS; 270 tests, zero failures/errors/skips | `cmp-fixed.log`; final preflight reran all tests and the seven capture scenarios |
| `cargo xtask preflight --changed-only` | PASS; all 7 selected gates, no skips | `preflight-final.log` |
| Chrome loopback observation | PASS | `web-observe-second.log`, `web/interactions.jsonl`, DOM/AX JSON |
| PostgreSQL, behavior, native, shared CMP admission | NOT_RUN | No persistence/model/shared-wrapper changes; native/production acceptance separate |

Retained failed attempts: sandbox repository/preflight had `os.killpg` PermissionError (the
correct `TMPDIR` run had 1 failure/37 errors); an unqualified temporary-path run also exposed
`/var` versus `/private/var` assumptions. Native-cache sandbox probing initially hid the installed
Gradle. Rust lint caught a missing explicit dynamic `<option value=value>` binding, now repaired.
CMP's first 270 run had one failing Escape test because the reopened input had no requested
focus; the fixed test requests/asserts focus and keeps the return-to-Intro assertion.
These failures are historical, not hidden or promoted to acceptance.

## Opened pixels and interaction limits

Surface: Membership introduction/form/receipt/plans, Medium-Low intensity, Web and JVM CMP.
Token source v0.4.0 is unchanged; both consumers reuse existing semantic roles, local fonts/artwork.
Vietnamese/English resources are exhaustive; runtime visual observations below are Vietnamese.

| Renderer | Opened captures / observations |
| --- | --- |
| Chrome headless, Leptos CSR | `web/web-unavailable-light-390.png`, `web-invalid-focus-light-390.png`, `web-received-light-390.png`, `web-plans-dark-1280.png`, `web-form-dark-text2-320.png`; 390×844/1280×900/320×800, no horizontal overflow; input/error names and first-field focus observed |
| JVM Desktop Compose | `cmp/membership-jvm-{light-intro,light-form,dark-form,dark-receipt,light-plans-failed}-390x844.png`, `membership-jvm-light-form-text2-320x800.png`; semantics/Back/reset and fixture flows tested separately |
| Same-scenario fix | Before 320 dp/2× header consumed about 400 px; shortened app header to Cantos while retaining preview notice/pane title. Reopened same 320×800/2× capture; header fits and form remains scrollable. Original retained in `cmp-before-header-fix/` |
| Current receipt | Exact PNG hash `820ac418a1fde8cb4d1adce026cbdee7d4f75c5dc742267154dd6a3c0b9147d2` reopened; header and receipt heading do not overlap |

Web text200% is an explicit CSS-root/rem reflow probe; browser zoom remains NOT_RUN. Reduced
motion was emulated; animation recording/profile and native IME were not observed. Keyboard,
labels/AX and focus checks cover the named flows; TalkBack/VoiceOver were not walked. Captures
prove neither shared-component admission nor native runtime. The seventh `cmp/membership-jvm-light-plans-unpublished-390x844.png` capture was opened after
final preflight: both period cards show unpublished terms, with no purchase/details control.

## Authorized cleanup and residual acceptance

Initial free disk was about 1.2 GiB, falling to 372 MiB during the first Cargo lane. All builds were
held until direct new Mac cleanup approval. Exact idle, nonsymlink caches were checked with
process/handle observations before deletion. Removed `target/debug/incremental` 664,472 KiB,
`~/.gradle/caches/8.14.3/transforms` 1,362,764 KiB, then 24 enumerated Gradle/CMP cache/intermediate
directories 1,238,108 KiB. Raw receipts: `cleanup-approved-exact.json`, `cleanup-expanded-caches.json`.
Free space afterward was 3,698,092 KiB (about 3.5 GiB); lanes ran sequentially afterward.
Reports/test-results/outputs, dependency reuse cache, keystores, credentials, SDK/toolchains,
app/private data and evidence were preserved. No live S3, secrets, infrastructure startup,
stopped acceptance restart, PostgreSQL baseline repair or remote CI polling occurred.

All 10 secondary worktrees were retained: every HEAD was remote/develop-preserved and tracked
state clean, but each had protected ignored evidence/reports/cluster records or unresolved
artifacts. Some nested targets are symlinks into other worktrees. No force removal, branch
deletion or prune occurred. Exact retained paths/reasons: `worktree-cleanup-audit.md`.

Reviewer entry: `cargo xtask run cantos-theatre --web`, then
`http://127.0.0.1:8082/?mode=membership-preview`; preview controls start a blank fixture session.
CMP: `CANTOS_MEMBERSHIP_PREVIEW_FIXTURE=1 cargo xtask theatre-demo run`, then Account → Membership
preview. Android/iOS hosts are unchanged/unavailable-default. Check all Back origins, stale
callbacks/session clear, unpublished/loading/error plans, long text and separate receipt authority.

Residual acceptance: #12/#34 still resolve submitter/contact/authority/retention/receipt/plan
policy before reviewed HTTP/SQL work; #129/#130 own live auth return; #127 owns shared admission;
#122 owns mobile release isolation. Native OS/system browser/secure store/screen reader,
production receipt durability, backend authorization, pricing and paid access remain unaccepted.
No issue closure or merge is authorized by this preview.

Final post-preflight free space: 2,282,048 KiB (about 2.18 GiB). All owned browser/server/build
processes finished or were stopped through their task-owned supervisor; evidence is retained.

## Independent follow-up — source repairs, not completed acceptance

Input scope: three-dot diff `f05b09bf24243bb99bc869f5009628294ebb15d8...c0cf32ec3ea1b0845a777929ca9949aeaecd252a`,
branch `feat/issue128-membership-interest`, initially clean with upstream 0/0. The reviewer read
issue 128 and its empty comments, dependencies 12/34/127/129/130, the Drive handoff, the
Membership contract, Rust/Kotlin state and renderer paths, navigation integration and relevant
tests. Source inspection supports the issue-permitted preview scope; it does not accept a
production contract or complete every independent review pass. Design HTML and final runtime
comparison were not independently accepted.

Two bounded findings were repaired under the owner's explicit authorization:

| Finding | Trigger and consequence | Owning repair / evidence |
| --- | --- | --- |
| P2 · confirmed defect · logic · confirmed-from-source | In Web, browser Back after a receipt visits the completed form hash. Restoring that hash rejects reopening the form, then `set_hash` pushes another receipt entry. Repeated Back can revisit the same form and lose forward history. Clearing a session while old receipt entries remain has the same cause. | [`bind_history` / `restore_history`](../../apps/cantos-theatre/web/src/membership_preview.rs) replaces a normalized visited entry rather than pushing navigation. A pure regression covers completed-form/cleared-receipt normalization and the next valid destination. Source-confirmed; the new Rust test and browser history walkthrough are NOT_RUN. |
| P2 · confirmed defect · rule · confirmed-from-source | In CMP, submit blank fields, focus Email, and submit again without editing. Equal `fieldErrors` do not restart `LaunchedEffect`, leaving focus on Email instead of the first invalid Name field required by issue 128. | [`MembershipForm`](../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/features/membership/presentation/MembershipPreview.kt) keys the focus effect by each submit intent too; button and IME share the trigger. A new JVM Compose regression reproduced the failure before the repair. Native/IME runtime acceptance is NOT_RUN. |

These changes preserve the unavailable production default, operation/payload binding, receipt
fences and separate plan origin. No auth, payment, API/SQL, PostgreSQL baseline or live S3 change
was made. No tool was installed; no caches, other worktrees or Trash were deleted.

| Claim / owner / target | Required by | Evidence held | Disposition |
| --- | --- | --- | --- |
| Preview receipt is separate from identity/entitlement; both client cores | issue 128; Membership boundary | Source inspection of default/fixture constructors, exact attempt comparison, retries, session clear and internal return origins; prior vectors/tests are not final-head reruns | Source scope held; live authority/durability deferred to 12/34 |
| Web history normalization preserves traversal | issue 128 Back/lifecycle; Web renderer | Source-confirmed defect and minimal repair; new pure regression added, not executed | Final Rust/WASM and browser traversal acceptance deferred to owner |
| CMP invalid submit moves focus to first error | issue 128 inline validation; CMP renderer | New JVM regression failed before repair: 271 tests, 1 failure; the repaired suite passed 271 tests with zero failures/errors/skips | Held for the JVM regression; native accessibility remains deferred |
| Final layered gates and complete independent acceptance | Revised owner request | No remaining gates or browser/CI steps started after deferral | NOT_RUN; owner reviews before acceptance |

Reviewer logs stay ignored in `artifacts/agent-runs/issue128-review/`. `cmp-red.log` is the
pre-repair reproduction. The second `theatre-demo verify` was already running when acceptance
was deferred; `cmp-green.log` reports `theatre-demo: PASS`. Its 39 JUnit XML reports total
271 tests, zero failures/errors/skips, including the new repeated-submit regression. It ran
the repaired Kotlin source with the new regression; neither Kotlin file changed afterward.
No additional suite was started after deferral. This result is `semantics-tested` /
`interaction-tested` on the JVM Compose host, not native or screen-reader acceptance.
`cargo xtask preflight --changed-only --dry-run` printed seven planned gates but executed none.
Repository/Rust/contracts/Web/preflight final-head execution is NOT_RUN by owner request.

The loopback Web supervisor started successfully on 8082 from the input revision and was
stopped through its owned session. Computer Use could not complete Chrome access: the Mac was
locked and automatic unlock failed. The later owner instruction removed browser acceptance
from this task, so Chrome was not retried or bypassed through another automation route.
No final-head CI status was checked; CI is deferred by owner request.

Owner review checklist: run final repository/affected Rust/Web/preflight gates; exercise browser
Back/Forward through completed form and cleared receipt history; check the new Rust regression;
recheck repeated invalid-submit focus and IME on the intended native hosts; inspect final
light/dark/narrow/large-text layouts and screen-reader behavior. Production receipt/auth/API/SQL,
pricing and entitlement remain separate decisions rather than unfinished preview test results.

Residual risk: the Web repair is source-reviewed but unexecuted; independent visual/browser
acceptance and the complete final-head gate set are explicitly deferred. JVM evidence cannot
establish Android/iOS, OS Back, VoiceOver/TalkBack or live registration semantics. Delivery is
source for owner review, not accepted, ready or a completed production Membership capability.
