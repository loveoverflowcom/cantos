# CMP mapping — deferred

This directory contains documentation only. There is no Kotlin module, Gradle project,
generated theme or native build claim in this delivery.

Future Theatre/Studio native owners consume the semantic roles in
[`shared/tokens.json`](../shared/tokens.json), not CSS lengths, Rust components or a WebView.
Map colors to Material ColorScheme and extensions, typography to Vietnamese-safe sp values,
spacing/shapes to dp, and motion intents to native specs plus platform reduced-motion preference.
The 48-unit comfortable Web profile is an intent reference, not proof of native target sizing.
Android/iOS safe areas, focus, large text, native Back and platform audio adapters remain governed
by [mobile architecture](../../../docs/architecture/mobile.md).

Only add a shared Kotlin module with real mobile consumers and a reviewed mapping. Do not
duplicate palettes per app. Native navigation and playback remain app/platform responsibilities;
shared tokens do not imply mobile runtime verification.
