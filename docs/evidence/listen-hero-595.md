# Listen hero — issue 595

Implementation and bounded renderer evidence, 28 October 2026. Base `develop@cdab25b54e052cb9d250681d1bb9bd9b4bf0caef`.
This record distinguishes the shared preview admission from product and native-device acceptance.

## Reference and boundary

The actual [Drive design folder](https://drive.google.com/drive/folders/1tYuvM9jwBoyw1f6k16ApjzVLlS7bTABK),
handoff, ZIP, board PNG and self-contained HTML were downloaded and read. ZIP SHA-256:
`a1472f8613a321810022d09f93fe08b00c065005fb08648bb14b7d51dfa9e863`; handoff SHA-256:
`299bef6540cbd7225cc458821fe885d072bb7d6f70d31c4b07b702acc9870bdb`.
Those Chromium concept images are design references, never product playback evidence.

Work detail has Medium intensity: Listen is primary, Saved secondary. Reuse catalog `button` /
`Button`, add `ListenHero`, keep compact `IconButton` and regular/auth buttons unchanged.
Use registry PlayArrow/Pause/Refresh and the existing LoadingIndicator; Wave SVGs remain references.
Token v0.12.0 adds seven scoped geometry/type roles; color roles are the existing Theatre
primary/onPrimary, disabled, focus and state-layer pairs. No API, DB, auth, player reducer,
native media adapter, full/mini-player redesign or deployment changes.

## Shared admission before adoption

Reviewer: Codex. Commands run at this checkout root with the pinned Rust 1.98.1, Gradle 8.14.3,
installed JDK 23 and a task-owned target directory. Full logs and captures are retained under
`/Users/manhblue/Documents/Codex/2026-10-29/task-2/evidence/listen-hero`.

| Claim | Actual evidence | Result and limit |
| --- | --- | --- |
| Shared Web structure | `cargo xtask ui-preview leptos verify`: native DOM tests, WASM clippy with warnings denied, actual Trunk gallery build | PASS; subsequent CSS-only nested text-scale correction rebuilt with `cargo xtask ui-preview leptos build` and inspected in the same recipe |
| Shared CMP structure/runtime | `cargo xtask ui-preview cmp verify`, 317 tests, zero skips/failures | PASS on JVM Desktop Compose test root; no device or OS-window claim |
| Stable focus and intent | Web trusted Enter: Ready→Resolving; repeated Enter leaves count 1 and the same active native button. Space: Buffering→Paused retains focus and count 1. JVM focus/request/callback test covers the same enabled→pending transition | PASS, accessibility-checked / semantics-tested; no screen-reader claim |
| Anatomy | Actual Web 208×56, glyph24, well36, 3px focus / 4px offset. CMP geometry test asserts 208×56 minimum and regular height48 | PASS; minimums grow with text |
| Paired visuals | Opened Web Light/Dark recipe images and 42 actual CMP PNGs/sidecars: seven states × both themes × (320dp normal, 320dp text2, 390dp text2). Six contact sheets were opened after individual initial captures | PASS for inspected specimens, no exact pixel-match or native-device fidelity claim |
| Reduced motion | Browser recipe `motion=reduced`: mounted MicroArc animation is `none`, reason remains visible; CMP injected reduced motion in captures/tests | PASS for injected renderer preference; OS binding and frame pacing NOT_RUN |

The Web recipe is `component=listen-hero`, states ready/resolving/buffering/playing/paused/failed/unavailable,
mode isolated/matrix, themes light/dark, vi/en, frame320/390 and text100/200. Browser viewport
390×844 is separate from the requested specimen frame; the diagnostic frame deliberately has its
own horizontal scroll. The final 200% recheck measured CTA font36px, line52px, body32px and well36px;
the paused label grows the control to about255px without clipping. This is text stress, not true browser zoom.
Browser screenshots were opened in the CUA session; CMP PNGs and their SHA sidecars remain in the
task evidence and the package's generated capture directory.

Same-scenario repairs: pending paint changes atomically to the disabled color pair instead of
passing through a low-contrast color-transition frame; the Web preview explicitly replaces,
rather than doubles again, the text multiplier inside its nested Theatre theme. Both were rechecked.
No shared qualification is inferred from the HTML concept or from another renderer.

## Product behavior and independent checks

The Web projection keeps listener intent separate from observed status, uses the exact release,
and dispatches through the existing root PlayerSession. A buffered/pending Play offers an
operable Pause; only Status::Playing yields Playing feedback. Resume requires a nonzero position
on the same loaded release. Resolve and transport failure retry their actual owners.

CMP receives a source-bound WorkListenRelease from the actual catalog/playback/request owners;
selected episode or transportRequested alone is insufficient. OpenEpisode and the existing native
transport dispatch remain the action path. Simulation remains inside the preview sandbox.
Rapid repeated OpenEpisode for the same pending immutable release is guarded at the shell owner,
including before recomposition. Snapshot decoding derives the new presentation fact from the
same declared synthetic fixture, keeping its existing schema and no-autoplay restore behavior.

The final review found and repaired two identity/availability edges: a prior publication's
pause observation cannot clear a pending new publication of the same unit, and a manifest
NotFound (CMP also IdentityChanged) disables the hero with a visible reason instead of offering
a transport retry. A failed list has its own localized explanation. Independent expected vectors
exercise these outcomes, together with the suspended real-owner manifest request counter.

The actual Work composition was opened as six JVM Compose captures, Light/Dark × (320dp normal,
320dp text2, 390dp normal), under `mobile/shared/build/reports/listen-595`. These use injected
immutable presentation facts and real shared controls, without a decoder or backend. Native
semantics tests exercise VI/EN names, resolve/Pause/Resume/retry/unavailable actions and no autoplay.

The product Web WASM bundle was built with a task-local HTTP/media double. Its first Work request
met an incorrect fixture metadata ID and showed the normal access error dialog. The fixture ID
was corrected. The browser then lost its connection; after recovery, reload was blocked at the
browser-generated `data:text/html` connection-error document, including one retry after the verified
loopback server restart. The product Work→resolve→buffer→engine-playing→pause→resume and
retry/unavailable browser flow is **NOT_RUN**, not inferred from the gallery or a native test.
No bypass, alternative browser route, real Server/Identity/CDN or deployed playback was attempted.

## Loading regression admission

Adding a Button variant invalidates the conservative whole-renderer qualification digest.
This change rechecks the previously current loading-state, edge-progress, surface-pulse and
filter-chip scopes, preserving the older records and all older stale qualifications. It does
not update unrelated evidence to make the repository gate green.

The task's `shared-regression-invariants.json` checks byte-identical Leptos feedback/navigation,
CMP Feedback/LoadingIndicator and the FilterChip region, unchanged EdgeProgress recipe,
all 99 existing token definitions and all 33 non-Button component declarations against the base.
The fresh full shared tests and gallery build accompany those delta checks. CUA trusted controls
rechecked EdgeProgress retained content and reserved 2px edge; native Space toggled FilterChip,
kept focus and a 3px ring; LoadingState retained visible polite status; SurfacePulse stayed
aria-hidden, opacity0.7 and zero-duration under reduced motion, then revealed actual text.
Light/Dark gallery images and six fresh CMP loading/edge/chip captures were opened. These are
bounded renderer regression admissions, without expanding native-device or AT acceptance.
The final gallery reload also repeated ListenHero Enter→resolving (one intent after repeated
native Enter) and buffering→Space→paused with focus preserved on the actual rebuilt bundle.

## Independent checks and review

All commands use this checkout root and the task environment wrapper recorded with the logs.
Python suite comparison uses canonical `TMPDIR=/private/tmp/cantos-595-python` and the same
unsandboxed loopback/subprocess permissions on an exact base archive and this branch: both run
1319 tests, with 1299 pass, 18 skips and the same two failures (`test_vps_epoch2_cutover` BSD sed
and `test_vps_oidc_reconcile` missing flock). This is a failing broad gate, not a PASS.
The first sandboxed attempts additionally failed on macOS temporary-path aliases and denied
loopback/process-group operations; those are retained as failed attempts. Focused localization
checks after the final copy change run 89 tests, 87 pass and 2 named skips.

Code review covered the shared API/selector boundary, source-bound action mapping, duplicate
resolve, callback focus, localization ownership, generated outputs, unchanged ordering/session/
Saved/progress paths and preview-versus-production separation. All identified in-scope findings
were repaired and rechecked. No playback reducer, wire format, DB, native media adapter or full/
mini-player redesign changed. Source and renderer hashes, final gate results and terminal CI
are retained in the PR and task handoff. A baseline comparison never turns a failing gate green.

## Outstanding acceptance

Android/iOS runtime, native OS-window input (the Mac was locked), actual TalkBack/VoiceOver,
OS motion/lifecycle, touch/IME, true browser zoom, real Server/Identity/CDN playback and deployed
behavior are NOT_RUN. JVM captures, injected HTTP/media fixtures and HTML concepts cannot fill them.
No unavailable target is recorded as PASS. Issue acceptance remains open wherever it requires those targets.


Final focused results at the source delivered by this PR:

| Command | Target/result |
| --- | --- |
| `cargo test -p cantos-theatre --lib --features preview` | Native Rust: 552 PASS, zero failures/skips |
| `cargo clippy -p cantos-theatre --target wasm32-unknown-unknown --all-targets --features preview -- -D warnings` | WASM source lint: PASS |
| `cargo xtask ui-preview leptos verify` | 167 native/DOM/doc cases PASS, WASM lint and gallery build PASS; final copy-owner move additionally rechecked 54 action/gallery cases and rebuilt/reloaded the actual gallery |
| `cargo xtask ui-preview cmp verify` | 317 JVM cases PASS, zero failures/skips |
| `cargo xtask theatre-preview verify` | 779 declared JVM cases, 772 executed PASS, 7 existing named skips, zero failures |
| `cargo fmt --all -- --check` / `git diff --check` | PASS |
| `cargo xtask preflight --changed-only --base cdab25b54e052cb9d250681d1bb9bd9b4bf0caef` | FAIL at repository; exactly the same 42 diagnostics as the clean base (tree `5515012bd8460784bb399b07ef9b33c35e0d2439`). Other selected gates NOT_RUN after that failure |

The initial final preflight inside the sandbox was UNSUPPORTED at Gradle probing. The actual
unsandboxed no-skip run used the installed toolchain and reached the failing repository gate.
Resource-limited Rust/Compose attempts are retained: ENOSPC corrupted a regenerable linked test
output and prevented a font fixture from writing its temporary file. Inventoried task-owned cache
was reclaimed; the successful source checks above were rerun serially afterward. No source,
WIP, capture, DB, secrets or protected retained cache was removed to make room.
