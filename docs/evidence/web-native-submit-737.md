# Native Web form submit and Button anatomy (#737)

This source-bound record covers the Web Leptos implementation and bounded renderer
renewal for #737. The fetched integration base is
`develop@77d722a301c46048ddefabf1dff5e2c88bd9fb77`, after #735 / PR #750.
Reviewer: Codex, one automated reviewer, 5 November 2026. This is neither a new auth
flow nor production, device, screen-reader or full product acceptance.

## Review boundary and API decision

| Consumer | Ownership and decision |
| --- | --- |
| `apps/cantos-studio/web/src/signin_form.rs::SignInPanel` | Entry and account reauthentication share the same native form. Forward `IconName::Login`; keep the shell's submit intent, duplicate guard, credential clearing and errors. |
| `apps/cantos-studio/web/src/view.rs`, `revision-read` | Retain the native form and its read/discard/activity guards; Tonal remains the default. |
| `apps/cantos-studio/web/src/live/delegation/view.rs`, `delegation-issue-submit` | Retain the native grant form, permission facts, validation and effect owner. |
| `apps/cantos-studio/web/src/ui.rs::FormSubmit` | Only adapt Studio's localized ContextReady/Busy reasons and boolean facts to `ButtonState`; no internal `.cui-button-*` markup remains. |
| `apps/cantos-theatre/web/src/features/auth.rs::LoginForm` | Reuse Button's explicit busy-label policy; preserve its existing form/key handler and one typed intent. |

The existing catalog API is `button`, including enabled/disabled/loading/focus and
AuthPrimary. `IconButton` already offers `submit: bool`; the same optional, default-false
input is the smallest compatible extension to `Button`. A second component or renderer
would duplicate its anatomy. A new button-kind enum would add a second spelling for the
existing binary contract without admitting another supported native kind.

`submit=true` changes only the native type. The form retains Enter/click, validation and
dispatch; it must not dispatch the same command in `on_press`. A disabled/loading click
also cancels its native default action when delivered synthetically. Ordinary Button
remains non-submit. ListenHero's existing focus policy is unchanged; a loading submit is
always natively disabled. Children and reactive labels remain inside one mounted control,
including the existing auth geometry observer.

`busy_label_only=true` is an explicit presentation policy: only a loading reason becomes
offscreen, with its description and polite status retained. Disabled reasons stay visible.
Theatre no longer uses the broad auth/checking selector that hid every button reason.
Bootstrap/recovery remain separate facts. Studio's old arbitrary decorative `ViewFn` slot
had one production consumer, a Login icon; it now forwards `IconName` to the existing owner
instead of requiring another generic shared slot. No public form, field value or auth
command contract changes. The existing auth model still clears secrets before I/O;
the renderer itself never writes field values.

No ADR or queue change: this extends an existing renderer API within the accepted native-HTML
and shared-anatomy contracts. The queue's order/rationale and all backend ownership remain.

## Checks and evidence boundary

Working directory:
`/Users/manhblue/Documents/Codex/2026-11-06/task-4/cantos`.
Local logs/scripts/JSON/PNGs stay ignored under `artifacts/agent-runs/issue737/`.
Rust 1.99.1, Trunk 0.21.14, macOS arm64, Chrome 156.0.8078.80 (headless CDP), DPR 1.
Cargo checks use `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0` to bound local resource use.

- New shared native-submit tests first failed on the missing API (`red-submit.log`), then
  all 19 action/navigation tests passed (`actions-test.log`). Assertions cover default
  non-submit, enabled/disabled/loading, one glyph/spinner, description IDs, and the explicit
  busy policy. The existing ListenHero regression remains.
- Studio adds a VI/EN state projection regression. An old assertion required hidden blocked
  copy even on an enabled action; it now asserts that the reason exists only while blocked,
  matching the shared state contract. The first suite recorded that assertion failure before
  the correction; it is not called a passing run.
- `cargo xtask ui-preview leptos verify`: PASS, component/SSR tests, WASM clippy and gallery
  build. `preview-verify.log` records the actual independent rows.
- The first `cargo xtask preflight --changed-only --base origin/develop` ran every selected
  Web gate: format, native lint/tests, WASM lint, both app builds and gallery build PASS;
  repository FAIL on the deliberately stale Leptos qualification records. Studio 493 unit
  tests and 5 doctests, Theatre 610 unit tests and 6 doctests, and the shared UI suites passed.
  `preflight.log` retains the original failure; a later renewal does not relabel it PASS.
- Theatre's explicit busy policy was separately reproduced as FAIL before adding the prop
  (`red-theatre-policy.log`); `theatre-policy-test.log` records the corrected focused test.
  Final gate results are in the PR and issue, bound to the actual checked head/tree.
- The next repository run accepted admission, then caught five displayed diagnostic literals
  added in the new specimen. They were moved without text changes into the existing preview
  message owner, reusing the existing language autonyms. The source-bound browser observations
  remain applicable to identical rendered content; subsequent static/build checks cover the
  resource indirection. The failed run remains `preflight-final.log`.
- Final preview-only language metadata sets the diagnostic shell to English and the native
  form to its selected VI/EN language. `preview-final-verify.log` records all three preview
  gates PASS; `language-dom.json` observes VI→EN on the same button with zero exceptions.
  No CSS `:lang` selector or displayed byte changes, so unchanged visual/input observations
  and unchanged product gate inputs are reused under the verification cadence.
- `preflight-handoff.log` covers the final runtime source: every Rust/Web gate PASS, repository
  FAIL in Python fixtures, and two CMP gates UNSUPPORTED (Gradle absent). The Python failures
  resolve `/var/folders/...` to `/private/var/folders/...`: 68 release-path errors rejected a
  symlink ancestor, with related physical-path/ownership assertions. These scripts/tests are
  unchanged from the integration base. A physical `TMPDIR=/private/tmp/cantos737-tests`
  makes `python3 -m unittest discover -s scripts -p test_android_release_evidence.py` PASS.
  The repository-only rerun uses that same physical temp root; its actual result is retained in
  `repository-physical-temp.log` and the PR/issue. No path guard or test threshold was relaxed.

### Browser form and renderer observations

`cargo xtask ui-preview leptos run` served the actual compiled gallery on loopback 8083.
`node artifacts/agent-runs/issue737/gallery-check.mjs` exercised 24 cases: required/invalid
email, trusted Enter/click, one dispatch, native and synthetic repeat refusal, default
non-submit action, VI/EN label changes with the same button/input/focus, preserved values,
and measured capsule radius. The 16 geometry cells are Light/Dark × VI/EN × 320/390 CSS px
× 100/200% gallery text. The capture viewport is 320×1600; this is root text scaling, not
browser page/text-only zoom. `gallery-observations.json` retains exact dimensions and facts.

Opened `submit-{light,dark}-{vi,en}-320-{100,200}.png`: one decorative spinner and visible
progress label, no second visible busy reason, and wrapped labels contained in the native
button. Long input text scrolls inside its field. The wider gallery has pre-existing overflow
in unrelated specimens at narrow/large text; this record makes no whole-gallery reflow claim.

`requal-check.mjs` drove actual Button/IconButton/FilterChip callbacks and blocked activation,
field/textarea edits, native select/checkbox/radio/switch input, theme/scale/reduced-motion
edit retention, and real dialog initial focus, Escape/return and reopen retention.
Representative current captures cover actions, fields, feedback/error, layout, icons/logo/
Avatar, dialog, and the existing AuthPrimary/Select/ListenHero/EdgeProgress recipes. Current
DOM/input observations supplement unchanged source-attributable reference observations in
[the preceding qualification](ui-requalification-615.md); they do not claim every state,
animation smoothness, every engine, native device or assistive technology.

### Mounted product frontend fixture

The documented `cargo xtask app cantos-studio --web` refused before startup because no
Server listened on loopback 8080 (`studio-serve.log`). Independent browser checks therefore
deliver the actual Trunk app assets through CDP and inject synthetic API responses at the
transport boundary. They run production Web components and event/HTTP-client code, with no
real session, cookie, backend permission or authentication acceptance.

For browser transport only, the oversized WASM diagnostic `name` custom section was omitted;
every retained section is byte-identical. Original/fixture hashes and retained-section hashes
are in `wasm-fixture-provenance.json`; no source or build output was patched. This is no
performance or release-artifact claim.

`product-check.mjs` records ten Studio observations: Light/Dark × VI/EN pending/error,
one refused request on repeats, revision-read native submit with a valid synthetic UUID,
and reauthentication pending. `product-observations.json` identifies the doubles and requests
without credential bodies. Opened Studio pending, revision-read and reauth captures show
one spinner/progress label in the CTA, visible blocking explanations and no redundant top
pending banner. Retained error text after refusal is asserted in the DOM separately.

`theatre-check.mjs` then passed four Light/Dark × VI/EN login cases at 390×1200, over the
same explicit transport double. Trusted Enter issued one request; synthetic/pointer defaults
and repeated Enter were blocked while pending. Opened `theatre-*-pending.png` show one spinner
and visible Signing in label inside the CTA, a separate visible registration blocker and no
top pending banner. The refusal error remained in the DOM afterward. All these product
fixture runs observed zero runtime exceptions. The final headless capture pass explicitly
emulated page focus to avoid background frame throttling; no OS-window or screen-reader
focus claim follows.

## Admission and limitations

The renderer-wide digest makes all 21 Leptos admissions stale when Button changes. The new
`ui-requalification-737-*-leptos.json` records renew the existing bounded admissions using
the current tests, driven input and inspected captures above; old #615 records remain intact.
Catalog qualification pointers change, with generated metadata regenerated by its owner.
No baseline import sites, component/state declarations, tokens or checker were weakened.
The existing `Button` import site in Studio `ui.rs` remains the owner; no new product UI API
or import site is introduced. Native form semantics are Web-specific, not a CMP product.
All CMP renderer inputs and tokens retain their prior digest; the existing CMP records remain
source-current. No new CMP runtime qualification is claimed. The metadata path conservatively
selects CMP preview gates; an absent Gradle executable is reported explicitly, not bypassed.

Canonical input digests: tokens
`79294511da18b463bd20056aa70cda914d488c43d6ccaabf0b398184b4fc1218`, Leptos
`e92cc3969ad8f6b2dd7947917bb21ab48c01e63fab2fdb078428c0bf8053590b`, unchanged CMP
`f88fcfc1645899658ad5aff58259ca5a263f4f0b09a3dac4e62e03b08149fe7a`.

Invariant: one native control and one form-owned dispatch; stable fields, locale and reasons.
Owner/boundary: shared Web Button anatomy; Studio/Theatre retain commands and permissions.
Behavior SSOT: no registered generic button/form capability; outcome-preserving refactor plus
the approved blocked-activation guard. Catalog/path lookup derives no formal obligation.
No model, lifecycle, session/authority, persistence or publication rule changes.
Evidence levels: source/static, example-tested, DOM-tested, trusted interaction-tested and
bounded screenshot/cross-theme/cross-viewport-inspected; synthetic product transport is mocked.
Surface intensity: existing quiet Studio form and existing auth CTA hierarchy; no retheme.
Tokens: unchanged canonical source, CSS template and CMP mappings; no new contrast claim.
Localization: app owners unchanged; preview-only Submitting has paired VI/EN resources.
Accessibility: DOM descriptions/live-region policy, trusted keyboard and dialog focus checked;
no screen-reader walkthrough. Motion: reduced-motion gallery policy exercised; smoothness and
OS setting propagation not observed. Performance/cache/media/provider costs: no change or claim.
L1/L2 preserve ownership/types and effects; L3 component/static suites and L4 frontend fixture
are independent of real L5 backend/auth acceptance.

AC-04 remains open for the complete mounted product Light/Dark, VI/EN, narrow/actual 200%
browser zoom, keyboard/accessibility matrix over real local services, including Theatre login.
Resume with local synthetic owner/session facts and `cargo xtask run cantos-studio --web` /
`cargo xtask run cantos-theatre --web`; next owner unassigned. Full UI, device, screen-reader,
deployed and production acceptance are not established by these fixtures. No deploy was run.
