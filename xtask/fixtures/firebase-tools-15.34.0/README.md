# Firebase CLI 15.34.0 contract fixtures

Sanitized inputs for the `cargo xtask distribute firebase` adapter tests. Every identifier is
synthetic (`example-project`, project number `000000000000`); no token, key, tester address or
signed link is real.

| Fixture | Provenance |
| --- | --- |
| `sdkconfig-unregistered.json` | Captured from `firebase apps:sdkconfig ANDROID <unknown app id>` on 2026-10-15 (exit 0, empty `client`), then sanitized. |
| `sdkconfig-registered.json` | Shape of a registered app, reconstructed from `lib/management/apps.js`; the API key is the literal `SYNTHETIC-NOT-A-KEY`. |
| `distribute-*.txt` | Reconstructed from the 15.34.0 sources `lib/appdistribution/distribution.js`, `client.js` and `commands/appdistribution-distribute.js`. The success transcript was confirmed against a live run on 2026-10-16 (same lines, with the interleaved `updating release notes` / `distributing to testers/groups` progress lines); failure texts remain source-derived. |

If a later CLI changes these texts, the adapter reports an *unknown* remote outcome instead of
guessing; update the fixtures only from a captured run of the new version.
