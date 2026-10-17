# Studio Forge mock composition

Status: debug-only implementation for review of [#227](https://github.com/loveoverflowcom/cantos/issues/227),
not shared-component admission, a live producer or production acceptance.

## Ownership and observable claims

The Studio-owned `studio-forge-mock` composition uses internal mock projections/intents and
an injected deterministic port. It is selected explicitly before the live Studio bootstrap;
the module and entry are excluded by `cfg(debug_assertions)` from release builds. No Forge
service crate, HTTP/provider adapter, credential entry, Drive access or publication is involved.
[ADR 0022](../decisions/0022-cantos-forge-producer-cli.md) and the
[worker boundary](../deployment/forge-worker.md) remain the authority for future live work.

Intent: help a creator edit one synthetic draft, retain immutable local revisions, compare
mock takes and confirm an exact sync plan. Text is the primary content; generate is the
primary editor action, save/sync/settings are quieter tools. Surface intensity is Low for
the editor and Low–Medium for run/sync status, pending inspection on the actual renderer.

| Claim | Failure prevented | Cheapest oracle |
| --- | --- | --- |
| Save appends an immutable revision, edits retain the prior bytes | accidental source replacement | native model tests |
| Execution, storage, selection and mock sync are separate | generated interpreted as saved/published | exact state tests and visible status |
| Unknown outcomes reconcile the same operation | accidental new paid-dispatch semantics | duplicate/unknown tests, no live port |
| Sync plan binds draft/revision/retained take | stale selection silently handed off | stale/changed-plan rejection tests |
| Draft switch, reset and disposal fence old completions | A→B→A or late callback overwrites | ticket tests |
| Locale/theme/panes retain input | editor remounted on preferences/navigation | native projection tests; browser acceptance pending |

## Component lookup and preview declaration

Stable composition ID: `studio-forge-mock`; owner: `apps/cantos-studio/web/src/forge`.
Anatomy: explicit mock banner/recipe controls, library, stable editor, take inspector,
connection settings and exact sync confirmation. This is a Studio Web composition, not a new
shared API or a Studio Mobile screen.

Existing catalog APIs looked up: `button`/`Button` (regular variants), `icon`/`Icon`, native
`select`/`Select`, `dialog`/`Dialog`, `text-field` and `text-area`. The composition reuses the
Studio `ActionButton` and `ContextSelect` boundaries and the existing dialog owner in `ui.rs`;
native script editing follows Studio's native text-first editor anatomy. No copied SVG,
palette, second Select or new shared control contract is introduced. The #225 `SelectMenu`,
compact BackButton and spacious AuthTonal candidates remain outside adoption. The debug mock
does not establish qualification of indirect wrapper use or product-flow acceptance.

Recipe v1 IDs: `ready`, `empty`, `loading`, `error`, `locked`, `stale`, `run-unknown`,
`storage-failed`, `sync-partial`, `sync-unknown`. Reset explicitly discards mock edits and
recreates the session fence; preference and pane changes do not reset it. No external state
or private content is serialized into a URL. Audio bytes are absent and playback is visibly
unavailable; no listening claim is made.

## Evidence and remaining acceptance

The native model and Leptos composition use the same mock-controller code. Typed scenarios
cover duplicate input/completion, immutable revision history, dirty/stale selection, failed
storage, unknown attempt and partial/unknown sync reconciliation, exact confirmation and
A→B→A/reset/dispose fencing. The shell preserves oversized raw text per draft for correction;
the bounded model keeps its last accepted value and Save/Generate/Select/Sync stay blocked
while those differ. IME events retain the native textarea; identical final values are not
dispatched twice. These source/test facts still require an actual IME/browser observation.

Dialog error recovery distinguishes an opening failure from a rejected return-focus target.
Only a live Preview can be canceled; a dispatched operation and its identity are preserved.
The app-owned focus helper targets the stable mock-step heading after the sync opener becomes
disabled. This is example-tested classification and compiled DOM code, not browser focus proof.

Tokens remain v0.6.0. The CSS adds only Studio-owned layout using existing semantic roles;
no motion or shared shape/palette is changed. Locale resources extend the paired vi-VN/en
Studio owner; synthetic story text retains `lang="vi-VN"` when UI language changes.

Focused command: `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 RUSTUP_TOOLCHAIN=1.98.1 cargo test --locked -p cantos-studio forge::`.
Fourteen tests passed at the review checkpoint, including native HTML serialization with no
render-time dispatch. The ordinary `cargo xtask verify rust` attempt passed workspace lint
but failed during compilation of workspace tests with `No space left on device`; test
results from that run are not a pass. Only this worktree's reproducible incremental cache
was removed after it finished. Incremental is disabled for subsequent scoped checks.

Implementation evidence and final commands are recorded in PR #230. Native string rendering
is structural evidence, never DOM/browser/keyboard evidence. The behavior catalog contains
only the separate draft/model-only producer attempt capability; this UI mock is unmodeled.

Reference files were resolved as Library v2 dated 2026-10-17, but the current bundled Library
helper reported `Library prepare_materialize is not available` on Mac. No reference bytes or
board pixels were read, so visual fidelity is unassessed. CUA inventory reported the Mac
locked with no browser available. Required browser light/dark, 320/390/768/1440 CSS px,
long vi-VN/en, 200% zoom, keyboard/dialog focus return, IME and reduced-motion observations
remain pending, as does a named assistive-technology walkthrough. Unassigned — assignment
required before acceptance. Neither missing evidence is filled by build/HTML results.

No startup, bundle-size or cache measurement was run. No API/DB/storage/provider integration,
Functions deployment, native Studio surface, publication or listening evidence is claimed.
The private mock has no schema, database migration or compatibility change. The only new
dependency direction is the debug Studio shell → Studio mock → its plain model/port; it never
imports another app or a service. Existing shared-admission failures remain separately gated.

### Resumed Mac check during session 4

The user unlocked the Mac; Chrome is now available, so the earlier lock is no longer the
runtime blocker. The canonical `cargo xtask app cantos-studio --web` attempt refused to start
because `cantos-server` was not listening on `127.0.0.1:8080`. It started no frontend or API.
No database/runtime provisioning or substitute listener was used. Forge browser acceptance
therefore remains pending behind the launcher's existing API prerequisite. The shared gallery
can run independently: session 4 observed Button/IconButton loading and click/Space/Enter in
Chrome, but those observations do not accept the Forge flow or dialog. Library materialization
was retried through the current helper and remained technically unavailable, with zero bytes
transferred. Keep #227 open; see PR #230 for the combined gate and acceptance ledger.
