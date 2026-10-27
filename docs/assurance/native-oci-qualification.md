# Exact native OCI pair qualification

`cargo xtask release oci-qualify` is the disposable-Linux fixture for the four
[native OCI gates](../deployment/native-oci-release.md#native-release-manifest-v1).
It exercises the production Server/Identity images and their migration executables, not Rust
test binaries, mocked HTTP handlers or `FakeHost`. It creates no admission or activation receipt.
The existing manual [OCI workflow](../../.github/workflows/native-oci-publish.yml) runs it after
publication/readback and retains the original unadmitted candidate plus a separate result.
Ordinary PR CI runs deterministic regressions; it does not claim image acceptance.

## Inputs and execution boundary

Use a disposable native Linux runner matching the exact `linux/amd64` or `linux/arm64` input.
Rootful Podman, Python 3.11+, `nsenter` and `ip` must already be available. macOS, emulation,
missing tools and non-root execution cannot establish acceptance. No machine is provisioned,
no sysctl/IAM/security settings are changed, and no production host or database is contacted.
The fixture uses a private run-owned VFS image store, never the host's existing Podman store.

Check out the exact candidate source with its Git objects, clean tracked/untracked files and
matching qualifier implementation. Supply all pins explicitly, as root on the disposable runner:

```sh
cargo xtask release oci-qualify \
  --manifest /private/candidate/manifest.json --manifest-digest FULL_SHA256 \
  --source-sha FULL_COMMIT_SHA --configuration-digest FULL_SHA256 \
  --platform linux/amd64 --output /private/fresh-qualification
```

For private GHCR packages, add `--authfile /private/ghcr-auth.json`: an existing root-owned 0600
authfile with the same bounded GHCR-only validation as the registry observer. Credentials never
reach the containers/HTTP/SMTP driver, receipts or a registry write. CI snapshots its existing
job authfile, removes that snapshot on exit and adds no permission.

Source, build configuration, platform, full registry refs and separate OCI config digests must
match the candidate, committed policy, registry bytes, pulled image and actual processes. The
runtime bundle and input evidence/record bytes are checked before runtime starts. Input gate
statuses are never reused. The candidate remains unchanged; output must be fresh.

## Gate coverage

| Record | Real boundary and positive/negative controls |
| --- | --- |
| `image-pair` | GHCR manifest/config readback; digest pull; fixed service UID/GID; actual process UID, zero effective capabilities and no-new-privileges; exact DB/socket mounts; all saved layer descriptors and uncompressed diff IDs; required executable set; serve-before-migrate refuses without creating data; current-schema migrations; production public HTTP readiness |
| `native-isolation` | Pair namespace differs from host, without external route; SMTP loopback positive and host-loopback sentinel negative; each UID reads its own DB and cannot read the other even when deliberately mounted; mounted synthetic credential/executable positives and absent-mount negatives; 0660 socket; real Server UID denied by a wrong Identity caller allowlist; stopped Identity fails closed |
| `native-e2e` | Production Server HTTP → OS-peer-authenticated Identity RPC → real Turso/Argon2id; local SMTP proof; confirmation/wrong-proof refusal; completion creates no session; exact replay; wrong password; native/browser and Studio refusal; native login/session/refresh; Secure/HttpOnly/SameSite cookie; Web CSRF/logout; policy lifetimes |
| `migration-restore` | Both owners stopped before copying every DB/sidecar; source/copy hash, size, mode, UID/GID equality; pinned backup rechecked before use; populated current-schema migration; HTTP verifies account/password/session; revocation persists across restart; fresh-root restore verifies original account/session |

Restore covers current-schema creation, idempotent migration and same-schema restore. It does
not accept historical schema upgrade/downgrade, restore rotation, arbitrary compatibility, live
VPS backup/restore, Quadlet/systemd/reboot, real-provider mailbox delivery, browser/device,
Kani, full token/proof expiry elapsed in wall time, or unported business journeys. No expiry/auth
policy, clock override or preseeded credential is introduced. These limits remain separate work.

The separate [historical v23-to-v24 rehearsal](../deployment/native-existing-host-migration.md#evidence-before-effects)
uses the old pair to create/populate v23 and restores that original pair after the actual upgrade.
Its `historical-upgrade.json` is independently source/image bound; it never relabels the
`migration-restore` record above or establishes live host/adoption acceptance.

The serve-before-migrate negative control allows only the owner's empty 0600 `.db.owner` lock
with the exact service UID/GID. The owner acquires that advisory lock before observing/refusing
a missing database. Creating a DB, WAL, unknown file, nonempty lock or permissive/foreign lock
still fails; refusing an uninitialized DB does not require removal of the ownership sidecar.

Peer-UID reconfiguration removes the owned Server before Identity because Server joins
Identity's network namespace. Both are recreated in the driver's retained private namespace;
correct credentials must work again after the negative control. No recursive dependent-removal
or broad cleanup option is used. Podman failures identify the operation and exit code without
echoing command arguments or remote stderr.

The wrong-peer request expects HTTP 503: Identity drops non-allowlisted OS peers before reading
RPC bytes, which the Server maps to unavailable transport, not password denial (401). This is
accepted only alongside the actual wrong-caller runtime environment and both static Identity
`ready` and `connection_refused` events; a generic 503 alone cannot pass this control. Correct
credentials must work before and after restoring the peer policy. Raw logs are never emitted.

The all-layer policy includes files deleted by later layers. It rejects database/audio assets,
credential paths/files and private-key payload markers, and checks the exact service binary set.
CA certificates and executable parser strings are allowed. This disclosed policy is not a
universal proof of absence of secrets. Operator review of source/build inputs remains required.

Only synthetic `.test` addresses, generated proof/session secrets, a synthetic password and a
run-owned SMTP sink are used. Mail/credentials stay in driver memory. Host subprocesses receive
a small environment allowlist. Containers are removed only by recorded run-owned names; the
private store and disposable data are removed with their run directory. No broad prune, host
restore, production cleanup or registry retry occurs.

## Receipts and retrieval

Dispatch **Native OCI publish** on the intended branch/ref once. The producer enforces matching
source/workflow SHA and first attempt; do not rerun uncertain publication. Reconcile its journal
before deciding on a new authorized attempt. The workflow never deploys.

Download `native-oci-candidate-FULL_SHA` from that exact Actions run. It includes:

- `release-build/oci/.../candidate/`: original `NOT_RUN` candidate and publication journal.
- `release-qualification/exact-pair/`: `attempt.json`, `qualification.json`, new `manifest.json`,
  `evidence.json`, source-bound `runtime.tar` and four `gate-records/GATE.json` files.

Each record binds source/config/platform/full image objects, input manifest digest, fixture
configuration digest (exact helper bytes and synthetic runtime settings), runner identity,
timestamp and assertions. Layer records retain config/diff IDs; restore records retain all file
hashes/sizes/modes/owners. Completed gates checkpoint immutable records before proceeding. The
final manifest pins new evidence and all gate records by content digest. Review the summary and
all records, not just job color.

`PASS` means named checks actually completed here, with `evidence_level=integration-tested` and
`execution_environment=linux-native-exact-images`. `FAIL` identifies an executed gate's failed
assertion/command; later gates stay `NOT_RUN`. Unsupported prerequisites produce `NOT_RUN`
without integration labels and a nonzero exit. Missing final summary/runner loss is incomplete:
checkpoints establish only their own checks. Missing records never become PASS. Input/pin
refusal happens before containers start and manufactures no qualification record.

Deterministic tests cover mismatched input/config/registry digest/UID, dirty qualifier, unavailable
Linux without integration claims, tampered backup refusal before restore, all-layer negatives,
and fabricated input PASS declarations discarded when execution fails. These are core/file/mock
tests, not runtime PASS.

The existing offline verifier can validate a self-consistent fabricated record: it authenticates
neither authorship nor execution. Hashes establish binding, not trust. This runner never accepts
input PASS as an execution shortcut. Output remains UNADMITTED and `release_eligible=false`;
separate operator admission and existing host gates remain mandatory. This fixture authorizes
no rollout.
