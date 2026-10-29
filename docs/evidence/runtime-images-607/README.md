# Image source audit and static migration (#607)

Source-bound implementation record, not production or complete Media acceptance.
Base: `develop@78557c9d6abbf2737991fb5cbd60b577acb80083`. The PR carries the final source SHA,
preflight results and exact artifact provenance. The [image policy](../../design/runtime-images.md)
owns requirements; [ADR0052](../../decisions/0052-bundled-avif-decoding.md) records the candidate
decoder choices. No remote storage, Drive source, production seed or deployed cover was changed.

## Inventory and consumers

The [baseline source inventory](source-inventory.json) records path, classification, owner,
consumer, bytes and SHA-256 for all **144** tracked images: 71 SVG, 60 PNG, 9 JPG, 4 WebP.
The 44 PNG and 8 JPG under `docs/evidence` remain historical evidence; no blind conversion.
This is a Git/source inventory, not a claim about an installed app or remote storage.

| Source owner / consumer | Change / retained boundary |
| --- | --- |
| Theatre Web `assets`, Trunk copy-dir, `features/common.rs`, Library and membership preview | Two paintings and the transparent Library raster converted to actual AVIF; load paths/assertions/provenance updated. Decode/load drives `data-cover=shown` so static covers are visible under the existing reveal CSS. Errors retain typography or SVG recovery. |
| Theatre CMP `TheatreEmptyArtwork`, preview `PreviewArtwork` | Three AVIFs in raw `files/artwork`; background decode through host-injected adapters; vector recovery, decorative naming and cancellation preserved. No `.avif` drawable accessor is assumed. |
| Android launcher / manifest `@mipmap/ic_launcher` | Ten density PNGs removed; base/night XML preserves full-bleed source geometry for API23–25. Existing adaptive foreground/safe circle/monochrome resources remain for API26+. |
| iOS Xcode target / AppIcon | Both source PNGs and appiconset removed; `.icon` + filled SVG layers are bound to Resources and both target configurations. Strokes are expanded analytically because Icon Composer changed the original round caps. Xcode owns native masks and generated compatibility PNGs, explicitly permitted by the owner. |
| Server Media + storage/HTTP/seed tests | Three exact positive PNG/JPEG decoder fixtures retained as visible migration debt. Current Studio/Server/cover contracts still admit PNG/JPEG. They are not renamed, hidden, accepted as AVIF or presented as completed #607 Media migration. |
| Tooling / release producer | Existing repository gate scans actual signatures/suffixes and SVG references. Web inventory and new release archive production reject legacy/spoofed raster; AVIF gets `image/avif`. Release v2 readers retain old suffix compatibility for historical records, not new producer permission. |

After static conversion the source contains **135** images: 73 SVG, 7 AVIF, 46 PNG and 9 JPG.
Six AVIFs replace application rasters; the seventh is an independent synthetic test fixture.
The remaining 55 PNG/JPG are the 52 evidence images and 3 frozen Media fixtures. Native XML
and `.icon` descriptors are not included in those image-file counts.

## Conversion and visual evidence

[Conversion records](conversions.json) preserve old/new paths, actual hashes, byte lengths,
dimensions, encoder settings and alpha checks. Pillow 12.3.0/libavif 1.4.2 decoded every result.
Paintings use quality75, 4:4:4, speed4; Library uses quality100. The 1374×1145 Library alpha plane
is byte-identical to the source. No source hash is reused for changed bytes, and existing
design/mock rights records retain their scope.

Six source rasters total **2,391,628 → 1,495,829 bytes** (895,799 fewer, 37.45%). Some painting
derivatives are larger than their already-compressed WebP sources. This is byte accounting,
not browser startup, native RSS or cache performance evidence.

Opened local evidence under ignored `artifacts/agent-runs/607/`:

- `web-discover-light.jpg`: in-app browser, 1280×720, Vietnamese, Light; visible painting and
  mini-player thumbnail. DOM showed actual AVIF natural sizes and opacity1/`data-cover=shown`.
- `web-discover-dark-390.jpg`: 390×844, Vietnamese, Dark; visible cover/crop and no horizontal
  overflow. Existing title/navigation/accessibility names remain outside the decorative image.
- `web-library-avif.jpg`: the actual transparent AVIF opened and decoded at 1374×1145.
  This is image decoding evidence, **not** authenticated live-Library acceptance.
- `launcher-outlined-comparison.png`: original launcher vs Xcode-generated output; round
  silhouette restored after replacing the unsupported stroke representation with filled paths.

The local Trunk response returned HTTP200, `Content-Type: image/avif`, length464587 for Library.
No deployed headers/CDN or production cache claim follows. Browser image-error recovery,
200% text, screen-reader and full live-account flow acceptance remain separate.

## Platform capability and qualification matrix

| Target / owner | Implemented path / observed boundary | Remaining acceptance |
| --- | --- | --- |
| Web in-app Chromium | Real AVIF paints opened at wide Light/narrow Dark; Library bytes decode. Existing SVG/typographic recovery is wired to errors. | Other browser engines, browser fault injection, live authenticated Library and deployed MIME/cache: NOT_RUN. |
| CMP Desktop macOS arm64 | Existing Skia-only resource test actually failed on AVIF; the pinned desktop JNI adapter decodes the real bundled paintings. Final suites/results are in the PR. | Other JVM OS/architectures and rendered Desktop Preview interaction/motion/a11y: NOT_RUN. |
| Android API23–30 | Official AOMedia AAR declares minSdk21; single-frame, ≤4096px/16MP plan precedes JNI pixel decode. One lazy 8MiB input buffer and one software decode; output ≤1536px. | Actual API23/old-device JNI, pixel/alpha, memory-pressure and native runtime acceptance: NOT_RUN. No total native-RSS claim. |
| Android API31–35 | Platform baseline decoder first, bounded software fallback; minSdk remains23. Source/build inspection is independent from device decoding. | OS/hardware decoding, pixel/alpha and accessibility/device runs: NOT_RUN. |
| Android launcher / APK/AAB | Vector XML and adaptive XML compile. Local optimized artifacts inspected during development contained the Library AVIF and no legacy raster paths. Final source-bound build/inspection is recorded in the PR. | Launcher appearance on actual old/new launchers and devices: NOT_RUN. |
| iOS ImageIO, deployment target15 | Platform-specific ImageIO adapter and vector recovery; no Skia AVIF assumption or minimum-OS change. | Framework command reports UNSUPPORTED: installed Kotlin/Native is2.4.20, required2.2.20 absent. ImageIO bridge, iOS15 recovery and newer-device pixels/alpha: NOT_RUN. |
| iOS launcher / Xcode26.6 | Actual `actool` compile of `.icon` + filled SVG for simulator/iOS15 floor succeeded; emits Assets.car and 2 compatibility PNGs. | Full Xcode app/installation/device appearances and older Xcode: NOT_RUN/unsupported respectively. Compiling icons is not an iOS app build. |
| Studio/Server/private/public storage + seeds | Audited actual code/constraints/contracts; no MIME relabel or immutable receipt rewrite. | Coupled Media transition and owner resolution with #608, real byte/SQL/upload/read/delivery qualification: NOT_RUN. |

Primary capability references: [Android image formats](https://developer.android.com/media/platform/supported-formats),
[Compose resource usage](https://kotlinlang.org/docs/multiplatform/compose-multiplatform-resources-usage.html),
[Apple Icon Composer](https://developer.apple.com/documentation/xcode/creating-your-app-icon-using-icon-composer),
[AOMedia Android JNI and its Maven release location](https://github.com/AOMediaCodec/libavif/tree/main/android_jni).
Package inspection is evidence for the named package, not any device or hardware-acceleration guarantee.

## Guard sensitivity and handoff

Running the new source guard against the clean base detected **18 actual source violations**
(6 UI rasters + 12 launcher PNGs), plus the separately classified 3 Media migration inputs.
Focused regressions reject real PNG/JPEG/WebP bytes under AVIF/other suffixes, legacy images
inside SVG, invalid/sequence AVIF signatures and new fixture debt. Tests preserve explicit
documentation screenshots and reject edits to the three frozen inputs. New archive production
rejects renamed legacy bytes **before creating an output archive**.

The final PR reports gate statuses as printed, including any failures and environment gaps.
Earlier Android attestation attempts refused concurrent source edits even though compilation
passed; only a final stable snapshot can supply its source-bound evidence. Python requires3.11+
and macOS temporary tests use `TMPDIR=/private/tmp`. No gate, fixture or evidence here accepts
production Media migration, grants public rights or closes either #607 or #608.
