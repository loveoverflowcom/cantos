# Google Cloud Secret Manager — Cantos operator setup and secret custody

**Status:** the operator import helper (`cargo xtask secrets`) and the tracked inventory are
**implemented and tested against a scripted gcloud with synthetic payloads**. Live metadata
reads and the two authorized ops-key imports below have passed ([observed state](#observed-live-state));
per-secret IAM grants, CI workload identity and runtime binding remain **NOT_RUN**.
Tracked by [issue #446](https://github.com/loveoverflowcom/cantos/issues/446).
This document contains **no secret values**. Committing it creates no Google Cloud resource,
IAM grant, secret version, VPS runtime mount or CI identity.

## Scope and authority

- Google Cloud **project ID**: `cantos-a348b` (owner-selected, nonsecret).
- [Secret Manager console](https://console.cloud.google.com/security/secret-manager/secrets?project=cantos-a348b).
  A browser Console session or its `authuser` selector is **not** a gcloud/API credential and
  does not prove any permission.
- Secret Manager owns provider/API credentials, file-based Identity/Server keys and operational
  keys. **Git owns only names, mapping, policy, instructions and reviewed numeric version pins;
  no SOPS ciphertext or Age identity.**
- [`infra/google-secret-manager/inventory.toml`](../../infra/google-secret-manager/inventory.toml)
  is the nonsecret mapping; [`cargo xtask secrets`](../../xtask/README.md#secrets) is the only
  repository command. There is no Cantos HTTP endpoint for uploading or reading secrets.
- This guide does not authorize DNS changes, service restarts, deployment, credential
  revocation, billing changes or broad IAM policy grants.

The [native OCI plan](native-oci-release.md) is not a live host-activation workflow.
[CircleCI release flows](circleci-builds.md) and [Firebase/Cloudflare operator flows](cloudflare-pages.md)
keep their own approval boundaries. An import is **not** evidence that Server or Identity
received or accepted the value.

## Inventory

The inventory lists each secret by **name, environment, consumer, contract and format**, derived
from the consumers' source (not from an old `.env.example`). Print it with
`cargo xtask secrets inventory`.

| Environment | Secret ID | Consumer · contract | Delivery |
| --- | --- | --- | --- |
| dev | `cantos-dev-identity-smtp-password` | Identity `CANTOS_IDENTITY_SMTP_PASSWORD` | runtime value, only when a developer explicitly selects SMTP |
| prod | `cantos-prod-identity-smtp-password` | Identity `CANTOS_IDENTITY_SMTP_PASSWORD` | Identity runtime only |
| prod | `cantos-prod-identity-firebase-api-key` | Identity `CANTOS_IDENTITY_FIREBASE_API_KEY` | Identity runtime only |
| prod | `cantos-prod-identity-firebase-service-account` | Identity `CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE` | private file, **never** JSON in an env value |
| prod | `cantos-prod-identity-firebase-envelope-key` | Identity `CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE` | private file, exactly 32 bytes |
| prod | `cantos-prod-server-playback-key` | Server `CANTOS_PLAYBACK_KEY_FILE` | private file, exactly 32 bytes, owner-only |
| prod | `cantos-prod-server-s3-access-key-id` | Server `CANTOS_S3_ACCESS_KEY_ID` (default `access_key_env`) | Server runtime only, when S3 is configured |
| prod | `cantos-prod-server-s3-secret-access-key` | Server `CANTOS_S3_SECRET_ACCESS_KEY` (default `secret_key_env`) | Server runtime only, when S3 is configured |
| ops | `cantos-ops-cloudflare-api-token` | `deploy cloudflare` / CircleCI `cloudflare-web` `CLOUDFLARE_API_TOKEN` | selected deploy job or operator only |
| ops | `cantos-ops-google-api-key` | owner-confirmed Gemini Flash TTS / Google AI for an external producer | operator custody only; no Cantos runtime binding or application env contract |
| ops | `cantos-ops-android-release-keystore` (+ `-keystore-password`, `-key-password`) | `distribute firebase --channel production` `CANTOS_RELEASE_*` | operator workstation only |
| ops | `cantos-ops-vps-ssh-private-key` | operator SSH | operator workstation only; **never** Server, Identity or CI |

The S3 pair is the **Long Van** object storage key. Its endpoint
(`https://s3-hn1-r1.longvan.net`), region and bucket are nonsecret Server storage TOML values.
The current pair was shared in an agent chat on 2026-10-25; the owner accepted that exposure and
kept it, so it is not marked `ROTATION_REQUIRED`. Reset it in the Long Van console ("Reset key")
and import the new pair if that record is disclosed further or the key is misused.

The separately supplied Google API key is held in ops custody for the owner-confirmed
**Gemini Flash TTS / Google AI external producer**. It is not mapped to
`CANTOS_IDENTITY_FIREBASE_API_KEY`, Server or Identity. Its inventory contract is a custody
note, not an environment variable to export; no producer execution or secret delivery is wired
by this addition. Audio production remains outside Cantos under [ADR 0018](../decisions/0018-external-audio-production.md).
Because the value was supplied in an agent chat, it is marked `rotation_required = true`:
the owner replaces it at its Google provider, then imports that replacement with the observed
version fence. This record neither revokes the supplied value nor grants runtime authority.

Deliberately **not** inventoried:

- nonsecret configuration: project IDs, origins, hostnames, SMTP host/user/from, Firebase
  project/action handler, S3 endpoint/bucket, `CANTOS_RELEASE_KEY_ALIAS`;
- short-lived values: an S3 session token, CI OIDC tokens;
- `FIREBASE_SERVICE_ACCOUNT_JSON` in the CircleCI `production-web` context: a long-lived key to
  be **replaced** by workload identity (below), not copied into another store;
- retired provider inputs (`CANTOS_*_OIDC_SECRET`, Kanidm/PostgreSQL credentials): the native
  Server refuses them; their legacy recovery copies stay where #348 preserves them.

### Status vocabulary

Report each secret by name with exactly one of these, never with its value, size or hash:

| Status | Meaning | Established by |
| --- | --- | --- |
| `MISSING` | no secret or no version exists | `secrets status` |
| `IMPORTED` | a version exists (`latest vN`) | `secrets import` / `status` |
| `VERIFIED_ACCESS` | the **consumer's own** reader identity read that numeric version | the reader, on its host/job; never the operator's account |
| `RUNTIME_BOUND` | the pinned version is installed in the consumer's protected store and the consumer started with it | host/runtime evidence under #348 |
| `ROTATION_REQUIRED` | the value left its protected store; rotate at the provider and import the replacement | owner decision, recorded with `rotation_required = true` |

An `IMPORTED` secret is not `VERIFIED_ACCESS`; neither is `RUNTIME_BOUND`.

### Observed live state

Initial operator-reported observation of 2026-10-25, source-bound to that day and to
`cargo xtask secrets status`; refresh it by rerunning `status`, not by editing this table.

| Layer | Result | Evidence and limit |
| --- | --- | --- |
| gcloud account | PASS | signed in as the owner's account `manhpdbroad@gmail.com`, default project `cantos-a348b` (nonsecret) |
| Secret Manager API | PASS | `secretmanager.googleapis.com` was already enabled; nothing was enabled by this run |
| `secrets status --environment prod` | PASS | first live run; all 13 inventoried secrets reported `MISSING`, including both Long Van S3 secrets |
| Imports at the initial observation | NOT_RUN | no version existed then; the later ops imports are recorded below |
| Per-secret IAM, `VERIFIED_ACCESS`, `RUNTIME_BOUND` | NOT_RUN | no consumer identity has read a version. Other operator machines (workstation, Mac mini) that sign in with the same account need no extra grant, but that is not verified here |

A passing `status` shows that the account can list version metadata. It does not show that any
value is correct, that a consumer can read it, or that the S3 pair works against Long Van.

### Authorized ops-key imports — 2026-10-25

Executed on the owner's authenticated operator workstation as `manhpdbroad@gmail.com`, with
the explicitly requested project `cantos-a348b`, using the helper from
`develop@409ace61360d303160e55828a9bf5ecdd0a9e97b` plus this nonsecret inventory addition.
The owner supplied a private Drive file containing a Cloudflare Bearer token and a separate
Google API key. The Drive command snippet was treated as data, never executed. Only the token
was extracted; neither the source file, source URL nor a credential payload is tracked.

| Secret | Status | Observed evidence and limit |
| --- | --- | --- |
| `cantos-ops-cloudflare-api-token` | `IMPORTED` | `secrets import` passed; metadata readback reports enabled version `1`, unpinned. Cloudflare token validity, scope and deploy-job access were not tested. |
| `cantos-ops-google-api-key` | `ROTATION_REQUIRED` | `secrets import` passed; metadata readback reports enabled version `1`, unpinned. The owner confirmed Gemini Flash TTS / Google AI for an external producer; key validity and producer access were not tested. |

Before each write, `secrets status --environment ops` reported the target `MISSING` and an
import `--dry-run` showed the exact project/account/consumer. Each real import used an
owner-only file outside the checkout and `--expect-latest none`, created the secret with
automatic replication, then returned `PASS add-version added v1 (IMPORTED)`. Values reached
gcloud through stdin, never argv. The ignored, sanitized
`artifacts/secrets/import-ledger.ndjson` records both successful create/add operations.
Post-import `cargo xtask secrets status --project cantos-a348b --account manhpdbroad@gmail.com
--environment ops` confirms version metadata without accessing payloads.

No consumer has performed `VERIFIED_ACCESS` or `RUNTIME_BOUND`; IAM grants, CI wiring,
numeric runtime pins, provider calls and deployment remain **NOT_RUN**. The other inventoried
ops secrets remained `MISSING` in this readback; no production-secret state was refreshed.
Importing these keys completes only this bounded custody step of #446.

### VPS SSH custody preparation — 2026-10-25

**Import NOT_RUN; exact storage authorization pending.** The existing inventory entry
`cantos-ops-vps-ssh-private-key` is the proposed destination in `cantos-a348b`; this record
does not authorize creating it or adding a version. A targeted metadata-only check on the Mac
mini, using the already credentialed operator account, returned `NOT_FOUND` for both secret
metadata and version listing. Thus the observed custody state was `MISSING`, with no numeric
version or pin. Refresh this observation immediately before any authorized import; do not
create a second secret under a similar name.

The supplied SSH private key appeared in an agent tool result on **2026-10-23**. It is marked
`rotation_required = true` / `ROTATION_REQUIRED` independently of whether a stored version
exists. The owner's bounded acceptance of SSH verification on 2026-10-25 does not remediate
that exposure or authorize indefinite use. Importing the same bytes cannot rotate or revoke
the key. Host-side replacement/revocation requires a separate owner-authorized operation;
no rotation, host trust change, new reader, IAM grant or deployment was performed here.

After the owner confirms the exact project and secret, reuse the approved existing local
`0600` regular file in its `0700` directory outside the checkout. Do not download another copy
from Drive when that file is available. Do not fetch the key into chat, render a preview, or
record its source/download URL. Check the file's ownership/mode without printing its contents.
Use the [import procedure](#importing-a-value), selecting the existing `opaque` format so the
SSH key's bytes and newlines are preserved. Never trim or re-encode it as a one-line text secret.

The reviewed plan should name only this destination and the explicit operator account:

```sh
# Plan only, after resolving SSH_KEY_FILE securely; never paste the key itself.
cargo xtask secrets import --project cantos-a348b --account OPERATOR_EMAIL \
  --secret cantos-ops-vps-ssh-private-key --file "$SSH_KEY_FILE" \
  --expect-latest none --dry-run
```

`none` is valid only while a fresh metadata read still shows no versions. Otherwise use the
highest version ever issued, including disabled/destroyed versions. Run the real import only
after the exact persistent-storage authorization; preserve all previous versions. On an
uncertain add result, stop and reconcile metadata before any retry. Record the resulting
secret resource name, numeric version and sanitized outcome here after verification; never
record key bytes, size, hash, local private path or a signed download URL. Byte equality, if
separately allowed, is checked privately without printing either value or a digest.

The only intended reader is the operator on the workstation. Existing gcloud credentials and
metadata access do not prove payload access. Import, file readback, `VERIFIED_ACCESS`, numeric
consumer binding, SSH acceptance and deployment are all **NOT_RUN** in this preparation.

## Identities and least-privilege IAM

Separate writers from readers. Grant on the **individual secret**, never project-wide, except
the creation role an operator holds while bootstrapping.

| Principal | Role | Scope |
| --- | --- | --- |
| Project owner (bootstrap) | enable `secretmanager.googleapis.com`, confirm billing | project; owner action, never xtask |
| Operator (human account) | `roles/secretmanager.admin` while creating, or a custom role with `secretmanager.secrets.create`/`get`/`list` plus `roles/secretmanager.secretVersionAdder` and `roles/secretmanager.viewer` | project for creation; per-secret `secretVersionAdder` afterwards |
| Identity runtime reader | `roles/secretmanager.secretAccessor` | each `cantos-prod-identity-*` secret only |
| Server runtime reader | `roles/secretmanager.secretAccessor` | each `cantos-prod-server-*` secret only |
| Cloudflare deploy job (WIF principal) | `roles/secretmanager.secretAccessor` | `cantos-ops-cloudflare-api-token` only |
| Developer | `roles/secretmanager.secretAccessor` | `cantos-dev-*` secrets only |
| Operator workstation | `roles/secretmanager.secretAccessor` | `cantos-ops-android-*`, `cantos-ops-vps-ssh-private-key`, `cantos-ops-google-api-key` |

No service-account key is created to bootstrap any of these. No production or ops reader is
granted to pull-request, checkout, compile or test jobs.

```sh
# Per-secret grant (REMOTE WRITE; owner/operator only, after review):
gcloud secrets add-iam-policy-binding cantos-prod-identity-smtp-password \
  --project=cantos-a348b --member=READER_PRINCIPAL \
  --role=roles/secretmanager.secretAccessor
```

## Bootstrap on an authorized trusted machine

Opt-in external operations, run by the operator on a trusted workstation, not a public build
job or an AI sandbox. Verify before mutating:

```sh
PROJECT_ID=cantos-a348b
gcloud auth login                         # interactive; the operator, not an agent
gcloud auth list                          # the intended account is credentialed
gcloud projects describe "$PROJECT_ID"    # the exact target and your access
gcloud services list --enabled --project="$PROJECT_ID" \
  --filter='NAME:secretmanager.googleapis.com'
# Only after explicit owner authorization and a billing check:
gcloud services enable secretmanager.googleapis.com --project="$PROJECT_ID"
```

## Importing a value

The helper uses gcloud as the client of the Secret Manager v1 API:
[`projects.secrets.create`](https://docs.cloud.google.com/secret-manager/docs/creating-and-accessing-secrets)
once (metadata only, automatic replication) and
[`projects.secrets.addVersion`](https://docs.cloud.google.com/secret-manager/docs/add-secret-version)
for each new immutable value.

1. Put the approved value **outside the checkout** in an owner-only file (`chmod 600`), or have
   a local tool pipe it with `--stdin`. Text secrets must not end with a newline (`printf '%s'`,
   not `echo`); the helper refuses control characters.
2. Review the plan: it names the account, project, secret, consumer, format and every remote
   effect, starts no gcloud and does not open the file.

   ```sh
   cargo xtask secrets status --project cantos-a348b --account OPERATOR_EMAIL --environment prod
   cargo xtask secrets import --project cantos-a348b --account OPERATOR_EMAIL \
     --secret cantos-prod-identity-firebase-envelope-key \
     --file /home/operator/private/envelope.key --expect-latest none --dry-run
   ```

3. Run the same command without `--dry-run`. `--expect-latest` is the version `status` reported
   (`none` for a new secret): if anything else exists, the import refuses rather than stacking a
   duplicate after an earlier uncertain write.
4. Record the issued version from the report. The ignored local ledger
   `artifacts/secrets/import-ledger.ndjson` keeps project, secret, account, action, result,
   version and time; never a payload, size, hash or path.
5. Remove the plaintext input with the operator's own secure process.

For a short value that never needs a file, read it at a hidden prompt and pipe it, so it is
neither echoed nor kept in shell history (the first import of a secret passes `--expect-latest
none`; the helper creates the secret then):

```sh
read -rs V && printf '%s' "$V" | cargo xtask secrets import --project cantos-a348b \
  --account OPERATOR_EMAIL --secret cantos-prod-server-s3-access-key-id --stdin \
  --expect-latest none; unset V
```

A successful run ends with `PASS add-version added v1 (IMPORTED)`; confirm with `secrets status`
(`IMPORTED latest v1`). `UNCERTAIN` follows the no-retry rule above.

If the add result is `UNCERTAIN` (transport failure, timeout, killed, unreadable output), do
**not** retry. Run `secrets status`: a version above the fence is this upload; otherwise rerun
with the same fence. A missing value stays `MISSING`; never generate it, paste it into
chat/GitHub or borrow another system's credential. Agents ask the owner for a secure local route.

Direct `gcloud secrets versions add --data-file=PATH` remains a valid manual fallback; it skips
the inventory, format and fence checks, so record the version by hand.

## Delivery to consumers

**Development.** A developer uses their own `gcloud auth login` and reads only `cantos-dev-*`
secrets for an explicit SMTP lane. Ordinary builds, tests, `run` and the local Auth Emulator
receive **no** Google/Firebase/SMTP credentials: the
[Auth Emulator isolation](../architecture/firebase-verification.md#local-development-composition)
refuses inherited inputs by design and must not be bypassed.

**CI/CD.** Current owners: CircleCI's `production-web` context holds a long-lived
`FIREBASE_SERVICE_ACCOUNT_JSON` for Hosting and `cloudflare-web` holds `CLOUDFLARE_API_TOKEN`;
both jobs are restricted to this repository and `develop` ([CircleCI builds](circleci-builds.md)).
GitHub Actions' `native-oci-publish` uses only the job's `GITHUB_TOKEN` for GHCR and
`repository-checks` uses no secret, so neither needs Google access. The target, not yet
provisioned (**NOT_RUN**):

1. A workload identity pool with a CircleCI OIDC provider (issuer
   `https://oidc.circleci.com/org/<ORG_ID>`, audience the organization ID) and an attribute
   condition on the CircleCI project ID, the `develop` ref and the restricted context, verified
   against [CircleCI's OIDC claims](https://circleci.com/docs/guides/permissions-authentication/openid-connect-tokens/)
   and [Google's pipeline guide](https://docs.cloud.google.com/iam/docs/workload-identity-federation-with-deployment-pipelines).
2. Hosting deploys impersonate a Hosting-only service account through that pool; the JSON key is
   then deleted in IAM and removed from the context (an owner action after a passing deploy).
3. The Cloudflare job's principal gets `secretAccessor` on `cantos-ops-cloudflare-api-token`
   only and fetches a pinned numeric version at job time into a private file it removes.
4. Pull-request and build-only pipelines attach no production context and match no condition.

The CircleCI config is unchanged by this work: switching it before the pool exists would break
deployment.

**VPS.** Native activation belongs to #348 and is not performed here. When it is ready, an
authorized deploy-side step (the operator's workstation or a reviewed deploy job, never the
application) reads the **pinned numeric version** and installs it into Podman's secret store
without writing it to a local file:

```sh
gcloud secrets versions access 3 --secret=cantos-prod-server-playback-key \
  --project=cantos-a348b --account=OPERATOR_EMAIL \
  | ssh deploy@HOST 'podman secret create --replace cantos-playback-key -'
```

then binds it in the unit, for example
`Secret=cantos-playback-key,type=mount,target=/run/secrets/playback.key,mode=0400,uid=<runtime uid>`
with `CANTOS_PLAYBACK_KEY_FILE=/run/secrets/playback.key`. Server and Identity get **separate**
secrets and units. File contracts keep their owners' validations: absolute path, owner-only
mode (`0400`/`0600`) owned by the runtime user, exact 32-byte keys and a service-account file
whose project matches ([Firebase verification](../architecture/firebase-verification.md#production-configuration-images-and-vps)).
Values never enter image layers, the checkout, Web/WASM bundles, CMP/APK artifacts or env dumps.
A host without its own workload identity receives values only through this operator handoff,
never project-wide credentials.

**Operations.** SSH and Android signing material are read on the operator workstation only.
Prefer a scoped deploy user over root; storing a key does not make it safe, and loss requires
host-side revocation.

### SSH retrieval to a private local file

This is an operator procedure for a separately authorized read of an already imported numeric
version. It is not executed by builds, CI or an import, and it installs no consumer. Review the
secret/version metadata and reader first. For operator SSH, the operator account is the consumer
reader; do not infer `VERIFIED_ACCESS` from another account's import or metadata result.

```sh
(
  set -eu
  umask 077
  SSH_SECRET_VERSION=REVIEWED_NUMERIC_VERSION
  case "$SSH_SECRET_VERSION" in
    ''|0|*[!0-9]*) exit 1 ;;
  esac
  ssh_key_dir=$(mktemp -d "${TMPDIR:-/tmp}/cantos-ssh.XXXXXXXX")
  chmod 700 "$ssh_key_dir"
  # Remove only the temporary copy created by this procedure, never the source input.
  trap 'rm -f "$ssh_key_dir/id"; rmdir "$ssh_key_dir"' EXIT
  gcloud secrets versions access "$SSH_SECRET_VERSION" \
    --secret=cantos-ops-vps-ssh-private-key --project=cantos-a348b \
    --account=OPERATOR_EMAIL --out-file="$ssh_key_dir/id"
  chmod 600 "$ssh_key_dir/id"
  # Use this file only within the separately approved SSH scope.
  # Keep established host-key verification; never bypass it or change known_hosts here.
)
```

`--out-file` keeps the opaque key off stdout and preserves its bytes. Do not use shell command
substitution to load the key, `cat` it, export it in an environment variable, print a public-key
derivative/fingerprint as evidence, or place payloads in argv/logs. A local readback may establish
file delivery for that reader/version; it does not establish successful SSH, host configuration
or deployment. Cleanup removes the temporary copy but does not claim secure erasure or erase
the approved existing source. Preserve that source until its owner authorizes its disposition.

## Version pin, rotation, rollback and incidents

- Production and ops bindings name a numeric version, recorded as `pinned_version` in the
  inventory by a reviewed change. `latest` is for operator discovery only; the inventory rejects
  it as a pin and `status` fails when a pin is not `ENABLED`.
- Rotate: import a new version → have the consumer's reader verify it → bind it → update the
  pin → revoke the old credential at its provider → disable the old version.
- Roll back: bind and pin a known-good **enabled** version. Disable a bad version; destroy only
  after confirming nothing references it.
- Revoking IAM stops future reads but does not erase plaintext already fetched. Compromised
  tokens, passwords and keys are revoked at their owning provider or host.
- Errors and ledgers name the failure class and whether a remote write is uncertain; never a
  payload or provider error body.

## Agent execution contract

1. Read this guide, [AGENTS.md](../../AGENTS.md), the consumer's config and the
   [`secrets` command contract](../../xtask/README.md#secrets); classify each credential by name,
   purpose, environment and consumer.
2. Missing value or missing authorization: report it as `MISSING` and ask for a secure local
   route. Never request it in chat or GitHub, invent it, harvest the environment or reuse
   another system's key.
3. Never assume a Console login grants CLI access. Use only an account the owner approved, with
   the narrowest role and an explicit numeric version.
4. Source changes and synthetic tests may proceed without live access. Report GCP API, IAM,
   import, CI and VPS evidence separately as `PASS`/`FAIL`/`NOT_RUN` with the exact target.
5. Importing, changing IAM, changing a production binding and deploying are distinct explicit
   remote effects; none is chained to a build, preflight or Git operation.
