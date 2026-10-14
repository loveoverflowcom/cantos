# Bounded audio Range transport and exact playback grant binding

**Prototype against contract fixtures — not public delivery acceptance.** Part of
[#120](https://github.com/loveoverflowcom/cantos/issues/120), retaining #10/#11/#13/#14 ownership.

## Review boundary and provenance

Base: `develop` at `1c1d2107f1ca1aacac8c66b7b418c1070b35160c`, tree
`38f9d4a5af9cea4b8811ba253b941d1086c5f513`. This draft does not depend on unmerged #117/#123.
Current issue/owner inventory: #10/#11 storage/publication; #13/#14 Web/native players;
#16 offline; #51/#64 profiles; #97 Studio wire; #106 proposed stateful behavior SSOT.
No accepted Quint catalog exists in the pinned base, so this HTTP adapter slice adds no model
or competing business reducer. No UI/client/native code changes, therefore no UI inspection
or Web/CMP build is claimed.

Implemented:
- `PlaybackDelivery` output binds the complete sealed immutable permit, including manifest
  digest/version; a replay for another permit fails closed. URL construction validates a
  credential-free absolute HTTPS location, bounded length, no fragment/whitespace/control,
  required byte ranges and bounded expiry. No Theatre wire DTO/schema changes.
- `audio_transport` is the one HTTP-only Range/validator/body implementation, with an 8 KiB
  aggregate budget per evaluated header field-set (including per-line bookkeeping), 128 list
  members, and precedence/HEAD-ignore preserved. The real consumer
  is the existing Studio owner-authorized byte route; its independent authorize/open/recheck
  application path is retained. After owner authorization, already-known missing storage is
  rejected before conditional/Range planning; real-SQL regression execution is deferred.
  No Studio handler, cookie or storage port is used by Theatre.
- A crate-root `cfg(test)` fixture exercises production Theatre JSON handlers, `ListenerService`
  and the transport helper together with named `FixtureRead` and `CapturingPlaybackDelivery`
  doubles. It captures a genuine sealed permit rather than manufacturing public authority.
  Every fixture GET/HEAD re-resolves publication eligibility and expiry before conditional or
  range planning. Arbitrary test bytes have their actual SHA/length pinned; codec admission,
  publication object verification and database authority are explicitly mocked.

Not delivered: production publication-class storage/promotion, signer/host route, delivery-mode
acceptance, CDN/CORS, actual HTML audio, Android/iOS native playback, codec/profile/quality
acceptance, sparse index, mandatory prebuffer, time-bucket grants, offline cache or Protobuf.
No release produced, existing release/pointer written, provider called, deployment or credentials.
Normal host `UnavailablePlayback`/`UnavailableDelivery` remains unchanged and fail-closed.
[ADR 0027](../decisions/0027-publication-streaming-evidence-gates.md) is **proposed**, not accepted.

## Claim and oracle ledger

| Invariant / owner | Failure mode | Cheapest oracle | Evidence scope |
| --- | --- | --- | --- |
| Exact permit identity / Media + immutable publication | Adapter replays different publication/rendition/manifest/SHA/object/length/window | Seven independent identity-field substitutions through real authorize_playback | Focused application test; named grant double |
| Safe transient grant / Media | Credential-bearing/malformed URL, invalid expiry, no Range, logged URL | Exact constructor partitions and Debug sentinel assertions | Pure example tests; no live URL request |
| HTTP Range/validators / HTTP + existing ByteRange core | Wrong slices/status/length, stale If-Range, fake multipart, precondition order | Finite/table oracles plus response-body slices | Transport example tests, not CDN evidence |
| Exact pull-driven bodies / HTTP | Truncation/oversize, premature lane release, eager polling, surviving transfer | Instrumented source polls/drop, length/error/deadline injection | Deterministic source double; not a total-RSS or zero-copy proof |
| Current public access before every fetch / Media | Withdrawn/revoked/superseded/owner-disabled/expired grant used by fixture | Real eligibility resolver/application with named fresh-read double | Mocked integration; not real SQL/provider revocation |
| No publication exposure / composition | Fixture or private preview accidentally becomes live Theatre bytes | cfg(test) boundary, normal-host absent-route/default-delivery regression | Source/static + Router tests; production policy unproven |

## Layer gates and commands

Toolchain: isolated official Rust 1.87.0, Cargo/rustfmt/Clippy; Linux x86_64. Builds use two jobs,
no incremental cache and existing locked dependencies. PostgreSQL/Trunk/Gradle/Kani are absent;
no SDK/provider/container installation or startup was performed.

Fresh results, with full output in ignored `artifacts/agent-runs/issue120/`:

| Gate / exact command | Status | Scope and limitations |
| --- | --- | --- |
| `cargo xtask verify repository` | PASS | Architecture/links/JSON/service manifest and 356 Python harness tests; no real provider/database/Kani acceptance |
| `TMPDIR=<private task-specific /dev/shm directory> cargo xtask verify rust` | PASS | Native workspace format, Clippy `-D warnings`, unit/in-process integration and compile-fail docs; API library 434 passed, 67 ignored; not a WASM/browser/device build |
| `cargo xtask verify contracts` | PASS | Script IR/Protobuf independent references, contract tests, WAV reference and two real synthetic FFmpeg Ogg/Opus admission tests; not published playback or listening-quality evidence |
| `cargo xtask preflight --changed-only --base 1c1d2107f1ca1aacac8c66b7b418c1070b35160c` | UNSUPPORTED | PostgreSQL 17 `initdb`, `pg_ctl`, `psql` missing; all other selected gates initially NOT_RUN by prerequisite precheck |
| `cargo xtask preflight --changed-only --base 1c1d2107f1ca1aacac8c66b7b418c1070b35160c --skip dbsp --skip postgres` | PASS with 2 explicitly skipped (NOT_RUN) steps | Repository, format, API lint/tests, WAV reference and audio inspection PASS; DBSP and PostgreSQL NOT_RUN |

The unchanged optional `dbsp-experimental` matrix was not rebuilt for this default-feature HTTP
slice; its CI/merge gate remains pending (`cargo xtask verify dbsp`). No earlier deleted
toolchain/cache/evidence is reused. No actual Kani run is claimed.

Intermediate failures were fixed and rerun, not hidden: an incomplete parallel test module,
two test-fixture typing mistakes, and a repeated-after-EOF panic caught by the new regression.
The stream is now fused without weakening that regression. Initial workspace tests also failed
the existing private-input guard because this environment places Git markers at `/tmp` and
`/workspace`; `/var/tmp` is a symlink and did not provide a valid fixture root. The passing
workspace rerun used a fresh `/dev/shm` directory verified outside any Git ancestor, preserving
the guard. These failed attempts are retained in the local run history.

L1–L3 are passed for the sealed grant/default-feature HTTP mechanics. L4 is
**mock-integration-tested** with the named doubles above. Delivery progress for that scope only:
`CORE_VERIFIED / IO_ACCEPTANCE_PENDING`. Actual SQL freshness/storage readiness, publication
promotion/isolation, production cache/revocation and client/runtime acceptance are not established.

## Deferred acceptance and activation conditions

| Check / layer | Reason / prerequisites and next command | Acceptance criteria / next owner |
| --- | --- | --- |
| Real Studio SQL/private HTTP regression | Local PostgreSQL 17 binaries absent; prepare disposable tooling then `cargo xtask verify postgres` | Existing owner preview exact bytes/Range/HEAD plus unauthorized surface; #10 owner, assignment required |
| Optional DBSP feature matrix | Unchanged experimental engine outside the default-feature transport scope; `cargo xtask verify dbsp` remains required by CI | Feature-mode compile/tests before merge acceptance; foundation owner, assignment required |
| Publication SQL/provider/CDN | No accepted mode, publication object adapter, credentials/topology or live authorized sample; the [existing separately authorized operator harness](longvan-s3-acceptance.md#operator-procedure) checks private storage only, not publication; a publication-class acceptance command must accompany the future adapter | Select ADR alternative, publication-class isolation and actual version/sha/length; 200/206/416/CORS/no encoding/log/cache/expiry/retraction; #10/#11 owners, assignment required |
| Web live media | No live approved delivery sample or HTMLMediaElement adapter in this slice; `cargo xtask verify web` after required WASM/Trunk, then named browser network probes | Actual play/failure, seek, refresh pinned release, lifecycle teardown and measured networks; #13 owner |
| Native media | Linux cannot establish target runtime; existing Theatre mock commands cannot accept native playback | Named Android Media3/iOS AVPlayer device/OS observations; fallback only after #51/profile ADR; #14 owner |
| Profiles and quality | Arbitrary transport bytes are not an Ogg corpus or listening evidence | Approved permitted 20/24/32/40 kbps short/long/VBR/corrupt corpus, actual admission, quality/listening; #51/#64 owners |
| Performance/security rollout | No production route or benchmark | Actual source-frame/RSS/allocation/cancellation and request/seek/startup p50/p95; safe cache/auth/log policy before activation; #10/#11/#13/#14 |

Inline `TODO(issue120, ...)` markers at delivery composition, transport memory/security and
fixture object-open boundaries name the next owner/acceptance condition. None returns fake
success or bypasses an unavailable production capability.

Residual risk: deterministic fixture evidence cannot establish production publication isolation,
real revocation/cache behavior, SQL transaction freshness, native codec support or delivery
performance. This draft is not issue/phase completion, merge authorization or production rollout.

The 60-second stream deadline is checked on downstream demand, not an independent eviction
watchdog. A deliberately unpolled body can retain its source/lane until poll or drop; actual
server/proxy idle-client policy and total memory behavior remain rollout acceptance.
