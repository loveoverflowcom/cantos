# Theatre CMP auth UI — issue #421 (24 October 2026)

Source: branch `fix/theatre-cmp-auth-ui-421` from `origin/develop` `486a27ee`. Renderer: **JVM Desktop
Compose UI tests only**. Android, iOS, TalkBack/VoiceOver, a recorded video and a real IME are `NOT_RUN`.

## What changed and why

| Symptom | Cause (observed in the code and a baseline render) | Change |
|---|---|---|
| Idle **Hủy đăng ký** | `RegistrationForm` rendered `RegisterCancel` in `Editing`, where `cancel()` only marks `Cancelled` and there is no request or mail to stop; Back and the Login link already leave | Cancel renders only in `Submitting`/`Waiting`; `RegistrationIntent.Cancel` and the controller are unchanged |
| Login vs Register fields differ | Login used Material `OutlinedTextField`; Register used shared `TextField(AuthTonal)` | Login email/password use the shared `AuthTonal` field; shared `TextField` gains an optional `visualTransformation`; locked states use `ReadOnly` so no third copy of the pending reason appears; field rhythm is `space.md` as in Register |
| Focus and error rings stacked | `cantosFocus` drew an outset ring while `Modifier.border` drew the error outline inside the bounds: two concentric strokes 4 dp apart | The error outline is painted by `cantosFocus` only while unfocused; focus shows the ring alone. Invalid state stays in the container fill, the message, and `error()` semantics |
| Loading ring never turned | `ActionGlyph` drew a fixed `drawArc` | `PendingRing` rotates once per `PendingRingTurnMillis` (1000 ms) under full motion; reduced motion keeps the stationary arc. Disabled/dedup, `stateDescription` and visible reason are unchanged |

`PendingRingTurnMillis` has no canonical `motion.*` token; it is a named constant like `SpatialStandardEasing`
until a reviewed token exists. The Leptos ring stays stationary and is not changed here (#223/#224 own Web).

## Evidence (all on the final diff)

| Claim | Oracle | Result |
|---|---|---|
| ring turns under full motion, returns near rest after a whole turn | `PendingRingAndFieldIndicatorTest` on the manual test clock (infinite animations need `autoAdvance = false` before composition) | PASS |
| ring is a painted, stationary arc under reduced motion | same, frames at 0/400/1700 ms identical and painted | PASS |
| focused invalid field has no error stroke and has the ring; unfocused keeps the error stroke | same, pixel line through the field | PASS; fails when the fix is reverted (mutation-checked) |
| Cancel absent while editing; present while waiting; Login and Register fields share height/width/semantics | `RegistrationPreviewUiTest` (loopback Identity fixture) | PASS |
| existing auth flows and every other suite | `cargo xtask preflight --changed-only` (runs `ui-preview cmp verify` and `theatre-preview verify`) | recorded in the PR description |

Opened images (JVM, density 1): baseline `field-light-390-focus-error` showed the double ring; after the change
the focused invalid field shows one ring; `register-Light-Vietnamese-390-text1.0` has no Cancel and matches the
Login fields in `login-failed-light-390x844`. The width 320/390, 1×/2× text and light/dark Register captures
are produced by `RegistrationPreviewUiTest`; the ring-motion frames are numeric, not a video.

## Not covered

Real Android/emulator rendering and animation, TalkBack, the soft keyboard/IME, iOS, Web rendering of the same
defects, and timing/frame pacing (manual clock only). The qualification records for the 11 CMP components were
re-run through `ui-preview cmp verify` and their source digest refreshed; no new matched-pixel or device review
was performed for components this change does not touch.
