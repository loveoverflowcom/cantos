# Google Cloud Secret Manager — Cantos operator setup and secret custody

**Status:** the operator import helper (`cargo xtask secrets`) and the tracked inventory are
**implemented and tested against a scripted gcloud with synthetic payloads**. Live project
setup, IAM grants, imports, CI workload identity and runtime binding are **NOT_RUN** until an
operator performs and records them. Tracked by [issue #446](https://github.com/loveoverflowcom/cantos/issues/446).
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
| ops | `cantos-ops-android-release-keystore` (+ `-keystore-password`, `-key-password`) | `distribute firebase --channel production` `CANTOS_RELEASE_*` | operator workstation only |
| ops | `cantos-ops-vps-ssh-private-key` | operator SSH | operator workstation only; **never** Server, Identity or CI |

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
| Operator workstation | `roles/secretmanager.secretAccessor` | `cantos-ops-android-*`, `cantos-ops-vps-ssh-private-key` |

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
