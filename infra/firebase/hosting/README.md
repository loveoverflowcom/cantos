# Firebase Hosting configuration

Operator configuration for publishing **one static Leptos Web app at a time** to Firebase Hosting
with the explicit, manual `cargo xtask deploy firebase`. It is not a `cargo xtask services` process
and is not in [the manifest](../../../services/manifest.toml); nothing starts or deploys it automatically, and no
GitHub Actions workflow is involved. Setup, the routine, verification and recovery are in the
[Firebase Hosting runbook](../../../docs/deployment/firebase-hosting.md); the command contract is in the
[xtask README](../../../xtask/README.md#deploy-firebase).

| File | Tracked? | Owns |
| --- | --- | --- |
| [`firebase.json`](firebase.json) | yes | The reviewed policy: two independent Hosting targets (`theatre`, `studio`), `ignore` rules, browser security headers, revalidation for every path and no-store for root/HTML documents. No rewrites, hooks, redirects or other Firebase product. Its `public` paths point at each app's default Trunk `dist` for the **local emulator** after `cargo xtask build <app> --web --release`; a deploy replaces them with the run's own bundle. Do not use this file for a raw `firebase deploy`: the CLI rejects a `public` path outside the project directory (`..`) only after it has created the preview channel and a version. `emulators` is local-only and dropped from a deploy |
| [`firebaserc.example`](firebaserc.example) | yes | The shape of your site mapping, with placeholders only. Never put a real project or site ID in a tracked file |
| `.firebaserc` | **no** (ignored) | Your `target -> site` mapping per project, from the example or `firebase target:apply hosting <target> <site> --project <id>`. Strict JSON |

`cargo xtask deploy firebase` never edits these files. It validates the policy, reads the mapping,
and writes a run-specific effective `firebase.json` and `.firebaserc` (one target, one site, a
run-relative `public`) into `artifacts/hosting/<run-id>/`, so it cannot reach the other app, another
project or another Firebase product.

The policy is checked by `cargo test -p xtask`. It is refused if it adds a `site`, a hook, a redirect,
another Firebase product, a rewrite that is not a literal route outside `/api`, a missing
revalidating rule, or any long-lived `Cache-Control`. Strict JSON has no comments, so the reasons
live here and in `xtask/src/deploy/hosting_config.rs`:

- **No rewrites.** The clients navigate by hash and query on `/` and call `/api/v1/...` with absolute
  paths; a blanket `** -> /index.html` would answer a missing `.wasm` and every `/api/*` request with
  `200` and the page.
- **`no-cache` everywhere.** Hosting's default is `max-age=3600`; a filename hash is not evidence that
  a URL's bytes never change (observed on the real bundles: the `.js` and `_bg.wasm` names of one app
  share a hash although their bytes differ).
- **Document responses also use `no-store`.** The `/` and `**/*.html` rules follow the universal
  rule and retain `no-cache` while refusing storage, including the first root response to a
  code-bearing email action. Hosting matches URL paths rather than individual query fields.
- **Browser headers apply to both surfaces.** `Referrer-Policy: no-referrer` protects first-response
  navigation and subresource requests; `X-Content-Type-Options: nosniff` retains MIME enforcement.
  `X-Frame-Options: DENY` and CSP `frame-ancestors 'none'` refuse embedding; `base-uri 'self'` and
  `object-src 'none'` restrict base injection and legacy objects. The limited CSP intentionally
  leaves existing Leptos/WASM, API, media and identity loading behavior for separate browser
  qualification. Camera, microphone and geolocation are disabled because these static clients
  do not capture them. No cross-origin isolation or unqualified custom-domain HSTS is introduced.

The [official Hosting header contract](https://firebase.google.com/docs/hosting/full-config#headers)
defines path matching and ordered rules; `scripts/test_edge_cache.py` checks the tracked values.
Only a deployed response/browser check establishes delivery. Firebase receives the initial
email-action query before JavaScript can remove it: these headers do not prove query-log
redaction or registration acceptance. Keep that [independent gate](../../../docs/deployment/firebase-enrollment-proof.md)
closed until the action origin, code handling and logging evidence are reviewed.

Site mappings accept Firebase hostname labels of at most 30 characters, never a domain or URL.
The preferred shorter names in [#237](https://github.com/loveoverflowcom/cantos/issues/237),
`cantos` and `cantos-studio`, remain conditional on verified allocation to the operator's project.
The 2026-10-17 read-only lookup was blocked by Firebase authentication; no new mapping was chosen.
`studio.cantos.web.app` is not a valid default Hosting name. Source package/target names remain
`cantos-theatre` and `cantos-studio`. See the [controlled transition and rollback](../../../docs/deployment/firebase-hosting.md#shorter-site-names-controlled-transition-237)
before changing the ignored mapping or transitional Caddy upstreams. Direct Hosting needs the
explicit API origin from ADR 0041; static delivery does not establish browser authentication.
