# Theatre Membership preview — issue 128

## Current CMP design follow-up

[PR137's token/component integration](pr137-cmp-design-v2.md) supersedes the earlier
app-owned CMP styling described below. It records external field labels, scoped token v0.5.0,
six existing shared API admissions, inspected actual renderer captures saved on Desktop and
the separate Android build/IDE/device boundaries. The Membership core and unavailable/fixture
semantics remain unchanged; the previous review's results retain their original revision scope.

## Current follow-up — PR 137 completion review

On 15 October 2026 a new owner request resumed review, repair and local verification of
[PR 137](https://github.com/loveoverflowcom/cantos/pull/137). Worktree
`/Users/manh.pd1/.codex/worktrees/pr137-completion/cantos`, branch `codex/pr137-completion`,
was created from fetched `develop@f05b09bf24243bb99bc869f5009628294ebb15d8` and fast-forwarded
through PR input `540ebe44589bf4ab3407b51b9b3e8ff0c09b52a7`. The owner checkout was preserved.
The earlier acceptance deferral below is historical; it does not constrain this resumed task.
This remains the issue-permitted preview slice, not production Membership or issue 128 closure.

Invariant/owner: the Web plan control must project the same `PlansStatus` as the displayed
content after Back/reopening; receipt, account completion and paid access remain independent.
A fresh read-only source review covered all 32 input PR paths, both membership cores, the
hand-authored corpus, mounting/mode boundaries, presentation/tests and owning docs. It found one
P2 confirmed presentation defect: selecting Failed, returning and reopening Plans retained the
failed core state but recreated the select on its first, unpublished option. The five options
now derive both their rendered selected attribute and DOM property from the existing state.
No core, production adapter, shared component, token or contract was changed. A focused second
source review found no further actionable issue in the follow-up code/test diff.

The new SSR regression calls production `Plans` after receipt → Plans → Back → Plans and uses
independent expected values for all five statuses. It failed before repair and passed afterward.
The CMP regression exercises Email IME Done twice through production `CantosDemoApp` and asserts
first-error Name focus, inline errors and no receipt. It is JVM semantics input, not native IME.

### Fresh evidence and independent layers

Logs and local-only images are ignored under `artifacts/agent-runs/pr137-completion/`.
`source-manifest.json` and `artifact-manifest.json` bind paths/bytes to SHA-256; final follow-up
code did not change after the passing checks. Documentation summaries were refreshed afterward.
Use `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0` and
`CARGO_TARGET_DIR=/Users/manh.pd1/Projects/Mine/cantos/target`. Existing Gradle 8.14.3 is at
`/Users/manh.pd1/.gradle/wrapper/dists/gradle-8.14.3-bin/cv11ve7ro1n3o1j4so8xd9n66/gradle-8.14.3/bin`;
existing JDK 17.0.19 is at
`/Users/manh.pd1/Library/Java/JavaVirtualMachines/ms-17.0.19/Contents/Home`.
Prefix `/opt/homebrew/bin` (Python 3.14.8), then their bin directories to `PATH`, and set
`JAVA_HOME` to that JDK. Installations, credentials, services and shared caches were not
provisioned or deleted.

| Layer / command or oracle | Actual follow-up result | Scope / log |
| --- | --- | --- |
| L1 ownership and L2 state barriers | Source-reviewed | Closed preview modes/origins, opaque attempts, exact callback fences; no authority or payment effect; unchanged cores |
| L3 selector regression: `cargo test --locked -p cantos-theatre membership_preview::tests::reopening_plans_keeps_the_fixture_selector_on_the_current_status` | FAIL before repair → PASS after repair | One selected test; `selector-red.log`, `selector-green.log`; SSR HTML, not live DOM |
| L3 `cargo xtask verify rust` | PASS | Format, workspace Clippy and workspace tests; `final-rust.log` includes the new history and selector regressions |
| L3/L4 `cargo xtask theatre-demo verify` | PASS | 272 tests, 39 XML reports, zero failures/errors/skips; new repeated IME Done test executed; `cmp.log` |
| L4 Web walkthrough: `cargo xtask run cantos-theatre --web` | Observed | Real Leptos CSR/Chrome DOM and keyboard/pointer interaction; no API; `web-runtime.log`, `web/` |
| `cargo xtask verify web` | PASS after disk recovery | Web lint, Studio/Theatre builds and shared Leptos preview; `web-retry.log`; original disk failure retained in `final-web.log` |
| `cargo xtask verify contracts` | UNSUPPORTED | Missing installed `ffmpeg`/`ffprobe`; four independent contract/reference gates NOT_RUN due prerequisite precheck; `final-contracts.log` |
| `cargo xtask preflight --changed-only` | PASS | All seven selected gates, no skips; repository includes 401 Python tests; `preflight-python314.log` and final `preflight-final.log` |
| L5 production/native | NOT_RUN | No Membership HTTP/SQL, live identity/signup/receipt, durable storage, payment, OS Back or screen-reader acceptance |

The first workspace run had `FAIL` at unchanged
`probe::tests::port_ownership_reflects_an_existing_listener` (port rebind assertion).
The xtask tree/dependencies equal develop. A single-thread exact rerun passed (1 test), and
the final full workspace gate passed. The cause remains unconfirmed, consistent with a
scheduling/environment race; no unrelated source fix was made. Both logs are retained as
`rust.log` and `probe-rerun.log`. The first Web run exhausted disk during the unrelated Studio
build; after free space returned, the unchanged-source rerun passed all four Web gates
(`web-retry.log`). No cache deletion was performed by this task. The first preflight failed
because PATH selected macOS
Python 3.9 (missing `tomllib`/`os.waitid`); `preflight.log` retains its five failures/48 errors.
Selecting existing Homebrew Python 3.14.8 resolved that prerequisite: all 401 Python tests and
all seven preflight gates passed in `preflight-python314.log`, with no script/test source change.
Contracts remain incomplete until trusted media tools are available. No failure was skipped
or rewritten as a successful gate.

The later standalone `cargo xtask verify repository` had FAIL at three unchanged
`test_render_compose_preview.CaptureTest` timeout/cleanup tests (3 failures/401 tests;
`repository.log`). These scripts/tests exactly match develop. Both the earlier preflight and
final preflight ran all 401 tests successfully; final `preflight-final.log` records every gate
PASS. Diagnosis is flaky unchanged subprocess timing, with startup/scheduling delay only a
hypothesis: the tests share a one-second prerequisite/render budget. Missing stop-marker alone
does not establish a leaked process, and temporary fixture cleanup removed the exact phase
metadata. No tests/assertions were weakened. Retain this intermittent infrastructure risk;
no further identical suite run was needed after the final PASS.

### Design, opened pixels and interaction observations

The issue 128 Drive handoff and complete Mobile HTML were read. Five actual reference PNGs
were downloaded and opened: Web form/plans and mobile form/received/plans. The Web HTML download
repeatedly ended with incomplete bytes; no Web HTML runtime claim is made. The reference
layout/flow is a design proposal; its fake timers, prefilled contacts and payment-detail
fixtures were not copied. References are retained in `design/` with PNG SHA-256 provenance.

The same Chrome scenario before/after the selector repair was opened at the actual
DOM-measured 1600×1125 CSS viewport, light, Vietnamese, same Failed fixture. The selected
option now agrees with the error status. All five plan statuses were independently exercised
through Back/reopen. Clean-tab browser Back traversed Plans → receipt → normalized old form
→ intro; Forward still traversed receipt entries → Plans with the received origin.
After session clear, old receipt entries normalized to intro without private contact/receipt.
Account-complete fixture → Plans → Back returned to account-complete without a receipt.
Pending prevented duplicate submission; unknown outcome locked both inputs and offered
same-intent retry. Blank click/Enter submission focused Name. A Tab tour traversed Back,
Name and Email with visible focus outlines.

Opened Web captures: `web/selector-{before,after}-1600-light.png`,
`form-invalid-light-narrow.png`, `receipt-dark-390.png`, `plans-dark-390.png` and
`form-english-light-320.png`. Actual narrow DOM viewports were 390×843 and 320×800 CSS px;
scroll width equaled viewport width, labels/diacritics/actions remained readable. Chrome
inherited 80% page zoom; viewport overrides were compensated using measured DOM dimensions.
Native desktop permission was unavailable, so real 200% browser zoom was NOT_RUN. No new motion
was introduced; reduced-motion recording and screen-reader walks were NOT_RUN. Only browser
extension diagnostics were observed in the checked console output, not app runtime errors.
Temporary viewport overrides, created tabs and the xtask-owned Web process were closed/stopped.

All twelve JVM Membership images were opened by the CMP reviewer. They include light/dark
390×844, English form and 320×800 at textScale 2. Receipt/plans top and scrolled-actions images
are separate: headings and email wrap; both receipt CTAs and Explore labels fit. In the plans
actions capture the Explore pill is flush with the bottom edge, while its full two-line label
remains inside the image; offscreen prior content is intentionally cropped after scrolling.
The parent additionally opened the English form and both large-text action captures. Image
files, scenario TXT and unmerged semantics are under `cmp/`; this is `screenshot-inspected` /
`cross-theme-inspected`, never Android/iOS evidence. Existing Kotlin formatter/lint tooling is
not configured; touched test style/imports were manually reviewed and compiler/tests ran.

Residual risk / next acceptance: rerun contracts with trusted `ffmpeg`/`ffprobe` on PATH.
The broad Web retry and changed-only preflight passed; the earlier failures stay recorded.
Intermittent unchanged infrastructure tests remain a risk. Native/platform and screen-reader
acceptance, real 200% browser zoom and production Membership policy remain
separate. No API, schema, pricing, entitlement, provider cost, playback, rights or approval
behavior changed. Issue 128 and shared admission/release-isolation dependencies remain open;
remote CI is not checked and no merge is requested.

## Historical handoff at 540ebe4 — acceptance deferred by owner

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
