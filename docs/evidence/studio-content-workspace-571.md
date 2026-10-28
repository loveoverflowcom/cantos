# Studio content workspace by task — #571

Source-bound observation, 28 October 2026, for [#571](https://github.com/loveoverflowcom/cantos/issues/571).
Base `develop@95fa9de267c05ee999ac55ae3eb92d65f504eb1d`. This records what ran. It does not accept
the issue: [#570](https://github.com/loveoverflowcom/cantos/issues/570) owns the typed lifecycle,
operation-bound feedback and keyed-row projections and is **not implemented here**, and no run used
the real Server, Identity or a real cookie session.

## Review boundary

Delivers: the existing `ContentWorkspace` composed by task (context header, chapter rail, one task
area at a time, width-limited creation forms, separate archive zone, single-pane navigation on narrow
containers), the editor status line, a head-versus-historical revision viewer with the raw document
behind a labelled disclosure, recovery controls shown only when applicable, and owned copy
corrections. Does not deliver: backend, contract, auth, autosave, new state store, topbar/shell
(#558/#131), structured-authoring/production/inspector migration (#500), per-operation feedback and
keyed rows (#570).

Every action and callback of the previous composition is still reachable: works/chapters lists,
paging, create Work/chapter, Work and chapter metadata, reload, ordering, archive, read/append/
history/revision, editor entry, artwork, delegation, brief, private audio, preview, review,
candidate and publication. No new capability (restore, publish, TTS) was added.

## Claim and evidence ledger

| Claim / invariant | Failure mode | Oracle | Evidence |
| --- | --- | --- | --- |
| A task area exists only for the selection that supports it; exactly one is shown | a unit-bound area with no chapter, a second area visible | `live::layout::tests::{a_unit_bound_task…, exactly_one_area…, tabs_never_offer…}` | native unit, PASS |
| Status never claims an unacknowledged save; missing head ≠ failure ≠ empty revision | "saved" shown while saving, dirty or archived | `editor_status` / `saving_selected` tests | native unit, PASS; browser: chip read *saved rN* only after the append reply |
| Archived parent makes new writes unavailable in the UI, reads stay | save/create-chapter active on an archived Work/chapter | `read_only_lifecycle` tests; browser archive journey | native unit PASS; synthetic browser |
| Opened historical revision never replaces head or the draft | history read overwrites the save base | `viewing` tests; browser | native unit PASS; synthetic browser (draft text and head chip unchanged) |
| Hidden areas keep drafts and focus targets | unmounting a pane drops a typed draft | areas are `hidden`/`inert`, never unmounted; existing draft owners untouched | source; draft continuity across tab switch **not** separately measured |
| No new generic disabled reason repeated | each disabled action printing the same sentence | measured visible reason lines | synthetic browser |

## Observations (actual Studio WASM, synthetic same-origin API)

Chrome-family built-in browser (version not exposed), debug CSR build served by `trunk serve` with a
scratch config; an in-page `fetch` stub supplied session, Work/unit/content endpoints for two Works and
three chapters (one with two saved revisions); everything else answered 404. This is **not** the
Server, Identity or a cookie session, and the stub is not committed. The "before" is a detached
build of the base commit with the identical stub and clicks.

Same data and state (Work 1, chapter 1 open, head read), Dark, 1280×900 CSS px:

| Measure | Base `95fa9de2` | This change |
| --- | --- | --- |
| Page height | 10 092 px | 1 285 px |
| Writing area top (document offset) | 4 270 px | 496 px |
| Visible filled primary buttons | 49 | 1 (Save content) |
| Visible repeated disabled-reason lines | 28 | 0 |
| Page horizontal overflow | not measured | none |

After-build layout checks (same stub): 1280 two columns with a sticky context header and rail; 768
(container 721 px) two columns `272px 425px`, no overflow, no clipped element; 390 and 320 single pane
with the back control, tabs wrapping, no horizontal overflow and no element past the viewport; Light at 1280 renders
the same layout with readable selected-row and tonal-button colours.

Journey exercised in the real WASM against the stub: load works → select Work → select chapter
(head read) → type (chip *unsaved changes*) → Save (chip *saved revision r3*; Work r2) → read history
(r3, r2, r1) → open r2 (viewer says historical, head chip stays r3, draft intact) → archive chapter
(chip *archived — read only*, Save disabled, text fields read-only, *read current content* still
enabled) → archive Work (create-chapter, Work save and archive disabled).

## Checks

| Layer | Command | Result |
| --- | --- | --- |
| Localization source/generated | `python3 scripts/localization_catalog.py --resources` then `--check` (3.14 shim) | regenerated; Theatre Web/CMP files differ only by the catalog digest |
| Studio native | `cargo test --locked -p cantos-studio` (`apps/cantos-studio/web`) | PASS, 419 + 5 tests (417 + 5 before the last two layout tests; rerun of `layout` alone: 10 PASS) |
| Studio lint | `cargo clippy -p cantos-studio --all-targets [--target wasm32-unknown-unknown] --locked -- -D warnings` | PASS (native and wasm32) |
| Format | `cargo fmt --check -p cantos-studio` | PASS |
| Handoff (partial, `--skip` below) | `cargo xtask preflight --changed-only --base origin/develop --skip repository --skip rust-tests --skip turso --skip forge-functions --skip audio-inspection --skip wav-reference --skip script-ir-reference --skip protobuf-reference --skip contract-tests --skip mutation --skip behavior --skip kani --skip cmp-ui-preview --skip theatre-cmp-preview` (python3 → 3.14 shim) | PASS: `rust-format`, `rust-lint (workspace)`, `web-lint (cantos-studio, cantos-theatre)`, `web-build` ×2, `ui-preview`; the 11 skipped gates are `NOT_RUN` |
| Repository gate | `python3 scripts/check_repository.py --base origin/develop` | FAIL on clean `develop` and on this tree with an identical finding set (34 UI-qualification lines: stale `*/leptos` qualifications and "new use requires both preview qualifications"); the two findings this change introduced (missing final newline in `catalog.json`, a literal `›` in the view) were fixed and are absent |

Skipped gates and why: `rust-tests`, `turso`, `forge-functions`, `contract-tests`, `mutation`,
`behavior`, `kani` and the reference/audio gates exercise nothing this change touches (no Rust
service, contract or Quint model changed); `cmp-ui-preview` and `theatre-cmp-preview` need Gradle and
the generated Kotlin changed only by the catalog digest header. The changed-only plan selected every
gate because `catalog.json` has no known impact, so none of these is evidence for or against.

## Not covered

- AC-04 (typed per-operation feedback, keyed rows) — waits on #570.
- AC-05 beyond the above: IME composition, caret and focus return across pane/theme/locale switches,
  and focus after Save were not exercised; the Save button is disabled while the request is pending,
  as before.
- AC-06: before captures only at 1280 Dark; no 200 % text, English long strings, forced colors,
  keyboard walk or reduced-motion observation; DPR and browser build unknown.
- AC-07: create-Work/create-chapter forms, metadata edit, a load/validation error and a
  conflict/unknown scenario were **not** run in the browser; no real-API or #565 journey.
- Child panels (artwork, delegation, brief, audio, publication) keep their own button hierarchy and
  copy; only their placement changed.
- The initial Work list still loads only from the dashboard or an explicit refresh (#131).
