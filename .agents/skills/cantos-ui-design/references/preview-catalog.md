# Component-first lookup and two-renderer admission

The user-requested development apps live inside the shared renderer packages:
[`leptos/preview`](../../../../packages/ui-system/leptos/README.md) and
[`cmp`](../../../../packages/ui-system/cmp/README.md). They are diagnostics, not Studio/Theatre
routes, a listener implementation, or a production approval surface.

## Lookup before product markup

1. Search the existing public APIs and
   [`shared/preview-catalog.json`](../../../../packages/ui-system/shared/preview-catalog.json)
   by stable ID, API, family, variant and state. Inspect the actual sample and contract rather
   than matching only its appearance. Record the consumer and the IDs/variants/states reused.
2. Reuse a fitting contract. If only a variant or state is missing, extend that existing owner
   with bounded typed inputs/intents and deterministic examples; preserve callbacks, controlled
   values, stable IDs and compatibility sites. Explain the missing behavior in the PR.
3. If no kind fits, classify it as shared primitive, composition, app-owned or platform-specific.
   Declare its stable ID, anatomy, variants, typed states/intents, intended consumer, owner and
   concrete preview scenario IDs **before** product markup/import. A planned specimen may use
   side-effect-free fixtures, but must state its reason and must not export an empty public API,
   invent backend fields or become qualified from a mockup.

The [owning UI contract](../../../../docs/design/ui-system.md#components-and-interaction-states)
defines the requirement. The existing galleries are the workbench; do not create a competing
gallery or hide a reusable button/field/card/palette in a product feature. Studio/Admin Web
compositions stay Web-owned and never scaffold Studio Mobile. Design/proposal status in the
[v3 inventory](../../../../packages/ui-system/shared/design-inventory-v3.json) is separate from
code registration and renderer qualification; adding a proposed specimen changes neither.

## Replay a preview scenario

Use the renderer package's implemented recipe contract, not an invented screenshot state flag:

| Host | Contract and launch path | Boundary |
|---|---|---|
| Leptos gallery | [scenario workbench](../../../../packages/ui-system/leptos/README.md#scenario-recipes-and-replay), `cargo xtask ui-preview leptos run` | browser presentation, no native evidence |
| CMP gallery | [scenario recipes and replay](../../../../packages/ui-system/cmp/README.md#scenario-recipes-and-replay), `cargo xtask ui-preview cmp run` | shared components on a JVM preview root |
| Theatre CMP composition | [desktop scenario workbench](../../../../apps/cantos-theatre/mobile/README.md#scenario-workbench), its documented `cargo xtask theatre-preview run` opt-in | the real `CantosTheatreApp`/`TheatreShell` over mock external ports and in-memory sessions, no durable storage or native audio |

Keep these steps in the evidence record:

1. Discover the component ID/API/variant/state in the existing catalog, then select a supported
   recipe and fixture. Restore through the host's URL or snapshot contract and check the restored
   environment and state. Record unknown/invalid recipe and schema-version outcomes; an import
   never authorizes a service, credential or private-content lookup or a fallback live adapter.
2. Choose catalog, isolated specimen or matrix for the claim. All modes use the production
   components and preview-host-owned state. Theme, density, scale and workbench filtering must
   retain edits; use explicit reset to restore the selected recipe. Record locale as an injected
   value; a control that selects a new localized fixture must name that seed reset explicitly.
   Record injected versus owned
   mounts, their disposal owner and any supported latency/fault preset. A matrix shows supported
   typed states, not a second component implementation or a collection of forced CSS classes.
3. Drive real keyboard/pointer input and inspect the intent log and final semantic state.
   Replay only supported typed intents; do not mutate a mock database or manufacture callbacks.
   Include press/release/cancel, rapid reversal, repeated dialog open/close, hidden/revealed
   samples and font wrapping where relevant. Confirm that callbacks commit before motion
   settles; blocked controls and canceled activation must not fabricate an intent.
4. Capture and **open** the affected states. Attribute source SHA/dirty state, recipe/version,
   component/variant, seed, ordered intents, snapshot schema, token/catalog digest, theme, locale, density, font readiness/scale,
   specimen frame width, actual CSS viewport/DPR and renderer/browser. Frame-width simulation
   is not a browser viewport or browser zoom. A missing target or recording remains explicitly
   unobserved; source/DOM assertions and still images do not demonstrate motion.
5. Rerun the identical recipe and input sequence after a fix, including normal/system and
   reduced-motion observations when affected. Open the new artifacts and record the recheck
   for each finding. For owner recreation, separately test A→B→A, reset/dispose during delayed
   work, injected-owner disposal and rejected stale callbacks with controlled time. A memory
   remount never proves OS process death. Diagnostics do not fill shared admission or product-flow
   evidence; [#122](https://github.com/loveoverflowcom/cantos/issues/122)'s production
   source/dependency and APK/AAB exclusion belongs to `cargo xtask theatre-host android-release`.

CMP workbench intent replay is separate from motion-frame replay. Motion recipes (inputs plus
observation marks at virtual times) run on a manual clock owned by the test, never by production UI
([motion.md § 7](motion.md#7-replay-recipes-on-a-virtual-clock)). The gallery offers no pause or
seek-to-frame controls. A sample exists only where a real transition does (today the Auth action
press and Theatre launch entrance); a planned sheet, dialog or mini ↔ full sample stays `planned`
until its consumer exists. Snapshot export contains versioned public recipe/fixture data and
bounded synthetic state only; scopes, controllers, services, paths, credentials and private media
are never serialized.

Keep recipe schema, available component states, presets and fallback details in the package
contract linked above. Recipes contain public fixture identifiers and bounded presentation
preferences only; never put tokens, signed URLs, private manuscripts or media in links. Use
existing `cargo xtask` commands for build/verify/run; any capture automation extends that command
surface rather than introducing a parallel runner. Planned sheets/player gestures remain
unsupported until a concrete consumer and its input/focus/cancellation policy exist.

## Required sequence for a new or extended shared component

1. Complete the lookup above and preserve the existing API/state baseline. Use concrete host-driven
   scenario IDs, not invented props or screenshot-only state flags.
2. Declare the component in **both** previews. Implement actual examples and deterministic
   anatomy/state/callback tests. If a target is unsupported or platform-specific, state its
   reason explicitly; registration and an unsupported placeholder never count as a pass.
3. Keep every token in [`shared/tokens.json`](../../../../packages/ui-system/shared/tokens.json).
   Tokens retain semantic names. Inspect each affected name/value in both previews, including
   theme/density/alias behavior. The host injects scoped Light/Dark/System preferences, density,
   locale, font scale, motion/gesture policy and immutable projections/intents; components do not
   read the OS or create services, ports, mock owners, network, audio or auth effects. Regenerate
   CSS/Rust/Kotlin and both catalogues with the existing
   generator primitives. Never patch generated files or duplicate native palettes.
4. At a coherent component checkpoint, run affected Rust/Web gates and
   `cargo xtask ui-preview cmp verify` under the foundation's
   [verification cadence](../../cantos-engineering/references/verification-cadence.md);
   reuse valid evidence and use handoff preflight for gates already in its plan.
   Use `cargo xtask ui-preview leptos run` and
   `cargo xtask ui-preview cmp run` for the runtime review. Inspect the actual design reference
   and rendered pixels, realistic Vietnamese text, narrow/wide space, light/dark, text scaling,
   keyboard/focus and relevant interrupted/repeated interactions. Record cancellation/reversal,
   keyboard alternatives to gestures and immediate dispatch separately from animation. Run the
   same scenario with reduced motion; input value and focus must survive theme/nested-theme
   changes. Desktop Compose evidence
   does not establish Android/iOS device behavior. A PNG is not a preview runtime.
5. Record fresh component evidence for **both** renderers before first real application use.
   The [catalog contract](../../../../packages/ui-system/shared/README.md#preview-registration-and-adoption)
   requires named structural/runtime/accessibility/visual checks, inspected reference/reviewer,
   artifacts, current token digest and source digests (including build inputs and the declared
   component/state contract). These records document review; the
   structural gate cannot authenticate a reviewer or compare design pixels on its own.
6. Only then import the new component in `apps/`. Record lookup IDs/APIs/variants, any missing-kind
   rationale, scenario IDs and the evidence/remaining blocker in the PR. The repository gate rejects unregistered APIs,
   missing examples, generated drift, stale qualification and new import sites without both
   fresh records. Complete the foundation's handoff checks without duplicating covered gates.

If any required check is `NOT_RUN`, `UNSUPPORTED`, failed or blocked, leave the component
unqualified and outside real app adoption. Report the exact blocker and resume the same scenario
when the needed renderer/tool is available. Never fill a PASS from a source test, static mockup,
the other renderer, or a passed build alone.

## Compatibility baseline

The catalogue freezes the exact Leptos import sites already present at develop
`209e9e905c680e7cecab831f2d8f7894656e14fe`. This preserves current apps without retroactively
claiming native/design qualification. Do not expand this list to bypass admission. A new
component or a new import site needs both fresh evidence records; an explicitly reviewed
compatibility migration must explain any baseline change.

## Limits of automated enforcement

The current inventory/adoption gate checks explicit shared-library imports and component
definitions. It is a conservative source check, not a Rust/Kotlin compiler, runtime semantics
oracle, screen-reader proof or visual-diff service. Keep imports explicit; wildcard/module-alias
imports are refused rather than allowed to hide adoption. Review indirect re-exports or wrapper
imports manually, and use the renderer's compiler/tests as independent evidence.

## Comparison evidence in admission

Consume an existing `comparison/report.json` under the
[reference-pairing owner](visual-review.md#10-reference-pairing-and-local-comparison).
Carry its baseline/policy hashes, opened context/detail images and region findings in each
renderer qualification record's existing visual evidence. The optional comparator now measures
pixels, while the inventory/adoption gate still does not authenticate a review or establish UX.
A metric PASS never fills runtime, accessibility, reference inspection or native acceptance.
A stale qualification must be rerun and reviewed; do not update hashes or baselines merely to
clear the structural gate.
