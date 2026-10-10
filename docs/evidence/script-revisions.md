# Script revisions implementation evidence

## Scope and provenance

- Work item / issue: [010](../work-plan/010-import-and-edit-script.md), [Part of #1](https://github.com/loveoverflowcom/cantos/issues/1).
- Review boundary: a creator saves, reopens and accepts private Script IR through a real Axum/PostgreSQL backend, with conflict detection and actor history.
- Branch / base: `feat/script-revisions` from `origin/develop` at `1a0e083`; evidence applies to this uncommitted working-tree implementation, not the base commit alone.
- Prerequisite: the proposed Script IR predecessor was absent on the remote base. The user authorized proceeding from the issue and work items; the minimum consumed Script IR contract is included here. No separate predecessor acceptance is claimed.
- Live vs mocked: PostgreSQL 17.11 (`postgres:17-alpine`) container, real SQLx queries and Axum handlers; the restart test starts the real server binary. No mock store, external identity service, AI provider, object storage or UI was used.
- Decisions: [0002 authentication](../decisions/0002-creator-authentication.md), [0003 content](../decisions/0003-script-ir-contract.md), [0004 persistence](../decisions/0004-editorial-persistence.md): implemented choices proposed for maintainer acceptance.
- Outward actions: none; no commit, push, PR, issue comment or Project mutation.

## Invariant and owner boundary

Only an authenticated owner may read or change a script. Each save appends a valid immutable
version and moves the head atomically from the named base; retries return their prior result.
Accepted revisions pin exact immutable content and source/rights references. A failed request
must leave the last committed script and history intact.

`script_ir` owns pure validation/canonicalization; `editorial` owns pure save/conflict/integrity
decisions; `auth` resolves opaque tokens to private-field actors and owns the pure owner check;
`store` owns transactions; `http` only extracts, authenticates and maps errors. No domain module
imports HTTP or SQL types. No port trait is introduced without a second consumer/implementation.

## Claim and oracle ledger

| Claim / invariant | Failure mode | Oracle / evidence | Level |
| --- | --- | --- | --- |
| Creator stores and reads valid Script IR | Fake save or lost data | `creator_saves_reads_and_audits_versions_with_preserved_source_and_pending_rights` | integration-tested |
| Restart preserves content and history | Process memory mistaken for persistence | `committed_script_revision_source_and_audit_survive_a_real_backend_restart` kills/reaps and restarts the executable | integration-tested |
| Every resource route enforces ownership | Private manuscript leak or unauthorized update | `every_script_route_rejects_missing_identity_and_hides_other_creators_content`; forged actor/owner rejection | integration-tested; type-enforced Actor |
| Token validity fails closed | Stale or ambiguous credentials authenticate | missing/unknown/revoked/expired/disabled/duplicate-header cases | example-tested; integration-tested |
| Concurrent saves never overwrite silently | Both sessions commit over the same base | two HTTP tasks wait behind an explicit PostgreSQL row-lock barrier; exactly one succeeds and one conflicts | integration-tested |
| Duplicate delivery produces one effect | Lost response creates duplicate version or false conflict | creation/save replay after head advances; changed request/base reuse rejected | example-tested; integration-tested |
| Accepted content and provenance remain immutable | Edit mutates history | accept then edit and reread; UPDATE/DELETE/TRUNCATE rejected with SQLSTATE 23514 | integration-tested |
| Failure commits no partial state | History row or head survives rejected save | trigger-injected insert exception, unchanged row counts/head, same operation retries successfully | fault-injected; integration-tested |
| Stored corruption is not silently consumed | Invalid/tampered content becomes trusted | corrupted digest rejected through all reader paths; full wire integrity regression | example-tested; integration-tested |
| Source bytes and actor remain traceable | NFC conversion rewrites original source or actor is forged | independent SHA-256 of NFD/CRLF source; audit fields and parent versions asserted | integration-tested |
| Invalid IR has exact diagnostics | Unresolved IDs or unknown fields become accepted content | committed rejection corpus, ordered exact issue code/path assertions | example-tested; type-enforced |
| Canonical digest ignores representation only | Whitespace/NFD produces new digest or performance change is dropped | fixed Python-produced canonical golden, independent SHA-256, sensitivity/equivalence examples | differentially-tested; example-tested |
| HTTP contract matches actual implementation | Handwritten DTOs drift from schema | fixture validation and real successful/error HTTP response schema assertions | example-tested; integration-tested |

The five user acceptance criteria are covered by the first, second, third, fifth and seventh
rows. This does not complete issue #1's minimal Leptos consumer or the full 010 review gate.

## Executed commands

Working directory for all commands: repository root. Rust toolchain pinned in
[rust-toolchain.toml](../../rust-toolchain.toml); dependency graph in [Cargo.lock](../../Cargo.lock).

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
TEST_DATABASE_URL=postgresql://postgres:cantos_test_only@localhost:55432/cantos_test cargo test -p cantos-server --test persistence --locked -- --ignored --test-threads=1
python3 contracts/script-ir/check_fixtures.py --schema
python3 scripts/check_repository.py
python3 -m unittest discover -s scripts -p 'test_*.py'
git diff --check
```

Final results: formatter and Clippy (`-D warnings`) pass; 20 Rust unit tests, 5 HTTP
fixture tests, 14 real-PostgreSQL integration tests and 23 Python repository-check tests pass.
The independent Python golden/schema check passes (1 accepted / 18 rejected IR fixtures).
Repository checks and `git diff --check` pass. Auth CLI issuance/revocation is exercised using
OS-generated credentials without emitting them in diagnostics. Tool versions: rustc/cargo 1.96.1,
PostgreSQL 17.11, cargo-mutants 27.1.0. Local raw logs are under ignored
`artifacts/script-revisions/` (`clippy.log`, `unit-contract.log`, `postgres.log`, `mutation.log`).
The default Rust suite explicitly ignores database tests; only the explicit PostgreSQL run is
integration evidence. CI is configured to run both suites; no remote CI result is claimed.

## Targeted mutation and independent review

```sh
cargo mutants --file apps/server/src/script_ir.rs --re 'Validator::id' --jobs 1 --copy-target true --timeout 60 --build-timeout 120 --output artifacts/script-revisions/mutation -C --offline --cargo-test-arg=--lib --cargo-test-arg=script_ir::tests
```

`mutation-tested` only for `Validator::id`: 14 mutants, 13 caught, 0 missed, 0 timeout,
1 unviable. The unviable `Default::default()` replacement cannot compile because `StableId`
intentionally has no `Default`; this is a tool substitution limitation, not a surviving defect.
Baseline passed; outcomes and individual logs remain under the ignored mutation output directory.
This is not a whole-validator or whole-backend mutation claim.

A bounded independent `cantos-code-review` pass checked root-authored persistence, HTTP,
authentication, CLI and migrations. Its sole actionable integrity finding was fixed: stored
wire data must be fingerprinted before normalization. The regression
`stored_wire_tampering_cannot_hide_behind_semantic_normalization` proves whitespace-equivalent
corruption is rejected; PostgreSQL tests also tamper excluded source/rights references.
Reinspection reported no actionable source findings. A second independent domain/schema pass
reported no actionable findings. These source reviews do not replace the executed tests above.

## Script IR compatibility

- Schema version: read/write `0.1.0`; first implemented contract. `0.1.0-draft` is an illustrative historical fixture and is deliberately rejected, never silently converted.
- Fixture set: [contracts/script-ir](../../contracts/script-ir/README.md): one accepted Vietnamese episode, 18 rejected fixtures with exact issues, a canonical-byte golden and tagged digest. No real private manuscript.
- Digest scheme: `cantos-script-v1:sha256:` with domain separation. The golden is newly established; no deployed revision is rewritten. Rights/provenance references are excluded from performance bytes but retained and protected by a separate complete wire digest in PostgreSQL.
- Migration / compatibility: initial migration has no prior deployed schema; migrations rerun without rewriting rows. Future changes require new forward migrations and explicit schema compatibility.
- SpokenContent impact: no per-dialogue synthesis contract or audio reuse claim is introduced. Provider, casting and production pinning follow later.

## Docs and follow-ups

Root/server guides, consumed environment configuration, architecture, contracts, work item and
queue now distinguish implemented backend behavior from future Web/mobile/production work.
`AGENTS.md` verification commands were expanded for the real Rust/PostgreSQL toolchain.
Canonical skills retain several historical bootstrap descriptions; their stated discovery rule
still applies, but a future skill-maintenance task should refresh that repository-reality prose.

Next boundary: minimal Leptos save/reopen consumer, browser login/session decision, then the
already planned import/adaptation/editor issues. No new GitHub issue or status was created.

## Not covered / residual risk

No UI, browser login, mobile, TTS, media, import conversion, AI adaptation, production or
publication flow has run. Opaque credentials need operator provisioning and TLS for remote use.
Rights records are assertions pending review; acceptance grants no production/publication right.
The local role owns the schema and can deliberately disable triggers; split runtime and migrator
roles and exercise backup/restore before shared deployment. Source/rights correction currently
requires a new script. History is a version-range inspection API, not a general change feed.
There is no live database failover/backup restore, sustained-load, fuzzing or full mutation campaign
claim. Resource limits cover request size and pronunciation expansion, not a deployment quota.
