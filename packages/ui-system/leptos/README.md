# Leptos foundation

`cantos-ui` pins Leptos **0.7.8**, matching the apps. Enable feature `csr` in browser consumers.
No HTTP, backend, provider, domain rule or playback dependency enters this crate.

Implemented exports:

| API | Responsibility |
| --- | --- |
| `UiRoot {theme: Signal<Theme>, density: Signal<Density>, class, children}` | Scoped CSS boundary; system/light/dark, compact/comfortable |
| `Button {id, variant, state: Signal<ButtonState>, on_press: Option<Callback<MouseEvent>>, children}` | Native non-submit action with primary/tonal/text variants; disabled reason required |
| `DisabledReason::new(text)` | Rejects empty/whitespace explanation with `EmptyDisabledReason` |
| `Icon {name: IconName}` | Decorative official outlined SVG subset |
| `tokens::Token` | Generated closed role API, CSS variable/static value lookup and `ALL` |
| `TOKEN_CSS`, `FOUNDATION_CSS` | Compile-time inclusion for hosts; apps may use direct Trunk CSS links |

Load [`tokens.css`](tokens.css), then [`foundation.css`](foundation.css), then scoped app styles.
The source for foundation styles is [`foundation.css.in`](foundation.css.in), with breakpoint
substitution. Generated outputs must be regenerated, not edited.

Focused checks from repo root (limit jobs on a constrained machine):

```sh
python3 scripts/ui_tokens.py --check
python3 scripts/ui_icons.py --check
CARGO_BUILD_JOBS=2 cargo test -p cantos-ui --locked
CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui -p cantos-studio --target wasm32-unknown-unknown --features cantos-ui/csr --locked -- -D warnings
cargo fmt --all -- --check
```

The small [development preview](preview.html) uses the [preview binary](preview/main.rs)
and representative Vietnamese text. It has only theme switching and a local counter.
Its `preview` feature enables CSR only for this diagnostic binary; normal app consumers need `csr`:

```sh
cd packages/ui-system/leptos
CARGO_BUILD_JOBS=2 NO_COLOR=true trunk build preview.html --locked --dist ../../../target/ui-foundation-preview
CARGO_BUILD_JOBS=2 NO_COLOR=true trunk serve preview.html --locked --dist ../../../target/ui-foundation-preview --port 8083
```

This is not a production route or site. No media, credentials, API call or rights/approval gate
is simulated. Capture actual browser evidence separately from the native string-render smoke.
Component breadth is the next [sequence item](../../../docs/work-plan/015-leptos-ui-system.md).
