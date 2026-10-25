# Google Cloud Secret Manager — Cantos operator setup and secret custody

**Status: proposed runbook, not a production activation record.** Tracked by
[issue #446](https://github.com/loveoverflowcom/cantos/issues/446).
This document contains **no secret values**. No Google Cloud resource, IAM grant, secret
version, VPS runtime mount, or CI identity is created by committing this file.

## Scope and authority

- Google Cloud **project ID**: `cantos-a348b` (owner-selected, nonsecret).
- [Secret Manager console](https://console.cloud.google.com/security/secret-manager/secrets?project=cantos-a348b).
  The browser's `authuser` selector is **not** an API credential or proof of permission.
- Secret Manager owns provider/API credentials, file-based Identity credentials and
  operational keys. **Git owns only names, mapping, policy, operator instructions and
  nonsecret version references; no SOPS ciphertext or private Age identity is required.**
- `infra/` owns nonsecret infrastructure mapping; `xtask/` owns any justified
  operator command, validation and provider-invocation implementation. Do not add a
  public Cantos HTTP endpoint for uploading or reading secrets.
- This guide does not authorize DNS changes, service restarts, deployment, credential
  revocation, billing changes or broad IAM policy grants.

The existing [native OCI plan](native-oci-release.md) is not a live host-activation
workflow. [CircleCI release flows](circleci-builds.md) and
[Firebase/Cloudflare operator flows](cloudflare-pages.md) have their own approval
boundaries. An import is **not** evidence that Server or Identity received the value.

## Target secret inventory — proposed names, not provisioned resources

Enumerate actual consumers before provisioning. Candidate names are illustrative;
an item becomes active only after verifying the current service contract and its owner.

| Example secret ID | Class | Existing consumer/contract | Delivery |
| --- | --- | --- | --- |
| `cantos-dev-identity-smtp-password` | development | Identity `CANTOS_IDENTITY_SMTP_PASSWORD` (Resend SMTP, when selected) | Scoped runtime value |
| `cantos-prod-identity-smtp-password` | production | Identity `CANTOS_IDENTITY_SMTP_PASSWORD` | Identity runtime only |
| `cantos-prod-identity-firebase-api-key` | production | Identity `CANTOS_IDENTITY_FIREBASE_API_KEY` | Identity runtime only |
| `cantos-prod-identity-firebase-service-account` | production | Identity `CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE` | Private runtime file, **not** JSON in an environment variable |
| `cantos-prod-identity-firebase-envelope-key` | production | Identity `CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE` | Private runtime file with exact owner-required bytes |
| `cantos-prod-server-s3-secret-access-key` | production | Server `CANTOS_S3_SECRET_ACCESS_KEY`, only when S3 is configured | Server runtime only |
| `cantos-ops-cloudflare-api-token` | operations | Explicit Cloudflare deployment command | Selected deploy job/operator only |
| `cantos-ops-vps-ssh-private-key` | operations | Operator-approved SSH access only | **Never** Server, Identity or ordinary CI build |

Avoid importing nonsecret origins, project IDs, hostnames or public browser
Firebase configuration solely because their names contain `KEY`. Conversely,
never place service account JSON, signing keys, passwords or a private SSH key
in a checked-in example. Additional actual secrets and owner-specific permissions
must be derived from source, not guessed from an obsolete `.env.example`.

Treat credentials previously disclosed outside their intended protected store
as **rotation-required** after import; Secret Manager cannot revoke an existing
provider token or SSH credential. Importing a root SSH identity is an exceptional,
explicit operator action, not a normal migration step.

## Bootstrap on an authorized trusted machine

These are **opt-in external operations**. Confirm the intended Google account,
project, billing state, Secret Manager API and IAM permissions before running
commands that mutate a Google project. Run from a trusted workstation/approved
operator environment, not a public build job or untrusted AI sandbox.

```sh
PROJECT_ID=cantos-a348b
gcloud auth login                           # interactive; operator only
gcloud auth list                            # verify the selected identity
gcloud projects describe "$PROJECT_ID"    # verify the exact target
gcloud services list --enabled --project="$PROJECT_ID" \
  --filter='NAME:secretmanager.googleapis.com'

# Only after explicit authorization and project/billing checks:
gcloud services enable secretmanager.googleapis.com --project="$PROJECT_ID"
```

Project owner/bootstrap permissions are not suitable for regular readers or CI.
Creating a secret may require Secret Manager Admin privileges; adding a version
can use `roles/secretmanager.secretVersionAdder` on the selected secret.
Only the specific authorized reader receives
`roles/secretmanager.secretAccessor` on each **individual secret**.
Check the [official IAM guidance](https://docs.cloud.google.com/secret-manager/docs/manage-access-to-secrets).

## Upload through the Secret Manager API

Google offers the versioned [Secret Manager v1 API](https://docs.cloud.google.com/secret-manager/docs/creating-and-accessing-secrets):
`projects.secrets.create` first, then `projects.secrets.addVersion` for new
contents. The supported `gcloud` CLI calls this API and is the simplest manual
import route. The bytes must be available **on the authorized operator machine**.

```sh
PROJECT_ID=cantos-a348b
SECRET_ID=cantos-dev-identity-smtp-password
# Secure approved local file, outside the repo; this is NOT a sample credential.
# Ensure file permissions and content have been verified before proceeding.
APPROVED_INPUT_FILE=/absolute/operator-private/secret-input

# One-time metadata creation (remote write, no payload):
gcloud secrets create "$SECRET_ID" \
  --project="$PROJECT_ID" --replication-policy=automatic

# Add an immutable secret version from exact file bytes (remote write):
gcloud secrets versions add "$SECRET_ID" \
  --project="$PROJECT_ID" --data-file="$APPROVED_INPUT_FILE"

# Verify metadata/version/state ONLY — never print the secret payload:
gcloud secrets versions list "$SECRET_ID" \
  --project="$PROJECT_ID" --format='table(name,state,createTime)'
```

Never put the raw value in a shell argument, `echo` command, Git diff, CLI log,
issue/PR, shell history, build artifact or a local test fixture. For credentials
already held in an authorized local file, prefer `--data-file`; for programmatic
import, pass payload bytes through a restricted stdin/API request path without
materializing a JSON body in the checkout. A locally cached plaintext file
still needs separate protection and secure lifecycle control.

Creating and **adding a version** are distinct operations. If an upload
times out or returns an uncertain result, reconcile existing version metadata
before retrying; never blindly add duplicate versions. Keep an import ledger
containing only secret ID, version ID, actor, time and result, without hashes
of small or guessable credentials, payloads, or sensitive file paths.

For automation, implement a **locally invoked, allowlisted operator helper**
under the existing `xtask` command owner only when there is a real consumer.
Its default dry-run must disclose planned names, scope and remote effects
without reading payload bytes or performing remote writes. No blanket
`printenv`/bulk environment harvest, remote upload from a CI pull request, or
server-side generic secret upload endpoint.

## Credentials and runtime delivery

**Development.** Use explicitly authorized interactive `gcloud` credentials
for narrowly scoped development secrets. Ordinary Rust builds, tests and the
local Auth Emulator must **not** inherit Google/Firebase/SMTP credentials:
[Auth Emulator isolation](../../docs/architecture/firebase-verification.md#local-development-composition)
refuses these inputs by design. Never bypass that check.

**CI/CD.** Evaluate the **actual** owner of the release job first:
[CircleCI](circleci-builds.md) already owns part of Cantos' build/promotion
policy; native GitHub Actions are not the default for all deployments. If a
trusted GitHub Actions deployment consumer exists, prefer short-lived
[Workload Identity Federation](https://docs.cloud.google.com/iam/docs/workload-identity-federation-with-deployment-pipelines)
with numeric repository/owner identity checks, reviewed ref/workflow conditions
and `id-token: write`. Grant only explicit deployment jobs access to
specific secrets. Public PR, checkout, compile and test jobs must not receive
production credentials. Do not export a long-lived Google service account
JSON key merely to simplify CI.

**VPS.** The current deployment contract uses operator-owned Podman secrets
and private read-only runtime files. A permitted deploy-time reader can fetch a
**pinned numeric secret version**, verify the target, and install it into the
existing protected runtime store. File-based Identity inputs must retain the
validations in the [Firebase verification contract](../architecture/firebase-verification.md#production-configuration-images-and-vps):
`CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE` and
`CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE` are private absolute-file paths.
Keep the material out of image layers, checkout, public web bundles, CMP
artifacts and general-purpose env dumps. A host without a trustworthy workload
identity must not be given unrestricted project-wide credentials; prefer an
authorized deployment-side handoff. Live native activation still requires its
own reviewed host/rollback/health evidence under #348.

**Operations.** Store any approved VPS SSH private key under an OPS-only secret.
An app runtime must never access it. Prefer a narrowly scoped deploy user to
a reusable root login; importing an existing root key does not make it safe,
and loss requires host-side key revocation/rotation.

## Version pin, rotation, rollback and incidents

- Record a specific numeric Secret Manager version for production release
  configuration, never blindly use `latest`. `latest` is acceptable only for
  intentional operator inspection and version discovery.
- Upload a new version; validate it with a selected consumer; update the
  reviewed reference; then revoke the old provider credential as applicable.
- If a version is wrong, roll back the binding to a known valid enabled
  version. Disable an obsolete version and verify dependencies before destroy.
- Revoking IAM blocks future API reads but **does not erase plaintext already
  fetched** by a machine or person. Compromised tokens, passwords and SSH keys
  must be revoked/rotated at their owning provider/host.
- Redact provider errors, execution receipts and logging; record failure type
  and whether a remote write has an uncertain outcome.

## Agent execution contract

When asked to develop Cantos:
1. Read this guide, [AGENTS.md](../../AGENTS.md), current service config and
   the relevant `xtask` command owner; classify each required credential by
   **name, purpose, environment and consumer**.
2. If an approved value is missing, ask for a **secure local provisioning
   route**. Never request the raw value in GitHub comments or chat, invent
   credentials, collect unrelated shell secrets, or use the supplied key of
   another system.
3. Use the lowest-privilege identity and specific version. A successful
   metadata check is not proof of content access or runtime application.
4. Source changes and synthetic tests may proceed without live access.
   Report GCP API, IAM, CI and VPS evidence as separate
   `PASS`/`FAIL`/`NOT_RUN` statuses with exact target/environment.
5. Importing credentials, modifying IAM, changing production bindings and
   deploying are **distinct explicit remote effects**. Do not chain them
   to a build, preflight or normal Git operation.

See [issue #446](https://github.com/loveoverflowcom/cantos/issues/446)
for the scoped implementation prompt and pending acceptance.
