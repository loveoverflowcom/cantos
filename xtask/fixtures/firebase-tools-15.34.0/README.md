# Firebase CLI 15.34.0 contract fixtures

Sanitized inputs for the `cargo xtask distribute firebase` adapter tests. Every identifier is
synthetic (`example-project`, project number `000000000000`); no token, key, tester address or
signed link is real.

| Fixture | Provenance |
| --- | --- |
| `sdkconfig-unregistered.json` | Captured from `firebase apps:sdkconfig ANDROID <unknown app id>` on 2026-10-15 (exit 0, empty `client`), then sanitized. |
| `sdkconfig-registered.json` | Shape of a registered app, reconstructed from `lib/management/apps.js`; the API key is the literal `SYNTHETIC-NOT-A-KEY`. |
| `distribute-*.txt` | Reconstructed from the 15.34.0 sources `lib/appdistribution/distribution.js`, `client.js` and `commands/appdistribution-distribute.js`. A live capture is part of the operator acceptance recorded in the PR, not of these fixtures. |

`hosting-*.json` back the `deploy firebase` (Firebase Hosting) adapter:

| Fixture | Provenance |
| --- | --- |
| `hosting-sites-list.json`, `hosting-sites-list-empty.json` | Shape of `firebase hosting:sites:list --json` (`{status, result: {sites}}`, each site the Hosting API `Site` resource with a `projects/<number>/sites/<id>` name), reconstructed from `lib/commands/hosting-sites-list.js` and the Hosting REST reference. Synthetic site IDs and project number `000000000000`. |
| `hosting-preview-success.json` | Shape of `firebase hosting:channel:deploy --json`: the action's return value keyed by target (`{target, site, url, version, expireTime}`), reconstructed from `lib/commands/hosting-channel-deploy.js` (15.33.0, identical in 15.34.0). |
| `hosting-live-success.json` | Shape of `firebase deploy --only hosting:<target> --json`: `{hosting: <version resource name>}` (one deploy), reconstructed from `lib/deploy/index.js`. |
| `hosting-error-auth.json` | **Captured** 2026-10-16 from the real CLI 15.33.0 (logged out, isolated `HOME`) running `hosting:sites:list`, `hosting:channel:deploy` and `deploy` with the effective configuration: identical `{status, error}` output and exit 1 for all three. |
| other `hosting-error-*.json`, `hosting-preview-changed-shape.json` | The `{status: "error", error: <message>}` envelope written by `lib/command.js`, with messages modeled on the CLI's own authentication, permission, HTTP and `requirePermissions` errors; the changed-shape file is an invented drift case. |

None of the Hosting fixtures was captured from a real deployment: the run that could do so is the
operator's live acceptance (`NOT_RUN`, see `docs/evidence/issue199-firebase-hosting.md`). The
Hosting commands were diffed between 15.33.0 (the locally installed CLI) and 15.34.0 (the pin) and
are unchanged.

If a later CLI changes these texts, the adapter reports an *unknown* remote outcome instead of
guessing; update the fixtures only from a captured run of the new version.
