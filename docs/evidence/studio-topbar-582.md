# Studio topbar #582 — keyboard layout follow-up

Status: bounded implementation/browser evidence for [PR #591](https://github.com/loveoverflowcom/cantos/pull/591).
[PR #590](https://github.com/loveoverflowcom/cantos/pull/590) landed the main v7/v8 styling while
this work was underway. The final diff preserves that implementation, including its semantic
container colors, 8px language padding, icon-only theme action and existing regression tests.

## Invariant, cause and boundary

Keyboard focus and the complete theme tooltip must remain visible without horizontal page
scrolling at narrow and desktop widths. Locale capitalization is presentation only: canonical
tags, registry choices, full menu names, saved preference and actual account/session authority
remain unchanged.

The landed baseline's centered tooltip extended to x=1302.29 at a 1265px client width under a
requested 1280px viewport. This follow-up right-aligns and bounds/wraps that tooltip, keeps a
wrapped toolbar at the header's right edge, and displays short codes as `Vi`/`En`.
Only Studio CSS, its README and this report differ from the integration base.

| Completion field | Scope / evidence |
| --- | --- |
| Work item / acceptance | Part of #582; v7/v8 core belongs to #590, remaining acceptance is below |
| Review boundary / owner | Studio Web topbar presentation in `studio.css`; existing Select, ThemeCycleButton and AccountEntry reused |
| Prerequisites / decisions | Landed #590 retained; no new shared API, token, dependency, ADR or domain decision |
| Branch / base | `codex/studio-topbar-582`; started from fetched/pulled develop `afed522366b1a498432686ba156978e7e46a17f9`, then merged develop `2a605959f3e7164b7c67759fc52ef92519f9a0b9` at `182137d9` to resolve the overlap |
| Docs / queue | Studio README updated; queue unchanged because order/dependencies/rationale did not change |
| Prompt / outward actions | Plain user request for implementation, PR and worktree removal; no issue edit, close, merge or deploy |
| Performance | Not affected; no query/cache/startup measurement or performance claim |
| L1 Architecture | Owner/dependency audit; consumer-only CSS, no new dependency edge or shared adoption |
| L2 Domain/types/errors | Inapplicable: no durable fact, domain rule, refined type, session policy or error behavior changed |
| L3 Static/unit | Existing landed v7 language/theme regressions retained; final format/native lint/unit/WASM lint/build results are recorded in the PR |
| L4 Application/UI | `DOM-tested`, `interaction-tested`, `accessibility-checked` for anonymous language/theme controls; real native local composition, no HTTP doubles |
| L5 Runtime | `screenshot-inspected`, `cross-viewport-inspected`, `cross-theme-inspected`; real anonymous CSR/WASM browser only |

## Commands and provenance

Run from the repository root with Rust 1.98.1 and Trunk 0.21.14. Build settings were
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`;
verification used `TMPDIR=/private/tmp`.

```sh
cargo xtask run cantos-studio --web --web-origin http://127.0.0.1:8084
cargo xtask preflight --changed-only --base 2a605959f3e7164b7c67759fc52ef92519f9a0b9
```

The local xtask runner owned Server/Identity with private Turso files and mail disabled. No
Studio account, provider, cloud storage, private manuscript or media was used. Browser evidence
used CUA's Codex in-app browser on macOS; its backend did not expose an engine version. Pointer
and keyboard were driven, at 100% text. No touch emulation, OS IME or assistive technology ran.

Final screenshots/measurements are in the ignored `artifacts/agent-runs/studio-topbar-582/`:
`integrated-before-{1280,320}-system-en-tooltip.jpg`,
`integrated-after-{1280,320}-system-en-tooltip.jpg`,
`integrated-after-{320,390,1280}-{light,dark}-{vi,en}.jpg`,
`integrated-browser-observations.json` and `integrated-tooltip.json`.
Exact captured JPEG bytes were opened by the implementation agent. Source/capture digests
and full logs remain there; they are preserved locally when the worktree is removed.
Earlier unprefixed images and logs describe the original afed5223/6459d079 attempt and are
historical evidence, not the final integrated source.

## Final browser observations

| Claim / edge class | Observed result |
| --- | --- |
| Same-scenario tooltip recheck | Requested 1280×800, System, en, anonymous, empty inputs, focused theme: before page width 1302/client 1265; after both 1280, tooltip x=1113.42–1264 |
| Narrow tooltip | Requested 320×720, System, en: after page/client width 320 and tooltip x=153.42–304; no clipping or horizontal scroll |
| Responsive matrix | Light/Dark × vi/en at 320×720, 390×844 and 1280×800 (12 cases); all final page widths match client widths |
| Targets / geometry | Language 59.52×48px (`Vi`) / 63.38×48px (`En`), inherited 8px padding; theme 44×44px with 12px radius |
| Contrast | Both language and theme text/icon pairs 11.11:1 Light / 10.04:1 Dark; computed opaque foreground/background pairs |
| Keyboard focus | Theme 3px solid indicator, 5.55:1 Light / 6.26:1 Dark against its fill; visibly retained in captures |
| Locale menu / persistence | Full `Tiếng Việt`/`English` autonyms and selected/expanded state; Arrow+Enter changed locale, `<html lang>` matched, Vietnamese survived reload; Escape kept focus on the collapsed trigger |
| Theme interaction | Enter then Space advanced Dark → Light → System once per activation; localized current/next names and System icon retained; System used current host Light appearance |
| Account / async | Anonymous account entry remained hidden; existing actor/session owners and intents unchanged; no request lifetime or async decision changed |
| UI intent / tokens / copy | Low-intensity Studio chrome; semantic source and both renderer mappings unchanged; no new localization key/literal; case changes only visual short-code presentation |
| Motion | No animation added; timing, smoothness and reduced-motion runtime not observed |

This is a CSS-only repair with reproduced and inspected before/after evidence; the retained
landed regressions cover existing language/theme composition. No new implementation-mirroring
test was added. The earlier input-value preservation probe was inconclusive and is not a pass.

## Gate diagnosis and deferred acceptance

The initial unskipped preflight failed on 16 stale Leptos qualifications plus 34 adoption
diagnostics. Its failing repository primitive also failed on a clean detached afed5223 base
with the exact same ordered 50 diagnostics. Those records/baselines were not modified.
The final integration preflight and any independent `--skip repository` summary belong to
the PR; an explicit repository `NOT_RUN` never replaces its original `FAIL`.

The earlier Python repository substep on the original base ran 1,318 tests: 1,298 passed,
18 skipped, two unchanged legacy VPS tests failed because BSD `sed` syntax differs and
`flock` is absent. This is source-bound historical evidence, not a rerun of the merged tree.

| Deferred check / status | Reason / exact next step | Next owner / residual risk |
| --- | --- | --- |
| Repository qualification: FAIL | Complete actual shared-renderer qualification/adoption, then rerun the unskipped preflight above; never refresh digests without observations | Unassigned; aggregate repository acceptance stays red |
| Legacy Python environment / optional skips | Run `python3 -m unittest discover -s scripts -p 'test_*.py'` in its supported GNU/Linux tool environment; no tool installation or legacy-runtime activation occurred here | Tooling owner unassigned; earlier 2 failures/18 skips remain disclosed |
| 200% text, forced colors, touch, reduced motion: NOT_RUN | Run the same Studio command in a browser supporting those settings; inspect controls, menu and tooltip without clipping/lost focus | Unassigned; full AC-03 remains partial |
| Dynamic System OS changes / refused storage: NOT_RUN | Change live OS appearance and refuse storage in that browser; verify one cycle action and localized storage warning | Unassigned; only current-host System and successful preference persistence observed |
| Authenticated account / screen reader / deployed: NOT_RUN | Use approved nonproduction Studio credentials/roster and a named reader/environment; repeat avatar, menu and focus scenarios | Unassigned; anonymous DOM/browser checks accept none of these |

AC-01/02 retain #590's implemented behavior with this bounded recheck; account/refusal paths remain
separate. AC-03 is partial. AC-04 has final-source browser captures and explicit check results in
the PR. No migration, Script IR/API, cache/audio revision, provider cost, rights/QC/approval or
publication behavior changed. Finish the deferred matrix before full issue/production acceptance.
