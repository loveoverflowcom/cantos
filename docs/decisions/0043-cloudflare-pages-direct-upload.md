# ADR 0043: Cloudflare Pages Direct Upload for the static Web apps

- Status: proposed by the implementing agent; the product owner requested Cloudflare **Pages** in
  [#278](https://github.com/loveoverflowcom/cantos/issues/278). Only the owner accepts it.
- Date: 2026-10-19
- Decision owner: Cantos product owner
- Related issue or PR: [#278](https://github.com/loveoverflowcom/cantos/issues/278) (publisher),
  [#277](https://github.com/loveoverflowcom/cantos/issues/277) (artifact contract),
  [#280](https://github.com/loveoverflowcom/cantos/issues/280) (rollout)
- Supersedes / superseded by: N/A. It does not change [ADR 0041](0041-direct-hosting-api-boundary.md)
  (direct static Web → explicit API origin, exact per-surface CORS, Bearer); it chooses a second
  static host for the same bytes.

## Context

Both Web apps are client-side-rendered Leptos/Trunk bundles: `index.html`, one `*.js`, one
`*_bg.wasm`, CSS, fonts and images. Routing is by URL fragment (`#/…`), the API, PostgreSQL and
Kanidm stay on the VPS, and a bundle contains no server code. The existing static host is Firebase
Hosting, whose policy (`infra/firebase/hosting/firebase.json`) carries no-store HTML, no-referrer,
nosniff, frame, CSP and Permissions-Policy headers.

Cloudflare's documentation currently steers new projects to **Workers** (static assets) while Pages
still offers **Direct Upload** and an official CircleCI guide (sources are listed in #278). Direct
Upload and Git integration are chosen when a Pages project is created and do not convert into each
other. Observed on 2026-10-19 in the owner's account dashboard: no Pages project, and one
Git-integrated Worker named `cantos` whose last build failed; it is unrelated to this decision and
was not changed.

Known and unknown are kept apart: the Pages behaviors this design relies on (a top-level
`404.html` disables the single-page-application fallback; `_headers` rules merge instead of
overriding; the Free plan's file-count and per-asset limits) come from the public documentation
cited in #278 and were **not** exercised against an account ([ACCEPTANCE(NOT_RUN)](../deployment/cloudflare-pages.md#acceptance-status)).

## Decision

1. Implement **Pages Direct Upload** only, as the owner requested. Workers Static Assets is not
   implemented, scaffolded or shipped as a second adapter, and the publisher never creates or
   changes a Worker, Pages Function, `_worker.js` or binding.
2. The publisher is `cargo xtask deploy cloudflare`, a provider under the existing `deploy`
   command ([ADR 0012](0012-xtask-command-surface.md)). It **consumes** the output of
   `cargo xtask release-build <app>-web` and never builds: Cloudflare runs no build.
3. Theatre and Studio are two independent Pages projects. An operator-owned private mapping
   (`infra/cloudflare/projects.json`) names account → app → project and production branch; the
   command requires account, project, branch, environment and artifact explicitly and refuses any
   disagreement with the mapping before extraction or upload.
4. The tracked delivery policy (`infra/cloudflare/<app>/_headers`, `404.html`, optional
   `_redirects`) is owned in this repository, validated before staging and added to the staged
   directory, so the final inventory, digest, smoke and upload cover the reviewed policy bytes.
   The release archive stays product-only; `release_build.py` is unchanged and still refuses
   extensionless files.
5. Wrangler is pinned (`infra/cloudflare/wrangler/`, lockfile, `npm ci --ignore-scripts`). There
   is no implicit `npx`, global install, login or project/zone/DNS/binding creation in xtask.
6. A deployment identity is taken only from Wrangler's machine-readable output and validated against
   the request; exit status 0 alone is never success and a timeout after the write is *unknown*.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Pages Direct Upload (selected, as requested) | Matches the request; static-only; official CI guidance; `_headers`/`404.html` policy is plain files; no Worker code | Cloudflare steers new projects to Workers; Direct Upload cannot become Git-integrated; Free limits of 20,000 files and 25 MiB per asset | Selected within the owner's request; limits and direction are tracked as revisit conditions |
| Workers Static Assets | Cloudflare's current direction; single runtime for later server features | Introduces a Worker application/config the static bundle does not need; different routing/limits semantics to qualify | Not selected; would need an owner decision and its own acceptance, never a silent substitution |
| Both adapters | Hedges direction | A multi-provider framework with two policy owners and two failure vocabularies | Rejected by #278 |
| Pages Git integration / Cloudflare build | No local upload step | Builds again on Cloudflare, so the bytes are not the verified artifact; cannot become Direct Upload | Rejected: violates "no second build" |

## Consequences

- Product and user impact: none until a domain is bound to a project and the owner accepts a
  cutover; a preview URL is public and the artifact's API origin is built in, so it is not a
  backend sandbox. Firebase Hosting remains the recovery path.
- Compatibility and migrations: no change to the API, Bearer, CORS or auth. Domains, DNS and
  `cantos.cc`/`studio.cantos.cc` binding stay with the [rollout runbook](../deployment/cloudflare-rollout.md).
- Operations: a token limited to Account / Cloudflare Pages / Edit (Cloudflare offers no
  project-scoped Pages token), two Direct Upload projects, a pinned Node 22+ and Wrangler. The
  token is never given DNS, Workers or zone permissions.
- Implementation and recovery requirements: one write, never retried; an unknown result is read
  back before any retry; no automatic rollback or deletion of deployments. Recovery is the
  dashboard's promote/rollback or a new deploy of a retained artifact.
- Deferred questions: whether preview deployments should use a separate non-production API;
  whether to adopt Workers before Pages is withdrawn; long-lived caching of hashed assets (needs a
  verified URL-to-bytes invariant).

## Validation and revisit conditions

Validated so far by unit/fixture tests only (`cargo xtask verify rust`, `scripts/test_circleci_cloudflare.py`).
Revisit when Cloudflare withdraws or restricts Pages Direct Upload, when a bundle approaches the
per-asset or file-count limit, when a server-side feature is needed, or when the first real deploy
contradicts a documented behavior listed above.
