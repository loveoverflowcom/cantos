# Cantos UI system

The implemented Web foundation follows the shared [product UI requirements](../../docs/design/ui-system.md).
One canonical [token source](shared/tokens.json) serves Cantos Studio and Cantos Theatre.
Studio and Theatre Web consume its controls, CSS and typed theme; Theatre CMP consumes the
separate Compose renderer. Product apps own live catalog, session, Library and playback adapters.
Preview qualification remains distinct from browser/device and deployed acceptance.
This is a small renderer package, not a second application or a domain authority.

| Owner | Contents | State |
| --- | --- | --- |
| [`shared`](shared/README.md) | Versioned semantic tokens, schema contract, licensed icon originals and provenance | Implemented |
| [`leptos`](leptos/README.md) | `cantos-ui` on Leptos 0.7.8; generated CSS/Rust; UiRoot, composable controls/layout/feedback, native dialog and decorative Icon | Implemented components and development gallery |
| [`cmp`](cmp/README.md) | Generated native tokens/icons, reusable Compose controls and JVM desktop preview | Implemented diagnostic source; target evidence recorded separately |

Generate with `python3 scripts/ui_tokens.py`, `python3 scripts/ui_icons.py` and
`python3 scripts/ui_preview_catalog.py`; append `--check`
to detect missing outputs or drift. Generated files are committed, so consumers do not run Python
during Trunk builds. The old `python3 scripts/studio_tokens.py` command forwards to the new generator.
See [migration](../../docs/architecture/ui-system-migration.md) and the
[four-session sequence](../../docs/work-plan/015-leptos-ui-system.md).

## Two diagnostic apps

Use `cargo xtask ui-preview leptos build|verify|run` for the existing Leptos gallery, now with
searchable component/state and named-token catalogues. Use `cargo xtask ui-preview cmp
build|verify|run` for the separate Compose desktop app. They share declarations and semantic
values; they share no UI runtime. Both remain under UI System and call no service or provider.

New components must be declared and individually tested in **both** previews before real app
adoption. [Registration and qualification](shared/README.md#preview-registration-and-adoption)
separate source parity from actual runtime, accessibility and inspected design-reference evidence.
An unsupported/native-only difference needs an explicit reason, never a fabricated counterpart pass.
The repository gate guards new explicit imports while retaining exact current Leptos import sites.
No newly native component is approved merely because its source or pure core compiles.

## Shared primitives and application patterns

Primitives own anatomy, semantic HTML, visual states, theme/density mapping and input dispatch.
Apps own localized labels, stable IDs, state, routes, asynchronous effects, form validation,
focus coordination across routes and permission explanations. Revision context, casting,
production status, QC findings, exact-render approval and publication are application patterns.
No UI primitive decides those gates, imports a service, calls a provider or starts playback.

Components use typed variants, reactive `Signal` props, `Children` for one-time content and
`Callback` events. Named repeatable slots may use `ViewFn` when a real component needs them;
session 1 does not add an unused slot abstraction. State and events must describe observable UI
intent; async application decisions stay outside components. No component owns browser storage.

CSS variables use `--cantos-<kebab-role>`. Shared selectors live under `.cantos-ui`, component
classes use `cui-`, and state styles follow native attributes or named `data-*` variants. There
are no shared global tag selectors or component-specific dark palettes. Load shared CSS before
app CSS. Each app must scope its own broad selectors when integrating components.

Button children provide visible localized text; SVG icons are decorative. A disabled Button
requires non-empty `DisabledReason`, rendered visibly and linked through `aria-describedby`.
Native disabled behavior and a callback guard prevent activation. Buttons are `type="button"`;
native Link/NavItem own navigation; submit flows remain app-owned. Callers provide unique IDs and
must keep the generated `<id>-reason` ID available. Focus rings, sufficient hit areas, text
wrapping, forced-colors and reduced motion are part of the foundation contract.

Documented API and compile/string-render checks do not prove keyboard, screen-reader, responsive
appearance or animation behavior. Use the [evidence record](../../docs/evidence/ui-system-foundation.md)
for the exact boundary of this delivery.

The [Leptos API/state contract](leptos/README.md) records controlled field values (including numeric
zero), safe class/description hooks, composition events, native navigation and dialog ownership.
The [component evidence](../../docs/evidence/ui-system-components.md) and its session 3 handoff
separate source/string-render/build results from actual browser observations.
