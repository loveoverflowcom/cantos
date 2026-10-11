# UI system migration

The user-authorized foundation branch starts at develop `dc7c2de`; no merge/cherry-pick from
`feat/client-shells-skeleton` is used. Backend, wire contracts, database, identity, TTS,
production gates and playback are outside this change.

| Before | Now | Compatibility |
| --- | --- | --- |
| `packages/design-tokens/` | `packages/ui-system/` | Package renamed; downstream file readers migrate explicitly |
| `studio-tokens-0.1.0.json` | `shared/legacy/studio-tokens-0.1.0.json` | Original bytes preserved; immutable historical artifact |
| Studio-only active JSON | `shared/tokens.json` v0.2.0 | Shared light/dark roles, type, shape, space, motion, density and breakpoints |
| `scripts/studio_tokens.py` | Compatibility entrypoint → `scripts/ui_tokens.py` | Existing generate/`--check` CLI continues; old Python internals are not a public API |
| `apps/cantos-studio/web/tokens.css` | `packages/ui-system/leptos/tokens.css` | Trunk link updated; no generated theme copy in each app |
| String theme state | `cantos_ui::Theme` | Existing system/light/dark selector behavior preserved |
| `.studio-shell` theme scope | `.studio-shell.cantos-ui` | Existing Studio CSS role consumers remain; shared selectors are scoped |

All eight old color variable names and seven dimensional names remain defined. `shape.field`
is an alias to `shape.reading`; spacing `sm/md/lg` remains 8/16/24. Public CSS names stay stable,
but values intentionally migrate to the selected paper/lavender source. Body size changes 16→14,
title 28→20 (headline stays28), reading radius4→12. These are design changes, not a byte-compatible
render promise. Apps must not redefine their own palettes to restore the previous appearance.

Theatre is still a skeleton and does not gain a catalog/player/demo production flow. Its first
integration will use the same source and renderer with task-appropriate density/layout.
CMP remains deferred, without a Kotlin build. Historical ADR/evidence commands record what ran
then and are not rewritten into claims of new verification.

Consumers to maintain: root Cargo workspace/lock; Studio WASM manifest, root theme and HTML CSS
link; repository required-path checks; token/icon tests; package/root/mobile docs; architecture
and design docs; canonical skill token reference. Session 3 will scope existing broad Studio
tag selectors and integrate shared components, rather than copying them into the app.
