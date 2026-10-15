# Leptos localization spike and pre-ADR slice — issue 168

This records the exploratory slice of
[issue #168](https://github.com/loveoverflowcom/cantos/issues/168), based on `develop` `34a3151`
(tree `b1699433134941521b9ce2bf54e14ece2c0e5ed5`). It measures three candidate approaches on the
pinned Leptos **0.7.8** and fixes the narrow interpolation regression. **No library is selected**:
the catalog/runtime decision and its ADR belong to
[issue #167](https://github.com/loveoverflowcom/cantos/issues/167), and broad namespace migration
waits for that accepted ADR.

## Review boundary

| Delivered here | Not delivered (waits for the #167 ADR or a later slice) |
| --- | --- |
| Candidate findings below, from a scratch spike outside the repository | an accepted ADR, a selected library, any new dependency |
| Theatre single-pass interpolation, CLDR plural category and Web/CMP speed/spoken-time parity | migrating Theatre `auth_copy`/membership catalogs or Studio `messages/*` to one API |
| Studio root owns a typed `Locale`; unmigrated namespaces read a read-only `english` adapter | removing the `english: bool` adapter from Studio copy functions |
| `<html lang>` follows the interface locale on every Studio and Theatre root | persisted locale preference and browser negotiation (open precedence question in #167) |
| Strict supported-tag parsing; language pickers show autonyms with their own `lang` | typed error → key mapping beyond what already exists; dates, numbers and money formatters |

## Invariants

| Claim | Failure mode | Oracle and evidence |
| --- | --- | --- |
| A template is scanned once; inserted values are never re-scanned | title `Lời hẹn {time}` loses its braces when `{time}` is replaced later | `messages::tests` swapped order, nested placeholder, repeated name, missing name, unclosed brace: example-tested |
| Inserted values stay text | a title containing markup becomes HTML | the rendered `aria-label` and text node are escaped by Leptos: example-tested via native string render; no `inner_html` carries localized text (the only use renders compile-time SVG icons) |
| Plural choice uses one CLDR rule per locale | scattered `count == 1` picks English rules for Vietnamese | `Locale::plural` for 0/1/2/21/101 and episode counts: example-tested |
| Web speed and spoken-time output match CMP | Web `1,50×` / `1 minute, 1 second` vs CMP `1,5×` / `1 minute 1 second` | the exact vectors of CMP `DemoCopyTest` asserted in Rust: example-tested (Kotlin side not run in this slice) |
| A locale switch changes document language without remounting active state | `<html lang="vi-VN">` stays under English UI; player or focus lost | Theatre mock/live/membership roots driven by keyboard in a browser: interaction-tested (below) |
| Unknown tags never become a silent default | any value other than `en` selected Vietnamese | `Locale::from_lang` rejects `""`, `vi`, `EN`, `en-US`, `fr`: example-tested in both apps |

## Candidate findings

Fixture, identical in each candidate: `common.cancel`; `theatre.player.bookmark.add(title, time)`
rendered with title `Lời hẹn {time}`; an episode count for 0/1/2 in `vi-VN` and `en`; one typed
error message; a reactive locale toggle. Each candidate is a separate CSR binary on Leptos 0.7.8,
`wasm-bindgen` 0.2.100 (the repository pin).

Provenance: Rust 1.98.1, `wasm32-unknown-unknown`, default release profile, 22-thread Linux host,
one clean build per candidate (no shared target directory) on 2026-10-16. Sizes come from the
Trunk-cached `wasm-bindgen` 0.2.100 and `wasm-opt` version 123 (`-Oz`), compressed with
`gzip -9`. One sample each: treat compile times as indicative only (the baseline repeated at 46.4 s
and 46.6 s). The spike sources stayed in a scratch directory and are not committed.

| Candidate | Resolves to one Leptos 0.7.8 | Clean release build | `Cargo.lock` packages | `.wasm` after bindgen (gzip) | after `wasm-opt -Oz` (gzip) | Δ vs baseline (gzip, optimized) |
| --- | --- | --- | --- | --- | --- | --- |
| 3a. typed Rust catalog + single-pass interpolator (baseline) | yes | 46.6 s | 180 | 191,819 (53,184) | 102,486 (40,212) | — |
| 1. `leptos_i18n` 0.5.11, JSON | yes | 60.5 s | 266 | 626,605 (124,868) | 281,178 (101,485) | +61,273 |
| 2. `leptos-fluent` 0.3.1 + `fluent-templates` 0.15.1, FTL | only after lockfile surgery | 57.1 s | 207 | 625,934 (168,520) | 364,194 (148,347) | +108,135 |
| 3b. typed Rust keys + `fluent-bundle` 0.16.0 (concurrent) | yes | 51.5 s | 189 | 448,983 (133,488) | 275,445 (115,464) | +75,252 |

Startup time, cache behavior and interaction cost were not measured; these are offline bundle
bytes only ([measurement boundary](web-performance-patterns.md)).

### 1. `leptos_i18n` 0.5.x

- **Compatibility.** 0.5.11 requires `leptos ^0.7.7` and resolves cleanly. The maintained line is
  0.6.x (0.6.2, 2026-04-14), which requires Leptos 0.8; 0.5.11 (2025-03-22) is the last 0.7
  release, so adopting it ties the next i18n upgrade to a Leptos 0.8 migration.
- **Keys and parameters.** Keys and interpolation variables are generated at compile time; an
  unknown key is a compile error (observed). A key missing from a non-default locale is only a
  `deprecated` warning, so a fail-closed gate needs warnings denied.
- **Plurals.** CLDR via ICU4X 1.5 compiled data, `_one`/`_other` suffixes. With `vi-VN` as the
  default locale a lone `_other` form is not treated as a plural (the parser needs two forms;
  observed). The workaround is a redundant Vietnamese `_one` that the macro reports as useless, or
  making `en` the default locale, which conflicts with the proposed `vi-VN` fallback.
- **Interpolation.** `{{ name }}` values are inserted, not re-parsed:
  `td_string!(Locale::vi_VN, …, title = "Lời hẹn {time}")` kept the braces (example-tested natively).
- **Runtime.** Needs `leptos_meta`; locale variants become identifiers (`Locale::vi_VN`). The
  default features add cookie handling, disabled in the spike.

### 2. `leptos-fluent` 0.3.x

- **Compatibility.** Its open ranges (`leptos >=0.7, <0.9`, `leptos_meta >=0.7, <0.9`,
  `fluent-templates >=0.13.1`) resolved Leptos **0.8.14** and `leptos_meta` 0.8.5 next to the
  app's 0.7.8 (observed in `Cargo.lock`), and `fluent-templates` 0.15.1 next to a direct 0.13.3
  pin (a type-mismatch compile error, observed). A single runtime needed
  `cargo update -p leptos_meta@0.8.5 --precise 0.7.8` and matching `fluent-templates`; any later
  unpinned update can reintroduce two Leptos runtimes.
- **Keys and parameters.** String keys in `move_tr!("id", { "name" => value })`; correctness
  depends on the optional `check_translations` pass (not exercised), not on types.
- **Built-in browser policy.** `sync_html_tag_lang`/`dir`, local storage, cookies, URL parameter
  and `navigator.languages` negotiation are provider options. They would need to stay disabled
  until #167 approves a precedence policy, and they place browser IO inside the provider.
- **Size.** Largest delta of the three.

### 3. Existing typed Rust catalog, optionally with Fluent or ICU4X adapters

- **3a (no dependency).** Keys are enums and both locales are exhaustive by construction
  (`resources!` match); placeholder parity and plural coverage are unit tests. The cost is
  hand-written CLDR rules and formatters per locale and no translator-facing file format. This
  slice implements it in Theatre without choosing it as the long-term strategy.
- **3b (`fluent-bundle`).** The default `FluentBundle` is not `Send`/`Sync`, so Leptos 0.7
  reactive closures reject it (compile error observed); `concurrent::FluentBundle` works.
  `use_isolating` is on by default and wraps every placeable in U+2068/U+2069
  (example-tested), which changes accessible names, `aria-label` strings and equality tests unless
  disabled. Message lookup is by string, so a typed key enum still needs a coverage test.
- **ICU4X formatting adapters** (numbers, dates, durations) were not measured.

### Observations for the #167 decision (no selection)

- Each library adds 60–110 KB gzip to a four-message fixture before any real catalog; most of it is
  runtime and CLDR data, not messages.
- Only 3a keeps a Leptos 0.8 migration independent of the localization choice.
- Every candidate inserts values once; the hazard was only in the hand-written fold fixed here.
- Vietnamese-first catalogs expose library assumptions (default-locale plural shape in 1, browser
  negotiation defaults in 2, bidi isolation marks in 3b) that need explicit tests.

## Browser observation (Theatre)

`cargo xtask app cantos-theatre --web` (debug Trunk build, Theatre API not running, so live data
requests returned 500 and the offline modes were used), in-app Chromium browser pane, 800×600.
Locale changes used keyboard `ArrowDown`/`ArrowUp` on the focused native `<select>`.

| Root | Before | After the switch | Retained |
| --- | --- | --- | --- |
| `?mode=mock#settings` with the sample episode simulating at 0:30 | `<html lang="vi-VN">`, `Cài đặt nghe · Cantos` | `en`, `Listening settings · Cantos`, speed options `0.75× 1× 1.25× 1.5× 2×` | same mini-player and shell DOM nodes, still simulating at 0:30, focus on the select, story title spans keep `lang="vi-VN"`; switching back restored `vi-VN` and `1,5×` |
| `?mode=membership-preview` | `vi-VN` | `en` on `<html>` and the preview root | options carry `lang="vi-VN"`/`lang="en"` |
| live shell `#settings` | `vi-VN` | `en`, `Listening settings · Cantos` | — |

The player's accessible position read `0 phút 30 giây trên tổng số 24 phút 40 giây`. Evidence
level: interaction-tested and DOM-tested for Theatre; one screenshot was captured and inspected at
800×600 in light theme only. Not covered: Studio in a browser (its `xtask app` requires a running
API and PostgreSQL is unavailable on this host), screen readers, IME during a switch, long-text
and narrow-viewport layout.

## Residual risk and follow-ups

- Studio's locale switch is compiled and component-tested natively, not browser-observed; draft,
  caret and IME retention across a switch relies on the switch not touching editor state.
- Kotlin `DemoCopyTest` was not run; the shared vectors are duplicated in Rust and Kotlin rather
  than generated, so a future change must update both.
- Locale persistence, first-render negotiation and the `english: bool` adapter removal remain
  open until #167 records a decision.
