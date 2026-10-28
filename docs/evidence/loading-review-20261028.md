# Loading v1 review and bounded repairs

Review base: `develop@15a675dfbfdbbeec02bd6442b2188a51fff265f3`, fetched on 2026-10-28.
The four merged contributions were read as each merge's first-parent diff:
PR #593 (`ccd0be5a`, head `d3d945e8`), #592 (`daf0662a`, head `6e399856`),
#598 (`706c76fd`, head `871e54da`) and #599 (`15a675df`, head `90089065`).
This records source review, tests and selected browser/JVM observations; it does not accept all
of issues #585–#588 or establish Android/iOS, assistive-technology or real-backend acceptance.

## Confirmed defects and repairs

The first three are P2 confirmed defects in presentation/lifetime ownership; the fourth is a P2
confirmed rule/compatibility defect in Web token ownership. Confidence is high:
each has a baseline failure and a production-code regression or browser before/after observation.
The pulse finding is logic/lifecycle; footer placement is logic/layout compatibility; chip focus
is compatibility/accessibility. Criteria: `cantos-ui-design` motion/accessibility policy,
`cantos-leptos-web` reactive/focus ownership and `cantos-cmp-mobile` lifecycle and VER-01–08.
No P0/P1 candidate survived review; this is a scoped finding statement, not universal safety proof.

1. **Pending fills continued outside the viewport or an unfocused host.** #599 mounted
   SurfacePulse with its default active value in pending rows and covers. On the preserved
   #588 production bundle, a 390 × 400 browser viewport still reported 12 running pulses;
   the first fill was at y=949.05 with a 1.4s infinite animation. The new JVM regression also
   failed on the base implementation with an unknown viewport (image distance 2939.56 instead
   of zero). Web now binds the pending group and each cover to the existing IntersectionObserver
   and document visibility owner. CMP uses the existing viewport/window facts for each fill.
   Missing facts keep motion off. No request, account, codec or token authority moved.
2. **Library append inserted feedback above retained rows.** #599 placed LoadingState before
   the list, adding height above the content a listener was reading. The regression checks
   actual rendered element order and retained rows after failure. Feedback now follows the
   list in its footer.
3. **Loading filter chips discarded keyboard focus.** New #599 adoption of FilterChip made
   a keyboard-activated chip natively disabled while pending. Browser Enter on Novel produced
   `disabled=true`, one arc, and an empty active-element ID. The shared Leptos chip now uses
   `aria-disabled` during Loading, keeps native disabled for Disabled, and retains the existing
   activation guard. Hover/pressed styling honors the same blocked fact. The source template
   was regenerated, not patched through its generated output.
4. **Web placeholders bypassed the Web shape roles.** The #599 stylesheet introduced
   `--cantos-shape-container` and `--cantos-shape-thumbnail` into pending rows/covers. Those are
   CMP-scale roles (16px/10px here), while Web containers/covers use their independent 18px role.
   The existing WebShapeIndependence regression failed on the clean base and identified both
   uses. Both placeholders now consume `--cantos-web-shape-container`, as the ready Web cover
   does. The existing token regression passes; no token value or gate was changed.

The initial footer test incorrectly split the HTML at every repeated heading ID, and one
short-name exact filter selected zero tests. Neither result is evidence for the defect.
The corrected full-name regression fails on the original placement and passes after the repair.
Those intermediate logs are retained alongside the final logs.

## Assurance ledger

| Claim | Owner / target | Required by | Evidence and disposition |
| --- | --- | --- | --- |
| Canonical loading cycles, geometry and finite bound | shared tokens/generator, MotionPolicy, CSS | #585 AC01/02; #586 AC01/02; #587 AC01 | 1400/1000/1200ms, 150ms reveal, 16/20/2 roles; finite 1000ms bound and 140ms feedback unchanged. Arc consumes LoadingCycle.Arc. Token/generated drift checked; held for source, no timing redesign. |
| Decorative anatomy and Reduced substitution | shared feedback/actions, Web/CMP | #585 AC05; #586 AC03; #587 AC03 | Fresh Leptos structural check and browser observations below; unchanged CMP source/admission retained. Native/AT remains gap-required. |
| Request and viewport lifetime | app loading/cover composition and platform binding | #585 AC03; #588 AC09/11 | Red/green JVM manual-clock regression plus Web unknown-fact and browser observations; held for named host facts. Actual native OS binding remains unobserved. |
| Pending/applied context, cursor and generation | PublicDiscoveryState, PublicCatalogReducer, Library/account owners | #588 AC03/04 | Existing production state/fence tests execute in affected suites. No state-policy change in this repair. Reviewed query, append, refusal, retry, route/account and duplicate paths; held for core, no real backend claim. |
| Cover disclosure and source identity | keyed WorkCard/PublicCover and CoverArtwork | #588 AC09 | Web updates the current keyed entity/cover tuple; CMP keys loaded bytes by cover and source. Existing cover/source/account tests retained; no old image is intentionally retained by this repair. Real HTTP/native cover acceptance remains separate. |
| Feedback placement and focus | Library live section, FilterChip | #588 AC05/07/10 | Corrected DOM order regression; native focusable Loading plus unchanged activation guard. Browser chip pointer/Enter over Loading preserved focus and emitted no intent. |
| Independent Web shape roles | Theatre pending row/cover CSS | policy:cantos-ui-design tokens; WebShapeIndependence | Existing regression red on base, green after using the ready cover's Web role; held for token ownership. |
| Admission freshness | shared preview catalog and qualification records | UI component admission policy | Five Leptos loading records renewed only after fresh tests, actual controls, opened reference and screenshots. CMP records retain their exact unchanged inputs. Other stale qualifications are not refreshed or waived. |

Coverage: the four contributions' runtime owners, token generator, declarations/generated mappings,
qualification records, associated regressions and actual app consumers. Backend/DB/auth protocol,
native media, OS adapters and unrelated preview APIs were not changed or qualified. Production
publication/durability passes are not applicable; no such effect was added.

## Fresh shared Leptos observation scope

Reviewer: Codex, 2026-10-28. Final source digest
`8c21983f9366c940888ae2c31208d87172a512778c85b6842200b48912855a77`;
token digest `afd666ab3f8d1d3fcff2147c2e7b82ea35f544acd147aedbaf3694048273ae08`.
The opened reference was the retained copy of `primitives-overview.png` from asset commit
`925b74cd3e9df2182b4c9ad9722d38cb370e3257`. Issue text/canonical tokens own anatomy, not raster gradients.

The initial `cargo xtask ui-preview leptos verify` passed native suites, WASM lint and gallery
build on digest `17d4cb42b16dee8043c120b35133f32e479f7449927a7368aabe821299fcd780`.
A subsequent review corrected blocked-chip selector specificity to preserve the prior disabled
colors. The final digest above was rebuilt and checked with the native checkpoint and
`cargo xtask verify web`; all five recipes below were repeated in that actual rebuilt gallery.
No historical record was rehashed into acceptance. Real controls were used; scripts only read
DOM/computed facts:

- FilterChip: v1 matrix, all four states, Vietnamese long copy, frame320/text200, Light/Reduced.
  Loading had `disabled=false`, `aria-disabled=true`, a decorative 16px arc and stable selection.
  Pointer input and native Return left focus on `scenario-filter-chip-loading-control`;
  the intent log stayed empty. Explicit Disabled remained natively disabled. A Playwright
  enabled check refused Enter on aria-disabled; native keyboard observation is recorded separately.
- Button: catalog Loading, Light/Full and Dark/injected Reduced. Compact arcs used 16px and
  canonical 1s in Full. Reduced had duration0s/one iteration; pending reasons remained readable.
  Opened screenshots showed wrapped labels, single glyph slots and unchanged control geometry.
- SurfacePulse: catalog running/ready, Light/Full and Dark/injected Reduced. Full fills shared
  opacity0.792059 in the observed frame and 1.4s/infinite; Reduced held opacity0.7 with the root's
  zero-duration/one-iteration restriction. Decorative blocks were absent from the AX tree.
  Revealed long Vietnamese/English titles and metadata remained readable; the region was no
  longer busy. Opened running/ready captures were compared.
- LoadingState: catalog polite announcement, Light/Full and Dark/injected Reduced. One decorative
  standard arc (computed20px), one host status and readable localized copy. Reduced was
  duration0s/one iteration. A rotated bounding rectangle is not a logical-size measurement.
- EdgeProgress: v1 isolated pending, frame320/text200, Light/diagnostic force-full and Dark/Reduced.
  Full: one active track, 2px and 1.2s; Reduced: animation-name none. Error: both tracks inactive
  and animation-name none. Opened screenshots preserved field outline and retained link/text.

Browser actual viewport during these gallery observations was 1280 × 720; frame320/text200 is a
specimen constraint, not browser zoom. Screenshots were opened in the conversation; they are not
committed or claimed as recordings. The browser's read-only DOM adapter did not expose
`getAnimations()`; no scheduler claim is derived from that unavailable method.

On the final rebuild, the Loading chip again retained pointer/native Return focus, emitted no
intent, and kept its blocked color (`rgb(108,97,111)`). Full pulse alpha was observed changing
from 0.71993 to 0.658901 with its 1.4s cycle; Reduced stayed at 0.7. The 16px Button and 20px
LoadingState Full arcs, ready content, and Edge pending/Reduced/error states were reopened and
checked. One attempted catalog click hit a hidden section after the component filter changed;
no side effect occurred. The filter was cleared before the final catalog observations.

The five renewed records cover these loading variants only: Button, FilterChip, LoadingState,
SurfacePulse and EdgeProgress. CMP digest remains
`1e97cdfaa764a4dd26dd30258b66b847550f7e61687e53a1d9d57d03c6007343`.
Its #588 scoped Web/JVM visual qualification remains attributable unchanged inputs, not fresh
native acceptance. This review reran `cargo xtask ui-preview cmp verify`: shared314 executed,
zero failures/skips; the old visual record was not rewritten. It separately reran Theatre CMP: 772 declared,765 executed,
7 explicit opt-in/live-origin skips, zero failures; the new manual-clock pending-row test passed.
Its transparent test-root PNGs are pixel probes, not full-screen UX approval.

## Verification and remaining acceptance

Full outputs, XML and opened JVM probes are retained outside Git at
`/Users/manhblue/Documents/Codex/2026-10-29/task/evidence/loading-review`.

- `cargo xtask preflight --profile local --changed-only --base origin/develop --skip repository
  --skip web-lint`: PASS for rust-format, affected native lint and native tests. Studio441,
  Theatre551+6doctests, shared Leptos186+5doctests. This is a checkpoint with two explicit skips,
  not a handoff PASS.
- `cargo xtask verify web`: PASS for both apps' WASM lint and application builds (dev profile, no preview) plus the rebuilt
  shared gallery. Theatre production-feature bundle (dev profile) `f03aa9dff98262da` was then opened over loopback.
- `cargo xtask theatre-preview verify`: PASS, 765 executed/7 named skips, zero failures. Uses
  production CMP code over injected JVM facts and fixture clients; no native/runtime-backend claim.
- `cargo xtask ui-preview cmp verify`: PASS, all314 shared CMP desktop tests, zero skips/failures.
- The final two app CSS substitutions do not change the shared renderer digest. The existing
  `python3 -m unittest discover -s scripts -p test_ui_tokens.py` suite passes; a fresh
  `cargo xtask build cantos-theatre --web` also passes after them.
- Two earlier native checkpoint attempts exhausted disk. Their output is retained as environment
  failures. Only this task's generated native debug cache was removed before the successful run;
  the prior protected target, other worktrees and evidence were preserved.

In the final Theatre production-feature bundle (dev profile) at 390 × 400, pending fills at y=949.05 reported idle
with no animation; after native End scroll, the visible last fills at y=243.92/275.92 ran the
canonical pulse. At 840 × 900, native Enter on Story followed by Return during Loading retained
`theatre-filter-story` focus, `disabled=false`, `aria-disabled=true` and one arc. Exactly one
Story request completed in the fixture log. The ready-state screenshot retained focus. These
requests used a local synthetic API; the deliberately slow case exceeded client timeout, so
its terminal result is a timeout observation, not a successful live-backend request.

The no-skip `cargo xtask preflight --changed-only --base origin/develop` failed at repository:
42 existing diagnostics on both clean base and final source, no additions/removals. Its later
steps were NOT_RUN, not passed; the independent checkpoint/gates above retain their own results.
The first sandboxed attempt reported Gradle UNSUPPORTED because its native library could not
load through the sandbox. Installed Gradle8.14.3 was verified outside it before the actual gate.
The independent final Python suite is FAIL: 1319 declared,1299 passed,2 failures,18 skips.
The clean base has the same1319 with1298 passed,3 failures,18 skips: the Web shape failure is
resolved and no failure is new. The clean base failed WebShapeIndependence plus two unchanged VPS
checks (BSD sed and missing flock); this patch repairs the in-scope shape failure. The final
Theatre rebuild was opened at840 × 1200: both pending row and cover computed18px and the
visible pulse was running. This final app stylesheet change leaves shared qualification inputs
unchanged. An earlier sandboxed/default-TMPDIR run had additional environment errors and is not the final result.
No behavioral model changed, and the selected handoff plan did not select Quint. No Quint
execution or conformance PASS is claimed. PR/HEAD and terminal GitHub CI are recorded in the PR
and task handoff. A baseline comparison never relabels a failing gate PASS.

No ADR: the issue and existing motion/focus policy already select the behavior. No queue-order
change. No compatibility baseline, gate, migration, auth or private-content policy was relaxed.
The known repository qualification failures and historical migration expected27/actual28 failure
remain independent. Android/iOS runtime, actual OS motion/lifecycle, IME/touch, real browser zoom,
screen readers, frame pacing and real Server/Identity integration are NOT_RUN.
