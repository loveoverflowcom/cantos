# Audio Lifecycle independent assurance review

Assessment: **no-actionable-findings** for private upload/admission, named review and candidate
readiness. Coverage: **complete-for-declared-scope**,83contribution paths plus owning-context
consumers. Independent read-only reviewer `/root/audio_boundary_review` followed
[cantos-code-review](../../.agents/skills/cantos-code-review/SKILL.md). No reviewer edits, heavy
test rerun, human review or GitHub review submission is claimed.

BASE `5202ed2cb74db80df290caceae4dbc21cd4ea2e7`; executable/test HEAD
`a91cb70eeda6770756cc1ea4209572b3e63f6a19`; tree
`aafb0c3fb5a77033fd656c5c9e0e622559496454`. Completion-record edits after this pin are Markdown
only and receive their own repository/format preflight. [Full execution ledger](audio-lifecycle.md)
and [contract](../../services/cantos-server/contracts/studio/audio-lifecycle/v1/README.md) delimit actual acceptance.

## Resolved findings

All three were **P2, confirmed defect, confirmed-from-source**:

- Security: one bearer could become both read/upload credentials if upload was issued first.
  Owning entrypoint checks serialize both issuance directions under the Catalog fence;
  sequential-order and concurrent-race regressions pass.
- Logic/integrity: review row restoration admitted different top-level/request operation IDs.
  Both canonical validation and equality now hold, with constructor/real operation regressions.
- Compatibility:405method rejection and malformed paths could bypass JSON ApiError transport.
  All three audio routers now use JSON405fallback; shared Studio path extraction handles
  new/legacy malformed parameters. Actual Router negative cases pass.

Exact state/event Result assertions, escaped snapshot bounds, commit-stage expiry and Mac
resident-memory/successful-exit descendant tests strengthen the respective assurance claims.
No outstanding review finding blocks the declared local handoff.

## Assurance ledger

| Claim / owning boundary | Evidence held | Disposition |
| --- | --- | --- |
| Current ownership and exact Content/Brief/profile/digest / domain+application | pure negatives, fenced commit revalidation, wrong owner/stale/tamper/expiry; example/integration-tested | held |
| Immutable distinct identities and row restoration / domain+SQL | private values, validating constructors, exact operation bindings, append-only privileges/triggers; type-enforced/example/integration-tested | held within backend authority |
| Caller declarations differ from server facts / Media | actual-byte inspector supplies size/hash/codec/profile facts; notes/rights/QC are declared/named decisions | held; no legal/listening certification |
| Read/upload/Studio credential separation / owning use cases | both issuance orders and simultaneous requests under common fence; integration-tested | held |
| Actual audio profiles / Media | real Ogg Opus mono20/24/32/40 signals; ffprobe/decode-null and codec/channel/bitrate/corruption negatives | held; playback/quality excluded |
| Bounded process/private objects / shell | fixed arguments, permits, timeout/output tests, sampled Mac RSS, process-group cleanup, pinned no-follow directories, atomic dedup/hash readback | held with Mac RSS limitation |
| Retry/crash recovery / SQL+filesystem | concurrent finalization, rollback/expiry, API kill, real PostgreSQL restart and exact prepare/PUT/candidate replay | fault-injected/integration-tested |
| Review and candidate staleness / pure gate | exact20state/event results, independent128gate combinations, latest decision and exact input/review/metadata pins | example/differentially-tested; publication always blocked |
| Studio/Theatre boundaries / HTTP | authenticated private routes, static-serving regression, Theatre/Admin404, no publisher/manifest/active pointer | held for declared scope |
| Migration compatibility / registry |0010equals original PR69;0011–0013 equal BASE; old9and already-applied11–13 gap upgrades pass | integration-tested; applied SQL preserved |
| Reads/contracts / SQL+transport | actual `cantos_app` SELECT/EXPLAIN baseline, schema/wire/router regressions, explicit DBSP rejection | held for tiny measured workload |
| Fresh checks / implementing session | all16preflight gates, final Rust/contracts and repository logs PASS | held; remote CI not checked |

Reviewer independently ran diff whitespace and SQL byte-comparison checks (PASS) and confirmed
HEAD/tree/base. Executable evidence was run by the implementing session; reviewer read the logs
and artifacts and did not rerun the heavy lane:

- `/tmp/cantos-audio-preflight-final.log`:16gates PASS, no skip.
- `/tmp/cantos-audio-rust-final.log`: format, lint, workspace tests PASS.
- `/tmp/cantos-audio-contracts-final.log`: reference/contract/WAV/real-audio PASS.
- Real PostgreSQL logs:33HTTP tests (10AudioLifecycle),19legacy intake,20Catalog/upgrade,
  plus2exact restart tests PASS.
- `target/catalog-evidence/audio-server-restart.json`: before/after facts equal; real socket
  requests, API kill, PostgreSQL restart and exact immutable replay/private-byte integrity.
- `target/catalog-evidence/audio-read-baseline.json`: `cantos_app`, PostgreSQL17.4,6377byte
  synthetic fixture,20samples each; status p95=15.985ms, candidate p95=11.204ms. Warm tiny local
  baseline only; no production SLO/cardinality claim.

Rust cases were serialized after an existing ephemeral-port flake. Explicit asynchronous
concurrent admission/credential-race tests still executed.

## Residual risk and excluded acceptance

Development identity/TLS/credential delivery and global-fence throughput need production
acceptance. Mac RSS is sampled leader memory with transient/descendant overshoot possible; CPU
limit is configured/source-reviewed and Linux hard-AS execution is NOT_RUN. Private orphan
retention/collection and S3/CDN remain follow-ups. Legal rights coverage, subjective QC/fidelity,
loudness, human listening, live UI and native/Web playback are NOT_RUN. Checksums establish no
audio quality. Public publication is always blocked; private byte GET/preview and Theatre API
are absent. Real prior-release preservation/delivery rollback is NOT_RUN because this boundary
has no release pointer. These declared limits do not block this private-admission local handoff.
