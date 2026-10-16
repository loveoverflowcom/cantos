# Theatre native launcher icons — issue 190

Review boundary: generated Android launcher resources and iOS AppIcon source/binding for the
existing demo hosts. Base: `develop@ce9b1ad76bdf8ed7f9c4d99a87fb6e752179134f`.
This implements the native asset slice of [#190](https://github.com/loveoverflowcom/cantos/issues/190),
not complete cross-platform launcher acceptance. Related foundations are
[#127](https://github.com/loveoverflowcom/cantos/issues/127),
[#133](https://github.com/loveoverflowcom/cantos/issues/133),
[PR 135](https://github.com/loveoverflowcom/cantos/pull/135) and
[#143](https://github.com/loveoverflowcom/cantos/issues/143). Their closure/merge records explicitly
retain separate component and native acceptance; no new shared component is adopted here.

## Artwork and adaptation

The actual light/dark/mono SVGs were read, rasterized with installed librsvg 2.60.0 and opened on
the Mac before editing. Source of truth is the local checksum-pinned
[Logo Kit](../../packages/ui-system/shared/assets/cantos-logo-kit-v2/provenance.json) and its
[guide](../../packages/ui-system/shared/assets/cantos-logo-kit-v2/Logo-Guide.md), originally from the
[owner's Drive kit](https://drive.google.com/drive/folders/1nIIehbnR1t2sXD0tKsYTXyHwekNtVly7).
No Drive transfer, new logo drawing or inferred license was needed.

| Official source | SHA-256 |
| --- | --- |
| `cantos-symbol.svg` | `5845decd2d648d92be5a8d11dde69525c31e58058e0d8f5819804fb827941cbd` |
| `cantos-symbol-dark.svg` | `2a8e60f2bfda6b26ff0b409ee149e14e7bb1acb29ef23c3ed3b22bfdb351ce53` |
| `cantos-symbol-mono.svg` | `e85ce3316e6b2ebee9a9025cea8429e2296cabb8bf8be9884b384cf9e166ace0` |

The existing `scripts/ui_logos.py` generator now has a native launcher mode. Android uses an
opaque full-bleed kit background with the original C and three bar paths, scaled uniformly 1.2
and translated 15.6 within the 108dp canvas. An independent analytic envelope, including round
stroke caps, fits inside the central 66dp safe circle. The monochrome drawable paints the
official knockout's original C/bar strokes as positive tintable ink; it does not copy a
multicolor tile. The outer rounded rectangle is owned by the OS mask. This deliberate native
adaptation avoids a second rounded tile within the adaptive icon.

Both Android icon attributes use the same resource, with ten opaque legacy density/night PNGs
for the actual API23 floor and light/night API26 adaptive descriptors; supporting API33+ launchers can use
its monochrome layer. iOS has opaque RGB 1024px default/dark sources, no baked outer rounding,
and AppIcon binding in both target configurations. Custom iOS tinted artwork is not supplied;
Xcode 16.3/OS tint acceptance remains pending. Platform references:
[Android adaptive icons](https://developer.android.com/develop/ui/compose/system/icon_design_adaptive),
[Apple asset catalogs](https://developer.apple.com/documentation/xcode/configuring-your-app-icon).

[Generated provenance](../../apps/cantos-theatre/mobile/launcher-icons.provenance.json) binds the
generator, source and output hashes. Regeneration requires existing `rsvg-convert 2.60.0`;
checks need only Python's standard library. No renderer/toolchain is installed automatically.
Generated resources must not be hand-edited.

## Independent evidence

Full logs, APK inspection, references and device captures are evidence-local-only under ignored
`artifacts/agent-runs/issue190/`. The PR owns the final checked commit identity and preflight
summary. No screenshots, caches, logs, private content or audio are committed.

| Claim / layer | Evidence and limit |
| --- | --- |
| L1 ownership | Native host resources only; same canonical brand source; no service/domain/API or shared component dependency added |
| L2 domain/types | Inapplicable: no durable decisions, state, permissions, identity or publication rule changes |
| L3 source/resources | Nine focused Python tests PASS: source hashes, exact paths/strokes/palette, safe circle, manifest references, legacy density inventory, iOS binding/size/opacity and stale-provenance rejection |
| Android package | `cargo xtask theatre-demo android-build`: PASS; aapt2 confirms icon/roundIcon resolve to the same resource, legacy/night density entries and adaptive/monochrome descriptors exist; original callback/singleTask/theme retained |
| iOS framework | `cargo xtask theatre-demo ios-build --kotlin-native-home /Users/manhblue/.konan/kotlin-native-prebuilt-macos-aarch64-2.4.20`: UNSUPPORTED; installed compiler is not required 2.2.20 |
| iOS catalog compile | `xcrun actool` simulator and device SDK attempts: FAIL (environment), `No available simulator runtimes for platform iphonesimulator`; partial PNG/info files are not a compiled catalog or app bundle |
| iOS runtime | BLOCKED / NOT_RUN: `simctl list runtimes` is empty; no iOS host build, installed bundle or Home/App Library/Settings observation |

The initial Android build and three XML-parser tests failed because a generated XML comment
contained a double hyphen in a command flag. The generator comment was corrected; focused tests
and the Android gate then passed. Real dark Settings inspection subsequently exposed a
resource-selection defect: night legacy PNGs outranked the unqualified adaptive descriptor.
A failing source regression reproduced the missing night-anydpi-v26 descriptor; the generator
now emits both appearance descriptors. Original failed logs and the square-icon screenshot
are retained, not relabeled as missing infrastructure. The unrelated existing Kotlin nullable-when warning was not changed.

Native launch configuration, Android splash vectors/themes, background-only iOS storyboard,
application owners, callback filters and first CMP readiness/navigation behavior are unchanged.
Existing source contracts remain in the repository suite. A source check alone does not prove
absence of runtime flash/double-splash.

## Opened native observations

Android 16/API36, existing `Medium_Phone_API_36` AVD, Pixel Launcher version16/code907,
1080×2400 pixels at420dpi, system font scale1.0. The AVD was started read-only, without snapshot
load/save, and stopped after capture. No original AVD data, account, launcher database or SDK
was deleted. The initial package lookup was empty; installation was fresh. The corrected APK
was subsequently installed with `-r` at the same version, preserving data. A version-code
upgrade and explicit launcher-cache corruption/recovery were not exercised.

Final APK SHA-256: `20855dbc33dcfc34f0c48ede21b5c8527256ba1367aa29a6a9fc2e0e78b00273`.
The final aapt2 inventory includes both `anydpi-v26` and `night-anydpi-v26` descriptors.

| Opened capture | Observation / evidence boundary |
| --- | --- |
| `source/cantos-symbol{,-dark,-mono}.png`, `native-source-review.png` | Official artwork, predicted circle/squircle and positive mono adaptations inspected; source evidence only |
| iOS generated default/dark 1024px PNGs | Original C/bar ratios, kit palettes and full-bleed corners inspected; no iOS masking/bundle claim |
| `launcher-home-initial.png` | Light Home prediction slot contains Cantos; the outer prediction ring is launcher chrome, not part of the asset |
| `launcher-drawer-light.png`, `launcher-settings-light.png` | Circle-masked light icon and Cantos label, complete C and three bars; those light resources are unchanged in the final APK |
| `launcher-settings-dark.png` → `launcher-settings-dark-final.png` | Same Settings scenario first exposed a square legacy fallback, then showed the corrected circle-masked dark icon |
| `launcher-drawer-dark-final.png` | Correct final dark palette and complete circle-masked mark, with readable Cantos label |
| `app-dark-final.png` | A real cold launch reaches the Vietnamese diagnostic guest Home; does not establish production data/audio or frame-by-frame splash quality |

System mask lookup returned a circle path. No second native mask or actual themed-mode toggle
was observed. Home dark prediction changed to another app after launcher refresh, so its capture
does not establish Cantos dark Home acceptance. A short launch recording exists but was not
frame-inspected; no flash, timing, reduced-motion or double-splash acceptance is inferred.
OS App Drawer/Settings label semantics say Cantos. Screen readers were not walked. Surface
intensity and shared component admission are inapplicable to OS-owned icon resources; no token,
component, focus, text layout or motion behavior was changed.

## Deferred acceptance and next review boundary

- Android: use the existing debug APK on API23–25 and a launcher supporting a second mask;
  inspect Home/drawer/Settings, both themes and themed mode. Prove fresh-install, version-update
  and launcher cache recovery separately. Analytic safe-zone evidence and predicted mask PNGs
  do not establish a second device mask.
- iOS: an operator must supply the separately managed installed Kotlin/Native 2.2.20 and iOS
  runtime, then run `cargo xtask theatre-demo ios-build --kotlin-native-home <installed-2.2.20>`,
  build the existing Xcode host, inspect its primary AppIcon bundle metadata and observe
  Home/App Library/Settings, default/dark/tinted and cached launch. No installation is authorized
  by this implementation. Next acceptance owner: unassigned.
- Native splash/first-frame regression, screen readers and production release isolation remain
  independently gated. No production Android host, native audio or public delivery is added.
  The missing cross-platform launcher acceptance prevents treating #190 as complete.

No ADR is needed: this is a platform resource binding using the existing official artwork and
platform masking contracts. No migration, Script IR/API, cache/audio revision, provider call,
rights/approval, token, localization or CMP component contract changes. Listener label remains
**Cantos**. Icon accessibility is OS-owned; no TalkBack/VoiceOver claim is made.
