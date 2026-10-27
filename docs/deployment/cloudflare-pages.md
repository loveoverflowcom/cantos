# Cloudflare Pages Direct Upload for the Cantos Web apps

Operator runbook for `cargo xtask deploy cloudflare`
([#278](https://github.com/loveoverflowcom/cantos/issues/278),
[ADR 0043](../decisions/0043-cloudflare-pages-direct-upload.md)). It publishes the **already built**
Web artifact of **one** app (Cantos Theatre or Cantos Studio) to a Cloudflare Pages **Direct
Upload** project. The command contract is the [xtask README](../../xtask/README.md#deploy-cloudflare);
the rollout/domain/recovery plan is [cloudflare-rollout.md](cloudflare-rollout.md); the Firebase
lane in [firebase-hosting.md](firebase-hosting.md) stays the recovery path.

> **Status — implemented, with source-bound preview observations recorded below.** Production,
> custom-domain cutover and browser/auth acceptance remain separate. See
> [what is and is not accepted](#acceptance-status).

## What this is, and what it is not

| It is | It is not |
| --- | --- |
| A manual, explicit command that uploads the verified output of `cargo xtask release-build <app>-web` | A build: Cloudflare runs no build and xtask never rebuilds or edits the bytes |
| Static HTML/CSS/JS/WASM for one app, one Pages project per app | Hosting for the Rust API, PostgreSQL, Kanidm, audio or private drafts |
| Pages Direct Upload through a pinned local Wrangler | A Worker, Pages Function, `_worker.js`, binding, Analytics, Zaraz or Workers Static Assets adapter |
| An honest report of what was and was not observed | A statement that sign-in, catalog, library or playback work on the deployed origin |

xtask never logs in, reads or stores a credential, creates a project/zone/token, changes DNS, the
production branch, bindings or billing, rolls back, or deletes a deployment.

## Identifiers, never interchangeable

| Identifier | Example | Source |
| --- | --- | --- |
| **Account ID** | 32 lowercase hex characters | dashboard → Workers & Pages → Account details; `--account` |
| **Pages project name** | `cantos-theatre` | yours; `wrangler pages project create`; `--project` |
| **Branch** | `main`, `review-pages` | `--branch`; Pages makes a deployment *production* exactly when it equals the project's production branch |
| **Environment** | `production` or `preview` | `--environment`, stated, never inferred from cwd, current Git branch or the default account |
| **Deploy target** | `theatre`, `studio` | fixed alias of `cantos-theatre`, `cantos-studio` |
| **Release artifact** | `artifacts/release-build/theatre-web/<SHA>/<CONFIG>/<ATTEMPT>` | the output of `release-build`; `--artifact` |

The mapping *account → app → project + production branch* is yours: `infra/cloudflare/projects.json`
(ignored by Git; start from `projects.example.json`). Production on a non-production branch, and
preview on the production branch (which Pages would publish live), are refused before extraction.
The offline `--dry-run` returns a failing status for a missing or disagreeing mapping while still
probing no tool, reading no credential and writing nothing.
Theatre and Studio must be different projects. Project names are global `*.pages.dev` labels: verify
availability when creating; do not assume `cantos-*` is free.

## One-time setup (operator)

Each item changes **your** Cloudflare account; nothing here is done by xtask.

1. **Pick the plan facts.** Free: 20,000 files per deployment and 25 MiB per asset. The check is on
   the decoded `.wasm` size, not its compressed size.
2. **Create two Direct Upload projects** (a Git-integrated project cannot become Direct Upload, and
   the converse also holds). `wrangler pages project create` prompts for a directory and fails in a
   non-interactive shell, so create the projects through the Pages API (no `source` means Direct Upload):

   ```bash
   # CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID come from step 3; the token is read from the environment.
   curl -sS -X POST "https://api.cloudflare.com/client/v4/accounts/$CLOUDFLARE_ACCOUNT_ID/pages/projects" \
     -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" -H "Content-Type: application/json" \
     --data '{"name":"PROJECT_NAME","production_branch":"main"}'
   ```
3. **Create an API token** (dashboard → Manage account → Account API tokens → Create custom token):
   permission **Account / Cloudflare Pages / Edit** on this account and **nothing else**: no DNS,
   Zone, Workers, R2 or Zero Trust scope. Cloudflare offers no project-scoped Pages token, so this
   token can edit every Pages project of the account; treat it accordingly. Keep it in a private
   file or your password manager, never in Git, chat, shell history or a project environment variable
   visible to every CI job.
4. **Install the pinned tools.** Node.js 22 or newer (the pinned Wrangler 4.151.2 requires it), then
   `npm ci --ignore-scripts` in `infra/cloudflare/wrangler`. xtask runs
   `infra/cloudflare/wrangler/node_modules/.bin/wrangler` and refuses any other version; it never
   uses `npx` or a global install.
5. **Write the mapping**: copy `infra/cloudflare/projects.example.json` to
   `infra/cloudflare/projects.json` and fill in your account ID, the two project names and the
   production branch.

## Routine

```bash
# 1. Build the artifact once from a clean checkout (the existing build-only command).
cargo xtask release-build theatre-web --api-origin https://api.cantos.cc

# 2. Review the plan. Starts no tool, reads no credential, writes nothing.
cargo xtask deploy cloudflare cantos-theatre --web --account ACCOUNT_ID --project cantos-theatre \
  --branch review-pages --environment preview \
  --artifact artifacts/release-build/theatre-web/SHA/CONFIG/ATTEMPT --dry-run

# 3. Publish a PUBLIC preview. A REMOTE WRITE; needs CLOUDFLARE_API_TOKEN in the environment.
cargo xtask deploy cloudflare cantos-theatre --web --account ACCOUNT_ID --project cantos-theatre \
  --branch review-pages --environment preview \
  --artifact artifacts/release-build/theatre-web/SHA/CONFIG/ATTEMPT

# 4. Production replaces what the project serves: the branch must be the project's production branch.
cargo xtask deploy cloudflare cantos-theatre --web --account ACCOUNT_ID --project cantos-theatre \
  --branch main --environment production --artifact artifacts/release-build/theatre-web/SHA/CONFIG/ATTEMPT
```

A **preview is public** (`https://<id>.<project>.pages.dev`) and does not isolate its backend: the
artifact's API origin was fixed at build time and the API's exact per-surface CORS allow-list is what
protects it. Do not add `*.pages.dev` wildcards to CORS to make a preview log in; use an operator-chosen
API environment with its own exact origin ([ADR 0041](../decisions/0041-direct-hosting-api-boundary.md)).
Current Web session cookies and fences go only to the API origin, never to the static host.
[#295](https://github.com/loveoverflowcom/cantos/issues/295) selects Web cookies on the custom
domains with migration pending; a `pages.dev` preview needs its own declared cookie/CORS/CSRF
qualification and does not inherit production acceptance. Native keeps Bearer.

### What the command checks, in order

Each failure stops the rest; unreached stages are `NOT_RUN`.

1. **Prerequisites** — Node.js, a non-empty `CLOUDFLARE_API_TOKEN` (name only; the value is never
   read), and no ambient `CLOUDFLARE_ACCOUNT_ID` that disagrees with `--account`.
2. **Policy and mapping** — account/project/branch/environment agree with the private mapping, and
   `infra/cloudflare/<app>/` (`_headers`, `404.html`, optional `_redirects`; Theatre's optional
   `.well-known/assetlinks.json` and `.well-known/apple-app-site-association`) passes validation.
3. **Wrangler** — the pinned release is installed and reports exactly 4.151.2.
4. **Release artifact** — `manifest.json` is strict (schema 1 or 2, this app's component, a full source SHA,
   an HTTPS API origin, `deployed: false`, exactly one archive) and the archive's size and SHA-256
   equal the manifest. Hashes show the bytes did not change after packaging; **they do not prove who
   built them** — provenance is a recorded claim until #277's trusted-promotion contract exists. The
   directory may hold nothing else.
5. **Staging** — a new private run directory; no `wrangler.*`, `.wrangler`, `functions/`, `.env*`,
   `.dev.vars`, `_worker.js` or `_routes.json` in the run or any ancestor; the archive is extracted
   entry by entry (regular files only; absolute, `..`, hidden, symlink, hardlink, directory,
   duplicate and over-limit entries refused).
6. **Bundle** — the tracked policy files are added, then the exact public bytes are validated: only
   allow-listed static types plus the three named root policy files (and, for Theatre, the two
   association files), the selected app's entry, Theatre's `verify-email.html` byte for byte equal to
   `index.html`, every reference present, no HTML posing as an asset, no server code, and the Pages
   limits.
7. **Project lookup** — `wrangler pages project list --json` shows the project exists and is Direct
   Upload.
8. **Intent receipt** — the staged bytes are scanned again and must have the same digest; intent is
   recorded. If it cannot be, nothing is uploaded.
9. **Publish** — one `wrangler pages deploy <staged dir>` with explicit project, branch and the
   artifact's source SHA; never retried.
10. **Readback** — the reported deployment ID is looked up with `pages deployment list --json`.
11. **HTTP smoke** — read-only HTTPS requests to the returned URL: the page (and Theatre's
    `/verify-email`, which must answer the same entry bytes), every script/WASM/CSS
    the page loads (status, media type, revalidating cache, decoded SHA-256 equal to the bundle) and
    two absence probes (`/api/...` and a missing `.wasm` must answer 404, never the page).
12. **Final receipt** — `artifacts/cloudflare/<run-id>/receipt.json` (private, ignored by Git) links
    the source SHA, manifest, archive digest, policy digest, account, project, branch, environment,
    staged bundle digest and the remote deployment, with each stage's outcome.

### Delivery policy

- Pages serves an SPA fallback (`index.html`, status 200, for every unknown path) **unless a top-level
  `404.html` exists**. Cantos routes by URL fragment, so no path needs a fallback; `404.html` is
  mandatory so `/api/*` and a missing `.js`/`.wasm`/`.css` do not answer `200 text/html`. A `_redirects`
  `200` rewrite is refused for the same reason.
- Theatre's signup email action handler is exactly `/verify-email` ([ADR 0048](https://github.com/loveoverflowcom/cantos/pull/428)). The Theatre Trunk
  build's `post_build` hook writes `verify-email.html` as a byte-for-byte copy of the processed
  `index.html`; Pages serves `name.html` at `/name`, so the path answers the entry with `200` without
  a rewrite, and `/verify-email.html` answers `308` to it. Both spellings carry the entry no-store rule.
- Theatre may publish Android App Links and iOS Universal Links association files from
  `infra/cloudflare/theatre/.well-known/`. They are delivery policy, reviewed in Git, never build
  output. The validator admits only `assetlinks.json` (one or two `handle_all_urls` statements, one per distinct exact
  `com.cantos.theatre` or `com.cantos.theatre.dev` package, each with 1–4 uppercase SHA-256
  approved signing certificate fingerprints) and `apple-app-site-association` (one `applinks` detail,
  `<TEAMID>.com.cantos.theatre`, the single component `/verify-email`), and requires
  `Content-Type: application/json` for the extensionless Apple file in `_headers`. Every other
  dot-file and every association for Studio stays refused. **Neither file is committed yet:** the
  release/dev certificate fingerprints and Apple team ID are operator facts that were not available to
  this change; add them by reviewed PR, then deploy.
- `_headers` sets `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`,
  `X-Frame-Options: DENY`, a `frame-ancestors 'none'` CSP and a camera/microphone/geolocation
  Permissions-Policy for every path, and `Cache-Control: no-cache, no-store` for `/` and `/index.html`.
  Pages **merges** matching rules instead of letting a later one win, so those entry rules are disjoint
  and `/*` sets no `Cache-Control`; assets keep Pages' revalidating default. There is no immutable
  caching until a URL-to-bytes invariant is verified, and no blanket Cache Everything rule.
- `Set-Cookie`, `Access-Control-*`, `Link` and `Refresh` headers are refused in `_headers`.
- No Analytics, Zaraz or third-party script is enabled.

### Outcomes that are not success

| Result | Meaning | What to do |
| --- | --- | --- |
| `Publish PASS`, smoke `FAIL` | Deployment confirmed and **known**; its URL did not serve the bundle as built (propagation, header or MIME problem) | Investigate; the deployment is never retried or deleted for you |
| `UNKNOWN` | Timeout, interruption, exit 0 without a recognised deployment identity, an ambiguous result or an unclassified failure after the write may have started | Read the project back (`wrangler pages deployment list --project-name … --json` or the dashboard) and compare branch, commit and time **before any retry** |
| `mismatch` | A deployment exists but not as requested (for example a preview when production was intended) | Its ID is in the receipt; decide in the dashboard. Nothing is rolled back or deleted automatically |
| `rejected` | A recognised refusal (authentication, project not found) and no deployment | Fix the token/project and run again |

Identity (deployment ID, URL, environment) is read only from Wrangler's own output file and validated
against the request; it is never constructed, never "latest", and exit code 0 alone is never success.

## CircleCI (opt-in)

Ordinary pushes and pull requests start no workflow. The Cloudflare lane is opt-in
(`web-provider=cloudflare`) and separate from the Firebase lane and from the `production` context:

```text
trigger: target=<branch or SHA in origin/develop>  component=theatre-web|studio-web|web
         web-provider=cloudflare  web-environment=preview|production  [web-branch=<branch>]
resolve target once → package (release prepare, manifest v2, no credentials) → manual approval
  → publish (context cloudflare-web, uploads the persisted artifact; never builds)
```

Create a CircleCI context named `cloudflare-web`, restricted to the Web deploy workflow, holding
only: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_THEATRE_PROJECT`,
`CLOUDFLARE_STUDIO_PROJECT`, `CLOUDFLARE_PRODUCTION_BRANCH`. Do not attach `production`, and do not
set these as project-level variables. The packaging job has no context. The publisher checks out the
artifact's exact source SHA through `release checkout`, validates the per-app v2 request/inventory
and actual archive bytes, and refuses if `origin/develop` has moved. The explicit `api-origin`
parameter reaches the credential-free producer. The publisher requires Node.js 22+, installs
Wrangler from the lockfile with the token removed from the install subprocess. The existing release
owner runs the strict dry run, persists a source/destination/job-bound intent, then invokes the
existing publisher once after CI has transferred that intent. Each app has a separate
workspace subtree to prevent additive workspace collisions. Context jobs have native repository
and develop/config filters before execution; account restrictions still require operator setup.
The Pages lane retains per-app approvals and provider receipts. `release pages-intent` binds the
exact v2 artifact, account/project/branch/environment and publisher job; CI persists it before
`release pages-promote` calls the existing Pages publisher. A rerun job, existing attempt, wrong
source/digest/component, stale develop or changed mapping refuses. Per-app outcomes preserve known
remote identity separately from command/smoke failure. There is no combined API/Web activation:
legacy `api`/`all` requests fail without credentials, and `all` starts no Pages job. An uncertain
job loss still requires provider reconciliation before a new pipeline. Real CI execution remains NOT_RUN.

## Acceptance status

Observed on 2026-10-19 from source `f76ad2b` with Wrangler 4.151.2 and an operator-supplied token, on the
operator's account (two new Direct Upload projects, `cantos-theatre` and `cantos-studio`, production
branch `main`):

| Observed (preview deployments, public URLs) | Result |
| --- | --- |
| Full flow for each app: verify, stage, bundle, project lookup, intent, publish, readback, HTTP smoke, receipt | PASS (Theatre `e0612871-04c6-486c-b2a5-0ca502e61798`, 8 smoke requests; Studio `39258edf-d235-41e8-8241-7c1a17411c24`, 10 requests; branch `review-pages`, environment `preview`) |
| Wrangler output-file entry, `pages project list --json` and `pages deployment list --json` shapes | Matched what the interpreters expect; the flow confirmed identity from the output file and found the ID in the listing |
| `404.html` precedence | Unknown paths, `/api/v1/theatre/x`, `/missing.wasm`, `/missing.js` and `/_headers` answer **404 `text/html`** (the 404 page), never `200` |
| Media type, cache, compression, validators | `.wasm` is `application/wasm`; assets `cache-control: public, max-age=0, must-revalidate` with a weak `etag`; `content-encoding: br` negotiated |
| Headers | `no-referrer`, `nosniff`, `DENY`, CSP `frame-ancestors 'none'`, Permissions-Policy on every response including 404s and redirects; `/` is `no-cache, no-store` |
| Smoke byte equality | Served `index.html`, JS, WASM and CSS decode to the SHA-256 of the staged bundle |

Surprises worth knowing (observed, not assumed):

- Pages adds `access-control-allow-origin: *` to every static response. It covers public static bytes
  only (no cookies, no Bearer is ever sent to this host) and does not touch the API's exact per-surface
  CORS allow-list, but do not treat it as configurable here: `_headers` still refuses `Access-Control-*`.
- `/index.html` and `/404.html` answer `308` (to `/` and `/404`); `/` serves the entry. The smoke checks `/`
  and, for Theatre, `/verify-email` (expected from Pages' documented `name.html` serving; not yet
  observed on a live deployment).
- Pages serves the 404 page with `cache-control: no-store`.

| Not accepted | Needs |
| --- | --- |
| A **production** deployment (branch `main`) of each app and the `production` readback | An explicit go-ahead; it replaces the project's live site (still only `*.pages.dev`) |
| Pages' real file-count and 25 MiB per-asset enforcement (sizes are far below: `.wasm` 2.8 MB and 5.9 MB) | A bundle near the limit; not expected |
| CircleCI execution of the lane (config validated with `circleci config validate`, not run) | `cloudflare-web` context, approval and a budgeted run |
| Browser, API, auth, custom domain, DNS and cutover | The [rollout runbook](cloudflare-rollout.md) under separate authority |

Public preview URLs stay reachable until the deployments are deleted in the dashboard (this command never
deletes). A stale development `dist` of Theatre has a 97 MB debug `.wasm`: only the release build is
deployable, and the limit check would refuse the debug one.

Observed 2026-10-19 in the owner's account dashboard before provisioning (read-only): no Pages project; one
Git-integrated Worker named `cantos` with a failed build. It is unrelated to this lane and was not touched.
Treat this as a dated observation, not current state.
