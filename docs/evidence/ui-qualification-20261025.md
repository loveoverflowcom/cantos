# Shared component qualification recheck — 25 October 2026

Status: **incomplete Draft PR; no new admission**. The existing qualification gate remains
closed. This record preserves executed checks and the exact missing evidence; it is not a
completed repair of the 32 qualification diagnostics. No qualification receipt, compatibility
baseline or gate policy was relaxed. Historical `ui402-*` records are preserved.

## Source and scope

- Original diagnostic base: `develop@b49845e53902c38d851d645c59e890dacb621500`.
- Updated integration base: `develop@fa114100baea6919515a29d4a5b4d9dcb573afa1`, including #457.
- Inspected renderer inputs: `255c593708f8b29a750aa3cb8abce55aa59c6a6c`, clean at execution.
  A later copy-only correction changes “Web-only” to “Web specimen” in the gallery note, because CMP now has its own Floating implementation. Browser stills remain attributed to this inspected SHA; no final admission is inferred.
- Existing checkout, branch `fix/ui-qualification-gates`, [Draft PR #459](https://github.com/loveoverflowcom/cantos/pull/459).
  No additional worktree, source cleanup, token creation, production activation or merge.

Owners: [preview/adoption contract](../../packages/ui-system/shared/README.md#preview-registration-and-adoption),
[admission method](../../.agents/skills/cantos-ui-design/references/preview-catalog.md),
[Leptos recipes](../../packages/ui-system/leptos/README.md#scenario-recipes-and-replay),
[CMP recipes](../../packages/ui-system/cmp/README.md#scenario-recipes-and-replay).

The #457 merge changed tokens and both renderer implementations. Earlier checks at `b49845e5`
and `179780f7` are historical evidence only; they do not qualify the integrated inputs.
No #453/#454 product composition or new design direction is implemented in this PR.

## Reproduced failures and disposition

The original clean-base `cargo xtask verify repository` failed with exactly 32 diagnostics.
Full output: `artifacts/agent-runs/20261025-ui-qualification/baseline-repository.log`.

| Class | Scope | Disposition |
| --- | --- | --- |
| 11 stale Leptos records | Button, TextField, Surface, Badge, BackButton, Logo, Select, Icon, Status, Dialog, Checkbox | Current package checks and bounded browser observations below; full admission incomplete. No hash-only renewal. |
| 11 masked stale CMP records | Same components; validation stops at the first failed renderer per component | Fresh JVM checks/captures and independent visual review below; remaining runtime/visual gaps keep records unqualified. |
| 21 dependent adoption diagnostics | Five Studio, eight Theatre CMP, eight Theatre Web paths | Remain blocked by two-renderer admission. No import exemption or compatibility-baseline expansion. |
| Missing first admission | ErrorState used in Studio authoring | Recovery callbacks observed on Web and tested on JVM; missing visual matrix prevents first admission. |

The 21 paths are Studio `authoring_view.rs`, `live/artwork/form.rs`, `live/dashboard.rs`,
`signin_form.rs`, `view.rs`; Theatre CMP `TheatreWorkbench.kt`, `AccountScreens.kt`,
`AuthScreens.kt`, `RegistrationForm.kt`, `VerifyEmailScreen.kt`, `CantosListeningUnavailable.kt`,
`MembershipComponents.kt`, `MembershipPreview.kt`; and Theatre Web `features/account.rs`,
`features/auth.rs`, `features/error_dialog.rs`, `features/library/live.rs`,
`features/public_home.rs`, `features/public_work.rs`, `features/signup.rs`, `view/live_shell.rs`.

Independent Git reconstruction showed intentional source invalidation, not a digest bug.
Current #457 source additionally changes the token palette and Theatre specimens. Neither the
old source digest nor the old token digest is valid for the combined tree.

## Diagnostic repair and current capabilities

The Select workbench now accepts the already registered `unsupported-option` state only for
Select, initializes its controlled value to the disabled `unavailable` option, and restores it
on explicit reset. The separate stable-value gallery uses a native select and could not exercise
this enhanced-menu case. Product Select behavior is unchanged.

The new parser/scope regression failed before the repair (`InvalidValue(State)`) and passes
after it; other component recipes reject the state. Logs `select-fixture-regression-before.log`
and `select-fixture-regression-after.log` are retained in the run directory below.

Capability descriptions distinguish implementation from admission. Web Floating is already
used in Studio sign-in and Theatre search; enhanced Select has actual consumers. Current CMP
now implements Floating/FloatingSearch and trailing actions through #457. The earlier
no-native-Floating diagnosis applied to the original base and is superseded. Unified Back and
Web AuthTonalSpacious remain unused candidates, outside this admission. No new receipt is issued.

## Executed checks on the integrated inputs

Run directory: `artifacts/agent-runs/20261025-ui-qualification/`.

| Check | Result | Evidence boundary |
| --- | --- | --- |
| `cargo xtask ui-preview leptos verify` | PASS | Native package tests, WASM lint, Trunk build; `leptos-verify-final-inputs.log`; copy-only final correction also PASS in `leptos-verify-copy-final.log` |
| `cargo xtask ui-preview cmp verify` | PASS | 248 tests / 39 suites, zero failures/errors/skips; actual `desktopTest` execution; `cmp-verify-final-inputs.log` |
| CMP capture attribution | PASS | 151 fresh PNG hashes in `cmp-final-run-manifest.json`; 87 individual sidecars match source, current token bytes, dimensions and hashes. The other 64 legacy-helper captures inherit the enclosing fresh-run attribution, not invented sidecars. |
| Bounded Web callback/state observations | PASS for the listed callback/value/blocked-state cases | Actual user Mac in-app browser, production gallery components, synthetic local fixtures only; excludes Dialog containment below |
| Dialog immediate focus-containment assertion | FAIL / unresolved | Tab after the last control makes BODY active; next Tab returns to Close. Native browser-chrome cycling versus background-page escape is not disambiguated. No containment PASS or confirmed product-regression claim. |
| Complete two-renderer qualification | NOT_RUN / incomplete | Remaining matrix and OS-window observations below; no admission PASS |
| Final changed-only preflight | FAIL | Repository rejects the same 32 qualification/adoption diagnostics; downstream steps are NOT_RUN in this invocation. `preflight-handoff.log`; an earlier PATH-only attempt was UNSUPPORTED, then corrected using installed Gradle/JDK. |
| Final-head CI | See the final PR check run | Automatic PR run only; the handoff reports terminal outcomes separately. No manual duplicate dispatch. |

Commands used Rust 1.98.1, `TMPDIR=/private/tmp`; CMP used Gradle 8.14.3 and
`JAVA_HOME=/opt/homebrew/opt/openjdk@23/libexec/openjdk.jdk/Contents/Home`.
`CANTOS_SOURCE_REVISION` was the full current renderer SHA. Fresh capture directory:
`artifacts/ui-evidence/20261025-qualification-cmp-final`. Older capture directories were preserved.

## Actual Web observations

`cargo xtask ui-preview leptos run` served loopback port 8083 with the current source SHA and
`CANTOS_PREVIEW_SOURCE_DIRTY=false`. The browser was reloaded after the build. Browser facts
captured at `2026-10-25T13:17:55.402Z`: Chrome 156, Mac user agent, DPR 1, fonts loaded,
1280×900 CSS viewport; narrow observations used 390×844. Gallery settings and workbench recipe
settings are distinct. A 320 specimen frame is not a 320 browser viewport or browser zoom.

- Select `unsupported-option`: open then Tab retains `unavailable`, no intent; ArrowDown then
  Escape retains it; End then Enter changes to `dark` with exactly one intent; Reset restores
  `unavailable` and clears the log without a callback. Repeated on the integrated source.
- Dark 200% long-Vietnamese Select was opened and inspected in the 390×844 browser with a
  320 frame. The long option wraps and the list scrolls. The menu measured x=8, y=8,
  width=359, height=348.65625, scrollHeight=733. No whole-product responsive acceptance follows.
- Floating: synthetic input signal survives theme, density, 200% text and read-only/disabled/
  loading changes; read-only stays enabled/selectable, blocked states disable field and CTA.
  Error focus has `aria-invalid=true` and help/error description IDs. Opened the dark, 200%,
  long-label error/focus capture: label wraps above the native single-line value; support/error
  text stays outside. Native horizontal editing is not a clipped-label finding.
- Email/password DOM values are redacted by the browser tool. Value-retention assertions use
  the fixture's explicit controlled-state output, not a false comparison against redacted DOM.
- Theatre password eye: selection 0–2 survives the first eye toggle; the confirmation input
  remains password-typed. The eye is restored. Search Enter increments submissions to 1 and
  a separate submit-button click to 2, without a duplicate callback.
- AuthPrimary Enter plus AuthSecondary Space yield exactly two intents. Loading disables all
  three Auth actions without increasing the counter. Baseline Back Enter yields one intent;
  switching it to Loading keeps that count and disables the target.
- ErrorState's two recovery actions yield count 2. Its Off announcement has no live role;
  the separate feedback is host-owned. Opened light wide recovery/layout capture with visible
  title, explanation, both actions and focused notes button.
- Dialog: edit, Escape, reopen retains text; Escape restores the trigger; close works again.
  Tab after the last control temporarily makes BODY active in the browser focus cycle, and
  the following Tab returns to Close. This is not an observed immediate in-dialog focus trap.
  Opened the wide light modal with complete title/body/input and focus ring.

Web images were captured and opened through the Mac browser tool; they remain in its transcript,
not fabricated local PNG paths or pixel-comparison reports. Screenshots establish the observed
still states only. Real autofill/IME, forced-colors, non-Chromium and AT are NOT_RUN.

## CMP and reference inspection

The primary reviewer opened current dark 320/200% Theatre field matrix and dark 390/200% Select.
Labels/support text wrap, the field grows, and long editable values retain a native horizontal
viewport. Select's selected check and leading glyph are separate; the disabled reason is visible.
Independent read-only reviewer `qualification_review` opened 31 current CMP images and relevant
references for ten components. It found no clipped labels in those inspected crops: large Badge
and Checkbox labels wrap; blocked Button/Back reasons remain visible; Logo knockouts/aspect-fit,
Icon glyphs, Status Info and normal-scale Dialog title/body/Close remain intact.

The v3 ZIP was downloaded with SHA-256
`5a14eda0991d7f2baf745cf63a92c1c76b32f70f2eca638f53074d524b869b01`;
27 PNG references were extracted without executing HTML. Current v4 boards
[4](https://drive.google.com/file/d/1PwHNisJ6VoV8nCj9NPiinuKlTbGntNHL/view) and
[5](https://drive.google.com/file/d/1qQoGVhDzWSBv3ko-TeuI0L7O5Uskg47q/view) were fetched and opened:
independent password eyes, one field shell/perimeter, 68/72 minima and light/dark counterpart
anatomy. This is contract/anatomy inspection, not matched-pixel fidelity. The current ErrorState
recovery composition differs from the historical centered offline-card reference; no fidelity
claim is made.

## Exact remaining blockers and continuation

- **CMP OS-window input: UNSUPPORTED by the available UI route.** The canonical run command
  reaches Gradle `:run`, and UI inventory reports `MainKt` / `net.java.openjdk.java`. Both
  documented app selections return `Invalid app`. No native window click/keyboard review is
  claimed. Resume with a supported route to that existing preview window; no new stack is needed.
- **Web Dialog focus containment: unresolved.** The immediate last-control-to-first-control assertion failed in the available browser. Disambiguate browser-chrome focus cycling from background-page escape with an observable focus destination before admission or any component fix; Escape/restore/retained-text passes do not fill this result.
- **Visual matrix: NOT_RUN.** ErrorState light plus narrow/large text; Dialog large text and
  full modal viewport; explicit Status tones beyond inspected Info; remaining Web component
  matrix. Existing Dialog PNGs are modal-node crops, not full host/scrim captures. The narrow
  200% gallery-shell image contains only controls and is not component evidence.
- **Admission: closed.** Complete the missing actual observations on current inputs before
  creating any PASS receipt. Recompute source/token bindings only after that review, regenerate
  the catalog, rerun preflight and review final-head CI. No baseline or gate waiver is authorized.
- Product browser/device journeys, Android/iOS, TalkBack/VoiceOver, actual motion interruption,
  native playback and deployed acceptance remain separate and NOT_RUN.

Evidence level: statically checked, example-tested, real JVM semantics/interactions, bounded
browser input and inspected stills within the limits above. No new model/proof, service,
authentication, audio, device or production result is inferred.
