# Theatre CMP focus perimeter — issue #478 (26 October 2026)

Source: branch `fix/478-theatre-cmp-focus-perimeter` from `origin/develop` `e62a6084`. Renderer: **JVM Desktop
Compose UI tests only**. Android, iOS, TalkBack/VoiceOver, a real IME, a recorded video and a pixel match against
the Drive v4 handoffs are `NOT_RUN` (the linked Drive files were not readable from this session).

## Observed gap (before)

`cantosFocus` paints a ring `size.focusOffset` (4 dp) outside the node plus a 3 dp stroke: **7 dp of focus paint
beyond the bounds**, with a 4 dp page-coloured gap between the control and the ring. The new pixel test fails on
the old sources at the "nothing outside the bounds" assertion (mutation-checked) and a Standard `Button` capture
shows the gap. Separately, the focus hue is the primary fill's own hue (`color.focus` on `color.primary` is
1.05:1 light / 1.01:1 dark Standard, 1.34:1 / 1.22:1 Theatre), so a plain inset ring would vanish on `Primary` and
`AuthPrimary`; a selected Theatre chip is 2.0:1 / 1.32:1.

## Inventory and classification

| Consumer | Class | Decision |
|---|---|---|
| `Button` / `BackButton` auth variants (`AuthPrimary/Secondary/Outlined`) | CONFIRMED | inset perimeter on the animated auth shape; every Theatre app auth/account/verify/logout action uses it |
| `Button` every variant under the Theatre expression | CONFIRMED | inset perimeter on `shape.control` |
| `IconButton` (Text/Tonal/Primary, circular or not) under Theatre | CONFIRMED | inset; plain glyph button uses Material's circle |
| `FilterChip` under Theatre | CONFIRMED | inset on the animated chip shape; selected ink chip reads from `color.onPrimaryContainer` |
| `TextField` `AuthTonal` | CONFIRMED | inset on `shape.authField`: idle none, invalid 1 dp, focus 2 dp, focused-invalid 2 dp error colour |
| `TextField` `Floating` / `FloatingSearch`, `Select` (Theatre) | NO_GAP | already inset; `Select` now uses the shared shape-aware modifier, same geometry |
| `TextField` `Regular` | INTENTIONAL_EXCEPTION | Standard-expression field with an underline error indicator; no Theatre consumer |
| `Button`/`IconButton`/`FilterChip`/`NavItem` under Standard | INTENTIONAL_EXCEPTION | Standard keeps the outset ring (Studio/preview consumers) |
| `Link`, `NavTabs`, `Checkbox`/`Radio`/`Switch` rows, `RangeField`, menu items | INTENTIONAL_EXCEPTION | no painted surface of their own; the outset ring is the only perimeter and `cmp-choice-focus-spacing.md` reserves clearance for it |
| Account portrait (`AccountScreens.AccountAvatar`) | NO_GAP | decorative `Box`, not focusable; unchanged |
| Shell header account/theme/search/back (`CantosPresentation`) | NO_GAP | raw Material `IconButton`: native state layer inside its bounds, no `cantosFocus` |
| Player controls (`FilledIconButton`…) | NO_GAP | raw Material, same as above |
| Shell `LibraryEmpty` Discover `Button` | CONFIRMED, NOT CHANGED | the shell runs the **Standard** expression, so this primary action keeps the outset ring; fixing it needs the app to adopt `CantosExpression.Theatre` (colours/type remap) or a per-control opt-in — an owner decision |
| `AuthTonal` label/help gap includes `focusOffset + focusWidth` | UNKNOWN | kept: removing it moves form rhythm and needs the v4 spacing value |
| No interactive avatar entry exists in the shell | UNKNOWN | nothing to style; when one is added it must be a focusable node with `cantosInsetPerimeter(CircleShape)` |

## Change

`cantosInsetPerimeter(shape, invalid, active, ring, inward)` measures the node's real outline (so the pressed
auth capsule keeps the ring on its edge) and strokes inside the bounds; the `Dp` overload delegates to it.
`ButtonControl`, `IconButton`, the Theatre `FilterChip` branch and `AuthTonal` opt in; `cantosFocus` keeps its
outset geometry and loses the unused `restingBorder`. `Primary`/`AuthPrimary` and the selected chip draw the ring
in their content colour, one 2 dp stroke inside the fill. No token, public API, layout, hit area or semantic changed.

## Evidence

| Claim | Oracle | Level | Result |
|---|---|---|---|
| Stroke geometry stays inside bounds; corner follows the outline; stadium cap; inward offset | `FocusPerimeterContractTest` (pure) | unit | PASS |
| Ring has ≥3:1 against its fill for every variant × theme × expression; focus hue fails on primary (premise) | same, independent WCAG math over the tokens | unit | PASS |
| No focus pixel outside the bounds, ring on the edge in the right colour, node bounds unchanged, 15 scenarios × light/dark | `FocusPerimeterPixelTest` | JVM pixel-tested | PASS; fails on the old sources |
| 2× text keeps the same | same, 4 scenarios | JVM pixel-tested | PASS |
| Ring follows the pressed auth corner (not a capsule) | same, reduced-motion press, red backdrop | JVM pixel-tested | PASS; fails with a capsule ring |
| Outset exceptions are deliberate | same, Standard `Button` and `Link` | characterisation | PASS |
| `AuthTonal` idle/invalid/focused/focused-invalid strokes | `PendingRingAndFieldIndicatorTest` (rewritten for v4) | JVM pixel-tested | PASS |
| Whole CMP suite, Theatre mobile suite | `gradle desktopTest` (268 tests), `cargo xtask theatre-preview verify` | JVM | PASS |

`AuthActionMotionTest` measured the painted capsule corner with the old outward ring beside it; the inset ring is
the same colour as that scan's background, so it now clears focus before each captured frame.

Opened captures (`build/reports/cantos-preview/cmp-focus-478-*`, light and dark, 390 dp wide, density 1): one ring
per control hugs its edge with no page-coloured gap; the primary ring is visible on both themes. Not opened: 320 dp,
2× text images (asserted numerically only), Vietnamese long labels beyond the specimens above.

## Not covered

Android/iOS rendering, TalkBack/VoiceOver, keyboard focus on a real device, IME, Compose state-layer interplay on
hardware, Drive v4 handoff parity, the Web counterpart (#477) and the `LibraryEmpty` follow-up above.
