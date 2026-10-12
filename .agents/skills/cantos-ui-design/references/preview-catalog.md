# Two-renderer component admission

The user-requested development apps live inside the shared renderer packages:
[`leptos/preview`](../../../../packages/ui-system/leptos/README.md) and
[`cmp`](../../../../packages/ui-system/cmp/README.md). They are diagnostics, not Studio/Theatre
routes, a listener implementation, or a production approval surface.

## Required sequence for a new shared component

1. Inspect [`shared/preview-catalog.json`](../../../../packages/ui-system/shared/preview-catalog.json)
   and the existing public APIs. Record one stable API name, component ID, family and the actual
   component states. States describe concrete host-driven scenarios, not invented props.
2. Declare the component in **both** previews. Implement actual examples and deterministic
   anatomy/state/callback tests. If a target is unsupported or platform-specific, state its
   reason explicitly; registration and an unsupported placeholder never count as a pass.
3. Keep every token in [`shared/tokens.json`](../../../../packages/ui-system/shared/tokens.json).
   Tokens retain semantic names. Inspect each affected name/value in both previews, including
   theme/density/alias behavior. Regenerate CSS/Rust/Kotlin and both catalogues with the existing
   generator primitives. Never patch generated files or duplicate native palettes.
4. Run `cargo xtask verify repository`, the affected Rust/Web gates, and
   `cargo xtask ui-preview cmp verify`. Use `cargo xtask ui-preview leptos run` and
   `cargo xtask ui-preview cmp run` for the runtime review. Inspect the actual design reference
   and rendered pixels, realistic Vietnamese text, narrow/wide space, light/dark, text scaling,
   keyboard/focus and relevant interrupted/repeated interactions. Desktop Compose evidence
   does not establish Android/iOS device behavior. A PNG is not a preview runtime.
5. Record fresh component evidence for **both** renderers before first real application use.
   The [catalog contract](../../../../packages/ui-system/shared/README.md#preview-registration-and-adoption)
   requires named structural/runtime/accessibility/visual checks, inspected reference/reviewer,
   artifacts, current token digest and source digests (including build inputs and the declared
   component/state contract). These records document review; the
   structural gate cannot authenticate a reviewer or compare design pixels on its own.
6. Only then import the new component in `apps/`. The repository gate rejects unregistered APIs,
   missing examples, generated drift, stale qualification and new import sites without both
   fresh records. Run `cargo xtask preflight --changed-only` before handoff.

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
