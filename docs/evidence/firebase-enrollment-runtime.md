# Firebase enrollment runtime: independent local evidence

Work item: [#206](https://github.com/loveoverflowcom/cantos/issues/206), part of #174/#191.
Review boundary: default-disabled Firebase mailbox proof → exact new Kanidm person → one-shot
setup continuation in the existing registration owner. It issues no authenticated session;
credentials/OIDC and Studio/Admin admission retain their separate owners.

## Authority and provenance

The product owner explicitly accepted direction A, a dedicated Firebase UID per enrollment and
one-shot RAM-only setup delivery to the confirming browser, with lost-response reconciliation.
[ADR0036](../decisions/0036-firebase-enrollment-proof.md) narrowly amends the SMTP setup-mail
rule for selected Firebase mode. Approval covers code and local tests; activation, effective
provider privileges, numeric policy/retention and safe origins remain separate gates.

New branch `feat/206-firebase-runtime-bridge-20261016` starts from freshly fetched develop
`25b1c701b686018503d0f341d3966d3d35eaaf4b`, which already merged PR208. The main checkout was
clean and fast-forwarded; old worktrees, branches/history and stash were preserved. No cleanup,
issue closure, GitHub CI inspection or software installation is part of this run. Implementation
commit: `16ec005d7f6befe4f3a4db50b965f64792b0b337`; the subsequent evidence-only commit does not
change its code, fixtures, lockfile or tooling. Later user authorization permits merge only when
eligible; required aggregate gates and live acceptance are incomplete, so this delivery stays Draft.

Host: Mac mini, Rust1.98.1, PostgreSQL17.4, Python3.13.5, Node22.15.0, installed
Firebase Tools14.2.2/Auth emulator. Raw logs and final input identities are in ignored
`artifacts/agent-runs/issue206-runtime/`; safe project metadata is in
`artifacts/agent-runs/issue206/firebase-readonly-inventory-safe.json`. No real recipient,
provider credential, action code or setup link is included in tracked evidence.

## Invariant and ownership ledger

| Claim | Failure mode / oracle | Owner / evidence boundary |
| --- | --- | --- |
| Only actual action possession authorizes provisioning | A initiates / V verifies / A polls; no public status/setup endpoint, same-code inspection/consume, exact UID/email and one winner | Pure types, application/REST/HTTP fixtures; no historical boolean/token admission |
| Every mutating effect is durably fenced | Held/expired exchange, duplicate confirm/cancel, dropped consume/create/setup/response; exact phases and no repeated effect | Migration0028 + application; real SQL separate from fake provider |
| Resend never revives an old enrollment | Same UID/absolute deadline under cooldown; cancel/expiry terminal; fresh successor UID and EMAIL_EXISTS refusal | Pure/app/SQL tests; production code invalidation not inferred from emulator |
| Setup belongs only to confirming request | Pinned Kanidm origin/path/token, durable HandedOff before response, no secret at rest, replay unavailable | Types/application/HTTP/Web; lost response requires operator reconciliation |
| Proof/setup are not authentication | No session/registry writes, existing OIDC required, Theatre/Studio denial and unconfigured Admin unavailable | Mounted router fixtures; live dedicated Admin acceptance remains unrun |
| Missing/partial/mixed provider config fails closed | Exact project/signing declaration, no endpoint/tenant/emulator/SMTP fallback; complete synthetic config constructs without dialing | Host composition tests, no cloud activation |

The module adds no service/crate or provider SDK. Domain decisions stay pure; the existing
Issuer is reused with positive new-person provenance. New ports have PostgreSQL/Firebase
adapters and deterministic doubles. PostgreSQL reads use indexed point lookups and bounded
budget counts under a namespace lock; performance is unmeasured, and DBSP is not adopted.

## Executed local evidence

Commands below run from the new worktree with `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp`
unless stated. Captured loopback/process tests use ordinary approved local execution after
sandbox socket/owned-child-cleanup failures; they never call a live provider.

| Layer / claim | Actual result | Evidence level / exact command and limits |
| --- | --- | --- |
| L1 source/architecture | No actionable findings in the declared default-disabled scope | Independent reviewer `/root/independent_review`, implementation `16ec005d7f6befe4f3a4db50b965f64792b0b337`; read-only review, no live acceptance or merge recommendation |
| L2/L3 action factories/binding | PASS6 | `cargo test --offline --locked -p cantos-api --lib firebase_action::tests`; exact purpose/email/UID/client, redaction and fixed-origin tests |
| L2 enrollment phases | PASS1 focused | `cargo test --offline --locked -p cantos-api --lib permanent_operation_fences_never_reopen_or_reuse_a_uid`; full module below also covers it |
| L3/L4 registration module | PASS107,26 ignored | `cargo test --offline --locked -p cantos-api --lib theatre_registration -- --test-threads=1`; example-tested/fault-injected/mock-integration-tested production service/REST/handlers, in-memory repository and fake mailbox/Kanidm;15 newSQL +10 priorSQL +1 optional emulator ignored here |
| L5 real PostgreSQL | PASS15 Firebase +10 SMTP; gate PASS | `cargo xtask verify postgres --suite theatre-firebase`; integration-tested constraints, locks/claims/expiry/quotas, receipt immutability, repository reconstruction, migration compatibility and one-shot fences; new cluster stopped. Full media/HTTP/process-restart campaign NOT_RUN |
| L5 installed Auth emulator | PASS1 | `CANTOS_TEST_FIREBASE_AUTH_MODULE=/usr/local/lib/node_modules/firebase-tools/lib/emulator/auth/server.js cargo test --offline --locked -p cantos-api --lib platform::theatre_registration::firebase_action::emulator_tests::installed_auth_emulator_exercises_exact_action_consumption_and_resend_limits -- --ignored --exact --nocapture`; real emulator action calls, no live mail/credentials |
| Tooling | PASS398,2 ignored | `cargo test --offline --locked -p xtask`; unit/fixture process tests; ignored Podman/live checks remain unrun |
| Disposable runner | PASS25 | `python3 -m unittest discover -s scripts -p test_postgres_runner.py`; mocked lifecycle, failure/interrupt, environment and zero-selection assertions |
| API library | PASS621,94 ignored | `cargo test --offline --locked -p cantos-api --lib`; ignored real-infrastructure/media checks remain separate; `api-lib-tests.log` |
| API doctests | PASS27 | `cargo test --offline --locked -p cantos-api --doc`; includes compile-fail sealed action witnesses; `api-doctests.log` |
| Theatre host / shared contracts | PASS221 Web +3 Web doctests;103 contract tests | `cargo test --offline --locked -p cantos-theatre -p cantos-contracts`; `web-contract-tests-frozen.log`; subsequent test-module placement only, reviewed unchanged behavior |
| API lint | PASS | `cargo clippy --offline --locked -p cantos-api --all-targets -- -D warnings`; `api-clippy.log` |
| Theatre/UI WASM lint | PASS | `cargo clippy --offline --locked -p cantos-ui -p cantos-theatre --all-targets --target wasm32-unknown-unknown --features cantos-ui/preview -- -D warnings`; `web-clippy-final.log` |
| Theatre WASM build | PASS(dev) | `cargo xtask build cantos-theatre --web`; `theatre-wasm-build-frozen.log`; no release or deployed-origin evidence |
| Python tooling suite | PASS502,5 optional skips | `python3 -m unittest discover -s scripts -p 'test_*.py'`; `python-tests.log`; fixture negative controls printing FAIL are not a real Quint/Kani run |
| Rust format / whitespace | PASS | `cargo fmt --all -- --check`; `git diff --check` and `git diff --cached --check`; separate focused checks because aggregate did not reach them |
| Native Chrome local interaction | Observed scenarios passed | Auth preview fixtures and synthetic action query, described below; no real inbox/provider/issuer/API session |
| Original changed-only preflight | UNSUPPORTED; other selected gates NOT_RUN | `cargo xtask preflight --changed-only`; missing ffmpeg/ffprobe and Gradle; `preflight-first.log` |
| Available-gate preflight | FAIL repository; all later gates NOT_RUN | `cargo xtask preflight --changed-only --skip audio-inspection --skip postgres --skip cmp-ui-preview --skip theatre-cmp-demo`; inherited20 qualification/adoption failures, described below; final handoff log `preflight-final.log` |

Auth emulator evidence cannot establish production API-key/project association, OOB expiry,
IAM, signature/revocation, old-code invalidation or inbox delivery. Its fake custom token is
confined to the isolated child; production accepts neither an emulator endpoint nor ID tokens.
The custom RS256 signing test uses a public synthetic key, not a real provider credential.

Retained actual failures: first scoped registration run had41 sandbox socket failures; ordinary
local rerun exposed an outdated ordered-trace oracle and an Admin unconfigured503 expectation.
After correcting those oracles,107 tests passed. The mail-expiry review finding was fixed with
an independent `MailStarted` DB fence after exchange and held-exchange initial/resend regression.
The first xtask suite had19 sandbox socket/process failures; local rerun passed398. Intermediate
compile integration errors were repaired; no failing result is relabeled as passing.

Native Chrome QA used an owned local `cargo xtask app cantos-theatre --web` process and a new
tab; both were closed after observations. Vietnamese copy, keyboard submit and visible focus,
desktop and responsive390×844/320×844, and browser zoom200% were observed. The setup button
disappeared immediately after one click. Cancel → login → Create → editable empty form →
synthetic email → submit → accepted fixture → Cancel remained usable. Back returned an empty
form; Forward/Reload did not restore a code/setup link. A standard synthetic Firebase action
query was replaced by `/#register?from=home` before UI interaction; explicit confirm with the
local API unavailable showed unknown outcome without automatic replay, and reload discarded
the code. Screenshots/AX observations are in this task's computer-use transcript; no saved PNG
or real live-session claim is made. Screen-reader, motion and deployed ingress observations
remain NOT_RUN.

This QA found that the first implementation misclassified ordinary `?mode=auth-preview` as a
Firebase action. A deterministic regression failed before the correction and passed afterward
(`browser-query-regression-before.log` / `browser-query-regression-after.log`), then the same
browser navigation preserved preview mode. WASM Clippy also first rejected test items before
`open_setup`; moving only the test module to EOF resolved it. Both earlier failures are retained.

Independent source review covered Firebase action/bootstrap, enrollment/application/SQL/config,
HTTP, contracts/Web, operator docs and scoped tooling. Its classification was
`complete-for-declared-scope`, `no-actionable-findings`. Findings corrected before freeze were
the post-exchange mail freshness fence, durable explicit cancellation, preview query routing
and phase consistency. Reviewer independently read source, official contracts and local logs;
browser observations above are attributed to the implementing agent. Review does not accept IAM,
real email, Kanidm, OIDC, native clients or activation.

## Aggregate gates and merge eligibility

The unmodified conservative plan selects every ordinary gate because `Cargo.lock` is a shared
input (only the Theatre dependency edge to already-pinned `url2.5.8` changed). Original preflight
reports `UNSUPPORTED` for audio inspection/full PostgreSQL (ffmpeg/ffprobe absent), and CMP
preview/Theatre CMP demo (Gradle absent); every other selected gate is `NOT_RUN`. A reasoned run
skipping exactly those unavailable gates reaches repository validation, which `FAIL`s20 times:
six stale Leptos component qualifications and14 downstream adoption checks. The captured check
output is byte-identical to the check run against clean develop `25b1c70`; it is an inherited
failure, not a passing gate. Qualification hashes, evidence files and admission rules were not
changed to make it green. Services static validation passes before that failure. Focused layer
passes above do not relabel the aggregate's unreachable steps.

Remaining checklist (unassigned — assignment required before acceptance):

- [ ] UI owner supplies genuine current component qualification and consumer evidence; rerun
  `cargo xtask verify repository` with zero failures. Existing20 failures block clean handoff.
- [ ] Operator explicitly provisions trusted ffmpeg/ffprobe and compatible Gradle if the complete
  selected campaign is required on this Mac; no installation was authorized or attempted.
- [ ] Run `cargo xtask preflight --changed-only` with all required prerequisites and no skipped
  gates. Rust workspace, DBSP, reference/mutation, Studio/shared previews, full media/PostgreSQL
  and CMP aggregates are currently NOT_RUN; their acceptance criterion is the named gate PASS.
  The affected API/Web/contracts/scoped SQL passes above are the delivered independent layers.
- [ ] Any requested registered Quint campaign needs locked Node24.19/npm11.9/Quint0.33 inputs;
  installed Node22.15/npm10.9 does not supply them. `cargo xtask verify behavior` is NOT_RUN,
  and no behavior model changed in this implementation.
- [ ] Operator/identity owners complete the live prerequisites and journey below; effective
  authorities, origins, policy and existing-person protection must be verified before activation.

Residual aggregate risk includes unexecuted consumers and platform suites. No mobile build or
release was attempted; no remote CI was inspected or substituted. Keep Draft/default-off and
do not merge based on this ledger while required gates remain unmet.

## Read-only project facts and remaining live work

The initial Firebase CLI login could not refresh (HTTP400/401). After the user reauthenticated,
`firebase projects:list --json --non-interactive` succeeded: display name `cantos` uniquely maps
to ACTIVE project ID `cantos-a348b`. Read-only Hosting/apps inventory reports default site
`https://cantos-a348b.web.app` and one Android demo app, no Web app. This is metadata, not an
observation that a deployed Cantos app or action handler works.

The user approved a specific test recipient, but **no real email has been attempted**. No runtime
server API-key binding, existing service-account signing file, admin OAuth bearer or Kanidm
issuer/client/target authority is configured in the inspected environment. Developer CLI login
is not imported as runtime credentials. Do not create a key/service account/grant, change Auth
templates/domains/security, deploy or accept terms to bridge those missing inputs without the
exact separate authorization. The operator needs the selected
[configuration and acceptance runbook](../deployment/firebase-enrollment-proof.md).

Deferred acceptance: named action/API/Kanidm origins and safe initial action GET; effective
Firebase/Kanidm least privilege and existing-email protection; policy/retention/cleanup;
real email → action → one-shot setup → user-established credentials → genuine OIDC → API session
→ reload/logout and direct Studio/Admin denial. Native/browser assistive-technology and full
required aggregate gates remain distinct. Next owner: operator/identity/UI owners; unassigned
where no person is named, assignment required before activation.

Residual risk: retained Firebase email records may block a new post-expiry UID; unknown effects
and lost one-shot handoffs require authorized reconciliation. There is no automatic cleanup,
adoption, reset, secret recovery or retry. Local passes accept only their named layers and never
complete #206 or permit activation.

## Concurrent develop reconciliation

While this work ran, develop advanced to `738bceb2de3ec79308a3268caa3b73a98ffa969d`
(PR207 Units continuation, PR210 Forge attempt library/tooling and PR211 documentation
consolidation). The clean main develop checkout was fast-forwarded. The feature branch then
merged that exact base without rewriting its two existing commits. Conflicts were limited to
the work queue/planning and xtask command documentation; the new concise queue/archive lifecycle
was preserved, and active identity/ADR/read-map pointers were reconciled with this decision.

The Firebase registration/action/bootstrap tree, migration0028, handlers, Theatre Web,
registration/identity contracts and scoped PostgreSQL gate have no delta from the reviewed
implementation. Their bounded local evidence above remains attributable to identical inputs.
The original whole-API/xtask/Python counts above describe the pre-merge source, not fresh runs
of the new Units/Forge tree. Fresh combined-tree checks are:

| Check | Actual result | Retained output |
| --- | --- | --- |
| `cargo test --offline --locked -p cantos-api --lib` | PASS625,94 ignored | `api-lib-tests-reconciled.log` |
| `cargo test --offline --locked -p xtask` | PASS406,2 ignored | `xtask-tests-reconciled.log` |
| `cargo clippy --offline --locked -p cantos-api --all-targets -- -D warnings` | PASS | `api-clippy-reconciled.log` |
| `python3 -m unittest discover -s scripts -p 'test_*.py'` |526 tests, OK with5 optional skips | `python-tests-reconciled-local.log` |
| `cargo fmt --all -- --check` | PASS | `rust-format-reconciled.log` |
| Original `cargo xtask preflight --changed-only` | UNSUPPORTED; other gates NOT_RUN | `preflight-reconciled-original.log`; now also selects behavior, whose locked Quint tool is absent |
| Available-gate handoff preflight | FAIL repository; later gates NOT_RUN | `preflight-reconciled-final.log`; skips only behavior/audio-inspection/postgres/cmp-ui-preview/theatre-cmp-demo because their tools are absent |

The first post-merge Python attempt failed with44 errors and1 failure: new upstream Kani
process fixtures could not call `os.killpg` on their owned groups in the sandbox. An ordinary
approved local rerun passed526 tests with5 skips; no source change, tool installation or real
Kani proof was involved. Both results are retained. Clean develop `738bceb` independently
reports the same20 repository qualification/adoption failures; this remains a real FAIL.

Independent merge-delta review found no actionable interaction: the Units guard concerns read
continuations; Forge remains outside the API and uses a mock provider; its Kani package
allowlist and scoped Firebase CLI/gate coexist. Reviewer read the resolved documentation and
unchanged Firebase source; this review is not a full re-review or live acceptance of upstream
features. Final merge-commit identity is recorded in the PR. Keep Draft/default-off; no merge
into develop is justified by these partial aggregate results.

## Follow-up: minted Auth-create bearer (no more one-hour operator token)

Branch `claude/firebase-sa-token-009524` from develop `8786bdc`. Code and docs only: no call to
Firebase or Google Cloud, no account, role, key or grant created, no deployment, no VPS access.
Design and runbook: [service-account design](../deployment/firebase-enrollment-service-account.md).

| Claim | Status |
| --- | --- |
| **Implemented** | `firebase_bootstrap/access.rs` signs a JWT-bearer assertion (`identitytoolkit` scope) with the already-read service-account key, exchanges it at the fixed `https://oauth2.googleapis.com/token` and caches the bearer in RAM. Refresh is lazy at 5 minutes before expiry, single-flight under one async mutex, and a failed refresh drops the stale value. HTTP policy is shared with the Identity Toolkit client (no redirects, proxies or pooled connections; 2s/5s). `FIREBASE_ADMIN_ACCESS_TOKEN` is retired and refused at startup; the bearer has no public constructor from configuration. |
| **Fixture-tested** | `cargo test --locked -p cantos-api --lib theatre_registration::firebase` PASS69, 17 ignored (12 new `firebase_bootstrap::access` + 1 new config test). Covers exact assertion claims and signature, request form shape, grant parsing and margin boundaries (`expires_in` ≤300 or >3600, wrong type, empty or oversized token rejected), provider/transport error mapping and redaction, reuse/expiry/refresh sequence, stale-not-served after failed refresh, 16 concurrent callers sharing one request, timeout/unreachable, fixed production origin, loopback-only test constructors, and `create_user` sending the minted bearer and dispatching nothing after a refused mint. `cargo clippy --locked -p cantos-api --all-targets -- -D warnings` PASS. Local fake token endpoints only. |
| **Proposed, not executed** | Dedicated account `cantos-enrollment`, a one-permission (`firebaseauth.users.create`) custom role and its binding, key handling as a root-only Podman secret, rotation and retirement of the broad default Admin SDK key (id prefix `2795eafc`). |
| **Not verified** | Google accepting the `identitytoolkit` scope and the one-permission role for `accounts:signUp`; the custom-token signer needing no extra grant; real token lifetimes; key creation not blocked by org policy; the Podman secret mount; deletion of the old key. |

Gates as printed (Linux, Rust 1.98.1, `CFLAGS=-std=gnu17`, `TMPDIR` outside any checkout):
`cargo xtask verify rust` **PASS** (rust-format, rust-lint workspace, rust-tests workspace);
`verify contracts` NOT_RUN (no contract touched). `cargo xtask preflight --changed-only`:
**UNSUPPORTED** (`postgres` missing `initdb`/`pg_ctl`/`psql`; nothing else started). With
`--skip postgres`: **FAIL** `repository` (`check_repository.py`: stale Leptos component
qualification and adoption lines for UI files this change does not touch; the identical 16 adoption
lines and header are reproduced on a clean checkout of the base commit), so rust-format,
rust-lint, rust-tests, dbsp, wav-reference and audio-inspection were NOT_RUN by preflight (the
rust ones were run through `verify rust` above). `verify postgres` was not run: no SQL changed.

Behavioral note: a mint refusal happens before the Auth create is dispatched and is mapped, like any
pre-dispatch failure, to a rejected create for that enrollment (no automatic retry, no user
created); a fresh attempt gets a new UID. There is no backoff between mint attempts; admission
limits bound the rate.

Unchanged and still unaccepted: custom Action URL on the Spark project, live IAM, project/API-key
binding, the real journey with the new credential, and cleanup of the live-test artifacts.
