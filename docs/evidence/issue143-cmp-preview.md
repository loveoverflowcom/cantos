# Issue 143 — CMP preview v3 foundation

[Issue 143](https://github.com/loveoverflowcom/cantos/issues/143) is the CMP counterpart of the
merged Leptos workbench ([evidence](leptos-preview-v3.md), [inventory](ui-design-inventory-v3.md),
[token reconciliation](../design/ui-token-v3-reconciliation.md)). This record covers its first
slice only. It qualifies no new API, grants no Android/iOS, screen-reader or product acceptance,
and does not change the canonical tokens, the preview registry or the admission contract.

```text
Item / issue:        016 CMP UI-system sequence / #143 — [P1][CMP][Preview v3] component catalog,
                     theme injection and token mapping for Theatre
This PR delivers:    the CMP gallery gains design discovery from the shared inventory (Design v3
                     tab), appearance injected as values (Light/Dark/System resolved only at a
                     composition root), named dp evidence constraints with a live probe line,
                     JSON capture sidecars, three Android Studio wrappers, Noto font delivery to
                     the desktop gallery host with nested-theme inheritance, and the CMP skill/doc
                     updates; the six qualified CMP records are re-bound to the new source digest
It does not deliver: new tokens or shape/size roles (deferred by the reconciliation), any Theatre
                     composition, Noto delivery to the Android wrappers or a product host, a change
                     to the shared line-box model, Android Studio/device/iOS/screen-reader
                     evidence, a new admitted shared API, anything on the Leptos side
Acceptance covered:  see "Deferred acceptance and residual risk"; remaining items stay with the
                     follow-up batches in the 016 work item
Closes the issue:    no — "Part of #143"
Decision record:     none needed — the slice selects nothing the docs leave open; the inventory,
                     token deltas and deferrals are already recorded by the #141 reconciliation
```

## Invariant, owner and decomposition

Design status, registration and qualification are three separate facts. The CMP gallery now shows
the first (and ownership) from the generated, display-only `DesignInventory`, keeps the second in
`PreviewCatalog`, and takes the third only from `qualifications`. A proposal never gains a sample,
an export or an adoption path by being displayed.

| Responsibility | Owner after this change |
| --- | --- |
| design specimens, scopes, owners, anatomy, token roles | `shared/design-inventory-v3.json` (unchanged, merged by #150), validated by `scripts/ui_preview_catalog.py` |
| Kotlin view of that data | generated `preview/generated/DesignInventory.kt`; under `preview/`, so the Theatre mock's build (`kotlin.exclude("com/cantos/ui/preview/**")` in `apps/cantos-theatre/mobile/shared/build.gradle.kts`) never ships it |
| filtering, counts, selection | pure `preview/DesignInventoryModel.kt` (`commonTest`) |
| rendering the tab | `preview/DesignInventoryView.kt`, wired by `PreviewApp` |
| theme mode | pure `ThemeMode.resolve` in `ComponentState.kt`; roots (`Main.kt`, three Android wrappers) own the OS query |
| preview-local geometry, dp constraints, page-scroll rule | `preview/PreviewGeometry.kt` (`PreviewGeometry`, `ViewportPreset`, `scrollsWholePage`) |
| capture records | `desktopTest/.../CaptureRecord.kt`, used by `PreviewCaptureTest` |

Imports still point inward: `preview` → shared components → tokens. No dependency, module, service,
port or mock owner was added. `UiPreferences` still carries only a resolved `Theme`. The only
shared (non-preview) source touched is `ComponentState.kt` (`ThemeMode`, `resolve`); no
implementation file of the six qualified components changed.

## Ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Live / mocked |
| --- | --- | --- | --- | --- |
| existing specimens are exactly the 32 registered APIs and native examples; every proposal has neither | a proposal looks adoptable; a baseline API is lost | `DesignInventoryModelTest`, `PreviewModelTest`, Python baseline tests | example-tested (JVM desktop) | real production data |
| the Kotlin view equals the inventory and is deterministic, escaped and registration-free | drift or a qualification word in display data | `test_ui_design_inventory.py` (8 tests), the `verify repository` drift check | statically-checked, example-tested | real data |
| search matches each of id, name, family, scope, owner and consumer, and filters compose | a field silently stops being searched | `DesignInventoryModelTest` per-field queries (each found through one field only) | example-tested | real data |
| `System` follows only the injected fact; explicit modes ignore it | a component reads the OS; a mode ignores the user's choice | `ThemeModeTest` table of six; `system_theme_follows_the_injected_host_fact_…` with a changing fact | example-tested, interaction-tested (JVM) | host fact injected |
| a nested theme cannot re-resolve `System` | a nested root flips with the OS | type-enforced: `UiPreferences` has no mode or host-fact field. No test observes it | none beyond the type | n/a |
| hit target (`size.control`) and glyph (`size.icon`) are separate roles | a visible icon size silently becomes the target | `HitTargetTest` (target equals `size.control`, glyph is smaller). Both densities are 48 after v0.6.0, so it cannot tell density handling from a hard-coded 48 | semantics-tested (JVM) | real component |
| the gallery states its real dp constraint, host density, host font scale × preview multiplier = effective scale | dp inferred from a PNG; host and preview scale confused | `the_gallery_reports_…`, and the line read in the opened captures | interaction-tested, screenshot-inspected (JVM) | real renderer |
| the page scrolls whenever the effective font scale or a narrow window enlarges the header; the inventory stays reachable | a weighted body collapses to zero height and no scroll reaches the detail | `PreviewGeometryTest` table, `enlarged_host_text_…`, `a_fixed_height_page_scroll_region_…` | example-tested, interaction-tested (JVM) | real renderer |
| the expanded layout is chosen on the window, not its padded content | 840 dp window shows the narrow picker | `at_exactly_the_expanded_breakpoint_…`, `one_dp_below_…` | interaction-tested (JVM) | real renderer |
| opening a component from the inventory clears the filters that would hide it | the link lands on an empty list | `an_existing_specimen_opens_its_component_and_clears_…`, `a_reuse_link_opens_…` | interaction-tested (JVM) | real renderer |
| width buttons exist only where a host can resize its window | an inert control in Android previews and test roots | two tests: absent without the hook, called with the chosen preset with it | interaction-tested (JVM) | real renderer |
| a capture sidecar binds scenario, pixels, token file and PNG bytes, and disclaims devices | a PNG is mistaken for device evidence | `CaptureRecordTest` (SHA-256 of `abc` vector, token-file digest, escaping) | example-tested | real files |
| the six qualified components are unchanged | a digest refresh hides a regression | no implementation file changed; baseline vs branch suites and PNG SHA-256 comparison below | example-tested, screenshot-inspected (JVM) | real renderer |
| the Theatre mock and the Android host still build | the shared source breaks a consumer | `theatre-demo verify`, `theatre-demo android-build`, `ui-preview cmp android-build` | example-tested, compiled | real build |
| a family injected at the root reaches every nested theme; a host that injects none keeps the fallback | a nested specimen silently reverts to the platform font | `ThemeFoundationTest` (nested, two-level, explicit override, no-host-families), `PreviewFontsTest` over every gallery text run | example-tested, interaction-tested (JVM) | real theme and gallery |
| the bundled faces are the bytes, family and weight the provenance record names | a different or missing font loads silently | `PreviewFontsTest` against `provenance.json`, and the design HTML's embedded faces by SHA-256 | example-tested | real files |
| the gallery text matches the design's glyphs and metrics | the headline stays sans; Vietnamese marks differ | `visual-compare` pair above, opened | compared, screenshot-inspected (JVM vs Chrome) | real renderers, `unreviewed` reference |

An independent read-only review of the first implementation by a separate agent found a vacuous
filter-reset test, an inventory body that collapsed to zero height under large text in a
fixed-height region, a probe line that labelled the effective font scale as the host's, tests that
asserted existence where they claimed behavior, and documentation that overstated what is
implemented. All were fixed. Opening the captures then found inert width buttons in hosts that cannot
resize, reviewing the Android wrappers found a page-scroll rule that ignored a large host font
scale, and the mutation ledger found that search by consumer was untested. All were fixed, and the
ledger below pins each of them.

## Executed evidence

Environment: macOS arm64, installed Gradle 8.14.3, Microsoft JDK 17.0.19, Rust 1.98.1, Homebrew
Python 3.14 shim for the repository gate, Kotlin/CMP/Material3 2.2.20/1.9.0/1.8.2. Nothing was
installed or provisioned. The slice was implemented and measured on base
`68b2fc6ca22f8d67b515b1b703ddd3ccf95623eb` (develop after #150). The branch was then merged with
develop `26dee0a78a8e1c53f538d8ff32ffc00d2029fc66` (after #151, the Leptos scenario workbench) and
the gates in the second table were re-run on the merged tree. Production composables ran on JVM
Desktop Compose, not an OS window, emulator or phone.

Every command ran from the worktree with `RUSTUP_TOOLCHAIN=1.98.1`, Gradle on `PATH`, `LC_ALL=C` and
`TMPDIR=/private/tmp` (plus `ANDROID_HOME` for the Android builds). Gates are reported as xtask printed them.

| Exact command | Result | Scope |
| --- | --- | --- |
| `cargo xtask ui-preview cmp verify` | PASS | `compileKotlinDesktop` and `desktopTest`: 152 tests in 24 classes, 0 failed, errors or skipped (develop: 115 in 18); eight JVM captures with sidecars written |
| `cargo xtask ui-preview cmp android-build` | PASS | `compileDebugKotlinAndroid` for the isolated Android library, including the three Design v3 wrappers; compiled, not rendered or captured |
| `cargo xtask theatre-demo verify` | PASS | 273 tests in 39 classes, 0 failed. After edits confined to `preview/` the task is UP-TO-DATE (the Theatre build excludes `preview/**`), so its test results were deleted and the tests re-executed through xtask |
| `cargo xtask theatre-demo android-build` | PASS | `:androidApp:assembleDebug`; the first run executed it, later runs were UP-TO-DATE for the same reason. It compiles `ThemeMode` and nothing under `preview/` |
| `python3 scripts/ui_preview_catalog.py --check` (Python 3.14) | PASS | Two registries, named tokens and guarded adoption agree. Before the six records were re-bound it failed on exactly those six records and the two Membership files that use them |
| `cargo xtask verify repository` (Python 3.14 shim on `PATH`) | PASS | 693 text files and 19 skills, local links, 432 Python tests OK (5 optional skipped), generated-output drift and qualification freshness |
| Mutation ledger (hand-run, not an xtask gate) | 28 of 28 killed | Below; M18–M22 are the font fix |

**Post-merge re-run (develop `26dee0a`, #151).** The merge conflicted in four files: the shared
`preview-catalog.json`, its two generated registries (`PreviewCatalog.kt`, `preview_catalog.rs`) and
the work-plan README. The catalog keeps #151's Leptos evidence paths (`issue142-<id>-leptos.json`)
beside this branch's CMP paths (`issue143-<id>-cmp.json`); both registries were regenerated from it
(one catalog digest, `dab470b8…`, in both headers); the README keeps both paragraphs. #151 touched no
hashed CMP source, so the CMP source digest stayed `fba02630…` and the six CMP records stayed fresh.
The preflight's own `cmp-ui-preview` and `theatre-cmp-demo` gates reported `desktopTest` UP-TO-DATE
(the merge left the compiled classes identical), which is a cached pass. So the cached test results
were deleted and both gates re-run through xtask: the counts below were executed after the merge, on
the merged tree.

| Exact command | Result | Scope |
| --- | --- | --- |
| `python3 scripts/ui_preview_catalog.py --check` (Python 3.14) | PASS | Registries, named tokens and guarded adoption agree on the merged tree |
| `cargo xtask ui-preview cmp verify` | PASS | `desktopTest` re-executed: 152 tests in 24 classes, 0 failed, errors or skipped. The eight captures written again have the same SHA-256 prefixes as the opened ones in the table below |
| `cargo xtask ui-preview cmp android-build` | PASS | `compileDebugKotlinAndroid`; compiled, not rendered or captured |
| `cargo xtask theatre-demo verify` | PASS | `:shared:desktopTest` re-executed: 273 tests in 39 classes, 0 failed, errors or skipped |
| `cargo xtask verify repository` (Python 3.14 shim on `PATH`) | PASS | 700 text files and 19 skills, 432 Python tests OK (5 optional skipped), generated-output drift and qualification freshness |
| `cargo xtask preflight --changed-only` (Python 3.14 shim, `CARGO_INCREMENTAL=0`) | PASS | Ten selected gates, none skipped: repository, rust-format, rust-lint, rust-tests, web-lint, web-build (Studio and Theatre), ui-preview, cmp-ui-preview, theatre-cmp-demo. The Rust and Web gates were selected because the shared catalog and its generated Rust registry changed; this PR changes no Rust behavior |

Not re-run after the merge: `cargo xtask theatre-demo android-build` (the merge changed only the
header comment of the generated registry, which that build compiles) and the hand-run mutation ledger
(no gallery or capture code changed in the merge).

**After the font fix** (same tree, Python 3.14 shim for the repository gates, `CARGO_INCREMENTAL=0`). As above, a task that
reported UP-TO-DATE had its cached results deleted and was re-run through xtask.

| Exact command | Result | Scope |
| --- | --- | --- |
| `cargo xtask visual-compare --python /usr/bin/python3 --self-test` | PASS | 12 tests with Pillow 11.3.0. The tool completing is not a design verdict |
| `cargo xtask ui-preview cmp verify` | PASS | `desktopTest` re-executed: 160 tests in 26 classes, 0 failed, errors or skipped (152 before: +2 `ThemeFoundationTest`, +3 `PreviewFontsTest`, +3 `TypeSpecimenCaptureTest`). All 54 CMP capture PNGs taken before the change are byte-identical after it |
| `cargo xtask ui-preview cmp android-build` | PASS | compiled with the new optional `PreviewApp` parameters; not rendered. The wrappers pass no family and keep the fallback |
| `cargo xtask theatre-demo verify` | PASS | `:shared:desktopTest` re-executed: 273 tests in 39 classes, 0 failed. The mock compiles the edited `CantosTheme.kt` |
| `cargo xtask theatre-demo android-build` | PASS | `:androidApp:assembleDebug` after the same edit |
| `cargo xtask visual-compare` ×3 | tool completed | fallback, Noto and the cssbox diagnostic against the one reference, one policy hash (see the section below) |
| `python3 scripts/ui_preview_catalog.py --check` | PASS | after the six CMP records were re-bound to `191bfd2e…`; before that it failed on exactly those six records and the two Membership files |
| `cargo xtask verify repository` | PASS | 432 Python tests OK (5 skipped), local links, drift and qualification freshness |
| `cargo xtask preflight --changed-only` | PASS | the same ten gates as above, none skipped; its CMP and Theatre-mock `desktopTest` were UP-TO-DATE, so the counts above come from the dedicated re-runs |

## Mutation ledger

Each row is one hand-written change to the final package (after the qualification records were
re-bound), applied in a scratch copy and run with `gradle desktopTest --no-daemon`. A mutant is killed
when at least one test fails. All 28 were killed on the final tree; no equivalent mutant was found and
none was discarded. An earlier run, on a tree without the per-field search test, let M2b (search no
longer reads the consumer) survive; the test was added and every search mutant re-run. Likewise M20
(a nested theme forgets the inherited editorial family one level further down) survived a test that
nested only one level; a two-level case was added and M18–M22 were all re-run. M1–M17 were run before
the font fix and were not repeated: it changed no gallery layout, search, theme-mode or capture code.
The mutants are hand-written rather than exhaustive and exercise the gallery, capture and font code only.

| Mutant | Change | Outcome | Failing tests |
| --- | --- | --- | --- |
| M1 | ThemeMode.System ignores the host fact | KILLED | `DesignInventoryViewTest.system_theme_follows_the_injected_host_fact_and_explicit_modes_do_not`<br>`ThemeModeTest.explicit_modes_ignore_the_host_and_system_follows_it` |
| M2a | inventory search drops the owner | KILLED | `DesignInventoryModelTest.each_searchable_field_finds_a_specimen_that_no_other_field_would`<br>`DesignInventoryModelTest.filters_compose_and_the_query_ignores_case_and_surrounding_space_without_mutating_the_inventory` |
| M2b | inventory search drops the consumer | KILLED | `DesignInventoryModelTest.each_searchable_field_finds_a_specimen_that_no_other_field_would` |
| M2c | inventory search drops the family | KILLED | `DesignInventoryModelTest.each_searchable_field_finds_a_specimen_that_no_other_field_would` |
| M2d | inventory search drops the scope | KILLED | `DesignInventoryModelTest.each_searchable_field_finds_a_specimen_that_no_other_field_would` |
| M2e | inventory search drops the id | KILLED | `DesignInventoryViewTest.a_reuse_link_opens_the_registered_component_it_names`<br>`DesignInventoryViewTest.a_web_only_specimen_says_so_and_never_offers_cmp`<br>`DesignInventoryModelTest.each_searchable_field_finds_a_specimen_that_no_other_field_would` |
| M2f | inventory search drops the name | KILLED | `DesignInventoryModelTest.each_searchable_field_finds_a_specimen_that_no_other_field_would`<br>`DesignInventoryModelTest.filters_compose_and_the_query_ignores_case_and_surrounding_space_without_mutating_the_inventory` |
| M3 | IconButton hit target uses the glyph size | KILLED | `HitTargetTest.icon_button_hit_target_follows_the_control_token_at_both_densities_and_not_the_icon_size` |
| M4 | web-only specimens lose their web-only note | KILLED | `DesignInventoryViewTest.a_web_only_specimen_says_so_and_never_offers_cmp` |
| M5 | opening a component keeps the old filters | KILLED | `DesignInventoryViewTest.an_existing_specimen_opens_its_component_and_clears_the_component_filters_that_were_set` |
| M6 | inventory body is a weighted child again (collapses at large text) | KILLED | `DesignInventoryViewTest.a_fixed_height_page_scroll_region_keeps_the_specimen_detail_reachable_when_header_and_filters_fill_it` |
| M7 | width buttons render without a host hook | KILLED | `DesignInventoryViewTest.a_host_that_cannot_resize_its_window_shows_no_width_buttons` |
| M8 | sidecar digests the scenario name instead of the PNG | KILLED | `CaptureRecordTest.sidecar_records_scenario_pixel_size_and_the_digest_of_the_written_png`<br>`CaptureRecordTest.the_png_digest_matches_the_published_sha256_vector_for_abc` |
| M9 | sidecar cannot find the canonical token file | KILLED | `CaptureRecordTest.sidecar_records_scenario_pixel_size_and_the_digest_of_the_written_png` |
| M10 | probe prints the host scale as the effective scale | KILLED | `DesignInventoryViewTest.the_gallery_reports_the_constraint_host_scale_preview_multiplier_and_effective_scale` |
| M11 | page stops scrolling when text is enlarged in an expanded window | KILLED | `DesignInventoryViewTest.enlarged_host_text_in_an_expanded_window_makes_the_whole_page_scroll`<br>`PreviewGeometryTest.the_page_scrolls_when_narrow_or_when_effective_text_is_enlarged_and_stays_fixed_only_when_expanded_at_normal_text` |
| M12 | CMP-only filter is inverted | KILLED | `DesignInventoryModelTest.filters_compose_and_the_query_ignores_case_and_surrounding_space_without_mutating_the_inventory` |
| M13 | every theme chip selects Light | KILLED | `DesignInventoryViewTest.system_theme_follows_the_injected_host_fact_and_explicit_modes_do_not` |
| M14 | selection no longer falls back to the first displayed specimen | KILLED | `DesignInventoryViewTest.narrow_inventory_uses_one_specimen_picker_and_keeps_filter_and_detail_in_agreement`<br>`DesignInventoryViewTest.one_dp_below_the_expanded_breakpoint_the_layout_is_the_narrow_picker`<br>`DesignInventoryViewTest.a_web_only_specimen_says_so_and_never_offers_cmp`<br>`DesignInventoryModelTest.filtering_projects_one_displayed_selection_and_preserves_the_preference` |
| M15 | opening a component stays on the inventory tab | KILLED | `DesignInventoryViewTest.a_reuse_link_opens_the_registered_component_it_names`<br>`DesignInventoryViewTest.an_existing_specimen_opens_its_component_and_clears_the_component_filters_that_were_set` |
| M16 | existing specimen always opens the button component | KILLED | `DesignInventoryViewTest.an_existing_specimen_opens_its_component_and_clears_the_component_filters_that_were_set` |
| M17 | page rule reads the preview multiplier, not the effective font scale | KILLED | `DesignInventoryViewTest.enlarged_host_text_in_an_expanded_window_makes_the_whole_page_scroll` |
| M18 | gallery root drops the injected families | KILLED | `PreviewFontsTest.the_gallery_renders_every_text_in_the_injected_families_including_nested_specimen_themes` |
| M19 | nested theme ignores inherited UI and editorial families | KILLED | `ThemeFoundationTest.a_nested_theme_inherits_host_injected_families_and_an_explicit_family_replaces_only_its_own_role` |
| M20 | nested theme forgets only the inherited editorial family (visible at the third level) | KILLED | `ThemeFoundationTest.a_nested_theme_inherits_host_injected_families_and_an_explicit_family_replaces_only_its_own_role` |
| M20b | nested theme forgets only the inherited UI family | KILLED | `ThemeFoundationTest.a_nested_theme_inherits_host_injected_families_and_an_explicit_family_replaces_only_its_own_role` |
| M21 | a bundled face is registered at the wrong weight | KILLED | `PreviewFontsTest.each_bundled_face_has_the_bytes_family_and_weight_the_provenance_record_names` |
| M22 | a missing face yields empty bytes instead of failing | KILLED | `PreviewFontsTest.a_missing_face_fails_loudly_instead_of_falling_back_silently` |

## Opened captures and same-scenario recheck

`PreviewCaptureTest` wrote eight PNGs with eight JSON sidecars; the older suites wrote 46 more PNGs
(no sidecar), all into the ignored `packages/ui-system/cmp/build/reports/cantos-preview/`. The eight
PNGs below, the four new and the four whose gallery header changed, were opened from the final run
after the last source edit. They are JVM Desktop Compose test-root renders at 1 px per dp and host font
scale 1, not an OS window, emulator or phone. Each sidecar states its renderer as
`jvm-desktop-compose-test-root` and lists what it is not evidence of.

| Capture (PNG SHA-256 prefix) | Constraint and inputs | Observed after opening |
| --- | --- | --- |
| `cmp-jvm-design-v3-light-390-text1-mini-player` (`2a01baf3`) | 390×1800 dp, light, preview 1×, mini-player proposal | The probe line reads "Viewport 390×1800 dp · 1.0 px/dp · font scale host 1.0 × preview 1.0 = 1.0 · fixtures vi-VN"; Light, Compact and Text 1.0× are selected; "Theme mode Light → light; host reports light. Only System follows the host."; Design v3 is the active tab; "102 of 102 specimens" with one specimen picker. Mini-player shows `design: proposed`, `scope: app-owned`, `owner: cantos-theatre`, `platforms: web · cmp`, the "No native sample: a proposal is not a public API" note, four reuse links (surface, stack, icon-button, range-field) and the verbatim Vietnamese description with intact diacritics, then the states and the start of the anatomy. The fixed page height ends the capture there; no width-preset row is shown |
| `cmp-jvm-design-v3-dark-320-text2-studio-workspace` (`1639b398`) | 320×5200 dp, dark, preview 2× | One column. Probe "font scale host 1.0 × preview 2.0 = 2.0". Chips and counts wrap without clipped labels. The Web-only Workspace navigation detail shows `platforms: web` in the warning tone, the "Web only: Studio and Admin compositions never target CMP…" note, reuse links nav-item/container/icon, and the verbatim description, states, anatomy and canonical token roles down to the page end |
| `cmp-jvm-design-v3-light-840-text1-button` (`1d563240`) | 840×1000 dp, light, preview 1×, button specimen | Exactly the expanded breakpoint, so the master/detail layout: Theme & density, Buttons (selected), Icon buttons, Filter chips and Links in the list pane, a sixth row cut at the pane's own scroll edge. The detail shows `design: existing`, `scope: shared-primitive`, `owner: ui-system`, "Registration is not qualification…" and the "Open Buttons in Components" action, cut at the detail pane's bottom edge because each pane scrolls on its own |
| `cmp-jvm-components-system-dark-390-text1` (`de26cecf`) | 390×1800 dp, `ThemeMode.System` with an injected dark host fact | "Theme mode System → dark; host reports dark. Only System follows the host."; the System chip is selected; dark roles are applied; the Button component lists its eleven states and its sample row; no width-preset row |
| `cmp-jvm-light-wide-components` (`f916bf87`), `cmp-jvm-dark-wide-components` (`951e4d3a`) | 1140×840 dp, light and dark | Legacy captures. The header gained the probe line, the three-way theme chips and the third tab; the Components content is unchanged |
| `cmp-jvm-dark-wide-tokens` (`bcdef12d`) | 1140×840 dp, dark | Legacy capture. Same header change; "115 roles · dark · compact · one generated source" and the named-token rows are unchanged |
| `cmp-jvm-light-narrow-text2-components` (`5389600a`) | 372×840 dp, light, preview 2× | Legacy capture. It shows only header controls, as it did on develop (where the old inert Narrow/Medium/Wide buttons were visible), because 2× text pushes all content below 840 dp. That limitation is inherited, not new |

**Same-scenario recheck.** The first Design v3 captures showed `320 dp … 1140 dp` as text-style
buttons in roots that cannot resize a window, so they did nothing. The buttons are now rendered only
when a host supplies the resize hook (two tests), and the same scenarios were captured again: the row
is gone from every capture above. The first narrow layout could leave the specimen detail unreachable
under large text; the layout became one scroll column, a test that fails on the old layout shape
(mutant M6) now covers it, and the 320×5200 capture shows the detail through its token roles.

**Identity against a clean develop run.** The same suites ran on develop at `68b2fc6` in an untouched
copy. 46 of its 50 capture PNGs are byte-identical to this branch's, by SHA-256: auth 19, foundation 7,
interaction 14, logo 5, regression 1. They include the dedicated captures of the six qualified
components. The four that differ are the gallery captures above, whose header changed.

## Type specimen against the design fonts

A request to make the CMP UI match design v3 more closely was run as one `cantos-ui-fix` repair: pair
a design-side render with the CMP render, compare with `cargo xtask visual-compare`, open the images,
repair the cause the comparison shows, and recheck the same pair.

```text
Observed (before): every CMP preview text ran in the platform sans, and the headline was sans too,
                   where design v3 shows Noto Serif; nested specimen themes dropped a host-injected family
                   (red test: expected Monospace/Cursive/Cursive, was SansSerif/SansSerif/Serif)
Expected:          the design's families. Design v3's HTML embeds four Noto TTFs whose SHA-256 equal the
                   shared bundle's (Sans 400 f5f552c8…, Sans 700 3a08a47d…, Serif 400 19e72cd8…,
                   Serif 700 96656aa5…); design HTML SHA-256 6b5c4235…
Root cause:        confirmed in source and by the red test: CantosTheme already takes injected families
                   but no preview root injected any, and a nested CantosTheme reset an unnamed family
Invariant:         a family injected at the composition root reaches every text run below it, nested
                   themes included; a host that injects none keeps the reviewed fallback
Owner / boundary:  CantosTheme (inheritance, shared), PreviewApp + desktop Main + PreviewFonts
                   (host delivery, preview only). Components, UiRoot and the registry are untouched
Fix:               private inherited locals in CantosTheme; PreviewApp takes two optional families;
                   PreviewFonts copies the committed bundle onto the desktop classpath via Gradle
                   (no duplicate bytes in Git, no installed-font lookup)
```

**Reference.** `docs/evidence/issue143/type-specimen-light-390x520.html` states the canonical v0.6.0
type roles (size, line-height, weight, family) and light colors in CSS and loads the shared Noto
bundle; Google Chrome 155.0.8059.41 headless at DPR 1 rendered it to a 390×520 px PNG (SHA-256
`461e0104…`). It is **not** an approved baseline: the attribution is `unreviewed`, I authored the page
from the tokens, and the design export's PNGs are not a pixel oracle (see the issue). The pair therefore
supports a raw comparison and a cause analysis, not a design-fidelity approval. The CMP side is
`TypeSpecimenCaptureTest`: the real theme roles, the same strings, 390×520 dp at 1 px per dp, light,
vi-VN, text 1×. The actual-side `font` field is left unknown in all three metadata files, because the
fallback family is not a hashed font and a known/different value would be a pairing mismatch (no
score); that keeps one frozen policy, `recheck_policy_sha256` `ccfac968706f…`, across every run below.
Threshold 0, tile 16, no masks, Pillow 11.3.0 (`/usr/bin/python3`; the tool's self-test passed 12 tests).

| Run (actual PNG) | Changed / 202 800 px | Ratio | Regions (x, y, w, h) | What is identical |
| --- | --- | --- | --- | --- |
| `…-fallback` (platform sans/serif; `440809a7…`) | 20 461 | 10.09 % | 1: 24, 24, 342, 243 | nothing; the headline already differs |
| `…-noto` (shared bundle; `6d380618…`) | 11 579 | 5.71 % | 1: 24, 80, 341, 187 | the whole headline row |
| `…-noto-cssbox` (diagnostic; `5cff26d5…`) | 6 754 | 3.33 % | 2: 24, 81, 341, 115 and 24, 253, 135, 15 | headline and label rows |

Top of each ink row against the reference (px; reference rows start at 27, 82, 126, 152, 176, 218, 253):
fallback −3, −9, −13, −14, −13, −20, −23; Noto 0, −1, −3, −4, −4, −7, −8; Noto with the CSS-style line
box 0, +1, +1, +1, +1, 0, +1. The percentages are changed pixels over the whole image, not a quality score.

**Opened images** (reference, the three actuals, the fallback and Noto overlays, the cssbox diff):
- Fallback overlay: every block is a double image. The headline is a sans, the reference a serif, line
  pitch and glyph widths differ throughout.
- Noto overlay: the headline coincides with the reference with no ghost; title, body, label and metadata
  show the reference and the render one to eight pixels apart vertically, growing down the page. The
  body wraps at the same words in both. Glyph shapes match: the headline row is pixel-identical, and
  with the CSS-style line box so is the label row.
- Cssbox diff: the headline and the label row are untouched; the rest shows one-pixel edges only.

**Cause of the remaining drift (identified by the diagnostic, not fixed here).** Compose's default
line-height style trims the leading above a block's first line and below its last line, while CSS keeps
half-leading on every line, so a block comes out a few pixels shorter (the body block is 3 px shorter, which
is the observed offset). Applying `LineHeightStyle(Center, Trim.None)` inside the fixture alone moved the
rows from 0 to −8 px to ±1 px, which points at the default style rather than the fonts (the headline row is pixel-identical). Making that
change in `typeStyle` alters the height of every text block in the Theatre mock, so it is a separate
reviewed change, listed under deferred acceptance.

**Seen against the design and deliberately left alone.** Button colors match exactly (sampled page
`#f7f3eb`, primary `#675080`, tonal `#e6d9f1`), but design v3 draws 56 px pill buttons with Material icons
and a label-above text field, where CMP keeps its 48 dp / 8 dp buttons and floating-label field. The
reconciliation records those shape and size roles as deferred ("Web geometry cannot override native
geometry"), and the issue asks not to turn every control into a pill, so they are decisions and not
defects of this repair. The design's own specimen cards also overflow a 390 px viewport on the right.

**Reproduce.** Reference: Chrome headless with `--force-device-scale-factor=1 --window-size=390,520
--virtual-time-budget=4000` and `--screenshot` on the HTML file. Actual: `gradle … desktopTest --tests
'com.cantos.ui.TypeSpecimenCaptureTest'` writes the three PNGs and sidecars to the ignored capture
directory. Compare: `cargo xtask visual-compare --python /usr/bin/python3 --reference … --actual …
--scenario cmp-type-specimen-light-390x520 --metadata … --output
artifacts/ui-evidence/issue143/comparison-<run>`. Images and reports stay in ignored `artifacts/`.

## Qualification records

Any change to a hashed file in `packages/ui-system/cmp` (Kotlin including tests and the Android host,
Gradle, resources; the generated Kotlin registry and Markdown are not hashed) changes the CMP source
digest. The six records written by #141 (`button`, `text-field`, `surface`, `badge`, `back-button`,
`logo`) therefore went stale, and `python3 scripts/ui_preview_catalog.py --check` reported exactly
those six records plus the two Theatre Membership files that use them ("new use requires both preview
qualifications").

| Digest | #141 records | This PR |
| --- | --- | --- |
| `tokens_sha256` | `25e99c49faa15a9112cd03e7e92f21360b808bbe68f0a2531017d9a0e21f0741` | unchanged |
| CMP `source_sha256` | `4dd8dab014506c8fa2e0ad4c619d8b1759af472b56e3e88a6d80921eee7cbe17` | `191bfd2ed04d74747a25da635b9c4d7cb03da204029c62232253bcfd64fc60d0` (first slice `fba02630…`; moved again by the font fix, see below) |
| Leptos `source_sha256` | `119f3dcf8490e7309c4127adb4103116ef2bb6170851bc8107057c293ea2618c` | `c8bf6aae2d787c593b86da2a2113cc1504ce7a6a15bf3c2c783dc8bf72bd47c8` after merging #151, which re-bound its six Leptos records (`issue142-<id>-leptos.json`); this PR changes no Leptos-hashed file, and the CMP source digest did not move with the merge |

I re-ran the command the old records name (`ui-preview cmp verify`, 152 tests), confirmed that no
implementation file of the six components changed, and compared the captures (above). The font fix
([below](#type-specimen-against-the-design-fonts)) then edited the shared `CantosTheme.kt`, the desktop
host, the Gradle file and tests, so the same six records were re-bound a second time: `ui-preview cmp
verify` re-executed 160 tests, a theme with no injected family is unchanged by the edit, and all 54 CMP
capture PNGs taken before it are byte-identical after it (SHA-256 list `artifacts/agent-runs/
issue143-png-baseline-pre-font-fix.sha256`, ignored), including every component and logo capture. Six new records,
`docs/evidence/issue143-<id>-cmp.json`, then bind those checks to the new digest, and the catalog's
`qualifications[].evidence.cmp` paths point at them; the #141 records stay in the tree as history. They keep
the shape and the PASS for structural, runtime, accessibility and visual that #141 reviewed. Their
`reviewer` says an agent re-bound the digest, and their `reference` says that no design reference was
re-opened and that the visual basis remains the #141 review. This is a bounded JVM claim: it adds no
Android, iOS, screen-reader or product acceptance and no component or state. A maintainer who wants a
human re-review of the six can ask for one; nothing else depends on the reviewer string.

`python3 scripts/ui_preview_catalog.py --digests` prints the current digests without approving anything.
`--check` and `cargo xtask verify repository` fail on any difference, and a hash is never edited to clear
them.

## Baseline state coverage and design-only states

The design export lists states the registry baseline does not (button `hover`, icon-button
`pressed`, checkbox `mixed`/`unselected`, field-label `required`/`optional`, loading-state
`determinate`/`indeterminate`/`reduced-motion`, icon `named`/`20px`/`24px`, number-field
`minimum`/`maximum`/`stepped`, logo `symbol`/`mono`, switch `on`/`off`, link `unavailable`).
Several are aliases of existing states, several are Web-only (`hover`), and a few are real gaps
(`mixed`, `determinate`, `required`/`optional`, `named`). None was added: the 32 APIs and their
198 baseline pairs are unchanged. A bounded extension needs the lookup → reuse/extend sequence, a
state in both previews and fresh two-renderer admission. Among interactive components only
`button` and `back-button` declare `pressed`; that is a candidate for the next extension, not a claim.

## Deferred acceptance and residual risk

Nothing here is production acceptance. This PR is preview and documentation only: no product screen,
controller, port or release path changed, and a proposal in the inventory cannot be imported.

| Check / layer and status | Reason | Prerequisites and exact next command | Acceptance criteria | Next owner / residual risk |
| --- | --- | --- | --- | --- |
| Android Studio capture of the three Design v3 wrappers — `UNSUPPORTED` here | Environment: no running Android Studio Quail 2+ with the isolated project open. The library compiles (`PASS`), nothing was rendered on Android | Open `packages/ui-system/cmp/android-preview`, sync, then `cargo xtask compose-preview ui-system-cmp` | Three PNGs opened and compared with the JVM captures above, renderer named | Whoever has the IDE; until then the wrappers are compile-checked only |
| Android/iOS device or simulator, OS IME, Back and insets, TalkBack/VoiceOver — `NOT_RUN` | Outside this PR; no native runtime gate exists | Target-specific scenarios in `cmp-testing.md` | A named device or simulator run with a reader walkthrough | Native acceptance owner; JVM captures cannot substitute |
| CMP font delivery beyond the desktop preview host — not implemented | The desktop gallery host injects the shared Noto bundle; the Android Studio wrappers and the test roots keep the platform fallback (the Theatre mock injects its own copies of the same bytes) | Item 2 of the [016 follow-up order](../work-plan/016-cmp-ui-system.md#design-v3-cmp-slice-issue-143) | Wrapper captures with Noto loaded through an Android resource path, opened and compared with the JVM ones | CMP owner |
| Line-box model against the CSS reference — not changed | Compose trims the leading above the first and below the last line of a block by default; the diagnostic below puts the vertical drift at 0–8 px with the default and ±1 px with a CSS-style line box. Applying it to `CantosTheme` changes every text block's height in the Theatre mock and the product | A reviewed shared-theme change with captures of the Theatre screens and the touch-target tests | The type specimen pair at ±1 px with the shipped theme and no layout regression in the Theatre mock | Theatre CMP owner |
| OS reduced-motion adapter, gesture intents — not implemented | Rules are written in `cmp-design-system.md`; the gallery sets reduced motion with a manual chip | A concrete consumer | A host that resolves the OS setting and injects it, with a test | Theatre screen owner |
| Theatre first-batch compositions — not started | Needs a concrete consumer and the lookup → declare → preview → admission loop | Item 1 of the 016 follow-up order | Preview first, then two-renderer admission, then product adoption | Theatre CMP owner |
| Human re-review of the six re-bound qualification records — optional | The records were re-bound by an agent on unchanged component sources | Re-open the #141 references and the captures above | A reviewer name on a fresh record | Maintainers |
