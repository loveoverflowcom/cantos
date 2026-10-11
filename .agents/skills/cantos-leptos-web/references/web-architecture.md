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
[`apps/cantos/web`](../../../../apps/cantos/web/README.md) (a skeleton with no listener behavior).
Every path, crate and type below that the repository does not contain is a **proposal** until a Web
work item creates it.

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
contract types       cantos-contracts raw DTOs shared with the server and checked against the
                     packages/contracts fixtures; never hand-copied into the client
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
| Web depends on pure core crates and `cantos-contracts` only, never on backend application or infrastructure crates | manifest review; `cargo tree --target wasm32-unknown-unknown -e normal` shows no server-only crate (proposed check) | manual until a dependency check exists |
| DTOs live in the adapter and map to view-model types | host tests decode the shared `packages/contracts/` fixtures (Studio `wire.rs`, adaptation tests) | partial; see [`packages/contracts/README.md`](../../../../packages/contracts/README.md) |
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

```text
apps/cantos-studio/web/           implemented as flat modules (adaptation, authoring, editor, import,
                                  inspector, production, view); this tree is the proposed direction
  src/
    app.rs                        router, surface root, context providers, theme/lang attributes
    script_editor/                page, rows, status region, resources, styles (.scss beside .rs)
    import/                       upload, source preview, adaptation review
    casting/                      character ↔ voice profile, sample preview
    production/                   run status, job progress, regeneration scope, QC findings
    publication/                  approval and publish controls (render server gates)
apps/cantos/web/                  skeleton today (lib, main, one placeholder view); proposed for the first item
  src/
    app.rs                        router, surface root, context providers, theme/lang attributes
    discovery/  work/  episode/  library/
    player/                       session, mini_player, full_player, media_host
packages/                         code both apps really use; nothing Web-side is shared yet
```

Each package also gets `platform/` (web-sys helpers: media, storage, clipboard, dialog, visibility),
`adapters/` (HTTP adapters per port over `cantos-contracts`) and
`i18n/` (locale resources and message keys) when it needs them. `ui/` primitives (button, chip,
dialog, field) move to `packages/` only once both apps have a real consumer; until then each app
owns its own. Studio and Theatre never share a route tree, session context or permission context.

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

Both packages are client-side rendered and built with Trunk: Studio is implemented, Theatre is a
skeleton. Moving any surface to server rendering (for example Theatre discovery) needs a decision
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
