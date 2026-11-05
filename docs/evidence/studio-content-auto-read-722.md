# Direct Content catalog activation

Issue [#722](https://github.com/loveoverflowcom/cantos/issues/722), based on
`develop@aa97e0a3c1c573a4ba9c1818fa8e4212cc80511a`. This Studio Web repair shares the
existing catalog observation and request authority between the desk and Content. It changes
no backend route, schema, permission, persistence or production deployment.

## Boundary and behavior

`ContentWorkspace` starts its existing first-page `Works` command when either catalog pane is
visible and a nonempty matching owner/session observation is ready. `DashboardQuery` keeps the
single attempt per actor/epoch, including after a failed or cancelled read. Existing blocking,
request-context, file-transfer, delegation, cancellation and session-ticket fences still apply.
Manual list refresh remains explicit and the existing page size remains 20.

The app-owned `CatalogStatus` reuses the desk's existing localized status/loading presentation
and `cantos_ui::LoadingState`; it adds no shared component API, tokens, CSS or copy resource.
Content now distinguishes unread, loading, failed, successful-empty and rows. The choose-Work
hint requires a successful page with choices. A failed refresh retains previously read rows;
explicit refresh shows loading and can recover through the existing dispatcher.

Behavior SSOT lookup found no registered capability owning this Studio catalog activation
guard. Class: code defect / missing model coverage, authorized by #722. No model, catalog or
lifecycle is changed; focused native and browser regressions cover the bounded repair.
No ADR is needed. Queue order and dependencies are unchanged; GitHub owns execution status.

## Reproduction and evidence

The two issue-linked production screenshots were downloaded, opened and compared. On the
local baseline with the original workspace activation, cold Content issued **zero** catalog
reads; pressing its list button issued one and displayed the synthetic Work. This reproduced
the reported defect without touching production.

Native command (Rust 1.99.1, Cargo 1.99.1; 12 focused tests passed):

```sh
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test --locked -p cantos-studio live::dashboard::tests
```

New regressions cover visibility/session readiness, empty owner, one attempt after loading,
failure and success, actor/epoch changes, stale request tickets, the 20-row request bound,
and status/choice mapping over both locales with and without rows. Native SSR is not browser
or screen-reader evidence. The PR records final changed-only preflight results and full-suite
counts separately.

Local CSR build: `cargo xtask build cantos-studio --web`, PASS, dev profile, Trunk 0.21.14.
The browser-tested dirty source is identified by SHA-256:

| Input | SHA-256 |
| --- | --- |
| `apps/cantos-studio/web/src/live/dashboard.rs` | `da2f8e5791c014ca9b9182f24ed918e071ce2c182ff7726dd91760c000e8501d` |
| `apps/cantos-studio/web/src/live/workspace.rs` | `de0248fcf2c7fabd691c7abd8f1dae5b9af0814cb5616a9613ab28e7d7ef2b35` |
| Served `cantos-studio-29347de7ce713f36_bg.wasm` | `ed0cd21284184889fae9ba90d5f3d20277391a777ffde5b194732a9551576107` |

Browser runs used Playwright 1.61.1, headless Chrome 156.0.8078.80, DPR 1, vi-VN,
reduced motion and trusted driver input. Main viewport: 1363×936 CSS px. Loaded-list captures
also opened at 390×844 dark and 1363×936 dark. These are scoped cross-theme/viewport
observations, not a complete accessibility matrix.

| Evidence boundary | Actual observations / result |
| --- | --- |
| Real Studio CSR and HTTP adapters; intercepted synthetic HTTP | PASS: cold Content and reload auto-list; desk/Content transitions and theme/viewport changes do not duplicate; hidden Import waits for Content; a delayed session starts no private read before confirmation; login returns to the same hash; empty, 503 failure, explicit retry/loading; delayed old-session response cannot replace the post-logout/reauthentication list. Ten bounded catalog requests, no page errors. |
| Real local Server router, Identity child and separate file-backed Turso stores | PASS: signup through a run-owned loopback SMTP sink and actual proof/password flow; roster from the real subject; browser password login with real HttpOnly/Secure cookie; login-return automatically reads an empty catalog; one original QA Work created through the real Studio form; reload automatically lists it; desk/Content does not repeat; a fresh browser context with the valid cookie cold-loads Content once. |
| Real local read recovery | PASS: abort one catalog transport at the browser boundary; failure retains prior rows and does not loop; explicit refresh returns through the real API and clears failure. Four catalog reads in the primary real-service context, one in the fresh context. No page errors. |

The real-service host reuses `identity_password_turso.rs`'s `World` and `identity_world.rs`'s
production-router adapter. A temporary test HTTP bridge forwards to that router without
mocking its authentication, authorization or Turso operations. A task-owned HTTPS front serves
the exact CSR artifact and forwards `/api/`; a self-signed local certificate is explicitly
trusted only by the browser test context. Identity uses the real executable, Argon2id and
private Unix socket. SMTP never leaves loopback. The temporary host source is preserved in
ignored evidence and removed from the source diff. This is local integration evidence, not
deployed-origin, public CA, production data or external-mail acceptance.

## Local evidence and limits

Required changed-only preflight ran against `origin/develop` and **FAILED** at the repository
gate: 20 shared Leptos component qualifications have stale source/token identities, followed
by consumer-admission errors across Studio, Theatre Web and CMP. The other planned gates were
`NOT_RUN` because preflight stops at its first failure; their separately executed commands and
results are listed in the PR. No check or qualification was weakened or rewritten.

This failure reproduces against a clean `git archive origin/develop` tree using
`python3 scripts/ui_preview_catalog.py --check --output-root /tmp/cantos-722-clean-baseline`.
Its renderer/token digests equal this checkout's: Leptos
`646ddccb291020483eab15d22410ed54816107f3e5a2ed6f78e4d1d8f69181fe`, tokens
`79294511da18b463bd20056aa70cda914d488c43d6ccaabf0b398184b4fc1218`.
The qualification currently referenced by the button, for example, records different
identities (`577751dd…` / `95b95845…`). Shared renderer, token and build inputs are byte-identical
to the base. Logs are `preflight.log`, `clean-baseline-catalog.log` and
`clean-baseline-digests.json`. The existing shared UI owner must reconcile current qualification
through actual renderer evidence; editing hashes alone would not qualify it. This remains a
required repository/merge blocker, separate from the passing issue-scoped local behavior.

The separately executed repository Python suite initially reported 14 failures and 88 errors
because the default macOS `/var` temporary path crosses a symlink. With the documented
`TMPDIR=/private/tmp`, it ran 1,384 tests with **one failure and 20 skips**. The remaining
`RegistryContracts.test_release_builds_carry_no_observer_or_test_dependency` assertion expects
`serde_json` to be enabled only by `service`, while the current Identity manifest also enables
it through `recovery-testing`. The exact test fails identically in the clean base. This is a
second pre-existing repository blocker, not a passing full-suite claim; no Identity manifest
or test expectation is changed here. Logs are `python-tests-private-tmp.log` and
`clean-baseline-python-failure.log`.

Drivers, build/test logs, reports, capture hashes and original PNGs are retained under ignored
`artifacts/agent-runs/issue-722/`. Commands were `node .../browser.cjs --baseline`,
`node .../browser.cjs`, and `node .../real-browser.cjs`; the full relative directory above
replaces `...`. The real fixture was built with `cargo xtask build cantos-identity` and a
temporary `cargo test --locked -p cantos-server --test issue722_browser_host --no-run` host,
then run against that prebuilt Identity executable. The build used two jobs and debug symbols
disabled; task-owned incremental caches were reclaimed to stay within local disk capacity.

Opened capture names include `baseline-unread`, `baseline-manual-loaded`,
`content-auto-loaded`, `content-valid-empty`, `content-failed`, `content-retry-loading`,
`session-pending`, `content-login-return`, `content-dark-narrow`, `content-dark-wide`,
`content-old-session-rejected`, `real-login-return-empty`, `real-direct-reload-auto-list`,
`real-cold-direct-list`, `real-read-failure` and `real-retry-real-api` (all `.png`). Status and
rows were legible, failure/empty differed and the choice hint appeared only with ready rows.
Captures are evidence-local-only; no Drive upload or public screenshot hosting was performed.

An initial browser-driver attempt timed out because navigating to the identical hash did not
reload the app. The driver was corrected to use `page.reload`; the completed report is PASS.
A test-authoring compile error compared opaque `RequestState` values directly; its oracle now
asserts the public phase/actor/target/result and the final focused suite passes.

Not run: deployed production acceptance, Firefox/WebKit, screen reader, OS IME, 200% text,
full keyboard/touch tour or playback. No production mutation, merge or deployment occurred.
These limits do not claim acceptance outside the four issue-scoped catalog criteria.

## Review follow-up: one announcer per catalog read

A read-only review of PR #724 at `b47ce453` found that direct Content showed the catalog
state twice. Observed in the real Studio CSR with an in-page synthetic `/api/*` stub
(Trunk 0.21.14, built-in Chromium pane, vi-VN): an automatic empty read showed the generic
request banner (`Đã nhận xác nhận từ server.` / `Trang này chưa có nội dung.`) above the new
`Trang này chưa có tác phẩm` block; a 503 and a pending read likewise produced two polite
status regions with overlapping text. The desk was unaffected because its banner is hidden.

`RequestBanner::of` is a pure projection of `RequestState`: idle is hidden; a `Works` read
keeps only the banner's existing cancel/reconcile/retry actions while one is available and is
otherwise hidden; every other request keeps the full banner. Cancelling a list read focuses
list refresh (`live-works`) instead of the now-hidden banner. The successful-empty catalog
block gained the same `role="status"`/`aria-live="polite"` as the other catalog states, so
the empty outcome remains announced once.

Native: `request_banner_defers_list_read_state_to_catalog_status_but_keeps_its_actions`
covers idle, pending, empty, failed and cancelled list reads and a non-list paged read; the
locale status test now also asserts that every non-row catalog state renders a status region.
`cargo test -p cantos-studio --locked`: 481 unit tests + 5 doctests PASS; native and WASM
clippy (`-D warnings`) and `cargo fmt --all -- --check` PASS.

Browser, synthetic API (not Server/Identity), served wasm
`98d56fee1024e6cb8f1fe13e78b241d3fd14cb1fbba069db0bf8794f3a2c5494`; the committed sources
differ from the served build only by `rustfmt` whitespace. Observed: cold direct Content issues
exactly one session read and one `works?limit=20` read; empty and failed reads show one visible
status region and no banner; pending shows `Đang đọc tác phẩm…` plus only the cancel action;
cancel moves focus to `live-works` and shows the failed state; explicit refresh lists two rows
with the choose-Work hint; opening a Work still shows the full banner with its failure detail;
desk → Content navigation issues no further list read. Not run in this follow-up: narrow
viewport, English locale, screen reader, real Server/Identity and other browsers.

## Merge-readiness after develop moved

The two repository blockers recorded above were resolved on `develop` by PR #723
(`919852be`: renderer requalification and the Identity dependency assertion). After merging
`origin/develop@1b885b7c` into this branch,
`cargo xtask preflight --changed-only --base origin/develop` reported **PASS** for
`repository`, `rust-format`, `rust-lint`, `rust-tests`, `web-lint` and `web-build`
(`cantos-studio`), with no registered assurance obligation. The run used `TMPDIR=/private/tmp`
and a scratch `PATH` directory whose `python3` and `bash` link to Homebrew Python 3.14 and
Bash 5, because macOS Python 3.9 lacks `tomllib` and Bash 3.2 cannot run the VPS shell
fixtures; under the same shims the scripts suite ran 1,384 tests, OK (16 skipped). Remote CI
was not consulted for this local merge decision.
