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

## Coverage and navigation

Choose a complete representative journey first, then distinct navigation/overlay/state and
responsive variants. For “all UI”, keep every discovered in-scope screen/state in the expected
set even when it cannot run. Include component/catalog/scenario IDs where the repo defines
them; record source-only components separately from observed ones. Token provenance can link
the existing token owner/version/hash; do not infer effective token values from image colors.

`NOT_IMPLEMENTED` needs current source evidence of absence, rather than a failed URL or missing
login. Keep a reproducible app error as `CAPTURED` with state `error`. Blocked coverage is not
a P0–P3 finding. Preserve unknowns instead of inventing fixture accounts or routes.

Method references inspected on 2026-11-04:
[Vink capture](https://github.com/loveoverflowcom/vink/blob/develop/.agents/skills/vink-ui-capture/SKILL.md)
and [Vink Drive evidence](https://github.com/loveoverflowcom/vink/blob/develop/.agents/skills/vink-ui-evidence-drive/SKILL.md).
Cantos source and owning docs determine its commands and platform scope.
