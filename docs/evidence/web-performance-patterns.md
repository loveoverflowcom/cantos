# Web performance patterns — issue 146

This is the first bounded implementation slice of
[issue #146](https://github.com/loveoverflowcom/cantos/issues/146), based on `develop`
`26dee0a` (tree `587fb0f932a4620ca751db09d21e32ff05760ed8`). It adds reproducible release
asset inventory and reduces redundant reactive projection work in the isolated Theatre mock.
It does not complete the issue's browser, deployment or compatibility experiments.

## Invariants and review boundary

| Claim | Failure mode | Oracle / evidence boundary |
| --- | --- | --- |
| Player metadata changes with the episode, independently of position ticks | cover/title/link projection repeats on each position update | production memo projections, deterministic counters and exact transition assertions; native examples, not DOM timing |
| Each library row shares its selected position across consumers | repeated whole-progress clones/scans on each consumer read | borrowed, memoized projection; multi-consumer/other-episode regression; no account-data cache introduced |
| Measurements describe the freshly built release assets | stale dist or a hashed-looking name is mistaken for immutable deployment evidence | release build before inventory, per-file SHA256 and explicit classification; filename shape never establishes cache eligibility |
| Existing API and private-storage boundaries remain intact | caching a draft/token/media response or re-enabling unsafe static delivery | source review and inherited gates; no HTTP, database, authorization or media implementation changes |

L1–L2: UI projections stay in the Theatre shell and depend inward on existing immutable core
values. No domain transition, DTO, schema, provider, persistence or API contract changes.
L3–L4: native reactive tests exercise production projections; fixture playback is simulated.
L5: browser interaction is recorded separately below; no live media/provider/CDN result is inferred.
Existing `Cover`, `ProgressMeter`, player controls and root-owned `PlayerSession` are reused; no new
shared component, visual contract, styling or admission rule is introduced.
No ADR is needed: no dependency, rendering/build system, deployment topology or product rule is selected.

## Current topology and pattern audit

Both Web packages pin Leptos **0.7.8**, mount with `mount_to_body`, and use Trunk **0.21.14**
in CI. They are CSR. Native `ssr` dev-dependencies support tests, not an SSR HTTP host.
The release command uses the existing locked Trunk build; this slice changes no optimization profile.

`services/cantos-api/src/interfaces/http/mod.rs` mounts `origin_and_cache` on `/api/v1`;
it retains `no-store`, with private audio additionally `private, no-store, no-transform`.
Its optional `ServeDir(dist)` fallback is deliberately absent whenever private audio storage or
artwork is composed, because `ServeDir` follows nested symlinks. The local development Web commands
use Trunk. No production CDN/reverse-proxy/static-host configuration is established by this audit.

Trunk emits hashed-looking top-level JS/WASM/CSS filenames. Theatre's `copy-dir` also ships fonts,
icons and artwork at stable, unhashed URLs. Per-file SHA256 records identify observed bytes;
they do not prove that a deployment never overwrites a URL. A host must explicitly admit public,
content-addressed assets before setting `public, max-age=31536000, immutable`; HTML must revalidate,
and retained prior assets must support old sessions and rollback. MIME, compression, validators,
304 responses and old/new HTML-to-asset mappings still need tests on the actual host.

| Technique | Decision in this slice | Evidence / next condition |
| --- | --- | --- |
| Immutable HTTP asset cache | Defer activation; adopt explicit inventory/classification | deployment topology, admission and deploy/rollback header tests required; no filename-regex cache policy |
| Compiled WASM code cache | Retain generated streaming loader; defer cache-hit claims | inspect loader text and actual `application/wasm` host response, then engine trace; warm resource hit alone proves no V8 code-cache hit |
| Bundle budgets / preload / compression | Adopt reproducible byte inventory; defer numeric budget and host changes | fresh release output and offline raw/gzip/Brotli estimates; multiple browser baselines needed before threshold/profile decisions |
| SSR + hydration | Defer gated Theatre public-route spike | compare TTFB/FCP/LCP/usable interaction/hydration work, preserve root player/auth, record migration/deploy/rollback ADR before adoption |
| Islands | Defer until SSR path is demonstrated | CSR cannot acquire partial hydration from a drop-in flag; measure actual client bytes and interaction/focus correctness |
| Lazy loading / WASM split | Defer compatibility spike | validate pinned Leptos/Trunk support before borrowing newer cargo-leptos examples or changing build system |
| Protobuf / data caching | Retain #97/ADR0023 pilot and JSON default; defer expansion | existing codec owner, compressed payload/CPU/bundle comparison and auth-generation invalidation; no private persistent cache |
| Fine-grained signals | Adopt narrow stable player and shared borrowed progress projections | deterministic before/after work counts and transition regressions; no browser speedup claimed |

Studio audit: `Document` is already backed by `Arc<Value>`, so a blanket `.get()` → `.with()`
rewrite would misidentify its cost. `Document::acts()` materializes act/scene/dialogue values,
including text, repeatedly in `authoring_view.rs` navigation and selection closures. Profile a
large permitted Vietnamese document and IME/save/selection behavior before replacing this with
a narrower projection. This remains a recommendation, not an implemented editor optimization.
Existing request tickets, auth-generation fences and response `Content-Type` decoding are good
patterns to preserve. No API data cache, Service Worker, IndexedDB or cache-based authorization
is added.

## Reproduction and measurement limits

```sh
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask web-performance cantos-theatre
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask web-performance cantos-studio
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify repository
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify rust
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify contracts
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify web
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask preflight --changed-only
```

The optional inventory is a local build/measurement command, not a new required gate or a
browser benchmark. Its primitive and command-plan tests run in the existing repository/Rust
gates. Reports and raw outputs stay in ignored `artifacts/`; publish only reviewed aggregate
measurements and public asset identities. Offline compressed sizes are estimates under recorded
parameters, never actual wire-transfer evidence. HTML-linked bytes exclude CSS/runtime-selected
fonts/images and dynamic requests, so they are not a complete initial-network-byte total.
Build duration is not WASM compile/instantiate/app-ready latency.

Four distinct cache layers remain separate: immutable public resources; browser-owned compiled
WASM; explicitly eligible public API data; and private/session data under API authority.
Code cache, network cache and memoization do not establish each other's correctness.

## Measured results and verification

Measured on 2026-10-15, macOS 26.5.2 (25F84), arm64 Apple M4, Rust 1.98.1, Trunk 0.21.14,
Python 3.14.8. The [versioned byte record](web-performance-assets.json) holds the source-input
fingerprints, exact JS/WASM names/SHA256 and all asset-category totals. Theatre release output
was preserved before its production edits; Studio source remained unchanged throughout; their offline inventory does not reconstruct a source
snapshot. Current commands capture before-build source fingerprints and refuse drift. Measurements
are one deterministic artifact per app, not latency samples or p50/p95. Both generated loaders
contain `WebAssembly.instantiateStreaming`; execution, actual MIME and code-cache hits are unrun.
The effective Trunk-managed `wasm-opt` version was not established (none on PATH).

| Artifact / raw bytes | Baseline | Current | Delta |
| --- | ---: | ---: | ---: |
| Theatre WASM | 1,492,689 | 1,498,423 | +5,734 |
| Theatre JS glue | 35,871 | 35,871 | 0 |
| Theatre all 32 output files | 1,903,386 | 1,909,120 | +5,734 |
| Studio WASM | 4,712,353 | 4,712,353 | 0 |
| Studio JS glue | 42,961 | 42,961 | 0 |
| Studio all 6 output files | 4,825,123 | 4,825,123 | 0 |

Theatre all-output gzip is 756,064 → 757,320 bytes; Brotli is 604,393 → 604,177 bytes.
Studio remains gzip 1,441,222 / Brotli 924,574 bytes. Gzip uses level9/mtime0 and the installed
Brotli CLI quality11 (version in JSON). These are whole-file estimates, not delivered encodings.
The new memo boundary trades 5,734 raw WASM bytes for reduced downstream work; the small Brotli
difference is not a startup benefit. Theatre includes four CSS-referenced font files and 22 other
copied files. Hashed-looking URLs changed for Theatre JS/WASM and `index.html` changed under its
same URL; unchanged copy-dir bytes prove no future URL-immutability policy.

Native `cargo test -p cantos-theatre --locked -- --nocapture`: **132 tests + 3 doctests passed**.
Its paired demand-driven scenario retains the old dependency shape beside production memos:

| Synthetic work scenario | Old projection evaluations | Current | Exact correctness assertions |
| --- | ---: | ---: | --- |
| Metadata, 1,000 position updates | 1,001 | 1 | same episode, each position, source switch, stale identity |
| Five row consumer memos, 1,000 unrelated checkpoints / 1,000 stored records | 5,005 | 5 | unchanged watched position, forward/backward seek, end and clear |

These counts describe native consumer-memo demands, not actual browser DOM evaluation counts,
allocations, CPU time or playback. Separate tests cover speed/end/duration rebinding and idle state.

Browser observation used `cargo xtask app cantos-theatre --web --release` at
`http://127.0.0.1:8082/?mode=mock` in the Codex in-app browser. Browser engine/version and native
zoom are not exposed by this tool, so this is a bounded local interaction observation, not the
versioned Chromium performance matrix. No network/CPU throttle or cold-cache conditions were
established. The observed runtime used the same source through the existing release serve command; exact
served bytes were not separately hashed. Observations:

- Opened Người giữ tiếng chuông at 12:48 paused; range ArrowRight plus +15 seconds produced
  13:04; Play confirmed simulated running. Title/work/cover stayed consistent.
- Navigated to Của tôi: Continue and History progress names read “Đã nghe 13:04 trên 28:42”;
  visible time, resume-button names and root mini-player agreed at 13:04.
- Opened Đèn khuya bên sông → Ngọn đèn đầu ngõ: source/title/cover changed, position reset to
  0:00 / 20:32 paused. ArrowRight produced 0:01. Settings speed1.50 returned as 1,50×.
- Inspected tool-returned screenshots at 1440×900 dark Library and 320×800 dark/light Player.
  Vietnamese names remained legible and narrow controls stacked. Tab from the range focused
  Lùi 15 giây mô phỏng with a visible focus ring; route changes focused main.
- Console warning/error query returned no entries. Closed the temporary tab, reset viewport and
  stopped only the xtask-owned Trunk process. Screenshot files were not exported/attached;
  these observations are not a reusable visual baseline or screen-reader acceptance.

Evidence: `example-tested`, `compiled`, `DOM-tested`, `interaction-tested`,
`screenshot-inspected` for the named mock scope only. Text scaling/200% native zoom, real IME,
allocation/long-session browser profiling, disposal instrumentation, browser startup/latency,
cache-hit and production-header results remain **NOT_RUN**.

The first unconfigured Rust gate had formatter/lint PASS and workspace tests FAIL at three
unmodified private-storage tests: macOS `/var/folders/...` temporary roots traverse `/var`'s
symlink, refused by directory admission. The same API suite with `TMPDIR=/private/tmp` passed
478 tests / 0 failures / 68 existing ignored integration tests. The first repository gate FAIL
used system Python3.9 (missing `tomllib`/`os.waitid`); the installed Python3.14 selected through
PATH passed **440 Python tests, 5 existing skips**. No gate or source was weakened; both original
logs and the per-command environment diagnosis remain under `artifacts/agent-runs/issue-146/`.
Final `cargo xtask preflight --changed-only` selected all gates conservatively: **16 PASS,
1 FAIL (`postgres`)**; no gate was skipped. Mutation killed9 / survived0. The actual disposable
PostgreSQL17.11 run stopped at
`tests/support/private_artwork_postgres.rs:674`: the artwork upgrade test expects migrations1–25,
while `platform/postgres/migrations.rs` registers1–26, including existing `0026_catalog_delegation.sql`.
All556 tracked API/contracts/root-Cargo inputs are byte-identical to exact base `26dee0a`.
An independent source review confirmed the inherited stale test expectation. No clean-base DB
rerun occurred; source/blob identity establishes inheritance, not a reproduced baseline run.
This is an actual **FAIL**, not an environment gap or evidence of migration corruption. The
preceding snapshot/checksum/catalog assertions passed; this PR does not alter that fixture.
Later image-repository/storage, listener-library/audio-intake and host/process-kill/PG-restart
steps were not reached and remain **NOT_RUN**. The runner stopped its owned cluster.

```text
preflight summary
  PASS         repository
  PASS         rust-format
  PASS         script-ir-reference
  PASS         protobuf-reference
  PASS         rust-lint (workspace)
  PASS         rust-tests (workspace)
  PASS         dbsp
  PASS         wav-reference
  PASS         audio-inspection
  PASS         web-lint (cantos-studio, cantos-theatre)
  PASS         web-build (cantos-studio)
  PASS         web-build (cantos-theatre)
  PASS         ui-preview
  PASS         cmp-ui-preview
  PASS         theatre-cmp-demo
  PASS         mutation
  FAIL         postgres                                  `python3 scripts/test_postgres.py` exited with exit status: 1
preflight: FAILED: a required step did not pass
```

Local evidence delivers the named projection/tooling scope; it does not establish complete
DB/runtime or production acceptance. Original failures were not skipped or changed to PASS.

## Deferred acceptance

| Check / status | Prerequisite and reproduction | Acceptance / next owner |
| --- | --- | --- |
| Inherited PostgreSQL fixture failure — FAIL; remaining DB/host/restart steps — NOT_RUN | review migration26 against the fixed artwork-upgrade expectation, then `TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify postgres` with supported tools on PATH | unassigned; repair under the persistence owner without weakening registry/snapshot/checksum assertions; all applicable remaining suites must run |
| Cold/warm/hot startup, Web Vitals, long tasks, heap and p50/p95 — NOT_RUN | prepared isolated browser profiles; current release on actual selected host; execute issue #146 scenarios 1–3 with browser/version, CPU/network conditions and multiple runs | owner unassigned; compare same bytes/URLs, separate resource cache and compiled cache, retain raw traces |
| Static MIME/compression/immutable headers, 304, deploy/rollback — NOT_RUN | reviewed static host/CDN topology and public asset admission; run issue #146 scenario 4 against both revisions | owner unassigned; HTML revision correct, old assets available, no shared private response or cross-release mismatch |
| Studio large editor / Vietnamese OS IME / account switch — NOT_RUN | authorized local fixture/backend and named input method; existing `cargo xtask run cantos-studio --web --test-cluster` after disposable PostgreSQL preparation | owner unassigned; text/focus/draft/session fences preserved before editor optimization |
| SSR/Islands/lazy compatibility — NOT_RUN | isolated reviewed spike using exact pins; existing `cargo xtask build <app> --web --release` remains the baseline | owner unassigned; establish reproducible spike command through xtask, measured adoption decision and ADR before activation |
| Firefox / Safari / iOS / screen reader — NOT_RUN | corresponding browser/device and named walkthrough | owner unassigned; Chromium/native test evidence does not transfer across engines or establish accessibility acceptance |

No remote CI was awaited. No provider calls, production writes, infrastructure provisioning or
identity/credential changes were performed. Residual risk: the native work-count improvement
may not dominate real browser cost; actual startup, deployment cache behavior, OS IME and
cross-browser/native media acceptance remain open in #146 and their existing owners.

## Sources and reusable method

The canonical reusable workflow is
[Leptos performance and caching](../../.agents/skills/cantos-leptos-web/references/performance-and-caching.md),
with the foundation's existing evidence vocabulary. Upstream sources describe techniques,
not proof that Cantos's pinned CSR build implements them:
[Leptos signals/effects](https://book.leptos.dev/reactivity/14_create_effect.html),
[Leptos binary size](https://book.leptos.dev/deployment/binary_size.html),
[V8 WASM code cache](https://v8.dev/blog/wasm-code-caching),
[MDN Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Cache-Control).
