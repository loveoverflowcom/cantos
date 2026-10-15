# Develop consistency review — 2026-10-15

## Pinned scope

Base: `f05b09bf24243bb99bc869f5009628294ebb15d8`, freshly fetched `origin/develop`.
Selection: `git log -15 origin/develop`, with Git's default graph traversal, **not**
`--first-parent` and not fifteen distinct pull requests. Merge commits count as commits.
Review includes each selected commit's first-parent integration diff, so older branch changes
introduced by a selected merge are not silently excluded. The union contains 212 historical
paths, including renamed/deleted paths; generated outputs were assessed with their owners,
not treated as separate policies. No deploy, product publication or merge was performed.

| Commit | Subject |
| --- | --- |
| `f05b09bf24243bb99bc869f5009628294ebb15d8` | Merge PR 134: storage acceptance safety |
| `fd04f9d5e618f14e25588bd1f794e29852c80ab5` | Merge PR 135: native splash |
| `300152d3c0dd4f13b9d873ddb2f34663587bb2fe` | Merge PR 132: auth/account UI |
| `756ca37ace365981583f91550bf9b33659e62a76` | Native launch layers and optional CMP splash |
| `6282d3fd52f947bb467438b02588f6c2fabe6a4e` | Probe guards and cached anonymous reads |
| `6ded1c581d6b963d389d622f2722fb10f4ad6293` | Auth motion and large-text layout |
| `3cfae65446b3ad4f825b141f752430a2706b7be7` | Auth/account variants and official logos |
| `b6066acc659b6d4cd030badb9c1abdad08b6dcf2` | Merge PR 126: Rust pin |
| `b347067cc7b033923f2ca4ec4cafa54d54404437` | Rust 1.98.1 compatibility |
| `d9cff230e4b584b48bfdbbcf7855aa78eb1414c0` | Merge PR 124: Range streaming groundwork |
| `8eca57a3808cc8bf07f29226e8c6e418a0792b58` | Bound grants and shared Range transport |
| `673011c1952de5482f0e1e8b90a24b8748c3c9df` | Merge PR 117: Quint foundation |
| `96674f725d12df2c28c76643f6f848bf1ca7a3f3` | Reconcile Quint ADR and evidence |
| `a234b5dd5cf22c3e94a63ceb14fe0558b56c1819` | Clean-head replay and restored contract gate |
| `792825c8766f6ea6bce8feeea41ba96bea1166e6` | Replay Quint against private admission core |

## Findings and bounded corrections

### P2: Conditional reads hid an already-known missing historical target

Class: **confirmed defect**; category: logic; confidence: confirmed by failing regression.
`authorize_audio_read` checked that any storage registry existed, not that it contained the
immutable object's recorded target. After switching to a registry containing only a new write
target, ordinary reads failed with 503 but newly introduced conditionals could return 304/412.
The [storage owner](../architecture/object-storage.md#immutable-location-and-integrity)
requires unavailable/no fallback for a missing historical target; HTTP preconditions do not
supersede cheaply detectable normal-request failures.

Correction: the private `AuthorizedAudioRead` factory now requires the registry's exact-target
routing check. `StorageRegistry::ensure_read_target` reuses the existing resolver without an
adapter call, byte verification, network I/O, fallback or new public capability. Current SQL
ownership resolution still precedes the factory. This is configuration readiness, not proof
that remote bytes exist. The [read contract](../../packages/contracts/private-content-reads-v1.md)
records that distinction.

Regression: an absent historical target fails with exact `Storage(Unavailable)`; a missing
registry retains `Unavailable`; a configured target permits construction even when no bytes
have been written. The existing real-router PostgreSQL acceptance scenario also covers a
replacement-only registry for GET/HEAD and 304/412/416 inputs, but that layer is separately
unrun in this executor.

### P2: Global middleware downgraded the private audio cache policy

Class: **confirmed defect**; category: compatibility; confidence: confirmed by failing regression.
`origin_and_cache` recognized audio policy by `Content-Type: audio/ogg`. A 304 has no such
Content-Type; 400/412/416 and other failures use the owning surface's JSON body. The middleware
therefore overwrote `private, no-store, no-transform` with plain `no-store`. No cache disclosure
is claimed: `no-store` remained present, but the documented stronger policy was lost.

Correction: transport owns one `PRIVATE_CACHE_CONTROL` constant. The outer middleware preserves
that explicit policy across response types and continues overriding missing/unknown/cacheable
policies with `no-store`; MIME is no longer a policy/ownership discriminator.

Regression: real production middleware over synthetic endpoint responses covers GET and HEAD
at 200/304/400/412/416/503, plus absent and explicitly public upstream cache policies. These tests
construct a lazy pool but never connect to a database. They are HTTP middleware evidence, not
authentication, object-storage or full-API integration acceptance.

### P3: Publication proposal reused Quint's ADR identifier

Class: **confirmed defect**; category: rule; confidence: confirmed from source and inbound links.
Both the Quint contract and the later publication-streaming proposal claimed ADR 0027.
The publication proposal is now [ADR 0028](../decisions/0028-publication-streaming-evidence-gates.md),
with inbound links updated. Quint remains ADR 0027. Both proposals retain their existing status;
no unresolved delivery/cache/native-codec choice is selected. The older duplicate ADR 0022
predates this scope and is not silently renumbered.

### P2, preview scope: Leptos supporting-text color crosses nested boundaries

Class: **confirmed defect**; category: compatibility; confidence: confirmed from source;
rendered/browser evidence is not held. Corrected during the independent second review.
`packages/ui-system/leptos/foundation.css.in` uses a descendant selector under a
`primary-container` Surface that reaches help/reason elements inside a nested ordinary Surface
or UiRoot. `color: inherit` then uses `onSurface` rather than normal `onSurfaceVariant` there.
CMP's nearest-Surface reset in `Layout.kt` and its nested metadata test establish the intended
renderer contract. The correction binds existing semantic colors through the renderer-local
`--cantos-web-supporting-text-color`, reset at every UiRoot/Surface and overridden by a
PrimaryContainer on that boundary only. Help and disabled reasons consume that binding; errors
retain their separate error role. The canonical palette, Kotlin output, geometry, strings,
focus and motion are unchanged. The CSS was regenerated from its template, not hand-patched.

The new source-contract regression fails on the before template and generated CSS and passes
after the patch. It asserts all binding owners and all help/reason color selectors, rejecting
the ancestor descendant-color override. A separate independent sRGB oracle checks the paired
and neutral metadata roles against their backgrounds in both themes. These are static/source
and token-pair evidence, **not** computed browser colors, DOM tests or rendered contrast.
The retained browser fixture covers direct, nested-Surface and nested-UiRoot help/reasons at
320/1280 CSS px in light/dark; its computed-color and opened-capture recheck remains pending.

A local installed Chromium run was attempted with a synthetic Vietnamese DOM and repository
CSS only. It could not launch: its local socket was denied by the syscall sandbox. The permitted
escalated run also could not start because the executor's `bwrap` setup encountered a read-only
sessions mount. No browser pixels, DOM result, native device, screen reader or interaction pass
is claimed; no security restriction or private-file guard was changed to obtain evidence.

### P2, test evidence: PostgreSQL preview helper retained the old MIME cache oracle

Class: **confirmed defect**; category: testing; confidence: confirmed from production route
composition and helper source, not an executed PostgreSQL failure. The independently reviewed
helper still expected plain `no-store` whenever MIME was not `audio/ogg`. An authenticated
missing-target 503 and authenticated 416/owner-resource errors pass through the audio handler's
private wrapper, so that assertion would reject the corrected policy before checking the
intended status. The helper now uses explicit caller expectations. Early Studio authentication
failures, read-access redirects and unregistered Theatre routes still expect plain `no-store`;
handler-owned private audio responses expect `private, no-store, no-transform`.
The non-PostgreSQL `surface_routes` suite passed unchanged: outer authentication failures never
reach the private wrapper. No production authorization or cache policy was broadened to make
tests pass. Real PostgreSQL execution remains required and unrun locally.

## Assurance ledger and target coverage

| Claim | Owner / target | Required by | Evidence and disposition |
| --- | --- | --- | --- |
| Historical audio retains exact storage identity before conditionals | Media application/registry; Rust server | doc: object-storage.md and private-content-reads-v1.md | Failing-then-passing factory and registry examples; held at L2–L4; PostgreSQL acceptance remains pending |
| Private cache mechanics survive HTTP composition | HTTP transport and middleware | doc: private-content-reads-v1.md | Failing-then-passing production middleware tests; held for the declared synthetic route scope |
| Grant/Range code does not publish private storage | Media Publication/Listening and separate HTTP groups | policy: AGENTS.md surface and privacy boundaries | Source review of grant identity, per-fetch eligibility, Range planning/stream bounds and consumers; existing native tests retained; real public delivery remains fail-closed and unaccepted |
| Quint owns accepted executable expectations, not transport or unresolved decisions | Catalog/models/protocol, Rust observer, runner/xtask | doc: specs/behavior/observation-protocol.md | Event-only observer calls production decision/constructors; no mirrored decision reducer found; lifecycle stays draft; model/core/API evidence remains distinct |
| Storage probes require exact ownership and stop on uncertainty | S3 acceptance/anonymous-privacy harness | doc: longvan-s3-acceptance.md; policy: storage ownership guards | Source review and existing offline unit coverage; live credentials, provider writes and cleanup never attempted |
| Shared UI preserves nearest-owner roles and host-owned effects | Leptos/CMP components, native launch hosts | doc: design/ui-system.md; shared renderer contract | Source review of auth/logo/motion and Android/iOS synchronous launch ownership; nearest-boundary Leptos binding corrected with failing-then-passing source regression and both-theme token contrast; no browser/native visual claim |
| Exact compiler pin does not alter domain semantics | Manifests, CI/xtask, fixed-array and default changes | doc: architecture/rust-toolchain.md | Source review plus applicable Rust checks; no new toolchain or dependency versions selected |
| Decision IDs point to one proposal | Architecture/contracts/docs | review-profile: conceptual/SSOT consistency | Publication renumbering and repository link checks; held; proposal statuses unchanged |

Coverage is complete for the declared targeted boundary passes, not a line-by-line safety
certificate for all 212 paths or the repository. Rust application/HTTP/storage consumers,
Quint models/catalog/ITF observation and actual core calls, scripts/xtask selection,
auth/logo/motion/launch consumers and owning docs were traced beyond changed lines.
Generated logo vectors, token outputs, static Android/iOS resources and evidence records were
reviewed against their source/generator boundaries, not freshly rendered on devices. Binary
assets were not individually pixel-inspected. No live issue/Project acceptance, provider/CDN
behavior, deployment identity or native playback claim follows.

## Verification

Toolchain: Rust/Cargo 1.98.1, existing installed tools. Full output is retained locally in ignored
`artifacts/agent-runs/develop-review/`; successful checks identify the final code inputs, while
intentional pre-fix failures remain separately named.

- `cargo xtask verify repository`: PASS (repository architecture/hygiene, Python tests and whitespace).
- `cargo xtask verify rust`: rust-format PASS; workspace rust-lint PASS; workspace rust-tests FAIL.
  The 17 failures are unchanged `xtask/src/services/tests.rs` private-fixture setup cases:
  this executor exposes `/tmp/.git`, so the existing safety rule correctly rejects those
  temporary fixtures as inside a Git checkout. Both permitted workspace roots also have a
  Git ancestor; no granted writable TMPDIR avoids the guard. No `.git` marker, policy or
  unrelated test was removed/relaxed. The affected API library suite separately passed
  462 tests, with 68 explicit ignores; the aggregate workspace failure is not called PASS.
- `cargo xtask verify contracts`: PASS for script-ir-reference, protobuf-reference,
  contract-tests, wav-reference and audio-inspection, including actual locally encoded
  synthetic Ogg/Opus signals. These are not provider or named-listening quality evidence.
- `cargo xtask verify behavior --setup`: PASS, model-only, with the locked development
  dependency installed from the pinned npm lock and lifecycle still draft.
- `cargo xtask verify behavior --conformance --capability cantos.media.private-audio-admission`:
  PASS for model execution and independently observed `pure-core-synthetic-shell` conformance
  (31 positive traces, 265 checkpoints, zero original mismatches; semantic mutation and
  observer-fault sensitivity stages PASS).
  Bundle: `target/verification/behavior/cantos.media.private-audio-admission-ca10d0cced9b4206bf26d97707a82c1f/`.
  DB/API/authorization/media/storage/client/device acceptance and exhaustive proof are NOT_RUN.
- Unskipped `cargo xtask preflight --changed-only`: PostgreSQL UNSUPPORTED because
  `initdb`, `pg_ctl` and `psql` are absent; every other selected gate initially NOT_RUN.
  The available gates are run independently with the supported explicit `--skip postgres`;
  PostgreSQL remains NOT_RUN, never a waived acceptance claim.
- The interrupted first available-only preflight has no terminal result and is not counted as
  a pass. The final `cargo xtask preflight --changed-only --skip postgres` rerun is terminal:
  repository, rust-format, cantos-api rust-lint/rust-tests, dbsp, wav-reference and
  audio-inspection PASS; PostgreSQL is explicitly NOT_RUN. This is not full acceptance.
- Session 1 changed no UI source. Session 2 corrects the bounded renderer-local binding above;
  browser launch failures mean browser/native/UI acceptance is still NOT_RUN, not a visual pass.
  No deployment or live storage work was attempted.

### Independent second review

Session 2 starts from session 1 commit `0f3fff0036abfe20f135eeecd9a769df564bfe8b`, after the
first review completed. It rechecks ownership-before-readiness, private construction paths,
exact-target/no-fallback routing, post-storage reauthorization, HTTP composition and test
consumers, draft Quint catalog/model authority and event-only observation, and shared UI/native
launch ownership. It found the stale PostgreSQL cache oracle above in addition to the queued
Leptos correction. No further concrete production auth, type-barrier or Quint defect was found
in those targeted boundaries. This is not exhaustive review or proof over all historical paths.

Session 2 uses the same installed Rust 1.98.1 and existing tools; no new software is installed.
Final code inputs are recorded locally in `session2-source-inputs.json`, with complete output
under the same ignored run directory. Independently executed results:

- `python3 -m unittest discover -s scripts -p test_ui_component_states.py`: before regression
  FAIL for both CSS source/generated owner bindings; after correction PASS, six tests including
  the new source-contract and light/dark independent contrast checks.
- `cargo test -p cantos-api --test surface_routes --locked`: PASS, 12 tests against the real
  router without connecting its lazy PostgreSQL pool. Early authentication policy remains intact.
- `cargo xtask preflight --changed-only`: PostgreSQL UNSUPPORTED (missing initdb/pg_ctl/psql),
  all other selected gates NOT_RUN because prerequisites are prechecked.
- `cargo xtask preflight --changed-only --skip postgres`: terminal PASS for all 11 available
  gates: repository (403 Python tests), rust-format, affected-package rust-lint and rust-tests
  (cantos-api/cantos-studio/cantos-theatre/cantos-ui), dbsp, wav-reference, audio-inspection,
  web-lint, both Web builds and ui-preview. PostgreSQL remains explicitly NOT_RUN. The API
  library alone passes 462 tests with 68 intentional ignores; the Leptos native/string-render
  suites compile and pass, but these do not establish browser behavior.
- The unchanged full-workspace xtask fixture failure recorded in session 1 remains an actual
  aggregate FAIL, not replaced by the affected-package PASS. It was not needlessly retried.

- `cargo xtask verify behavior --conformance --capability cantos.media.private-audio-admission`:
  independent fresh rerun PASS for model execution and `pure-core-synthetic-shell` conformance,
  including original traces, model-mutant sensitivity, observer-fault sensitivity and freshness.
  Bundle: `target/verification/behavior/cantos.media.private-audio-admission-0f1662dd352046ffa7734e0fa38a76b4/`.
  DB/API/authorization/media/storage/client/device acceptance and exhaustive proof remain NOT_RUN.
- The post-report `cargo xtask verify repository` result is supplied in the final PR handoff;
  no pending run is counted as PASS here.

Assessment: bounded source corrections are ready for draft review with acceptance pending.
The fixes do not accept production/private-read integration, shared renderer parity or either
proposed ADR.

## Residual risk and next owner

The Leptos scoped-color source correction is complete; browser computed colors and inspected
same-scenario captures remain unaccepted. Real PostgreSQL private-read acceptance needs
PostgreSQL 17 binaries and a new disposable cluster via `cargo xtask verify postgres`;
no production/inherited URL is allowed.
The historical private-artwork fixture baseline is separate and has not been repaired here.
Provider privacy, public delivery, native devices and screen readers remain outside this
session. Missing infrastructure and actual test failures are reported separately, never as PASS.
The remaining runtime/SQL acceptance assignee is unassigned and must be assigned before acceptance.
