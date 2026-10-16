# Firebase Hosting deploy of Theatre and Studio Web — issue 199

This records the implementation boundary and evidence of `cargo xtask deploy firebase` for
[#199](https://github.com/loveoverflowcom/cantos/issues/199), based on `develop@2e3f29a` plus this
change. **Live acceptance is `NOT_RUN`** (see below): no preview or live release was created, no
Firebase or DNS resource was touched and no Hosting URL was observed. The command contract is the
[xtask README](../../xtask/README.md#deploy-firebase); the operator steps are the
[Firebase Hosting runbook](../deployment/firebase-hosting.md). It serves static files only; the
API-origin and authentication contract for a deployed origin is a separate gate this change does not
deliver. The issue's own baseline was `develop@159f57c`; #198 and #200 landed since and do not touch
the files involved.

## Review boundary

- Delivers: the opt-in `deploy firebase` command with a pure plan and offline `--dry-run`; the
  tracked Hosting policy (`services/firebase-hosting`) with the operator-owned site mapping; a
  policy validator and a run-specific effective configuration; a read-only site lookup before the
  build; one locked release build of the selected app; exact-bytes bundle validation and a manifest;
  one never-retried CLI release; a closed remote-outcome vocabulary; an intent-then-outcome per-run
  receipt; a read-only HTTP smoke; redaction; the shared helpers extracted from `distribute`
  (`run_files`, `firebase_cli`, `build::release_bundle`); the runbook, docs and preflight
  classification; and in-code `ACCEPTANCE(NOT_RUN)` markers for everything below.
- Does not deliver: a real preview or live deployment of either app; captured (not reconstructed)
  Firebase CLI responses; any observation of real Hosting responses; the API origin and
  authentication contract; long-lived caching of hashed assets; promotion, rollback or deletion
  automation; browser or device acceptance; any GitHub Actions, Firebase SDK, Functions, Cloud Run
  or App Hosting work.
- `Closes #199`: no. Live acceptance is pending, so the PR says `Part of #199`.

## Claim ledger

| Claim | Oracle | Level reached | Result |
| --- | --- | --- | --- |
| `deploy` is explicit; no gate, CI step or build plan (any app, surface or profile) contains a Firebase, Hosting or deploy command | a test walks every gate, CI step and build plan | example-tested | held |
| app, `--web`, `--project` and `--channel` are required; `cantos-api`, `cantos-forge`, `all`, `cantos`, mobile and `--no-build`/artifact/`--site`/`--only` are rejected; Studio mobile is a usage error and Theatre mobile `UNSUPPORTED`, as for `build` | request and clap-parse tables | example-tested | held |
| a live-like spelling (`Live`, ` live`, `LIVE`) and any channel the CLI would rewrite (`/ : _ #`, uppercase) are refused; `live` is exactly `live`; expiry is CLI syntax, positive, at most 30 days and refused for live | request tables | example-tested | held |
| the dry run probes no tool, starts no process, creates no directory and shows an unresolved mapping instead of guessing | a `ToolProbe` that panics, a recording runner, an absence check, a real invocation of the plan | example-tested + observed | held |
| the tracked policy is accepted; only `hosting`; no `site`, hook, redirect, i18n or non-static rewrite; no rewrite that is not a literal route outside `/api`; a `**` revalidating rule is required; no long-lived `Cache-Control`, including a rule that tries to cache Trunk-hashed assets | policy tables over the real tracked file and hostile variants | example-tested | held |
| `.firebaserc` must map the target to exactly one site in the requested project; the two targets to different sites; another project's mapping and project aliases are never used | mapping tables | example-tested | held |
| the effective `firebase.json`/`.firebaserc` hold exactly one target, one site, one project and a run-relative `public`; every Firebase subprocess gets explicit `--project`, `--config` and the run directory | coordinator and adapter tests over argv | mock-integration-tested (named fakes: host, capture, runner) | held |
| prerequisites, a missing or incompatible Firebase CLI and an inherited `TRUNK_*` variable stop everything before any run file or build; only the version probe may run | per-case coordinator tests | mock-integration-tested | held |
| a failing site lookup (auth, permission, unknown site, timeout, changed output) stops before the build and the upload | coordinator table over fixture-shaped results | mock-integration-tested | held |
| the build uses the run's own fresh `dist`; the validated, re-validated and uploaded directory is that one; a source change during the build refuses the bundle | coordinator tests recording build argv and scan paths | mock-integration-tested | held |
| bundle: only regular, visible, allow-listed, non-private files; no symlink (a link swapped in after the check is refused by the open); the selected app's page and `*.js`/`*_bg.wasm`; no other-app artifact; every reference present and inside the bundle; `integrity` pins match; no HTML named as an asset; `.wasm` magic; local size and count bounds; findings bounded and never echo content | bundle tests; scan tests over real temporary trees (links, FIFO, unreadable, deep, many, oversized index) | example-tested + integration-tested (local files) | held |
| a bundle that changes after validation, or stops validating, is not uploaded; intent is recorded before the write and its failure prevents it | coordinator tests with a changed scan and a failing persist | mock-integration-tested | held |
| only exit 0 plus a result naming exactly the selected target, site and channel (an `https` Hosting URL for a preview, a version of exactly this site for live) is a success; a second result entry, a success document on a failing exit, changed output, timeout, interruption and lost output are *unknown*; a recognised refusal is *rejected*; nothing is retried and no identity is invented | adapter and coordinator tests | example-tested (fixtures reconstructed from the CLI sources) | held |
| a confirmed release followed by a failed final receipt keeps a recovery pointer; a failed smoke is a separate local failure and the release stays known | coordinator tests | mock-integration-tested | held |
| tokens, addresses and foreign links from CLI output never reach the report or the receipts | leak tests with hostile output | example-tested | held |
| the smoke is read-only HTTPS (no redirect following, no body, header or credential flags); it passes only for `200`, the right media type, a revalidating `Cache-Control` and the SHA-256 of the validated bytes, and `404` (not the page) for a missing `.wasm` and an `/api/` path; a missing `curl` is `NOT_RUN`, not a pass | smoke and coordinator tests, including real response headers captured from the Hosting emulator | example-tested | held |
| extracting the shared helpers did not change `distribute firebase` | its 88 existing tests pass unchanged | example-tested | held |
| the tracked policy behaves as designed under the Hosting emulator: `/` revalidated HTML, `application/wasm`, scripts and styles with the right types, every non-bundle path `404` (never the page), the served WASM byte-identical to the file | manual run, below | integration-tested (Hosting emulator, local; not production Hosting) | held |
| both real release bundles validate; each is refused as the other app; one appended byte in a real script breaks the real `integrity` pin | ignored host test, below | integration-tested (real Trunk, local files) | held |

Non-vacuity: eighteen hand-made mutants of the invariants above (live-like spelling accepted, zero
expiry, any `Cache-Control`, shared site, another project's mapping borrowed, catch-all rewrite,
integrity not checked, HTML masquerade, hidden files, a success document read from a failing exit,
a second preview entry, a preview URL not tied to the site, pre-upload digest not compared, source
change ignored, intent-persist failure ignored, smoke ignoring bytes, smoke accepting a page for a
missing path, scan following links) were each killed by a failing test and the sources restored
byte for byte.

## Real local build and validation (host-capable)

Host: macOS (Darwin 25.5.0, arm64), Rust 1.98.1, Trunk 0.21.14, `wasm32-unknown-unknown`, working
tree `develop@2e3f29a` plus this change. `cargo test -p xtask --locked -- --ignored --nocapture
real_release_bundles_validate` ran the shared `build::release_bundle` command for each app into a
fresh directory, scanned it with the real host scan and validated it. It starts no Firebase process
and makes no remote request.

| App | Files | Bytes | Manifest SHA-256 | Page loads |
| --- | --- | --- | --- | --- |
| `cantos-theatre` | 32 | 2 444 616 | `d4e51d0e7d1be6b44e379e9a2499d22cbb1c79b1175d1fc5d45f7de1b07b8653` | `cantos-theatre-a34b29a2a671bedf.js`, `…_bg.wasm`, three stylesheets |
| `cantos-studio` | 7 | 5 505 340 | `a9da21d1747d6794f495d0433c283ab3e37e5d7d0fbadc0f4720c428100148f7` | `cantos-studio-11df3230f3449e82.js`, `…_bg.wasm`, four stylesheets |

The same source produced the same hashed names in three separate builds. Each real bundle was
refused as the other app, and appending one byte to a real script produced an `IntegrityMismatch`
against the `integrity="sha384-…"` pin in the real `index.html`.

**Finding — a filename hash is not byte identity.** On the real Theatre bundle the SeaHash of each
stylesheet's delivered bytes equals the hash in its name (`foundation-951b963d9814b6c.css`,
`theatre-ca2c9510ca313bc0.css`, `tokens-3708958437b0fef6.css`), but `cantos-theatre-a34b29a2a671bedf.js`
and `cantos-theatre-a34b29a2a671bedf_bg.wasm` carry the *same* hash although their bytes differ (their
SeaHashes are `7b7ce1ce89da524b` and `51d5a295159d0498`): the name identifies the build input, not the
delivered file. That is why the policy refuses any long-lived `Cache-Control` and why the issue's
"a hash-looking filename alone is not evidence" holds here. The existing VPS edge
(`services/edge/Caddyfile`, `cantos_spa`) grants `immutable` to these names on the same premise; this
change does not touch it, but the premise deserves its own review.

## Hosting emulator observations (local, no remote write)

`firebase emulators:start --only hosting --project demo-cantos` (firebase-tools 15.33.0, installed
here) served both release bundles under the tracked `firebase.json`:

| Request (Theatre, port 5000) | Result |
| --- | --- |
| `/`, `/index.html` | `200`, `text/html; charset=utf-8`, `Cache-Control: no-cache` |
| `*_bg.wasm` | `200`, `application/wasm`, `no-cache`; body SHA-256 equals the file's |
| `*.js` / `*.css` | `200`, `application/javascript; charset=utf-8` / `text/css; charset=utf-8`, `no-cache` |
| `assets/provenance.json`, `*.woff2`, `*.svg` | `200`, `application/json`, `font/woff2`, `image/svg+xml`, `no-cache` |
| `/__cantos-smoke-missing.wasm`, `/missing.js`, `/some/deep/route` | `404`, `text/html`, never the page |
| `/api/v1/theatre/works` | `404` (no API on Hosting; not the page) |
| `/.env`, `/firebase.json` | `404` |

Studio (port 5005) answered the same way for its page, scripts, WASM, stylesheets, a missing `.wasm`
and `/api/v1/studio/session`. The captured `curl --dump-header` output for six of these is committed
as [`xtask/fixtures/hosting-emulator`](../../xtask/fixtures/hosting-emulator/README.md) and drives the
smoke judge's tests. The emulator shares the configuration semantics, not Hosting's CDN, TLS, HTTP/2
or propagation, so this is not production evidence.

## Firebase CLI contract

The adapter targets `firebase-tools` 15.x and the pin **15.34.0**, read from the CLI sources: the
installed 15.33.0 (`lib/commands/hosting-channel-deploy.js`, `hosting-sites-list.js`, `deploy/…`,
`hosting/config.js`, `detectProjectRoot.js`, `rc.js`, `command.js`, `bin/cli.js`) and the v15.34.0
`hosting-channel-deploy.ts`. The 15.33.0→15.34.0 comparison touches `command.ts` (analytics error
handling) and `deploy.ts` (a Cloud Run target) only; the Hosting commands are unchanged. Facts used:

- with `--json` the console loggers are off and stdout is one `{"status":…}` document; a failure also
  exits nonzero (`bin/cli.js`, `command.js`); the CLI leaves `firebase-debug.log` in its working
  directory on failure, which is why the run directory is the working directory;
- `--config` makes the config's directory the project root, so `public` and `.firebaserc` resolve
  beside it (`detectProjectRoot.js`, `rc.js`), which is what lets a run-specific pair isolate one target;
- `hosting:channel:deploy` rewrites `/ : _ #` to `-`, refuses a `live` channel after `trim().toLowerCase()`,
  accepts `--expires` as `^\d+[hdm]$` up to 30 days (default 7), creates or re-times the channel
  *before* the upload, takes `--only` as bare target names and returns `{target, site, url, version,
  expireTime}` keyed by target; `--no-authorized-domains` skips the Firebase Auth domain sync;
- `deploy --only hosting:<target>` returns `{hosting: <version resource name>}` and no URL;
- `hosting:sites:list` returns `{sites}` with `projects/<id-or-number>/sites/<site>` names.

**The Hosting response fixtures are reconstructed from these sources, not captured** from a real
deployment (see `xtask/fixtures/firebase-tools-15.34.0/README.md`). The adapter is fail-closed: a
shape it does not recognise is an *unknown* outcome, never a success. The locally installed 15.33.0 is
below the 15.34.0 pin, so a real run on this machine stops at the `Firebase CLI` stage as
`UNSUPPORTED`; that is by design and the reason no real run was attempted here.

## The issue's acceptance checklist

| Criterion | Status | Evidence |
| --- | --- | --- |
| CLI/help/plan tests (each app, explicit project/channel, target→site, preview/live argv, expiry, malformed and live-like channels, unsupported surfaces); dry run without subprocess, remote request, output or installed tools | met | request, mod and coordinator tests; `--help` and a real `--dry-run` run |
| Reuses the selected-app release plan and the captured process boundary; builds once; never reaches the other app, Android, API, services, migrations, preflight or App Distribution | met | `build::release_bundle` shared with `web-performance`; coordinator test over every command |
| Read-only destination/auth failures stop before build/upload; no default project, no both-targets selection, no added products/hooks, no Auth-domain mutation | met | coordinator and policy tests; `--no-authorized-domains` in argv |
| Bundle/config tests: missing assets, HTML as API/WASM, path escape/symlink, stale output, concurrent runs, source changes, cache eligibility, secret/log exclusion | met | bundle, host, policy and coordinator tests; unique run directories (`run_files`) |
| Sanitized adapter fixtures: preview and live success, auth/site rejection, channel-created/upload-failed, interrupted/unknown, changed shape, post-release local/receipt failure; no retry, no invented identity | met against **reconstructed** fixtures | adapter and coordinator tests |
| One real release build of each app; one real preview per target and one live deployment with the unselected site unchanged | build **met**; remote **NOT_RUN** | real build above; remote needs an operator project and go-ahead |
| Hosting smoke records real HTTPS HTML, JS/CSS/WASM media type and bytes, fallback and cache headers; API/login/browser integration separate | implemented and emulator-checked; real Hosting **NOT_RUN** | smoke tests; emulator headers; origin/auth dependency named |
| Practical Firebase CLI + xtask runbook and recovery guidance; no GitHub Actions, workflow, secrets, automatic trigger, Firebase SDK or unrelated infrastructure | met | `docs/deployment/firebase-hosting.md`; no workflow touched |
| Canonical `cantos-work-item` + `cantos-engineering`, `cantos-leptos-web` for browser/build behavior, `cantos-code-review` at review; branch from `develop`, PR to `develop`; focused DEV, CHECKPOINT and one HANDOFF preflight | met except the separate read-only review pass | see Verification |

## Deferred acceptance

| Gate | Why not run | Prerequisites | Next command | Acceptance | Owner |
| --- | --- | --- | --- | --- | --- |
| a real preview deploy of Theatre and of Studio | needs the operator's Firebase project, two Hosting sites and an explicit go-ahead; creating them is not authorized by the issue | `firebase-tools` ≥ 15.34.0, `firebase login`, sites created, `.firebaserc` mapped | `cargo xtask deploy firebase <app> --web --project <id> --channel <name> --expires 1d` | `PASS` on `Hosting release` and `HTTP smoke`, a `confirmed` receipt, the printed URL reachable, the other site unchanged | the Firebase project owner |
| a real live deploy of one app | same; it replaces what a site serves | a preview that passed | `cargo xtask deploy firebase <app> --web --project <id> --channel live` | as above, with the live URL and the previous release identity noted for rollback | the Firebase project owner |
| capture of real CLI output | the same runs | the two above | copy the sanitized `--json` outputs over the reconstructed `hosting-*.json` fixtures, or prove them equal | adapter tests pass against the captured text unchanged | the same operator |
| Hosting's media types, headers and propagation | the same runs | the two above | the `HTTP smoke` row and the manual `curl` list in the runbook | every probe `ok`; any deviation recorded | the same operator |
| Hosting's own channel-ID, file-count and size limits | the same runs | the two above | none beyond the above | no service refusal on the real bundles | the same operator |
| the API origin and authentication contract | a design and client work, not configuration; the issue makes it a separate gate | an ADR choosing one of the three options in the runbook, an actual client reader for any API-origin setting, covering catalog, library, artwork, auth navigation and callbacks | not applicable | sign-in, catalog and playback demonstrated on the deployed origin | a follow-up issue (not created here) |
| long-lived caching of hashed assets | needs an oracle that a URL's bytes change whenever its name must, per asset class | the finding above | not applicable | real response headers and an old-tab/deploy/rollback walk | a follow-up |
| browser walk of a preview | needs a deployed URL | a passed preview | open the URL; check the console | no failed request other than `/api/*` | the same operator |

## Verification

- Full final-diff `cargo xtask preflight --changed-only` (all 17 gates) `PASS` on the tree before the
  review fixes below (Python 3.14 shim, `LC_ALL=C`, `TMPDIR=/private/tmp`, Gradle on `PATH`).
- After an independent read-only review the following were fixed and re-checked with `cargo fmt`,
  `cargo clippy -p xtask -D warnings` and `cargo test -p xtask` only (not a second full preflight):
  the confirmed release identity is persisted before the smoke; a failed intent write no longer leaves a
  `write_pending` receipt; curl ignores `~/.curlrc`; rewrites may not name a file; private names are
  matched case-insensitively; a missing default Hosting site is a distinct refusal; a trailing slash on
  the reported URL no longer fails the smoke.
- The real CLI (15.33.0, logged out, isolated `HOME`) parsed all three invocations (site list, preview
  deploy, live deploy) with the effective configuration and returned the real
  `{"status":"error","error":"Failed to authenticate, have you run firebase login?"}`, now the
  captured `hosting-error-auth.json` fixture.
- Known and not fixed: a preview result is not checked to name the requested channel in its URL;
  a directory swapped for a link between scan and upload is only bounded by the private run directory
  and the second scan; `ignore` globs and `Expires`/`CDN-Cache-Control` headers are not constrained by the
  policy validator; `--account` selection is not offered.

## Residual risk

- Every live behavior is unobserved: Hosting's acceptance of the effective configuration, the real
  result shapes, notification-free but still public preview URLs, required IAM roles (the lookup needs
  `firebasehosting.sites.get`, a deploy `firebasehosting.sites.update`), CDN propagation and the
  one-hour-versus-immediate reality of revalidation.
- A same-user process could alter the run's `dist` between the second scan and the CLI's read; the
  private run directory narrows this, and the second scan, the integrity pins and the smoke's byte
  comparison bound it. They do not remove it.
- The Firebase CLI may write `firebase-debug.log` into the private run directory; it is not scrubbed.
- Execution requires a POSIX host; `--dry-run` works anywhere. The smoke needs `curl`.
- The smoke retries read-only requests (3 attempts, 5 s apart); a slower propagation reads as a
  failure and is reported as one, with the release still known.
- `.firebaserc` is read as strict JSON; the Firebase CLI tolerates comments, xtask refuses them.
- The receipt records absolute local paths (the build command); it is private and ignored by Git.
- A full `cargo xtask verify web` build of Trunk's incremental state was not repeated beyond the
  release builds above; preflight covers the debug builds.
