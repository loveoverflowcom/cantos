# Forge worker boundary and acceptance

Implementation boundary: local Rust worker and Node process adapter for
[#203](https://github.com/loveoverflowcom/cantos/issues/203), option A in
[ADR 0022](../decisions/0022-cantos-forge-producer-cli.md). This is not a deployed Function,
a Studio production workspace or a live Drive/Gemini integration.

## Source and compatibility

[`services/cantos-forge-functions`](../../services/cantos-forge-functions/README.md) owns one
`cantos-forge` Rust library, the compatible offline `cantos-forge` CLI and the finite
`cantos-forge-worker` binary. Its `firebase/` directory owns runtime transport/process source.
`infra/firebase/functions` is the future deployment-policy destination, owned separately
under #212. No Functions deploy/emulator command is implemented or implied by these paths.

`cargo xtask build cantos-forge` builds the package's binaries. `cargo xtask verify forge`
checks the producer package and the dependency-free Node wrapper tests. Neither command
reads credentials, invokes a provider, starts infrastructure, uploads drafts or deploys.
Build artifacts are native to the current host. A macOS executable is not a Linux Functions
bundle and must never be renamed or copied into deployment as though it were compatible.

Existing offline workspace, artifact, evaluation and handoff-bundle schemas are unchanged.
There is no data migration and no second implementation in `apps/`. Existing workspace
directories are operator data outside Git, independent of the source checkout's location.

## Finite process boundary

The worker accepts one bounded versioned JSON request on stdin and emits one bounded JSON
result, then exits. Its only operations are capability discovery and deterministic text
inspection through the existing Rust core. It has no generation, Drive, filesystem workspace,
preview, handoff or publication operation. Unknown fields/versions/actions are rejected;
request-supplied executable paths, argv, environment and filesystem roots are not accepted.
See the [source protocol](../../services/cantos-forge-functions/src/interfaces/worker.rs) for
exact fields and limits. This is an internal local worker protocol, not a new Cantos API DTO.

The Node adapter chooses a fixed package-relative prebuilt executable, uses argument-vector
process invocation without a shell, supplies a minimal environment, bounds input/output and
terminates timed-out or disconnected children. Completion waits for process closure. It does
not compile, install or download during a request, return `202` for detached work, or forward
worker diagnostics/private text to HTTP clients. Test injection is explicitly named and does
not replace the production HTTP entrypoint's authority checks.

The HTTP entrypoint returns an unavailable outcome with private/no-store response semantics
until a trusted invocation/access grant adapter exists. CORS, an `Authorization` string,
request-supplied owner flags, the Studio cookie and Catalog delegate grants are not Forge
authority. Enabling an endpoint requires a reviewed server-held actor/audience/action/expiry/
draft-and-input binding plus current revocation checks for every operation, including reads.
No browser flag or environment switch bypasses this gate.

## Ping/health probe slice (#486)

`firebase/probe.mjs` is the source for a narrow deployment smoke probe, exported by
`index.mjs` as `forgeProbeHttp` with the fixed packaged worker. It is not a registered Firebase
trigger: there is no `firebase-functions` dependency, Linux worker bundle, codebase config or
deploy command, so nothing is deployed and cloud acceptance is `NOT_RUN`.

| Route | Result |
| --- | --- |
| `GET /ping` | `200` `status: live`. Proves only that the function answered; it never starts the worker |
| `GET /health` | `200` `status: ready` only when the packaged worker ran a fixed `capabilities` request and the validated reply reports `generation: unavailable`. Spawn failure, deadline, malformed reply or a rejection is `503` `status: degraded` with a closed `BoundaryError` code, never a healthy answer |
| other method / path | `405` (with `Allow: GET`) / `404` |

Bodies are `cantos-forge.probe/v1` with `status`, an optional closed `error` code and the
`K_REVISION` revision when it is a plain identifier. They carry no paths, environment, stderr or
request echo, and responses are `private, no-store`. The probe reads no body, grant, secret,
provider or Drive state and does not authorize `forgeWorkerHttp`, which keeps its fail-closed
grant gate. The probe checks run in the existing `cargo xtask verify forge` suite, including one
real local Node→Rust `health` run; that proves the wrapper contract on the host OS, not the
Linux ABI, Node 22 or a deployed Function.

Remaining acceptance before live probe evidence, in order: the Gen2 trigger and dependency
with a private invoker default and its own codebase in `infra/firebase/functions` (#212); a
reproducible Linux worker build and package with ABI, mode and digest checks; an
operator-authorized scoped deploy to `cantos-a348b` after reading project, region, IAM, billing
and existing functions; real `ping`/`health`/wrong-method transcripts, sanitized logs and a
scoped rollback. Missing-binary and timeout faults stay offline fault injection.

## Local attempt durability

The local filesystem `AttemptStore` is a concrete adapter for the existing reserve-before-call
use case. Immutable events are published without overwrite only after their contents are
synced; required directory sync failures return uncertainty. Readers validate schema,
operation key, contiguous index, constructors and full domain replay under fixed size/count
bounds. Temp files left by interruption never become committed events automatically. A
process restart preserves open attempts; it never infers that a potentially billed call was
cancelled. An explicit recovery/acknowledgement remains necessary before regeneration.

This adapter assumes an operator-controlled local filesystem with supported atomic hard links
and directory durability. It is not suitable for Drive, object storage, an untrusted shared
directory or automatic local/cloud writer coordination. No production provider uses it yet.
Do not store durable attempt history in a Function's ephemeral `/tmp`.

Store/event/temporary bounds are deliberate fail-closed limits. Ordinary capacity refusal
preserves existing histories and removes only this invocation's newly created empty directory.
A process crash during concurrent directory creation near the operation limit can still leave
an over-capacity root unavailable. An operator must reconcile confirmed-unused empty directories
or retained crash temporaries after excluding active writers; the adapter never deletes another
invocation's files automatically. No cleanup CLI or arbitrary power-loss recovery is claimed.

## Supported-runtime feasibility

Official [Firebase runtime guidance](https://firebase.google.com/docs/functions/manage-functions)
supports Node 22; the [Cloud Run functions runtime list](https://docs.cloud.google.com/run/docs/runtimes/function-runtimes)
does not provide a native Rust runtime. Node 22 is the candidate wrapper runtime; native Rust
producer rules remain in the Rust library. Runtime support does not prove an arbitrary child
binary/encoder bundle. Local Node tests may run on supported newer Node versions and report
the actual version; that is not deployed Node 22 acceptance.

Before packaging a deployable Function, independently establish:

- Linux OS/CPU/ABI, executable mode, dynamic/static linking, TLS/CA and native dependencies.
- Reproducibly packaged encoder/version/profile and independently inspected output bytes.
- Cold start, bounded memory including PCM/base64/encoder overhead, request/response limits,
  deadline/resource policy, child cleanup and no post-response work.
- Exact project ID, region and Functions codebase, with no Hosting/App Distribution coupling.
- Effective private ingress, grant/revocation enforcement and secret binding redaction.

The [Functions lifecycle guidance](https://firebase.google.com/docs/functions/tips) and
[quotas](https://firebase.google.com/docs/functions/quotas) remain external runtime constraints.
An emulator or local subprocess cannot substitute for these deployed observations. Cloud Run
is a possible alternative requiring an explicit topology decision, not an automatic fallback.

## Next independent implementation boundaries

| Boundary | Required invariant / acceptance |
| --- | --- |
| Invocation authority | Every operation checks trusted actor, audience, action, expiry, resource/input and revocation. Existing [Catalog delegation](../../services/cantos-server/contracts/studio/delegation/v1/README.md) has only its documented Work scopes; it does not authorize private drafts or spending |
| Paid dispatch coordination | Demonstrate durable reservation/fencing across local, deployed and overlapping writers before a provider adapter is reachable. Drive files, process mutexes and `maxInstances=1` do not prove this |
| App-data store | Actual owner/application namespace and `drive.appdata`; paginated bounded reads, immutable output readback digest/size, interrupted-write reconciliation and rebuildable indexes. No ordinary-folder default or cross-space move |
| Gemini and encoder | Officially available endpoint/model/config; exact unary/streaming media handling; bounded one-call adapter with refusal/quota/timeout/invalid-audio fixtures, safe post-generation storage failure and independently inspected Cantos Opus bytes |
| Studio workspace | Separate Forge client, structured drafts/revisions/runs/takes, bounded edits, explicit generation/regeneration/resume, exact authenticated previews, selected-artifact handoff and reload/work/session race evidence; browser light/dark, keyboard, narrow and Vietnamese qualification |
| API handoff | Explicit owner preparation, exact retained-byte producer PUT and receipt reconciliation using the [existing audio lifecycle](../../services/cantos-server/contracts/studio/audio-lifecycle/v1/README.md). No invented finalize route, automatic content writes or publication |
| Deploy tooling | Exact-project/codebase dry-run and explicit deploy contract, independent of Hosting/distribution, with uncertain-result reconciliation and separately authorized runtime acceptance |

The [Gemini speech guide](https://ai.google.dev/gemini-api/docs/speech-generation) is the model
availability and media-format authority. This slice chooses no production model and makes no
paid call. A later adapter must recheck it rather than pin a suggested model name or assume
all responses are headerless PCM.

Drive app-data has no shared links or Trash. Export/backup/restore must copy and verify exact
bytes/lineage; tombstones preserve references and immutable outputs. Purge requires separate
authorization plus reference/retention checks. Existing ordinary Drive files are never moved
or deleted by this implementation.

Live OAuth consent/refresh-token lifecycle, Secret Manager bindings, billing/provider calls,
deployment and VPS handoff are outside local/mock authorization. None has been attempted by
build/tests. Missing credentials alone do not justify enabling calls before grant and fencing
designs are accepted. Publication remains an independent authoritative API action.
