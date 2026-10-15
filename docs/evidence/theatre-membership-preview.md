# Theatre Membership preview — issue 128

## Boundary and provenance

Base: `develop@f05b09bf24243bb99bc869f5009628294ebb15d8`; branch
`feat/issue128-membership-interest`; Mac mini checkout
`/Users/manhblue/projects/loveoverflow/cantos`. The initial clean splash branch and all other
worktrees/commits were preserved; develop was fetched and fast-forwarded before branching.

[Issue 128](https://github.com/loveoverflowcom/cantos/issues/128), its empty comments and
#127/#12/#34/#129/#130 dependencies were read. The [owning preview contract](../product/membership-interest.md)
separates account registration, interest receipt, plans and paid entitlement. This is the
issue-permitted pure-state/preview slice, **not completion of the production capability**.
No HTTP/SQL, signup/provider, payment/entitlement, native host or shared component admission
change is included. PR132's tokens/assets are merged; auth wrappers/Logo remain unqualified.
Existing app-owned/native Material controls reuse canonical tokens, fonts and local artwork.

The [Drive handoff](https://drive.google.com/file/d/1ETmdL9w1ywC10OqjPIrhV9ERy2l59tvJ/view)
and `auth-runtime.mjs` were read. Web plans and mobile form PNGs were opened and inspected.
Their copies are `artifacts/agent-runs/issue128/reference-{web-plans,mobile-form}.png`.
The large linked [Web HTML](https://drive.google.com/file/d/17qt_BRSuG0iV3PBX0g4FG2uH9w4GOcw4/view)
and [Mobile HTML](https://drive.google.com/file/d/1dYf5Ww16naGGU0RGTluVoGhNg--Id0_b/view)
read failed with transport closure; streamed references were returned but local materialization
failed DNS resolution. Their rendered app behavior is not claimed from the source/PNG reads.

Final app source manifest: `artifacts/agent-runs/issue128/source-manifest.json`, SHA256
`5eaaef908d9345aa69c3bc746384f7fce8257ae5d7bd0ba9a11047c786f11ede`.
The commit/PR head is supplied in the PR and handoff; this manifest records the app bytes without
self-referencing the documentation commit. All raw evidence remains ignored and local.

## Invariant, implementation and oracles

Invariant: only a matching explicitly confirmed fixture receipt may reach received; unknown
outcomes, navigation or account changes cannot confirm interest or confer paid access.
Owner/boundary: pure Theatre client cores → controlled presentation shells; no IO adapter.

Typed validation accumulates name/email findings, keeps candidate input and binds trimmed contact
to an opaque intent. Pending duplicates/edits are refused. Unknown retry keeps the operation/body,
uses a fresh request fence and rejects the old callback. Back preserves the session draft and
fences pending results; clear-session removes private projections without reusing request IDs.
Plans status is independent of the receipt and returns to its closed internal origin. Default
submission is unavailable; receipt/account-complete fixtures are explicitly opt-in. Neither
renderer connects this memory-only diagnostic to live identity or playback.

The hand-authored [shared corpus](../../apps/cantos-theatre/web/fixtures/membership-interest-v1.json)
has 12 validation vectors and 7 transition traces, consumed by Rust and Kotlin tests. No
Membership Quint model/observer exists; none was invented or accepted here. Added test declarations
cover exact refusals, scalar limits, retry/replay, stale callbacks/session reset, plan origins,
receipt preservation, form anatomy and fixture isolation. Declarations are not executed evidence.

| Layer / claim | Status | Evidence and limit |
| --- | --- | --- |
| L1 scope, dependency guards | PASS | `statically-checked`: staged repository structure/link/architecture check passed; no new shared-component imports/baseline exemptions |
| L2–L3 initial Rust core | PASS | `example-tested`: initial dirty snapshot ran 119 unit tests, including 12 Membership cases, plus 3 doc tests; `core-first.log` |
| L2–L3 final Rust/core corpus/UI | NOT_RUN | Final corpus and Web UI were added after that run; final compile/test not claimed |
| L2–L3 Kotlin core/corpus | NOT_RUN | 10 core + 2 vector test methods declared; no Kotlin compile |
| L4 Web SSR/input/history/focus | NOT_RUN | 6 SSR/history/projection tests declared; browser not launched |
| L4 CMP semantics/interaction/pixels | NOT_RUN | 5 desktop flows + 1 capture test (6 scenarios) declared, none executed/opened |
| L5 Membership HTTP/SQL | NOT_RUN | No accepted contract/persistence implemented; no auth/idempotency/durability acceptance |
| L5 native/provider | NOT_RUN | No device, OS Back, secure-store, signup, payment or screen-reader observation |
| Independent review | NOT_RUN | Parent must commission its separate Extra High review; this implementation pass does not replace it |

## Commands and failures

Commands below used `RUSTUP_TOOLCHAIN=1.98.1`; raw logs are under
`artifacts/agent-runs/issue128/`. No live S3, secrets, infrastructure startup, stopped acceptance
restart, PostgreSQL baseline repair or remote CI polling occurred.

| Command | Status | Actual output / limitation |
| --- | --- | --- |
| `cargo xtask doctor` | PASS / UNSUPPORTED by tool | Rust 1.98.1/WASM/Trunk available; Gradle, official Android CLI and several unrelated optional tools absent from PATH (`doctor.log`) |
| `cargo xtask preflight --changed-only --skip repository --skip rust-format --skip rust-lint --skip web-lint --skip web-build --skip theatre-cmp-demo` | PASS with 6 NOT_RUN | Initial Rust tests only, 119 + 3; `core-first.log`; before final UI/corpus |
| `TMPDIR=/private/tmp cargo xtask verify repository` | FAIL | Structure check PASS; Python401 ran with 1 failure/37 errors, chiefly sandbox `os.killpg` PermissionError (`repository.log`) |
| `cargo fmt --all -- --check`, `git diff --check`, staged whitespace | PASS | Syntax/format/whitespace only, not compilation |
| `cargo xtask preflight --changed-only --skip repository --skip rust-lint --skip rust-tests --skip web-lint --skip web-build --skip theatre-cmp-demo` | PASS with 6 NOT_RUN | Only rust-format executed (`preflight-disk-held.log`) |
| `TMPDIR=/private/tmp cargo xtask preflight --changed-only --skip rust-lint --skip rust-tests --skip web-lint --skip web-build --skip theatre-cmp-demo` | FAIL | Staged structure check PASS; repository Python failure prevents rust-format; five heavy gates explicitly NOT_RUN (`preflight-staged-light.log`) |
| `cargo xtask theatre-demo verify` | UNSUPPORTED | Missing installed Gradle on PATH; nothing built (`theatre-demo.log`) |
| `cargo xtask verify rust`, `verify contracts`, `verify web` | NOT_RUN | Held after low-disk instruction; no final Rust/WASM compile or broader contract run |
| Shared preview / official Compose capture | NOT_RUN | No shared component changed; official Android CLI unavailable; no capture run |
| Web/CMP actual visual matrix | NOT_RUN | No final renderer pixels, keyboard/text scaling, motion or screen-reader acceptance |

A diagnostic preflight without `TMPDIR=/private/tmp` also failed with 6 failures/39 errors,
including `/var` versus `/private/var` assumptions (`preflight-final-light.log`). The staged rerun
uses the documented temporary-path setting; its process-group failures remain. Unchanged Kani
fixture tooling fails while trying to kill owned synthetic groups; an assertion also sees no
probe records after those errors. No isolated clean-baseline rerun or permission expansion was
performed, so this is an environment diagnosis, not a claim that every failure is pre-existing.
The embedded negative-control behavior FAIL lines are Python fixture output, not an actual
Membership model run.

## Resource hold and reviewer procedure

Disk was checked before build: about 1.2 GiB available. The initial Cargo lane finished; free
space dipped to 372 MiB and later remained around 400 MiB. Parent directed no more heavy builds
until new Mac cleanup approval. No cache/evidence/worktree/data was removed. Read-only candidates
are recorded in `storage-inventory.md`: `target/debug/incremental` 664,472 KiB and
`~/.gradle/caches/8.14.3/transforms` 1,362,764 KiB (about1.93 GiB combined). Deletion is **PENDING**,
not authorized/executed here. Process inspection is sandbox-limited; no lsof handles were seen
in those candidates. Old PostgreSQL clusters/evidence are explicitly excluded.

After resolving disk/tool/process permissions, run the final required gates and inspect both
renderers before promoting this draft. Web: `cargo xtask run cantos-theatre --web`, then
`http://127.0.0.1:8082/?mode=membership-preview`; preview controls enable a blank fixture session.
CMP: `cargo xtask theatre-demo verify`, then
`CANTOS_MEMBERSHIP_PREVIEW_FIXTURE=1 cargo xtask theatre-demo run`; Account → Membership preview.
Android/iOS remain unavailable-default; their hosts were not modified.

Drive pending, known failure, unknown/reconcile, matching receipt, account completion, all plans
Back origins, deep-link fallback, session clear and late callbacks. Observe small widths,
light/dark, long Vietnamese text, 200% text, focus/error order and keyboard/OS Back. Open captures;
label JVM versus native evidence separately. Check that navigation never starts audio or changes
a release. No accessibility or motion claims are made before those observations.

Deferred production owners: #12/#34 resolve submitter/contact/authority/retention/receipt/plan
policy before reviewed HTTP/SQL work; #129/#130 own live auth entry/return; #127 owns shared
admission; #122 owns mobile release isolation. Residual risk: **final source remains uncompiled,
renderer interactions/pixels unaccepted, repository gate failed, backend business decisions and
all native/production acceptance remain open**. No issue closure or merge is authorized.
