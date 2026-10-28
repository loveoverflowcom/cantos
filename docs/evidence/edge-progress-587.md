# EdgeProgress shared primitive (#587)

Source boundary: `develop@568eefdc7af8102b54e019182aef54c681f44a85` plus this change.
The [issue](https://github.com/loveoverflowcom/cantos/issues/587) owns the design and acceptance;
[observations](edge-progress-587-observations.json), [browser measurements](edge-progress-587-browser.json)
and [artifact hashes](edge-progress-587-artifacts.json) bind this review to the actual renderer inputs.
This is a shared primitive/preview delivery, with no application import, request timing, debounce,
API, authentication, storage or native lifecycle-adapter change. Production adoption belongs to #588.

## Controlled contract and admission scope

Both renderers require host-owned `active`, `visible`, `foreground` and a nonblank localized `label`.
Only in-flight + visible + foreground shows the indicator. Success/error/cancel clear `active`;
hidden or background hosts clear their independent fact, and disposal cancels the mount. The slot
stays 2 logical units through those changes. One quarter-width segment crosses a straight clipped
track linearly over canonical `motion.loadingEdge.cycle` (1200 ms); it is never determinate progress.
Reduced keeps a centered static segment. Semantic colors come from `primaryContainer` and `primary`.
Frame work stays in transform/draw, with no field/list layout animation or focus changes.

The host keeps the status outside its busy content region and opts into announcements once per
significant request change. The track is decorative, carries no percentage and cannot take focus.
The paired preview reserves two slots but activates only the selected search/list placement.
The Web preview root observes document visibility and viewport scroll/resize/reflow without a poll
or per-frame timer. Components receive those facts; they never fetch or read an OS preference.

Fresh records cover the named **browser and JVM Desktop Compose** renderers:
[Leptos qualification](edge-progress-587-leptos-qualification.json),
[CMP qualification](edge-progress-587-cmp-qualification.json). They do not accept Android/iOS device,
OS background/reduced preference bindings, screen readers or smooth real-time frame pacing.
App adoption must supply its real viewport/lifecycle/request facts and observe that flow separately.

## Invariant and evidence ledger

| Claim / failure mode | Owner / oracle | Held evidence and limit |
| --- | --- | --- |
| AC-01: geometry/timing diverges or bypasses canonical roles | Feedback/CSS, EdgeProgress.kt; token and production-composable tests | PASS: shared 2-unit role, 25% anatomy, Edge cycle 1200 ms, linear. Browser measured 2 px and exact quarter-width; JVM clock measured equal 304 ms displacements over the actual tween. |
| AC-02: an idle, terminal, hidden, background or disposed host keeps a loop | Three required facts; full-motion branch owns the only ticker | PASS for injected controlled facts in SSR truth table, JVM lifecycle/idle clock and final browser lifecycle. Web offscreen observed `animation:none` and visible resume. Actual OS/native adapters remain outside this primitive delivery. |
| AC-03: Reduced travels or announcement is suppressed/repeated per frame | CSS/media and injected CMP motion; separate status semantics | PASS: JVM unchanged pixels over 2400 ms; browser injected Reduced `animation:none`, same centered transform, one nonempty localized status; no own aria-busy or percentage. Actual OS Reduced and screen-reader walkthrough NOT_RUN. |
| AC-04: request chrome moves/blocks the old content or steals focus | Permanent slot outside the existing field/list; focus and geometry checks | PASS: final browser field 48 px and retained bounds `[41,634.4375,356,122.390625]` unchanged across eight host states; text, caret `[0,0]` and focus preserved. JVM independently asserts field/list bounds and focus. No field owner or app timing edited. |
| AC-05: preview registration is mistaken for qualification | Shared catalog and real existing recipe hosts | PASS for scoped browser/JVM admission with fresh source/token digests; Light/Dark, Full/Reduced, 320/840 frames and host terminal/interruption cases. Web VI/EN and narrow 200% text inspected. Native/AT acceptance remains separate. |
| AC-06: source/still pixels are called runtime or native evidence | Existing renderer tests + separate observations | PASS: 186 Leptos SSR/recipe tests, WASM warnings-denied clippy and Trunk build; 313 JVM tests, including 4 EdgeProgress clock/lifecycle/capture tests. Browser measurements are separate from screenshots and JVM clock; no device or pacing claim. |

The four opened CMP captures use widths 320 (text scale 2) and 840 (scale 1), both themes,
Reduced, injected bundled Noto families. Web opened eight detail captures use frames 320/text200
and 840/text100, both themes and locales; actual viewport overrides were 390x1000 and 1000x1000
(the content widths exclude a 15 px scrollbar). Extra full-page narrow contexts verify that
retained paragraphs wrap and remain reachable by scrolling. A single-line editable field keeps
native horizontal scrolling; it is not a multiline text container. The original reference PNGs
were fetched and opened at pinned asset commit `925b74cd3e9df2182b4c9ad9722d38cb370e3257`.
Reference pixels illustrate placement; the issue text/token contract owns exact geometry.

## Verification and review boundary

The exact commands, source digests and log hashes are in the linked observation record.
All final browser data comes from `task-9/cantos`, confirmed in the Trunk compile path. An earlier
preview launch reused a compile-time-root xtask from a temporary base checkout sharing target;
that run was stopped and discarded, xtask rebuilt, and final source/runtime rechecked. Its output
is retained locally as failed attribution evidence, never reused for qualification.

The clean base repository gate reproduced 50 stale qualification/import diagnostics. Final
preflight results and terminal CI belong to this PR's verification report; a base failure does
not become PASS here, and no baseline/gate is widened. The unrelated historical migration failure
is not asserted as reproduced by this slice's local focused checks.

Independent code review uses cantos-code-review against the pinned base, including new files,
controlled construction paths, lifecycle cleanup, generated mappings, stale-result safety and
qualification freshness. There is no durable decision, record mutation or service dependency to
review in this presentation-only slice; ownership stays inside the existing feedback/renderers.
No ADR is needed because #587 already selects the anatomy, timing and ownership. Queue: no change,
because the agreed #585 -> #586 -> #587 -> #588 sequence and production boundaries are unchanged.

Residual risk: hosts can pass incorrect visibility/lifecycle facts. #588 must connect real request
and retention state without changing existing submit/debounce rules, and must qualify SurfacePulse
and MicroArc independently before new imports. Their merged status did not supply admission;
MicroArc's recorded 1000 ms renderer seed and broader qualification gaps remain untouched.
Android/iOS lifecycle, OS Reduced, screen readers and frame pacing require named target evidence.
