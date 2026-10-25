# Shared component qualification recheck — 25 October 2026

Status: **in progress; no new admission yet**. This record belongs to the separate qualification
repair PR, not the design changes in #452–#454. Repository qualification remains closed until
the actual checks below pass and fresh two-renderer records are reviewed. No deployment or
merge follows from this record.

## Boundary and source

Base: `develop@b49845e53902c38d851d645c59e890dacb621500`.
Existing checkout and branch `fix/ui-qualification-gates`; no additional worktree.
The existing compatibility baseline, renderer digests and qualification gate are unchanged.
Historical `ui402-*` evidence records are preserved.

Owners: [preview registration/adoption](../../packages/ui-system/shared/README.md#preview-registration-and-adoption),
[admission method](../../.agents/skills/cantos-ui-design/references/preview-catalog.md),
[Leptos recipes](../../packages/ui-system/leptos/README.md#scenario-recipes-and-replay) and
[CMP recipes](../../packages/ui-system/cmp/README.md#scenario-recipes-and-replay).
This changes no service, public product API, SQL, auth authority, native playback or token palette.

## Reproduced failure and classification

`RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask verify repository` at the clean base:
**FAIL**, exactly 32 messages. Full output:
`artifacts/agent-runs/20261025-ui-qualification/baseline-repository.log`.

| Class | Observed scope | Cause and required resolution |
| --- | --- | --- |
| 11 stale Leptos records | Button, TextField, Surface, Badge, BackButton, Logo, Select, Icon, Status, Dialog, Checkbox | Renderer implementation/preview/state contract changed after the recorded source. Rerun and inspect actual current behavior; no hash-only renewal. |
| 11 stale CMP records, masked by the first Leptos error | The same eleven components | Shared declarations changed after the recorded CMP source. Conservative invalidation is intentional; run and inspect current native counterpart contracts. |
| 21 dependent adoption messages | Five Studio paths, eight Theatre CMP paths, eight Theatre Web paths | They depend on fresh two-renderer evidence; do not expand the compatibility baseline. |
| Missing first admission | ErrorState in Studio authoring | Not one of the eleven records. Its implemented recovery/announcement contracts need fresh evidence in both renderers. |

Independent read-only reconstruction from Git objects:
the stored Leptos digest corresponds to `4104f4e6`; changed inputs include foundation CSS,
three preview files, fields/Select model, tests and shared declarations. The stored CMP digest
corresponds to `0b672442`; only shared declarations changed. Tokens, root build inputs, fonts
and design inventory did not change. The existing state-contract invalidation regression
covers this policy; there is no evidence of a digest bug.

The 21 adoption paths are:

- Studio: `authoring_view.rs`, `live/artwork/form.rs`, `live/dashboard.rs`, `signin_form.rs`, `view.rs`.
- Theatre CMP: `TheatreWorkbench.kt`, `AccountScreens.kt`, `AuthScreens.kt`, `RegistrationForm.kt`,
  `VerifyEmailScreen.kt`, `CantosListeningUnavailable.kt`, `MembershipComponents.kt`, `MembershipPreview.kt`.
- Theatre Web: `features/account.rs`, `features/auth.rs`, `features/error_dialog.rs`,
  `features/library/live.rs`, `features/public_home.rs`, `features/public_work.rs`,
  `features/signup.rs`, `view/live_shell.rs`.

### Actual consumer scope, not stale candidate descriptions

Web Floating fields already serve Studio email/password sign-in and Theatre search.
Enhanced Select already serves Studio and Theatre, including compact-inline locale selection.
A baseline-only renewal that leaves those adopted variants unreviewed would be incomplete.
CMP implements Regular/AuthTonal fields, not Floating; its catalog fallback must never be
called a native Floating rendering. Web-specific geometry needs an explicit counterpart limit.

Button AuthPrimary/AuthSecondary/AuthOutlined have real consumers. Unified Back and
TextField AuthTonalSpacious have no current app references and remain outside this admission.
Back consumers use the existing default IconTonal behavior.

## Implemented diagnostic increment

The Select workbench previously exposed a missing current value but no disabled current value.
The separate stable-value gallery specimen uses a native select, so it cannot exercise the
enhanced menu's disabled-current Tab/blur case. The already registered `unsupported-option`
state is now reachable through the existing recipe owner, with controlled value `unavailable`.
Both initial mount and explicit reset preserve it. No product Select implementation changed.

- New parser/scope regression: **FAIL before** (`InvalidValue(State)`), **PASS after**.
- Existing roundtrip coverage includes the new state automatically; other controls reject it.
- Independent read-only increment review found no correctness/scope issue.
- Runtime opening, Tab/blur with no navigation, deliberate replacement, Escape and reset/no-dispatch
  remain **NOT_RUN** at this checkpoint. Parser/source review does not establish them.

Logs: `select-fixture-regression-before.log`, `select-fixture-regression-after.log` under
`artifacts/agent-runs/20261025-ui-qualification/`.

## Executed checks and retained inputs

| Check | Status | Attributable scope |
| --- | --- | --- |
| Leptos verify on clean base | PASS | Native SSR tests, WASM clippy, Trunk gallery build |
| Leptos verify after diagnostic increment | PASS | Same actual package checks, including new scope regression; log `leptos-verify-scenarios.log` |
| CMP verify on base | PASS | 221 executed tests in 35 suites, zero failures/errors/skips; ten Gradle tasks executed; 93 PNGs produced |
| Runtime/input qualification | NOT_RUN | Actual current browser/window review still required |
| Accessibility/visual admission | NOT_RUN | No screenshot or assistive-technology result inferred from compilation/tests |
| Repository/adoption | FAIL | Baseline 32 messages; no qualification records renewed |
| Final preflight / exact-head CI | NOT_RUN | Run on the final handoff content; intermediate package checks are not final admission |

Commands:
`RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask ui-preview leptos verify`;
`RUSTUP_TOOLCHAIN=1.98.1 JAVA_HOME=/opt/homebrew/opt/openjdk@23/libexec/openjdk.jdk/Contents/Home
PATH=/Users/manhblue/.local/share/gradle/gradle-8.14.3/bin:$PATH
CANTOS_UI_PREVIEW_CAPTURE_DIR=/Users/manhblue/Documents/Codex/2026-10-25/task-2/cantos-develop/artifacts/ui-evidence/20261025-qualification-cmp
CANTOS_SOURCE_REVISION=b49845e53902c38d851d645c59e890dacb621500 cargo xtask ui-preview cmp verify`.

The native captures name `jvm-desktop-compose-test-root`; they do not establish an OS window,
Android/iOS device, screen reader or matched design pixels. Captures stay under ignored
`artifacts/ui-evidence/20261025-qualification-cmp`; unopened PNGs are only screenshot-captured.

The [existing design ZIP](https://drive.google.com/file/d/1GWM-cA5AZEEEx_wv8UMPsgEQmIRwdSVU/view)
was downloaded and its SHA-256 matches the prior source-bound reference:
`5a14eda0991d7f2baf745cf63a92c1c76b32f70f2eca638f53074d524b869b01`.
Only 27 relevant PNGs were extracted into ignored
`artifacts/ui-evidence/20261025-qualification-reference`; HTML was not extracted or executed.
Reference-pixel inspection is still **NOT_RUN** at this checkpoint.

## Completion fields and limits

Invariant: current adopted contracts require actual, fresh two-renderer evidence before adoption.
Owner/boundary: shared UI previews, record provenance and the existing adoption gate.
Evidence level: example-tested, statically-checked and JVM semantics/interaction tests within
their executed scope; no new runtime/visual admission yet.
Screens/consumers: the shared controls and ErrorState above; product business flows remain separate.
Surface intensity: not changed; no new visual direction selected.
Tokens/localization: no palette, semantic role or product string changes.
Accessibility: automated JVM semantics/input scope only; browser focus and real AT not yet observed.
Visual: 93 current JVM captures and 27 pinned reference PNGs retained; not yet inspected.
Motion: not observed; no motion or device claim.
Residual risk: stale records and missing admission continue to block the repository/release gate.
Do not turn any missing check into PASS or deploy from this unmerged branch.
