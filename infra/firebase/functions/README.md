# Private Forge smoke package

This is the concrete #486 consumer of #212's Functions policy location. Runtime source belongs
in `services/cantos-forge-functions/firebase`; the policy here selects only project
`cantos-a348b`, codebase `cantos-forge-smoke`, function `forgeSmokeProbe`, Node 22 and a static
Linux x86_64 musl worker. It contains no Hosting/App Distribution config, aliases, secrets,
predeploy hook or producer trigger. No region or invoker principal is selected implicitly.

The existing Node 22 candidate in [ADR 0022](../../../docs/decisions/0022-cantos-forge-producer-cli.md)
owns this runtime choice. The pinned SDK registers a Gen2 HTTP trigger with `invoker: private`,
CORS disabled, min/max instances 0/1, concurrency 1, CPU 1, memory 256 MiB and timeout 10 seconds.
The worker deadline is 5 seconds. These resource limits bound this probe, not a provider budget.
IAM authentication is enforced by the cloud platform; a local invocation cannot verify it.
`forgeWorkerHttp` stays unavailable and is never exported as a Firebase trigger.

## Local source and package verification

```sh
cargo xtask verify forge
cargo xtask forge-probe self-test
cargo xtask forge-probe package --project cantos-a348b --region REGION --output artifacts/forge-probe/RUN --dry-run
```

`REGION` is an operator-selected real Functions region; `asia-southeast1` in tests is only a
fixture. Python validates the project and region syntax, resolves output from the repository
root, refuses an existing directory or an escape outside `artifacts/forge-probe`, and uses no
subprocess/network/artifact write in dry-run. The xtask binary works from other working directories;
all paths in its plan remain literal arguments, including spaces.

For a real package, supply Rust 1.98.1 and the already installed
`x86_64-unknown-linux-musl` target plus linker. No target/compiler/VM is installed or started by
these commands. `package` builds `cantos-forge-worker` with locked Cargo dependencies and the
explicit Linux target in `target/forge-probe-build`, then stages an allowlisted wrapper and
static ELF worker into a **new** run directory. An absent target reports `UNSUPPORTED`. A native
macOS build is refused. Cross-build success still requires actual Linux runtime qualification.

```sh
cargo xtask forge-probe package --project cantos-a348b --region REGION --output artifacts/forge-probe/RUN
cargo xtask forge-probe inspect --bundle artifacts/forge-probe/RUN
# On disposable Linux x86_64 with Node 22 and npm already available:
cargo xtask forge-probe qualify --bundle artifacts/forge-probe/RUN
cargo xtask forge-probe deploy-plan --bundle artifacts/forge-probe/RUN
```

The run directory contains `firebase.json`, `functions/`, a local qualification test and
`receipt.json`. The config has exactly one Functions codebase with source `functions`, below
the same Firebase project directory. No `../../services` upload path is used. The package's
`main` is `functions.mjs`; its only registered export is `forgeSmokeProbe`. Firebase installs
the exact lockfile dependencies at build time; there is no install/build/download during an HTTP
request. `qualify` explicitly installs that SDK locally using `npm ci --ignore-scripts`, with no
lifecycle scripts, then tests discovery and actual loopback HTTP against the fixed packaged
worker. Missing binary, malformed reply, deadline and recovery use disposable local faults;
the saved binary is restored in `finally`. Run only on a disposable package, never deployed files.

The receipt binds compiler, source commit, source-input digest, target, binary SHA-256, mode and
all staged source bytes. ELF inspection requires ELF64 little endian x86_64 with a load segment,
no interpreter or dynamic dependency and exact executable mode `0755`. This removes a glibc
loader dependency; it does not establish sandbox compatibility, kernel support, cold-start or
cloud execution. A receipt is an operator-held integrity record, not a signed build attestation.
A digest is no substitute for running the binary. `inspect` refuses unexpected files, symlinks,
missing/mismatched hashes, rewritten config and wrong binary mode. `node_modules` is local-only
qualification output and is excluded by Firebase policy; it is not part of source provenance.

Retain the receipt and package until deployment/recovery is reconciled. An interrupted package
without a complete receipt is not deployable; do not overwrite it. Prepare a fresh run after
inspection. No command deletes an old bundle, cloud function, log, key, backup or other session.

## Operator-only cloud phase

Source preparation authorizes no cloud write. Before a deploy the operator must explicitly
approve the project, chosen region, invoker principal, service identity and billing/budget scope.
Read the current enabled APIs, existing Gen1/Gen2 functions/codebase labels and IAM first. A
`SERVICE_DISABLED`, denied read, missing budget or unknown principal leaves cloud `NOT_RUN`;
never treat it as an empty project or permission to enable APIs. Do not print access/identity
tokens, account credential files, headers containing credentials or private runtime diagnostics.

The local plan prints the exact underlying Firebase primitive with an absolute effective config,
explicit project and only `functions:cantos-forge-smoke:forgeSmokeProbe`. It never invokes it.
This selector and run-relative source were checked against installed Firebase CLI **14.2.2**
(`functionsDeployHelper.parseFunctionSelector` and config parsing); recheck the actual CLI version
before separately authorized operator execution. Do not use blanket `firebase deploy`, `--force`,
predeploy builds, implicit aliases, a Hosting selector or a broader Functions codebase. Firebase
can enable APIs/create build artifacts on deployment, so those effects also require operator
approval. Do not add `allUsers` or `allAuthenticatedUsers`; private invoker is the default.

After the authorized write, use the URL actually returned by Firebase/GCP. Do not manufacture a
region/project URL. Record the redacted command/result, CLI version, project/region/codebase,
function/build/revision identifiers, returned URI, source-input/binary digests and effective IAM.
Use a securely supplied short-lived identity token for the exact service audience; keep it out
of shell tracing, receipts, screenshots and logs. Make the following observations:

| Observation | Required result |
| --- | --- |
| Anonymous invocation | IAM denies before the handler; record actual status (platform may use 401/403) |
| Authorized `GET <actual-URL>/ping` | 200, schema `cantos-forge.probe/v1`, status `live` |
| Authorized repeated `GET <actual-URL>/health` | 200 `ready`, worker generation `unavailable`, deployed revision; no path/body/environment echo |
| Authorized `POST <actual-URL>/health` | 405 and `Allow: GET` |
| Authorized `GET <actual-URL>/unknown` | 404 |
| Authorized `POST <actual-URL>/producer` | 404; the producer trigger is absent in deployment discovery |
| Logs/resources | Sanitized Cloud Logging, actual revision, no private payload/stderr/token, cold-start/deadline/memory/error observations |

Worker failure/timeout fault injection stays offline. No real provider, Drive, Server API,
Studio operation, media generation or production deployment is involved in this probe. Source,
mock, host-native, Linux, emulator and cloud results must be separate evidence rows. Closing
#486 requires its actual operator-approved cloud evidence, not this source package alone.

## Scoped recovery and cleanup

Retain the previous reviewed source/package/receipt before updating a probe. If a deploy fails
or its result is unknown, first read the exact `forgeSmokeProbe` resource in the selected region,
codebase label, revision/build and logs; reconcile the candidate receipt before retrying. Do not
blindly replay or infer rollback from a failed CLI exit. A new source build gets a new run directory.

A separately approved rollback redeploys the retained known-good **probe** package using the same
explicit selector/project/config after inspection and Linux qualification. It does not delete
other codebases or alter IAM/billing/Hosting/Distribution. First deployment has no earlier probe
revision to restore. Deletion of the probe, Artifact Registry cleanup, IAM changes and local
bundle cleanup need their own operator authority and retention review; there is no automatic
cleanup or fallback. Cloud recovery is not accepted until actually observed.
