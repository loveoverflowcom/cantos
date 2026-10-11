# Cantos Theatre Web (skeleton)

`cantos-theatre` is a Leptos 0.7.8 CSR/WASM crate that mounts one page saying that no catalog,
account or playback exists. It is a build target and a location, **not** a Theatre feature: it has
no API client or player, and it must not be cited as evidence for any listening
work item. It consumes `cantos-ui` for the themed shell, native appearance selector and explicit
empty state; [integration evidence](../../../docs/evidence/ui-system-integration.md) separates
compile/native-string checks from unrun browser observations. Both themes use canonical roles
and the comfortable density (48px control target); measured browser bounds remain pending.

## Build

Requires Rust 1.87 with `wasm32-unknown-unknown` and Trunk 0.21.14, as for [Studio](../../cantos-studio/web/README.md).

```sh
cargo xtask build cantos --web   # NO_COLOR=true trunk build --locked in this directory
cargo xtask verify web           # WASM clippy plus Trunk builds of Studio and Theatre
```

`cargo xtask run cantos --web` serves it with `trunk serve` on 127.0.0.1:8082; there is no backend
proxy because no listener API exists. `--mobile` is `UNSUPPORTED` until `apps/cantos/mobile` has
a Gradle project.

## Before building the listener experience

Follow the [listening rules](../../../.agents/skills/cantos-listening/SKILL.md) and the
[Leptos web method](../../../.agents/skills/cantos-leptos-web/SKILL.md): a versioned listener
contract in `packages/contracts`, a playback reducer, then the player. The page must stay free of
Studio code, and the API must keep private drafts, voice assets and previews out of public
delivery. Theatre uses the shared Material 3 Expressive roles at its own calm, media-first
intensity; see the [UI system](../../../docs/design/ui-system.md).
