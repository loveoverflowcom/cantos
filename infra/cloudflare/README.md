# Cloudflare Pages delivery assets

Tracked, nonsecret inputs of [`cargo xtask deploy cloudflare`](../../xtask/README.md#deploy-cloudflare)
(issue [#278](https://github.com/loveoverflowcom/cantos/issues/278),
[ADR 0043](../../docs/decisions/0043-cloudflare-pages-direct-upload.md)). `infra/` owns the policy;
`xtask/` owns planning, validation, invocation and receipts. Nothing here is a Worker, a Pages
Function or a build: Cloudflare runs no build and receives only the verified static artifact.

| Path | Owns |
| --- | --- |
| `theatre/_headers`, `studio/_headers` | Security and cache headers for each app (no-referrer, nosniff, frame/CSP/Permissions-Policy; no-store for the HTML entry). Pages **merges** matching rules, so entry rules are disjoint from `/*` and `/*` sets no `Cache-Control`. |
| `theatre/404.html`, `studio/404.html` | The mandatory top-level 404 page. Without it Pages serves `index.html` with status 200 for every unknown path, hiding a missing `.wasm`/asset or `/api/*`. Both apps route by URL fragment, so no path needs a fallback. |
| `<app>/_redirects` | Optional; absent today. Only 301/302/303/307/308 rules; a `200` rewrite and anything under `/api` are refused. |
| `theatre/.well-known/assetlinks.json`, `theatre/.well-known/apple-app-site-association` | Optional, absent today ([ADR 0048](https://github.com/loveoverflowcom/cantos/pull/428)). The only dot-files a bundle may carry: release `com.cantos.theatre` only (no debug association) and the single path `/verify-email`; `_headers` must then type the Apple file `application/json`. Add them with the operator's release certificate fingerprint and Apple team ID. |
| `projects.example.json` | Shape of the **private** `projects.json` (ignored by Git): account ID → `theatre`/`studio` → Pages project name and production branch. Contains no allocated identifier. |
| `wrangler/package.json`, `package-lock.json` | The pinned Wrangler (4.151.2). Install with `npm ci --ignore-scripts` in that directory; xtask never uses `npx`, a global install or an implicit download. |

Theatre and Studio are separate Pages projects and separate policy directories; the technical
`cantos-theatre`/`cantos-studio` names and Cargo targets are unchanged. The policy files are validated
by xtask, copied into the run's staged directory and become part of the hashed, smoked and uploaded
inventory; they are never uploaded from this directory directly.

The [Cloudflare Pages runbook](../../docs/deployment/cloudflare-pages.md) owns setup, routine, recovery
and acceptance status. The [rollout runbook](../../docs/deployment/cloudflare-rollout.md) owns domains,
cutover and Hosting retirement. This directory changes no account, DNS record, production setting or
cloud resource. `cargo xtask hosting-report` reads an operator-declared local record only and never
publishes or queries a provider.
For a known release, use the independent
[`cargo xtask hosting-smoke`](../../xtask/README.md#read-only-hosting-http-observation) to compare
the served static bytes without another publish. Its receipt is HTTP evidence only; provider
identity, browser/auth, domain cutover and recovery need their separate observations.
