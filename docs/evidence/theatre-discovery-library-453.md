# Theatre Discovery and Library: bounded #453 source evidence

## Continuation with the exact #459 dependency

Feature merge `20134b9a7093f66a65cf172f1eef31c10427754f` integrates #459 at
`a5a13c7b2c2e09b772414905a9366c7132510401` and preserves the earlier #461 history.
This is a feature-branch dependency, not permission to merge #459 or its combined commits
into `develop`. The [dependency's final report](ui-qualification-459-final.md) admits 12
bounded APIs through 24 receipts; independent exact-tree reconstruction matched all three
token/renderer digests. App-only adoption does not change those shared inputs or receipts.

The continuation adopts qualified Logo Symbol (44 units, 12-unit gap), the existing Floating
Web field's trailing slot, CMP FloatingSearch and the shared Library Button. Hero containment
and bold headings use the dedicated canonical Theatre roles locally. No global expression
map is copied, no palette is invented and no unqualified API is substituted with a clone.
Web keeps a visible-label submit Button until IconButton admission; CMP keeps query-on-change
and its existing Clear action without adding a pretend submit or kind-filter policy.

**Avatar, IconButton, FilterChip and UiRoot/Theatre expression remain unqualified.** Full text
avatar/tonal-header controls, icon-only Web submit and root/palette/type adoption still depend
on their two-renderer qualification. Resolving the former 32 diagnostics does not admit them.

Fresh continuation evidence is separate from the historical checkpoint below:

- `cargo test --locked -p cantos-theatre --features preview`: 489 unit tests and 3 doc tests PASS.
- `cargo xtask verify web`: both Web lints/builds and the shared Leptos preview PASS.
- `cargo xtask theatre-preview verify`: 528 shared tests (3 skipped), 137 sandbox tests
  (2 skipped), zero failures/errors. A 2 px centerline finding was repaired; the new assertion
  checks Logo and theme control at the same centre. JVM PNGs/sidecars are under
  `artifacts/ui-evidence/453-resume/cmp-after/`; their original working-tree attribution remains.
  The README clarification happened after capture; tested rendering-source bytes are unchanged.
- Repository checker PASS, including qualified imports and unchanged receipt bindings. The
  full Python suite FAILS two unchanged retired-VPS synthetic cases on this Mac: BSD `sed -i`
  syntax and missing `flock`. These are environment limitations, not live VPS operations or
  evidence that the full repository gate passed. No checker/test/receipt was relaxed.
- Fresh Web observations used the real loopback WASM app with API unavailable: 390 light VI,
  320 light VI/dark EN and 1280 light EN; long query, Tab to the trailing action, Enter and Clear
  back to input focus were observed. Locale/account/settings paths remain. Browser zoom shortcut
  produced no measured scale change, so the real Web 200% matrix remains NOT_RUN.

The continuation logs are under `artifacts/agent-runs/453-resume/`. Final preflight/CI results
belong to their exact published head in the PR, not the old `a8a2e7c9` results. On the integration
head `20134b9a`, remote repository/Web passed, Rust was cancelled and native qualification was
skipped; these are not CI results of the later app-only working changes. Full product/IME/AT,
Android/iOS, wide Library and production acceptance remain pending. The fixture denial and
dependency merge-approval restriction remain in force. Queue order is unchanged.

## Historical checkpoint at a8a2e7c9

The following record preserves the earlier independent source checkpoint. In particular its
32 qualification failures and blanket adoption deferral are historical, superseded only by
the precise continuation scope above; its runtime gaps are not silently converted into PASS.

This is an implementation/evidence boundary for [#453](https://github.com/loveoverflowcom/cantos/issues/453),
not full design admission, native-device acceptance or production acceptance. The PR and its
check output own the final execution status. Base: `origin/develop` at
`fa114100baea6919515a29d4a5b4d9dcb573afa1`; integration target: `develop`.

## Review boundary and prerequisite

The source increment makes app-owned Discovery cards compact, packages the actual empty artwork,
and preserves search, navigation, pagination, Library eligibility and independent save actions.
It changes neither service contracts nor auth, publication, playback or durable state policy.

[#457](https://github.com/loveoverflowcom/cantos/pull/457) merged foundation source, but its
[record](theatre-expressive-foundation-452.md) leaves component admission incomplete. The existing
32 qualification diagnostics and the continuation in [#459](https://github.com/loveoverflowcom/cantos/pull/459)
are inherited evidence, not a new #453 pass. IconButton, Avatar, FilterChip and the Theatre
expression have no admitted two-renderer records. No shared source, qualification digest,
compatibility baseline or catalog is changed here.

Component lookup: `button`, `text-field`, `icon` and the existing app imports remain in place.
The new v4 `icon-button`/`avatar`/`filter-chip`/Theatre-root variants are deferred. CMP composes its
already-used stable Material IconButton/Card APIs and existing typed intents; it does not copy
the v4 implementation or claim its tonal presentation. Web keeps the existing Floating field
and submit owner. The official combined-logo, text-avatar, expressive palette and docked search
requirements remain incomplete pending admission. The queue order and rationale are unchanged.

## Sources and assets

The [owner v4 folder](https://drive.google.com/drive/folders/1CT8-icdYTGtCJQCNzb3jP3AwCxMnLFZW)
and [full package](https://drive.google.com/file/d/16A3X5SkhV4lPDiBbGp5K-da_ryY1kk3w/view)
were fetched through Drive. The implementation handoff, proposal, manifests and review log were
read. Boards 1, 2, 3 and 6 and the actual Library PNG were opened before visual edits.
All 74 manifest entries matched their downloaded bytes. Package SHA-256:
`2d9778d86abeb0be05f5d3718c8087c6bae3dc3874da96e30ae8dfc95764228d`.

Library uses the transparent **1374 × 1145 raster PNG**, SHA-256
`96babcc01a2f692c941fcc199d30d521c97f9ccc95b65381571cbc66004a302a`, not the original Library SVG.
Works and History use the supplied light/dark SVGs on Web and actual Compose-resource XML on
CMP. The XML compiled and rendered on the JVM; this does not establish Android/iOS rendering.
All images are decorative, untinted, preserve aspect ratio and use app preferences for theme.
Web [asset provenance](../../apps/cantos-theatre/web/empty-artwork.md) and CMP's bundled JSON
retain exact source hashes. No sample cover, manuscript, audio or runtime Drive fetch was added.

## Invariants and state coverage

| Claim / failure mode | Owner and cheapest evidence | Boundary |
| --- | --- | --- |
| A card has one navigation target; its arrow is decoration | Web PublicHome HTML tests; CMP production presentation semantics and owner-flow tests | Save remains independent on CMP; Web public catalog has no save control |
| Reflow/paging does not replace Work identity | Web keyed For including the first result; CMP keyed direct FlowRow children and reordered-render test | Navigation and one process-wide player owners unchanged |
| Successful complete emptiness is different from loading/error/partial/stale | Existing Library projections; extended Web/CMP presentation tests and actual owner recipes | No empty success fabricated from failed or intermediate reads |
| Empty catalog does not link to itself | Web completed-empty/no-results tests; CMP distinct copy/action tests | Filtered query keeps clear/recovery; incomplete empty page keeps pagination |
| Search retains its platform contract | Web Enter/Clear/history observations; CMP query-on-change owner tests | Web IME guard unchanged; actual OS IME not run |
| Localized action and content title keep different language tags | Web mixed VI/EN aria-labelledby regression | Real screen-reader pronunciation not run |
| Clear does not abandon keyboard focus | Actual Web Clear → input observation; connected same-mount frame guard in platform helper | Late-disposal and focus-nonstealing runtime matrix remains unobserved |

## Attributable checkpoints

The exact commands, source identities and final summaries belong in the PR. Local full output
and captures are ignored under `artifacts/agent-runs/453/` and `artifacts/ui-evidence/453/`.

- Web baseline `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify web`: PASS, before this source delta.
- Focused final Web: PublicHome 15, Library 17, contextual History artwork 1: PASS. These are
  host-rendered production component tests, not a browser populated-flow result.
- CMP baseline and changed-source `cargo xtask theatre-preview verify`: PASS at the named
  checkpoints, 528 shared tests (3 skipped) and 135 sandbox tests (2 skipped). Tests use real
  production composables/controllers with mock external ports and an in-memory Library.
- New-source CMP failures were repaired: retained Work error call missing theme argument,
  generic test-list inference, obsolete generic-empty copy expectation and header DSL receiver
  shadowing. Failed output is retained where available; the first missing-theme log was
  overwritten before preservation, so no complete original-log claim is made.
- Final handoff requires the canonical changed-only preflight on this exact base. Its repository
  result is independent of passed renderer/core checkpoints; no skip or digest renewal accepts
  inherited qualification failures.

## Visual and interaction observations

Surface intensity is Medium for Discovery/Library. Compact content leads, while the full v4
color/type/header treatment remains deferred. No new semantic tokens, palette or font assets
were introduced. VI/EN use their existing copy owners; new CMP catalog-empty/theme strings are
paired. Motion captures use the injected reduced policy and do not prove animation smoothness.

CMP captures use the JVM Desktop Compose test root, logical/physical 1:1 pixels, fixed fixtures,
VI/EN, light/dark, 390 × 844 and 1100 × 900, plus 320 × 800 with actual injected 2× font scale.
The 43 before and 44 after PNGs include the existing 36-cell Library matrix and bounded new
Discovery/Search/History recipes; not every capture was opened. Metadata and semantics accompany
each PNG. `cmp-after/source-inputs.json` binds the dirty patch and every changed/new source hash;
it does not mislabel the pre-commit source as clean HEAD. All artifacts are local-only.

Opened findings include compact Discovery390VI, wide DiscoveryEN, narrow Discovery2×,
Search390darkVI/Search320darkEN2×, Library empty and retained/stale states, and History light/dark.
The source action tests distinguish whole-card navigation from save. XML History artwork is
actually visible in the opened JVM capture. A scrolled narrow card capture shows its wrapped
title, arrow and separate save action. A new header midword-wrap regression at 320/2× was fixed
by reflowing the same two mounted groups; the original-resolution same-scenario recheck shows
Cantos on one line. A suspected missing-draw finding from small previews was disproved by
reopening the original PNG and inspecting the exact semantic-region pixels; no image or golden
was changed to clear it. Semantics alone were not used to accept painted glyphs.

Actual Web runs used the loopback WASM app via `cargo xtask app cantos-theatre --web`, with the
API unavailable. Browser captures were opened at 1280 × 800 light and narrow 390/320 light/dark;
the guest/account/settings paths remained available. Enter wrote the encoded query; Clear
removed it and left `document.activeElement.id == theatre-discovery-search`; Back restored the
query and Forward cleared it. This is interaction-tested for the unavailable-data shell only.
The in-app browser engine/version and real 200% Web text matrix were not established. Browser
populated/Library artwork and cards are not accepted by native HTML tests or JVM pixels.

## Deferred acceptance and blockers

- Shared foundation admission: pending actual two-renderer runtime/accessibility/visual evidence;
  the known Dialog focus, incomplete matrices and CMP window-input gaps belong to #452/#459.
  Do not expand #453 into that task or refresh hashes to clear its diagnostics.
- Populated/Library Web browser: no real local API was running. Automatic approval review denied
  the proposed read-only synthetic catalog HTTP fixture because of the explicit no-mock-fallback
  constraint. The process was not started; no retry or workaround was performed. Resume only with
  explicit relevant user approval or an authorized real test environment.
- Android/iOS: read-only device inventory found no booted simulator or attached Android device.
  No host build/install, native IME, TalkBack or VoiceOver result is claimed. Use canonical
  `theatre-host` commands with configured targets before named-device acceptance.
- Motion interruption, real OS IME, screen readers, wide CMP Library runtime, Web 200% long VI/EN, and native/public-cover
  delivery remain NOT_RUN. No production deployment, provider call, credential read, seed or
  migration was performed. Next acceptance owners are unassigned; assignment is required.

Review and layer results do not complete #453, close #416/#417, grant merge through failed gates
or authorize #454. The source increment remains reviewable in Draft until required gates and
the deferred admission/acceptance obligations permit completion.

## Follow-up #467: centered empty group, pill locale, Avatar deferred

Source base `bc0cc7b0`. The Web-only delta is app CSS plus focused regressions; it adds no shared
source, token, catalog or admission change.

- **Empty group.** `.theatre-public-empty` is one centered flex column (artwork, heading, help,
  Refresh) with content-driven height. The artwork resets its row-oriented `flex` basis, which
  would otherwise become a 16rem column height. Completed-empty and filtered no-results share it;
  loading, incomplete, expired and failed regions do not.
- **Locale pill.** The topbar `Select` keeps the admitted compact/inline API. App CSS scoped to
  `.theatre-locale-field` gives the control `--cantos-shape-pill` and the tonal
  `surface-container` role, and centers value and chevron as one group. The shared Theatre
  expression that already styles this role is unqualified and is not enabled globally; labelled
  form selects keep the field radius. Retire the local rule when the expression is admitted.
- **Avatar deferred.** Shared `Avatar` is still `registered` (Leptos) and `implemented-unverified`
  (CMP) with no two-renderer qualification record, so the account entry keeps the app-private
  `PersonGlyph`. Initials, the guest `C` and image-error fallback bind only after admission
  (#452/#459 owner); the component is not copied. Resume: Avatar qualification receipts at the
  current catalog digest, then bind a resolved display name or the guest fallback.
- **Observed** (Trunk dev server, Chromium pane, dark, real shell and stylesheet): the empty
  markup was substituted into the live region because no API was running, so this is a
  presentation observation, not an API-backed one. At 390 and 1280 the artwork, heading, help and
  button centerlines equal the panel centerline (195 / 740.5 ± 0.5); no horizontal overflow at
  390. The locale trigger measured 134×48, radius 999px, value + chevron centered in the trigger.
  A light-scheme capture showed the pill. 320, 200% text, EN long labels, keyboard/typeahead on
  the pill and production deployment are `NOT_RUN`.
