# Loading v1: Theatre request state integration (#588)

Initial source base: `706c76fd6c438a50884d3ed6ea7a8ca853fef0ac` (`develop`), 2026-10-28.
Final integration base: `ebe822972bfa2168b524bf34aeae866c5491b64d` after concurrent topbar PR #597.
Implementation `b29c6853` was merged with that base at `588e0152`; overlapping locale messages
were preserved and all resource conflicts resolved through their canonical generator. The final
Web gate, CMP suite and runtime were rerun on this integrated source; shared primitive inputs
did not change.
This is an implementation and bounded evidence record, not full native acceptance or a design
pixel-match claim. The PR is **Part of #588**; native/device/AT criteria remain open.

## Behavior and ownership

The Web `PublicDiscoveryState` and CMP `PublicCatalogState` retain an applied snapshot separately
from the requested query/walk. Query replacement resets its cursor/budget and fences old responses;
only a valid page replaces rows and commits the applied query. Transient failure retains the old
context. Access/withdrawal refusal clears it; account changes retain their existing fences. Web
Library/History retain authorized rows until an accepted replacement; a refused private read
clears the private projection and requires the existing account recheck recovery without asserting
logout. Existing sync journals, API protocols and backend ownership are unchanged.

App-owned placements reuse shared primitives: SurfacePulse for initial card/cover geometry,
EdgeProgress for actual replacement over retained rows, compact MicroArc for the pending shared
Web FilterChip, and standard MicroArc for append. A pending chip owns that request indicator;
there is no simultaneous search edge. Stale/error are not pending. Covers retain existing
URL/digest/source fences; cover success uses the canonical 150 ms reveal and Reduced is static.
Keyed Web cards/dock results read current entity metadata, preserving identity/focus without
retaining an old cover disclosure for a reused identity. Auth actions reuse their existing shared
loading API; there is no new auth operation. Web submit/Enter/chip timing and CMP 350 ms debounce
with explicit-submit bypass remain separate.

## Admission and source inputs

Before adding app imports, the loading variants were qualified on both renderers. The canonical
arc cycle replaces the temporary Web/CMP seed without adding a token owner or compatibility
baseline. The five paired records cover SurfacePulse, Button, FilterChip, LoadingState and
EdgeProgress: `loading-588-<component>-{cmp,leptos}.json`. The
[admission record](loading-588-admission.json) binds exact shared source/token hashes, opened
reference/capture hashes and runtime observations. Shared inputs remained unchanged during app
implementation; Markdown documentation is outside their executable-source fingerprint.

The opened design authority was `primitives-overview.png` at asset commit
`925b74cd3e9df2182b4c9ad9722d38cb370e3257`, reconciled with canonical tokens. Full-screen
reference images do not authorize a layout redesign. No native device qualification is inferred
from the paired Web/JVM admission.

## Actual verification

Full local outputs and inspected captures are outside Git at
`/Users/manhblue/Documents/Codex/2026-10-28/task-10/evidence/588`.

| Layer | Actual result / artifact | Boundaries |
|---|---|---|
| Shared Leptos | PASS: 186 tests, WASM clippy and gallery build; `leptos-admission.log` | Component renderer, not product/backend acceptance |
| Shared CMP | PASS: 314 executed, zero failures/skips; `cmp-admission-final.log` | JVM Compose, manual-clock and semantics checks |
| Theatre CMP | PASS: 595 shared tests declared (592 executed, 3 skipped); 176 sandbox declared (173 executed, 3 skipped); `cmp-integrated.log` and copied XML | Production reducers/projection/composition over declared ports; no Android/iOS execution |
| Theatre Web | PASS: 548 tests and 6 doctests on integrated source; `web-native-integrated-2.log` | Production Rust core and component tests; final account refusal regression included |
| Web lint/build compatibility | PASS for Studio/Theatre WASM lint, both builds and UI preview; `web-integrated.log` | Integrated final source; Studio has generated localization digest and canonical arc consumption only |
| Browser | PASS bounded observations in `browser/web-runtime-final.json`, `web-runtime-integrated.json`, `http-fixture.jsonl` | Production Web bundle + synthetic loopback HTTP catalog; **mock-integration-tested**, no real Server/Identity/storage |
| Repository | FAIL: 42 diagnostics versus 50 on clean integrated base, none new/worsened; `repository-comparison-integrated.json` | No baseline expansion, gate weakening or unrelated qualification repair |
| Handoff preflight | UNSUPPORTED: locked Quint absent; `preflight-integrated.log` | All other selected steps NOT_RUN, not PASS; installed Gradle/JDK works in separately executed affected CMP checks |

The first Web gate and native rebuilds failed with `No space left on device`. After recording logs
and removing only completed task-owned native outputs, the same gate passed. Earlier regression
runs exposed intentional old clear-on-query assertions, an account-publication assertion at the
wrong ownership layer, and a real CMP edge-placement spacing regression that pushed a card off
the tested viewport. Their logs are retained; corrected reducer oracles and an overlay edge slot
passed the final scoped suites. An uncertain interrupted CMP run was not counted as PASS.

## Rendered and interaction evidence

Opened CMP app captures: `cmp-app/initial-Light.png`, `refresh-Light.png`, `append-Dark.png`,
`failed-retained-Dark.png`. They show the static shell with initial placeholders, retained
interactive card with a 2 px edge, append footer arc and failure with retained card/retry. Shared
admission captures cover Light/Dark, 320/840 widths, text scales 1/2 and Full/Reduced. Only the
opened subset cited by the admission record is claimed inspected; a PNG alone proves no motion.
The same four scenarios were recaptured and opened after integration under
`cmp-app-integrated-corrected/`; its metadata identifies `588e0152`. The first integrated capture
attempt had a stale runner source label and was retained but replaced by a focused capture rerun;
the full suite's XML was copied before that focused run.

Opened Web captures: `browser/web-initial.png`, `web-refresh.png`, `web-success.png` and
`web-failed-retained-final.png`. Initial loading kept shell/filter geometry; refresh kept the old
query/type context and card while the input held the new text; failure stopped the edge and kept
the old context. Before/after states are distinct transitions, not a baseline design comparison.
Read-only DOM observations measured an active 2 px edge, retained input focus, inactive edge after
failure, pending chip with the old applied chip still selected, and zero rows/context after a
synthetic 403. Input alone produced no catalog request; Enter did. Append preserved rows and
focused the newly appended link after success. A slow old response arriving after `newest` did
not overwrite it; only a successful empty response committed no-results.
After the develop merge, `browser/web-integrated-refresh.png` and `web-integrated-failure.png`
were opened: the updated topbar coexists with old-context retention, accepted new context,
preserved input focus and an inactive 2 px edge after failure. Capture hashes are in
`loading-588-app-integration.json`.

The browser was local macOS in-app Chromium 156, viewport 1280×720, DPR 1. Shared gallery also
used a real 390 px viewport and explicit recipe frame320/text200. Browser zoom at 200%, native
IME/touch/OS motion/background, VoiceOver/TalkBack and Android/iOS lifecycle were **NOT_RUN**.
No production catalog/auth/media request, database migration, deployment or APK distribution ran.

## Acceptance boundaries and resume

AC-03/04 have direct paired production-core regression evidence for applied snapshot, reset,
response/token/cursor/account fences. AC-01/02/05–09 have bounded implementation and Web/JVM
evidence, with broader target acceptance still separate. AC-10/11 retain native/AT/zoom gaps;
AC-12 includes this source-bound handoff but no claim of a complete native matrix. GitHub ticks
must remain conservative and the issue stays open.

Resume on an authorized Android/iOS runtime: exercise initial/refresh/append/error/refusal and
cover-source/account changes, actual OS Reduced motion, foreground/offscreen/disposal, Vietnamese
IME submit/debounce, touch and screen reader; repeat narrow/wide and 200% browser zoom on Web.
Bind OS/browser/device/build/source IDs to opened captures and observations before closing those
criteria. Missing capabilities do not waive acceptance or convert fixtures into live evidence.

Residual risk: native lifecycle/AT/zoom acceptance and real backend integration remain unproven;
the repository-wide stale qualification failures remain outside this bounded loading change.
