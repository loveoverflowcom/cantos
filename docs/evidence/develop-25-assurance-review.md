# Develop 25-commit assurance review — 15 October 2026

## Pinned scope and counting convention

Develop was fetched successfully on this host and pinned to
`e312ce7b2f036913351819608103e6206f464645`, tree
`38d34ac8a615b92498c9ddd27f8645918219a635`.
The window below is exactly `git log -25 --format='%H %s' <baseline>` in Git's
normal graph/date ordering: merges count, it is not 25 first-parent or 25 non-merge commits.
For changed-path inventory, each selected commit was compared to its first parent; the union
contains 205 paths. This is a risk-directed source review, not an assertion that every line
of those paths or every earlier ancestor was independently audited. The two requested review
sessions share one branch and one Draft PR; session 2 must append, not create a second PR.

```text
e312ce7b2f036913351819608103e6206f464645 Merge pull request #173 from loveoverflowcom/docs/i18n-skill-runtime-alignment-20261015
ce9b1ad76bdf8ed7f9c4d99a87fb6e752179134f Merge pull request #192 from loveoverflowcom/fix/membership-contact-privacy
1bf42c615312288b2bb3ebe48f13d8faf35bcb14 Merge origin/develop into fix/membership-contact-privacy
4ccca7ff1d3c52451c80325ae49c4039125e253e fix: redact Membership contact diagnostics
34a3151b0adc05b779087a4032f2ac5dc82ee91a Merge pull request #189 from loveoverflowcom/feat/theatre-player-scrub
b5cdaef490c92fa84334474c083a7c45387b7543 docs(i18n): reconcile copy owners with latest develop
b55d64e629b11acf7317c8157bf41f45630e829d Merge latest develop into docs/i18n-skill-runtime-alignment-20261015
16709b0cfb284e4d9e0f63cc073b59559998b429 feat(theatre): preview scrubs and preserve pending seeks
e998fd974bd1a671fca3c811949597023dfbf549 Merge pull request #188 from loveoverflowcom/feat/firebase-demo-apk-distribution
4663efbaa13e05179865bb09d423c3279671ded8 feat(xtask): add opt-in Firebase distribution of the Theatre demo APK
87b49a329ded9ce476d801bc86b17a4f3a41e7df Merge pull request #187 from loveoverflowcom/feat/edge-duckdns-cantoss
12d4f3e660a7dfec0a3153c948f6745702667d89 feat(services): route Theatre, Studio and API hosts through the edge
00225e9c7db9c35f967f46933ec145a010097093 docs(services): add DuckDNS and Caddy edge for cantoss.duckdns.org
c6353c77b59161b2600d83c330db9c01c334e0ec Merge pull request #186 from loveoverflowcom/fix/theatre-full-stack-local-run
7666e998c4613916decdf082eb296e2f074db38a fix(xtask): start and check Theatre API for local web runs
fbf4879e9261f6bc1fe2e423a2c5aabdeac4a627 Merge pull request #185 from loveoverflowcom/feat/studio-private-artwork
4b6e63dede3fa6acd243477f25386bdfffd0ad3e feat(studio-web): manage private Work artwork
6f3e582bf93626014666da8e10891e102931df44 Merge pull request #182 from loveoverflowcom/docs/issue176-plan-publication-proposal
6da77f8ccab170c213ddba7485687852aa395737 docs: propose Membership plan publication decision matrix
6818642a7c62cb35c9da04ed6d7ac2daeb1008b1 Merge pull request #165 from loveoverflowcom/refactor/cmp-preview-sandbox-isolation
e4cf33c07d8ac3987df159c301791f2735a51c7e test: pin cache expectations for PostgreSQL socket recovery
5f55a18587a3ab5ada407135706b1471a0de71c7 docs(i18n): align references/cmp-design-system.md with current catalog and open library choice
c42a9a342796ccc23dd8efe1963ffb81b72e22da docs(i18n): align cantos-cmp-mobile/SKILL.md with current catalog and open library choice
3087a6f088b5d700e6ecc575f902da05415b13b3 docs(i18n): align cantos-leptos-web/SKILL.md with current catalog and open library choice
98a585c9fda79af322e0d8074bd20b0a0f617eb6 docs(i18n): align cantos-ui-design/SKILL.md with current catalog and open library choice
```

Session 1's isolated branch is `review/develop-25-quint-verification-20261015`.
Existing worktrees, local refs, unpublished history and source were not moved or discarded.
Cleanup removed only four inactive, untracked Python bytecode cache directories: 94 `.pyc`
files, 2,669,983 file bytes. No Cantos `target` directory existed before this review. Git,
source, private inputs, evidence, toolchains and non-Cantos caches were preserved.

## Authority and assurance ledger

| Claim / owner | Review disposition and cheapest useful oracle |
| --- | --- |
| Quint stateful expectation ownership: [catalog](../../specs/behavior/catalog.json), [inventory](../../specs/behavior/inventory.md), [ADR 0027](../decisions/0027-quint-executable-behavior.md) | Only private-audio-admission has a registered model; it remains **draft**. Publication is unmodeled/draft and progress needs-decision. No model/lifecycle promotion or new mirrored reducer. |
| Private-admission observer: [protocol](../../specs/behavior/observation-protocol.md) | Source audit confirms events alone enter the real Rust decision/constructor seam; expectations do not enter it. Original observations, mutant sensitivity, observer-fault sensitivity, receipt immutability, call counts and source/tool freshness remain separate. Python codec/runner tests are not a Quint or Rust observer run. |
| Local Firebase notes: [distribution guide](../../apps/cantos-theatre/mobile/docs/firebase-distribution.md) | Introduced P2 resource defect: `std::fs::read` consumed the entire file before the existing 16 KiB validation, even in dry-run. Replaced by regular-descriptor validation and a 16 KiB+1 bounded read. Unix nonblocking open prevents a FIFO open from waiting before rejection. Symlinks to regular files stay usable. Native boundary/no-tool tests added, not executed here. A bounded Unix child-process FIFO regression guards the nonblocking-open claim separately. |
| Theatre retained-fixture launch: [local run evidence](theatre-local-run.md) | Added real `serve_studio.main` orchestration tests with filesystem/environment/chdir/exec doubles: exact Theatre DB/origin, no editorial script/token lookup, preserved editorial output, and invalid fixture rejection before chdir/exec. These are mocked-shell results, not PostgreSQL/API startup. |
| Listener work ordering: [Discovery v1](../../packages/contracts/theatre-discovery-v1.md#opaque-keyset-pagination) | Existing production ordering is correct. The prior Kani irreflexivity/asymmetry assertions also admit both directions reversed, an assurance opportunity rather than a production defect. Added independent `(Reverse(time), id)` exact-key oracle and result-sensitive older/tie/equal covers; no assumptions, stubs or proof lane added. Full-width extrema tests added. New source is **NOT_RUN**, not bounded-model-checked. |
| Theatre scrub/player, Studio artwork, edge and CMP sandbox | Reviewed selected changed boundaries and existing tests. Studio defects and edge risk below remain for independent session-2 review. Existing native/mock coverage is not browser/device/release-isolation acceptance. Membership and localization proposals remain proposals; no library, commercial policy or persistent locale choice is accepted. |

No schema, migration, wire contract, authorization decision, publication gate, production
player or backend generation behavior changed in session 1. No Firebase call or deployment ran.

## Session 1 local evidence

The substantive source inputs are the final files under `scripts/test_postgres_runner.py`,
`xtask/src/distribute/{host,mod,request}.rs` and
`services/cantos-api/src/contexts/media/domain/listening.rs` on the pinned baseline.
Commit/tree identity is the final PR branch; packaging the same bytes invalidates no test result.

| Check | Actual result / evidence scope |
| --- | --- |
| `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts -p test_postgres_runner.py` | **PASS**, 21 tests, including the three new production-main tests. `mock-integration-tested` only for the retained-fixture Python shell with named OS/filesystem/exec doubles. |
| `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts -p 'test_verify_behavior*.py'` | **PASS**, 36 Python contract tests. Expected injected FAIL diagnostics are negative-control fixtures, not a real failed or passed model/observer campaign. |
| `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts -p 'test_*.py'` | **PASS**, 466 discovered, 461 executed, 5 skipped. All skips are `test_visual_compare.PngAdapterTests`: optional pinned Pillow PNG adapter absent. No package installed. Mock Kani logs in this suite are parser/runner fixtures, not verifier execution. |
| `PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_repository.py` | **PASS**, 735 text files and 19 skills; local links, JSON syntax, skill structure and API/Theatre architecture hygiene only. This is an independent primitive, not a code-change preflight fallback. |
| `git diff --check` and staged whitespace check | **PASS** at handoff. Whitespace only, not Rust formatting. |
| `cargo xtask preflight --changed-only --base e312ce7b2f036913351819608103e6206f464645` | **BLOCKED**, shell exit 127 (`cargo: command not found`), no xtask summary was produced. All selected gates **NOT_RUN**, including Rust formatting/Clippy/compilation. Python suite results are independent diagnostics, not a code-change preflight fallback. |
| `cargo xtask verify kani --harness listener-access` | **BLOCKED**, shell exit 127; assertions, unwinding and all five changed covers **NOT_RUN**. Historical proof records do not cover the changed harness. |
| Actual Quint/model/conformance | **NOT_RUN**: no Cargo, no installed locked Quint dependency. Tooling tests do not substitute. |
| DB/API/storage/browser/device/Caddy/Firebase | **NOT_RUN**, unavailable or outside this bounded local test scope. |
| GitHub CI | **Not checked**, as requested. No polling, dispatch or rerun. |

The final Python-suite rerun completed on the unchanged substantive source in 24.344 seconds:
466 discovered, 461 executed, five optional Pillow skips. Its complete local output is retained
in the ignored `artifacts/agent-runs/review25-recovery/python-tests.log`; injected model/Kani
fixture messages in that log are test data, never live verifier evidence. Independent source
review found no concrete defect in the bounded-read patch or exact ordering oracle. It identified
a missing FIFO-specific regression; a bounded Unix child-process test was added to detect
blocking without hanging the test runner. That test, Rust compile/format and actual Kani
acceptance remain pending.

L1 architecture/authority review is source-inspected; L2 production ordering is unchanged;
L3 Python tests passed but changed Rust/native/proof checks are unrun; L4 Python launcher
mocked orchestration passed; L5 real IO/runtime acceptance is unrun. Do not label the whole
change CORE_VERIFIED or production-ready. No failing production test was hidden as a blocker.

## Session-1 handoff findings for sequential session 2

Confirm each independently before editing. These are source-backed findings, not executed
browser/API reproductions or accepted new product decisions.

1. **P2 Studio ambiguous artwork upload cannot recover from exact expiry.**
   `live/command.rs` UploadImage reconciliation requires `Some(asset)`; an exact matching
   `Expired` intent with `asset=None` therefore stays ambiguous in `live/state.rs` and locks
   prepare/discard plus the wider workspace. The image domain explicitly projects Admitted
   before Expired if a settled asset exists. Distinguish correlated terminal expiry from
   successful admission; keep Pending/no asset, foreign intent and foreign owner unresolved.
   Add native regressions for expired-none, pending-none, foreign/changed binding and an
   admitted receipt observed after expiry. Preserve local bytes and never fabricate success.
   Backend Work locking/rechecks exist, but `owner_work_now` uses `clock_timestamp()`; do not
   claim a global monotonic-clock proof.
2. **P2 invalid artwork replacement guard is cleared by Refresh.**
   `artwork/panel.rs` retains valid local A when replacement B fails File/Dimensions/Decode;
   `Projection::apply` then unconditionally clears `problem` on a successful server reply.
   Refresh can re-enable A despite the guide's explicit invalid-selection guard. Preserve
   file-bound validation errors through unrelated replies; clear only after valid replacement
   or explicit discard. Test all three error classes, with/without a prepared intent.
3. **P2 private artwork Blob URL retirement is lost.**
   `Projection::apply(Reply::Artwork)` clears `current_url` before the preview effect can
   retrieve/revoke the previous URL. Refresh/select/clear can leak retained private Blob bytes.
   Keep URL retirement owned by the imperative preview lifecycle. Add deterministic lifecycle
   coverage and, if an available browser permits, create/revoke instrumentation across repeated
   refresh, replace, clear and disposal. Native assertions alone are not browser evidence.
4. **Edge immutable-SPA fallback risk, runtime unconfirmed.**
   `services/edge/Caddyfile` matches immutable headers on requested hash paths before
   `try_files` may fall back to `index.html`. A missing retired `.js`/`.wasm` might return a
   year-immutable HTML shell, especially after `rsync --delete`. Confirm with real Caddy if
   available; otherwise keep probable source risk explicit. Check both apps, real/missing
   assets, shell and client routes before adopting a repair.
5. **Proportional Theatre mock additions, not defects.**
   Existing host tests already cover 1000 preview inputs, generation/position acknowledgment,
   duplicate commit, rapid second seeks, cancel/source change and route disposal. If useful,
   add real TimelineControl invalidation matrix and preparing-engine seek acknowledgment
   tests. Browser ordering/focus/resize/reader observations remain their own deferred evidence.

## Session-1 handoff instructions (historical)

Continue the same isolated branch and Draft PR. Fetch its remote ref and inspect HEAD/tree
before adding commits. Fast-forward only; never force-push, reset/stash other work or create a
second PR. Keep the baseline/window above frozen even if develop advances; record any later
normal integration separately and invalidate only affected evidence.

First independently review session-1 diffs, especially Rust compilation/formatting, notes
reader limits/nonregular behavior and the strengthened Kani oracle. Then choose the highest-value
bounded findings above. Add tests against real production transitions, not copied reference
reducers. Do not expand the draft audio Quint capability to artwork/player or silently accept
Membership/localization proposals. Add no decorative Kani harness or new toolchain/dependency.

When the documented tools are genuinely available, run the affected focused native tests and:

```sh
cargo xtask verify rust
cargo xtask verify contracts
cargo xtask verify kani --harness listener-access
cargo xtask preflight --changed-only --base e312ce7b2f036913351819608103e6206f464645
# Only with the already installed exact locked behavior tool:
cargo xtask verify behavior --conformance --capability cantos.media.private-audio-admission
```

No auto-install, paid provider, infrastructure startup or remote CI is authorized as a substitute.
Unavailable gates stay explicit with prerequisite, exact command and acceptance criteria: Rust
format/compile/tests must pass, Kani must select the exact harness and satisfy assertions/unwind
and all five covers, and model/observer must report independent fresh PASS before those claims.
Next acceptance owner is **unassigned — assignment required before acceptance**; session 2 is
responsible for the next implementation/review pass, not a fabricated acceptance grant.

## Session 2 implementation and independent review

Session 2 began only after session 1 completed. Explicit remote fetches confirmed the same
Draft PR head `63feaba6ab535027fa0e5c2ecb3b27265e81da3a` and unchanged develop baseline above.
The 25-commit review window was not enlarged. An independent read-only review of all nine
session-1 changed files, their callers and tests found no remaining concrete defect or obvious
Rust compilation hazard in the bounded notes reader, FIFO test, launcher doubles or ordering
oracle. This is source inspection, not a substitute for compilation or proof execution.

The session-1 findings were independently traced through the production transitions before
the following bounded changes. New native regression source was written before the Studio
repair; attempting its focused test returned exit 127 because Cargo is absent, so no failing
or passing native execution is claimed.

| Invariant / actual repair | Regression source and evidence boundary |
| --- | --- |
| A fully correlated server `Expired` intent with no asset settles an unknown artwork PUT as expiry, never admission. Pending/no-asset still does not settle the write. | Real `Command::reconciles` → `RequestState::finish` → `Projection::apply` tests assert unlock, preserved local bytes, no fabricated asset, exact pending/foreign/changed binding rejection and authoritative admitted receipts. Independent review also found expiry → Refresh lost terminal knowledge in transient Stage; a separate server `intent_state` now drives shared prepare/upload predicates, with refresh → newly identified prepare coverage. The server's admitted-before-expired projection and Work read/finalize locking/rechecks were source-inspected; no new backend rule or global monotonic-clock claim. Native tests **NOT_RUN**. |
| A failed local replacement stays file-invalid through unrelated request starts, server replies and form errors. Only successful local replacement or explicit local reset/discard clears that guard. | File/Dimensions/Decode × prepared/unprepared cases call real projection methods; replacement/discard tests assert old bytes returned for retirement and old intent/asset invalidated. Production panel uses those methods, including successful selection's local reset. Native tests **NOT_RUN**. |
| A server artwork reply never destroys the old private Blob URL before the imperative preview lifecycle can retire it. | Projection retains the handle until `take_current_preview`; the production preview effect takes and revokes it. Native source asserts handle preservation/take across repeated unchanged refresh, confirmed replacement, clear and reply-before-disposal, with one-time take. It does not execute `on_cleanup` or actual revoke. Browser create/revoke instrumentation, scheduling and heap behavior **NOT_RUN**. |
| Immutable edge caching classifies the rewritten, existing asset, never the original missing hashed request that falls back to HTML. | Shared SPA `route` runs `try_files`, default no-cache, existing hashed-file immutable override, then file serving. Four structural stdlib Python tests pass for ordering, existence, asset regex and both web hosts/API isolation. These are source/configuration regressions, not a Caddy parser, HTTP integration or MIME/cache observation. |
| Theatre control invalidation and preparing seeks retain the existing reducer's meaning. Production player code is unchanged. | Two native tests exercise real `TimelineControl`/context/session with a deferred MediaPort: eight invalidation causes before/after reconciliation; paused/play preparing seeks require exact generation+position acknowledgment, not readiness or stale/wrong/out-of-bounds positions. Native tests **NOT_RUN**. |

The edge cause follows Caddy's documented ordering, not a guessed server experiment:
[official SPA pattern](https://caddyserver.com/docs/caddyfile/patterns#single-page-apps-spas)
and [header behavior](https://caddyserver.com/docs/caddyfile/directives/header). Caddy is absent,
so adaptation, validation and HTTP acceptance remain unrun. The four configuration regressions
were checked against the original source: three failed before the repair. Seven bounded
in-memory textual negative controls also triggered those assertions; this is sensitivity of
the structural tests only, not mutation-tested Caddy behavior.

No new Kani lane, mirrored reducer, Quint model, dependency, schema, contract, backend
authorization, commercial/localization policy or publication behavior was introduced. The
private-audio-admission capability remains draft. Existing screenshots and historical proof
records do not cover these changed transitions.

### Session 2 local evidence

| Check | Actual result and scope |
| --- | --- |
| `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts -p test_edge_cache.py -v` | **PASS**, 4 structural configuration tests. Original source: 3 assertion failures, not a runtime reproduction. |
| `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts -p 'test_*.py'` | **PASS**, 470 discovered, 465 executed, 5 optional Pillow skips, 23.583s. Mock Kani/model fixture output is still parser/runner data, not live verifier evidence. |
| `PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_repository.py` | **PASS**, 736 text files, 19 skills; local links/JSON/skill/API-Theatre architecture hygiene only, not Rust correctness or a code-change preflight fallback. |
| `git diff --check` and staged whitespace | **PASS** at final handoff. Whitespace is not a Rust formatter or Caddy validation result. |
| `cargo test -p cantos-studio live::artwork::tests -- --nocapture` | **BLOCKED**, exit 127 (`cargo: command not found`). New native regressions were not executed. |
| `cargo xtask verify rust`, `cargo xtask verify contracts`, `cargo xtask verify web` | **BLOCKED**, each exit 127; compile, format, Clippy, contracts, host native tests and Web/WASM build **NOT_RUN**. |
| `cargo xtask verify kani --harness listener-access` | **BLOCKED**, exit 127. Exact changed harness, assertions, unwinding and all five covers **NOT_RUN**. |
| `cargo xtask preflight --changed-only --base e312ce7b2f036913351819608103e6206f464645` | **BLOCKED**, exit 127, no xtask summary. No selected gate is represented as passed by the independent Python primitives. |
| Actual Quint/conformance | **NOT_RUN**, Cargo and existing exact locked behavior tool unavailable. No auto-install or lifecycle promotion. |
| Real DB/API/storage/browser/device/Caddy/Firebase | **NOT_RUN**. No infrastructure started, distribution or deployment. |
| GitHub CI | **Not checked**, as explicitly requested. No polling, dispatch or rerun. |

Complete Python output and unavailable-command diagnostics are retained only in the ignored
`artifacts/agent-runs/review25-session2/` directory; they are not committed source. Final repository
hygiene and whitespace results are recorded in the PR handoff. No toolchain/package was installed.

### Current acceptance checklist

Next acceptance owner: **unassigned — assignment required before acceptance**.
Implementation/source review is delivered in the same Draft PR, not a clean production acceptance.

- [ ] With the existing pinned Rust 1.98.1 toolchain, run `cargo xtask verify rust`,
  `cargo xtask verify contracts`, `cargo xtask verify web` and final changed-only preflight
  against the frozen base above. Require actual PASS on final source, all new native tests,
  formatting and Clippy; repair any discovered compile/format/test failure before acceptance.
- [ ] Run the exact `listener-access` Kani harness and require selected assertions/unwinding
  plus all five reachable covers. Historical output is not evidence for the changed harness.
- [ ] Only with the already installed exact locked behavior tool, run
  `cargo xtask verify behavior --conformance --capability cantos.media.private-audio-admission`.
  Require independent fresh model/observer results; keep capability lifecycle draft.
- [ ] In a built Studio browser with authorized HTTP doubles, drive lost PUT → expired/none
  reconciliation, pending and foreign receipts, invalid B after valid A → repeated Refresh,
  valid replacement/discard, and admitted-after-expiry observation. Assert controls, local
  fields and no fabricated success. Instrument Blob create/revoke across repeated unchanged
  refresh, replacement, clear, session/context switch, stale completion and disposal.
- [ ] With real Caddy, adapt/validate the config and inspect both web hosts: existing hashed
  JS/WASM/CSS immutable, missing hashed paths returning no-cache HTML (or the existing
  configured error), normal shell/client routes no-cache, correct MIME and unchanged API
  passthrough. Missing shell/error responses must not become year-immutable.
- [ ] Keep Theatre browser-native range event ordering, keyboard/focus, route disposal,
  responsive/reduced-motion and screen-reader observations separate from host mock assertions.

L1 source review and available Python configuration/mocked-shell tests passed their named
scope. Changed Rust L2/L3 execution and Web L4/L5 runtime acceptance remain unverified. Do not
label this combined PR CORE_VERIFIED, production-ready or fully Kani/Quint-verified. No merge,
close, force-push, remote CI or deployment is part of this review.
