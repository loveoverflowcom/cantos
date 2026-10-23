# Web performance and caching

> **Scope.** Review or change Web startup, bundle size, cache behavior or interaction cost.
> Compose the foundation and this renderer's architecture, async and testing references. Product
> authorization, publication and delivery rules remain with their existing owners. The recorded
> [pattern audit](../../../../docs/evidence/web-performance-patterns.md) is evidence for its named
> revision and conditions, not a permanent performance guarantee.

## Start from the executed path

Inspect both Web manifests, their `main.rs`, `Trunk.toml`, generated release HTML/JS and the
actual delivery topology before choosing an optimization. Current manifests pin **Leptos 0.7.8**;
`csr` is enabled for wasm32 and native dev-dependency `ssr` supports host tests. Both entrypoints
mount with `mount_to_body`; native string rendering is not an SSR/hydration deployment. CI pins
**Trunk 0.21.14**; record the tool actually executed. Recheck these facts after dependency changes.

Name the route, surface, user-visible bottleneck and unchanged correctness contract. Measure the
cheapest plausible fix before changing architecture. A source pattern, smaller raw binary or one
warm run cannot establish faster browser interaction. Reuse [`web-testing.md`](web-testing.md)
for browser evidence and [large-script rules](script-editor.md#9-large-scripts) for the editor.

## Pattern contract

These are manual review requirements; this table creates no new command or gate. Existing tests
enforce only their asserted behavior. Choose a scoped exception with its rationale and residual
risk; exceptions never waive authorization, private-data isolation or mutation reconciliation.

| Rule / failure prevented | Good example | Counterexample | Oracle | Enforcement status / exception |
|---|---|---|---|---|
| Classify every cache by owner and eligibility; avoid private data exposure | Public content-addressed static asset policy separate from an account-bound in-memory projection | Apply a long public TTL to every GET because URLs contain a digest | Header/body tests plus auth-switch, revocation and deployment scenarios | Manual; route-specific policy change needs its owning contract and real host acceptance |
| Establish immutable URLs and HTML update behavior together; avoid mixed releases | Same binary keeps the same URL; new HTML selects new assets; rollback retains old assets | Overwrite `app.wasm` under immutable headers or timestamp the same binary on every request | Release inventory, exact hashes, deploy/rollback requests | Manual; real-host header/deploy/rollback acceptance is required before activation; unhashed public files need an explicit revalidation policy |
| Attribute browser code-cache claims to observed events; avoid fabricated hits | Record streaming loader/MIME eligibility and name a trace hit if observed | Treat `transferSize == 0` as compiled-WASM cache proof | Versioned browser trace and resource requests | Manual; unavailable events mean code-cache evidence `NOT_RUN`, not a guessed hit |
| Match reactive read and lifetime to the consumer; avoid deep copies and broad reruns | Borrow the fields needed by a label; keep player/draft state at its required lifetime | Clone an editor tree per label or recreate the player on route entry | Pinned API review, semantic regression, profiler/allocation or rerun counts | Manual; cheap `Copy` reads and deliberate owned snapshots need no mechanical rewrite |
| Preserve async/session fences while reducing work; avoid stale private state or duplicate writes | One read owner, full request identity, fenced success/error, cleared prior-session projection | Global memoized private response or a refetch effect that resubmits a write | Out-of-order success/error, unmount, account-switch and lost-response scenarios | Existing async owner/tests; a performance target never exempts a fence |
| Compare measured costs under the same scenario; avoid speed claims from incompatible runs | Paired release build and browser distributions with fixture, host and cache state | Compare a warm debug candidate with a cold release baseline | Reproducible record with sample counts, correctness checks and limitations | Manual; pure metadata/style edits report no measured runtime delta |
| Spike rendering/splitting compatibility before adoption; avoid broken lifetimes and deployment | Bounded public-route SSR experiment with ADR and player/auth/navigation acceptance | Add current `#[lazy_route]` examples to pinned CSR/Trunk as a drop-in optimization | Exact-version build plus first-interaction, security and deploy/rollback tests | Manual; no compatibility or benefit evidence means defer |

## Keep four cache layers distinct

| Layer | Owner and inventory | Reuse / invalidation question |
|---|---|---|
| Public static assets: JS/WASM/CSS/fonts/images | Static host/proxy/CDN; actual generated paths, MIME, hashes, encodings, preload links | Is this exact URL public and immutable by content? How do HTML refresh, old sessions, deploy and rollback find the right bytes? |
| Browser compiled WASM | Browser engine; generated loader and versioned trace | Did streaming compilation occur and did this engine observe a code-cache hit? HTTP reuse alone does not answer this. |
| Public API data | Owning HTTP/application contract; route, revision, representation and freshness | Is it still public and eligible after publication changes, rights withdrawal or retraction? Who invalidates it? |
| Private/session/user data | API authorization; client projection lifetime and auth generation | Does account switch/logout purge visibility and fence pending results? Can revocation/expiry deny the next operation? |

The current API HTTP shell uses `Cache-Control: no-store`; protected audio uses
`private, no-store, no-transform`. Inspect the actual handler/middleware and preserve this
security contract. Asset caching does not authorize relaxing API/media policy. An immutable
object digest proves byte identity, not permission, publication status or permanent entitlement.
Use the [wire-codec owner](../../cantos-engineering/references/wire-codec.md) for negotiation,
`Vary: Accept` and codec scope; benchmark compressed payload plus encode/decode, bundle and memory
cost on the specific route. Keep each client's implemented default and explicit pilot opt-ins.

Do not cache private drafts/revisions, tokens/capabilities, auth responses, signed delivery URLs,
artwork or audio in shared caches, Service Workers, Cache API or IndexedDB as a startup shortcut.
Session-bound in-memory projections need explicit owner/key/freshness, auth-generation purge and
late-response fences. A cached projection never decides authorization. Service Worker/offline
caching needs a separate lifecycle/version/invalidation design and threat model; it is not a
default performance technique. Reuse publication/listening contracts for media eligibility.

### Static delivery and WASM eligibility

Inventory the executed host: Trunk development server, API `ServeDir`, reverse proxy, CDN or
another static host. Some private-storage API configurations deliberately do not serve static
files. Put headers/compression in the actual owning host/adapter; never a UI component. Confirm
what the browser receives through every intervening proxy, not only the origin configuration.

- Inspect fresh release output, including `copy-dir`, fonts and referenced URLs; Trunk's hash
  setting does not prove every copied asset is content-addressed. Apply a policy such as
  `public, max-age=31536000, immutable` only to verified public immutable URLs. HTML/app shell and
  mutable URLs need their own update/revalidation behavior; never blanket immutable headers.
- Keep old release assets for supported old sessions and rollback. Test a fresh profile, same
  release reload, HTML update, old tab after deploy, rollback and missing asset; a missing WASM
  or CSS must not receive an HTML fallback with `200` and a misleading MIME type.
- Check `Content-Type: application/wasm`, emitted `instantiateStreaming`/`compileStreaming` path
  and fallback, `Content-Encoding`, encoded transfer bytes and validators/304 where applicable.
  Do not force every request to re-download unchanged bytes, add random query strings or invent
  a native-code cache. Do not double-compress Ogg/Opus or other already compressed media.
- Record gzip/Brotli sizes as modeled compression unless that host served the encoding. Record
  preload behavior from generated HTML and network requests before adding hints; duplicate
  fetches or excessive preloading can increase startup work. Compare size-oriented release/LTO/
  `wasm-opt` settings with interaction CPU and compile/instantiate cost before adopting them.

V8 documents streaming APIs, stable resource identity and engine-managed compiled code caching.
Its published thresholds and trace names are historical implementation details, not cross-browser
contracts. Inspect the executed Chromium version's trace where available; Firefox and Safari
need their own evidence. A resource-cache hit, `304`, zero transfer or faster reload alone never
establishes a compiled-code hit. Report missing instrumentation explicitly.

## Reactive CPU and retained memory

For each hot read/write, record signal value size, read mode, dependency scope, write frequency,
owner lifetime and consumers. Follow [Leptos async](leptos-async.md) for resources/actions,
request identity, cancellation and receipt semantics; do not create another async policy here.

Pinned Leptos signal traits distinguish tracked `.get()` (clone) and `.with()` (borrow), and
their untracked equivalents. Borrow when only a projection is needed; clone deliberately for
owned render values, an async snapshot or a pure transition. A borrow prevents the value clone,
but still subscribes to that signal: it does not make one field of a large `RwSignal` independently
reactive. Splitting signals or adding a memo needs a real consumer and consistent state ownership.

```rust
// Illustrative projection, not a new component API. `editor` owns a large state value.
let busy = move || editor.with(|state| state.busy()); // tracked, borrowed scalar
// Counterexample: clones the whole editor value merely to read one flag.
let busy_by_copy = move || editor.get().busy();
// An owned save snapshot is a deliberate boundary, outside a borrow and before awaiting.
let snapshot = editor.with_untracked(|state| state.save_snapshot());
```

- Use a derived closure for cheap projections. A memo is a candidate for expensive, repeated
  derivation or suppressing downstream work when its output is unchanged; measure it. Adding a
  memo to every scalar read adds lifetime/retention cost without establishing a gain.
- Untracked reads suit intentional event snapshots. Do not change a rendered tracked read to
  untracked merely to hide reruns: that can leave the view stale. Keep guards/borrows out of
  `.await`, drop them before writes, and avoid nested read/write guards on the same signal.
- Prefer derived state over effects that copy a signal into another signal. Effects own actual
  external synchronization, never a trigger/write feedback loop. Reuse exact reducer outcomes
  and typed intents; a performance refactor cannot weaken transactional state transitions.
- Keep playback session/media owner above route changes; editor draft above pane changes. Scope
  temporary resource/listener/timer state below its actual owner and clean up on disposal. Do not
  move private data to a global owner for reuse. Measure retained heap after repeated enter/leave
  and account switches, distinguishing a deliberate draft/history bound from an unbounded leak.
- Key repeated rows by stable domain identity under the
  [keyed-row correctness rules](web-architecture.md#dynamic-lists-and-keyed-rows). Exercise
  insertion/reorder/focus and Vietnamese composition before/after list optimizations. A
  large-list strategy must preserve focus, reading order and IME state; do not adopt
  virtualization only because a fixture contains many rows.
- Suppress no-op notifications at the write, as the
  [publication rules](web-architecture.md#reactive-state-ownership-and-publication) require;
  a downstream memo cannot recover the work an unconditional `try_update` already triggered
  upstream of it.

The cheap correctness oracle remains exact state/typed-intent results plus deterministic race
tests. Browser profiler, allocation or rerun-count evidence establishes only the measured cost
under its recorded fixture; host string-render tests establish neither DOM cost nor browser CPU.

### Keyed rows are not automatically O(1)

`<For>` plus a per-row memo does not by itself make a one-row update O(1). Name which cost a
claim is about, because they differ:

| Cost | What it scales with | Typical trap |
|---|---|---|
| selector evaluation | number of memos/derivations that read the notified signal | every row memo reads the root aggregate, so one write re-evaluates all of them |
| row lookup | the lookup structure | `find` by ID over a `Vec` makes each row O(n) and the update O(n²) |
| list reconciliation | keys produced by `each` | rebuilding and hashing the full key list on every unrelated notification |
| row computation and clone | the projection size | `document.get()` per row clones the whole document per row |
| DOM mutation | nodes whose text/attributes really changed | the only part keyed rows minimize |

At `d02b2dc8`, Studio [`authoring_view.rs`](../../../../apps/cantos-studio/web/src/authoring_view.rs)
has correct identity (keyed acts/scenes, titles read by ID) but each row title derives from
`document.get()` plus a linear `find`, so an edit costs clones and scans proportional to rows ×
document. Treat that as a measurement candidate, not a proven bottleneck.

Deeper granularity options (normalized entities with indexed lookup, scoped per-entity signals,
a compatible field-tracking store) are evaluated per workload with a concrete consumer and a
before/after measurement; none is adopted by default, and no Leptos upgrade or new dependency is
implied. Changing a source pattern is not a performance improvement until measured.

## Comparable performance evidence

Before changing code, record a baseline and a falsifiable target for the named scenario. Reuse
the foundation's [report and evidence vocabulary](../../cantos-engineering/SKILL.md#9-completion-report).
Performance numbers are observations attached to those claims, not a new evidence level or gate.
Repository execution/reproduction goes through [`cargo xtask`](../../../../xtask/README.md);
inspect its current command surface, and extend it for new repository orchestration rather than
creating a competing script/runner. No command becomes implemented by appearing in a skill.

| Record | Required detail |
|---|---|
| Provenance | Source SHA and dirty diff, asset hashes/URLs, fixture revision/size, exact command, tool versions, release/debug profile, features, `wasm-opt`/compression settings |
| Environment | Browser engine/version, headless/headed and flags, OS/CPU, throttle/viewport, actual host/proxy topology and headers, mocked/live backend and media, extensions/background load |
| Cache condition | Fresh isolated profile/cold caches; same-profile full reload with resource cache enabled; repeat same release after restart where possible; same-tab navigation measured separately; bfcache history restoration explicitly named |
| Samples | Same scenario and fixture before/after; warmup and run count, ordering/reset procedure, median and p95 with distribution/range; no p95 claim from one sample |
| Cost | Initial JS glue/WASM/CSS/fonts/assets raw and encoded bytes; actual transfers; observed compile/instantiate/app-ready boundaries; FCP/LCP/CLS/INP or named interaction latency; long tasks/CPU/frame drops; peak/retained heap where instrumented |
| Correctness | Stale success/error, duplicate submit, unmount, logout/account switch, unsaved draft, IME/focus/navigation/player continuity, deploy/rollback and access/publication invalidation applicable to the change |
| Decision | Adopt/defer/reject per technique; baseline/delta and method or explicit `NOT_RUN`/`UNSUPPORTED` evidence gap; scoped risks, prerequisites and next owner |

Cold means the specified caches were reset, not just a new tab. Warm resource reuse is distinct
from compiled-code reuse. Hot/repeat is an explicitly recorded repeat condition, not a universal
browser hit guarantee. Keep lab synthetic interaction latency separate from field INP and state
the collection method. A stable DOM is not automatically app-ready or first usable interaction;
define that milestone before comparing it. Do not convert synthetic/mock playback into a live
audio claim. Redact private content, cookies, tokens and signed URLs from retained traces/HARs.

Keep full artifacts in the existing ignored evidence directories. Commit a concise versioned
report under `docs/evidence/` with reproducible provenance, measurements and limitations; never
commit private browser profiles or downloaded/generated media. Browser/device evidence is scoped
per engine/platform. Build/static/header/unit checks and runtime performance acceptance retain
their independent statuses; unavailable browser measurement cannot turn a build into a speedup.

## Rendering and splitting experiments

SSR/hydration, Islands and lazy WASM loading are compatibility experiments until the exact pinned
Leptos/build/host path runs and a reviewed decision adopts it. Current Leptos book examples for
`cargo leptos --split`, `#[lazy]` and `#[lazy_route]` describe another build path; do not assume they
work in 0.7.8 + Trunk. Islands require an SSR/hydration path rather than a CSR flag flip.

Begin an SSR/Islands experiment on a small public Theatre route with no private Studio payload.
Compare TTFB, first content/LCP, actual hydration/initial bytes and first usable interaction with
CSR under identical conditions. Preserve route/root player lifetime, focus, session boundaries,
browser-only helpers, loading/error/retry and deploy/rollback. For splitting, measure chunk and
data-loading waterfalls and cross-release compatibility, not only the first chunk's reduced size.
Keep a clear defer/reject outcome when build compatibility, deployment cost or benefit is missing;
changing the stack/build host still needs the existing architecture decision process.

## Primary technical sources

Rechecked 2026-10-15. Local pinned manifests/source and executed output establish repository
facts; evolving upstream documentation establishes a technique to investigate, not adoption.

- [Leptos 0.7.8 signal traits](https://github.com/leptos-rs/leptos/blob/v0.7.8/reactive_graph/src/traits.rs): tracked borrowing/cloning and untracked read APIs.
- [Trunk 0.21.14 configuration](https://github.com/trunk-rs/trunk/blob/v0.21.14/site/content/configuration.md): release, public URL, hashing and injected scripts configuration; inspect output for each asset.
- [V8 WASM code caching](https://v8.dev/blog/wasm-code-caching): streaming eligibility and engine-specific observations; historical heuristics require versioned verification.
- [MDN `Cache-Control`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Cache-Control) and [`instantiateStreaming`](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/instantiateStreaming_static): cache semantics and correct WASM MIME.
- [Leptos SSR modes](https://book.leptos.dev/ssr/23_ssr_modes.html), [Islands](https://book.leptos.dev/islands.html) and [binary size/splitting](https://book.leptos.dev/deployment/binary_size.html): current upstream techniques, subject to pinned-version/build-path compatibility.
