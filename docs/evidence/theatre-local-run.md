# Theatre Web local full-stack run — issue 183

Related issue: [#183](https://github.com/loveoverflowcom/cantos/issues/183).
Implementation baseline: `develop@6f3e582bf93626014666da8e10891e102931df44`.
This record separates the compatibility proposal, launch behavior and attributable local
evidence. It does not accept real playback, live issuer or production delivery.

## Review boundary and source facts

The outcome is a truthful local Theatre Web launch: full-stack `run` starts and checks the real
API plus Trunk; frontend-only `app` names its limited readiness and warns about its live API
dependency. The source issue has no comments. Its historical reproduction inspected
`6818642a7c62cb35c9da04ed6d7ac2daeb1008b1`; that source is distinct from this implementation's
latest-develop baseline above.

Before this change, `plan_run(CantosTheatre)` and `plan_app(CantosTheatre)` shared one
Trunk-only plan with no backend requirements. Trunk forwarded `/api/` from 8082 to 8080, yet the
supervisor printed `All processes are ready` after only frontend readiness. The existing
anonymous discovery client already called JSON `/api/v1/theatre/works?limit=20`; the API
existed, so the failure was orchestration and readiness. The previous xtask claim that no
listener API existed was stale.

Owning contracts remain [xtask](../../xtask/README.md#run-and-app),
[Discovery v1](../../packages/contracts/theatre-discovery-v1.md),
[API contexts](../architecture/api-contexts.md) and the
[Web mode/runbook](../../apps/cantos-theatre/web/README.md#build-run-and-checks).
No domain, schema, migration, listener DTO, session policy, provider, media adapter or
infrastructure-provisioning contract changes in this slice.

## Compatibility decision — PROPOSED, pending review

Decision owner: Cantos maintainers; no acceptance is recorded here. The issue explicitly
proposes A while leaving xtask backward compatibility open. This implementation therefore
ships for review in a Draft PR, with `Part of #183`; neither this ledger nor passing local
checks promotes the decision to accepted.

| Option from #183 | Benefit | Cost / boundary | Review proposal |
| --- | --- | --- | --- |
| A: `run` starts API+Trunk; `app` remains frontend-only | Same meaning as Studio; checks the real dependency before claiming full-stack readiness | Changes former Theatre `run` behavior; callers need a prepared DB or must migrate to `app` | Implement for review with explicit command migration and dependency warning |
| B: keep frontend-only `run`, add `--with-api` | Preserves that historical call | Different default semantics from Studio, another flag/profile and continued ambiguity | Not implemented; revisit if A's compatibility cost is rejected |
| C: service-dependent profile/harness | Can compose already-authorized infrastructure | Adds runtime ownership/cost that this bug does not require | Not implemented; infrastructure stays explicit under `services` |

Migration: replace old frontend-only `cargo xtask run cantos-theatre --web` with
`cargo xtask app cantos-theatre --web`. For offline work, open an explicit preview mode.
For full-stack work, keep `run` and supply a migrated local app-role `DATABASE_URL`, or select
`--test-cluster` after explicit disposable verification retained its fixture. A process already
using 8080 is never taken over: use `app` with an appropriate existing API or release a port
belonging to your own process. There is no automatic mock fallback.

Web profiles accept an omitted origin or their exact actual origin: Studio
`http://127.0.0.1:8081`, Theatre `http://127.0.0.1:8082`. A mismatched explicit `--web-origin`,
including `localhost`, is rejected before launch to preserve host/cookie/callback identity.
Standalone `run cantos-api --web-origin URL` retains its configurable loopback origin.
Frontend-only `app` accepts neither API flag. The single-origin development host does not
establish simultaneous Studio and Theatre authentication.

Revisit A if reviewers reject this command migration, if a multi-origin host is accepted or
if an actual consumer needs different ports. Such a change needs its own compatibility and
readiness evidence; do not silently weaken this profile's check.

## Implemented launch boundary

| Surface | Planned processes / dependency | Readiness claim |
| --- | --- | --- |
| `run cantos-theatre --web` | Build/start API on 8080 with Theatre origin 8082, then Trunk on 8082 | Direct anonymous HTTP 200 JSON catalog envelope, then Theatre HTML and the proxied JSON envelope |
| `app cantos-theatre --web` | Trunk on 8082 only; advisory direct public-catalog check before launch | Always frontend-only HTML readiness; prominent `WARNING` for absent/invalid API dependency |
| `run cantos-studio --web` | Existing API on 8080 + Trunk on 8081 | Existing Studio profile; matching explicit origin allowed |
| `run cantos-api` | Existing standalone API on 8080 | Existing standalone API readiness; loopback origin configurable |

Theatre public-catalog readiness uses `/api/v1/theatre/works?limit=20`, with anonymous JSON
semantics. This is a catalog-envelope smoke (object items and nullable/string cursor), not full
WorkSummary schema/domain validation; those remain with the API/shared contract and Web decoder. An empty `items` list with a valid page is legitimate success. Connection refusal,
HTTP errors, invalid media type or malformed page do not become empty success. A 404 does not
diagnose frontend connectivity. `/api/v1/theatre/session` is independently optional: an
unconfigured or unauthenticated 404/401 cannot block an otherwise valid anonymous Home.

API readiness has a 60-second bounded wait; Trunk has up to 15 minutes for its first WASM build.
Probes back off within those budgets. Full-stack startup stops owned groups when a process
exits or readiness fails. Port checks refuse collisions without signalling unrelated processes;
Ctrl-C/SIGTERM/SIGHUP use the existing owned process-group cleanup. Logs identify the service,
actual origin, failed route and next action without printing `DATABASE_URL` or passwords.
Frontend-only completion never says that an integrated API is ready, even when its advisory
probe succeeded. A readiness observation cannot guarantee later availability.

The retained test-cluster path selects `CANTOS_TEST_CONTEXT=theatre` and a separate
`theatre_app_url` field naming the existing disposable audio fixture DB. Bounded discovery
read grants are prepared only by explicit `verify postgres --keep`; launch neither provisions
nor grants anything. The fixture contains no invented approved publication and can return a
valid empty catalog. An older record missing the Theatre field is refused with an instruction
to perform fresh explicit disposable verification. Inherited `DATABASE_URL` never selects the
retained fixture's database. Studio's editorial/catalog/audio contexts remain separate.

`run`/`app` do not start PostgreSQL, Podman, S3 or Kanidm, apply migrations, create credentials,
write production storage or call an AI provider. Keep credentials, retained cluster records
and raw local logs outside Git. Share redacted diagnostics, not environment/configuration dumps.

## Commands and mode handoff

The one-command full-stack recipe assumes `DATABASE_URL` is already exported privately and
points to an operator-prepared migrated local app-role DB:

```sh
cargo xtask run cantos-theatre --web
```

Alternatively, after successful explicit `cargo xtask verify postgres --keep`:

```sh
cargo xtask run cantos-theatre --web --test-cluster
```

To reuse an appropriate API already listening on 8080, run only
`cargo xtask app cantos-theatre --web`. The separate API command is
`cargo xtask run cantos-api --web-origin http://127.0.0.1:8082` with its own explicit DB input.

| Mode at `http://127.0.0.1:8082/` | Network / evidence boundary |
| --- | --- |
| Default live | Actual anonymous catalog; independent session/account APIs; unavailable remains unavailable |
| `?mode=mock` | Explicit session-only samples and fake player; no audio or persistence |
| `?mode=membership-preview` | Explicit no-network interest/receipt/plan fixtures; no durable submit, entitlement or payment |
| `?mode=auth-preview` | Explicit no-network synthetic account/auth diagnostics; no real provider login/registration |

Unknown modes stay live. No network failure selects a fixture. Full-stack discovery success is
independent of the existing fail-closed public media delivery and production player gaps.

Manual anonymous smoke commands, with no auth cookie/token:

```sh
curl -i -H 'Accept: application/json' 'http://127.0.0.1:8080/api/v1/theatre/works?limit=20'
curl -i -H 'Accept: application/json' 'http://127.0.0.1:8082/api/v1/theatre/works?limit=20'
```

Expected success is HTTP 200 JSON `WorkPage`, possibly empty. Preserve actual error status and
the existing `code`/`retryable` contract. To observe nonempty live data, a separately approved
published Work must exist; this repair does not create one or require live audio to start a
development server. On failure, inspect API process output, database reachability, migrations
and app-role read grants; inspect issuer configuration only when identity is explicitly enabled.

## Claim, oracle and evidence ledger

Local evidence is retained under ignored `artifacts/agent-runs/issue183/`; it contains no
production database or provider access. Controlled HTTP peers are doubles. The real runtime
uses only the newly retained PostgreSQL 17.4 synthetic fixture, actual API and Trunk 0.21.14.
The final PR records the final-diff preflight summary; this document records checkpoint and
runtime observations without predicting that later gate's result.

| Claim / failure mode | Oracle actually exercised | Evidence / result |
| --- | --- | --- |
| The original Theatre `run` omitted its API and rejected `--test-cluster` | Two named pre-fix plan regressions | `example-tested`: expected red, 2 failed; `dev-red.log` |
| Full-stack plan owns API+Trunk/correct origin; app remains frontend-only | `cargo test --locked -p xtask` at checkpoint | PASS, 179 tests; `checkpoint-xtask.log`. Final-diff tests belong to HANDOFF |
| Missing/error/non-JSON catalog cannot be ready; valid empty catalog can | Controlled HTTP probe regressions plus actual direct/proxy requests | Checkpoint PASS; real runtime PASS for the empty page. Final envelope regressions are rerun by HANDOFF |
| Session success is not a catalog gate | Real anonymous catalog success with an absent/unauthenticated session route | Runtime PASS; no live issuer configured |
| Offline app warns, serves explicit preview HTML and never claims integrated readiness | Actual `app cantos-theatre --web` without port 8080 | Runtime PASS; HTML-only observation, no browser JavaScript/interaction acceptance |
| Startup/port collision/Ctrl-C preserve ownership | Production supervisor regressions; real foreign-listener refusal and owned shutdown | `fault-injected` / runtime observations; see `runtime-results.json` |
| Retained fixture is explicit and never an inherited DB URL | `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p test_postgres_runner.py` | PASS, 18 tests; rejects legacy/unretained/wrong database context |
| Real catalog survives PostgreSQL restart | `cargo xtask verify postgres --keep` with the recorded local tools | PASS; `checkpoint-postgres.log`, including anonymous empty WorkPage before/after restart |
| Actual API and Trunk serve the same catalog | Real `run cantos-theatre --web --test-cluster`, anonymous GET at 8080 and 8082 | Runtime PASS, HTTP 200 `application/json`, `{"items":[],"next_cursor":null}`; no publication invented |
| Existing Studio and standalone API start/stop | Actual retained-cluster `run cantos-studio --web` and `run cantos-api --test-cluster --web-origin http://127.0.0.1:8082` | Runtime PASS; authentication acceptance remains separate |
| Final repository/format/lint/test obligations | HANDOFF `cargo xtask preflight --changed-only` | Exact final result is recorded in the associated PR; the full selected plan includes PostgreSQL |

The command environment prefixes existing tools only:

```sh
PATH=/tmp/cantos-audio-tools:/tmp/cantos-gradle-8.14.3/gradle-8.14.3/bin:$PATH \
GRADLE_USER_HOME=/tmp/cantos-gradle-user-home RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp \
cargo xtask preflight --changed-only
```

No tool was installed. Source/tool fingerprints and full logs remain in the local run record;
commit, push and PR packaging reuse identical checked inputs. An initial handoff attempt failed
repository admission after a new xtask-only path dependency changed the global lock fingerprint
and invalidated six UI qualifications. The dependency was removed; the smoke stays scoped to
JSON reachability without changing a renderer or its qualification. The failed log is retained
as `handoff-preflight.log`; the final full plan is `handoff-preflight-final.log`. The prior ignored evidence and
Web build files were copied to `preserved-before-checks/` before verification; existing
PostgreSQL cluster directories were not altered or deleted.

Initial sandbox failures are retained separately: `ps` and Git `FETCH_HEAD` access were
blocked until their authorized read/fetch operations ran with suitable permissions; focused
socket tests failed with `EPERM` before their elevated checkpoint passed. The initial runtime
harness also failed a post-shutdown raw Python bind assertion: independent `lsof` found no
listener, TCP connections were refused and a reuse-enabled bind succeeded. Correcting only
that ignored harness's TIME_WAIT oracle produced the later runtime observations. Its original
script/logs and diagnosis remain in `runtime-initial-oracle-failure/`; no production fix was
needed for that test-oracle defect.

### Independent acceptance and remaining scope

| Layer / gate | Result | Required follow-up / owner |
| --- | --- | --- |
| L1 architecture/compatibility | Documented proposal; acceptance pending | Maintainer review of option A and migration before declaring #183 accepted |
| L2–L3 launch plans/types and focused regressions | Checkpoint PASS; final-diff evidence in PR | 179-test checkpoint; HANDOFF reruns final sources |
| L4 controlled HTTP/process lifecycle | Checkpoint PASS | Actual probe/supervisor modules with scripted local HTTP peers and owned child processes; no provider double is production evidence |
| L5 disposable PostgreSQL + API + Trunk | PASS for the stated empty-catalog slice | Actual direct/proxy JSON, optional session and owned cleanup; nonempty approved publication remains outside this fixture |
| HANDOFF preflight | Exact final summary in associated PR | Final diff; preserve every printed gate result and any unavailable checks |
| Real Kanidm/issuer, storage and deployed delivery | NOT_RUN; outside this repair | Authorized operators and capability owners under #179/#170; no credentials provisioned |
| Browser interaction/accessibility and real playback | NOT_RUN; outside this repair | #13/#179 owners; startup and HTTP probes are insufficient |
| Android/iOS/native runtime | NOT_RUN; no native change | Native owners retain device/provider/playback acceptance |
| GitHub CI | NOT_RUN / not checked by request | Local evidence is handed off without polling CI |

Do not convert missing infrastructure into a passing claim or report a controlled peer as
live API acceptance. Any actual failed check remains FAIL with its diagnosis; a missing
prerequisite is BLOCKED/NOT_RUN in the handoff and retains its precise next action. Reuse
evidence only while its relevant fingerprint remains valid under the
[verification cadence](../../.agents/skills/cantos-engineering/references/verification-cadence.md).

The next recommended developer handoff is #183, then #179's capability acceptance/runbook
and #180's consumer tracking, followed by the independently gated #13/#170 listening/delivery
work. Storage/publication/listener owners and prerequisites remain unchanged. GitHub owns
execution and review state; this ledger closes none of those issues.
