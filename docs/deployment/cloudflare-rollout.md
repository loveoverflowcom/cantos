# Cloudflare Web rollout, recovery and Hosting retirement

This is the source preparation and operator execution runbook for
[#280](https://github.com/loveoverflowcom/cantos/issues/280). The offline report below is
implemented; it does not inspect accounts or perform a rollout. GitHub owns live acceptance.
Every live milestone starts **NOT_RUN** until attributable evidence is collected under an
explicit operator request. Merging this document does not complete the rollout issue.

The requested destination is static Cloudflare hosting for **Cantos** at `cantos.cc` and
**Cantos Studio** at `studio.cantos.cc`. `api.cantos.cc` and its API/PostgreSQL/Kanidm runtime stay
on the VPS. [ADR 0041](../decisions/0041-direct-hosting-api-boundary.md)'s direct browser/API,
exact per-surface CORS and #295 Web-cookie/native-Bearer direction remain authoritative.
The Web-cookie migration is pending and must be qualified before its acceptance. The current
[VPS runbook](vps-production.md) owns installed-runtime/configuration operations. The existing
[Firebase Hosting lane](firebase-hosting.md) stays available for the reviewed recovery window;
do not interpret the requested target as observed DNS or an accepted provider cutover.

## Prerequisites and authority

| Owner | Required before operator execution |
| --- | --- |
| [#277](https://github.com/loveoverflowcom/cantos/issues/277) | Reviewed source/config/artifact identity, one resolved SHA, build-only credential isolation, workflow transfer/approval/order, artifact-consuming VPS deployment and partial-release receipts. Existing manifest v1/build/deploy commands do not establish these guarantees. |
| [#278](https://github.com/loveoverflowcom/cantos/issues/278) | The Pages Direct Upload versus Workers Static Assets decision is recorded as [proposed ADR 0043](../decisions/0043-cloudflare-pages-direct-upload.md) (the owner accepts it); the publisher, policy and artifact checks are implemented in `cargo xtask deploy cloudflare`, with source-bound preview observations in its [acceptance record](cloudflare-pages.md#acceptance-status). Production, CircleCI, browser/auth and domain cutover remain separate. This runbook neither accepts the ADR nor substitutes a Worker runtime. |
| [#279](https://github.com/loveoverflowcom/cantos/issues/279) | Optional submit/status convenience. It is not a cutover prerequisite when an authorized CircleCI UI/API can run the reviewed #277 contract. Proposed `pipeline` commands are not available until implemented. |
| [#295](https://github.com/loveoverflowcom/cantos/issues/295), [#253](https://github.com/loveoverflowcom/cantos/issues/253) | Qualify the pending custom-domain Web-cookie migration, callback/gateway, exact credentialed CORS and CSRF; native retains Bearer. Preview origins need their own declared policy. Do not repair auth or registration as a hosting workaround. |

The present source has manifest-v2 packaging, same-workflow promotion and explicit Firebase/VPS commands; account/executor/live acceptance is separate. Do not use the
Firebase build-and-publish command as a substitute for Cloudflare artifact promotion. Re-read
the owners' implemented command contracts before each live stage; there is deliberately no
invented Cloudflare publish/rollback or CircleCI-submit command in this runbook.

Record a separate operator request covering the exact account/project/environment, component,
source/config SHA, action, time window, maximum CI credits/runtime and recovery authority.
Start with **one build-only Web target**, not `all` (which includes Android). Review resource
class rather than increasing it. Source/PR authority does not authorize account login/consent,
terms, GitHub access, token/context/project provisioning, billing, DNS changes, deployment,
test-account mutations or deletion. If authority is missing, keep that stage BLOCKED and finish
independent source/read-only work. An approved preview is public, including its data connections.

## Private read-only inventory

Collect the following from trusted read-only account views under the authorized scope. Record
unknowns explicitly; a repository name, YAML or old receipt does not prove account state.
Keep reviewed snapshots and raw recovery assets private, outside Git. Keep only sanitized
evidence in any shared report, and record observation time and operator/source for each item.

| Inventory | Required facts and stop conditions |
| --- | --- |
| CircleCI | Integration type, project/definition IDs, trusted config and checkout source/revisions, plan/credits/concurrency/resource limits, exact context project/branch/actor restrictions, artifact access/retention and expiry. Confirm build jobs receive no deploy contexts or broadly shared project credentials; Web and VPS deploy contexts are separate. Missing restriction/support is a blocker. |
| Cloudflare | Actual account/zone/project IDs, Direct Upload versus Git integration, selected provider, production branch, custom-domain associations, current and retained successful production deployment IDs, returned deployment URLs, effective token permissions, limits and installed/pinned tools. Do not infer availability, project names or `pages.dev` URLs. |
| Firebase | Actual project/site IDs, exact current releases/version IDs and custom-domain bindings, retained release bytes/config, recovery access and retention. `cantos-a348b` / `cantoss-theatre` / `cantoss-studio` are prior source references to recheck, not an account discovery result. |
| DNS/TLS | Current authoritative zone and registrar (not assumed Spaceship), delegation/TTL, DNSSEC/DS, CAA, certificate state, all A/AAAA/CNAME/MX/TXT/verification records and any API/mail/identity ingress. A nameserver move is not a registrar transfer. Review the complete snapshot before editing any record. |
| VPS | Installed image digest/revision, CPU/OS, schema epoch and migration registry, origin/callback/gateway settings, health and retained backup/image/host-bundle/config recovery evidence. Use the [read-only status lane](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/vps-production.md#read-only-inventory) only for the explicitly named host; do not run a login/exchange/refresh as a route probe. |

Do not export secret values, bearer/refresh credentials, mailbox action codes, personal emails,
signed URLs or private content. A sanitized evidence file may list nonsecret remote IDs and
full source/config/artifact hashes. Retain separate private source snapshots for recovery;
hashing a file does not prove its provenance or permissions.

## Execution milestones

Complete one stage at a time. Preserve component outcomes, including pending approval,
rejection, failure, cancellation and unknown results. Never label accepted upload or pipeline
processing as successful rollout. A zero-job pipeline or missing terminal receipt is incomplete.

1. **Qualify source and tools.** Run the final source's applicable local gates and #277/#278
   adapter/config tests. Validate/process the actual CircleCI config with its supported tool.
   A known qualification baseline failure remains FAIL with its owner; do not refresh evidence,
   remove a check or skip it to manufacture acceptance. Inspect rolling executor/tool versions.
2. **First build-only run.** Within the granted budget, select one reviewed Web component and
   collect the actual pipeline/workflow/job IDs, timestamps, config/checkout/resolved release
   SHAs, gate outcomes and artifact manifest/actual-byte digests. Read back job selection and
   credential separation. Record elapsed time, credits and cache observations. Stop on ambiguous
   identity, no jobs, failed gates or unexpected resource usage; do not trigger another run to
   repair a missing status result. [CircleCI setup](circleci-builds.md) owns current selectors.
3. **Prepare both approved Web artifacts.** Pin the same resolved release SHA, each app's
   component/configuration/policy digest, explicit approved API origin, file sizes and hashes.
   Use #277's trusted manifest and #278's static policy checks. Refuse stale/foreign/mixed source,
   altered bytes, unsafe archives or server/private content. Keep build outputs immutable and
   reuse them for publish; no Cloudflare build or hidden local rebuild. Missing artifact-contract
   support blocks this stage; a legacy v1 archive is not automatic promotion evidence.
4. **Publish before moving domains.** Invoke only the implemented selected publisher under
   separate authority, one project/surface at a time. Record actual account/project/branch,
   deployment ID, environment and returned URL. Run the targeted static HTTP matrix below on
   that URL, then actual browser checks where authorized origins/accounts exist. Without exact
   preview API/CORS scope, perform only authorized static/read-only checks and keep auth NOT_RUN;
   never add wildcard CORS or treat a preview as a backend sandbox.
5. **Confirm API compatibility.** Read the installed revision/schema/origins and link compatible
   API evidence. Do not redeploy a compatible API to prove a Web move. When API promotion is
   separately requested, require #277's same-source image/host bundle, architecture/schema
   checks, backup/grants/health and recovery protocol; the VPS does not compile. Schema prefix
   refusal is a real blocker, not permission to migrate/restore by hand. The existing
   [configuration-only reload lane](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/vps-production.md#selected-custom-domains) requires its own
   named-host permission; origin correction never implies bootstrap, SQL or identity changes.
6. **Rehearse recovery, then cut over one surface.** Qualify the three recovery boundaries
   below with pinned IDs/hashes and operator access. Review TLS/static/API readiness before
   domain changes. Pin the production deployment actually serving the domain, repeat the HTTP
   and browser matrices on that final origin, and record propagation/certificate observations.
   A pending Studio outcome does not undo a successful Theatre or API operation automatically.
7. **Operator acceptance and hold.** Obtain explicit production/recovery acceptance with each
   capability gap named, and an agreed end to the Firebase recovery window. Keep recovery
   releases/config/credentials usable throughout it. There is no automatic retirement timer.
8. **Hosting-only retirement.** Follow the dependency review and separately authorized actions
   below. Reconcile the actual final configuration before marking rollout acceptance in #280.

## HTTP and browser evidence are separate

For each preview and final domain, attribute observations to the same release/config/artifact
and deployment IDs. Keep raw captures private and sanitize before export. Use synthetic
code-free paths for read-only probes; never put a real email action code in a smoke URL.

| Check | Required observations |
| --- | --- |
| Entry/config identity | Fresh HTML and public API-origin configuration match approved bytes. Check root and actual entry/action paths from both clients. Do not substitute a home-page screenshot or deployment time for hashes. |
| Assets | Actual decoded SHA-256, size, status and MIME for JS/WASM/CSS/fonts/images; WASM is `application/wasm`. Exercise content encoding and compare decoded bytes to inventory; compressed size alone cannot satisfy provider per-file limits. |
| Refusals/routing | `/api/*` and unique missing JS/WASM/CSS/asset paths on the static host return genuine absence, never HTML 200. Check actual hash/query navigation and email-action entry routes. Pages' fallback must not masquerade as an API or asset. |
| Cache/security | First response for root/HTML/action entry has no-store/no-referrer; security headers include nosniff, embedding/CSP and Permissions-Policy from the reviewed owner. Record Cache-Control, encoding, ETag/revalidation, fresh browser/hard reload and prior-version navigation. Immutable caching requires an established URL-to-bytes invariant. No automatic Analytics/Zaraz or blanket cache rule. |
| API/UI | A fresh actual client requests the explicit API origin and its own `/api/v1/theatre/*` or `/api/v1/studio/*` surface. Inspect loading/empty/error/retry and real catalog/detail/player using authorized published material. Empty catalog is not playback acceptance; no production seeding or TTS for test data. |
| Auth isolation | With separately authorized accounts and the #295 migration: custom-domain login/callback, API-confirmed session, reload/reopen under the reviewed cookie policy, protected requests, logout and refusal of old sessions; Studio denial and Theatre-to-Studio isolation. Inspect HttpOnly/Secure cookie scope, exact credentialed CORS and CSRF including same-site siblings. No account credential reaches the static host or an unrelated media/provider origin. Native retains [Bearer v1](../../packages/contracts/bearer-v1.md); previews require separate topology qualification. Do not weaken auth/issuer to make hosting pass. |
| Registration | Only with explicitly authorized test mailbox/account/actions: first-response protections, URL-code removal before telemetry, no code in screenshots/logs/receipts, real email verification and current handler behavior. Do not create Admin, reset credentials or broadly enable signup. Preserve the [activation gates](firebase-enrollment-proof.md). |

Reuse #278's publisher smoke and the standalone read-only command below, both using #199's
[static-byte judge](../../xtask/src/deploy/smoke.rs). The standalone check covers entry
JS/CSS/WASM bytes/MIME/cache, missing WASM/JS/CSS/PNG and static `/api/*` absence, plus entry
no-store/no-referrer/nosniff/DENY. It does **not** cover this entire matrix, fonts/images,
CSP/Permissions-Policy, ETag round trips, API, browser or auth. Missing checks are explicit NOT_RUN
with prerequisites; local fixtures never accept live
HTTP, an installed issuer, playback or human UI quality. Product gaps stay with their owners
(for example #253, #269–#272, #174/#191/#206), not hosting workarounds.

## Standalone read-only HTTP check

Use this after a known publish, on a permitted preview origin, or on the final domain after
operator cutover. It performs no publishing or provider readback and requires no cloud token.
Copy `STATIC_ORIGIN` and `DEPLOYMENT_ID` from the actual provider readback; do not derive a URL
from a proposed project name. Both are operator declarations in this receipt. Preserve the
provider receipt separately to establish where/when the bytes were installed.

```sh
cargo xtask hosting-smoke cantos-theatre \
  --origin https://STATIC_HOST --api-origin https://api.cantos.cc \
  --deployment DEPLOYMENT_ID --release-manifest /ABS/ARTIFACT/manifest.json \
  --release-digest MANIFEST_SHA256 --release-sha FULL_SOURCE_SHA --local-artifact --dry-run
```

Remove `--dry-run` for the read-only GETs. Select `cantos-studio` with its own artifact/origin/ID
for Studio. `--local-artifact` explicitly acknowledges independently checked local provenance;
within the producing CircleCI workflow use `--release-request /ABS/REQUEST.json` instead, under
the [release contract](circleci-builds.md). The manifest digest and full source SHA must come
from the reviewed artifact, not a directory timestamp. Both extraction and local static bundle
validation must pass before the first HTTP request. This checks the application bytes; policy
files added by the Pages publisher remain bound to its separate bundle/policy receipt.

The [command contract](../../xtask/README.md#read-only-hosting-http-observation) owns bounds and
receipt fields. Artifact refusal records HTTP NOT_RUN; missing curl records UNSUPPORTED;
incomplete HTTP, stale bytes, wrong MIME/cache, HTML fallback and missing entry privacy fail.
There is one round with no publish, retry or rollback. Store the sanitized receipt's digest as
HTTP evidence in the [offline record](#offline-report-contract); `hosting-report` still only
validates that declaration and does not read or verify this receipt. Keep browser/auth rows
independent, and keep raw run directories private.

The reused promotion contract accepts only fresh artifacts (24 hours) and the current build
policy. Older retained production artifacts require a separately reviewed read-only byte
comparison during recovery; do not relax promotion freshness, rebuild the artifact or treat
a failed smoke as an instruction to deploy. No production/cutover/retirement acceptance follows
from source tests or an HTTP PASS alone.

## Domain plan and three recoveries

For Pages, an apex domain requires a zone in the same Cloudflare account and Cloudflare
nameservers; an externally managed subdomain may use CNAME. Associate each custom domain
with the correct Pages project before relying on DNS. Inventory first, review DNSSEC/DS and
CAA/certificate prerequisites, preserve API/mail/verification/identity records, and change only
the approved mapping. Do not proxy/cache all `api.*`, invent IPv6, rename Kanidm or add
`idm.cantos.cc`. Repeat TLS/routing/exact-CORS/browser observations after propagation.
[Cloudflare custom domains](https://developers.cloudflare.com/pages/configuration/custom-domains/)
also documents reactivation delays when records move away and back.

| Recovery | Rehearsal and evidence | Boundary |
| --- | --- | --- |
| Cloudflare release rollback | In an authorized project/environment, select a specific previously successful **production** deployment, record its source/artifact/remote ID, perform the approved rollback and verify restored decoded bytes/headers and API compatibility. Keep before/after IDs and observations. | Preview deployments are not rollback targets. A rebuild of old HEAD is a new artifact. A first deployment with no retained successful production target cannot claim rehearsal. See [Pages rollbacks](https://developers.cloudflare.com/pages/configuration/rollbacks/). |
| Provider/DNS rollback | Keep the exact Firebase release, site/custom-domain associations, ownership verification, config/origin snapshots and operator access through the agreed window. Rehearse the permitted recovery route; document who may restore which records, TLS/reactivation conditions and observed propagation. | DNS is not instantaneous. Switching nameservers back alone is not a proven recovery. Coordinate Pages/Firebase domain association and complete DNSSEC/CAA state; preserve non-Web records. |
| API image/config recovery | Use the [VPS protocol](vps-production.md) and #277's compatible source-bound image/host bundle, backup/grants/health and switch-back/status receipts. Recover browser config independently through the reviewed configuration lane. | Image rollback does not roll back migrations. Schema epoch/prefix and forward-only constraints can block old images. DB restore, epoch cutover, account/issuer and credential recovery are separate operations; never roll back DB for static Web failure. |

| Observed partial outcome | Next action |
| --- | --- |
| Upload/submit timeout or lost response | Keep UNKNOWN and the local attempt/source/time/project details. Read back that exact operation with the approved owner tool/UI; check current state before retry. Do not issue a second publish/submit to discover the first result. |
| Deployment confirmed; smoke or browser fails | Keep the known deployment ID and record the failed check separately. Diagnose read-only; restore the approved production target only if recovery authority covers it. |
| API fails before dependent Web | The reviewed #277 DAG must block dependent Web. Record which jobs actually ran; a later independent Web success cannot turn the failed API green. |
| Theatre succeeds; Studio fails/pends | Preserve Theatre's known result and Studio's exact outcome. Reconcile/repair only the affected operation under explicit authority; no whole-release replay or automatic cross-component rollback. |
| Receipt write/status read fails after an effect | Record the known remote effect separately from local failure and reconcile its pinned ID. Missing local receipt never proves nothing happened. |

## Hosting-only retirement

Retirement requires reviewed production **and** recovery acceptance, the agreed window ended,
and a fresh dependency inventory. This source PR removes no Firebase lane, site, key or policy.
After acceptance, prepare a reviewed source cleanup of only obsolete Firebase **Hosting**
selectors/jobs/provider defaults/policy/mappings/docs and Hosting-exclusive credentials. Check
the actual CircleCI contexts and all credential consumers first: do not revoke shared Firebase
credentials used by Auth/email verification, Forge Functions/Gemini or App Distribution.

Cloud site deletion, custom-domain detachment and credential revocation each need explicit
targeted authority; source cleanup is not cloud deletion. Preserve the Firebase project,
Functions, Auth, App Distribution, media/private-draft storage, API, Kanidm, DB volumes and
recovery evidence. Do not leave a default release path that publishes to the old site after
cutover, or keep both providers indefinitely without an operator decision. Mark retained old
receipts/instructions historical rather than rewriting their evidence.

Reconcile [#237](https://github.com/loveoverflowcom/cantos/issues/237) as a conditional old
Firebase naming request: Cloudflare rollout should not allocate/rename Firebase sites in
parallel. Propose supersession only for the naming portion after checking current acceptance;
do not close #237, #199, #212, #253 or any capability issue merely because hosting changes.

## Offline report contract

Copy [the empty record](../../templates/hosting-rollout-record.json) into a private ignored run
directory and fill it from sanitized observations; keep GitHub as the live acceptance owner.
The template deliberately says NOT_RUN everywhere and contains no claimed account setup.

```sh
cargo xtask hosting-report --record artifacts/agent-runs/ROLLOUT/record.json --dry-run
cargo xtask hosting-report --record artifacts/agent-runs/ROLLOUT/record.json
cargo xtask hosting-report --self-test
```

`--dry-run` only prints the local reading plan; it probes no tools, reads no record and writes
nothing. Normal execution reads one regular local file (maximum 256 KiB, final symlink refused),
validates strict schema version 1, and prints fixed stage/status/evidence-kind labels and SHA
identities. It never follows references, contacts a provider, reads credentials, builds products,
performs HTTP smoke, checks a browser, writes a receipt or retries an operation. `--self-test`
exercises synthetic local records. Its tests also run in the existing repository Python suite;
there is no new runtime/CI acceptance gate. The new tooling retains conservative full-preflight
selection. These commands do not replace #277/#278's artifact validators or provider receipts.

| Field | Meaning |
| --- | --- |
| `release_sha` / `config_sha` | Actual full lowercase Git SHAs of resolved release and trusted CircleCI config, independently recorded. Null means unknown; never copy config SHA into release SHA to silence a mismatch. |
| `approved_api_origin` | Explicit reviewed canonical `https://` plus lowercase DNS hostname; null until known. All origin fields refuse whitespace/controls, paths, ports, credentials and query/fragment delimiters, even empty ones. Each component's `api_origin` must match for declared PASS. Artifact configuration/policy digests and checkout SHA belong in the referenced sanitized build evidence. |
| `circleci` | Actual pipeline/workflow IDs and job number (string); all required for declared build PASS. No inferred URL/`latest` or submission acknowledgement as build evidence. |
| Component identities | Artifact digest, actual provider deployment ID/environment/returned origin, observed final origin, and distinct successful production rollback target ID/environment. Missing values stay null. The target final domains are not evidence they are active. |
| Check `status` | Operator-declared `PASS`, `FAIL`, `BLOCKED`, `NOT_RUN` or `UNKNOWN`. UNKNOWN preserves ambiguous effects; BLOCKED names missing prerequisites in the private evidence. A valid record may contain any mixture and never promotes another row. |
| Check `kind` / `source_sha` / `evidence_sha256` | Existing engineering evidence vocabulary, bound release SHA for source-related PASS, and up to 16 SHA-256 references to sanitized evidence files. Null/empty means absent. Record evidence locations, timestamps, operator and content inventory privately; hashes are references, not signatures. |

The reader refuses unknown/duplicate fields, malformed identities, missing PASS evidence,
foreign source, wrong surface/API origin and preview rollback/final-domain claims. Live rows
require observed/integration evidence; browser rows require interaction evidence, so fixture
or compiled evidence cannot satisfy them. It does **not** read/hash referenced files, validate
provider receipts, independently check config/artifact bindings, authorize the actor, establish
freshness, or verify an operator declaration. Row PASS remains declared even when every row
says PASS. Exit 0 / xtask `hosting-report-schema PASS` means only a valid local record; invalid
records fail with a fixed reason code and no raw record fields or parser dump. It is not a
general log sanitizer. Never feed it raw HARs, tokens, signed URLs or email action URLs.

Keep underlying evidence for each row: provider decision; exact operator/budget scope;
inventory; qualification output; actual CircleCI build; API compatibility; DNS/provider
recovery; operator production/recovery acceptance; completed hold window; retirement dependency
review and Hosting-only cleanup. Per surface retain artifact, publish, preview HTTP, preview
browser, final-domain, final HTTP, final browser and release-rollback evidence independently.
Product/auth sub-scenarios and real performance measurements live in those referenced records;
one row must not hide an unrun required sub-scenario.

For performance, compare the same source/config/fixtures and named cold/warm cache conditions:
per-file/raw/encoded static bytes, measured transfer bytes, CI elapsed time and actual credits.
Use the existing [offline bundle inventory](../../xtask/README.md#optional-web-release-inventory)
only for its measured scope. Unmeasured fields remain NOT_RUN; no claim about audio/VPS bandwidth
savings follows from moving Web between CDNs. Keep full source/manifest/deployment identities,
tools and observation times with measurements rather than inferring a saving from compression.

Official implementation inputs, not Cantos account evidence:
[serving/cache/SPA](https://developers.cloudflare.com/pages/configuration/serving-pages/),
[static headers](https://developers.cloudflare.com/pages/configuration/headers/),
[CircleCI workspaces](https://circleci.com/docs/guides/orchestrate/workspaces/) and
[context restrictions](https://circleci.com/docs/guides/security/contexts/).
