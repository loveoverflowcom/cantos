# Shared UI requalification for #615 / PR #723

Reviewed by Codex on 5 November 2026, one automated reviewer. This records a bounded
requalification of the existing twenty paired component admissions and the missing UiRoot admission, including the new
`LoadingState` import in Theatre sign-in. It does not expand the compatibility baseline,
change a component contract, or establish full browser/device/assistive-technology acceptance.
The integration base is `origin/develop@aa97e0a3c1c573a4ba9c1818fa8e4212cc80511a`.
The observed source is `95647b321f24b82e526ea92722c76cf3d129c415` with a clean tracked tree;
subsequent edits add this evidence, qualification pointers, generated catalog metadata and the
repository test-oracle correction below. Runtime renderer inputs remain unchanged.

## Why the old records were stale

The previous [compact-preferences records](theatre-preferences-646.md) were source-bound to
an earlier renderer, token v0.12 and Rust 1.98. Since then, develop changed the Avatar alias
roles, ListenHero supporting chip, FilterChip multiline shape, Button reactive projections
and teardown ownership, icons, generated tokens and the build toolchain. The digest also
covers workspace manifests/lockfile and declared states. The original failure was reproduced
on the clean integration base; it was not caused by the sign-in presentation repair.

The first unskipped run after restoring the twenty pairs exposed a further existing missing
admission: `UiRoot` in Theatre's `SharedCatalogPresentationTest.kt`. That source is unchanged
from develop. A twenty-first pair now records the independently observed root/theme scope;
no test import was removed or baseline expanded.

The old evidence is preserved. New `ui-requalification-615-*-{leptos,cmp}.json` records reference
this review and current inputs. No checker, qualification requirement, baseline import site,
component declaration or runtime implementation was weakened to make admission pass.

## Executed renderer checks

Working directory: repository root. macOS arm64; Rust 1.99.1, Gradle 8.14.3,
Microsoft JDK 17.0.19 and Homebrew Python 3.14.8. Logs and original screenshots stay ignored
under `artifacts/agent-runs/615-signin-pending/`.

| Check | Result / actual boundary |
| --- | --- |
| `RUSTUP_TOOLCHAIN=1.99.1 CARGO_INCREMENTAL=0 cargo xtask ui-preview leptos verify` | PASS: real SSR/component tests, WASM clippy and Trunk gallery build; `requal-leptos.log` |
| `RUSTUP_TOOLCHAIN=1.99.1 CANTOS_UI_PREVIEW_CAPTURE_DIR=/private/tmp/cantos-615-cmp-requal cargo xtask ui-preview cmp verify` | PASS: 320 JVM tests, zero failures/errors/skips; `requal-cmp-fresh.log` and Gradle XML results |
| `cargo xtask ui-preview leptos run` | Actual compiled gallery observed at loopback 8083, in-app browser, DPR 1; `requal-serve.log` |
| `cargo xtask ui-preview cmp run` | Actual JVM preview process launched; `requal-cmp-run.log`. OS input inspection UNSUPPORTED: computer-use listed MainKt but rejected its name and bundle ID. No OS-window interaction is claimed. |

The first CMP invocation reused up-to-date test results. Those results were moved to
`previous-cmp-test-results/`; the recorded second run executed `desktopTest` and emitted fresh
PNG/JSON/semantics artifacts. The JVM tests mount production composables, drive input and
controlled clocks, and assert actual semantic nodes; they are not source-string checks or
static mockups. Tests cover blocked callbacks, typed values, focus, dialog Escape/return,
Select selection, navigation activation, aliases, hit targets, loading ownership/disposal,
reduced-motion policy and normal motion timelines. Build/semantic/pixel layers remain distinct.

## Repository test-oracle correction

Once UI admission passed, the full repository suite exposed an independent stale assertion in
`test_release_builds_carry_no_observer_or_test_dependency`: it expected `serde_json` only under
`service`, although develop already enables it for the explicit `recovery-testing` fixture.
Both the manifest and original assertion match the integration base. The assertion now permits
that exact existing fixture feature and strengthens transitive checks: default/persistence
cannot reach either service/recovery or JSON, service cannot reach recovery, and the fixture
binary requires exactly `recovery-testing`. No Identity manifest/runtime dependency changed.
The focused conformance-registry test file passed after correction; final repository/full
preflight results are recorded in PR #723. The failed run's 1,384 cases included one failure
and 16 internally optional skips; no preflight gate was skipped to hide the failure.

## Current Web observations

Trusted browser input in the existing gallery activated the primary action and edited the
TextArea to `Nguyễn Thị Ánh Nguyệt — Ỗ, Ữ` / `Lời hẹn còn nguyên.`. Dark theme, 200% gallery
text and reduced motion retained that edit. The real native dialog opened with focus inside;
Escape closed it and returned focus to `gallery-dialog-open`, retaining the typed text.
No API, credential, playback or persistence service was used.

Opened captures: `requal-web-fields.png`, `requal-web-actions.png`, `requal-web-icons.png`,
`requal-web-avatar.png`, `requal-web-feedback.png`, `requal-web-dialog.png`,
`requal-web-hero-matrix.png`, `requal-web-auth-loading.png`, `requal-web-edge-progress.png`.
The gallery window was 1265×712 CSS pixels, DPR 1. The workbench's 320px specimen frame is
not a 320px browser viewport. The earlier actual 320/390px application observations remain in
[the sign-in report](web-auth-pending-615.md).

Dark/200% fields retained legible Vietnamese labels and entered text; the actions wrapped
within their own surfaces. SVG icons/logo and initial/fallback Avatar tones rendered in their
existing wrappers. Feedback retained text plus icon distinction and a separate polite loading
announcement. The real dialog retained its close action and visible focus. The ListenHero matrix
used the real `listen-hero` recipe, dark, 200%, 320px frame and reduced motion; its supporting
text remained readable below the action. `auth-primary/loading` used the light, 100%, isolated
320px-frame recipe: the action remained mounted, natively disabled and busy, with a contained
spinner and wrapped label. The reduced-motion pending EdgeProgress fixture retained its old
content and textual updating explanation. These stills establish geometry/readability only;
they do not prove animation smoothness or spoken announcements. `requal-web-ax.txt` retains the
final accessible tree.

## Current independent CMP observations

Opened fresh originals under `/private/tmp/cantos-615-cmp-requal/`:

- `generated-alias-dark-320.png`: 200% Avatar aliases A/B/C render distinct tones with readable
  Vietnamese initials/name; `avatar-library-646-light-840.png` retains globe, initial, image,
  failed-image and size specimens without claiming a real user photo.
- `loading-admission-dark-320-reduced.png`: 200% text, reduced-motion policy; skeletons retain
  geometry, LoadingState has text plus spinner, all blocked action variants and FilterChip
  retain reasons and zero callbacks. Long diagnostic English variant names may wrap inside a
  word; this is not an every-label typography approval.
- `listen-dark-320-2-ready.png`: 200% hero action and Vietnamese chapter title fit the narrow
  frame. Current loading/reason-chip implementation also ran in the complete capture/test suite.
- `cmp-qualification-dialog-dark-text2.0-320-focused.png`: focused editable content and close
  action remain visible in the narrow/200% dialog. Large title content scrolls; the still is
  not a claim that the entire dialog content fits simultaneously.
- `cmp-theatre-filter-chip-theatre-selection-dark-text2.0-320.png`: the changed fixed-radius
  shape surrounds the multiline Vietnamese label without clipping its first/last lines.

JSON sidecars bind PNG hashes, actual bitmap/frame dimensions, theme, scale, font source,
fixture and reduced-motion policy. Their `source_revision` is `unrecorded`; this report binds
the fresh run to the clean source revision above rather than pretending the sidecars contain a
Git SHA. Source digest below is independent of the PNG/token-file byte hash in those sidecars.
These are JVM Compose test-root observations, not OS-window input, Android/iOS, browser parity,
OS reduced-motion settings or screen-reader output.

## UiRoot admission

Both real galleries mount UiRoot and apply its scoped preferences. The Web theme/text/reduced
motion changes above retained actual edited content and dialog behavior. Current Leptos
foundation/theme tests and the fresh CMP `ThemeFoundationTest` independently check nested theme
restoration, absolute text scale, unchanged platform density, scoped colors, inherited fonts
and sibling/host restoration. All opened current captures are rendered inside those production
roots; their light/dark and enlarged text differences are observable. This qualifies the root's
existing presentation contract within browser/JVM scope. It does not turn the unchanged Theatre
catalog tests into real private-catalog or native audio acceptance.

## Attribution and qualification scope

The fresh full suites and current observations above cover changed shared behavior. The
unchanged component/reference review remains attributable to the
[shared navigation review](theatre-navigation-646-shared-qa.md),
[guest/FilterChip review](theatre-guest-tonal-646.md), and
[compact-preferences review](theatre-preferences-646.md). Those records supply retained
reference/visual judgments for unchanged fields, surface, badge/status/error, logo,
Back/NavItem, dialog and progress contracts. Their historical screenshots are not relabeled
as new captures. Current full tests independently check those components against the current
build inputs; the opened fresh representative captures add affected visual evidence.

The paired records admit the existing component APIs within this bounded scope, including
LoadingState, and retain prior limits. They do not claim that every catalog state was visually
re-reviewed, a pixel comparator was run, native/device accessibility passed, or app authentication
was accepted. Actual Android/iOS touch/Back/IME, TalkBack/VoiceOver, other browser engines,
browser zoom, complete state/theme/viewport matrices and production authentication remain
NOT_RUN, outside this presentation repair. Owner: unassigned — assignment required before
those broader acceptance claims. No deployment, distribution or credential operation occurred.

Canonical qualification digests (not screenshot byte hashes):

- Tokens: `79294511da18b463bd20056aa70cda914d488c43d6ccaabf0b398184b4fc1218`
- Leptos: `646ddccb291020483eab15d22410ed54816107f3e5a2ed6f78e4d1d8f69181fe`
- CMP: `f88fcfc1645899658ad5aff58259ca5a263f4f0b09a3dac4e62e03b08149fe7a`

Final-diff unskipped preflight and source revision are recorded in PR #723. Commit/push/merge
packaging of the same content does not invalidate these renderer-input digests.
