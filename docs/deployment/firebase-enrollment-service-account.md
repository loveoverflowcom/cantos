# Firebase enrollment: dedicated service account and key lifecycle

**Status: proposed design and an operator runbook. NOT EXECUTED.** No service account, role,
binding or key described here has been created, and nothing in this document or its code change
calls Google Cloud or Firebase. [ADR0036](../decisions/0036-firebase-enrollment-proof.md) owns the
decision; the [operator plan](firebase-enrollment-proof.md) owns the other activation gates; the
[runtime ledger](../evidence/firebase-enrollment-runtime.md) separates what is implemented from
what is verified.

Three layers are kept apart throughout:

| Layer | State |
| --- | --- |
| Implemented in code | The API mints, caches and renews its own Auth-create bearer from the service-account key. Verified only against local fake token endpoints. |
| Proposed design | The dedicated account, custom role, scope, key handling and retirement plan below. |
| Verified live | **Nothing.** Not the scope, the custom role's sufficiency, the custom-token signer requirements, the Podman secret mount or the retirement of the old key. |

## What the code does

`FIREBASE_SERVICE_ACCOUNT_FILE` (read once at startup) holds the RS256 key. For the single call
that needs a bearer, the API signs a JWT-bearer assertion (`iss` = the account's `client_email`,
`scope`, `aud` = `https://oauth2.googleapis.com/token`, 5-minute `exp`) and POSTs it, form-encoded,
to that fixed origin with `grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer`. The response
must be a `Bearer` token valid for more than 5 and at most 60 minutes. It is held in RAM only,
reused until 5 minutes before expiry, then replaced; concurrent callers share one request, a failed
refresh never serves the stale token, and the next call retries. The token endpoint is never taken
from the key file's `token_uri`; the client follows no redirects, uses no inherited proxy, opens
fresh HTTP/1 connections and has a 2s connect / 5s total bound. A refusal is a typed closed error
before the Auth create is dispatched, so a failed mint cannot create a user, but it does end that
enrollment attempt like any other pre-dispatch failure (no automatic retry); a fresh attempt gets a
new UID.

The static `FIREBASE_ADMIN_ACCESS_TOKEN` input is **removed**, not kept as an alternative: a mixed
mode would preserve the one-hour failure and a second credential path. Setting it now fails
startup, like tenant, emulator and endpoint inputs.

## Calls the code makes

Only the first needs an OAuth bearer, so only the first needs an IAM permission. Every other
Firebase call authenticates with the API key (server-side) plus a user credential the caller holds.

| # | REST call (`identitytoolkit.googleapis.com/v1/`) | Credential | IAM permission |
| --- | --- | --- | --- |
| 1 | `accounts:signUp` with `localId`, `email`, `emailVerified:false`, `targetProjectId` (dedicated unverified user) | API key + **minted bearer** | `firebaseauth.users.create` (the [access-control table](https://docs.cloud.google.com/identity-platform/docs/access-control) maps `SignUp` to it) |
| 2 | `accounts:signInWithCustomToken` | API key + a custom token the API signs locally with the account's key | none; the key's account must exist in the project (see "Unverified assumptions") |
| 3 | `accounts:sendOobCode` (`VERIFY_EMAIL`) | API key + the ID token from 2 | none |
| 4 | `accounts:resetPassword` (non-consuming inspect, `oobCode` only) | API key | none |
| 5 | `accounts:update` (consume the same `oobCode`) | API key | none |
| 6 | `https://oauth2.googleapis.com/token` (mint) | signed assertion | none (it authenticates as the account) |

The documented scopes for `accounts:signUp` are `https://www.googleapis.com/auth/identitytoolkit`
or `https://www.googleapis.com/auth/cloud-platform`. The code requests **only
`identitytoolkit`**. The scope bounds the token; the IAM role bounds the account; both are needed.

## Proposed account and role

- Account `cantos-enrollment@PROJECT_ID.iam.gserviceaccount.com`, dedicated to this flow, used by no
  other service, with no console login and no human impersonation grant.
- A project **custom role** with exactly one permission, `firebaseauth.users.create`.
- One project-level binding of that role to that account only. The Auth API has no finer resource
  scope than the project.

The account must NOT hold: `roles/owner`, `roles/editor`, `roles/firebase.admin`,
`roles/firebaseauth.admin` or the Firebase Admin SDK service-agent role (these read, update and
delete every user and change configuration), any `iam.serviceAccountKeyAdmin`/`serviceAccountAdmin`
/`serviceAccountTokenCreator` grant (on itself or others), `roles/serviceusage.*`, Hosting,
Functions, Storage, Firestore or any deploy role, and no `firebaseauth.configs.*` or
`firebaseauth.users.update|delete|get` permission. The API's own runtime database role and the
Kanidm onboarding token are separate credentials with separate owners.

Residual authority of a stolen key (accepted, not removed): create Auth users in this project,
including with `emailVerified:true`; mint custom tokens for any UID it chooses; and with the API key
send verification mail to arbitrary addresses for users it creates. Cantos admits no Firebase ID
token as authentication and keeps Kanidm as issuer, so this cannot open a Cantos session, but it can
burn mail quota and fill the user table. Restrict the API key to the Identity Toolkit API in the
console, keep Firebase client sign-up disabled, and watch Auth user counts and mail volume.

## Key handling

- The key exists only as one JSON file: created on a trusted operator workstation in a mode-0700
  directory with `umask 077`, never in Git, chat, a tool-result transcript, argv or an environment
  variable. Print no contents; compare only the key-id **prefix**.
- It reaches the VPS as a file copy (the secret is never a command-line argument), is installed
  root-only and becomes a Podman secret mounted read-only into the API container at an absolute path
  that `FIREBASE_SERVICE_ACCOUNT_FILE` names. The local copy is then securely deleted.
- The API reads it once at startup (regular file, at most 64 KiB), keeps only the parsed RSA key in
  RAM and never writes or logs it, its path or any token.
- Rotation: at most every 90 days and after any suspected exposure. Create a second key, deploy it,
  restart the API (the file is not re-read), verify one enrollment, **disable** then delete the old
  key. Emergency revocation is `keys disable`; an already minted access token lives at most about an
  hour and disappears when the process restarts.
- An organization policy `iam.disableServiceAccountKeyCreation` would block step 5; stop and decide
  separately rather than relaxing it.

## Runbook (NOT EXECUTED)

Everything below needs the operator's explicit approval naming the project, run from an
authenticated `gcloud`. `PROJECT_ID` is the Firebase project (`cantos-a348b` per the live record, to
be re-verified, and itself an unaccepted binding). Take no step on the VPS from this document alone;
the final container wiring belongs to the VPS deployment runbook.

```sh
# 0. Read-only pre-checks. Stop if the account or role already exists unexpectedly.
PROJECT_ID=cantos-a348b
SA_ID=cantos-enrollment
SA_EMAIL="$SA_ID@$PROJECT_ID.iam.gserviceaccount.com"
ROLE_ID=cantosEnrollmentUserCreate
gcloud iam service-accounts list --project "$PROJECT_ID"
gcloud iam roles list --project "$PROJECT_ID" --format='value(name)'

# 1. Custom role with the single permission.
gcloud iam roles create "$ROLE_ID" --project "$PROJECT_ID" \
  --title "Cantos enrollment: create Auth user" \
  --description "Only accounts:signUp admin create for Cantos enrollment" \
  --permissions firebaseauth.users.create --stage GA

# 2. Dedicated service account.
gcloud iam service-accounts create "$SA_ID" --project "$PROJECT_ID" \
  --display-name "Cantos enrollment (Auth create only)"

# 3. The only binding.
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
  --member "serviceAccount:$SA_EMAIL" \
  --role "projects/$PROJECT_ID/roles/$ROLE_ID" --condition None

# 4. Read-only proof of least privilege: expect exactly the custom role.
gcloud projects get-iam-policy "$PROJECT_ID" --flatten bindings \
  --filter "bindings.members:serviceAccount:$SA_EMAIL" --format 'value(bindings.role)'

# 5. Key into a private directory; never print it.
( umask 077; mkdir -p "$HOME/.cantos-private/firebase-enrollment" &&
  gcloud iam service-accounts keys create \
    "$HOME/.cantos-private/firebase-enrollment/key.json" \
    --iam-account "$SA_EMAIL" --key-file-type json )
```

VPS delivery (operator, root on the host; adjust the host and paths to the deployment runbook):

```sh
# Copy the file (contents never appear in argv), then install it root-only.
scp -p "$HOME/.cantos-private/firebase-enrollment/key.json" VPS:/root/firebase-enrollment-key.json
ssh VPS 'install -m 0400 -o root -g root /root/firebase-enrollment-key.json /root/cantos-fb-sa.json \
  && podman secret create cantos_firebase_sa /root/cantos-fb-sa.json \
  && shred -u /root/firebase-enrollment-key.json /root/cantos-fb-sa.json'
shred -u "$HOME/.cantos-private/firebase-enrollment/key.json"
```

Mount it in the API container with `--secret cantos_firebase_sa,type=mount,target=firebase-sa.json,mode=0400,uid=API_UID`
(it appears at `/run/secrets/firebase-sa.json`) and set
`CANTOS_THEATRE_REGISTRATION_FIREBASE_SERVICE_ACCOUNT_FILE=/run/secrets/firebase-sa.json`. Remove the
retired `..._FIREBASE_ADMIN_ACCESS_TOKEN` line; startup refuses it.

Live acceptance (separately approved, one approved recipient, within the mail budget): one real
enrollment must pass create → exchange → mail → action. A `403 PERMISSION_DENIED` on `signUp`
identifies a missing reviewed permission; add only that permission to the custom role, never
Editor/Owner/Admin. A rejected `identitytoolkit` scope means a reviewed switch to `cloud-platform`
(one constant in `firebase_bootstrap/access.rs`) with this document and its tests updated.

Retire the broad default Admin SDK key (id prefix `2795eafc`, recorded in the
[live record](../evidence/theatre-live-auth-206.md)) **only after** acceptance above passed, each
step with its own approval:

```sh
ADMINSDK_EMAIL=firebase-adminsdk-XXXXX@cantos-a348b.iam.gserviceaccount.com   # from step "list"
gcloud iam service-accounts keys list --iam-account "$ADMINSDK_EMAIL" \
  --format 'table(name.basename(), validAfterTime, keyType)'     # find the id starting 2795eafc
gcloud iam service-accounts keys disable KEY_ID --iam-account "$ADMINSDK_EMAIL"   # reversible; observe
gcloud iam service-accounts keys delete  KEY_ID --iam-account "$ADMINSDK_EMAIL"   # destructive
```

Then delete every local copy of that old key (the operator's private live-test directory, any shell
history or transcript that held it) and the other [recorded cleanup](../evidence/theatre-live-auth-206.md)
items (test Firebase user, Kanidm person, local containers). The default Admin SDK account is
Firebase-managed and is not deleted.

## Unverified assumptions (each needs the live acceptance above)

1. `identitytoolkit` is sufficient for `signUp` with `localId`/`targetProjectId`.
2. `firebaseauth.users.create` alone satisfies the admin create, and the permission is allowed in a
   custom role.
3. `signInWithCustomToken` accepts a token signed by this new account without further grants.
4. The Podman secret `mode`/`uid` options and the mounted path work on the deployed Podman version.
5. Project binding (the exact project, the API-key association) is separately unaccepted.
