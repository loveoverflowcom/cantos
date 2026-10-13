# Theatre Web library MVP — mock review boundary

Part of [#35](https://github.com/loveoverflowcom/cantos/issues/35). The user selected a Web mock
PR first, CMP later, with real-data acceptance explicitly deferred. Baseline: fetched
`origin/develop` at `64979f3700f76ea0f47b1afd70f3d26d6eb6c98b`; branch
`codex/theatre-web-library-mvp`. Evidence below covers that baseline plus this PR's Web diff.

## Review boundary and prerequisites

**Khám phá** and **Của tôi** are the only primary navigation destinations. Search is inside
Khám phá; the avatar opens Account. Của tôi presents **Nghe tiếp**, **Đã lưu** and **Lịch sử**
together, without sub-tabs. All-history is a secondary hash route. The existing root-owned
mini-player survives navigation. Point bookmarks remain secondary tools in the full player.

This is an authorized prototype before publication, listener identity/API and real playback
prerequisites are accepted. It completes no production criterion of #35,
[040](../work-plan/040-theatre-web-listening.md) or #12–#17. Broader identity/retention/guest
policy remains with [#34](https://github.com/loveoverflowcom/cantos/issues/34).
No ADR is needed: the change retains the existing stack, pure app-local core, ports and synthetic
session lifetime; it selects no backend schema, endpoint or synchronization policy.

The default live capability boundary is unchanged and constructs no synthetic catalog/player.
Only `?mode=mock` activates this implementation. Reload discards local facts and restores the
explicit demo seed. There is no audio, account, disk persistence or synchronization.

## Invariants, owners and evidence

| Claim / invariant | Failure mode | Owner / oracle | Evidence held |
| --- | --- | --- | --- |
| Exactly two main destinations; search and avatar remain reachable | Extra primary tabs recreate rejected UX | Routes/root navigation and feature composition | Native rendered-string tests; live DOM and driven Chrome navigation |
| Three library sections are visible directly, including useful empty states | Content hidden behind a sub-tab or empty dead end | Library presentation | Five feature tests; populated/empty DOM and opened captures |
| Desired Save/Unsave is idempotent and independent of positions/bookmarks | Retry duplicates records or unsave destroys progress | Pure library reducer and context | Exact state tests; card Save → detail Unsave state; unsave keeps 00:15 progress |
| History admits committed simulated starts only, newest work first | Page visit, seek, duplicate or stale callback fabricates listening | Pure `HistoryState` + existing player transition | Core/context tests; source opening excluded, Play adds item in browser |
| History stores identity; positions have one owner | A second progress store drifts | History projection + `ProgressState::recent/find` | Source review and exact per-work/episode projection tests |
| Clearing history preserves progress, saved works, bookmarks, player and queue | A destructive action clears unrelated facts | Targeted context command, separate UI confirmation | Exact context regression; browser cancel/confirm, 00:15 position/player retained |
| Reopen uses current progress and remains paused | Stale resume navigates or silently starts playback | Guarded context command + existing player restore | Unavailable/invalid/stale regression tests; browser restored 00:15 paused |
| Only explicit mock mode exposes samples | Mock success replaces absent listener API | Unchanged main/live mode boundary | Source review, existing mode tests and default-route browser observation |

L1 architecture review found no actionable findings: core imports no Leptos/browser/I/O,
application observes committed transitions, and features consume root capability handles.
No dependency, crate, shared UI component, backend or CMP source changed. Local facts are
immutable reducer values; settled backend records are untouched. Closed `Route` mapping keeps
secondary Work/History views associated with their primary destination.

## Verification commands and layer gates

All commands use `RUSTUP_TOOLCHAIN=1.87.0`; Python-heavy xtask runs additionally use
`TMPDIR=/private/tmp` and final repository/preflight use `PATH=/opt/homebrew/bin:$PATH`. Full logs are local-only under
`artifacts/agent-runs/theatre-web-library-mvp/`.

- L3 core first: eight framework-free listening tests passed before context wiring.
- `cargo test -p cantos-theatre --locked`: 75 passed, zero failures.
- `cargo clippy -p cantos-theatre --all-targets --locked -- -D warnings`: PASS.
- Final workspace format check: PASS. Initial format failure in two touched files was corrected.
- `cargo xtask verify rust`: format/lint PASS; the parallel workspace test run failed in unchanged
  `xtask::probe::tests::port_ownership_reflects_an_existing_listener` at the assertion after
  dropping and rebinding an ephemeral port. All 75 Theatre tests passed. The focused port test
  passed when run alone. This is an intermittent port-test observation; no Web regression or
  baseline failure is inferred solely from it. Original failure remains recorded in `rust-final.log`.
- `cargo xtask verify contracts`: `audio-inspection` UNSUPPORTED (ffmpeg/ffprobe absent);
  `script-ir-reference`, `contract-tests`, `wav-reference` NOT_RUN because suite prerequisites
  are checked together. No contract or media-inspection source changed.

- `RUST_TEST_THREADS=1 cargo xtask verify rust`: rust-format, rust-lint (workspace) and
  rust-tests (workspace) PASS; every test retained. Serial success is consistent with shared-port
  contention in the parallel harness, an inference rather than a proven root cause.
- `cargo xtask verify web`: web-lint (Studio/Theatre), both web-build gates and ui-preview PASS.
- First repository run used Xcode Python 3.9 and failed on missing `tomllib`/dependency-alias
  inspection. The installed Homebrew Python 3.14.8 is selected by
  `PATH=/opt/homebrew/bin:$PATH` for subsequent repository/preflight commands; no tool installed.
- `PATH=/opt/homebrew/bin:$PATH cargo xtask verify repository`: repository PASS; 225 Python
  tests passed, repository/text/link/API-boundary checks and whitespace checks passed.
- Final `cargo xtask preflight --changed-only` output is retained in `preflight.log` and the PR
  description. Selection is limited to repository, rust-format, Theatre lint/tests and Theatre
  WASM lint/build; no gate is skipped. Source and documentation are finalized before this run.

| Layer / scope | Status / evidence | Covered boundary / gap |
| --- | --- | --- |
| L1 Architecture / abstraction | PASS — documented independent source review | Inward dependencies, explicit mock activation, one root player |
| L2 Types / core rules | PASS — type-enforced route match, example-tested states | Desired saves, per-work identity history, current position lookup, rejected restore |
| L3 Unit / static checks | PASS — Theatre 75 tests/native lint/format; serial workspace suite PASS | Earlier parallel port-test failure retained above |
| L4 Application / orchestration | PASS — mock-integration-tested with `MockCatalog`/`FakeMedia` | History admission, isolation, stale callback and paused resume guards |
| L5 Web runtime | PASS for named mock flows — DOM-tested, interaction-tested | Chrome only; real DB/API/media are outside this slice |
| L5 DB/API/storage | NOT_RUN — outside mock review boundary | No persistence, migrations, HTTP handler or listener contract changed |

## Browser and UI evidence

Serve command: `cargo xtask run cantos-theatre --web`; installed Trunk 0.21.14, Leptos CSR/WASM.
Chrome **155.0.8059.40** on macOS; the agent reviewed the opened pixels. Source is the baseline
above plus the final Web implementation. Artifacts are local-only in
`artifacts/ui-evidence/theatre-web-library-mvp/`; `observations.json` records measured dimensions.
Viewport filenames from requested browser overrides are not authoritative: the DOM measurements
below describe actual CSS widths. Full-page captures place the fixed player at the capture
viewport edge; keyboard observations separately verify focused controls remain reachable.

| Capture / state | Actual viewport / theme / locale | Observation |
| --- | --- | --- |
| `discover-desktop-dark.jpg` | 1642×975 CSS px; system dark; vi-VN | Two main links, avatar, Continue and embedded search/shelf; typographic cover fallback |
| `work-saved-desktop-dark.jpg` | 1642×975; dark; vi-VN | Saved state matches discovery card; current mini-player preserved |
| `library-populated-1440-dark.jpg` | Requested 1440×1000 override; actual width not measured for this capture; dark; vi-VN | Three direct sections and recent items; no sub-tabs |
| `history-confirm-320-dark.jpg` | Measured 381×1000; dark; vi-VN | Inline confirmation, explicit retained-data copy and focus ring |
| `history-confirm-focused-narrow-dark.jpg` | 381×1000; dark; vi-VN | Confirm focused at bottom 387.9, mini-player starts at 840.0 CSS px |
| `library-narrow-light.jpg` | 825×1500; light; vi-VN | Light palette and wrapped direct sections |
| `library-small-light.jpg`, `library-empty-301-light.jpg` | 301×800; light; vi-VN | Narrow reflow and three empty CTAs; document scroll width equals client width |
| `library-final-dark.jpg` | 1642×975; system dark; vi-VN | Final direct sections, Saved and simulated history with root mini-player |

Driven flow: search Đèn khuya → Save → detail (Unsave label) → open Ngọn đèn đầu ngõ;
opening the episode did not add history. Explicit simulated Play added it; skip 15 seconds →
Pause → Của tôi retained 00:15. History sorted the new work first. Keyboard Enter opened clear;
Tab reached Cancel with a solid focus outline; Cancel restored the clear trigger and both rows.
Confirm then cleared only history, focused main and retained 00:15/20:32 player and Continue.
Unsave removed the work while its Continue entry remained. Reopen restored 00:15 paused.
Avatar exposed disabled Sign in/Sync with reasons. Broad demo clear produced three separate
empty states while retaining the player. English headings/empty states reflowed at 301 CSS px.
The default live URL showed no mock library/player. No app console error was identified in the
inspected log subset; the returned warnings belonged to an installed browser extension.

Surface intensity: discovery/library Medium, full player Medium-High, mini-player Medium-Low.
Existing canonical v0.2.0 semantic colors/spacing/shapes are consumed; no token output or CMP
mapping changed. Local CSS geometry handles grouping and wrapping. vi-VN/en typed resources
cover every new visible/accessibility string; existing resource parity tests remain applicable.
Evidence: screenshot-inspected, cross-viewport-inspected, cross-theme-inspected for the listed
states; accessibility-checked for the named keyboard/focus confirmation flow only.
Screen reader: not walked. Motion/reduced-motion: not observed. 200% zoom/text scaling: attempted
shortcuts did not establish a changed zoom level, and native app control lacked permission;
no 200% acceptance claim. Firefox/Safari, touch and native devices were not exercised.

One observed copy finding was fixed and rechecked in the same Settings scenario: broad demo
clear help now includes history and explicitly retains player/queue, in both languages.
No image establishes real audio, account isolation, motion or assistive-technology behavior.

## Deferred acceptance and compatibility

| Boundary / status | Reason / prerequisites | Acceptance / next owner |
| --- | --- | --- |
| Full contracts suite: UNSUPPORTED / remaining gates NOT_RUN | Trusted installed ffmpeg and ffprobe | `cargo xtask verify contracts`; all gates PASS; media/verification owner unassigned |
| Parallel workspace port test: FAIL observation; serial PASS | Isolate competing port users before claiming the parallel harness reliable | `cargo xtask verify rust`; serial command uses `RUST_TEST_THREADS=1` and skips no tests; xtask owner unassigned |
| 200% scaling and screen reader: NOT_RUN | Browser zoom control and named reader available | Repeat changed routes/actions at 200%, then VoiceOver/other named reader; Web accessibility owner unassigned |
| Real account sync/history/progress: NOT_RUN | Reviewed listener identity/API/durable store; #12/#34 | After implementation, real account A→logout→B, reload, second-session and conflict acceptance under 040; #12/#35 owners |
| Published playback and immutable release resume: NOT_RUN | Publication/delivery and real Web media adapter; #11/#13 | Real discovery→playback→reload/resume review in 040; publication/Web owners |
| CMP: NOT_RUN | Explicitly deferred by the user | Separate 050 implementation and Android/iOS acceptance; CMP owner unassigned |

No migrations, Script IR/API/schema/vector changes, cache/audio invalidation, provider calls/costs,
rights or publication-gate changes. No production work check was run: this mock makes no listener
HTTP request or real production effect. Existing seven playback vectors remain unchanged and
run in the native Theatre suite; WASM vector harness, Kotlin and devices were not run here.
No progress conflict/rewind synchronization policy is selected; mock manual rewind tests do not
accept #12. Activation/recovery uses the existing explicit mock URL and reload/reset.

The app README and work queue link this presentation review step while preserving production
prerequisite order. Requested commit/push/PR actions publish this mock boundary only; issue
status, comments, labels and closure are unchanged. GitHub CI is not checked or awaited.
