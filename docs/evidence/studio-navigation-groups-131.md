# Studio grouped navigation and modal drawer (#131)

Source-bound implementation evidence, based on `develop@31fa8d86fe83a9fb3db814081acb0479d9044cdc`
after PR #747. This record covers shell navigation only. It does not accept the complete legacy
#131 login/dashboard scope or the Content internals (#571) and utilities footer (#582).

## Boundary and component lookup

The Studio-owned composition reuses qualified `NavItem`, `Button` (tonal) and `Dialog` APIs.
No shared component, token, preference, transport or domain contract changes. Theatre expression
remains selected. Three presentation groups expose all six existing pane hashes, including
native modified-click links. Two scoped link sets share one panel signal; only chrome is
responsive. The existing mounted workspace tree and history/draft/session owners stay intact.
A scoped footer callback can render the same injected preference owner with unique IDs in rail
and drawer; this slice supplies no new preference/account UI.

The rail uses 15 rem, leaving approximately 50 rem for the current task before the 68 rem
container breakpoint. The drawer uses shared native modal opening, initial close focus,
Escape and focus restoration. A cleaned-up ResizeObserver observes CSS visibility rather than
duplicating the breakpoint in JavaScript. Expanding to the rail closes an open drawer and
focuses the corresponding link. Session gating closes the drawer before private content is hidden.
No new router, hydration workaround, data read or mutation is introduced.

No ADR: this selects no library or domain policy. Behavior SSOT impact: none; groups and modal
visibility are presentation state over existing pane identities and retained session fences.
Queue: no change; ordering and ownership are unchanged.

## Design and provenance

Read HANDOFF-v3.md (`1nlZGJQnNMUShKjR624Zb0tE_WtF7h9ve`) and opened v3 workspace and
390px drawer captures from `Cantos-634-UI-UX-v3.zip` (`1GGXqs3wYUPv6ifcWJympahRr4Rhm7cHp`).
Opened the current production reference `1L2D-zH0UPHXdpe50PZMy8Safwi7gDWs_` as provenance.
Reference fixtures do not establish Leptos behavior. Geometry stays Studio-owned and visual
roles come from the existing shared Theatre expression; fixture colors are not copied.

Full local logs, build copies, capture driver, reports and screenshots are retained in ignored
`artifacts/agent-runs/navigation-131/`. The before build is the pinned base; the after build is
the PR source. Both were built by `cargo xtask build cantos-studio --web` (debug CSR).
The capture harness serves the exact built static bytes on task-owned loopback port 18131;
Playwright intercepts every API request with synthetic session/catalog/content responses.
It performs no production I/O and provisions no real identity, cookie session or database.

## Verification boundary

Native Studio tests include the six-pane partition, stable hrefs, scoped IDs and current-group
rendering in VI/EN, alongside existing draft/session guards. WASM lint runs with warnings denied.
The final PR contains the actual final-diff preflight summary; broad gates and environment gaps
are reported individually rather than inferred from the command's exit code.

The browser ledger records Chrome version, CSS viewport, DPR 1, zoom 1, theme, locale and root
font size per capture. Text 200% means a 32px root font, not browser page zoom. Browser/native
focus checks are separate from screen-reader acceptance. Chrome may place focus in browser
chrome between the last and first native-dialog controls; no background DOM control receives
focus during that cycle. No custom focus trap replaces the shared native modal owner.

Real native API/authentication, OS IME, screen reader, actual browser zoom, forced colors,
production/deployed acceptance and a populated #582 footer are NOT_RUN in this slice.

## Observed results

- `cargo test --locked -p cantos-studio`: 498 tests and 5 doctests passed on the final source.
- `cargo xtask verify web`: PASS for Studio/Theatre WASM lint, both app builds and shared preview.
- `cargo fmt --all -- --check`: PASS. Production resource generation/checks remain canonical;
  Theatre Web/CMP generated changes contain only the source-catalog digest, not new behavior.
- Chrome 156.0.8078.80 (headless): 14 before captures and 31 final after captures; DPR 1,
  zoom 1. Matched Content/draft data at 1440, 1120, 1100, 1024, 768, 390 and 320 CSS px,
  Light/Dark. After also records 1121px, the other side of the 68rem container transition.
  No recorded document horizontal overflow; no page errors in the final run.
- Driven initial close focus, native Tab/Shift+Tab cycles, attempted focus on the inert header,
  Escape/Menu restoration; drawer → Import → browser Back retained exact draft text, native
  selection 8–28, textarea scrollTop 80, and the same textarea/import/production DOM nodes.
  Resize while open dismissed the drawer and focused the matching rail link. All six hashes
  were independently loaded into a fresh document and selected their correct group and task.
  Modified-click preserved the original pane. A protected read returning synthetic 401
  withdrew the current session, closed the drawer and hid/inerted the private workspace.
- English at 600×420 with root text 200% reached the last task and a temporary synthetic
  footer-slot button by keyboard focus. The temporary button was removed afterwards; this is
  geometry evidence, not #582 preferences/account implementation or qualification.

Opened before/after pairs: desktop 1440, transition 1120, narrow 390 and 320 in both themes.
Opened drawer 390 Light, 320 Dark and the English 200% short-height/last-task/footer specimens.
The same long Vietnamese work/chapter names and unsaved body were used. Hierarchy is now
explicit, all labels stay visible, and the narrow shell no longer requires a horizontal nav strip.
Other captured widths remain available for inspection in the retained local bundle.

The full `cargo xtask preflight --changed-only --base origin/develop` broadened to all gates
because the catalog path has unknown impact in the selector. It was interrupted during
workspace tests at a 1.8GiB free-disk floor after producing over 10GiB of build files.
Rust format, Script IR reference, Protobuf reference and workspace lint had completed; workspace
runtime tests are partial. Forge/Turso/audio/mutation gates are NOT_RUN, and CMP preview gates
are UNSUPPORTED (Gradle missing). These do not establish full-repository acceptance. Their
unaffected runtime domains are outside this presentation slice. Final scoped Web/Studio checks
above cover the changed behavior; the PR records repository-gate results separately.

An initial repository check classified a DOM heading ID constructed inside `view!` as displayed
copy. ID construction moved to the group identity helper; no allowlist or checker was weakened.
The source check then passed. The first Python gate run in the restricted sandbox failed on
loopback/process-group EPERM; its complete log is retained, separately from the permitted run.

Final `cargo xtask verify repository` in the permitted local environment: PASS, including
1,384 Python tests (20 explicitly skipped), static architecture/localization checks and Git
whitespace checks. The earlier restricted-sandbox failure is environmental and retained; it
is not reported as a successful run. The only later tracked edit is this result record,
validated again by the repository source primitive.

Self-review covered all handwritten source, generated resource/digest changes, responsive CSS,
the shared Dialog/NavItem consumers, unchanged workspace composition and relevant existing
session/history/draft owners. No blocking finding remained. No dependencies, durable records,
API, permissions, migrations, publication or playback behavior change. No provider cost or
deployment was incurred. Performance and motion were not benchmarked/recorded; screen reader,
real browser zoom, OS IME and populated #582 utilities remain separate acceptance.
