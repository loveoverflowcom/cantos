# Speech fingerprint, verified clip reuse and regeneration planning

Status: bounded backend implementation, 2026-10-12. Branch `feat/speech-clip-reuse`, based on
`develop` at `342d65fdbd3764f76b772ee0a398a9e209115b65`. It advances the reuse criterion of
[#7](https://github.com/loveoverflowcom/cantos/issues/7) and the planning half of
[#8](https://github.com/loveoverflowcom/cantos/issues/8) and **closes neither**: #8's Studio
journey, object storage, a live provider, listening review and publication are all absent. Contract:
[clip reuse v1](../../packages/contracts/clip-reuse-v1.md). Decision:
[ADR 0016](../decisions/0016-speech-fingerprint-and-clip-reuse.md) (proposed).

Everything below is **local, development-only and synthetic**. There is no provider call, no real or
generated production audio, no object storage and no CDN. eSpeak NG was **not** run: the worker's
explicit synthetic provider supplies the audio, and its create-only execution ledger is the
independent "provider executions" oracle. A synthetic result is `example-tested` or `fault-injected`,
never `provider-live-tested`; real PostgreSQL results are `integration-tested`.

## Invariants and their oracles

| Claim | Failure it prevents | Cheapest oracle | Evidence |
| --- | --- | --- | --- |
| Equal valid requests give equal fingerprints | a spurious miss and rebilled speech | oracle vectors, generated law | differentially-tested, property-tested |
| Dialogue, character, rights record, scene, position and pronunciation order never enter the key | a reorder or move rebills paid speech | exhaustive destructuring, vectors, generated law, PostgreSQL revisions | type-enforced, differentially-tested, property-tested, integration-tested |
| Every effective field changes the key; equal keys imply equal effective requests | stale or wrong audio reused (cache collision) | per-field mutation, generated iff-law against an independent projection | example-tested, property-tested |
| An unpinned model has no fingerprint | silent drift reuse | refusal vectors, generated law | differentially-tested, property-tested |
| The database accepts a clip only if its stored document hashes to its fingerprint and it restates the accepted observation | a forged or mispaired registry row | direct SQL forgeries on a real cluster | integration-tested |
| A hit needs scope, producer, contract, a fresh probe and an equal checksum; missing, corrupt, unsafe, unprobed or foreign objects miss | another creator's, a synthetic or a damaged object reused | one example per refusal, exhaustive probe states, five damage kinds end to end | example-tested, integration-tested |
| A completed job is not a reusable clip | audio treated as reusable without evidence | a plan with no output root counts no hit; an unreadable root regenerates | integration-tested |
| One edit regenerates exactly one line; a revert, reorder and move regenerate none | uncontrolled rebilling | named plan scenarios, differential against a naive recompute, generated edit laws, real revisions | example-tested, differentially-tested, property-tested, integration-tested |
| A reused step has no attempt, reservation, dispatch or spend | a cache hit recorded as a provider call | invariant checker over every PostgreSQL test, provider ledger counts | integration-tested |
| Selection is fenced, idempotent and conflict-safe | double selection, a stale worker's write | stale fence, replays, two racing connections, four racing workers | integration-tested |
| A recorded provider attempt is reconciled, never bypassed by a hit | duplicate billing after an ambiguous attempt | intent then worker death then recovery | integration-tested |
| Lapsed authority blocks reuse and pauses the job | reuse past a revoked right, session or approval | rights revoked and actor deactivated | integration-tested |
| A closed gate blocks reuse and production in the plan | a hit widening a gate | blocked plan scenarios, unapproved snapshot | example-tested, integration-tested |
| Worker death between probe and selection leaves nothing durable | a lost or half-recorded hit | real `cantos-worker` process killed at `before_reuse`, then restarted | fault-injected |

## Defects found and fixed during verification

Found by the author's own tests, by an independent read-only review (a separate agent applying the repository's
code-review skill; PostgreSQL tests were reasoned about, not run, by that reviewer) and by tooling. Each fix has a
test that fails without it.

| Found | Defect | Fix and evidence |
| --- | --- | --- |
| tests | the existing migration-upgrade test hardcoded 7 migrations | expects 8 and checks `speech_clips` exists |
| self-review | the worker stopped probing at the first clip whose *digest* verified, and the lookup window was not scoped, so an older clip of another producer or voice record could hide or crowd out a valid one | scope (producer, voice record, contract) filtered in SQL before the 16-clip bound; store test with both producers registered |
| self-review | an authority lapse between lookup and selection left the step leased until expiry | the selection reports `Conflict` and the worker falls back to `prepare_step`, which pauses `authorization_invalid`; store test |
| review | a canonical document over 64 KiB (admission allows about 106 KB of pronunciation rules) would fail the registry `CHECK` and roll back acceptance of an already synthesized line | bound raised to 2 MiB in the type and the `CHECK` (a fingerprinted request is always registrable); `input_too_large` has no fingerprint; PostgreSQL test with a 100-rule list at the admission maximum |
| review | the database did not tie a clip to its step: a hash-consistent bogus document, another licensed voice record or contract could be registered | trigger binds the step's fingerprint and the frozen line's voice-rights record and contract; forgery cases on a real cluster |
| review | "no hit without a probe" was worker discipline, not a type barrier | `VerifiedObject` witness with private fields, issued only by `ClipProber::inspect`, required by `select_reused_clip` |
| review | exhaustive destructuring covered only the top-level struct | `SynthesisPerformance` and every `ResolvedPronunciation` are destructured too |
| review | a concurrent writer bypassing the job lock could add an attempt beside a reuse; reservations were unguarded | attempt and reservation inserts read the step `FOR SHARE`; a two-connection race test shows the insert waits and then fails |
| review | upgrade behaviour over real pre-migration rows was not exercised, and the invariant checker contradicted "no backfill" | upgrade test seeds a run at migration 7, upgrades, completes it through the ordinary path and shows the new success rule; the checker requires a clip only for fingerprinted steps |
| review | a stored fingerprint disagreeing with the recomputed one stalled a step; `JobSummary` could not read old reports; `placement_changed` was documented as covering reorders | such a step simply is not reused or registered; `reused_steps` defaults to zero; wording corrected |
| tooling | the mutation runner restored files with their old modification time, so cargo kept the last mutant's build and a later contract test saw a broken planner | the runner now touches each file; the whole tree was rebuilt and every reported run is from a clean build |

Review findings that were not changed: the vendor pin policy lives in the pure fingerprint module and the oracle
(a second pinned provider edits both and the vectors); a plan can under-forecast hits relative to the worker and
probes every candidate it lists; `Store::regeneration_plan` composes SQL and file reads in one shell function.

## Verification run

Final tree, 2026-10-12. Toolchain: rustc/cargo/clippy **1.97.0**; the repository pins **1.87.0**, which is **not
installed** here, so no gate ran on the CI toolchain. PostgreSQL 17.11 (Homebrew), Python 3.9.6, macOS. The shell
needed `LC_ALL=C` (PostgreSQL refuses to start without a locale) and `TMPDIR=/private/tmp` (short temp paths for
process tests). `proptest 1.11.0` is the only new dependency, dev-only, adding three locked crates.

Gate results exactly as the tools printed them. Gates that fail here fail identically on clean `develop`
(`342d65f`, checked in a separate worktree); no failure is attributed to this change without that check.

| Gate / command | Result |
| --- | --- |
| `cargo xtask verify contracts` | **PASS**: script-ir-reference, contract-tests (includes the three new targets `speech_fingerprint_contract` 8, `clip_reuse_contract` 5, `regeneration_plan` 23), wav-reference |
| `cargo xtask verify postgres` (new disposable cluster) | **PASS**: revisions 17, imports 9, adaptations 21, production 11, durable 50 (22 clip-reuse tests plus the real-process kill at `before_reuse`) and every HTTP/restart journey |
| `cargo xtask verify web` | **PASS**: web-lint, web-build (Studio, Theatre), ui-preview |
| `cargo xtask verify mutation` | **PASS**: 9 killed, 0 survived (the repository's Script IR mutation gate; unrelated to this change) |
| `cargo fmt --all -- --check` | **PASS** |
| `cargo xtask verify rust` | rust-format **PASS**; rust-lint **FAIL**: two `cloned_ref_to_slice_refs` errors in `tests/wav_inspection.rs`, a file this change does not touch, from a lint newer than the pinned 1.87 (identical on clean `develop`); rust-tests **NOT_RUN** (an earlier step failed) |
| `cargo clippy -D warnings` on `cantos-api` lib, bins and every test target this change adds or edits, plus `xtask` and `cantos-contracts` | **PASS** (run directly, not the xtask gate) |
| `cargo test --workspace --locked` (run directly) | 647 passed, 0 failed, 113 ignored (the PostgreSQL-only tests). One unrelated xtask test (`port_ownership_reflects_an_existing_listener`) failed once on a busy machine and passed 5 of 5 on re-run |
| `cargo xtask verify repository` | **FAIL**: `scripts/check_repository.py` and `git diff --check` pass; `python3 -m unittest discover` runs 202 tests (192 on `develop` plus this change's 10 oracle tests, which pass) with the same 4 failures and 1 import error as clean `develop` (`test_mutate_script_ir`, `test_ui_preview_catalog`; Python 3.9). One compose-preview process test is also timing-flaky (1 failure in 3 identical runs, none in the others) |
| `cargo xtask preflight --changed-only` | **UNSUPPORTED** as selected (the diff touches `Cargo.lock` and `xtask`, so it broadens to every gate, including `cmp-ui-preview`, which needs Gradle on `PATH`). With `--skip cmp-ui-preview` it stops at the failing `repository` gate and reports every later gate **NOT_RUN**; those gates were therefore run individually above |
| `cmp-ui-preview` | **NOT_RUN** (explicit skip; **UNSUPPORTED** here: no Gradle on `PATH`). This change touches no CMP code |

Mutation checks of this change (a recorded manual procedure, not a repository harness): 31 single mutants, each
applied to one file and run against its focused tests, **31 killed, 0 survived**. They cover the checksum, producer,
owner and voice-record checks, "not probed counts as a hit", rights lapse, newest-wins ordering, dropping
pronunciation or adding a byte to the canonical document, the pin policy and size bound, scene-sequence staleness,
a closed gate still reusing, hits counted as dispatches, placement change, witness issue and unsafe-object
classification, selection re-validation, the attempt-history guard, authority lapse at selection, the lease check,
registration omitted or ignoring the step fingerprint, the summary count, and eight database mutants (registry
bound, step-fingerprint and rights/contract binding, producer scope, attempt check, row lock, reservation guard).

### Counts added

Unit tests in the library: speech fingerprint 9, clip reuse 9, clip probe 8, `Failpoint` 1, `production-jobs plan`
parsing 1. Integration tests without a database: oracle vectors and generated laws 8, plan scenarios and generated
edit laws 23, contract schema/fixture/planner 5. Python oracle tests 10. PostgreSQL: 22 clip-reuse tests and one
real-process kill test. Property tests run 512 generated cases each with `proptest`; no failing-seed file is committed because none was
produced by the unmutated code.

### Completion report

```text
Invariant:          one changed dialogue regenerates only itself and its dependent mixes; a reused clip needs
                    fingerprint, scope, producer, fresh object evidence and equal checksum; a hit is never a send
Owner / boundary:   pure core (speech_fingerprint, clip_reuse, regeneration) / shell (postgres::durable::clips,
                    clip_probe, worker); no HTTP, Theatre or Admin change; one service, no new crate
Evidence level:     type-enforced, differentially-tested, property-tested, example-tested, fault-injected,
                    integration-tested (real PostgreSQL), mutation-tested (manual, 31/31)
Command:            cargo xtask verify contracts | postgres | web | mutation ; cargo test --workspace --locked
Provenance:         synthetic fixtures only; synthetic provider ledger as the execution oracle; oracle vectors by
                    scripts/reference_speech_fingerprint.py; no live provider, no eSpeak, no real audio
Edge classes:       equal inputs, position/identity changes, every effective field, unpinned model, oversize
                    document, missing/corrupt/swapped/unsafe objects, other owner/producer/contract/record,
                    lapsed rights/approval/session, stale fence, duplicate and racing selects, cancel/pause,
                    recorded attempt, lost worker, forged and concurrent database writers, upgrade over live rows
Not covered:        see "Not covered and residual risk" above
```


## Not covered and residual risk

- **Concurrent runs that miss the same fingerprint both dispatch.** Execution is at-least-once with
  idempotent selection, not exactly-once; both outputs register and the oldest valid one wins later.
  Cross-run in-flight de-duplication is deferred (ADR 0016).
- **Object storage.** The only store is the worker's private filesystem root; a worker with another root
  misses. A selected clip is not re-verified after selection, so the future mixer must re-check the
  checksum. No host power-loss, retention or distributed-storage evidence.
- **No live provider, no eSpeak run, no listening review, no audio-quality claim.** Google and VieNeu have no
  fingerprint and never reuse.
- **Staleness is a forecast.** Mix and QC records do not exist; marks are plan output only and ignore which
  clip a previous mix used when several share a fingerprint.
- **Backfill.** Runs and accepted attempts from before migration 0008 have no speech fingerprint or clip and never
  reuse.
- **Bound.** At most 16 registered clips per fingerprint are probed, oldest first; more fails closed as a miss.
- **Pronunciation.** One changed pronunciation rule regenerates every line carrying that list (business rule 16 as
  written); a per-line projection needs the product rule changed first.
- **Toolchain.** See the run record: Rust 1.87.0 is not installed here.
