# Surface discovery and capture matrix

Use the [platform owner](../../../../docs/architecture/repository-layout.md#platform-scope),
current routes/source and the selected runtime as the inventory. The following is a discovery
checklist, not a claim that every screen or state is implemented.

| Surface | Candidate coverage | Execution and evidence owner |
| --- | --- | --- |
| `theatre-web` | guest entry, discovery/catalog, search, work/episode detail, Library, account/auth, full/mini-player, dialogs, loading/empty/error/unavailable | [Web app](../../../../apps/cantos-theatre/web/README.md), [Web testing](../../cantos-leptos-web/references/web-testing.md); browser on selected local/QA or requested `cantos.cc` deployment |
| `theatre-cmp` | native entry, discovery/search/detail, Library/account, full/mini-player, dialogs, offline/unavailable states; OS chrome/insets and large text where available | [Mobile app](../../../../apps/cantos-theatre/mobile/README.md), [CMP testing](../../cantos-cmp-mobile/references/cmp-testing.md); running production APK on a named Android emulator/device, iOS app on a named simulator/device |
| `studio-web` | dashboard/work list, work/chapter editor, instructions/production/private assets, forms and validation, processing/empty/error states, visible permission blockers | [Studio app](../../../../apps/cantos-studio/web/README.md), Web testing above; browser on selected local/QA or requested `studio.cantos.cc` deployment; permitted synthetic creator data |

For each Web surface include desktop, tablet and narrow responsive layouts, light/dark,
guest/authenticated roles where meaningful, and representative Vietnamese text. Record exact
sizes; useful initial widths are 1440, 768 and 390 CSS px, with 320 CSS px and 200% zoom for
affected reflow. These are sample points, not a substitute for the requested viewport set.
Record form focus/keyboard and overlays without claiming accessibility acceptance from a still.
No sample fixture authorizes public media, approval or publication.

For CMP choose available Android and iOS runtime cells separately, with OS/device, package ID,
version/build, installed artifact binding, orientation, logical size, physical pixels, density,
locale and OS font scale. Prefer the already installed attributable test build; a requested
new build uses the existing `theatre-host` lane, not a parallel Gradle wrapper. Install/reset,
fixture writes and playback require task scope permitting their effects. Missing device tools
are `UNSUPPORTED`; inaccessible test-account/fixture states are `BLOCKED`. An APK digest without
proof of which APK is running does not identify the installed UI. No native screen may be
replaced by a resized Web or desktop screenshot.

## Reuse existing hosts without promoting their evidence

- `cargo xtask ui-preview leptos run` serves the [Leptos gallery](../../../../packages/ui-system/leptos/README.md#styling-and-gallery).
  Use its selected registration and state, or a supported workbench recipe. Save the exact recipe
  URL and observed metadata when available. Use `cargo xtask ui-preview cmp run` for the
  [CMP Desktop gallery](../../../../packages/ui-system/cmp/README.md#preview-contents) and record
  the component/state and selected settings. Both galleries are `component-preview` targets.
- `cargo xtask theatre-preview verify` exercises the common/JVM desktop suites. Discover the
  current `DemoCaptureTest` in `preview-sandbox` and its `CANTOS_THEATRE_DEMO_CAPTURE_DIR`
  output, then preserve PNGs and sidecars. It is a mock desktop composition, not the APK.
- `cargo xtask ui-preview cmp verify` includes shared host captures such as
  `AuthAccountCaptureTest` and `PreviewCaptureTest`, with `CANTOS_UI_PREVIEW_CAPTURE_DIR` output.
  Inspect actual sources before assuming a test or environment variable still exists.
  `CANTOS_SOURCE_REVISION` is supplied provenance, not proof that a running app matches it.
- The optional [Compose preview procedure](../../cantos-cmp-mobile/references/compose-preview.md)
  owns `cargo xtask compose-preview`, Android CLI and IDE readiness. Theatre's Android preview
  wrappers were removed in #287; the shared `ui-system-cmp` host is separate. Do not resurrect
  removed wrappers solely for capture. Missing prerequisites remain `UNSUPPORTED`.
- Import original metadata/semantics and their hashes, recording missing values as unknown.
  Classify these artifacts under `previews/` with `evidence_kind=component-preview`,
  `mock-app` or `host-preview` and the exact renderer. Synthetic account text is not real auth.
  Dumped semantics are not asserted semantics; preview images never satisfy native coverage.

## Component and token evidence

Look up shared components and scenarios in the [preview catalog](../../../../packages/ui-system/shared/preview-catalog.json).
For a broad capture, inventory the supported scenarios by renderer, then choose variants for the
requested scope and budget. A proposed entry from a design gallery belongs to reference coverage.
Record each component's source owner, observed app uses and source-only uses separately. Give an
app-local component a stable source-based ID when it has no catalog registration.

Use the [canonical token source](../../../../packages/ui-system/shared/tokens.json) and
[renderer mappings](../../cantos-ui-design/references/tokens.md#3-names-and-mapping).
Record path, version and SHA-256 for the token source and catalog used for lookup. Keep that
checkout provenance separate from any digest the target actually reports. Choose roles that
matter to the captured component and state, such as a foreground/background pair, type scale,
control shape or reduced-motion override.

- **Leptos.** Inspect the exact element's computed property and trace it through the applicable
  declaration and generated CSS variable, including nested theme, density, local overrides and
  pseudo-element if relevant. A resolved custom property alone shows availability in that scope.
  Record usage only after tracing the property the component actually consumes.
- **CMP.** Trace the semantic role through the generated `CantosTokens` mapping and the local
  `CantosTheme` or Material role. Use values exposed by the selected runtime or existing
  diagnostic output for that component and scope. A preview swatch or a sidecar's token digest
  does not measure the native product component. Use source-only or unmeasured rows when the
  running target cannot expose a value.

For example, a Web button row may link the scoped `color.onPrimaryContainer` role to the
element's computed `color` after the matching rule is traced. A color sampled from the screenshot
alone establishes neither the role nor the active theme override. If the browser exposes the
computed `color` but the active rule cannot be traced, report the color as a renderer observation
and keep the semantic role unmeasured.

## Coverage and navigation

Choose a complete representative journey first, then distinct navigation/overlay/state and
responsive variants. For “all UI”, keep every discovered in-scope screen/state in the expected
set even when it cannot run. Include component/catalog/scenario IDs where the repo defines
them; record source-only components separately from observed ones.

`NOT_IMPLEMENTED` needs current source evidence of absence, rather than a failed URL or missing
login. Keep a reproducible app error as `CAPTURED` with state `error`. Blocked coverage is not
a P0–P3 finding. Preserve unknowns instead of inventing fixture accounts or routes.

Method references inspected on 2026-11-04:
[Vink capture](https://github.com/loveoverflowcom/vink/blob/develop/.agents/skills/vink-ui-capture/SKILL.md)
and [Vink Drive evidence](https://github.com/loveoverflowcom/vink/blob/develop/.agents/skills/vink-ui-evidence-drive/SKILL.md).
The report's component and token evidence adapts the [Vink bundle contract](https://github.com/loveoverflowcom/vink/blob/136c1fdfe5399436bbe02e653a29b7e797d6b155/.agents/skills/vink-ui-capture/references/evidence-contract.md).
Cantos source and owning docs determine its commands and platform scope.
