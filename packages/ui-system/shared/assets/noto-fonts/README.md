# Local Noto font bundle

The Leptos component preview loads these complete, unmodified Noto Sans / Noto Serif 2.015
TrueType faces from its own origin. Regular (400) and bold (700) are the same bytes already
bundled by the [Theatre CMP mock](../../../../../apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/files/provenance.json).
The [provenance record](provenance.json) names that source package, each source path/hash and the
[SIL Open Font License 1.1](OFL.txt). The license permits bundling and embedding. No font
download, service credential or font installed on the developer's machine is needed.

[`preview.html`](../../../leptos/preview.html) copies the bundle to `fonts/` through Trunk;
[`fonts.css`](../../../leptos/preview/fonts.css) declares each face without `local()`. The shared
family roles remain Noto Sans UI / Noto Serif editorial with fallbacks. Product hosts choose
their own licensed font delivery; this change only wires the existing component preview.

The available static weights are 400 and 700. CSS requests for 500/600 follow the browser's
nearest-weight selection within these faces; this bundle does not claim separate medium or
semibold masters. Record the requested weight and renderer in evidence. Normal faces include
the precomposed Vietnamese range, Đ/đ, Ơ/ơ, Ư/ư, combining tone marks and OpenType positioning.
The repository gate checks that coverage and the exact bytes; it does not inspect glyph
placement, clipping or prove a font actually loaded in a browser.

For capture, explicitly load both families at 400 and 700 and await `document.fonts.ready`;
record `document.fonts.check`/loaded face status before opening screenshots. `font-display:
block` reduces fallback flash but is not a readiness oracle. Browser/network failure must
remain visible as missing font evidence. Default/largest text scale, long Vietnamese content
and both themes retain their separate runtime acceptance.
