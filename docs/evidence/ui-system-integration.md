# Leptos consumer integration — session 3

Date: 2026-10-11. Local-only continuation on `feat/leptos-ui-system`, cwd
`/Users/manhblue/projects/loveoverflow/cantos`. Starting HEAD was component commit
`e6d87ad5fabebac17b2873a82541b1c6e0195b78`, after foundation
`cecad0472e4f8cef18160c542acbfa7ac4844536`; original develop base was
`dc7c2de311c400a7581347a80b33d9b8b34c25a4`. The starting tree was clean.
The final integration commit is supplied in the task handoff; verify full HEAD before session 4.

## Boundary and reconciliation

This slice integrates shared presentation into the actual Studio and the existing Theatre
skeleton. It introduces no listener catalog/account/player, authentication system, editor feature,
provider request, CMP code, backend rule, schema, database, worker or publication change.
Studio reducers, HTTP adapters, immutable snapshots, permission enforcement, casting and retry
commands remain their existing owners. Native buffered fields retain NodeRefs, IME/blur/Escape,
selection and undo handling. The backend remains the authority on every durable gate.

A read-only `git ls-remote` discovered develop had advanced to
`03ba1416aa0e646b1d6022534f97e44f5b2ea1a4`. The requester then explicitly authorized reconciliation
of [PR29](https://github.com/loveoverflowcom/cantos/pull/29), whose implementation commit is
`143c42b5e4cd5bb0a7b2f29738d397a546edcfeb`. Its diff and
[casting evidence](studio-provider-casting.md) were read before integration resumed. Session-owned
edits were snapshotted under `/tmp/cantos-ui-session3-snapshot` and in a named Git stash. The
clean branch merged develop locally at `83fbadb40c7655d5721c6c8b346638390ef49c74` (first parent
component HEAD, second parent the new develop SHA). Stash application exposed two explicit
conflicts: casting field/select anatomy and Studio CSS. Both were resolved while retaining PR29's
neutral/inherit choices, provider/model switching, reactive bounds, unsupported values/help and
focus rules. No rebase, reset, force operation, old client-shell merge, push or PR occurred.
The recoverable session stash remains local, alongside the external snapshot.

An in-progress session-3 zero-entry attempt and its redundant test were removed before
reconciliation. PR29 owns that behavior. The integration diff against the merge parent leaves
`production.rs`, `messages/production.rs`, the catalog fixture, contracts and service files
unchanged. Imported PR29 backend/browser evidence remains historical; it is not a fresh Mac run.

## Implemented consumers and presentation intent

Studio prioritizes the current script and exact input context, then save/review actions; its
chrome stays quiet and task-oriented. Theatre has a calm reading shell and an honest unavailable
state. Canonical v0.2.0 palette, font names/fallbacks, semantic roles and official offline icons
are unchanged. No artwork/font bytes or external font load is added. Both roots select canonical
comfortable density for 48px control targets; Studio retains compact pane spacing. These are
source settings, not measured browser hit areas.

The [package consumer map](../../packages/ui-system/leptos/README.md#consumer-map) records:

- Studio UiRoot/Container, language Button, appearance/session fields, controlled pane FilterChips,
  revision/validation/casting actions, Status and explicit draft-discard Dialog.
- Theatre UiRoot/Container/Surface/Stack/EmptyState and a native theme Select; no playback action.
- Studio-local `WorkspacePane`, ActionButton, FormSubmit, ContextSelect and DiscardDraftDialog in
  [`ui.rs`](../../apps/cantos-studio/web/src/ui.rs). They own presentation state/mapping only.
- Buffered editor/casting native controls use FieldLabel/FieldError and shared control anatomy.
  Real form submit remains `type="submit"` with original form handlers/guards, including Enter.
  Non-submit actions retain Button's guarded dispatch and visible bilingual blocker reasons.

All four workspace panels remain mounted; navigation changes `hidden`, preserving drafts and
local state. This is ordinary native pressed-button pane selection, with no route/router or ARIA
tab widget added. Native submit/action controls use actual disabled states with their existing
application guards; permission failure still preserves draft state in the unchanged reducer.
Explicit revision discard now confirms in a native modal; cancel/Escape keep the draft, confirmation
rechecks availability, and browser/focus errors are visible. Other existing discard/navigation
paths retain their browser-owned confirmations and beforeunload protection.

ContextSelect retains PR29's memoized selected value and choice list, so ordinary selection
changes do not recreate equal native option lists. It binds selected on initial/late options;
unknown values remain in both the options and complete error note. The standalone shared label
keeps help out of the accessible name. Migrated selects use shared `<id>-error` associations in
place of `<id>-unsupported`; buffered numeric controls retain their existing IDs and reactive
bounds. Zero is passed unchanged, distinct from empty/inherit. No library API changed.

Studio removes its duplicate generic control palette/shape rules. Native anatomy is shared;
app rules cover panes, reading/export geometry and host margin only. Selects no longer impose a
fixed text height. Section grouping uses tonal surfaces/spacing with fewer decorative borders.
App selectors are scoped to `.studio-shell`/`.theatre-shell`, apart from body margin resets.
Studio pane queries use the actual `studio` container width; wide editor panes fold progressively,
comparison/casting regions become one column, and long controls/IDs wrap. Measured 320/360/768/
desktop reflow and both-theme acceptance still require runtime evidence.

## Claim and evidence ledger

| Claim / invariant | Failure mode | Owner / oracle | Evidence held |
| --- | --- | --- | --- |
| Presentation depends on packages, never services or the other app | private Studio/backend code enters Theatre | manifests, unchanged HTTP boundary, focused target lint | source-reviewed, statically-checked |
| Pane selection changes visibility while retaining app drafts | pane switch loses editor/production state | typed WorkspacePane, actual pane-selection callback test; mounted shell source | type-enforced, example-tested; browser preservation NOT_RUN |
| Form submit remains native; actions never become submit and disabled actions retain reasons | Enter lost, accidental submission or silent block | FormSubmit/ActionButton anatomy + production state mapping tests + existing library guard tests | example-tested; browser events NOT_RUN |
| Only an open, available, accepted discard request closes/dispatches | repeated hidden activation discards later text or blocked confirmation closes | actual `confirm_discard_request` callback seam: blocked, rejected, accepted and repeat | example-tested; modal Escape/restore NOT_RUN |
| PR29 casting semantics, raw zero and unknown values survive | provider/model drift or explicit0 lost | unchanged production reducer + four PR29 examples; actual ContextSelect fixtures and memo source | example-tested, statically-checked; trusted keyboard after migration NOT_RUN |
| Shared label/error/reason IDs remain distinct and serialized text stays escaped | help pollutes names or control descriptions dangle | actual consumer HTML fixtures plus 35 shared tests | example-tested; DOM/AT NOT_RUN |
| Scoped CSS uses semantic roles and preserves focus/48px source targets | app styles override modal controls or clip focus/text | complete manual cascade/token review, drift/contrast regressions, WASM build | source-reviewed, compiled; measured bounds/contrast NOT_RUN |

The functional core added here is small presentation mapping/callback guards; browser input/focus
and Leptos are the shell. No durable rule moved into UI and no new settled record is written.
WorkspacePane excludes invalid numeric pane states; DisabledReason validates both locales and
invalid catalog text removes a control rather than enabling it. No new schema/deserialization or
publication/cache/worker path exists. Crash/provider/lease/cancellation/cost-reservation passes are
not applicable to the UI-only diff; their production gates remain unchanged.

## Source review and corrections

A read-only self-review applied `cantos-code-review`, engineering, UI/Leptos and
`personal-ui-design` criteria to the complete integration source/manifests/lock/CSS/docs and actual
consumers. The comparison is the integration working tree against reconciliation HEAD above;
the original branch range also contains the two prior UI commits and inherited PR29. No subagent
or session 4 was started. No backend review or independent final assurance is claimed.

Confirmed source findings were repaired in the implementation lane before the final checks:

- The discard callback initially lacked an open-state guard. It now refuses dispatch after close;
  the production callback seam regression covers a second request after accepted close.
- Scoped label specificity would have overridden existing checkbox-label flex layout. Low-specificity
  label selectors retain component/app display contracts. App shrink rules also stopped overriding
  shared minimum button width, preserving the canonical control target in source.
- Casting reconciliation preserves memoized options and full unsupported notes. The PR29 invalid
  outline remains restricted to `:not(:focus-visible)` and the shared focus ring stays separate.
- Dialog error copy covers both modal and return-focus failures without asserting that a draft was
  retained after an accepted discard. Failure status is opt-in polite feedback.
- Final distribution inspection found Studio still linked only tokens/app CSS. Shared components
  require `foundation.css`; Studio now links it between tokens and app geometry. A regression
  parses both real Trunk entrypoints and requires exactly one of each stylesheet in that order.
  Fresh distribution inspection then verified all three linked CSS assets byte-for-byte against
  their sources in both apps. A successful WASM build alone had not detected the missing style.

Assessment: `no-actionable-findings` for the source comparison after these repairs;
`insufficient-evidence` for browser/visual/accessibility acceptance. Coverage is
`complete-for-declared-scope` for changed UI source and supporting docs, and `partial` for runtime.
The unchanged backend/worker/CMP/native mobile/CI implementations were not reviewed or run.
Required runtime gap (category: evidence, confidence: confirmed-from-source/tool): UI-PIXELS and
UI-A11Y cannot be established from compiled code/native strings; session 4 needs the matrix below.
The requester explicitly allowed source/build completion with runtime NOT_RUN when Mac stayed locked.

## Provenance and execution

Read-only local materialized design references were used; no repeat Drive download was needed.
Recomputed manifest, Theatre HTML and Studio HTML SHA256 values exactly match
[foundation provenance](ui-system-foundation.md#source-provenance). Those files are static source;
Drive PNGs are boards, not browser captures. Private IDs/URLs, raw HTML, artwork/fonts and media
stay outside Git. Native tests use real consumer/library code and existing synthetic contract
fixtures; no HTTP service, database, account credential or provider participates.

Python 3.13.5, Rust/Cargo 1.87.0 and Trunk 0.21.14 on macOS Apple Silicon. Build jobs limited to 2,
existing target reuse, one heavy lane at a time. Output lives in attributable task tool results and
`/tmp/cantos-ui-session3-{native,native-clippy,wasm-clippy,python,studio-build,theatre-build}.log`.
Commands below run against reconciliation HEAD plus final integration edits. Native SSR is enabled
only for HTML-string tests; both browser distributions remain debug CSR.

| Command (root unless stated) | Result / exact boundary |
| --- | --- |
| `python3 scripts/ui_tokens.py --check` | PASS: all canonical mappings/template agree |
| `python3 scripts/ui_icons.py --check` | PASS: eight official SVG hashes/anatomy |
| `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS: 128 Python tests, including the real consumer stylesheet-loading regression |
| `CARGO_BUILD_JOBS=2 cargo test -p cantos-ui -p cantos-studio -p cantos-theatre --locked` | PASS: 35 shared UI, 127 Studio (including seven consumer tests and four PR29 additions), 1 Theatre; zero doctests are not evidence |
| `CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui -p cantos-studio -p cantos-theatre --all-targets --locked -- -D warnings` | PASS: focused native targets |
| `CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui -p cantos-studio -p cantos-theatre --all-targets --target wasm32-unknown-unknown --features cantos-ui/preview --locked -- -D warnings` | PASS: shared/gallery source and both app WASM targets |
| `CARGO_BUILD_JOBS=2 NO_COLOR=true trunk build --locked` in `apps/cantos-studio/web` | PASS: actual debug CSR distribution |
| same Trunk command in `apps/cantos/web` | PASS: actual debug CSR skeleton distribution |
| `cargo fmt --all -- --check`, `python3 scripts/check_repository.py`, `git diff --check` | PASS after final edits |

Initial development failures are not counted as passing evidence: the first locked check required
an offline lock update (only the Theatre dependency edge changed); unsupported ViewFn construction
and a moved label ID were corrected; an overbroad dialog assertion matched English text instead of
the dialog tag; the first error+reason fixture still expected reason alone, then asserted the exact
composite association. Wrong-cwd edit snippets failed before writes and were rerun at the root.
The final passing commands above replace those attempts. No dependency version changed or large
SDK/toolchain/model was installed. Gallery Trunk, full workspace/backend Rust, PostgreSQL,
Script IR reference/mutation, WAV/provider/audio and mobile checks were NOT_RUN for this scoped
UI diff. PR29's existing real backend/browser results are not recast as fresh integration evidence.

Final ignored debug distributions contain one WASM and three CSS assets each. Their generated
HTML links tokens, foundation and app CSS in order; all six CSS asset bytes match their source.
WASM build identity (not runtime evidence):

| App / file under its `web/dist` | Bytes | SHA256 |
| --- | ---: | --- |
| Studio / `cantos-studio-777ed208edd3881a_bg.wasm` | 38,095,281 | `3228ba861de4aad95154db10297ccbc584b5f25aaf7bb7e63c094654d0c549f5` |
| Theatre / `cantos-theatre-8bb9e5043a118ce0_bg.wasm` | 2,038,085 | `61a2bd7c7e37ce1b98b8ca6da27bd4ecdf0246cc0a1717cc95e31519c1f86592` |

## Runtime gap and session 4 handoff

Computer Use `cua.getState()` initially and at the final availability recheck reported no
apps/browsers and: the Mac is locked, and automatic
unlock could not unlock it. No bypass, security setting change, alternative browser driver or
unbounded wait was attempted. No app server or browser session remains running from this task.
All app runtime claims are NOT_RUN: baseline/after pixels, exact viewport dimensions, horizontal
overflow, long Vietnamese diacritics/wrapping, light/dark appearance, real 200% page zoom/text
scaling, 48px measured touch bounds, keyboard/focus/IME/undo, Enter submission, rapid Select keys,
modal Escape/Tab/Shift+Tab/return focus, forced colors, reduced-motion input/recording and VoiceOver.
Native strings do not supply DOM-tested, interaction-tested or accessibility-checked evidence.

Session 4 should independently review the exact final HEAD, using merged develop as the UI
comparison base and the original `dc7c2de…` range for historical context. Inspect 320/360/600/768/
840/1440 CSS px, light/dark, Vietnamese/English and actual 200% zoom. Drive session Enter → pane
selection → buffered editing/validation/save failure/conflict/retry, then PR29 mixed casting
(provider/model bounds, retained values, neutral0/inherit) and discard cancel/Escape/confirmation.
Use a fresh disposable database if a real backend journey is run; never inherit a production URL.
Do not rely on the gallery or PR29 Linux captures to accept these changed Mac surfaces.

Completion: work item 015/session 3, presentation owner and app state boundaries above;
evidence levels type-enforced, example-tested, statically-checked, compiled. Native examples and
local design hashes supply provenance; no live/mocked provider result or media path exists.
Docs/consumer map/queue rationale are updated; linked GitHub execution status is unchanged.
Only authorized local reconciliation/integration commits are made. Session 4, push, PR, merge to
develop, CI monitoring, Docker and shutdown are not started by this task.

Residual risk: runtime matrix remains required and blocked by the locked Mac; font distribution
rights remain unresolved, system fallbacks are in use, Theatre remains a skeleton, CMP remains
deferred, and no production/audio/listening acceptance follows from this integration. Initial free
disk was 2.1 GiB; final available disk is about 1.5 GiB (also recorded in the task handoff). No cache/model/audio/database
was deleted and no unrelated process was stopped.
