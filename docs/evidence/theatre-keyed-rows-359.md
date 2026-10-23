# Theatre Web keyed rows — #359

Source-bound implementation and observed evidence for [#359](https://github.com/loveoverflowcom/cantos/issues/359)
Parts A and B, based on `refactor/cantos-native-turso@fce2934a5eed0b24fdcda3a6b338cf014bcce249`
(the merged #365). Before final checks, the branch incorporated current base
`80dca572b4a81fb662e0586709ea6669306bd5b9` (#366 behavior models/tests; no UI overlap).
The counter and before-change observations below retain their original source provenance.
This record accepts no live provider, database, native playback or performance claim.
The PR owns the final head SHA, review and gate ledger; the work queue order is unchanged.

## Implementation boundary

Library separates section-scoped Work membership/order, optional row projections and section
chrome. `For` keys Work rows by `work_id`; retained rows read their current presentation,
position/completion, command and resume feedback. The existing publication-based Resume IDs
require replacing that control when the publication changes; the Work row remains mounted.
Pinned keyed DOM moves can still clear native focus/selection. A list-scoped platform helper
snapshots raw selection endpoints and focus before membership diffing, then restores connected,
unchanged endpoints in a microtask before the existing completion-focus animation frame. It
does not restore removed rows/account owners or override a different intentional focus target.
A missing row yields `None`. Existing signed-in `Show` ownership still disposes sections on
logout, account switch and session recheck. Read-model transitions, authentication, persistence,
outbox publication, preview gating, dependencies and copy remain with their existing owners.

Public Work keys by `unit_id` and projects current content/availability by ID. Listen and Resume
read the current row at activation. Before an unavailable unit's action subtree disappears,
focused actions hand focus to its retained `unit_heading_id` heading. The page-completion focus
helper preserves this handoff; focus elsewhere retains its existing behavior.

## Regression and browser boundary

The exact base plus the new Public Work scenario times out at `same-ID title refresh`; the base
retains the old title. The candidate passes the same scenario. The first Library identity test
also passes on the base for a sibling-section update: no claim that this scenario reproduced
focus loss or row remounting. Keyed membership correctness and construction counts are separate.

Host tests execute the production optional selectors, mutation/receipt projections and retained
actions. They cover individual fields, pending/unknown/failed/conflict/idle command outputs,
completion, null metadata, insert/remove/reorder, removed lookups, current publication/position,
unavailable actions, logout/account switch and a stale A completion. They establish
`example-tested` behavior, never DOM identity.

`cargo xtask verify web-bearer` uses the existing Node/CDP harness, Chromium
`155.0.8059.85`, isolated profile, debug CSR production modules and synthetic loopback API/issuer
at ports 18080–18083. No live service or actual media is used. The 25 scenarios include:

- Same-ID Public Work title/description/duration/ordinal updates with retained row, heading,
  Listen control and sibling text selection; replacement Listen requests the new publication.
- Keyboard Tab into Listen during a held walk, then unavailable-action focus handoff to the
  same heading, with the row retained.
- Library insertion and removal with retained sibling nodes/controls; Pending → Failed on a
  retained Saved control; unchanged Continue focus and zero mutations in that row.
- Account A node disposal before B mounts and rejection of the delayed A page.
- Existing navigation, auth/refresh, private artwork/source and Studio draft recovery scenarios.

The final fixture adds held Saved-page insertion and Continue reorder scenarios. History can
report a newer release/order for an existing Continue Work; production sync then reorders the
retained Work keys. Before the platform helper, this scenario retained nodes but lost focus
and selection (`web-bearer-1792587576268/reorder-observation.json`). Assertions cover retained
nodes/selection, focus observed after the move, the existing later completion focus to section
status, and DOM/keyboard row order. Final results and source fingerprint are recorded in the
PR separately from the earlier 25-run observations above.

The driver waits for rendering and re-scrolls/hit-tests the current control before trusted
pointer input. This avoids a measured race where the previous page's focus scroll invalidated
the next click coordinates. The mock's second Work has its own publication/audio identity;
sharing one publication across two Works was an invalid fixture that sync reconciliation exposed.

## Four distinct observations

A temporary **log-only** instrumented build counted production function entries and `For each`
entries. The normal MutationObserver measured actual DOM changes. The instrumentation was
removed before final checks; it adds no shipped logging, feature or dependency.

| Debug fixture update | Selector/lookup computations | Membership reconciliation calls | Row/component constructions | Actual DOM mutation records |
| --- | --- | --- | --- | --- |
| Two Public Work rows; one same-ID title/description/duration/ordinal update through cursor restart | 9: 3 membership + 6 row lookups | 0 | 0 | 7: 6 character-data + 1 attribute; 0 child-list records, added/removed rows |
| Six Library rows; one Saved Pending → Failed command | 36: 9 membership + 18 row + 9 chrome | 0 | 0 | 21 across Library; **0** in the unchanged Continue row |

These are one synthetic update each, not latency/CPU/allocation samples. Counts include the
existing aggregate notifications and remain above zero. Linear `find` lookup and aggregate
subscriptions remain; no O(1), zero-selector-work or overall performance improvement is claimed.

Ignored evidence: `artifacts/agent-runs/web-bearer-1792586173202/{report,row-metrics}.json`,
`artifacts/agent-runs/issue359/{instrumentation-originals.json,instrumented-diff.patch,row-metrics-summary.json}`.
Instrumented source fingerprint:
`db7501e396cc10fe866744749b79d0dfc5e5106f931b17d35c28ab842ed81b7c`.
Final uninstrumented provenance and results are recorded separately in the PR.

## Inspected captures and remaining limits

Opened synthetic Public Work and Library captures at 1440px/dark and 390px/light, Vietnamese,
100% CSS text size, under `artifacts/agent-runs/web-bearer-1792586007725/`. The unavailable heading
has a visible focus outline; retained sibling selection is visible; the sampled text/controls
are readable. These are `screenshot-inspected` observations, not a contrast, screen-reader,
200% readability or cross-browser acceptance claim. CSS/tokens/layout policy are unchanged.

Browser insertion/reorder observations cover synthetic fixtures in the named Chromium version;
they do not establish focus or selection for every update, engine or device. Real OS IME,
screen readers, native media,
real issuer/database and CPU/allocation profiling are `NOT_RUN`. Retained callbacks after an
owner is disposed are not driven by artificial clicks on detached WASM event closures; absent
and unavailable current-state actions are tested on their live production selectors.
PostgreSQL is not installed/started and Kani's pinned Linux bundle is unavailable on this Mac.
The unrelated PR2 transition/notification work and #360–#364 remain outside this change.
