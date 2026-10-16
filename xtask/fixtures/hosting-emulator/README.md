# Firebase Hosting emulator response headers

Real `curl --dump-header -` output (the form `deploy firebase`'s smoke reads) captured on
2026-10-16 from the **local Hosting emulator** (`firebase emulators:start --only hosting`,
firebase-tools 15.33.0) serving the release Cantos Theatre Web bundle under the tracked policy
`services/firebase-hosting/firebase.json`:

| File | Request |
| --- | --- |
| `page.headers` | `GET /` |
| `wasm.headers`, `script.headers`, `style.headers` | the bundle's `*_bg.wasm`, `*.js` and a `*.css` |
| `missing-wasm.headers` | `GET /__cantos-smoke-missing.wasm` (not in the bundle) |
| `api.headers` | `GET /api/v1/theatre/works` (the API is not on Hosting) |

They are emulator output, **not** production Hosting: the emulator shares the Hosting config
semantics (headers, rewrites, MIME database) but not the CDN, TLS, HTTP/2 or propagation delay.
Real-Hosting acceptance of the smoke stays `NOT_RUN`; see
`docs/evidence/issue199-firebase-hosting.md`.
