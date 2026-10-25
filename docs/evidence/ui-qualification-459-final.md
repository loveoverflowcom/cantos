# Shared component qualification — PR #459

This record accepts the **bounded shared-component scopes below** on Leptos in the
Mac Chromium browser and CMP on the JVM renderer. It does not accept a production
journey, Android/iOS device, assistive technology, real IME, OS-window input or
motion smoothness. Unused Unified Back and Web AuthTonalSpacious candidates are
outside this review. Other registered public APIs are not newly admitted.

## Inputs and method

- Integration base: `fa114100baea6919515a29d4a5b4d9dcb573afa1`, including #457.
- Reviewed source: `84020c9d0e7323492a828ec653ff4141c63ca105`.
- Canonical token digest: `f0ab05cb589a06626850889f108f9fa7dc0db5505783574beceee2aee80ed1a3`.
- Leptos input digest: `94c7a7d6c00402385da9218abbb1966df8c5c39af821138b352325d8b9ea31a2`.
- CMP input digest: `25686dbe46377286f9622084eb234cb719d2932f8bec4bf8c8a5b46f53bf60dd`.

Evidence/catalog packaging is excluded by the existing digest implementation;
no renderer declaration, compatibility baseline, validator or gate is relaxed.
Historical `ui402-*` receipts remain intact. The [chronological investigation](ui-qualification-20261025.md)
preserves failed probes and intermediate evidence, superseded where identified.

The inspected references are the retained owner-supplied v3 component boards
(`Cantos-UI-System-v3-2026-10-15.zip`, SHA256
`5a14eda0991d7f2baf745cf63a92c1c76b32f70f2eca638f53074d524b869b01`),
previously opened v4 boards 4/5 attributed in the [foundation record](theatre-expressive-foundation-452.md),
the current token roles and canonical component contracts. This is a semantic,
anatomy and readability review, not matched-pixel fidelity to a whole board.

## Executed layers

| Layer | Result and actual observation |
| --- | --- |
| Leptos structural | `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview leptos verify` PASS; package tests, WASM lint and Trunk build. Current diagnostic-only focus observer is covered by `leptos-resume-focus-facts.log`. |
| Leptos runtime | Canonical `cargo xtask ui-preview leptos run`; real controls driven in the user's Mac in-app browser. Input, state, keyboard and focus observations below, with no API/backend fixture. |
| Leptos accessibility | Native DOM names, labels/help/error references, disabled/read-only state, decorative assets, live-region opt-in, keyboard dispatch and modal restoration observed. This is DOM/keyboard evidence, not a screen-reader walkthrough. |
| Leptos visual | Opened current rendered light/dark stills, narrow 320/390×844 and wide 1280×900 CSS viewports, 100/200% injected text and both densities. A workbench frame is not a browser viewport. Browser DPR 1, Chromium 156 Mac, fonts loaded; computed Floating family is Noto Sans. Latest manual facts: `2026-10-25T16:01:36.139Z`, 320×844, visible, document focused. Screenshots were opened in the CUA transcript; no local PNG export or invented file hash is claimed. |
| CMP structural/runtime | `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview cmp verify` PASS: **253 tests / 41 suites**, zero failures/errors/skips; actual desktopTest execution. Native production composables, keyboard/pointer actions, callbacks, semantics and controlled state, not screenshot-only mocks. |
| CMP accessibility | Named Compose semantics, disabled/read-only state, labels, selection, focus restoration and actual keyboard actions. No device/AT/OS-window acceptance. |
| CMP visual | 197 final-host PNGs, 133 sidecars; root and independent reviewer opened current Noto matrix. Independent reviewer reopened 94 changed images, including all 46 feedback/modal images, plus five byte-identical Logo diagnostics reused from an earlier opened set. No clipped glyph/label/focus ring or hidden focused editor remained in this reviewed matrix. |

Logs are retained under `artifacts/agent-runs/20261025-ui-qualification/`.
Final CMP captures are under `artifacts/ui-evidence/20261025-qualification-noto-final-host/`.
All 133 sidecars were independently checked against image hashes, byte lengths and dimensions.
There are 125 explicit Noto sidecars, one deliberate fallback, and seven unrecorded
(four Noto legacy helpers and three fallback pixel diagnostics). The other 64 legacy
PNGs inherit enclosing run attribution, not invented individual sidecars. Some behavior
tests intentionally retain platform fallback; **not all 253 tests are Noto tests**.
The new named-layout font assertion detects losing the injected family, but does not
independently prove every role assignment or detect swapping Sans and Serif.

The tested renderer digests were rechecked unchanged at the packaging boundary.
Some original captures truthfully identify a dirty working tree; the enclosing run
binds tested bytes to the committed source. Original sidecars were not rewritten.
JDK 23, Gradle 8.14.3, Rust 1.98.1 and macOS were used. Font bytes/provenance are separately
checked by PreviewFontsTest. Same-font defect before/after pairs are retained separately;
fallback-before versus Noto-after is not presented as a matched comparison.

## Per-component acceptance

Each row supplies the bounded runtime/accessibility/visual scope for both receipts.
Structural checks above cover each implementation. Ordinary control state tests,
blocked callbacks and pure motion policy remain distinct from observed pixels.

| Component | Leptos: actual current observations | CMP/JVM: executed tests and opened matrix |
| --- | --- | --- |
| Button | Ordinary/auth names and long Vietnamese wrapping; real Enter/Space produces two intents for two presses, loading disables all three auth controls; reduced sample motion preserves dispatch and edited signal. | Ordinary/auth variants, blocked dispatch, manual-clock press/reversal policy, long text and light/dark/large-text captures. |
| TextField | Regular/AuthTonal/Floating, long persistent label, help/error IDs and aria-invalid; synthetic signal retained through theme/density/text/reduced-motion/read-only/loading; read-only enabled, loading input+CTA disabled. Theatre independent trailing eyes and search Enter/click count checked. | Regular/AuthTonal/Floating/FloatingSearch, native editing, read-only selection, blocked edits/reasons, trailing-eye dispatch, label growth, error/focus pixel tests and Noto layouts. Single-line text can scroll horizontally; no full-string-at-once claim. |
| Select | Enhanced regular/compact-inline and native baseline. Unsupported current value retained on Tab/Escape with no callback; End/Enter explicitly selects dark once. Missing controlled value stays `missing`; empty options show no items and emit no callback. Reset restores fixture. Long Vietnamese popup scrolls at 200%; language autonyms and disabled option semantics inspected. | Stable values survive reorder, unavailable options refuse click, caller owns acceptance, missing/empty do not invent values; native popup Escape restores focus; loading closes without recovery reopen. Open/selected/error/missing/loading captures at320/390/840; 390 dark200 case is1000 high, not844. |
| Surface | Reading/auth/profile/membership and nested groups; paired foregrounds, light/dark, narrow reflow and wide200% three-column layout opened. | Same inspected compositions, injected themes/density/type, reading region and paired foreground captures. |
| Badge | Five tones opened in light/wide and dark320/200%; decorative icons and complete Vietnamese labels. | All five tones, wrapped200% text in both themes; corrected glyph paint checked against independent unshaped-text ink oracle and opened images. |
| BackButton | Existing IconTonal baseline only: Enter sends one named intent, disabled refuses activation, count retained; host owns destination. | Named destination/callback, blocked states, wrapping/focus inspected. Unified candidate excluded. |
| Logo | Local vector asset tones, aspect fit and transparent knockouts opened. All 12 img assets complete; decorative alt empty/aria-hidden, standalone symbol has explicit alt; object-fit contain and intrinsic dimensions verified. | Official tones/aspect/knockout diagnostics; five identical image bytes reused from actually opened prior captures. |
| Icon |37 official outlined glyphs; every gallery SVG decorative aria-hidden=true/focusable=false; adjacent text or owning control supplies the name. | Registry geometry and decorative semantics, adjacent text/control naming, current captures. |
| Checkbox | Space toggles false then true, selection count 1 then 2; named native input and retained value, blocked/error contract covered. | Native label/key selection, state semantics, blocked callbacks, large-text/focus capture. |
| Status | Five tones and long Vietnamese text opened; announcement off has no own live role, explicit polite loading has role=status; retry transitions only local fixture. | Five tones, announcement Off/Polite/Assertive semantics, retained text and large-text matrix opened. |
| ErrorState | Complete title/body/two recovery controls opened at390dark200 and wide; two deliberate actions increment recovery count exactly twice; own announcement opt-in distinct from host feedback. | Recovery dispatch, announcement modes, complete title/body/actions in four 320px scenarios (light/dark ×100/200%). |
| Dialog | Real native modal, large title/body scroll, reachable editor/Close/Done at320dark200, Escape/reopen retains caller text and restores opener. BODY observation was disambiguated by actual document blur while :modal remained true; no background-escape defect inferred from BODY alone. | Six scenarios / 12 full modal root captures at 320/840, light/dark,100/200%; actual keyboard editing, Escape/reopen/Close, callback count, opener restoration and complete focused ring after scroll. |

## Defects repaired and negative evidence

1. CMP Badge's proportional capsule clipped wrapped glyph paint. Independent ink control
failed before (2180/2209 foreground pixels, below99%) and passes with the existing bounded
reading radius. This is a named fixture oracle, not proof of every possible font/string.
2. CMP Dialog's fixed large title hid the editor and squeezed Close at320/200%.
Title/body now share the bounded scroll region; native dismissal action remains outside it.
3. Focused editor's outset ring was clipped by that scroller. A bottom-stroke pixel control
failed before; token-based clearance and scoped bring-into-view policy make it pass.
Nested editors keep the inherited policy. Full-root images were inspected after the fix.
4. Existing capture helpers omitted the desktop host's Noto injection. The test-only
capture root now uses the actual PreviewFonts composition, with explicit font attribution
and a runtime layout guard; no product font asset/API was changed.

The earlier Select recipe parser defect was repaired with red/green scope coverage;
unsupported-option now initializes/resets the disabled selected value in that specimen.

## Remaining boundaries

CMP canonical desktop preview launch was attempted, but the available UI binding cannot
select its actual OS window. OS-window input is UNSUPPORTED; executed Compose JVM input
and semantics are the bounded runtime layer accepted here. Android/iOS, screen readers,
real OS IME/autofill, forced colors, other browser engines, production pages/auth/audio,
performance and smoothness are NOT_RUN. These records do not turn those limits into PASS.
No #453/#454 composition is implemented, no merge or deployment is performed.

The repository/preflight/CI outcomes for the final receipt-bearing head are reported at
handoff. Creating a receipt is not itself evidence that those commands passed.
