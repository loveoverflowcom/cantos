# Theatre Web: Của tôi and Tài khoản (Web v2 visual pass)

## Boundary

Base: `origin/develop` `3623afd`, working tree clean before the branch. Scope: the Theatre Web
mock screens **Của tôi** (`#library`) and **Tài khoản** (`#account`), plus the shared pieces they
need (Web panel shape role, bundled fonts, bundled mock artwork, shortened mock notice). No API,
DTO, backend, CMP or domain-state change. Studio is not touched; its generated CSS gains one
unused Web role.

**The Drive reference was not read.** The Web v2 HTML, the Design Handoff and the token file are
behind a Google sign-in that the in-app browser does not have, and this task did not authorize
entering credentials or using another browser session. Everything below follows the task text,
the repository's recorded 2026-10-11 handoff values and the mock's real state. "Follows the
reference" claims are therefore limited to the explicit specs in the task (24px panel and
button radii, Noto Serif/Sans roles, hit target 44px, quiet tonal grouping). A side-by-side
comparison with the HTML is an open acceptance step.

## Cause of the preliminary look (confirmed from source and captures)

| Symptom | Cause | Owner |
| --- | --- | --- |
| Continue row: text at the left edge, CTA at the right edge, no artwork, no progress | `.theatre-continue`/`.theatre-listening-row` were `flex` rows with `justify-content: space-between` over the full content width; the row had no art or progress element | `features/library.rs`, `theatre.css` |
| Account: one wide slab with a sentence, two gray disabled-looking buttons and two text links | one `Surface` holding every item; `UnsupportedAction` rendered disabled `<button>`s | `view.rs` |
| Three repeated mock notices on Của tôi (global, page, section help) | `DemoNotice` + `LibrarySession` + `ResumeSampleHelp`, plus a rail note | `messages.rs`, `view.rs` |
| Noto Serif/Sans not guaranteed | the type roles name the families but no `@font-face` shipped, so a machine without Noto fell back to Georgia/system-ui. This workstation has Noto installed, which hides it | `theatre.css` |
| Typographic placeholder covers | no Web artwork existed (CMP bundled it) | `common.rs` |

## What changed

Reused: `cantos-ui` tokens, `UiRoot`/`Container`/`Select`/`Surface`/`Stack`/`EmptyState` at
their existing baseline sites in `view.rs`, shared focus and forced-color rules, existing Material
Outlined icons (`headphones`, `bookmark_border`, `replay`, `settings`, `library_books`).

- **Web-only `--cantos-web-shape-panel` (24px)** in `foundation.css.in`; generated
  `foundation.css` regenerated; `scripts/test_ui_tokens.py` pins it; documented in the shared and
  Leptos READMEs. Canonical `tokens.json`/CMP outputs unchanged. Panels use `panel` (24), nested
  art/rows use `container` (18), buttons keep `control` (24).
- **Của tôi**: one Continue cluster (artwork, work, episode, progress bar with elapsed/total, the
  action directly below). Other recent works are compact rows capped at 44rem. Saved works use the
  existing `WorkCard`; the saved grid caps card width. History stays quiet. Empty Continue and
  Saved states have an icon, a short title, one sentence and a "Khám phá tác phẩm" link to
  `#discover`; the quiet History empty has no second call to action.
- **CTA label**: reopening a position restores it **paused** (`resume_progress`), so the label is
  "Mở vị trí nghe" and its help says to choose Phát mô phỏng; "Nghe tiếp" stays only as the
  section heading.
- **Tài khoản**: identity panel (guest glyph, "Người nghe khách", one-line status) beside a
  content column with status rows (Đăng nhập and Đồng bộ "Chưa tích hợp", storage "Chỉ trong phiên
  này") and link rows to Cài đặt nghe and Của tôi. One column below 56.25rem; width bounded to the
  content. Status rows are text, not disabled buttons. No conflict choice is drawn because the
  mock has no conflict state.
- **Copy/persistence**: one short global mock notice; page-level duplicates removed; wording says
  data lives only in this session (it is `RwSignal` memory).
- **Assets**: Noto Sans/Serif regular+bold (OFL) as Vietnamese/Latin WOFF2 subsets (about 22 KB
  each) and two 800px WebP derivatives of the CMP mock paintings; hashes and derivation in
  `apps/cantos-theatre/web/assets/provenance.json`. `Cover` shows a painting when one exists
  (Sơn Hà Ký, Đèn Khuya) and keeps the typographic cover for Mộng Hồ Điệp. This also changes the
  covers on Discover, work detail and player (same component). Artwork rights are **unverified**.

### Adjustments made to keep the UX simple (not in a demo)

No extra tabs or functions. Sync/sign-in are status text rather than buttons. History is not
promoted. The hero is one cluster, not a carousel.

### Blocked by repository policy

The shared `Avatar` (and `Surface`/`EmptyState` at new import sites) cannot be adopted by new
Theatre files: `scripts/check_repository.py` requires both-renderer qualification records for a
new use, and none can honestly be produced here (CMP runtime/accessibility/visual review). The
new compositions are app-private Theatre classes, and the existing topbar guest glyph is reused
for the account identity instead of the shared `Avatar`. **Follow-up:** qualify `Avatar` in both
preview apps, then swap `PersonGlyph` for it. No shared settings/action-row or progress component
exists in either renderer, so the rows and progress bar are app-private too.

## Verification by layer

| Layer | Result |
| --- | --- |
| L1 architecture | Presentation only; state still from `TheatreContext`; no port/DTO/IO added; Web shape role kept out of CMP |
| L2 types/mapping | `progress_percent` (pure, clamped), `artwork_src` (exhaustive over `WorkId`); typed `Key` resources vi/en, parity test |
| L3 regression | `cargo test -p cantos-theatre`: 91 passed. New: cluster contents, `progress_percent` edges, artwork mapping and fallback, account state (no buttons, no conflict text), vi/en. Written with the change; not run red against the base (they reference new items) |
| L4 mocked | server-render component tests above; the Theatre mock is the only data source |
| L5 renderer | Chrome 153 headless on Linux against `trunk build` output (dev profile), see below |

Gates: `cargo xtask verify web` PASS; `cargo xtask verify repository` PASS (includes the
token drift and `WebShapeIndependence` tests); `cargo fmt --all -- --check` clean.

### L5 observations (Chrome 153, Linux, headless; opened images)

Same scenarios for before (`3623afd` build) and after (working tree): Của tôi populated and
empty, Tài khoản, Lịch sử, at 1440×1000, 1024×768 and 390×844, light and dark, vi-VN. Every
cited image was opened; captures live in the ignored
`artifacts/ui-evidence/theatre-library-account-v2/{before,after,checks}`.

- Fonts: four faces report `loaded` from `assets/fonts/*.woff2` (this machine also has system Noto,
  so loading is proven by `document.fonts`, not by appearance). Computed: h1, section and work
  titles Noto Serif; episode, time, button, navigation, notices Noto Sans. Weights 500/600 resolve
  to the regular/bold faces.
- Radii: panel 24, primary button 24, navigation 24, cover 18, notice 12.
- Hit targets (measured on Của tôi at 1440 wide): no link or button under 44px after fixing the 27px title links.
- Keyboard: on Tài khoản, Tab reached both link rows and the mini-player link, each with a 3px
  `:focus-visible` outline.
- Flows: Của tôi → "Mở vị trí nghe" reached `#player` paused ("Phát mô phỏng" offered, 12:48 / 28:42)
  and Back returned to `#library` with the cluster; Tài khoản → Cài đặt nghe reached `#settings`.
- Long name (`Ánh đèn cuối sân khấu — Một lời hẹn chưa bao giờ nói ra`): wraps in the cluster and
  mini-player at 390 and 1440; no overflow.
- 200% text (Chrome default font size 32px, so rem media queries respond): no horizontal
  overflow on either screen at 1440, 1024 and 390; the hero stacks at 390. The first attempt
  overflowed at 390 (grid column, saved grid minimum, nowrap time); those were fixed and rechecked.
- Mini-player: scrolled to the bottom, the footer clears the mini-player at 1440 and 390 and at
  200% text on all three widths.

## Not covered

- Comparison with the Drive HTML/handoff/tokens (not readable here).
- Screen readers, forced-colors mode, Safari/Firefox, touch input, real Android/iOS browsers.
- Reduced-motion: no new motion was added; not driven.
- Progress bar semantics are `role="progressbar"` with a localized name; no assistive technology
  was used.
- Studio was not rendered; its `foundation.css` only gained an unused custom property.
- `cargo xtask verify postgres`: unsupported on this machine and unrelated to this change.
