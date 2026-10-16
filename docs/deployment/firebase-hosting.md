# Firebase Hosting for the Cantos Web apps

Operator runbook for `cargo xtask deploy firebase`: build **one** Leptos Web app (Cantos Theatre or
Cantos Studio) and publish its static bundle to **Firebase Hosting**, locally and by hand, from a
Mac or Linux machine. The command contract is the
[xtask README](../../xtask/README.md#deploy-firebase); this page is the setup, the routine, the
verification and the recovery.

> **Status — implemented, with live acceptance `NOT_RUN`.** The command, its tests and a local
> Hosting-emulator check exist. No real preview or live deployment has been performed: that needs
> your Firebase project and your go-ahead. See [what is and is not accepted](#acceptance-status).
> Creating a site or running a deploy changes **your** Firebase project; nothing here does it for
> you, and nothing here changes DNS, billing or IAM.

## What this is, and what it is not

| It is | It is not |
| --- | --- |
| A manual, explicit command: `deploy firebase <app> --web --project … --channel …` | A GitHub Action, a deploy-on-push hook, or something `build`, `run`, `verify`, `preflight` or `ci` ever calls |
| Static hosting of one app's HTML, CSS, JS and WASM, one app at a time | Hosting for the Rust API, PostgreSQL, Kanidm or object storage (they stay where they are) |
| Independent of Android distribution, backend deployment and the other web app | A way to build `all`, start services, run migrations or run a verification campaign |
| Static delivery with an honest report of what was and was not observed | A statement that sign-in, catalog, library or playback work on the deployed origin |

No Firebase application SDK, Analytics, Firestore, Firebase Authentication, `google-services`
configuration, Cloud Functions, Cloud Run proxy or App Hosting adapter is involved. The CLI
authenticates by your own `firebase login` (or Application Default Credentials you chose to
configure); xtask never logs in, reads, prints or stores a credential. Do not create or commit a
service-account JSON, use the legacy `login:ci` token or add Firebase SDK configuration just to host
static files.

### Four identifiers, never interchangeable

| Identifier | Example | Where it comes from |
| --- | --- | --- |
| **Project ID** | `cantos-prod-1234` | `firebase projects:list`; passed as `--project` |
| **Hosting site ID** | `cantos-theatre-1234` | globally allocated; yours, from `hosting:sites:list` or `…:create` |
| **Deploy target** | `theatre`, `studio` | local alias fixed by the repository (`cantos-theatre` → `theatre`, `cantos-studio` → `studio`) |
| Android Firebase App ID | `1:123:android:abc` | App Distribution only; unrelated to Hosting |

The mapping *target → site* is yours: it lives in `services/firebase-hosting/.firebaserc`, ignored by
Git. The repository never contains a real project or site ID.

## One-time setup

Read-only commands are marked **read**; **WRITE** commands change your Firebase project and are
only needed if the site does not already exist.

```bash
# One-time operator installation (Node.js supported by the npm package is required).
# 15.34.0 is the version this feature was verified against (the same pin as App Distribution),
# not a claim that it is the latest. xtask never installs or upgrades it.
npm install -g firebase-tools@15.34.0
firebase --version

# Authenticate explicitly. xtask never opens a login flow.
firebase login
# On an SSH or headless host:
firebase login --no-localhost

# read: the project must already exist (create it in the console if it does not)
firebase projects:list

# Use your real project ID. Do not copy a sample ID into production.
PROJECT_ID='REPLACE_WITH_YOUR_PROJECT_ID'

# read: site IDs are allocated globally, so list yours before choosing names
firebase hosting:sites:list --project "$PROJECT_ID"

# WRITE, only if a required site does not exist. Pick available IDs; the project's default
# site (its project ID) can be reused for Theatre.
firebase hosting:sites:create THEATRE_SITE_ID --project "$PROJECT_ID"
firebase hosting:sites:create STUDIO_SITE_ID --project "$PROJECT_ID"
```

Then tell the repository which site each target means. Either copy the example and edit it:

```bash
cp services/firebase-hosting/firebaserc.example services/firebase-hosting/.firebaserc
# edit the three REPLACE_WITH_* values
```

or let the CLI write it (run from the directory that holds `firebase.json`):

```bash
cd services/firebase-hosting
firebase target:apply hosting theatre THEATRE_SITE_ID --project "$PROJECT_ID"
firebase target:apply hosting studio STUDIO_SITE_ID --project "$PROJECT_ID"
cd ../..
```

xtask requires each target to map to **exactly one** site in the project you pass, and the two
targets to name **different** sites. A missing file, a different project's mapping, two sites for one
target, or one site for both targets is a refusal before any tool runs. xtask never edits this file.
It reads it as strict JSON (no comments).

Notes:

- The Firebase CLI also needs the project's **default** Hosting site to exist before it deploys, even
  though the configuration names other sites, and `firebase.projects.get` on the project.
- `firebase init hosting --project "$PROJECT_ID"` is optional bootstrap only when no reviewed
  configuration exists; here the tracked [`firebase.json`](../../services/firebase-hosting/firebase.json)
  already is that configuration, so do not run it in this directory: it can overwrite files and
  replace the multisite configuration. Decline GitHub integration if you ever see it.
- Preview channels and live releases are public the moment they succeed. Nothing in this setup makes
  a site private.

## Routine deployment

```bash
# 1. Review the plan. Starts no tool, reads no credential, writes nothing.
cargo xtask deploy firebase cantos-theatre --web \
  --project "$PROJECT_ID" --channel review-hosting --dry-run

# 2. Build Theatre Web once, validate the bundle, publish a PUBLIC preview. A REMOTE WRITE.
cargo xtask deploy firebase cantos-theatre --web \
  --project "$PROJECT_ID" --channel review-hosting --expires 7d

# 3. A deliberate LIVE release of Theatre only. A REMOTE WRITE that replaces what the site serves.
cargo xtask deploy firebase cantos-theatre --web --project "$PROJECT_ID" --channel live

# 4. Studio is independent: it never builds or deploys Theatre or Android.
cargo xtask deploy firebase cantos-studio --web --project "$PROJECT_ID" --channel live
```

- **App, `--web`, `--project` and `--channel` are all required** and never defaulted. `--channel
  live` is the only way to reach the live release; any other value is a *preview channel*. A spelling
  the Firebase CLI would quietly send somewhere else (`Live`, `feature/x`, `a_b`, `issue#1`) is
  refused rather than normalized.
- `--expires` (`30m`, `12h`, `7d`; at most `30d`) applies only to preview channels. Without it the
  CLI's default of 7 days applies to a new channel, and an existing channel is extended to at least
  that. It is refused with `--channel live`.
- Targets other than the two Web apps (`cantos-api`, `cantos-forge`, `all`, mobile) are rejected
  before anything runs. There is no `--no-build`, artifact path, deploy-all, automatic promotion or
  automatic rollback.

### What a run does

Each stage stops the run on failure; unreached stages print `NOT_RUN`. Nothing is retried.

| # | Stage | Does | Writes remotely? |
| --- | --- | --- | --- |
| 1 | local prerequisites | Rust, the wasm32 target, Trunk and Git are installed; no inherited `TRUNK_*` variable could redirect the build | no |
| 2 | Hosting configuration | the tracked policy is acceptable; `.firebaserc` maps the target to one site in this project | no |
| 3 | Firebase CLI | `firebase --version` is 15.x and at least 15.34.0 | no |
| 4 | run record | a private `artifacts/hosting/<run-id>/` with an **effective** `firebase.json` and `.firebaserc` for this one target | no |
| 5 | Hosting destination | read-only `hosting:sites:list` in the explicit project: the mapped site must exist. Not proof of deploy permission | no (read) |
| 6 | release build | one locked release Trunk build of the selected app into the run's own `dist` | no |
| 7 | bundle validation | the exact public bytes are validated and fingerprinted; the source must not have changed during the build | no |
| 8 | intent receipt | the bundle must be unchanged since validation; intent is recorded | no |
| 9 | Hosting release | **one** `hosting:channel:deploy` or `deploy` call | **yes** |
| 10 | HTTP smoke | read-only HTTPS requests to the URL the CLI reported | no (read) |
| 11 | final receipt | the observed outcome | no |

Every Firebase subprocess after the initial `firebase --version` probe gets an explicit `--project` and the run's own `--config`, and runs
with the private run directory as its working directory, so a developer's active project, other
products and the `firebase-debug.log` cannot leak into the checkout. It never reaches the other web
app, Android, the API, services, migrations, the broad preflight or App Distribution.

The report uses `PASS`, `FAIL`, `NOT_RUN` and `UNSUPPORTED`, and the per-run `receipt.json` records
source commit (and a content fingerprint if the tree is dirty), tool versions, project, site, target
and channel, the bundle's file manifest and digest, the release identity the CLI actually printed
(version, URL, expiry), the smoke observations and each stage's outcome with a timestamp. It holds no
credential or account address. Keep it: it is how a lost response is reconciled.

### The bundle that is uploaded

Only the selected app's Trunk output is deployable. A bundle is refused if it contains a symbolic
link, a hidden file, a file named like configuration or a record (`firebase.json`, `receipt.json`,
`package.json`), a type that is not an allow-listed static web file (source maps, logs, keys, audio and
unknown types), an empty file, or an HTML page named like a `.js`, `.css` or `.wasm`; if
`index.html` is not the selected app's page, loads no `*.wasm` and script named for it, references a
file that is absent or outside the bundle, or pins an asset by `integrity="sha384-…"` that does not
match the bytes; or if the other app's artifacts are present. The directory is fresh per run (never an
old `dist`), and it is scanned again immediately before the upload.

## Hosting behavior

The tracked policy, [`services/firebase-hosting/firebase.json`](../../services/firebase-hosting/firebase.json),
is short on purpose, and xtask refuses to deploy a version of it that weakens any of these:

- **Two independent targets**, `theatre` and `studio`, each its own site. Static Hosting only: no
  Functions, Cloud Run, Firestore, App Hosting, hooks or redirects. A `site` key would bypass your
  mapping and is refused.
- **No rewrites.** Both clients navigate by hash and query on `/` and call the API with absolute
  `/api/v1/…` paths, so there is no path-based client route to rewrite. A blanket
  `** → /index.html` rewrite would answer a missing `.wasm` or `.js` and every `/api/*` request with
  `200` and the page; xtask refuses any rewrite whose source is not a literal route outside `/api`.
  Add a rewrite only together with a real path-based route and a test of the deployed result.
- **Revalidating `Cache-Control` for every path** (`no-cache`). Hosting's own default is
  `max-age=3600`, which would let a stale `index.html` outlive the assets it names. No rule may grant
  long-lived caching: a hash in a filename is not evidence that its bytes never change. On the real
  release bundles each `*.css` name equals the SeaHash of its bytes, but the `*.js` and `*_bg.wasm`
  names of one app carry the *same* hash although their bytes differ (the name identifies the build
  input). The cost is a conditional request per file per load; granting immutability for a class of
  assets needs its own evidence and is a follow-up. (The VPS edge, `services/edge`, keeps its own,
  separate policy.)
- **MIME types** are Hosting's. Observed on the local emulator: `.wasm` →
  `application/wasm`, `.js` → `application/javascript`, `.css` → `text/css`, `.json`, `.svg`,
  `.woff2` as expected. Verify the deployed responses with the smoke below.

### What a preview or live site can and cannot do

The static pages load. Their requests to `/api/v1/…` go to the Hosting origin, which has no API, and
answer `404`, so the clients show their honest unavailable states. **That is not login, catalog,
library or playback acceptance.** A preview URL is also **public** (unlike App Distribution, nothing
gates testers), expires on its own and is not isolated from any backend: never upload private Studio
content or credentials, and never point a preview at production services. `--no-authorized-domains`
keeps the CLI from editing Firebase Authentication's authorized domains as a side effect.

## Verify a deployment

xtask's own `HTTP smoke` row requests the page, every script, WebAssembly module and stylesheet the
page loads, a path that does not exist and a path under `/api/`. It passes only if the assets answer
`200` with the right media type, a revalidating `Cache-Control` and **exactly the validated bytes**
(compared by SHA-256), and the other two answer `404` rather than the page. It needs `curl`; without
it the row is `NOT_RUN`. It speaks for static delivery only.

By hand, against the URL the report printed:

```bash
URL='https://SITE--CHANNEL-HASH.web.app'    # or the site's default URL for a live release
curl -sS -D - -o /dev/null "$URL/"                                  # 200, text/html, no-cache
curl -sS -D - -o /dev/null "$URL/cantos-theatre-HASH_bg.wasm"       # 200, application/wasm
curl -sS -D - -o /dev/null "$URL/definitely-missing.wasm"           # 404, not the page
curl -sS -D - -o /dev/null "$URL/api/v1/theatre/works"              # 404 on Hosting (no API)
```

Then open the URL in a browser, check the console for failed requests other than `/api/*`, and
confirm the unselected site is unchanged (its URL still serves what it did).

### Optional: a Hosting-only emulator check (local, no remote write)

The Hosting emulator applies the same configuration semantics (headers, rewrites, MIME database)
without touching your project. Use a `demo-` project ID so nothing can reach a real one:

```bash
cargo xtask build cantos-theatre --web --release
cat > services/firebase-hosting/.firebaserc <<'EOF'
{ "targets": { "demo-cantos": { "hosting": { "theatre": ["demo-theatre"], "studio": ["demo-studio"] } } } }
EOF
firebase emulators:start --only hosting --project demo-cantos \
  --config "$PWD/services/firebase-hosting/firebase.json"
# Theatre is served at http://127.0.0.1:5000 (Studio at :5005 once built); probe it with curl as above.
```

Remove or replace that `.firebaserc` with your real mapping afterwards. The emulator proves the
policy, not Hosting's CDN, TLS or propagation.

## API origin and authentication: a separate gate

Static upload alone cannot connect the clients to an API. The development proxies in both
`Trunk.toml` files are dev-server behavior and are not part of the static build. Firebase Hosting's
backend rewrites target Cloud Functions or Cloud Run, not an arbitrary external server, and a
redirect is not an equivalent same-origin proxy. The clients use relative `/api/v1/…` URLs and
same-origin credentials, so moving them to another API origin is a client change plus a
security design, not configuration.

Before claiming an integrated deployment, review these and record the choice in an ADR:

1. **Custom web and API hosts under one registrable domain you own.** Firebase serves the web hosts
   and the VPS serves the API host. They are different origins: explicit API URL resolution,
   credentialed `fetch`/CORS, CSRF and origin validation and OIDC callback configuration are all
   required. Same-site is not same-origin. Keep Studio and Theatre sessions and authorization
   separate; do not widen a cookie `Domain` to share privileges.
2. **Free `*.web.app` web origin with an external API.** Fine for static or anonymous inspection;
   authenticated access is cross-site and subject to third-party-cookie policy. `SameSite=None;
   Secure` plus CORS alone do not establish reliable login. No wildcard credentialed origins, and no
   disabling browser protections.
3. **Keep the existing same-origin VPS edge for authenticated production** and use Firebase only for
   static previews until a reviewed contract exists. A Cloud Run proxy is not added silently.

Do not invent an environment variable and assume the WASM reads it: an API-origin setting needs an
actual client reader, validation, tests and coverage of catalog, library, artwork, auth navigation,
callbacks and every protected request. Never expose the development-only API to satisfy this gate.
Until it passes, the VPS route in [`services/edge`](../../services/edge/README.md) remains the
authenticated alternative and is untouched by this feature.

## Recovery and reconciliation

- **A failed or uncertain run exits nonzero** with a typed reason and keeps the stage vocabulary.
  `REFUSED` means the CLI or service refused with a recognised reason and reported no release;
  `UNKNOWN` means the write may have happened (timeout after 20 minutes, interruption, changed or
  unrecognised output, an unclassified failure).
- **Reconcile before any replay.** Stopping a local process does not undo a remote upload. A failed
  preview may have created its channel; a failed deploy may have uploaded a version that was never
  released. Open the Hosting page of the site in the Firebase console, look at the release history
  and channels, and use the version named in `receipt.json`. Rebuilding the current checkout is
  **not** restoring a known previous release: it uploads new bytes.
- **Rollback is manual.** Use the console's rollback for a live site (release history), or
  `firebase hosting:clone` to copy a known version or channel to live, as the official documentation
  describes. xtask never rolls back, promotes, or deletes channels, releases, sites or the VPS
  deployment.
- **Retention.** Preview channels expire by themselves (CLI default 7 days, at most 30). Hosting keeps a
  limited release history, set in the console's release-history settings; check the current default
  there rather than relying on this page, and note the version ID of any release you may need to
  return to.
- **A successful release followed by a failed smoke or a failed receipt write** is a known remote
  release with a separate local failure. The report says so and prints a recovery pointer; record the
  outcome by hand and do not rerun to regenerate it.
- Raw `firebase` commands are diagnostic primitives. For orientation, the two release invocations
  xtask builds are below; note the different `--only` syntax. Routine deployment goes through xtask,
  which owns building, validation and the effective configuration.

```bash
# Preview (--only takes the bare target name; the preview command refuses `live`):
firebase hosting:channel:deploy review-hosting --only theatre \
  --project "$PROJECT_ID" --config /ABS/PATH/firebase.json \
  --expires 7d --no-authorized-domains --non-interactive --json

# Live (--only takes hosting:TARGET):
firebase deploy --only hosting:theatre \
  --project "$PROJECT_ID" --config /ABS/PATH/firebase.json --non-interactive --json
```

## Troubleshooting

| Message | Meaning and next step |
| --- | --- |
| `no services/firebase-hosting/.firebaserc` | create the mapping ([setup](#one-time-setup)) |
| `has no targets entry for this project` | the mapping is for another project ID; xtask never borrows it |
| `both deploy targets map to site …` | Theatre and Studio need two distinct sites |
| `firebase-tools X is outside the documented line` | install 15.34.0 or a later 15.x yourself |
| `inherited Trunk environment variables …` | `unset` the named `TRUNK_*` variables for this command |
| `the mapped Hosting site is not in this project` | create the site, or fix `.firebaserc` |
| `the project has no default Hosting site` | both release commands look up the project's default site first, even though the config names yours; set up Hosting in the console so the default site (named like the project ID) exists |
| `authentication was rejected` | `firebase login` (or `--no-localhost`) |
| `lacks a required Hosting permission` | the account lacks a Hosting permission: the site lookup needs `firebasehosting.sites.get`, a deploy needs `firebasehosting.sites.update` |
| `bundle validation … FAIL` | the finding names the file; the bundle is never uploaded |
| `the source tree changed while the build ran` | do not edit the checkout during a deploy; rerun |
| `UNKNOWN remote outcome` | [reconcile](#recovery-and-reconciliation) before rerunning |
| `HTTP smoke … FAIL` after a confirmed release | the release is known; read the named probes, check the URL by hand |

## Acceptance status

| Gate | Status |
| --- | --- |
| Command contract, plan, request validation, configuration policy, bundle validation, Firebase adapter, receipt, coordinator ordering, redaction | tested with fixtures, fakes and local files (`mock-integration-tested`) |
| Real release build and validation of both apps | `integration-tested` on a capable host (real Trunk, real files) |
| Tracked policy under the Hosting emulator (headers, MIME, 404 behavior) | observed locally (emulator, not production Hosting) |
| One real preview deployment per app, one real live deployment, unselected site unchanged | **`NOT_RUN`**: needs your Firebase project and explicit go-ahead |
| Firebase CLI response shapes from a real run | **`NOT_RUN`**: fixtures are reconstructed from the CLI sources |
| HTTP smoke against real Hosting (media types, headers, propagation) | **`NOT_RUN`** |
| API origin and authentication | **separate gate**, not delivered by this feature |

The ledger, evidence, owners, prerequisites and exact commands are in
[the evidence record](../evidence/issue199-firebase-hosting.md). Each unaccepted part is also marked
at its place in the source with `ACCEPTANCE(NOT_RUN)`.

## Official references

- [Firebase CLI setup and authentication](https://firebase.google.com/docs/cli)
- [Hosting multisites](https://firebase.google.com/docs/hosting/multisites) and
  [deploy targets](https://firebase.google.com/docs/cli/targets)
- [Hosting configuration: files, rewrites, headers](https://firebase.google.com/docs/hosting/full-config)
- [Local testing, preview and live deployment](https://firebase.google.com/docs/hosting/test-preview-deploy)
- [Channels, releases, versions and rollback](https://firebase.google.com/docs/hosting/manage-hosting-resources)
- [`hosting:channel:deploy` in firebase-tools v15.34.0](https://github.com/firebase/firebase-tools/blob/v15.34.0/src/commands/hosting-channel-deploy.ts)
- [MDN: cookie behavior](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Cookies)
