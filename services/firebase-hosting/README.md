# Firebase Hosting configuration

Operator configuration for publishing **one static Leptos Web app at a time** to Firebase Hosting
with the explicit, manual `cargo xtask deploy firebase`. It is not a `cargo xtask services` process
and is not in [the manifest](../manifest.toml); nothing starts or deploys it automatically, and no
GitHub Actions workflow is involved. Setup, the routine, verification and recovery are in the
[Firebase Hosting runbook](../../docs/deployment/firebase-hosting.md); the command contract is in the
[xtask README](../../xtask/README.md#deploy-firebase).

| File | Tracked? | Owns |
| --- | --- | --- |
| [`firebase.json`](firebase.json) | yes | The reviewed policy: two independent Hosting targets (`theatre`, `studio`), `ignore` rules and a `Cache-Control: no-cache` rule for every path. No rewrites, hooks, redirects or other Firebase product. Its `public` paths point at each app's default Trunk `dist` for the **local emulator** after `cargo xtask build <app> --web --release`; a deploy replaces them with the run's own bundle. Do not use this file for a raw `firebase deploy`: the CLI rejects a `public` path outside the project directory (`..`) only after it has created the preview channel and a version. `emulators` is local-only and dropped from a deploy |
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
