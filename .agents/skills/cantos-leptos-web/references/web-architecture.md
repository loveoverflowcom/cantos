# Web architecture: surfaces, layers and modules

> **Scope.** Decide where Leptos code lives and which way it depends: the Studio and Theatre
> surfaces and their permission contexts, the component → resource/action → port → HTTP adapter
> layering, which rules the browser may evaluate and which it may only render, pure Rust core
> crates in WASM, secrets, the shell-owned mini-player, module organization and the open SSR/CSR
> decision. Use before creating a route tree, context provider, port, adapter, crate or module.

The product owner is [`apps/cantos-studio/web/README.md`](../../../../apps/cantos-studio/web/README.md) and the Web row of the
[architecture overview](../../../../docs/architecture/overview.md#runtime-and-ownership). The
dependency rule itself is the foundation's [`decoupling.md`](../../cantos-engineering/references/decoupling.md);
this reference applies it to the browser. Two Leptos packages exist, one per product:
[`apps/cantos-studio/web`](../../../../apps/cantos-studio/web/README.md) (implemented) and
[`apps/cantos-theatre/web`](../../../../apps/cantos-theatre/web/README.md) (discovery, account/Library and an isolated listening mock).
Use the [implementation map](../../../../docs/architecture/repository-layout.md#platform-scope)
and shared [`cantos-ui`](../../../../packages/ui-system/leptos/README.md)/
[`cantos-web-auth`](../../../../packages/web-auth/README.md) owners rather than a fixed module snapshot.
Every path, crate and type below that the repository does not contain is a **proposal** until a Web
work item creates it.

## Web authentication migration

The [v1 amendment](../../../../docs/architecture/native-turso-refactor.md#owner-amendment-v1-authentication-and-parallel-integration)
owns selected login factors and Web/native transport; the
[shared Web auth owner](../../../../packages/web-auth/README.md) implements the **cookie** client
(email/password, HttpOnly cookie, session fence, idle renewal). The Server's
[Identity auth v1](../../../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) endpoints issue the cookies;
browser and real Server/Identity acceptance remain pending.

When changing it, trace session issuance/restore/revoke and every fetch, XHR, private-byte and
resource adapter. Review credential inclusion, HttpOnly/Secure cookie scope, exact-origin
CORS/CSRF, expiry and server-confirmed reload restore together. Preserve one owner per surface,
account/family/intent fences and unknown-write reconciliation; never add a cookie/Bearer fallback
or authenticate from local storage. Exercise missing/stale/ambiguous cookies, cross-surface/origin
refusals, late account-switch responses and actual browser restore. Source tests are not proof
that the real cookie path works in a browser.

## 1. Two surfaces, two permission contexts

Studio serves authenticated creators editing private drafts, casting, previews, production status,
QC and approval. Theatre serves listeners and shows only published releases. They share brand,
tokens and primitive components; they do not share permission context, ports or session state.

| Rule | Failure mode | Good | Counterexample | Oracle | Status / exception |
|---|---|---|---|---|---|
| Each surface has its own route root that provides its own context (`StudioContext`, `TheatreContext`) | a Theatre route reaches a Studio port and renders a private draft or preview URL | `theatre::player` only calls `expect_context::<TheatreContext>()` | an episode page that falls back to `StudioPorts` "when the user is a creator" | mount every Theatre route in a test shell that provides only `TheatreContext`; a Studio lookup panics the test | proposed; one package is fine initially if it keeps the first slice smaller |
| `theatre::*` never imports `studio::*` and vice versa; shared code lives in `ui/` or `platform/` | Studio code paths become reachable from listener screens | `ui::button` used by both | `theatre::library` importing `studio::production::JobBadge` | import review; a module-edge check once the package exists (proposed) | a primitive moves to `ui/` only after its second real consumer |
| A hidden or disabled control is never the permission check | bypassing the UI (devtools, crafted request) publishes or reads private data | backend rejects; UI renders the server's `Denied { reason }` | `if session.is_creator { show_publish() }` as the only guard | backend authorization test (owned by the backend), Web test only for the rendered reason | always; the backend enforces ([AGENTS.md](../../../../AGENTS.md#architecture-boundaries)) |

Everything in the WASM bundle is public. Shipping Studio components in a single package exposes
their code, not their data; that is acceptable only because the backend authorizes every request.
Separate origins or bundles for Studio and Theatre (cookie scope, storage isolation, bundle size)
are a deployment decision to record in an ADR, not something to improvise per feature.

## 2. Layering inside a surface

```text
component            reads reactive state, renders semantic HTML, dispatches typed intents
    ↓
resource / action    owns one remote read or one mutation: states, stale guard, submit policy
    ↓
port (trait)         capability the screen needs, provided through Leptos context
    ↓
HTTP adapter         URLs, contract DTOs, representation (wire codec), status → typed PortError
    ↓
contract types       the app's own raw DTOs (src/contract) checked against the Server-owned
                     fixtures; never imported from the Server crate (ADR 0046)
```

| Layer | Owns | Must not own |
|---|---|---|
| Component | markup, local element state (open menu, focus), localized text lookup, token classes | URLs, DTOs, HTTP status codes, provider error text, retry loops, durable rules |
| Resource / action | `Idle/Loading/Ready/Empty/Failed`, request keys, duplicate-submit behavior | transport details, permission policy |
| Port | a narrow trait named for the job: `ScriptPort`, `CastingPort`, `ProductionPort`, `CatalogPort`, `PlaybackAccessPort`, `ProgressPort` | vendor names, `reqwest`/`gloo` types in signatures |
| HTTP adapter | request building, auth header/cookie, codec headers and body bytes, DTO ↔ view-model mapping, `PortError` classification | rendering, Leptos signals, codec fallback or retry |
| Pure core crate | validation, reducers, merge, retry and poll decisions, progress reconciliation | `web-sys`, Leptos, async runtime, I/O |

```rust
// Illustrative, proposed — not existing code. Verify Send/Sync bounds for the pinned Leptos
// version: Leptos 0.7+ context values must be Send + Sync; request futures need not be.
pub trait ScriptPort: Send + Sync {
    fn load(&self, episode: EpisodeId) -> LocalBoxFuture<'static, Result<EditableScript, PortError>>;
    fn save(&self, request: SaveRequest) -> LocalBoxFuture<'static, Result<SaveOutcome, PortError>>;
}

#[derive(Clone)]
pub struct StudioContext {
    pub session: StudioSession,
    pub script: Arc<dyn ScriptPort>,
    pub casting: Arc<dyn CastingPort>,
    pub production: Arc<dyn ProductionPort>,
}
```

Counterexample — a component doing an adapter's job:

```rust
// Counterexample: URL building, DTO decoding and status interpretation inside a component.
let url = format!("/api/episodes/{id}/script");
let response = Request::get(&url).send().await?;
if response.status() == 409 { set_banner.set("Conflict".into()); }
let dto: ScriptDto = response.json().await?;
```

The test for the layering is mechanical: a component test can mount the screen with an in-memory
port and no network. If it cannot, a transport concern has leaked upward.

### Reactive state ownership and publication

These rules apply to the Leptos renderer only; CMP keeps its own Kotlin state owners and never
adopts Rust types or Leptos primitives. API facts below were checked against the pinned
`leptos 0.7.8` / `reactive_graph 0.1.8` / `tachys 0.1.9` sources in `Cargo.lock`; recheck them
after any upgrade. The transition contract itself belongs to the foundation's
[transition semantics](../../cantos-engineering/references/functional-core.md#transition-semantics)
and [closed mutation boundary](../../cantos-engineering/references/functional-core.md#close-the-mutation-boundary).

**Read-only presentation (MUST).** A component that only reads domain state receives a
`ReadSignal` (`RwSignal::read_only()`), a `Memo`, a read-only `Signal`, or an immutable UI
projection, plus `Callback`s or typed intents for what it may request. Do not pass the
`RwSignal` of a whole account/editor aggregate to a reader. Projections need not be `Copy`:
scalars and control projections usually are; text-bearing projections can be `Clone + PartialEq`.
Local input, menu and theme state may use ordinary signals owned where they are used.

- Counterexample at `d02b2dc8`: [`features/account.rs`](../../../../apps/cantos-theatre/web/src/features/account.rs)
  takes `RwSignal<AccountViewState>` and renders `state.get().phase()` and
  `state.get().can_authenticate()`, cloning the whole account aggregate (Library projection and
  sync coordinator included) to read one enum.
- Patterns to reuse: `sync_admission` in [`view/live_shell.rs`](../../../../apps/cantos-theatre/web/src/view/live_shell.rs)
  memoizes `state.with(AccountViewState::fence)` so unchanged writes cannot reschedule its
  effect; [`features/public_work.rs`](../../../../apps/cantos-theatre/web/src/features/public_work.rs)
  derives `Memo::new(move |_| state.with(shape))` once for the page.

**Reading an aggregate (MUST).** Never `get()` or `get_untracked()` a large aggregate to read a
field. Use tracked `.with(|s| …)` while rendering, `with_untracked` for a deliberate event-time
snapshot, and a `Memo`/selector when several consumers share a derivation or downstream work
must stop when the result is equal.

- `.with()` avoids the clone but still subscribes to the whole signal; it does not create
  field-level reactivity.
- A `Memo` recomputes on every upstream notification and suppresses only equal *outputs*
  (`PartialEq`); it makes downstream work conditional, not upstream updates free.
- Do not switch a rendered read to untracked to hide reruns; the view can go stale.
- Do not hold a borrow or guard across `.await`, or write the same signal inside its own read.

**No-op suppression happens before notification (MUST).** In the pinned version `set`, `update`
and `try_update` always notify (`try_update` is `try_maybe_update(|v| (true, f(v)))`);
`maybe_update`/`try_maybe_update` notify only when the closure returns `true`;
`update_untracked`/`try_update_untracked` never notify. A rejected or unchanged transition MUST
NOT notify the aggregate's subscribers, so derive the notify flag from the transition outcome:

```rust
// Illustrative over the pinned API; `decide`, `Transition` and `StateChange` are the owner's
// pure contract (see the foundation link above), not existing Cantos types.
let outcome = state.try_maybe_update(|current| match decide(current, command, &facts) {
    Ok(Transition { change: StateChange::Replace(next), effects }) => {
        *current = next;
        (true, Ok(effects))
    }
    Ok(Transition { change: StateChange::Unchanged, effects }) => (false, Ok(effects)),
    Err(refusal) => (false, Err(refusal)),
});
// The write guard is dropped here. Interpret Ok(effects) even when nothing was published.
```

- Never return `false` after mutating: subscribers keep showing the old value. Never use an
  untracked write as an "optimization" for a change the UI must reflect.
- Discovering a no-op inside `update`/`try_update` is too late: the notification already happens.
- An effects-only transition is still interpreted although no state was published.
- A UI error projection that really changed (a visible failure banner) is a state change and
  notifies; "on `Err` publish nothing" applies to a refused command, not to an absorbed event.
- Counterexample at `d02b2dc8`: [`live/runtime.rs`](../../../../apps/cantos-theatre/web/src/live/runtime.rs)
  `persist_sync_journal` ends with `state.try_update(|s| s.record_storage(&fence, result))`,
  which notifies every `AccountViewState` subscriber even when the fence is stale or the stored
  fault is unchanged; most `try_update` calls in that file share the shape.

**Split signals keep one authoritative owner (MUST).** If an aggregate is split into several
signals for granularity, one owner still performs each transition and writes all affected
signals; do not chain `Effect`s that copy state from one signal to another. Logout and account
switch publish the private purge with the phase change, so no observable moment shows a new
account with old Library rows, and fenced late completions are ignored rather than written.

### Dynamic lists and keyed rows

For a list with insert, remove or reorder, or with row-local state (focus, an open menu, an
input, an in-flight action), MUST use keyed `<For>` with a stable domain identity as the key,
never an index or a per-render random value. In the pinned `tachys` keyed diff, only the key
set is compared: a row whose key survives keeps its existing view and `children` is not called
again. Therefore:

- separate membership/order (`each` yields IDs or `(id, …)` identities) from row content;
- each row reads a reactive projection of its own entity by ID, never only the snapshot it was
  created from, or an updated entity with the same ID stays stale;
- a row whose entity was removed between notifications renders nothing or a removed state; its
  lookup returns `Option` and never panics;
- do not put mutable content (availability, progress, title) in the key unless remounting the
  row is the intended behavior, because a key change destroys DOM identity, focus and IME
  composition;
- preserve DOM identity, focus, selection and Vietnamese IME composition across sibling updates.

`Memo<Vec<RowId>>` is one way to publish membership. The pinned `For` requires
`each: Fn() -> I` with `I: IntoIterator + Send + 'static`; an `Arc<[RowId]>` is not an owned
iterator, so `each` still has to produce one (for example a collected `Vec`) and saves no
allocation by itself. `.map(…).collect_view()` remains fine for a small static list with no
identity requirement.

- Pattern to reuse: Studio [`authoring_view.rs`](../../../../apps/cantos-studio/web/src/authoring_view.rs)
  keys acts and scenes by ID and reads titles by ID inside each row.
- Counterexample at `d02b2dc8`: Theatre `LiveSection` in
  [`features/library/live.rs`](../../../../apps/cantos-theatre/web/src/features/library/live.rs)
  rebuilds every row and its action buttons inside one `state.with(…)` closure on any account
  notification, so focus on a row control cannot survive an unrelated progress or sync write.
- Review question: [`features/public_work.rs`](../../../../apps/cantos-theatre/web/src/features/public_work.rs)
  keys units by `(unit_id, publication_id, unavailable)` and passes the row snapshot to
  `children`; an availability change remounts the row by design or by accident, and other row
  fields do not update under an unchanged key.

Cost is a separate claim; see [performance: keyed rows](performance-and-caching.md#keyed-rows-are-not-automatically-o1).
Granularity beyond this (normalized entity state with indexed lookup, scoped per-entity signals,
or a field-tracking reactive store) needs a compatibility check against the pinned version, a
concrete consumer and a recorded decision; `reactive_stores` is only a transitive dependency
today and is not adopted by this guidance.

## 3. Which decisions the browser may make

The browser gives fast feedback; the backend decides. Mirror a rule in WASM only when the *same*
pure crate runs on both sides, so they cannot drift.

| Decision | Browser role | Authority | Owner reference |
|---|---|---|---|
| Script IR validation | run the shared validator for inline feedback; never suppress a server finding | server re-validates on save | [`cantos-script-ir`](../../cantos-script-ir/SKILL.md) |
| Save conflict (stale base revision) | render the conflict, preserve the draft | server revision check | [`revision-lifecycle.md`](../../cantos-script-ir/references/revision-lifecycle.md) |
| Regeneration scope, cost estimate | render the server's plan, including "estimate unavailable" | server plan and budget | [`cost-and-budget.md`](../../cantos-production-pipeline/references/cost-and-budget.md) |
| Approval staleness, publish eligibility | render gate results and reasons | server gates only | [`approvals-and-gates.md`](../../cantos-publication/references/approvals-and-gates.md) |
| Playback state | run the shared reducer over media events | the media element's observed events | [`playback-semantics.md`](../../cantos-listening/references/playback-semantics.md) |
| Progress reconciliation | keep a local candidate, submit with the observed server revision | server revision | [`progress-sync.md`](../../cantos-listening/references/progress-sync.md) |

Counterexample: computing `can_publish = qc_passed && approved` in a component. It duplicates a
gate that changes with rights, render revisions and approval invalidation, and it will drift.

## 4. Pure core crates in WASM, contracts across the wire

A pure crate may be compiled into the Web client when it has a real browser consumer today (the
Script IR validator for inline errors, the playback reducer, the editor reducer). It must build
for `wasm32-unknown-unknown` and for the host, so its tests run as plain `#[test]` without a
browser. It must not pull in `tokio`, SQL clients, Axum, provider SDKs or object-store clients.

| Rule | Oracle | Status |
|---|---|---|
| Web depends on pure core crates and its own `contract` module only, never on a service crate | manifest review; `cargo tree --target wasm32-unknown-unknown -e normal` shows no server-only crate (proposed check) | manual until a dependency check exists |
| DTOs live in the adapter and map to view-model types | host tests decode the Server-owned fixtures (`contract::conformance_tests`, Studio `wire.rs`, adaptation tests) | partial; see [`services/cantos-server/contracts/README.md`](../../../../services/cantos-server/contracts/README.md) |
| A new crate needs a consumer and a reason to change of its own | review against [decision 0001](../../../../docs/decisions/0001-modular-monolith.md) | always |

Shared DTOs follow the foundation's [`http-api-boundary.md`](../../cantos-engineering/references/http-api-boundary.md);
a second representation follows [`wire-codec.md`](../../cantos-engineering/references/wire-codec.md).
A client-side copy of a server struct is the drift this rule prevents.

## 5. Secrets and private media never reach browser code

Provider credentials, object-store keys and private bucket access belong to the server
([business rule 12](../../../../docs/product/business-rules.md#rights-and-access)). In Web code:

- Anything reachable from the client crate at build time (`env!`, `option_env!`, a bundled config
  file) is published with the bundle. Only public configuration such as an API base path belongs
  there. A proposed check greps the built `.wasm`/JS for names from [`.env.example`](../../../../.env.example).
- Studio previews and Theatre media arrive as backend-authorized, short-lived URLs or a proxied
  stream. Never put a signed URL in a route, `localStorage`, logs, error text or telemetry.
- The browser never lists buckets, builds object keys or chooses a CDN path; it plays the URL the
  server returned for an exact publication or preview.

## 6. Shell-owned state that must survive navigation

The mini-player keeps the current episode across navigation ([UI system § Theatre](../../../../docs/design/ui-system.md#theatre-compact-listening)).
Model it so that a route change *cannot* stop playback:

```rust
// Illustrative, proposed. The <audio> element and session live above <Routes>.
#[component]
pub fn TheatreShell() -> impl IntoView {
    let session = PlaybackSession::new(expect_context::<TheatreContext>());
    provide_context(session.clone());
    view! {
        <Router>
            <main id="content"><Routes fallback=NotFound>/* discovery, work, episode, library */</Routes></main>
            <MiniPlayer/>
            <MediaHost session/>   // owns the single <audio> element
        </Router>
    }
}
```

| Rule | Failure mode | Oracle |
|---|---|---|
| Exactly one media element, owned by the shell | each episode page creates its own `<audio>`; navigation restarts or doubles audio | real-browser test: play, navigate work → episode → library, assert one `audio` element and advancing position |
| Routes attach to the session; they never construct it | unmounting a route disposes the player | component test: unmount the episode route, session state unchanged |
| Studio clip previews use their own preview player, not the Theatre session | a preview writes listener progress or replaces the listener's episode | review plus a test that previews never call `ProgressPort` |

The same principle protects Studio: the editor draft is owned at the episode-editor route root,
above the inspector, timeline and narrow-screen panes ([`script-editor.md`](script-editor.md)).

## 7. Feature-first module organization

Inspect [Studio](../../../../apps/cantos-studio/web/README.md),
[Theatre](../../../../apps/cantos-theatre/web/README.md) and the
[implementation map](../../../../docs/architecture/repository-layout.md#map) before adding a module.
Both apps already consume shared UI and auth packages. Reuse those public APIs; keep app
routes, feature ports, copy and permission contexts with their current owner. Add a platform
helper or feature folder only for an actual consumer, not to realize an illustrative tree.
Shared code never means a shared route tree, session authority or permission context.

Never create `components/`, `hooks/`, `utils/`, `helpers/` or `common/` buckets; they collect
unrelated reasons to change. A feature folder owns its page, its resources, its styles and its
tests. Keep presentation transitions near their lifetime; give a lifecycle (save, upload, playback,
job tracking) a pure reducer in a core crate and a thin Leptos layer over it.

## 8. Shell concerns owned once

| Concern | Owner | Rule |
|---|---|---|
| `lang` on `<html>` and on mixed-language spans | app shell + i18n | `lang="vi"` affects font fallback, hyphenation and screen-reader voice; never leave the default |
| Theme attribute (`data-theme`) | app shell | set once at the root; components consume tokens only ([`styling.md`](styling.md)) |
| Focus after client-side navigation | router integration | move focus to the new page heading or main region and update the title ([`dom-and-media-interop.md`](dom-and-media-interop.md)) |
| Localized strings | [`localization.md`](../../cantos-ui-design/references/localization.md) | no string literals in `view!`; error text comes from typed error → message key |

## 9. CSR today; SSR needs a decision

Both implemented Web packages are client-side rendered and built with Trunk.
Moving any surface to server rendering (for example Theatre discovery) needs a decision
recorded with [`templates/adr.md`](../../../../templates/adr.md) first; do not add `cargo-leptos`
or server functions without it.

| Concern | Client-side rendering | Server rendering + hydration |
|---|---|---|
| Theatre discovery first paint, link previews | content waits for WASM | content in the first HTML response |
| Studio editor | simplest; authenticated, no listener benefit from SSR | hydration constraints with little gain |
| Backend coupling | HTTP contract only | server functions become a contract surface and must call the same application layer and permission checks, never a shortcut around them |
| Browser-only code | must still stay in effects and helpers | required in effects; component bodies run on the server |
| Delivery | static assets behind the CDN | a Rust process renders pages |

A hybrid (server-rendered Theatre, client-rendered Studio) is a legitimate option to evaluate.
Keep that option open: no `window()` or `document()` in a component body, browser APIs only inside
effects and `platform/` helpers.

## 10. Performance order

`correctness → architecture → measured bottleneck → bundle/startup → micro-optimization`. Measure
compressed `.wasm` size, first meaningful render and input latency before changing structure, and
report before/after numbers with the method. A Studio bundle shipped to listeners, a list that
renders every dialogue of a long script and a signal that invalidates a whole subtree per keystroke
are architecture problems; [`script-editor.md`](script-editor.md#9-large-scripts) covers the editor.

## Review questions

1. Which surface and permission context does this route belong to, and can it reach the other's ports?
2. Does any component build a URL, hold a DTO or read an HTTP status?
3. Is a durable rule evaluated in WASM? Is it the same crate the server runs, and is the server still the authority?
4. Could any build-time value, URL or log line expose a credential or a private object?
5. Does any state that must survive navigation live below `<Routes>`?
6. Does the new module, crate or `ui/` primitive have a consumer today?
