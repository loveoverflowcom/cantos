# Native listening JVM capture evidence

These sanitized synthetic captures render the actual production `NativeCatalogScreen` and
`NativeSessionRuntime` composition through `NativeCatalogPresentationTest`. Catalog, playback
access and streaming decoder ports are explicit fakes. **No audio is produced.** This is JVM
Desktop Compose evidence, not Android/iOS runtime, touch, native audio or screen-reader acceptance.

The accepted repository command was `cargo xtask theatre-demo verify`; its final local output
is retained under ignored `artifacts/agent-runs/step2/cmp-final-verified.log` (`PASS`, both shared
and preview-sandbox suites executed). Earlier failures and the focused repair run remain in the
same ignored run folder. Captures were opened with `view_image` on 2026-10-18; every listed PNG
was inspected and its SHA-256 checked against its JSON sidecar before copying. PNG and semantics files are copied
byte-for-byte, without cropping, resizing or image editing. JSON copies add only a final newline
for repository text hygiene; their parsed values and PNG binding are unchanged. Private accounts, credentials,
signed media URLs and real manuscripts do not occur in this fixture.

## Matrix and observed states

All cells use `vi-VN` and synthetic Vietnamese title “Ánh đèn cuối sân khấu” / chapter
“Chương 1: Người dẫn chuyện trở về”. Viewport dimensions are logical dp; the JVM renderer here
uses 1 pixel per dp. Mini PNGs capture the entire host. Full-player PNGs capture the dialog node:
320x495 pixels at the 390x844 host, and 320x568 pixels at the 320x568 host.

| Viewport | Theme | Text scale | Captures / observations |
| --- | --- | --- | --- |
| 390x844 dp | Light | 1.0 | Mini reports playing with Pause/Open player and both navigation tabs; full reports paused, time 0:00/2:05, Play/skip/speed/Back all visible.|
| 390x844 dp | Dark | 1.0 | Same states, readable Vietnamese text and controls; full dialog has no vertical overflow.|
| 320x568 dp | Light | 2.0 | Mini title wraps to 3 lines and actions stack; navigation stays visible. Full dialog scrolls vertically; no horizontal label clipping.|
| 320x568 dp | Dark | 2.0 | Same large-text behavior. Full screenshot is scrolled 303 of 319 pixels; Back and all speed controls are visible, with title above viewport.|

The test asserts no `Playing` before explicit fake `EngineStarted`, including after `EngineReady`;
then drives mini/full pause, speed 1.5x, resume and navigation using the same single factory owner.
It scrolls each of 10 full-player controls into view and asserts at least 48x48 semantics bounds in
all 4 cells. Semantics identify buttons/tabs, localized Play/Pause labels, and seek’s “Điểm nghe”
name/range plus `SetProgress`. Screenshots alone do not establish those interactions.

## Target correction and preserved before pairs

The first opened regular-text full-player trees exposed 40 pixel-high Play, skip, speed and Back
button semantics, plus a 44 pixel-high seek node. The repair adds explicit 48 dp minimums to the
full controls and sizes the standard Material 3 thumb to make the slider’s own pointer/semantics
node 48 dp high. Retagging an outer layout was not used.

Final semantics: all regular-text buttons and seek are 48 pixels high; large-text buttons are 58
pixels and seek 48 pixels. Widths are at least 48 pixels. Mini controls remain 48 pixels at regular
text and 58 pixels at large text. No horizontal overflow or clipped Vietnamese labels was observed.

`before/` preserves two full-player PNG/JSON/semantics triples from before the target repair:
390x844 Light/text 1.0 and 320x568 Dark/text 2.0. The regular pair demonstrates 40→48 button height and
44→48 seek height. The small pair demonstrates 44→48 seek height while large-text buttons remain 58.
Both pairs have the same fixture, viewport, theme, scale, paused state and speed 1.5x. The small
before frame is scrolled 237/319 pixels, versus 303/319 in the final frame after the added target
traversal assertions; it is a geometry/semantics comparison, not an aligned pixel-diff baseline.
Partial title visibility at these intentional scroll positions is not fixed text clipping. The
test scrolls to and activates Back successfully.

These measurements follow [UI system accessibility targets](../../design/ui-system.md#accessibility-and-motion-acceptance).
No approved-reference pixel comparator or contrast measurement ran. TalkBack/VoiceOver, physical
touch, keyboard traversal, device safe areas, motion, background playback and real networking
remain outside this bundle.

## Hash binding

Each JSON sidecar names renderer, entrypoint, viewport, text scale, theme, locale, fake media and
its PNG’s `png_sha256`. The table also binds the corresponding `.semantics.txt` bytes. The
before PNG and semantics files remain byte-identical to the preserved local archive. Before
JSON copies also add only a final newline. Logs remain ignored.

| Set | PNG | PNG SHA-256 | Semantics SHA-256 |
| --- | --- | --- | --- |
| final | [native-dark-320x568-text2.0-full.png](native-dark-320x568-text2.0-full.png) | `a7f22dd0f38ec9f412d39ebe093c62a99add756e98edec749761ee658e8f6dff` | `1b1231f53aecc3ea0b211cf4c231f0b504d3d9c7576e6f30b664afaf04625203` |
| final | [native-dark-320x568-text2.0-mini.png](native-dark-320x568-text2.0-mini.png) | `b867b1a209ace388292eb3ab0c76ca3fd752366ceebc9cfdf60f4135dbc26cf6` | `7a1052b8ffee8aea244b6df79e61b787bcf580be10bdcfdf0be8fda27fcb2d61` |
| final | [native-dark-390x844-text1.0-full.png](native-dark-390x844-text1.0-full.png) | `ee044b5a21f528880bb0b83f98ba0c6243de9b9a747b66e515bc25659d534fea` | `29d3b798da085d6c289f6444f343f5e263fa20412dca37c6a2c27f6bd9ef34f7` |
| final | [native-dark-390x844-text1.0-mini.png](native-dark-390x844-text1.0-mini.png) | `b9bbc7426ea175bc178574c0ac8388c7fac40255ec159a1267fd5cbaceb06367` | `3cc48477cf83b9199b9c5dbbe1ead4f56fcf937ed0fbf69b2df2c2a35bc75525` |
| final | [native-light-320x568-text2.0-full.png](native-light-320x568-text2.0-full.png) | `57aa2d694873a5c6782f7c734d294f7b28b2faf597184703f2c285f3f8a239c2` | `2fde3ee5affd085ed1208d91fdbfd7eae7763d1c1ca0339c2b5fb7a021c715c5` |
| final | [native-light-320x568-text2.0-mini.png](native-light-320x568-text2.0-mini.png) | `b179287a64c5d5bb99fe6f9b7747ea1308135703e70ae0bbe7e47631613236b2` | `0e959e2b47bec7d308a51f58822ebadb7caf21edfcacae262b9102b629029d30` |
| final | [native-light-390x844-text1.0-full.png](native-light-390x844-text1.0-full.png) | `a0032deb338417274019ddaa9ade9aeb4a4a963402856f1d898ff09b19046eaa` | `62bde41fe63d41fa17e00f764253bcc2dc71e65f34e99a88f3fe26673851c54b` |
| final | [native-light-390x844-text1.0-mini.png](native-light-390x844-text1.0-mini.png) | `0a1681b84e260c775435067da0576296e952504306024a7a8c875387252b7dbf` | `27dfebac7019c86ed688cc4b7d2f0c05e4e3b41f55a0f2b79a09786e7e8167f2` |
| before | [before/native-light-390x844-text1.0-full.png](before/native-light-390x844-text1.0-full.png) | `7aa1679f54113b5f00404a1d603ba5130c9a061edadb742beb3f093fb75b6ef8` | `81f64ad954e36e00fb4f392099de5fd23e68256357b964962a1cefb71042672d` |
| before | [before/native-dark-320x568-text2.0-full.png](before/native-dark-320x568-text2.0-full.png) | `b85a7fab688bcc72e16faf2aed8c927ad22480630dcc3f987100d52a99f4adce` | `673703a59913b798f4266767e40e7cb6f6b7b04585ccf00921402fed4c1d9e72` |

The final capture inputs include the following source content. Captures do not embed a commit
identifier; the source files and these hashes bind the presentation/test patch in this change.

| Source | SHA-256 |
| --- | --- |
| `NativeCatalogScreen.kt` | `89e52c4c4c4252e252c29e232512ddaa8b4e2bd6f00abdc0ad53f432c2f40a9b` |
| `NativeCatalogPresentationTest.kt` | `2f3b285326e05aaeed0d09da316be7e02f4c2e47012956da1aa3f49de0f8706a` |
