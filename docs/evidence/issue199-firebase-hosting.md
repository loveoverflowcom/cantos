# Historical Firebase Hosting tooling evidence — issue 199

Source-bound evidence for the original `deploy firebase` implementation in PR #202, based on
`develop@2e3f29a`. This is **historical tooling evidence**, not current production topology,
deployment state, account state or user acceptance. The original run observed local builds and
the Hosting emulator, not a production user journey. Current operations live only in the
[production runbook](../deployment/vps-production.md); command behavior is owned by
[xtask](../../xtask/README.md#deploy-firebase).

Retained for audit: exact bundle hashes, reconstructed-versus-captured CLI fixture provenance,
byte-identity/cache reasoning, emulator behavior and implementation limitations. Obsolete live
status/checklist tables and environment-specific activation instructions have been removed.
These observations are tied to the named historical source and have not been rerun by editing
this document. They do not establish today's code or deployed releases.

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

## Retained implementation limitations at the recorded source

A same-user process could alter the private run bundle between the second scan and the CLI read;
second-scan, integrity pins and post-release byte smoke bound but do not remove that risk. The
preview URL's requested-channel correspondence, ignore/header policy constraints and account
selection had recorded review gaps; re-inspect current source before treating them as present or
fixed. Raw Firebase debug logs may contain private information and stay ignored. Receipt paths
are private. Read-only smoke retries never authorize replaying a release write.
