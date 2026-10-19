# Theatre Web search, pagination, routes and the production/preview split — issue 270

Related issue: [#270](https://github.com/loveoverflowcom/cantos/issues/270) (slice 2 of 4 of
[#13](https://github.com/loveoverflowcom/cantos/issues/13)). Baseline:
`develop@26fd84c52d4b57f68beeaa7c9c78b2b68a3d1e0c` (it contains the #269 player). The tested tree is
that baseline plus this change; the pull request records the head SHA, which a commit cannot
contain. This record names what ran, with which doubles, and what did not. It accepts no deployed,
CDN, native or human-listening result.

## Review boundary

Delivers: Khám phá (`#discover`, `#explore`) searches the public catalog through the existing
`GET /api/v1/theatre/works` (`q`, `kind`, `language`, `limit`, `cursor`) and pages through it with
the opaque cursor; the Work list and detail and the Unit list consume the `presentation=1` DTOs
(`unit_count`, `ordinal`); unknown and unwired routes have a recovery view instead of Home; a
build without the new `preview` Cargo feature contains neither the `?mode=mock`,
`?mode=auth-preview` and `?mode=membership-preview` entrypoints nor their sample data.

Does not deliver: Library/auth (#271), saved/progress/history/resume (#272), a backend search or
genre capability, public artwork/genre (#157), a change to the HTTP codec, any Hosting release, CMP
or native work, Safari/iOS.

## Source facts that drove the design

| Fact (read from `develop@26fd84c5`) | Consequence |
| --- | --- |
| `live/discovery.rs` sent `GET /works?limit=20` and the decoder refused `items.is_empty() && next_cursor.is_some()`, which the contract allows | One walk owner (`application/public_discovery.rs`) with query, cursor and a bounded follow-up of empty pages; the decoder accepts the empty intermediate page |
| `PublicDiscoveryState` kept one page with a generation and had no cursor or query | It is extended, not paralleled: a ticket carries `generation + query + cursor`; changing the query starts a fresh walk and fences every older answer |
| The live shell's `_ =>` arm rendered Home for Explore, Queue, NotFound and mock slugs, with a title of the requested route | Exhaustive route match; `from_live_hash` maps mock slugs to `NotFound`; recovery views |
| `Mode::from_query`, `main.rs` and Cargo had no gate, so a release bundle selected the mock by URL | Cargo feature `preview` (off by default) gates the entrypoints, the mock shell, `MockCatalog`, `FakeMedia`, the diagnostic controls and the preview-only copy |
| `web-performance` and `release-build` already refuse inherited `TRUNK_*`; `deploy` did not | `build::release_bundle` removes `TRUNK_BUILD_FEATURES`, `TRUNK_BUILD_ALL_FEATURES` and `TRUNK_BUILD_NO_DEFAULT_FEATURES` from the child (`env -u …` in the plan) |
| The seed has four public Works (< 20) | The Work walk needs a smaller page to show a real second page; the Unit walk (23 episodes) is a real two-page walk without any shim |

## Design in one paragraph

Pure core, thin shell. `DiscoveryQuery::new` trims and refuses what the server refuses (512
bytes, control characters, a language tag outside `[A-Za-z0-9-]{1,63}`), so equal spellings share
one identity and an invalid search is never sent. `PublicDiscoveryState` issues tickets and takes
each outcome back: stale tickets are `Ignored`; an answer that adds no row but keeps a cursor
continues within a budget of four, then waits for the listener (`Incomplete`, never "no results");
a later-page failure keeps the rows and retries the same cursor; `invalid_cursor` keeps the shown
rows but only a fresh walk continues, never merged. The applied search lives in the link
(`#discover?q=…&kind=…&language=…`), one history entry per distinct search, so Back/Forward and
returning from a Work restore it; an answered search is kept with its rows and cursor. The Work
and Unit adapters send `presentation=1` and decode `Presented*` DTOs; `unit_count` and `ordinal`
are displayed as a public-list count and position, never as an identity or authored number.

## Acceptance criteria (issue #270)

Evidence labels follow the [engineering vocabulary](../../.agents/skills/cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified).
"Browser" is the Claude **in-app browser** on this Mac, against `http://127.0.0.1:8082/` (Trunk,
proxying `/api/` to the real API on 8080 over the retained, seeded PostgreSQL cluster), unless
stated otherwise. Fault injection and the page-size shim below are page-level wrappers of
`window.fetch`; the server, cursor and PostgreSQL were real.

| Criterion | Status | Evidence |
| --- | --- | --- |
| Browser search by `q`, `kind`, `language` matches the API/PG; A→B does not accept A's late answer | met (local) | `integration-tested` + `interaction-tested`: `q=đêm` → 2 Works; `+kind=podcast` → 1; `+language=en-us` → 0; `language=en-us` → 1; each equals the same request sent to the API with `curl`. The request is `GET /works?q=%C4%91%C3%AAm&language=en-us&limit=20&presentation=1`. With the answer to **A** delayed 1.8 s and **B** issued 250 ms later, B's rows were on screen at 0.7 s and A's late answer changed nothing (`fault-injected` delay) |
| Dataset over ≥ 2 pages or a page small enough for a real cursor; Unit and Work pagination | met (local) | Units: _Chuyện đêm khuya_ is a real 20 + 3 walk: "Tập đã công bố: 23", ordinals 1–20, **Tải thêm tập**, ordinals up to 23, then the end. Works: the seed has four public Works, so the page's `fetch` rewrote `limit=20` to `limit=2` (shim); the real API then returned a real `lc1_…` cursor for rows 3–4. Two quick activations of **Tải thêm** sent one request |
| Regression for an empty intermediate page, `invalid_cursor` and a later-page failure; no lost rows, no loop | met | `example-tested` (state machine, decoder, view): empty page + cursor continues within a budget of four then shows "keep searching"; exact `ListenerError` pairing classifies `invalid_cursor`; a failed page keeps its rows and the next request carries the same cursor. Browser: a later page rejected by the shim (`fault-injected`) kept the rows and **Thử lại phần này** re-sent the same cursor; a cursor replaced by another unknown cursor made the **real API** answer `invalid_cursor`: rows stayed, "hết hạn" with **Tải lại từ đầu**, whose fresh page one replaced them. An empty intermediate page cannot come from the seed: not observed in a browser (unit-tested only) |
| Work/Unit show total and ordinal from the Presented DTOs, never a page length | met (local) | `example-tested` (rows 7 and 8 of a Work of 40 show "Chương đã công bố: 40", "Vị trí 7/8"). Browser: counts 4, 2, 23, 2 from PG; the story with one withdrawn chapter (positions 0 and 2) shows ordinals 1 and 2 |
| Unknown route has a recovery view; Explore/Player no longer a one-screen title over Home; Back keeps the search; navigation never plays | met (local) | `#nope`, `#work/son-ha-ky` (a mock slug), `#library?q=x` → "Không tìm thấy trang này" with **Về Khám phá**; `#queue` → "Tính năng này chưa mở"; `#explore` renders the screen; `#player` keeps its empty state. Search → open a Work → Back: same rows, query and focus on the opening card, **no** new `/works` request; Back/Forward through two searches restored each. No `<audio>` was created |
| A release build does not choose mock/preview from the URL; an opt-in preview build still opens the specimens | met (local) | Browser on the **release** bundles served statically: production bundle, all three URLs → the live Khám phá page with the honest "could not open the catalog" state, no demo notice, controls or sample titles. Preview bundle (`TRUNK_BUILD_FEATURES=preview`): default and unknown mode → live; `?mode=mock` → demo notice and sample catalog; `?mode=auth-preview` → fixture controls; `?mode=membership-preview` → the Membership preview. Secondary: the production `.wasm` contains none of `mode=mock`, `Sơn Hà Ký`, `Auth preview`, `Fixture:`, `Mock demo`, `Simulate play`, `mô phỏng`; the preview one contains all |
| API down shows a real error and a retry; no sample catalog | met (local) | The API process was stopped while the page was open: **Chưa mở được danh mục công khai**, **Thử lại**, zero cards, no sample text; retry stayed in that state. The page-level failure injection gave the same |
| Existing UI at narrow/desktop, long Vietnamese, light/dark and keyboard | **partial** | `cross-viewport-inspected` 1280 × 900 (dark) and 375 × 812 (light): no horizontal overflow, 48 px controls, chips wrap. `interaction-tested` keyboard: Tab to a chip and Enter applied `kind=podcast` once and kept focus on the chip (a first version swallowed Enter on chips; found by this run, fixed). Not run: text scaling, forced colors, reduced motion, a screen reader, 320 px, a dark narrow capture |

The issue is referenced with `Part of`, not `Closes`: the acceptance list's text-scale and
accessibility parts were not run, the empty intermediate page was not seen in a browser and the
Work walk used a page-size shim.

## What ran

| Layer / claim | Status | Level and command | Verified scope / gaps |
| --- | --- | --- | --- |
| Query normalization, walk state machine, route/hash codec, decoders, views | PASS | `example-tested`; `cargo test --locked -p cantos-theatre --features preview` | Exact variants and states; tickets, budgets, same-cursor retry, expired cursor, identity once, generation exhaustion, hash round trip and refused values, both locales |
| Production lane compiles and lints | PASS | `statically-checked`: `cargo clippy -p cantos-theatre --target wasm32-unknown-unknown --locked -- -D warnings` | No dead code or unused import with the feature off |
| Preview lane | PASS | `cargo clippy -p cantos-ui -p cantos-studio -p cantos-theatre --all-targets --target wasm32-unknown-unknown --features cantos-ui/preview,cantos-theatre/preview --locked -- -D warnings` | Tests and lint always include the preview code |
| xtask | PASS | `cargo test --locked -p xtask` (474 passed) | Gate commands, `env -u` of the release bundle, deploy dry-run text |
| Release bundles | PASS (local) | `trunk build --locked --release …` of the production and the `TRUNK_BUILD_FEATURES=preview` lane, grepped and opened in the browser (above) | Local static serving; not a deployed Hosting site |

### Resumed handoff: completed `preflight3.log`

On 19 October 2026 the existing scratchpad run completed with exit 0 and the exact result
`preflight: PASS with 7 explicitly skipped (NOT_RUN) step(s)`. This is a partial preflight,
not an all-gates pass. Command (Rust 1.98.1, Python 3.14 on `PATH`, `LC_ALL=C`,
`TMPDIR=/private/tmp`, `CARGO_INCREMENTAL=0`):

```sh
cargo xtask preflight --changed-only --skip repository --skip postgres \
  --skip audio-inspection --skip cmp-ui-preview --skip theatre-cmp-demo \
  --skip mutation --skip dbsp
```

| Printed status | Gate |
| --- | --- |
| PASS | `rust-format` |
| PASS | `script-ir-reference` |
| PASS | `protobuf-reference` |
| PASS | `rust-lint (workspace)` |
| PASS | `rust-tests (workspace)` |
| PASS | `forge-functions` |
| PASS | `wav-reference` |
| PASS | `web-lint (cantos-studio, cantos-theatre)` |
| PASS | `web-build (cantos-studio)` |
| PASS | `web-build (cantos-theatre)` |
| PASS | `ui-preview` |
| NOT_RUN | `repository`, `dbsp`, `audio-inspection`, `cmp-ui-preview`, `theatre-cmp-demo`, `mutation`, `postgres` — explicitly skipped |

The earlier `preflight2.log` failed `rust-format`; formatting was corrected before this run,
which passes that gate. No failure remains among the executed gates of `preflight3.log`.
The dependency warning about future Rust incompatibility in `proc-macro-error2 2.0.1` did not
fail lint or compilation.

The original `preflight.log` failed `repository`. A clean-baseline check at
`develop@26fd84c5` reproduces six stale Leptos component qualifications (`Button`, `TextField`,
`Surface`, `Badge`, `BackButton`, `Logo`) and the resulting guarded-adoption errors. This tree
has the same failure class, including the new `TextField` use in `public_home.rs`.
The repository gate remains **FAIL**, not an environment gap or a passing qualification.
Refreshing hashes without fresh renderer review would not repair it; shared-component
qualification and the new adoption remain acceptance blockers.
The resumed `cargo xtask verify repository` on the handoff documentation and unchanged Rust
sources reproduced exactly these errors (`04-repository.log`, exit 1); service validation
passed before the checker failed. The Python unit suite was not reached in that gate.

The other six skips were retained from the supplied run. Their scope is unchanged backend
SQL, audio inspection, DBSP, Script IR mutation and CMP consumers, selected conservatively
because `xtask` changed. They were not attempted in this preflight; no missing-tool claim is
made. The separately recorded `postgres --suite schema --keep` PASS covers only that narrow
suite, not the full `postgres` gate. To finish these checks, provision the documented tools
and run `cargo xtask verify dbsp`, `cargo xtask verify contracts`, `cargo xtask ui-preview cmp
verify`, `cargo xtask theatre-demo verify`, `cargo xtask verify mutation` and
`cargo xtask verify postgres`. Each required gate must report PASS before its acceptance;
next owner: unassigned — assignment required before acceptance.

Provenance: baseline `26fd84c52d4b57f68beeaa7c9c78b2b68a3d1e0c` plus the resumed dirty tree.
The SHA-256 of `git diff --binary -- apps/cantos-theatre/web/src xtask/src` is
`f0c54345eabd15ce056614336fa5bd5bd60e203e05930509e26762699a5773cd`.
Those Rust sources were unchanged after the run; subsequent edits correct preview instructions,
a Cargo manifest comment and this record only. The browser observations above are retained
from the original local session; the resumed handoff did not rerun the browser journeys.
Raw logs and the diagnosis index are retained locally under
`artifacts/agent-runs/2026-10-19-issue270-resume/`; they are ignored and are not PR attachments.

Layer ledger: L1/L2 architecture, query types and closed errors are source-reviewed; L3/L4
query/walk/routes/decoders/views and request orchestration are `example-tested` and
`statically-checked` by the passed workspace and Web gates. L5 local API/PostgreSQL/browser
acceptance is limited to the journeys and named fetch doubles above. Shared-component
admission, full conservative gates and the unrun accessibility/browser matrix remain deferred.
No database query, persisted record, API schema, publication, rights or approval rule changes;
no provider call, audio production or deployment occurred. Existing presentation DTOs are
consumed without changing default-v1 clients. No new ADR is needed for this consumer slice.
The 040 work-plan boundary and live-client matrix link the implementation; queue ordering
and GitHub issue status are unchanged.

Not covered: Safari/iOS and Firefox, a screen-reader walkthrough, text scaling, an empty
intermediate page in a browser, deployed Hosting/CORS addressing, a long-lived cursor expiry (the
15-minute vault TTL was not waited out; an unknown cursor was used), native and CMP clients.

## Environment, doubles and pre-existing conditions

- Stack: `cargo xtask verify postgres --suite schema --keep` (PASS), `cargo xtask seed theatre`,
  `CANTOS_TEST_CONTEXT=theatre cargo xtask run cantos-theatre --web --test-cluster`. No TLS front:
  discovery needs none and audio was not exercised here.
- The request line seen by the server ends with a trailing `&`; it comes from the HTTP client
  builder, predates this change (the unit requests had it) and the API ignores it.
- The in-app browser has no text-scaling control; text scale is `NOT_RUN`.

## Reproduce

```bash
cargo xtask verify postgres --suite schema --keep
cargo xtask seed theatre
CANTOS_TEST_CONTEXT=theatre cargo xtask run cantos-theatre --web --test-cluster
```

Open `http://127.0.0.1:8082/`, search **đêm**, choose **Podcast**, open _Chuyện đêm khuya_ and
press **Tải thêm tập**. For the previews: `TRUNK_BUILD_FEATURES=preview cargo xtask app
cantos-theatre --web` and open `?mode=mock`.
