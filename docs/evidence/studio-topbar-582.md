# Studio topbar v7/v8 — bounded browser evidence

Status: implemented presentation with local browser observations for [#582](https://github.com/loveoverflowcom/cantos/issues/582).
The remaining acceptance below is separate from these results. The issue's v7/v8 direction
supersedes #490/#558's earlier topbar styling; their other work is outside this record.

## Scope, invariant and ownership

The compact topbar must preserve canonical locale selection/storage, the three-state theme
action, accessible names/focus and actual-account-only visibility. Its previous v6 composition
selected a labelled theme pill and overrode shared tonal controls with account-action colors.
Studio now selects the existing tonal, icon-only `ThemeCycleButton`, inherits the shared tonal
Select states, and paints the short locale code with initial capitalization and 12px side padding.
The registry tags and full menu autonyms are unchanged. The existing actor badge/disclosure and
all account/session intents are unchanged; no identity is invented for the login screen.

The browser also reproduced a centered theme tooltip overflowing the right viewport edge
(page width 1302 at a 1265px client width). Studio now anchors that tooltip to the control's right
edge, bounds/wraps its text, and right-aligns a wrapped toolbar. The same keyboard scenario then
stayed within the 320px viewport, including the full Vietnamese tooltip.

| Report field | Scope / result |
| --- | --- |
| Work item / review boundary | #582; one Studio Web presentation change, with remaining acceptance explicitly deferred |
| Prerequisites / decisions | Existing Select short-label callback, tonal theme variant and account disclosure reused; no new shared API, token, dependency or ADR |
| Owner / boundary | `apps/cantos-studio/web/src/view.rs`, `studio.css`, existing `messages/entry.rs` regression; shared renderers and backend unchanged |
| Branch / base | `codex/studio-topbar-582`; fetched/pulled `origin/develop` at `afed522366b1a498432686ba156978e7e46a17f9` before edits; isolated worktree |
| Docs / queue | Studio README updated; queue unchanged because dependency/order/rationale is unchanged |
| Prompt / outward actions | Plain user request to implement/open a PR and remove the worktree; no merge, deployment, issue edit or account provisioning |
| Performance | Not affected; no query, cache, startup or runtime-performance claim |
| Layer L1 | Owner/dependency review: consumer-only composition and geometry; no new dependency edges or shared adoption |
| Layer L2 | Inapplicable: no domain value, durable decision, type, error or settled record changed |
| Layer L3 | `example-tested`: replacement of the superseded v6 regression failed on the original labelled action and passed after the presentation change; final package/format/lint results belong to the PR's preflight summary |
| Layer L4 | `DOM-tested`, `interaction-tested`, `accessibility-checked`: real Studio language/theme flow, keyboard traversal, menu dismissal and names/state; no HTTP doubles |
| Layer L5 | `screenshot-inspected`, `cross-viewport-inspected`, `cross-theme-inspected`: real local anonymous browser; no authenticated-account, deployed or production acceptance |

## Commands and provenance

Run from the repository root with Rust 1.98.1. Builds used
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0` to limit local disk use.

```sh
cargo xtask run cantos-studio --web --web-origin http://127.0.0.1:8084
cargo test --locked -p cantos-studio --lib studio_topbar_keeps_registry_selection_and_the_shared_tonal_icon_action
cargo xtask preflight --changed-only --base afed522366b1a498432686ba156978e7e46a17f9
```

The focused test was run red before the production patch and green afterward. Both the baseline
and changed CSR/WASM builds were served through the actual xtask-owned native Server/Identity
composition with mail disabled and no Studio account. Browser work used the Codex in-app browser
on macOS through CUA; the backend did not expose its engine version. Pointer and keyboard were
driven; no touch emulation, OS IME, assistive technology, paid service or private media was used.
Final preflight commands/results are recorded in the PR rather than implied by a build.

The initial `preflight --changed-only --base origin/develop` returned `FAIL` in `repository`: 16 stale Leptos qualifications
and 34 consumer-adoption diagnostics. The exact failing primitive,
`python3 scripts/check_repository.py --base origin/develop`, also failed in a clean detached
worktree at the pinned base with the identical 50 diagnostics. Classification: **pre-existing**,
not an environment gap or a passing gate. No qualification records, hashes or adoption baselines
were changed. `origin/develop` advanced during verification, so subsequent checks
use the immutable fetched base SHA shown above. A subsequent preflight with `--skip repository`
runs the independent format/lint/test/Web gates while explicitly keeping repository `NOT_RUN`;
it does not replace the original `FAIL`. The PR records both summaries and remaining repository
substep results.

The independent repository Python substep ran 1,318 tests: 1,298 passed, 18 skipped, and two
failed on this macOS environment. The unchanged legacy tests
`test_vps_epoch2_cutover.Epoch2CutoverTests.test_journal_metadata_must_also_bind_the_exact_attempt`
and `test_vps_oidc_reconcile.VpsOidcReconcileTests.test_nested_operations_reuse_the_same_lock_descriptor`
encountered BSD `sed` syntax and missing `flock`, respectively. These are actual failures with
environment causes; no tool installation, legacy runtime activation or test repair was attempted.

Production-source SHA-256 at browser observation:

| Path below `apps/cantos-studio/web/` | SHA-256 |
| --- | --- |
| `src/view.rs` | `02e845656d422cea0e49f132c007303a4b59514d2dd930a2aafd939126eba50d` |
| `studio.css` | `f1e96ad1efa7a4b0c6f3cf986d5bf9b9ce125edd1b3ba26186af42679c13a751` |

Evidence files remain outside Git in `artifacts/agent-runs/studio-topbar-582/`:
`browser-observations.json`, `keyboard-observations.json`, source/capture digests and logs.
The four same-scenario before/after pairs are named
`{before,after}-{1280-dark,1280-light,320-light,390-dark}-vi.jpg`.
Viewport heights are respectively 800, 800, 720 and 844 CSS px. Captures use empty login inputs,
100% text, an anonymous session and the same focused theme control with its tooltip dismissed.
The actual saved pixels were opened by the implementation agent; the smaller controls remain
legible, and the final 320px header fits on one row where the baseline had wrapped.

## Browser observations

The changed page was inspected in Light and Dark at 320×720, 390×844 and 1280×800, in both
vi-VN and en (12 combinations). No horizontal overflow was measured in those final states.
The exact same shared components receive semantic roles from the existing Theatre expression;
no canonical token or generated file changed. This is quiet Studio chrome at Low intensity.

| Observation | Actual result |
| --- | --- |
| Targets / geometry | Language 67.52×48px (`Vi`) or 71.38×48px (`En`), 12px side padding; theme 44×44px, 12px radius, no decorative border or visible mode label |
| Text/icon contrast | Language 13.12:1 Light / 12.64:1 Dark; theme icon 11.11:1 Light / 10.04:1 Dark; computed opaque foreground/background pairs |
| Theme keyboard focus | 3px solid indicator, 5.55:1 Light / 6.26:1 Dark against the button fill; visible in the opened captures |
| Locale menu | Full `Tiếng Việt` / `English` names, selected option, expanded state, no leading language icon; short code does not replace the accessible full name |
| Locale interaction | Arrow keys + Enter changed locale and `<html lang>` to `en`/`vi-VN`; English survived reload; canonical preference owner/callback and warning handling retained |
| Dismissal / focus | Escape collapsed the language menu and kept focus on its trigger; opening then Tab without navigation retained English and moved focus to theme; pointer outside dismissed |
| Theme interaction | Enter/Space each advanced one mode, System → Dark → Light → System, with localized current/next tooltip/name; System used the host's current Light appearance |
| Tooltip recheck | At 320px, Vietnamese tooltip bounds 120.38–304px and page width 320px; full label visible, keyboard focus retained |
| Account / async | Anonymous account entry hidden; actor mapping and availability tests retained. No account, session authority, request lifetime or async race changed |
| Localization / motion | No new key or literal label; existing resources in both locales. No animation changed; timing, reduced-motion runtime and smoothness were not observed |

The input-value preservation probe was inconclusive; it is not counted as an interaction pass.

## Deferred acceptance and residual risk

| Check / status | Reason and next acceptance step | Next owner / risk |
| --- | --- | --- |
| Repository qualification: FAIL | Identical 50 diagnostics on clean `develop`; complete actual shared-renderer qualification/adoption through its owners, then rerun the unskipped preflight command above. Do not refresh digests without the required observations | Shared UI qualification owner unassigned; aggregate handoff remains red |
| Repository Python substep: FAIL | Run the unchanged legacy tests on their supported GNU/Linux tool environment; the local run exposes BSD `sed` and absent `flock`. Eighteen optional tests also skipped and are not counted as passes | Repository tooling owner unassigned; full repository acceptance remains pending |
| 200% text, forced colors, touch, reduced motion: NOT_RUN | Browser automation exposes viewport sizing but no corresponding emulation controls. Run the same xtask command above in a browser with those settings; inspect controls, menus and tooltip without clipping or lost focus | Unassigned — assignment required before full AC-03 acceptance |
| Dynamic System/OS changes and refused preference storage: NOT_RUN | Observe live OS Light/Dark changes and blocked storage in that browser; preserve one cycle action and the localized storage warning. Only current-host System and successful saved locale were exercised | Unassigned; runtime failure branches remain unqualified |
| Authenticated avatar/account: NOT_RUN in browser | Use an approved nonproduction Studio account/roster, then repeat avatar, account actions and narrow popover/sheet focus checks; existing native account regressions establish only their tested scope | Unassigned; authenticated header and actual-session interactions remain acceptance gaps |
| Screen reader / production: NOT_RUN | Named reader/browser walkthrough and separately authorized deployed observation are required for those claims | Unassigned; DOM/keyboard evidence does not establish either |

AC-01/02 have implemented source and bounded evidence; authenticated and refusal paths remain
separate. AC-03 remains partial. AC-04 has actual before/after browser evidence and final checks
reported in the PR. The issue is not automatically closed. Complete the deferred matrix before
claiming the full issue or production acceptance; no backend, migration, audio, rights or publication
behavior changed.
