# Private Audio Lifecycle evidence

Review boundary: actual-byte private Content/Brief admission, immutable submission/rendition,
independent upload capability, append-only named review and private candidate/readiness.
[Contract](../../packages/contracts/audio-lifecycle-v1.md),
[frontend handoff](../work-plan/audio-lifecycle-frontend-handoff.md) and
[independent review](audio-lifecycle-review.md) own the exact interface and limitations.

## Provenance and scope

BASE `5202ed2cb74db80df290caceae4dbc21cd4ea2e7` (PR71). PR69
`86e2dde58f2028a777da27bfe007aa89db7f3708` is cherry-picked with original author and `-x`
attribution as `4286056181aa9f86ace881297229d1ed84a12d5d`. PR69 itself remains unchanged/open.
SQL0010 is retained exactly,0011–0013 checksums stay intact; additive0014 introduces lifecycle.
Registry1–14/count14. No synthesis, mixing, transcoding, generation job, DBSP consumer or live
frontend integration was added. Existing Media inspector/storage are shared by both intake scopes.

Mac ARM64, Rust/Cargo1.87.0, PostgreSQL17.4, Trunk0.21.14. Trusted FFmpeg/ffprobe6.0 from the
installed Android Studio ARM64 runtime were copied into `/private/tmp/cantos-audio-tools`, without
changing the application or installing a provider. Synthetic2second sine fixtures are encoded
only by tests outside Git. Server uses only ffprobe and decode to null; no speech was produced.
Cargo build artifacts reuse the prior session's target cache under Cargo locking; this worktree's
ignored test/evidence records remain separate. Only stopped disposable cluster data owned by
this session was reclaimed to keep disk available; raw logs were retained.

## Claim, oracle and evidence ledger

| Invariant / owner | Failure mode | Cheapest actual oracle | Evidence held |
| --- | --- | --- | --- |
| exact Content/Brief/profile/rights scope / domain + owning context | different episode/revision/profile inherits bytes | per-field pure negatives + actual HTTP recording/text/four profiles | example/integration-tested |
| producer claims differ from verified facts / Media | caller metadata certifies invalid bytes | actual Ogg Opus inspection; schema/DTO and preserved declarations | example/integration-tested; no quality certification |
| exact digest/size admission / Media | caller hash trusted or contradictory retry succeeds | actual upload+readback, wrong bytes, tamper/missing stored file | integration-tested |
| read/upload/Studio secrets remain separate / shell | reused bearer silently broadens rights | both issuance orders + simultaneous requests, wrong owner/scope | integration-tested |
| one receipt/object and immutable history / SQL+filesystem | concurrent finalize duplicates or overwrites bytes | concurrent PUT, second submission dedup, privilege/trigger refusal | integration-tested |
| commit after inspection rechecks authority / shell | upload expiry between inspection and commit | controlled rendition INSERT pause, expiry, rollback; private orphan remains | fault-injected/integration-tested |
| metadata rollback never makes public audio / Media | completed object gains an accepted/public pointer | fail INSERT, retry one receipt; reserved Theatre/Admin404 | fault-injected/integration-tested |
| review/candidate exact bindings / domain + SQL | stale/withdrawn approvals reused | named kinds, op replay/conflict, changed checksum, latest rejection/withdrawal | example/integration-tested |
| all candidate blockers are deterministic / domain | gate short-circuits or grants publish | independent128combination truth table; publication always unavailable | differentially-tested over7binary dimensions |
| explicit admission state transitions / domain | terminal/stale state admits new bytes | literal5state×4event exact Result matrix | example-tested/statically-checked |
| migration compatibility / SQL registry | gap0010 skipped after0011–0013 | old9baseline and already-applied11–13 upgrade; exact version list/checksums | integration-tested |
| recovery / real socket HTTP + PostgreSQL | lost response creates a new record | API kill, PostgreSQL restart, exact prepare/PUT/review/candidate history and private hash readback | fault-injected/integration-tested |
| inspector resources / process adapter | runaway subprocess or leaked descendant | time/output tests, sampled Mac RSS threshold, successful-exit descendant/unrelated-child fixture | example-tested on Mac; CPU configured/source-reviewed; Linux NOT_RUN |

## Read inventory and bounded performance evidence

| Consumer/read | Query and consistency | Bound/index |
| --- | --- | --- |
| owner/producer status and prepare/PUT replay | submission ID, owning Catalog/Brief facts and current permission under fence | PK ID; owner+operation unique; token hash unique |
| rendition status/review/candidate | exact submission ID, digest-admitted snapshot, actual private size/hash | UNIQUE submission ID; bytes≤32MiB |
| named review status/append/replay | rendition sequence DESC and owner+operation | UNIQUE(rendition,sequence), UNIQUE(owner,operation);≤1000rows;latest3on wire |
| candidate status/create/replay | exact candidate/rendition and owner+operation | PK ID; UNIQUE(owner,operation);copied metadata/review pins |
| scope/revocation/currentness | owning Catalog/Production entrypoints; Media revocation ID | prior owning indexes, PK submission revocation; no cross-context SQL |

Target chosen for the tiny local warm workload: p95≤500ms, concurrency1,20status+20candidate
Router samples over1submission/rendition/review/candidate and a real synthetic2second private
file. The real `cantos_app` role, exact SELECTs and `EXPLAIN(ANALYZE,BUFFERS)` plans are captured
in ignored `target/catalog-evidence/audio-read-baseline.json`. This establishes neither large
review histories, full32MiB file cost, throughput, cold cache nor production SLO. Frequencies
and production cardinality are unknown. Constraint indexes already serve these bounded point
lookups; no measured optimization/speedup is claimed. DBSP fit: **reject for this slice**—no hot
aggregate, repeated large join or CDC consumer justifies maintenance/state/operational costs.
The inherited optional engine gate is verification only, with no new DBSP-backed read model.

## Local verification

Final gate ledger is filled from the final attributable xtask summaries before handoff.
Command prefix in worktree `/private/tmp/cantos-audio-asset-admission`:

```sh
PATH="/Users/manhblue/.local/share/gradle/gradle-8.14.3/bin:/private/tmp/cantos-audio-tools:/opt/homebrew/opt/postgresql@17/bin:$PATH" \
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.87.0 \
CARGO_TARGET_DIR=/private/tmp/cantos-production-brief/target cargo xtask preflight --changed-only
```

Existing process/socket tests and disposable PostgreSQL require local-host sandbox escalation;
no production/inherited URL, remote CI request, paid vendor or cloud storage is used.
Initial failures were corrected before final verification: Mac variadic mode cast, old Clippy
violations, unsupported Darwin AS reduction, ffprobe6 packet side-data empty separators, test
borrow/token-length/schema selection. Reviewer fixes: reverse credential reuse and review
operation-ID split. Earlier FAIL outputs are diagnostic history, not passing evidence.

## Publication and residual risk

Release ID: none; manifest digest: none; active pointer: not implemented. Gates evaluate exact
inputs, recorded rights references, named review kinds, approval sequence, metadata and verified
private object integrity, then always include `public_delivery_not_implemented`. Storage class
is private development filesystem, SHA-256 immutable keys; no signed URL/public bucket exists.
Prior release/rollback and CDN range/CORS/delivery GET: NOT_RUN, because no release/delivery
feature exists. This PR cannot replace a prior active release; that future transaction needs
separate fault-injected Theatre/delivery evidence.

No `audio-listened`, fidelity/QC accuracy, loudness/true-peak, native/Web playback, screen-reader,
production identity or production publication acceptance is claimed. macOS limits sample leader
RSS every5ms and allow transient overshoot; they are not a hard/group-total memory bound. Linux
hard-AS path is source-reviewed only. Orphan collection/retention, S3/CDN, stronger legal rights
coverage and secure production credential delivery remain deferred. Global Catalog fencing
limits concurrency. Broader #51/#64/#66 stay open; remote CI is not checked or awaited.
