# Cantos Theatre Web

`cantos-theatre` is an independent Leptos 0.7.8 CSR/WASM listener shell. Studio remains at
[`apps/web`](../web/README.md). This shell has no API client, authentication, media element or
publication data. Every screen identifies its synthetic demo content. Playback, library saves
and downloads are disabled with an adjacent reason.

## Run locally

Use Rust 1.87, the installed WASM target and Trunk 0.21.14. No Node runtime is needed.

```sh
cargo test -p cantos-theatre --locked
cargo clippy -p cantos-theatre --target wasm32-unknown-unknown --locked -- -D warnings
cd apps/theatre-web
NO_COLOR=true trunk build --locked
NO_COLOR=true trunk serve --locked --port 8082
```

Open `http://127.0.0.1:8082`. First-time Trunk builds need the matching wasm-bindgen 0.2.100
helper; use the installed cache when offline. Do not add a Studio proxy to this app.

## Screens and state

Hash navigation allows static hosting without a server route fallback. Exact routes are
`#/discover`, `#/explore`, `#/library`, `#/work/demo-son-ha` and `#/player/demo-episode`.
Unknown routes display a recovery link. Browser back/forward uses the same route parser;
navigation focuses the permanent page heading. Search filters three local synthetic titles
and preserves the query across screens. It performs no network request.

The collapsed **Demo states** control exposes ready/loading/empty/error/not-implemented
examples and a return action. These states simulate no backend operation or successful retry.
Language and light/dark/system theme remain local in-memory choices.

`demo.rs` owns display fixtures, `navigation.rs` owns pure route/state rules, `messages.rs`
owns Vietnamese/English copy and `view.rs` owns Leptos/DOM behavior. Shared semantic roles
are generated from `contracts/design/cantos-tokens-0.2.0.json`; renderer CSS owns layout.
Cover placeholders and system fonts avoid redistributing unconfirmed design assets.

The [surface map](../../docs/architecture/client-surfaces.md) and
[verification record](../../docs/evidence/client-shells.md) identify missing listener contracts
and native/media gates. This app does not complete work-plan 040.
