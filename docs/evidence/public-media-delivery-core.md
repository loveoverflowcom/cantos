# Public media delivery core (#170) and cover projection kernel (#171)

**CORE_VERIFIED / IO_ACCEPTANCE_PENDING — not public delivery acceptance.** Part of
[#170](https://github.com/loveoverflowcom/cantos/issues/170) and
[#171](https://github.com/loveoverflowcom/cantos/issues/171); FE consumers #13/#14/#16/#154/#155
and #158/#159 are unchanged. [ADR 0032](../decisions/0032-theatre-origin-playback-delivery.md)
and [ADR 0033](../decisions/0033-release-pinned-public-cover.md) are **proposed**, not accepted.

## Review boundary and provenance

Base: `develop` at `f3619bf` (Merge PR #193). One boundary: the release-bound public media
authority core — pure kernels, ports, the composition-gated Theatre route, mock integration,
registered bounded proofs and draft behavior models. No host composes origin delivery; no
DTO/schema/migration, provider, storage object, deployment, credential or client code changes.

Implemented (production code, default host unchanged and fail-closed):

- `domain::playback_grant` — HMAC-SHA256 grant sealing/parsing/authentication, `grant_expiry`
  and the per-fetch `decide_media_fetch` precedence. Pure; no clock, randomness or I/O.
- `OriginPlayback` (`PlaybackDelivery` for mode A), `PublicMediaOrigin`, the publication-class
  `PublicationObjects` port and `PublicationObjectRef` (redacted Debug). No production adapter of
  the port exists; only named test doubles implement it.
- `ListenerService::{authorize_media, open_media, media_lane}` and `GET/HEAD
  /api/v1/theatre/media/{grant}`: re-resolve → authenticate → decide → plan → open → re-admit.
  `with_origin_playback` is the single trusted composition point; `router_with_origin_playback`
  exposes it to trusted host code only.
- `domain::public_cover` — release-pinned cover projection over existing `artwork_at` history
  with an explicit per-image disclosure; total fallback; public facts exclude rights/provenance.
  It has no production caller and no shell that supplies a disclosure (proposal for #157).
- `ByteRange::normalize`, `playback_grant` and `public_cover` Kani harnesses, registered in
  `cargo xtask verify kani` as `byte-range`, `playback-grant` and `public-cover`.
- Draft Quint capabilities `cantos.media.public-playback-delivery` and
  `cantos.media.public-cover-projection` with named runs, witnesses and negative controls.

The earlier test-only `fixture_bytes` handler ([#120 record](playback-range-transport.md)) is
replaced: the Theatre contract fixture now exercises the production route and `OriginPlayback`.

## Claim and oracle ledger

| Invariant / owner | Failure mode | Cheapest oracle | Evidence level |
| --- | --- | --- | --- |
| A grant authenticates exactly one sealed identity / `playback_grant` | Edited expiry, other publication/rendition/manifest/SHA/object/length, other key, field-boundary shift or one-character change accepted | Six-dimension substitution, expiry edit, length-prefix shift, wrong key, property over every single-character change | example-tested, property-tested |
| Canonical bearer shape / `playback_grant` | Alternate encodings, uppercase, wrong length/prefix, non-ASCII, expiry > i64::MAX parse | Exact `Malformed` partition table | example-tested |
| Grant expiry bound / `grant_expiry` | Grant outlives TTL/rights, non-future grant, overflow near i64::MAX | Boundary table; property; Kani harness (registered) | example-tested, property-tested; bounded-model-checked **NOT_RUN** |
| Per-fetch admission precedence / `decide_media_fetch` | Expired, over-long, non-current, forged or outside-rights grant admitted; wrong precedence | Exact precedence table; property against an independent i128 specification; Kani (registered) | example-tested, property-tested; bounded-model-checked **NOT_RUN** |
| Every fetch rechecks eligibility before and after open / `listener_media` + route | Withdrawn/revoked/superseded/owner-disabled/expired grant served, including changes during object open | Production route with `FixtureRead` and `FixtureObjects` doubles; open hook flips eligibility/clock | mock-integration-tested |
| Exact bytes, Range and headers / shared transport + route | Wrong slice/status/length, checksum drift, 304/412/416 opening storage, multi-range 206, cache/encoding headers | Range table with body slices and full-body SHA-256; conditional table; open counter | mock-integration-tested |
| No credential or object leak / adapters | Object ID in URL/Debug, key/MAC in Debug | Sentinel assertions over URL, token and Debug output | example-tested |
| Bounded lanes and cancellation / route | Unbounded concurrent bodies; lane not released; reads after drop | One-lane fixture: busy 503, poll count after drop, reuse after drop | mock-integration-tested |
| Typed object failures / `listener_media` | Missing object or integrity failure presented as success or other rendition | Exhaustive `StorageError` mapping; route status/code table; declared-length and truncation cases | example-tested, mock-integration-tested |
| Default host stays fail-closed / composition | Grant-only adapter or default host serves media | `with_playback_delivery` route test → 404 with zero opens; no host calls `with_origin_playback` | mock-integration-tested, statically-checked (source) |
| Hostile Range normalization / `ByteRange::normalize` | Range escapes object, wrong unsatisfiable set, byte-count overflow | Existing property; Kani harness (registered) | property-tested; bounded-model-checked **NOT_RUN** |
| Release-pinned cover / `public_cover` | Private replacement B or clear changes the public cover; cover before first selection | Real artwork decisions w1..w4, pinned revision projections | example-tested |
| Explicit same-image disclosure / `public_cover` | Undecided/revoked/expired/other-image decision shows a cover | Exact table; bounded exhaustive precedence oracle (non-vacuity count 10); Kani (registered) | example-tested; bounded-model-checked **NOT_RUN** |
| Cover failure never breaks listening / `public_cover` | Missing/foreign asset or corrupt history errors | Fallback table, duplicate-history case | example-tested |
| Stateful rules (both capabilities) / Quint | Vacuous property or insensitive assertion | Named runs, sampled properties, witnesses, one mutant per property | model-only (outside the xtask gate, see below) |

## Layer gates and commands

Host: macOS arm64, Rust 1.98.1 (`RUSTUP_TOOLCHAIN=1.98.1`), `TMPDIR=/private/tmp`, `LC_ALL=C`;
Python gates under a `python3 -> python3.14.8` PATH shim (system Python 3.9 lacks `tomllib`).
Base `f3619bff1d69246f22be2906570a3933ea922f53`, tree `3afb48539fb35520c95b6813f0366e98b345ab32`,
plus this branch's working tree. Full output is in ignored `artifacts/agent-runs/issue170-171/`.
Changing `xtask` broadened changed-only preflight to every gate.

| Layer | Status and evidence level | Covered claims |
| --- | --- | --- |
| L1–L3 pure core (`playback_grant`, `public_cover`, `ByteRange`) | PASS — example-tested, property-tested, bounded-exhaustive; Kani **NOT_RUN** | Grant shape/authentication/expiry/precedence; cover precedence and pinning |
| L4 application + route over named doubles | PASS — mock-integration-tested | Per-fetch rechecks, open-race denial, bytes/Range/headers, lanes, typed failures, fail-closed composition |
| Behavior model (two draft capabilities) | Gate **UNSUPPORTED**; same stages PASS outside the gate (model-only) | See below |
| L5 PostgreSQL / storage / browser / device | **NOT_RUN** | See deferred acceptance |

## Gate results

| Gate / exact command | Status | Scope and limitations |
| --- | --- | --- |
| `cargo xtask preflight --changed-only --skip repository --skip rust-format --skip behavior --skip rust-lint --skip web-lint --skip cmp-ui-preview --skip theatre-cmp-demo --skip audio-inspection --skip postgres` | PASS with 9 NOT_RUN | script-ir-reference, protobuf-reference, rust-tests (workspace: 1765 passed, 0 failed, 301 ignored; API library 509 passed, 68 ignored), dbsp, wav-reference, web-build (Studio, Theatre), ui-preview, mutation (Script IR 9/9 killed) |
| `repository`: `python3 scripts/check_repository.py` | FAIL, **pre-existing** | Only finding: `.agents/skills/cantos-cmp-mobile` SKILL.md has 307 lines (limit 300). Identical on clean `develop` `f3619bf` in a detached worktree; this branch does not touch that skill. The branch's own architecture finding (HTTP importing `OriginPlayback`) was fixed by allowlisting the trusted composition type |
| `repository`: `python3 -m unittest discover -s scripts -p 'test_*.py'` | FAIL (1 of 475, **pre-existing**) | `test_repository_skills_pass` for the same SKILL.md length; 474 others pass, 5 skipped. The behavior catalog test was updated for the two new capabilities and passes |
| `repository`: `cargo run -p xtask -- services validate`; `git diff --check` | PASS | — |
| `rust-format`: `cargo fmt --all -- --check` | FAIL, **pre-existing** | Diffs only in `apps/cantos-studio/web/src/live/artwork/{preview,tests}.rs`, `apps/cantos-theatre/web/src/features/player_timeline.rs`, `xtask/src/distribute/host.rs`; identical on clean `develop`. `rustfmt --check` over every changed/new `.rs` file of this branch: PASS |
| `rust-lint (workspace)`: `cargo clippy --workspace --all-targets --locked -- -D warnings` | FAIL, **pre-existing** | `clippy::type_complexity` in `apps/cantos-theatre/web/src/features/player_timeline.rs:314` (byte-identical to `develop`). Scoped `cargo clippy -p cantos-api -p xtask -p cantos-contracts --all-targets --locked -- -D warnings`: PASS |
| `web-lint` | NOT_RUN (skipped) | Not executed; it lints the same Theatre crate as the pre-existing workspace finding, and no Web source changed |
| `behavior`: `cargo xtask verify behavior` | UNSUPPORTED | Locked tool needs Node 24.19.0/npm 11.9.0; host has Node 26.5.0/npm 11.17.0 |
| Behavior stages outside the gate | PASS (model-only, not gate evidence) | Locked Quint 0.33.0 from `tools/behavior/package-lock.json` installed in a scratch directory under Node 26.5.0; a scratch driver reused `verify_behavior.py`'s own catalog loader and type/scenario/simulation/ITF validators with the catalog's seeds and budgets. Delivery: typecheck, 12/12 runs, 500×30 properties, 10/10 witnesses, 5/5 controls killed. Cover: typecheck, 10/10 runs, 500×30 properties, 8/8 witnesses, 5/5 controls killed. No setup receipt or freshness stage |
| `kani` (`byte-range`, `playback-grant`, `public-cover`) | NOT_RUN | Fixed x86_64 Linux model unavailable; xtask registration tests PASS |
| `postgres`, `audio-inspection`, `cmp-ui-preview`, `theatre-cmp-demo` | NOT_RUN | No ffmpeg/Gradle on PATH; no SQL or mobile code changed |

### Hand mutation of the new production lines

A scratch driver applied one substitution at a time, ran the focused API tests
(`playback_transport_tests`, `contexts::media::domain::{playback_grant,public_cover}`,
`contexts::media::listener*`), restored the exact bytes and touched the file. **9/9 killed**,
each by a named test; proptest seeds recorded only for mutants were deleted.

| Mutant | Killed by |
| --- | --- |
| Post-open recheck removed (`open_media`) | `withdrawal_or_expiry_while_the_object_opens_drops_the_stream_unread` |
| Grant expiry made inclusive | precedence table, widened-specification property, issued-grant property |
| MAC result ignored (`authorize_media`) | `forged_tampered_foreign_or_malformed_grants_are_an_indistinguishable_not_found` |
| Object ID dropped from the MAC preimage | `sealed_grant_round_trips_and_authenticates_only_the_exact_subject` |
| TTL (`Unbounded`) bound removed | precedence table, widened-specification property |
| Undecided cover shown | precedence oracle, explicit-decision table |
| Other-image decision treated as consent | precedence oracle, pinned-replacement and explicit-decision tests |
| Media `private_response` wrapper removed | six route tests asserting private/no-store headers |
| `with_playback_delivery` keeps the media route | `grant_only_composition_serves_no_media_even_for_a_valid_origin_token` |

This is `mutation-tested` for these nine inserted faults only, not a mutation-tool campaign.
Note that the route's expiry case also loses rights eligibility in this fixture (rights end at
10000), so expiry inclusivity is killed by kernel tests, not by the route test.

## Edge classes covered

Grant: canonical shape, every identity dimension, expiry edit, field-boundary shift, wrong key,
every single-character change, negative/overflowing expiry, clock rollback (`Unbounded`),
last admitted millisecond, rights-end bound. Route: withdrawal, revocation, supersession, owner
disablement, expiry before and during object open, forged/foreign/malformed/percent-invalid
tokens, open/closed/suffix/oversized/unsatisfiable/multi-range, `If-Range` strong/weak/other,
`If-None-Match` (304 before Range), `If-Match` (412), malformed Range/validator (400), HEAD,
`Accept-Encoding`, lane exhaustion and release, caller drop, missing/timeout/corrupt objects,
declared-length mismatch, truncated stream. Cover: pin before/at/after A, private replacement,
explicit clear, metadata-only edit, undecided/revoked/expired/other-image decisions, missing or
foreign asset, duplicate history, negative clock, privacy sentinels.

## Not covered / residual risk

- No real publication-class storage, SQL resolution, deployed proxy, restart or key rotation was
  exercised; the object double returns exactly the requested slice, which a real adapter must
  guarantee by version pinning (per-range SHA cannot be checked per request).
- The bearer grant travels in the URL path. The API logs no paths, and responses carry
  `Referrer-Policy: no-referrer`, but proxy/CDN access logs must redact `/theatre/media/*`.
- `GrantKey` is not zeroized on drop; no key loading, storage or rotation path exists yet.
- 32 lanes and the 60 s demand-polled deadline are unmeasured; slow clients can hold every lane
  and cause retryable 503s until a deployment idle policy exists. HEAD still opens the object.
- No CORS for a cross-origin Theatre Web; no browser/native player observed.
- The cover kernel has no production caller and no disclosure source; #157 must accept a contract
  before any wire, storage or delivery work.
- Kani harnesses are registered but unexecuted; the Quint models ran outside the gate under an
  unpinned Node, and their witnesses rely on documented step bias.
- Pre-existing `develop` failures (rustfmt in four files, workspace clippy in
  `player_timeline.rs`, the 307-line CMP skill) keep full preflight red independently of this change.

## Deferred acceptance

- **STORAGE / L5**: a promoted publication-class `PublicationObjects` adapter (#10/#11) with
  version-pinned reads; real bytes through the deployed proxy (GET/HEAD/206/416, checksum),
  restart and key rotation. Owner: Publication/Media + deployment. **NOT_RUN / BLOCKED**: no
  publication-class storage or environment permission exists in this change.
- **POSTGRES**: `PgRead` publication resolution under concurrent withdrawal during object open.
  Command: `cargo xtask verify postgres` once a real adapter test exists. **NOT_RUN**.
- **BROWSER/DEVICE**: Web `HTMLMediaElement`, Android Media3, iOS seek/refresh with the same
  release (#13/#14/#16). **NOT_RUN**.
- **Proof**: `cargo xtask verify kani --harness byte-range --harness playback-grant --harness
  public-cover` on the pinned x86_64 Linux model. **NOT_RUN** (host unsupported).
- **#171 contract**: #157 acceptance of ADR 0033, then wire/storage/delivery. **BLOCKED** on #157.
