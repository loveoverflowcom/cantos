# PR69 review and history-preserving conflict resolution

## Boundary and source pins

This record covers the requested review, conflict resolution and new commit on
[PR69](https://github.com/loveoverflowcom/cantos/pull/69). It preserves the existing private
audio implementation on fresh `develop`; it adds no media feature or acceptance policy.

- Original PR69 head: `86e2dde58f2028a777da27bfe007aa89db7f3708`.
- Original merge base: `b620109d93f7ced11628ce380c282ccf7a9c351c`.
- Fresh fetched `develop`: `4066c462cb3e13bb1738d5ccaed19a08acc1f788`.
- That develop tree: `3c1a51d82a54be7ee80c5d7bd9a8ed031ad5b3a0`.
- Original three-dot contribution: 60 paths, 5,100 additions and 55 deletions.
- GitHub state before work: open, draft, not mergeable. PR69's original description is
  historical evidence and is not a description of this resolved delta.

The original intake checkout and the separate domain-assurance checkout were left intact.
Resolution used an isolated worktree, a task-local Cargo target and the available Rust 1.87
toolchain. Latest develop instructions were read directly, including the layered verification,
typed-error policy and focused logic/UI repair entrypoints. No new ADR was needed: this patch
selects no new product rule. Queue: no change because recommended order and rationale are unchanged.

## Supersession and resolution

[PR72](https://github.com/loveoverflowcom/cantos/pull/72), merged as
`7396ede88466ef56dc9e7759956a4072d6672a5b`, explicitly reused PR69. Commit
`4286056181aa9f86ace881297229d1ed84a12d5d` carries the original author and the
`cherry picked from commit 86e2dde58f2028a777da27bfe007aa89db7f3708` attribution.
It was followed by Content/Brief-bound Audio Lifecycle work and later frontend/skill/runtime changes.
Thus Git history does not contain PR69's original commit as an ancestor, although its feature
already exists and has evolved on develop. A positive `git cherry` result alone is not evidence
that this older branch still supplies a missing product feature.

All 60 originally changed paths were inventoried against the fetched develop tree:

- 22 retain exactly the original blob and mode.
- 37 exist with later changes.
- The remaining intake ADR was renumbered from `0019-studio-private-audio-intake.md` to
  `0020-studio-private-audio-intake.md` and updated. `0019-content-identity.md` already owns 0019.
- No original feature path is absent after accounting for that documented ADR move.

Merging fresh develop into the original PR head produced 32 conflicted paths. The newer
implementation was retained for the media domain/application/SQL/HTTP modules, contract
consumers, migration registry/tests, server privacy boundary, repository checks, xtask gates,
architecture/queue documentation and prior evidence. The only extra file left by automatic
merging was the obsolete 0019 intake ADR; it was removed in favor of current 0020.

Before adding this evidence record, `git diff --cached --exit-code origin/develop` succeeded:
the complete resolved tree was exactly develop, including unchanged executable/symlink modes.
The final delta against the pinned develop is this evidence file only. There is no parallel
intake path, additional migration 0010, reverted Content/Brief behavior or fabricated product change.
The published commit is a normal two-parent merge: original PR69 head first, fresh develop second.
It does not rewrite the old head, merge into develop, close the PR or change its draft state.

## Review assessment and assurance ledger

Assessment for the resolved product delta: `no-actionable-findings`.
The original conflict/stale-branch problem is `confirmed-from-source` and resolved by retaining
the current implementation. This is not approval, production acceptance or a safety certificate.

| Claim | Owner / target and required source | Evidence held | Disposition |
| --- | --- | --- | --- |
| Exactly four explicit Opus mono 20/24/32/40-v1 profiles, no default/AAC acceptance | Media profile/inspector; ADR 0020 and intake contract | Closed `DeliveryProfile`; exact profile/limit tests; real-media gate recorded below | held for named local checks; listening/interoperability deferred |
| Actual bytes determine checksum, size, Ogg framing/CRC/granules, mono 48 kHz, duration and coded-payload ceiling | Media `audio_intake`/`structure`; ADR 0018/0020 | Private `VerifiedAudio` has no serde/raw constructor; structure/metadata regressions; bounded real decode to null | held for named local checks; not subjective QC |
| Inspection is bounded, local-file-only and cleans only its owned process group | Media `process`, quarantine and shared inspector; intake/lifecycle contracts | Fixed argv, CPU/wall/output/descriptors/AS limits; timeout/output tests on Linux | held for executed cases; no fuzz or macOS run claimed |
| Immutable private SHA-256 objects survive dedup/restart and reject path/symlink/FIFO/tamper | Media `storage`; ADR 0020 | Descriptor-relative no-follow/no-overwrite, 0400 objects, 0700 roots, readback/fsync; real local-filesystem tests | held for tested Linux filesystem cases; S3/CDN/orphan collection deferred |
| Receipts and lifecycle records bind exact versioned digests and replay identity | Media core/application/row admission; intake/lifecycle contracts | Pure exact-error/replay/tamper/state/gate tests; immutable snapshot/digest checks traced | held for core; real SQL acceptance not rerun |
| Wrong owner, stale inputs, revocation/expiry and credential separation are rechecked before durable acceptance | Editorial/Catalog/Production/Media application and HTTP boundaries; AGENTS correctness rules | Current owner/scope/fence and post-INSERT rechecks traced; pure admission regressions; existing PG concurrency/recovery tests inspected | core held; real DB/auth concurrency remains gap-required |
| Private bytes cannot become Theatre/public delivery or an automatic release | HTTP router/static boundary and Media readiness; ADR 0018/0015 | Audio-enabled host has no ServeDir; Theatre/Admin stay reserved; candidate gate always includes `public_delivery_not_implemented`; route/core tests | held for source/ordinary route/core checks; deployed isolation not established |
| Applied SQL and original source history are retained | Migration registry and Catalog upgrade tests; ADR 0018/0020 | All migrations match develop; 0010 also matches original PR69 byte-for-byte; registry 1–14 and gap-upgrade tests retained | static preservation held; fresh migration/restart execution gap-required |

SQL 0010 SHA-256 remains
`afc799315314e97583bebc67390a17320c6abdcfff8ff124e0d8d59406f5c6bd`.
No applied SQL checksum, canonical digest format, DTO/error code, rights/approval rule,
storage key or public route changes relative to develop. No provider cost, TTS, encoding,
mixing, publication, deployment or persistent access was added.

## Fresh verification

The commands used Rust 1.87 and a separate build target. Ordinary suites use
`CARGO_NET_OFFLINE=true` and `RUST_TEST_THREADS=1`; dependencies were fetched from the locked
registry after the first offline xtask attempt identified one missing cache entry.

| Command | Result and actual scope |
| --- | --- |
| `cargo test --offline --locked -p cantos-api --lib contexts::media -- --test-threads=1` | PASS: 27 tests; exact Media core errors, packet/limit decisions, Linux process timeout/output and real private-filesystem cases |
| `cargo xtask verify rust` | PASS rust-format and workspace rust-lint; FAIL rust-tests: 17 xtask service-fixture setup errors (see below); all preceding package tests passed |
| `cargo xtask verify contracts` | PASS script-ir-reference, contract-tests, wav-reference and audio-inspection; 2 actual-media tests passed |
| `cargo xtask verify repository` | PASS: 464 text files, 17 skills, 225 Python tests, static service validation and both diff checks |
| `cargo xtask preflight --changed-only` | PASS repository and rust-format; selected only the final document delta against develop |
| `git diff --check` and `git diff --cached --check` | PASS on both the resolved product snapshot and final evidence snapshot before publication |
| `cargo xtask verify postgres` | NOT_RUN: this cloud environment's PostgreSQL Unix-socket creation was already denied, including reviewed escalation. The denial was respected; no alternate DB/socket/container route was attempted |
| Web WASM build, browser interaction, CMP/device, production storage/delivery | NOT_RUN: no product/UI change relative to fresh develop; this review does not renew their prior acceptance |
| Kani, fuzzing, mutation, listening, paid providers, remote CI | NOT_RUN: no new kernel/behavior or such acceptance claim; no CI polling/dispatch/retry |

The first offline `cargo xtask verify rust` invocation could not start because
`toml_writer 1.1.3+spec-1.1.0` was absent from the dependency cache (exit 101).
`cargo fetch --locked` supplied the missing registry dependency; the actual rerun is recorded
above. An earlier direct Cargo invocation was interrupted and rerun offline; only the completed
run counts as evidence. No toolchain, dependency version or source was changed for these retries.

## Existing environment/test-fixture failure

The full Rust workspace run passed format and clippy, then failed 17 existing develop xtask
service tests while creating their synthetic private configuration inputs. Their unchanged
`private_path` guard rejects any `.git` ancestor. This cloud sandbox injects a read-only
`/tmp/.git`; `/var/tmp` resolves to `/tmp`, so ordinary test temporary inputs are rejected with
`private input must stay outside any Git checkout`. The xtask test binary reported 113 passed
and 17 failed; these are actual failures, not a PostgreSQL skip or a Media regression.

An attempt to create an owned non-Git temporary fixture directory with reviewed filesystem
approval could not execute (`CreateProcess: TurnAborted`). No retry, removal of sandbox markers,
guard weakening or source change was made. The relevant Media suite, contract gates and
repository checks passed independently. A developer can rerun `cargo xtask verify rust` with
a permitted `TMPDIR` genuinely outside every Git checkout; no runtime/container startup is
needed for these ScriptedPodman fake tests. This pre-existing environment-specific failure is
retained in the report and does not create a product change in this conflict-resolution PR.

## Layer gates, deferred acceptance and coverage

- L1: documented ownership/dependency/supersession review. No new dependency edge or abstraction.
- L2/L3: the fresh Media core/type/error tests above support their named scope. Broader executed
  static/test results are recorded independently, not inferred from compilation.
- L4: ordinary route tests exercise the real router with a lazy, never-dialled PostgreSQL pool
  for pre-store refusals. Filesystem/process tests use real owned local resources. There is no
  fake-success full admission workflow presented as real database evidence.
- L5 DB/API/recovery: `NOT_RUN / IO_ACCEPTANCE_PENDING`. The required disposable PostgreSQL
  runner did not execute here; passing core and storage tests do not prove transactions,
  post-INSERT expiry, real owner revocation races, database restarts or SQL constraints.

Next integration owner is a developer with an authorized disposable PostgreSQL 17 environment,
Rust 1.87 and trusted ffmpeg/ffprobe. Run `cargo xtask verify postgres` and require migration
replay, the 0010 gap upgrade, exact owner/scope/stale/revocation cases, duplicate finalization,
receipt/object readback, rollback/expiry and process/database restart oracles to pass. Do not
substitute a production/inherited database. Prior Mac evidence in
[Audio Lifecycle](audio-lifecycle.md) and its [review](audio-lifecycle-review.md) remains
historical evidence; it was not rerun and is not attributed to this cloud task.

Coverage is complete-for-declared-scope for supersession, conflict preservation and the final
document-only delta. Wider assurance is partial: current Media profile, inspector/framing,
process/storage, legacy intent/application/receipt, lifecycle binding/admission/row restoration,
HTTP/static boundary and relevant contracts/migration tests were traced. Later unrelated client,
runtime and skills implementations were preserved by whole-tree identity rather than re-reviewed
as new feature work. No deployed server, browser, Android/iOS device, real PostgreSQL, S3/CDN or
human-listened audio was exercised.

Residual risk: current development-host limits and deferred orphan retention/production
identity/storage/rights/QC/publication/playback acceptance remain. Fresh database/API integration
is blocked in this environment; local successful core checks do not waive that acceptance.
