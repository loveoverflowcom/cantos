# Cantos infrastructure configuration

`infra/` owns tracked, nonsecret deployment policies, templates and environment wiring. It is
not a runnable product, a Cargo member, a second configuration authority or an orchestration
framework. [`xtask/`](../xtask/README.md) remains the repository command surface: it owns planning,
validation, build/capture, provider invocation and attributable receipts.

| Location | Owns | Explicit operation |
| --- | --- | --- |
| [`local-mail/resend-local/`](local-mail/resend-local/README.md) | Approved synthetic-only Resend HTTP fixture lock, release hashes and guarded launcher; no Identity proof provider | `cargo xtask run resend-local`; no automatic install or live mail |
| [`vps/`](vps/README.md) | Native/legacy Quadlet assets and retained PG/provider recovery inputs | `release native-upgrade` is same-schema NATIVE_READY only; first-native cutover remains #348; old `deploy vps` refuses |
| [`oci/`](oci) | Native Server/Identity image definitions and compatibility identifiers | `release oci-build` produces candidates; `cargo xtask release oci-qualify` tests the exact pair on disposable Linux ([fixture](../docs/assurance/native-oci-qualification.md), [contract](../docs/deployment/native-oci-release.md)); never deploys |
| [`edge/`](edge/README.md) | Caddy ingress/TLS/routing policy (run as the `cantos-edge` Quadlet unit) and a DuckDNS updater example | installed by `cantos-vps`; never an implicit deploy side effect |
| [`firebase/hosting/`](firebase/hosting/README.md) | Static Hosting policy and placeholder project/site mapping | `cargo xtask deploy firebase <app> --web --project ID --channel CHANNEL` |
| [`cloudflare/`](cloudflare/README.md) | Cloudflare Pages Direct Upload policy (`<app>/_headers`, `404.html`), private-mapping example and the pinned Wrangler lockfile | `cargo xtask deploy cloudflare <app> --web --account ID --project NAME --branch BRANCH --environment production\|preview --artifact DIR` ([runbook](../docs/deployment/cloudflare-pages.md), [rollout](../docs/deployment/cloudflare-rollout.md)); offline `cargo xtask hosting-report` never publishes |
| [`google-secret-manager/`](google-secret-manager/README.md) | Nonsecret Secret Manager inventory: secret IDs, environment, consumer, env/file contract, format and reviewed numeric pins; never values | `cargo xtask secrets inventory\|status\|import` ([runbook](../docs/deployment/google-secret-manager.md)); never a build, CI or deploy side effect |
| [`firebase/app-distribution/`](firebase/app-distribution/README.md) | Existing Android distribution inputs; no invented Firebase configuration section | `cargo xtask distribute firebase cantos-theatre --channel development --api-origin https://api.example.org --app-id ID --groups GROUPS --dry-run` |

[`firebase/functions/`](firebase/functions/README.md) owns the concrete private Forge smoke
policy (#486/#212). Producer source and the pinned supported-runtime SDK remain in
`services/cantos-forge-functions`; `cargo xtask forge-probe package|inspect|qualify|deploy-plan`
owns local build/staging/qualification and a plan-only scoped Firebase primitive. There is no
automatic deployment. Source and ELF inspection do not establish deployed Linux compatibility.

The [native OCI contract](../docs/deployment/native-oci-release.md) owns current image/admission
and bounded upgrade operations. The [legacy topology/runbook](../docs/deployment/vps-production.md)
retains recovery history and is not a native activation guide. ADR 0041's direct Hosting direction supersedes the old
Web-proxy default; retained source assets and installed assets must be reconciled separately.

## Boundaries and path resolution

- Hosting builds/publishes exactly one selected Web app. App Distribution builds/distributes the
  supported Theatre development or production Android APK to explicit recipients. Neither starts edge/local services,
  deploys Functions, selects an implicit project, performs bootstrap or changes DNS/IAM/billing.
- [`services/manifest.toml`](../services/manifest.toml) remains the typed local process inventory.
  The native process inventory stays in `services/`; retained PG/provider assets are historical migration inputs, not active setup definitions.
  Application/domain code stays in its owning app, service or justified shared package.
- Hosting policy is read from this repository root, never the caller's working directory. Its
  diagnostic `public` paths resolve relative to `infra/firebase/hosting`; they reach the apps'
  default `dist` trees for the local emulator only. Raw deployment of that outside-project path
  is not supported: Firebase's deployment packaging rejects it. Routine deployment uses xtask.
- xtask renders the selected target and site into a new private ignored
  `artifacts/hosting/<run-id>/firebase.json` plus `.firebaserc`, with `public: "dist"` below that
  same directory. Every provider invocation receives explicit project/config and working directory.
  Generated effective configuration and bundles are outputs, not editable policy authorities.
  `--dry-run` starts no tools, makes no provider request and writes no staging directory.
- Functions smoke packaging validates the run-relative `functions` source path and packages the
  static Linux Rust worker and Node 22 wrapper together. It must not download compilers/encoders
  during invocation or use a source path escaping the Firebase project directory.

## Private files, migration and rollback

[`oci/historical-v23.json`](oci/historical-v23.json) contains public immutable old-image pins
for the [historical native rehearsal](../docs/deployment/native-existing-host-migration.md).
It establishes no actual DB state, admitted previous release or live-host acceptance. Adoption
preserves the known existing data/proxy layout; it never reuses legacy PG dumps as native backups.

The repository move changes no live resources. Public hosts, routes, ports, Podman network/volume
names and installed absolute `/root/cantos-services/...` paths remain unchanged. Caddy's imports
are in-file snippets and container static roots remain `/srv/theatre` and `/srv/studio`; there are
no relative mount/include paths to rebase. The only tracked policy path adjustment is Hosting's
extra `../` to the same app outputs.

1. Stop concurrent local deployment work before changing a checkout. Preserve any existing
   ignored `services/firebase-hosting/.firebaserc`; Git does not move private files. Compare it
   with any existing `infra/firebase/hosting/.firebaserc` before moving/copying anything. Choose
   one reviewed mapping at the new path; resolve conflicts manually, never merge destinations
   by guess or let two editable authorities drift. xtask does not read the old path or root aliases.
2. Never commit personal mappings, OAuth/token/credential JSON, certificates, debug logs, audio or
   build outputs. `.firebaserc`, `firebase-debug*.log`, `artifacts/`, `env.local`, `data/` and key
   files remain excluded. Keep external operator secrets and installed assets outside the checkout.
   No personal Drive draft is migrated, deleted or provisioned by this change.
3. Returning to a prior revision restores tracked paths, not private mappings. Keep a private
   backup and restore the reviewed mapping to the path expected by that revision. Do not prune
   volumes or recreate sites/apps/codebases as a directory-move recovery step.
4. Retain run receipts/bundles until deployment outcomes and rollback needs are reconciled.
   Staging cleanup is explicit operator work on identified local run directories after retention
   review, not an automatic side effect of plans/deploys. Remote release/channel cleanup and
   rollback remain separate operator actions in the [Hosting runbook](../docs/deployment/firebase-hosting.md).

Historical evidence and immutable migrations remain at their original identities, including old
path strings and hashes. Local source/configuration checks do not establish live Caddy, Firebase,
login, provider execution or production acceptance; #199/#184/#203 retain their own gates.
