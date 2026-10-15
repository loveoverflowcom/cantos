# PR 137 — CMP Membership design-v2 integration

Owner request, 15 October 2026: replace screen-local styling with declared tokens and reusable
UI, align the CMP Membership preview with the supplied Mobile design, add a commit to PR 137,
and save real rendered screenshots on this Mac's Desktop.

## Invariant, ownership and decomposition

The mounted CMP form, receipt and proposed-period cards consume the shared design-v2 roles
and controls while retaining the existing fail-closed Membership state machine. Visual changes
must not confirm an unknown request, discard a draft, purchase a plan or grant paid access.

| Responsibility | Before | After |
| --- | --- | --- |
| Membership transitions, opaque attempts and callback fences | `domain/membership/MembershipInterest.kt` | Unchanged |
| Mounted session state and event dispatch | `app/CantosDemoApp.kt` | Same owner/lifetime; supplies the header and content slots |
| Form/receipt/period presentation | One `MembershipPreview.kt` with private Material wrappers | `MembershipPreview.kt` binds events; `MembershipComponents.kt` composes declared shared variants; `MembershipCopy.kt` owns vi/en copy |
| Tonal field anatomy | Shared `AuthTonal` still used a native floating label | Both raw/rich overloads keep the external label above the input; the native edit node retains its accessible name, error, selection and IME contract |
| Visual values | Screen-local geometry and the general headline style | Canonical token v0.5.0, generated CSS/Rust/Kotlin, `CantosTypography` and existing auth/membership variants |

Imports remain app → feature presentation → shared UI/domain. No new controller, mutable store,
port, dependency, subscription or platform adapter was introduced. Name/email focus requesters,
the repeated-submit focus trigger, scrolling per Membership screen, operation identity and
callback ordering retain their owners. The header's theme action updates the existing preferences
owner; a new production-composition regression checks two toggles without losing draft/destination.
Other routes retain their existing header, density, shape and typography mappings.

L1/L2: ownership/source reviewed; no durable business-rule or fallible-I/O change. This is L4
presentation work over the already tested core. TYPE-04, FP-01/02, ARC-01, KT-API/STATE/LIFETIME
and VER-01/02/08 remain applicable; no new refinement or native-conformance claim is made.

## Token and component contract

Nine additive roles are declared in `packages/ui-system/shared/tokens.json` v0.5.0:
`type.authHeading.{size,lineHeight,weight}` = 32/1.22/400,
`type.membershipTitle.{size,lineHeight,weight}` = 28/1.4/400,
`size.authLogo` = 128, `size.statusSymbol` = 72 and `shape.statusSymbol` = 28 logical units.
They follow the supplied Mobile type/symbol anatomy. Existing colors, fonts, regular geometry,
breakpoints and contrast pairs are unchanged. CSS, Rust, Kotlin and both named-token catalogues
were regenerated through the existing primitives; generated outputs were not hand-edited.
The CMP adapter preserves the injected Noto Serif family and host font scale. Leptos preview
headings consume the same new type roles; the other Web headings retain their original mapping.

The six existing APIs used by Membership are `Button`, `TextField`, `Surface`, `Badge`,
`BackButton` and `Logo`. Both previews already declare real specimens/states for them. Their
fresh, source/token-bound JSON records reference this bounded review; the exact import baseline
is unchanged. This admits those primitives to the memory-only mock, not native listening,
live auth/Membership, screen-reader or release-isolation acceptance. The remaining 26 APIs
are not newly qualified. Historical catalog implementation statuses are not native approval.

| API | CMP renderer evidence | Browser evidence |
| --- | --- | --- |
| Button | `AuthAccountComponentTest`, `AuthActionMotionTest`: native role/name, blocked callbacks, focus/bounds, pressed/cancel/reduced-motion behavior | Actual auth-primary click produced count 1; Tab moved to auth-secondary with solid focus outline; loading disabled all three variants and retained count/reason |
| TextField | Raw/rich external-label regression, error/read-only/selection/IME tests; 2× field capture | Label bounds end above input; `aria-invalid=true`; help/error IDs linked; read-only remained enabled and retained the input echo |
| Surface | Auth/profile/membership/group specimens, foreground pairing tests and opened light/dark captures | Opened actual auth/profile/membership/group surfaces; 840 CSS px reflow, auth heading 32px, membership title 28px, regular section heading 20px |
| Badge | Text/tone specimens in the actual gallery and receipt; opened captures | Opened all five named status tones in light/dark; status remains textual |
| BackButton | One named focusable target, minimum bounds, host callback, blocked states | Click produced exactly one host intent; Tab moved to the next contextual action; light/dark specimens opened |
| Logo | `LogoRendererTest`: sole localized image name, official inks, aspect fit and transparent knockout | Exactly one visible standalone `img` named “Biểu tượng Cantos”; official light/dark/mono local vectors opened |

## Executed evidence

Base/input: `961a632735279ab92020e41a4db50918742b021f`, isolated worktree
`/Users/manh.pd1/.codex/worktrees/membership-cmp-refactor/cantos`.
Use `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0`,
`CARGO_TARGET_DIR=/Users/manh.pd1/Projects/Mine/cantos/target`, Homebrew Python 3.14.8,
Gradle 8.14.3 and the installed Microsoft JDK 17.0.19. Kotlin/CMP/Material3 are pinned
2.2.20/1.9.0/1.8.2; Android uses the existing SDK 35/AGP pins. No toolchain or service was installed.

| Command / observation | Result | Scope |
| --- | --- | --- |
| `cargo xtask ui-preview cmp verify`, label regression before repair | FAIL | 114 executed tests; the new label-outside-input assertion failed on the old floating label |
| Same command after repair | PASS | 114 tests, zero failures/errors/skips; shared production composables on JVM Desktop Compose |
| `cargo xtask ui-preview leptos verify` | PASS | 76 native tests, WASM lint and actual preview build |
| `cargo xtask theatre-demo verify` | PASS | Mounted mock common/JVM tests and screenshots; includes repeated IME, unknown retry, Back/session clearing and the new theme regression |
| `cargo xtask theatre-demo android-build` | PASS | Real mock debug APK compilation, including all seven new Android preview wrappers; not launched on a device |
| `cargo xtask ui-preview leptos run` + CUA walkthrough | DOM-tested / interaction-tested / accessibility-checked | Real Chrome controls and Codex browser pixels, actual 390×844 and 840×900 CSS viewports, light/dark; no horizontal overflow |
| `cargo xtask ui-preview cmp run` | Started/stopped; OS window not observed | xtask stopped its owned group. Java printed two `getApplicationProperty` diagnostics; no native UI acceptance is inferred |
| `cargo xtask compose-preview cantos-theatre --file apps/cantos-theatre/mobile/shared/src/androidMain/kotlin/com/cantos/theatre/app/MembershipPreviews.kt --composable TheatreMembershipFormLightPreview --label membership-form-light-390-vi --timeout-seconds 15` | UNSUPPORTED | `studio-check` exited 1; no connected ready IDE. CUA native access also refused permission |
| `cargo xtask verify contracts` | UNSUPPORTED | Missing ffmpeg/ffprobe; four independent contract gates NOT_RUN by precheck |

Full output remains in ignored `artifacts/agent-runs/pr137-cmp-refactor/`; the Compose CLI failed
bundle is under `artifacts/ui-evidence/compose-preview/cantos-theatre/20261015T032744Z-d1e1a3acc44d/`.
Repository/Rust/Web/preflight final summaries are recorded in the final handoff section below.
The first app recheck had one old header assertion expecting text after the intentional switch
to a named image. It now asserts the actual logo accessible name; the same 2× bounds test passed.
No failure was skipped or weakened into a passing native claim.

## Visual review and reproducibility

Codex opened the actual Mobile request/received/plans reference PNGs retained by the preceding
PR137 review, plus the relevant Mobile HTML layout/styles. References originate from
[issue 128's Mobile PNG handoff](https://drive.google.com/drive/folders/16prSfqzeFv2rIVluO6sPU9iHt1nCnEKS).
The official Logo Kit remains the existing local vector source. No manuscript/media was added to Git.

Same-scenario form before/after: `membership-jvm-light-form-390x844.png`, Vietnamese, light,
390×844 logical units, 1× host density/text, production `CantosDemoApp`, blank unavailable form.
Before: duplicate notices above the title, floating labels/underlines inside a large enclosing
card, generic text header. After: official logo/contextual Back/theme control, 32px editorial
heading, permanently visible labels, separate tonal fields and one full-width auth primary CTA.
The preview/unavailable notices remain explicit below the form. Receipt shows a decorative
success symbol, a named sample badge, its sample email and the two independent actions.
Monthly uses the paper surface; annual uses primary-container with its paired foreground.
Unpublished prices/details keep the buttons disabled with a visible reason; no fake checkout/modal.

Actual JVM image bytes, per-image semantics and scenario records are saved on the Desktop at
`/Users/manh.pd1/Desktop/Cantos-PR137-CMP-2026-10-15/`. This is real Compose renderer output,
not a prototype, generated illustration, OS-window screenshot or Android/iOS device capture.
The bundle contains 390×844 light/dark form/receipt/plans, English form, 320×800 at 2× text
with scrolled actions, and 390×1320 complete-period views. Images are opened before inspection
claims; the bundle manifest binds them to hashes, source inputs and the final commit.
Shared CMP auth, error/focus, gallery, logo and light/dark specimens were also opened.
Web images were opened inline through CUA; initial Chrome captures were partly black, so visual
review used the working Codex browser screenshot API. Browser pixels are not CMP evidence.

Surface intensity is Medium for this brief account/interest flow: editorial title and the primary
CTA lead, with quiet tonal grouping. Vietnamese/English copy lives in `MembershipCopy.kt`;
no price, entitlement or provider policy is encoded there. Accessibility evidence is JVM
semantics/input/focus and browser DOM/keyboard only. Shared auth motion has deterministic JVM
pressed/cancel/reduced-motion tests; the app keeps its existing reduced-motion preference.
No native motion recording, TalkBack/VoiceOver walk or actual 200% browser zoom was observed.

## Final handoff and residual risk

Final source checks: `cargo xtask verify repository` PASS; `cargo xtask verify rust` PASS
(workspace format, lint and tests); `cargo xtask preflight --changed-only` PASS on the
unchanged-source retry, all ten selected gates with no skips. Both Web builds/lint and the
shared Leptos preview passed. Final shared CMP results are 114 tests/18 reports; mounted mock
results are 273 tests/39 reports, all with zero failures/errors/skips. The final Android build
also PASSed after the last receipt type adjustment. Applicable touched/executed source sets:
`commonMain`, `commonTest`, `desktopMain`/`desktopTest`; `androidMain` compiled; iOS was not run.
No Kotlin formatter/linter is configured: touched code/imports were manually reviewed and the
pinned compiler/tests executed. No new persistence/HTTP/behavior-model gate is applicable.

The first full preflight exited 101 during Theatre Kotlin output: a generated class write
reported Permission denied, Gradle could not persist its error/cache cleanup, and xtask stdout
panicked with `No space left on device`. Disk exhaustion is confirmed; the exact OS mapping of
the earlier compiler write refusal is not separately established. This is an environment failure,
not a passing check. After stopping the failed run, its same directory was writable and an
fsynced temporary write probe passed. Cleanup removed 73 **new test executables named in this
run's successful Rust output**, with filesystem creation times after this worktree; 1405 MiB
were recovered. No source, assertion, dependency, existing cache entry or user data was changed.
The exact cleanup inventory and probe are retained as `owned-build-cleanup.json` and
`filesystem-probe.json`; `preflight.log` retains the failed run. With 2.8 GiB free, the same
source and full unskipped command passed in `preflight-retry.log`. Future disk pressure remains
a host risk. No instrumentation or owned preview process remains; the worktree/logs stay
available for review. The diagnosis is retained in the local `run.md`.

The 2× receipt inspection found the general title role broke “Membership” inside a word.
The card now consumes the existing label role, matching the reference's supporting brand
hierarchy. The same 320×800/2× scrolled receipt was rerendered and opened: “Cantos” and
“Membership” each fit a complete line, both CTAs remain readable. This is visual regression
evidence over the existing capture scenario, not a font-size unit test.

L5 real Membership HTTP/SQL, identity/payment,
OS Back/IME/insets and Android/iOS runtime are NOT_RUN; no new I/O is introduced by this change.
To close native visual acceptance, an operator must provide a ready Quail 2+ project for the
command above, then open/assert its PNG and semantics; device/reader acceptance needs its own
named target. Contracts require trusted installed ffmpeg/ffprobe and all selected gates PASS.
These follow-ups are unassigned; assignment is required before production acceptance.
Source logs/screenshots are checked for credentials/signed URLs before handoff. Remote CI is
not checked. The PR is updated without a merge or a claim that issue 128/#127/native release
acceptance is complete.
