# Theatre Web Discover / Home design parity — issue 282

Related issue: [#282](https://github.com/loveoverflowcom/cantos/issues/282). Baseline:
`develop@54dbee19a887ba29dccce75ee6c4fc4eba263f15`. This record names what ran, with which doubles,
and what did not. It accepts no deployed, seeded-API/PostgreSQL or reference-pixel result.

## Review boundary

Delivers: live Discover renders the Home anatomy of the design — one wordmark in the rail, a
context topbar on the same axis as the content, an intro, a Work hero (copy + cover region +
open action) and a grid of Work cards with a cover region — on the real catalog, search,
pagination and single-player code that already exist. Search/filter is folded into that hierarchy.

Does not deliver: backend, auth provider or CMP/Studio changes; Save/resume/history (#272); a
real Work cover or editorial selection (no public contract); a direct **Nghe** from the list;
a fresh render of the design reference; seeded-API acceptance.

## Reference region → component → facts → states → delta

| Reference region | Component | Permitted facts | Live states | Delta (deliberate) |
| --- | --- | --- | --- | --- |
| Rail wordmark | `LiveBoundary` rail (`Logo`) | none | all | Two nav items gain Material outlined icons from the app icon registry (`home`, `library_books`). The design's third Account destination is **not** adopted (#35/#129): Account stays on the avatar |
| Topbar | `LiveBoundary` topbar | route name | all | The second wordmark is removed; the topbar names the current place. It now sits in the same `Container` as `<main>`, so both share one inline axis |
| Intro heading | `PublicHome` header | static copy | all | Sign in / Create account are quiet text actions; they never replace the Work action |
| Hero | `FeaturedWork` (`.theatre-feature` anatomy) | first row of the answered walk: title, description, kind, language, `unit_count`, ID | ready only | Eyebrow is neutral: "First in the catalog", or "First match" when narrowed. It never says featured, trending or rated: the contract has no editorial selection |
| Hero cover | `WorkCover` | kind | ready | Neutral fallback (decoration + kind). No artwork, no other Work's image: the public contract has no cover |
| Hero action | link to `#work/{id}` | canonical Work ID | ready | **Mở tác phẩm** only. Listening starts from the Work's own units; the summary carries no first publication, so a list-level **Nghe** would need a contract change |
| Shelf heading | `PublicHome` | static copy | all | — |
| Search + filters | `SearchForm` | `q`, `kind`, `language` | all | Compact search with an icon button; kind chips; the content language is chosen **by name** from a `Select` (the tag stays the request). Raw tag field and API help text are removed |
| Work cards | `WorkCard` | kind, title, description, `unit_count` | ready | Cover region plus body; the title link is still the card's one named target. The hero Work is not repeated in the grid |
| Loading / empty / no match / incomplete / expired / error | `PublicHome` regions | typed `PublicDiscoveryPhase` | each | Same shell and search; error has exactly one retry; loading shows three neutral placeholders |

The language names are the app's supported choices, not a statement of which languages the
catalog holds. A tag carried by a link that is not named stays selectable under its own tag.

## Ledger

| Claim | Failure mode | Oracle | Level |
| --- | --- | --- | --- |
| Hero and cards partition the rows; each Work has one link and one focus ID | duplicate IDs; broken Back focus | `the_first_work_leads_as_a_neutral_hero_and_the_rest_are_cards_with_one_link_each` | example-tested (DOM string render) |
| No invented editorial claim, cover or audio | fake "featured"/art | same test | example-tested |
| Language is chosen by name; tag unchanged; link tag preserved | listener types `vi-VN`; link filter dropped | `discovery_language` tests; `language_is_chosen_by_name_and_no_api_instructions_fill_the_form` | example-tested |
| Outage keeps search and one retry, no hero or fake data | white form / success on error | `an_outage_keeps_search_and_one_retry_in_the_same_shell_without_a_hero` | example-tested |
| One wordmark, context topbar, icons on both nav items | competing brand | `live_shell_has_one_wordmark_a_context_topbar_and_icons_on_both_destinations` | example-tested |
| Existing `q/kind/language/cursor` ownership, stale fences, retries | regression | the unchanged `application::public_discovery` and `public_home` suites | example-tested |
| Layout, dark, 390/320 px, error state | overflow, collapse | local browser pass below | screenshot-inspected (pane), not committed |
| Language change, open Work, Back restores query and focus | lost state | local browser pass below | interaction-tested (scripted, one browser) |

## Local browser pass (mocked API)

Built with `cargo xtask build cantos-theatre --web` (debug) for this change and for the
`develop` baseline, each served by a loopback script that returns a **synthetic** five-Work
catalog (or `503`) and `404` for every other API path; the script is not committed. Same
scenario for both: 1440×900, light, `vi-VN`, ready. After: also 390 dark (API down: one retry,
search kept), 320 light (a topbar locale field collapsed to a letter per line at 320; fixed
with a minimum width and re-observed), language `en` → URL `#discover?language=en`, hero "First
match", then open Work, Back → same query and the Work's link focused. Chromium engine of the
desktop browser pane; the version was not recorded.

Baseline reproduced the issue's findings on the same catalog: second wordmark link, a raw
language-code field with API help text, and no hero or cover region.

## Not covered / residual risk

- The design reference (Drive HTML/PNG) was **not opened**; parity is to the issue's written
  anatomy and the existing mock Home markup, not a region-by-region pixel pairing. No
  reference-versus-implementation comparison exists.
- No seeded API/PostgreSQL run, no real cover, no `1280/768` captures, no true browser text
  zoom (a root font-size override was only a probe), no keyboard/screen-reader walk, no
  light/dark and VI/EN matrix beyond the cases above, and no deployed acceptance.
- Screenshots were observed in the pane and not stored.
- Direct **Nghe** from Discover needs a public contract fact (first playable unit); recorded as
  a gap, not worked around.
