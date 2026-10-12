# CMP choice focus spacing

On 2026-10-12 the requester authorized a follow-up to
[PR43](https://github.com/loveoverflowcom/cantos/pull/43): separate the checkbox focus ring from
its error message, test the change, capture Android Compose Preview and publish to `develop`.
This accepts the bounded shared-component layout fix. Product qualification remains **0/29**;
the next adoption prerequisite is still native, reference and accessibility acceptance.

## Change and regression evidence

`ChoiceLabel` owns the full-label native target shared by Checkbox, Radio and Switch. Its ring
paints 7 dp outside the row (`size.focusOffset` 4 dp + `size.focusWidth` 3 dp), while the previous
supporting-text gap was only `space.xs` 4 dp. The supporting text box therefore began within the
ring's painted extent. The existing Android specimen's actual glyphs were very close to the ring.

The owner now reserves the 7 dp paint outset whenever a visible helper, error or disabled reason
exists, in addition to the normal 4 dp gap. The reservation is present before focus; empty messages
reserve no space. A nested message column preserves the existing gap between messages. All three
choice wrappers retain their native target, state, callbacks and existing semantic/token values.
No functional core was extracted: this is presentation layout in the existing Compose owner.

| Claim | Oracle and result | Evidence level |
| --- | --- | --- |
| Supporting text clears focus paint by at least the normal message gap | Production Checkbox/Radio/Switch bounds at 300 dp, light/dark, compact/comfortable, 1×/2× text: 24 cases PASS | JVM semantics-tested |
| Requesting focus keeps control bounds stable | Before/after bounds and actual focused state in the same regression | JVM interaction-tested |
| Blank helper/error reserves no supporting height | Production checkbox/control bounds equality | JVM semantics-tested |
| Narrow Android error and long Vietnamese label remain readable | Fresh same-scenario PNG opened; observed tree bounds/text asserted | screenshot-inspected; bounded static semantics-tested |

`InteractionComponentTest.choice_messages_leave_readable_space_beyond_the_focus_ring_without_focus_layout_shift`
was added before the fix. `cargo xtask ui-preview cmp verify` executed **77 tests, 1 failure** on
the old owner: the light/compact/1× checkbox failed the paint-clearance assertion. After the
layout fix the same command executed **77 tests, 0 failures/errors/skips**. The second regression
is `choices_without_visible_messages_do_not_reserve_supporting_space`.
Logs and original failing XML remain under ignored `target/cmp-focus-spacing/`.

## Design and fresh Android capture

The authenticated Drive references are saved outside Git in
`/Users/manh.pd1/Projects/Mine/CantosDesign/`:

- [Cantos Mobile v2](https://drive.google.com/file/d/1tOmQ2ADirE-ys9Qd_E_R2AWvWoxMBwcc/view),
  `Cantos-Mobile-v2.html`, SHA256 `162a4dbb9021ed9baecbe0caf92c1989c0961b033959a537bff1ae3420503560`.
- [Cantos Studio Mobile Preview](https://drive.google.com/file/d/1yE6y-1z86Of9v_j9lm4c2fOTNfcxkj_p/view),
  `Cantos-Studio-Mobile-Preview.html`, SHA256 `3e159a246e4954a5bfb88753c29bea647d8a9bfaff77eebdac5f747ce1dfeb13`.

The inspected Studio `#edit` reference at 372×840 has a separated functional focus outline and
quiet reading surfaces. The fix follows that spacing intent using the existing CMP tokens.
Reference evidence is `Studio-reference-focus-372x840.png` in that local folder; the scoped review
is `PR43-review-2026-10-12.md`. Full reference parity remains a separate qualification task.

`cargo xtask ui-preview cmp android-build` **PASS**ed with `compileDebugKotlinAndroid` executed.
The official Android CLI 1.0.16500706 connected to Android Studio Rabbit 1 / 2026.2.1, PID 13949,
with the isolated Android library READY. Gradle 8.14.3, full Microsoft JDK 17.0.19 and the existing
SDK compiled the shared common source. The existing annotation/fixture was retained:

```sh
cargo xtask compose-preview ui-system-cmp \
  --file packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/preview/AndroidPreviews.kt \
  --composable AndroidChoicesDarkLargePreview \
  --label choices-dark-large-vi-focus-spacing --pid 13949
```

Capture **PASS**: run `20261012T080523Z-d282e8ec15cd`, 372×1000 dp, dark, Vietnamese, font scale
2×, actual PNG 977×2625 px (density 2.625). The PNG was opened with an image-capable tool on
2026-10-12. The full checkbox label and three-line error are readable; the ring has clear space
above the error. Select and RangeField retain their labels/values. No clipping or overlap was
observed in this static scenario.

Compared with the identical pre-fix wrapper/fixture in run `20261012T063439Z-96c00a87a1c0`, the
checkbox row bottom stays at y=529 px; the error box moves from y=540 to y=558 px. Its clearance
after the outer ring is `(558 - 529) / 2.625 - 7 = 4.05 dp`. Separate assertions PASS for source
and output hashes, root/descendant bounds, seven localized/value strings and exactly one each
of CheckBox, Button and SeekBar. The observed text tree exposes classes, text and bounds;
accessibility actions/state are outside that format. Generated metadata flags remain unchanged.

Artifacts live under ignored
`artifacts/ui-evidence/compose-preview/ui-system-cmp/20261012T080523Z-d282e8ec15cd/`.
A byte-identical user-facing copy is saved as
`/Users/manh.pd1/Projects/Mine/CantosDesign/Android-choices-focus-spacing-2026-10-12.png`.

| Freshness input/output | SHA256 |
| --- | --- |
| Canonical tokens | `e9713d5ad276be53bf3a63b6ce72ad04d1dbf0b5918cbb759a39ea7c2c1f39a2` |
| CMP source including regression tests | `2597f45b0195be8b4e9533dd0c5f36837b4b74978a0ee8d9ef223b2b1ca08ca1` |
| Preview wrapper source | `77e25c4ffc08d3af55f5d32706639a0a23cd7ca3fec4ea97538d33d8ce23e5dd` |
| Android PNG | `8b048ca9e7213f2b9f00eeb80618a2e81e69bedf4175003676736a9aeb2d5c76` |
| Android semantics text | `f8217db34b247a5a71a9d913fd45ddad7f0e44e73d88653ccbee60c59bbde9c6` |

Capture metadata records base HEAD `773cd285b7ecd1b23aaf937ca7da779f5a8dd32d`, dirty source.
The hashes above bind the tested and captured code to this follow-up. Documentation is outside
the renderer digest. Final repository/preflight and publication results belong to its GitHub PR.

Residual: Android evidence uses the Studio host renderer. Device input, TalkBack/VoiceOver,
native IME, motion and product-app acceptance remain untested by this follow-up. Backend,
durability, publication and listening behavior are outside this layout-only change.
