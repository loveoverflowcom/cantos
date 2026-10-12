# CMP UI-system session 3 — review and readiness

Status: final bounded local handoff of the authorized Mac mini Ultra → Extra High → Ultra
sequence. This delivers reviewed shared components and diagnostics, with **0/29 production
qualifications**. It does not implement Cantos Studio or Cantos Theatre mobile.

The clean starting HEAD was `d3e96f2b43b5e1fb93715309dbd2e1b30667d275`, retaining session 1
`b1e3c39a1039976eefcd6cd3ea9431d8138ff4c2`. Fresh read-only fetch confirmed develop/base PR41
at `7f416a1e1bac5bbf338d461463770bffd983b5c1`. Review used the two-dot base→HEAD change plus
whole owners, consumers, pinned dependency bytecode and the previous 302-row inventory.
Review passes were read-only; the request separately authorized the engineering fix phase.
No reset, history rewrite, push, PR, merge or additional workchat was performed.

During final handoff, a second read-only fetch confirmed external
[PR42](https://github.com/loveoverflowcom/cantos/pull/42) merged at 03:39 UTC, advancing develop to
`342d65fdbd3764f76b772ee0a398a9e209115b65`. The branch now diverges from develop; its merge-base
remains `7f416a1e1bac5bbf338d461463770bffd983b5c1`. The tested tree is deliberately preserved.
Final handoff preflight pins that original base explicitly; it is not integrated PR42 evidence.

## Later integration boundary

PR42 renames `apps/cantos` to `apps/cantos-theatre` and updates workspace, xtask, scripts and
skills; `cantos` remains a command alias. Its canonical Studio API prefix is `/api/v1/studio`,
with deprecated legacy aliases; Theatre/Admin routes remain unimplemented 404 surfaces.
These are external changes, not implemented or accepted by this session's tests. External
Rust1.97 results or a skipped CMP gate cannot replace this tree's pinned1.87/CMP evidence.

Before an authorized publication, parent must reconcile the renamed Theatre paths and the
single moved compatibility-baseline import site, regenerate both catalogues, refresh source
digests/matrix and run integrated pinned local gates. Review baseline migration without expanding
admitted APIs or treating a rename as qualification. Do not publish this old-base sequence directly
or infer publication permission from the external merge. No merge/rebase or integration fix is
performed here.

## Invariant and review disposition

Native callbacks stay inside the accepted finite value/tick contract, preserve caller-owned
state and complete only actual requests. Preview discovery and application admission cannot
silently hide component adoption. Owners remain the existing CMP package, both package-local
diagnostic previews and the shared registry/checker; no backend, product route, platform target,
dependency, font resource or canonical token value changed.

| Finding | Relation to reviewed change | Fix / cheapest evidence | Disposition |
| --- | --- | --- | --- |
| P1 native SetProgress can emit an off-tick negative value or NaN, crashing accepted controlled input | Session 2 forwarded Material3 1.8.2 callbacks before its stricter validator | Finite bounded request projection and wrapper-owned progress semantics; actual production-wrapper failing-first regression | Closed |
| P2 unchanged decimal aliases could cause callback/completion in the first review fix | Found during independent recheck of this working tree | Compare the physical current tick, including native lower ties, while requests choose the higher tie; decimal, endpoint-keyboard and dense-tick failing-first regressions | Closed |
| P2 continuous arrows could round to the current value on an accepted narrow Float range | Session 2 keyboard projection | A one-percent/page step that cannot be represented advances by the next Float in that direction, bounded at endpoints; subnormal/large-offset failing-first examples | Closed |
| P2 fractional preview value was truncated by toInt | Pre-existing specimen retained in session 2 | Complete Float text with Vietnamese decimal comma; actual registered enabled specimen failing-first regression | Closed |
| P2 product Kotlin could enter com.cantos.ui and call components without imports | Pre-existing at PR41 base | Fail-closed product package guard; ordinary and escaped/spaced package failing-first fixtures | Closed |
| P2 public inline/generic and qualified/aliased composables could evade shared API inventory | Pre-existing at PR41 base | Recognize modifiers/type parameters, qualified/escaped annotations, imported/direct type aliases and empty annotation arguments, retaining private/internal helper exclusion; failing-first fixtures | Closed |
| P2 LoadingState announcement specimen could not be discovered | New API/preview branch from session 2 lacked its registered state | Add the state to both generated catalogues and an actual Leptos fixture; declaration/SSR failing-first test, CMP semantics and opened capture | Closed |

Select stable identity/availability, controlled action and rich-field guards, native tabs,
Dialog lifetime, layout and feedback had no other confirmed defect in the declared scope.
Stale/removed Dialog focus targets remain explicitly caller-owned. No durable domain rule was
extracted: range projection is the small deterministic core; native focus/input remain adapters.
The independent final CMP review returned `no-actionable-findings`, not production approval.

## Readiness inventory and admission

The [normalized inventory](cmp-ui-system-readiness.json) has one row per current
component/state/renderer, with pooled source references, precise test claims/results,
captures, runtime observations, blockers and reviewer/reference records. It also maps every
named token to each renderer's generated declaration, source use and diagnostic specimen.
This is reviewer-authored evidence metadata, not verifier output or qualification records.

| Dimension | Held evidence | Acceptance still missing |
| --- | --- | --- |
| 29 APIs / 152 scenarios / 304 renderer rows | Both registries/examples; exact test identifiers and state-specific assertions where present | Rows without direct assertions remain explicit; suite PASS does not establish every state |
| 73 named token roles | 72 canonical roles plus shape.field→shape.reading alias, unchanged source v0.2.0; generator/drift/contrast and scoped density/theme tests | Complete per-name rendered overrides, reference comparison and effective native motion/font use |
| Actions/fields/choices | Controlled callbacks, blocked reasons, readonly selection, blank errors, raw text/zero and JVM keyboard/focus tests; bounded Safari paste/readonly observations | CMP OS pointer/keyboard/resize, real Vietnamese IME/active-composition locking, assistive technology |
| Range/navigation/Dialog | Finite and native tick closure, completion, repeat/interruption, tab reflow; JVM semantics/input; bounded Safari modal observations | Full OS interaction, outside-click/fallback-focus acceptance, screen-reader walk |
| Layout/feedback | Reflow, reading/inset/recovery and explicit live-region tests; opened specimens | Every density/theme/viewport/text-scale cell and announcement behavior with a real reader |
| Application adoption | Qualification list remains empty; compatibility baseline unchanged; alias/import, state/source freshness and inventory regressions | Both fresh structural/runtime/accessibility/visual records for each first new app import |

Canonical admission token digest remains
`e9713d5ad276be53bf3a63b6ce72ad04d1dbf0b5918cbb759a39ea7c2c1f39a2`.
The inventory distinguishes canonical JSON digests from raw file hashes, and current records
from historical session-2 observations. New declaration/state/source changes invalidate
qualification through the existing digest contract. The checker remains a conservative source
gate, not a compiler, reviewer authenticator, screen-reader oracle or visual comparison service.
Indirect wrappers/re-exports still require manual review.

## Actual execution and provenance

Rust/Cargo 1.87.0, Trunk 0.21.14, Python 3.13.5, task-local official Gradle 8.14.3 and full
Microsoft JDK 21.0.7+6. Pinned Kotlin/compiler 2.2.20, Compose 1.9.0 and stable Material3 1.8.2
are unchanged. Commands use `TMPDIR=/private/tmp`, Cargo two jobs and one heavy lane at a time.
Initial host evidence: 40 GiB available and memory_pressure 85% free. No toolchain/SDK install,
cache deletion, Gradle9.7 change, TLS trust change or OS/security preference change occurred.
Ordinary sandbox approval enabled authorized cache/process/Git access; the initial sandboxed
doctor could not load Gradle native services, while the approved normal-cache doctor found the
pinned toolchain. No automatic approval rejection remains.

Ignored logs, XML, images and local handoff live under `target/cmp-session3-evidence/`.

| Command | Tool-printed result / actual scope | Artifact |
| --- | --- | --- |
| cargo xtask ui-preview cmp verify | PASS; final 74 executed, 0 failures/errors/skips: 25 common, 38 desktop semantics/composition, 11 capture methods | cmp-verify-final.log; test-results-final; final preflight |
| cargo xtask ui-preview leptos verify | PASS; 49 native package tests including 16 gallery tests, warnings-denied WASM lint, real Trunk build | leptos-verify-final-formatted.log |
| cargo xtask verify repository | PASS; 195 Python tests including qualified/alias/empty-argument admission regressions and final evidence hygiene | repository-annotation-final.log; final handoff |
| cargo xtask verify rust | PASS after formatting the touched Rust test; workspace format/lint/tests | rust-final-formatted.log |
| cargo xtask verify contracts | PASS; script-ir-reference, contract-tests, wav-reference | contracts-final.log |
| cargo xtask verify web | PASS; Web lint, both consumer builds and shared gallery build | web-final.log |
| cargo xtask preflight --changed-only --base 7f416a1e1bac5bbf338d461463770bffd983b5c1 | Original-base handoff result recorded separately after PR42 fetch; prior all 9 selected gates PASS, no skips/FAIL/UNSUPPORTED; current CMP desktopTest executed 74 tests | preflight-diverged-base-final.log; preflight-annotation-final.log |
| cargo xtask ui-preview cmp run | Owned foreground launcher reached MainKt; OS input BLOCKED | cmp-runtime.log; runtime-observations.json |
| cargo xtask ui-preview leptos run | Native Safari observations below; no backend started | leptos-runtime.log; runtime-observations.json |

Failing-first logs preserve the negative/invalid/fractional range failures, adjacent-Float arrow
failure, same-tick alias/keyboard/tie failures and admission/discovery failures. A new capture
initially lacked its required experimental test annotation; it failed compilation, was fixed,
then ran. The Rust gate initially stopped at the new test's formatting difference; lint/tests
were NOT_RUN in that attempt, then PASS after formatting. A LoadingState SSR assertion initially
looked for role after class; its corrected complete-opening-tag assertion passed. None of these
failed attempts is counted as passing evidence. UP-TO-DATE is not another test execution.

## Opened pixels and runtime observations

Surface: Low-intensity diagnostic controls and reading specimens, paper/lavender tonal grouping,
Material outlined icons and spacing. Localization is synthetic Vietnamese preview copy;
no production resource key changed. No Kotlin formatter/linter is configured; Kotlin was
manually reviewed, compiled and tested. Rust formatting and warnings-denied checks ran.

The independent reviewer opened all 26 current JVM PNGs; the primary opened five named
range/loading/navigation/token specimens. Roots are 1140×840 or 372×840 at density 1,
platform fontScale 1, with the listed preview multiplier. All final PNG hashes match the opened
set fingerprint `d3f8cff9e8a7b00635f592457437abd079c42c7496b7acb9fab6c22bcc906c39`.

Negative-range dark/comfortable/narrow/2× shows complete −100..0 text and diacritics, with −50
at the center tick. Loading and four navigation labels wrap completely at narrow/2×. Actions
retain light/dark icon foregrounds; field focus rings, raw 1,25, blank errors, nested 2× theme,
choice/Select reasons, recovery actions and reading/grid specimens show no new pixel defect.
The narrow whole-root image shows the upper scrollable toolbar only: it does not prove lower
component reachability. The token image shows first rows only: it does not inspect all 73 roles.

The actual local Studio reference HTML (SHA256
`de39cc8d0a032a1a2abd9d021b4703ec7af288c0124abab585c41533bf5b1ae6`), route `#design-system`,
was rendered and opened in Safari light/dark. It supports bounded direction observations about
paper/purple hierarchy, rounded actions and icon treatment. The reference home artwork paths
returned 404; those images were not used for the component-board comparison. Full same-state
renderer/reference fidelity and Noto metrics remain NOT_RUN. Screenshots are in the computer-use
transcript, not persisted qualification artifacts; CSS viewport/zoom was not instrumented.

Safari 18.6 (20621.3.11.11.3), tool window pixels 1621×862:

- New native feedback option opened the complete announcing LoadingState specimen and visible
  selector focus ring. No audible announcement was observed.
- TextField native editing specimen preserved `Người dẫn chuyện · Ỗ, Ữ` by paste; Return changed
  Enter count 0→1. Readonly allowed selection 0–23 and blocked attempted paste/Return, retaining
  text and count 1. Paste is not Telex/VNI or active OS composition evidence.
- Dialog initially focused Close. After paste of `Nguyễn Thị Minh Phương · Ỗ, Ữ`, Escape requested
  dismissal and restored opener focus. Reopen retained text; CloseButton restored opener again.
- Only the task-created tab was closed. Owned Leptos and reference-server sessions stopped.

CMP app inventory showed MainKt/net.java.openjdk.java on the intermediate build before the last
tick-comparison recheck. Supported binding by ID, observed name
and actual Java executable path all returned Invalid app. No OS input was sent. Ctrl-C stopped
the owned launch; recorded xtask33596/launcher33631/Main33648 were absent from the subsequent
PID check. That proves this shutdown observation, not every possible lifecycle failure.

Evidence levels held: `example-tested`, `compiled`, `semantics-tested`, bounded JVM/Safari
`interaction-tested`, `screenshot-inspected` and the specifically listed cross-theme/viewport
observations. Screen reader: not walked. Motion/reduced-motion runtime: not observed.

## Remaining acceptance and manual handoff

Structural/behavioral preview readiness is separately reviewable. Production qualification remains
**0/29** because required runtime/accessibility/reference evidence is incomplete. Both Android/iOS
product app directories remain guides: app/static capture UNSUPPORTED, mobile runtime NOT_RUN.
PostgreSQL/mutation were not selected by this UI-only diff; remote CI was not checked.

To accept a component for a first new app use, its owner must:

1. Open these package-local previews with the pinned xtask run commands at the exact reviewed
   source/token digests. A human can operate the existing CMP window while tool binding is blocked;
   record actual pointer, Tab/Shift+Tab, activation and focus results, without changing security.
2. Run the component's exact matrix states, including disabled/loading/rejected callbacks, readonly
   selection, Vietnamese paste/zero, menu interruption, range completion and modal cancel/reopen.
   Record native Telex/VNI composition separately, including locking during active composition.
3. Capture and open both renderers at the missing narrow/wide, theme, density and 1×/2× cells;
   inspect named effective tokens/aliases and compare the actual corresponding reference. Resolve
   approved Noto delivery/metrics before claiming font fidelity.
4. Walk a named screen reader and observe announcements, names/roles/values; record motion and
   reduced-motion interruption on the actual target. Still images cannot close those gates.
5. Supply both fresh evidence records under the existing admission contract only after every
   required check passes. The compatibility baseline cannot grant new use. Android/iOS acceptance
   needs later authorized real targets and device/simulator evidence; desktop results cannot supply it.

Parent owns any later push/PR/merge. There is no automatic fourth session or app expansion.
Residual: CMP OS binding/input/focus/resize/IME BLOCKED; assistive technology, motion, Noto,
full state/token/reference cells NOT_RUN; mobile UNSUPPORTED/NOT_RUN. These limitations block
qualification, while the reviewed local implementation can be handed off with its passing gates.
