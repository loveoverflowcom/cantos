# Draft: `cantos.presentation.discovery-retired-language` (#424)

Module-owned inventory of [`cantos.presentation.discovery-retired-language`](../../../specs/behavior/catalog.json). The [root inventory](../../../specs/behavior/inventory.md) keeps the shared authority framing; the registry owns the stable ID and lifecycle.

Separate from the CMP [locale preference](locale_preference.md): this is Theatre Web's discovery
link, not a preference store.

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Reviewed request | [Issue #424](https://github.com/loveoverflowcom/cantos/issues/424) removes the discovery content-language control, forbids mapping the interface locale to an API filter and forbids a hidden language constraint surviving an old link, Clear or Back/Forward. | `draft`; a PASS accepts no content-language preference policy. |
| Implemented, independently of this model | [`routes.rs`](../web/src/app/routes.rs) `discovery_query` drops `language` and `retired_language_rewrite` returns the canonical address or `None`; [`live_navigation.rs`](../web/src/app/live_navigation.rs) applies it with `replaceState` on load, `hashchange` and `popstate`; the search form submits without a language. Native tests marked `Quint run:` mirror `run.retired-deep-link` and `run.language-only`. | No Rust observer is registered (`--conformance` does not apply). Back/Forward and locale runs have no native mirror; the History API is wasm-only. |
| Finite assumption | One document's same-tab history of at most four entries; text and kind are one abstract search class; two locales and two named tags stand for any tag. A new document (deep link, reload, cross-document Back) is `arrive`. | Covers the applied search, the address of the current entry, entry counts and locale isolation. It does not model URL decoding, canonical encoding, unknown parameters, focus/scroll restoration or browser timing. |
| Needs decision, not modeled | Whether the initial language choice should become a catalogue content-language preference. | No action models such a preference; adding one needs an accepted owner first. |
