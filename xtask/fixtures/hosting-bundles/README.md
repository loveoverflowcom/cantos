# Release Web bundle fixtures

Real `index.html` files written by Trunk 0.21.14 for the release builds of Cantos Theatre Web and
Cantos Studio Web (`cargo xtask build <app> --web --release`, captured 2026-10-16 from the
issue-199 work tree). They are inputs to the `deploy firebase` bundle validator tests: they show
the real shape of the entry page (root-relative `href`/`src`, inline module bootstrap, SRI
`integrity="sha384-..."` pins, `modulepreload`/`preload` tags).

Only the HTML is kept. The referenced `*.js`, `*_bg.wasm` and `*.css` files are build outputs and
stay out of Git; the validator is exercised on them end to end by the ignored host test
`real_release_bundles_validate` (needs Trunk and the wasm32 target).

Observed while capturing them, and the reason long-lived caching is not granted (see
`hosting_config::check_cache_value`): each `*.css` file name carries the SeaHash of its delivered
bytes, but the `*.js` and `*_bg.wasm` names of one app carry the same hash although their bytes
differ.
