# PR1 identity registry evidence

Scope: internal actor/external key domain, registry application/operator and additive PostgreSQL.
Base: fresh origin/develop `df7d62f`; VOT reference `f3fea641`.
Worktree `/private/tmp/cantos-identity-domain`, branch `feat/identity-domain`;
migration `0018_actor_external_identities.sql`. Existing checkouts/databases/evidence are preserved.

## Invariants and evidence targets

| Claim | Owner / failure | Oracle |
| --- | --- | --- |
| Legacy IDs/ownership/history | ActorId; identity rewrite would break FKs | Constructor/serde examples + old-schema fixture |
| Exact keys/issuer isolation | ExternalIdentityKey; case/email/issuer alias | Bounds/Unicode examples, properties, real SQL |
| Stable atomic registration | Domain+adapter; duplicate/orphan actors | Decision partitions, service/fake, two PG pools/fresh-pool replay |
| Explicit immutable linking | Domain+constraints; transfer/reactivation | Partitions, fake rejection snapshot, real conflict/inactive/immutability |
| Mapping is not authentication | Service/operator; raw claims granting authority | Source review, HTTP-role denial, no identity HTTP route |
| Fail closed | RegistryError; success fallback | Unavailable registry through service, SQL permission/fault rollback |

## L1–L5 results

| Layer / claim | Result | Held evidence / limits |
| --- | --- | --- |
| L1 ownership/decoupling | PASS | Static repository architecture checker; two independent source/docs review passes. Pure domain and real operator consumer, no identity HTTP route. |
| L2 types/errors/decisions | PASS | Private ActorId/key fields, fallible serde, exhaustive registration/link partitions and exact errors; identity9-test focused run. |
| L3 examples/properties/static checks | PASS | Two generated ASCII laws plus Unicode/NFD/control/byte-bound examples. Full workspace Rust format/lint/tests PASS in native local serial run. Contracts/reference/actual fixture media inspection PASS. |
| L4 application/fake | PASS | Production IdentityService with atomic FakeRegistry and UnavailableRegistry; replay, rejection snapshots, isolation and no success fallback. `mock-integration-tested`. |
| L5 PostgreSQL/operator | PASS | 8 tests on fresh real PostgreSQL 17.4: old-schema upgrade, ownership/bytes/timestamps/checksums, concurrent registration/candidate collision, fresh-pool replay, linking/inactive, immutable constraints, raw Unicode checks, trigger-injected actor rollback, HTTP-role denial, actual operator executable without sessions, indexed lookup. Full PostgreSQL gate passed twice. |
| Repository gate | FAIL | Architecture/links/JSON/skill static checks PASS; 304 Python tests retain 37 upstream Darwin `killpg` EPERM errors + 1 fallout failure, including after approved native-local rerun. |
| Original changed-only preflight | FAIL | Repository fails first; other selected gates NOT_RUN. |
| Available changed-only gates | PASS with 1 explicit skip | Repository NOT_RUN via `--skip repository`; format/API lint/tests/DBSP/WAV/actual audio inspection/PostgreSQL PASS. This does not replace full preflight acceptance. |

The PostgreSQL replay test discards a completed acknowledgement and uses a fresh pool; it does
not inject a network partition or prove identity recovery after a database process crash. The
broader existing gate does exercise API kill/database restart for legacy Studio workflows.

## Reproducible commands and provenance

Common environment: `RUSTUP_TOOLCHAIN=1.87.0 CARGO_BUILD_JOBS=2 TMPDIR=/private/tmp`.
Native full/available gates additionally use `RUST_TEST_THREADS=1` and existing real
`/private/tmp/cantos-audio-tools` executables prepended to PATH. PostgreSQL 17.4 is installed
Homebrew; each gate created and stopped its own disposable cluster, never read an inherited URL.
No Podman/identity provider was started, no tool or credential installed, no real account linked.

| Command | Actual result | Raw local output |
| --- | --- | --- |
| `cargo test --locked -p cantos-api --lib platform::identity` | 9 PASS | `/tmp/cantos-identity-core.log` |
| `cargo xtask verify repository` | FAIL (native-local/TMPDIR reruns retained) | `/tmp/cantos-identity-repository-escalated.log` |
| `cargo xtask verify rust` | format/lint/tests PASS (native, serial) | `/tmp/cantos-identity-rust-local.log` |
| `cargo xtask verify contracts` | script-ir-reference/contract-tests/wav-reference/audio-inspection PASS | `/tmp/cantos-identity-contracts.log` |
| `cargo xtask verify postgres` | PASS | `/tmp/cantos-identity-postgres-available.log` |
| `cargo xtask preflight --changed-only` | FAIL; later gates NOT_RUN | `/tmp/cantos-identity-preflight.log` |
| `cargo xtask preflight --changed-only --skip repository` | remaining gates PASS; repository NOT_RUN | `/tmp/cantos-identity-preflight-available.log` |

The final 8-test SQL/lookup log is `target/revision-evidence/identity-postgres-tests.log` under
this isolated worktree; earlier 7-test SQL output is retained in the first full PostgreSQL log.
The audio tools are actual Mach-O arm64 FFmpeg 6.0 executables, not shims. SHA-256:
ffmpeg `d42c2c1c90597054a16591b56875836a4cd98b5d94c46513022d1759135f7652`,
ffprobe `2c2cf5313a0f8c720ff99fdb7dc1559e14cab2cf119010c0431a2cab3b0ad6fc`.
No listening/audio-quality claim follows from structural fixture checks.

Initial failed output is retained: focused compile had temporary async borrows (fixed);
repository initial run additionally had `/var` alias mismatches (fixed by required TMPDIR);
default full Rust failed one unchanged macOS child-group test (native serial rerun PASS);
initial PostgreSQL was UNSUPPORTED without audio-tool PATH (resolved). No assertion was weakened.
`scripts/verify_kani.py` and the macOS media process test are unchanged from the pinned base.

## Read baseline and review findings

Operator workload: 2000 synthetic bindings/1 actor, 1 pool warm-up, 20 sequential service lookups.
Retained samples 141–250µs; EXPLAIN used `actor_external_identities_pkey`, shared hits 3 for the
binding and 1 for the actor, execution 0.007ms. Explain timing is not service latency; these tiny
warm local observations establish no production p95/p99, cold-cache, load or speedup claim.
No simpler extra index/cache is justified for the exact key; DBSP is rejected for this workload
as documented in the [read inventory](../architecture/identity-registry.md).

Independent review found stale migration oracles (count 14/range 1–14); they now assert count 15
and explicit versions 1–14+18, retaining historical checksum assertions. Follow-up tightened
Unicode SQL admission, moved implemented README text, narrowed property claims and added warm-up/
retained lookup output. Both read-only source/docs passes ended `no-actionable-findings`.

Original preflight summary:

```text
FAIL         repository
NOT_RUN      rust-format
NOT_RUN      rust-lint (cantos-api)
NOT_RUN      rust-tests (cantos-api)
NOT_RUN      dbsp
NOT_RUN      wav-reference
NOT_RUN      audio-inspection
NOT_RUN      postgres
```

Available-gates summary:

```text
NOT_RUN      repository               skipped by --skip
PASS         rust-format
PASS         rust-lint (cantos-api)
PASS         rust-tests (cantos-api)
PASS         dbsp
PASS         wav-reference
PASS         audio-inspection
PASS         postgres
```

## Residual risk and deferred acceptance

Draft delivery: named identity core and local SQL evidence are held; full repository acceptance
remains incomplete. Resolve/recheck the unchanged Darwin child-group fixture with the environment/
verification owner before clean merge acceptance: `cargo xtask verify repository`, then full
`cargo xtask preflight --changed-only`, with the same assertions. The combined Storage/Publication/
Discovery/Identity tree needs migration registry reconciliation and its own full gates.

Kani identity: NOT_RUN. Finite decision partitions have exhaustive examples. Properties generate
ASCII subjects/actor IDs; Unicode/NFD/control and byte limits have named examples and SQL cases.
A symbolic wrapper would not prove SQL concurrency; no
speculative runner/dependency is added. Provider-live/OIDC/browser/native/production: NOT_RUN,
outside PR1. See [contract](../architecture/identity-registry.md) and
[successor handoff](../work-plan/identity-authentication-handoff.md). Remote CI is not checked/waited for.
