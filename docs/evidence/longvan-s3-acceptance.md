# Long Vân S3 private target: profile, live-acceptance harness and evidence

Status record for using Long Vân as a **configuration target** of the existing provider-independent
private storage ([ADR 0037](../decisions/0037-provider-independent-private-storage.md),
[object storage](../architecture/object-storage.md)). It separates what the repository implements,
what was observed without credentials, and what is still `NOT_RUN`. The original profile
record below used no credential. The [operational follow-up](#offline-harness-hardening-and-live-blocker)
records a later private-access failure separately; **full Long Vân acceptance remains blocked**.

## Review boundary

Delivers:

- a non-secret [Long Vân profile](../../services/cantos-server/config/storage.longvan.example.toml)
  that the production configuration parser accepts (unit-tested) and that declares no
  unaccepted capability beyond the two the active write target requires;
- an operator-run live-acceptance harness over the **production** S3 adapter, with a redacted
  ledger and ownership-checked cleanup, plus a credential-free TLS preflight;
- the observations below, the reconciliation of the supplied integration plan against the code,
  and the deferred-acceptance checklist.

Does not deliver: a new adapter, port, route, migration, dependency, publication/CDN, Theatre
listening, multipart, deletion/listing, orphan collection or any change to the metadata wire.
Those are either already present, deliberately excluded by ADR 0022, or owned by other work.

Baseline: `origin/develop` `5dccf3a`, 14 October 2026.

## What already exists (no new abstraction needed)

| Need | Owner in the code | Gap for Long Vân |
| --- | --- | --- |
| Provider-independent private port | `PrivateStorage` in `services/cantos-api/src/contexts/media/storage/port.rs` | none |
| Generic S3-compatible adapter, SigV4, path or virtual addressing, no redirects, no proxy | `storage/s3.rs`, `storage/sigv4.rs` | no live evidence for any provider |
| Checked non-secret topology, named secret env vars, capability flags | `storage/config.rs`, `config/storage.example.toml` | no Long Vân profile |
| Durable target selection and historical reads | `storage/registry.rs`, migration `0015_media_storage_targets.sql` | none |
| Content-addressed keys, integrity, normalized ranges | `domain/object_storage.rs` | none |
| Owner-authorized Range/ETag delivery and optional version-pinned 60 s grants | Media read application and `/read-access` routes | grants and Range unproven on Long Vân |

## Supplied plan versus repository reality

The integration plan was written without auditing `develop`. Each item was checked against the
code; the decision column says what this change does about it.

| Plan item | Repository reality | Decision |
| --- | --- | --- |
| Implement under `apps/cantos-forge/src/{domain,application,adapters}` | Forge is the producer-side CLI; it registers, validates and exports a local bundle and has no object-storage or API handoff ([ADR 0022 Forge](../decisions/0022-cantos-forge-producer-cli.md)). Storage lives in `services/cantos-api` Media | not done; wrong owner |
| New ports `Put/Head/Get/Delete/PresignGet/Multipart`, `StorageRef` | `PrivateStorage` (`put_if_absent`, `verify`, `open_verified`, `temporary_read`) and `PrivateObjectRef`; ADR 0022 excludes HEAD, delete, listing and multipart on purpose | not done; would duplicate and widen the reviewed port |
| Keys like `cantos/dev/works/<work>/audio/<asset>/<rev>/audio.ogg`, configurable `key_prefix` | Keys are the SHA-256 of the actual bytes under the fixed `private/audio/` prefix; target is recorded per row | not done; the bucket is the isolation unit |
| Scripts, images and thumbnails in object storage | Script IR exports stay in PostgreSQL by ADR 0022 §2; private artwork already reuses this registry | not done; scripts are not object-storage content |
| Reconciliation worker for orphans | No orphan collector by design; needs reference-aware retention | deferred; unchanged |
| `[storage]` keys `backend`, `provider_label`, `presign_ttl_seconds`; `AWS_*` env vars | Schema is `[storage.targets.<id>]`; secrets use named env vars (`CANTOS_S3_*`); grants are 1 to 300 s | the existing schema is used unchanged |
| Runbook 03 through the AWS CLI | The CLI cannot prove the adapter's conditional create, pins or exact `206` handling | adapted as the harness below |

## Observed without credentials (14 October 2026)

These are single observations, not acceptance and not provider-confirmed.

| Observation | Meaning and limit |
| --- | --- |
| Unauthenticated `GET /` on `https://s3-hn1-r1.longvan.net` returns `200` with an empty anonymous `ListAllMyBucketsResult`; `Server: openresty`; request IDs shaped `tx…-CMC-HN` | consistent with Ceph RGW behind nginx; an inference, not a vendor statement |
| `GET /<missing-bucket>/x` returns `404 NoSuchBucket` XML | path-style routing is accepted by the front end |
| `<anything>.s3-hn1-r1.longvan.net` resolves to the same address | virtual-hosted DNS exists; addressing not tested |
| The adapter's transport refuses the endpoint's TLS chain from the author's workstation: `live_endpoint_tls_preflight` printed `tls_certificate_not_trusted` | **not a Long Vân defect**: `https://example.com` and `https://www.rust-lang.org` fail identically, and `openssl s_client` shows a Fortinet CA re-signing every site, so this network inspects TLS |

The last row matters operationally. `cantos-api` builds `reqwest` with `rustls-tls`, which
resolves to bundled Mozilla roots (`cargo tree -e features -i reqwest` shows
`rustls-tls-webpki-roots`). A corporate CA installed in the operating system is therefore not
trusted. Never disable verification. See [Open decisions](#open-decisions).

## Live ledger

Copy this into the operator's private record (outside Git). Every entry is `NOT_RUN` until the
command below produces it.

```yaml
run_date: null
operator: null
environment: dev
endpoint_class: https  # do not record the account bucket or keys here
signing_region: null
addressing_style: null
tls_dns_preflight: NOT_RUN  # a FAIL from the author's network is an environment result, see above
put_head_get_integrity: NOT_RUN
duplicate_put_idempotent: NOT_RUN
read_pin_available: NOT_RUN
full_read_actual_bytes: NOT_RUN
range_206_first_middle_final_suffix: NOT_RUN
anonymous_object_denied: NOT_RUN
anonymous_listing_denied: NOT_RUN
conditional_create_enforced: NOT_RUN
size_ladder_up_to_envelope: NOT_RUN
presigned_get_range_206: NOT_RUN
presigned_unsigned_denied: NOT_RUN
presigned_expired_denied: NOT_RUN
cleanup_current_object: NOT_RUN
cors_web: NOT_RUN
multipart_abort: NOT_RUN  # no multipart consumer exists
opus_play_seek_web: NOT_RUN
opus_play_seek_cmp: NOT_RUN
performance_measurement: NOT_RUN  # deferred by request
notes: []
```

## Operator procedure

Preconditions: a private test bucket that this account owns exclusively and that holds no
real data; a confirmed signing region from Long Vân (never guessed from the hostname); a
network that does not re-sign TLS, or the decision below. Use a credential scoped to that
bucket if Long Vân allows it. Never paste credentials into chat, a file or a command line.

1. Credential-free check that the adapter's transport can reach the endpoint:

```sh
CANTOS_LIVE_S3_ENDPOINT=https://s3-hn1-r1.longvan.net \
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp \
cargo test -p cantos-api --lib --locked -- --ignored --nocapture live_endpoint_tls_preflight
```

2. Full acceptance. The prompts keep the secret out of shell history and off the command line.

```sh
printf 'Access key ID: ';     stty -echo; read -r CANTOS_LIVE_S3_ACCESS_KEY_ID;     stty echo; printf '\n'
printf 'Secret access key: '; stty -echo; read -r CANTOS_LIVE_S3_SECRET_ACCESS_KEY; stty echo; printf '\n'
export CANTOS_LIVE_S3_ACCESS_KEY_ID CANTOS_LIVE_S3_SECRET_ACCESS_KEY
export CANTOS_LIVE_S3_ENDPOINT=https://s3-hn1-r1.longvan.net
export CANTOS_LIVE_S3_REGION='<region confirmed by Long Vân>'
export CANTOS_LIVE_S3_BUCKET='<private test bucket>'
export CANTOS_LIVE_S3_ACKNOWLEDGE=write-and-delete-private-probe-objects
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp \
cargo test -p cantos-api --lib --locked -- --ignored --nocapture live_private_bucket_acceptance
```

Optional inputs: `CANTOS_LIVE_S3_PATH_STYLE=0` to test virtual-hosted addressing,
`CANTOS_LIVE_S3_FULL_ENVELOPE=1` to include the 32 MiB envelope, `CANTOS_LIVE_S3_REQUEST_TIMEOUT_MS`
(default 60000) and `CANTOS_LIVE_S3_PRESIGNED_READS=1`, which exercises the grant paths only on a
version-pinned bucket (otherwise they report `UNSUPPORTED`; the grant code paths are compiled
but have not been exercised even against the fake provider).

The test refuses to start without every input and the exact acknowledgement, writes only
random probe objects under the adapter's `private/audio/` prefix, and retains them by default.
Cleanup additionally requires `CANTOS_LIVE_S3_CONDITIONAL_DELETE_VERIFIED=1`, an operator
declaration of separately obtained evidence that this provider honors conditional DELETE.
Do not set it from an assumption or from passing offline tests. The harness does not perform
a destructive capability-discovery probe. Failed, changed, uncertain and versioned probes are
retained even with that declaration. The harness
prints the ledger without credentials, raw headers/bodies, endpoint, bucket or any signed URL.
Retained probe keys are reported for the operator's private reconciliation record only. It exits non-zero on
any `FAIL`. Clear the variables afterwards (`unset CANTOS_LIVE_S3_…`).

A zero process exit with required `NOT_RUN`, `REVIEW` or `UNSUPPORTED` rows is not full target
acceptance. Inspect every required ledger row; the default cleanup hold is deliberate.

### Acceptance criteria

| Entry | `PASS` means | Typical `FAIL` and first check |
| --- | --- | --- |
| `put_head_get_integrity` | signed PUT with `If-None-Match: *` accepted, and a full GET hashes to the key | `object_forbidden`: region, keys or bucket policy; `storage_unavailable`: addressing, TLS, timeout |
| `conditional_create_enforced` | a second create of the same key with different bytes returns `412` and the original bytes remain | `if_none_match_ignored`: the provider accepts a duplicate create; it **cannot** be the active write target. Not every S3-compatible release honors the header: ask Long Vân |
| `read_pin_available` | the response carries a usable version ID or a strong ETag | none: reads fail closed and API delivery is unavailable |
| `range_206_first_middle_final_suffix` | exact `206`, `Content-Length` and `Content-Range` for first, middle, final and suffix ranges | a proxy that drops `Range` returns `200` |
| `anonymous_object_denied`, `anonymous_listing_denied` | `401/403` | `200` means exposure: stop and investigate policy, authorization and any response cache; owner-only ACL alone does not establish privacy; `404` is `REVIEW` |
| `size_ladder_up_to_envelope` | 1 MiB + 1 byte and 8 MiB (and 32 MiB when enabled) store and verify | `http_413…`: the gateway body limit (nginx-family defaults to 1 MiB) is below the 32 MiB envelope |
| `presigned_*` | version-pinned grant serves `206`, unsigned is denied, a fresh request after expiry is denied | `no_usable_version_id` is `UNSUPPORTED`: delivery stays on the protected API stream |
| `cleanup_current_object` | eligible current probes deleted conditionally and then `404`, after separate conditional-DELETE verification | `NOT_RUN` by default, on uncertainty or for versioned probes; retained keys require operator reconciliation; delete permission is never widened automatically |

A passing run accepts only the observed capabilities for that account and endpoint on that
date. It is not a CORS, player, throughput, durability or SLA result.

## Verification ledger for this change

| Layer / claim | Status | Evidence level and command | Scope and gaps |
| --- | --- | --- | --- |
| L1 ownership and dependencies | PASS | documented review; no `Cargo.toml`/`Cargo.lock` change; the harness is a test-only child module of the S3 adapter and imports no domain, HTTP or platform code | architecture only |
| L2 domain and types | N/A | not touched | no new type; existing refinements reused |
| L3 unit and static | PASS | `cargo xtask verify rust` (format, clippy `-D warnings`, workspace tests) and `cargo xtask preflight --changed-only --skip repository --skip audio-inspection --skip postgres` (`rust-format`, `rust-lint`, `rust-tests`, `dbsp`, `wav-reference` all PASS), with `RUSTUP_TOOLCHAIN=1.87.0 RUST_TEST_THREADS=1 TMPDIR=/private/tmp` | 1 profile test and 8 harness tests ran in 467 `cantos-api` library tests; 2 tests are `ignored` by design |
| L3 repository gate | FAIL (baseline) | `cargo xtask verify repository`: `services validate` and `scripts/check_repository.py` PASS (592 text files, links valid); `python3 -m unittest discover -s scripts` ends with 5 failures and 46 errors, **identical on a clean `origin/develop` worktree** | Darwin/Python 3.9 baseline, not caused by this change |
| L4 harness against a fake provider | PASS | example-tested, **mocked**: in-process in-memory provider proves the harness reports PASS, `FAIL` (ignored `If-None-Match`, 413 body limit, public read) and exact-key cleanup | proves harness logic only; no provider behavior |
| L5 real PostgreSQL and API | NOT_RUN | `postgres` and `audio-inspection` were skipped with `--skip`; no migration, route or query changed; `ffmpeg` is not installed on the author's machine | existing gates unchanged |
| Live: TLS preflight from the author's network | FAIL (environment) | `live_endpoint_tls_preflight` against three public hosts, identical result | network re-signs TLS; not a Long Vân result |
| Live: Long Vân put, read, range, privacy, cleanup | NOT_RUN | no credential used by the author | see procedure |
| Live: CORS, presign expiry, Opus playback, performance | NOT_RUN | no consumer or measurement exists yet | deferred; speed and quality verification is separate follow-up |

## Open decisions

1. **TLS roots.** To run the API or this harness from a TLS-inspecting corporate network the
   adapter needs the operating-system trust store (enable `reqwest`'s
   `rustls-tls-native-roots` alongside the bundled roots) or an explicit private-CA input.
   Either widens the trust surface and changes the dependency set that ADR 0022 pinned, so it
   needs an owner decision and its own change. Until then, run acceptance from a network that
   does not re-sign TLS.
2. **Signing region** and whether Long Vân's release honors `If-None-Match: *`: only Long Vân
   or the live run can answer. Do not set `conditional_put = true` on an active write target
   whose `conditional_create_enforced` is not `PASS`.
3. **Bucket versioning**: needed before `presigned_reads = true` is meaningful.
4. **Production isolation**: a private bucket for intake only. Publication/CDN delivery needs its
   own reviewed storage ([ADR 0037 §8](../decisions/0037-provider-independent-private-storage.md)).

## Deferred acceptance

| Check and status | Reason | Prerequisites and exact next command | Acceptance criteria | Owner and residual risk |
| --- | --- | --- | --- | --- |
| Full Long Vân live acceptance, `NOT_RUN / BLOCKED` | later scoped diagnostics observed anonymous access to private-ACL probe bytes; the full harness was not run | resolve private access at the endpoint, private test bucket, confirmed region and valid TLS; the two commands above | every required `PASS` row above, including anonymous denial after signed reads warm the exact URL | operator; no private audio activation until privacy is reverified |
| CORS from the real Studio origin, `NOT_RUN` | matters only for direct browser reads (grants) | browser DevTools from the real origin after grants pass | allowed origin reads, others blocked, `Range` and `Content-Range` readable | Studio owner; none while delivery stays on the API stream |
| Opus playback and seek, `NOT_RUN` | no player consumes these objects until #11/#12 | publication and listener access landed | playback, seek and resume on Web and CMP | Theatre owners |
| Throughput, latency, cost, `NOT_RUN` | deferred by request | representative audio, location, concurrency | recorded size, time to first byte, throughput, error rate | operator; unmeasured |

## Offline harness hardening and live blocker

The follow-up changes only the ignored test harness and its offline fake-provider tests.
It does not change the production adapter, API, schema, ACL or policy. No real provider call
is part of its offline verification; generated audio, credentials, account reports and raw
request evidence stay outside Git.

An operator's separately authorized, bounded probe observed a conditional conflict rejected
with `412` and unchanged bytes, plus one exact `206` range. It also observed ordinary
anonymous GET returning the actual private-ACL probe bytes with `X-Cache-Status: HIT`, while
query variants returned `403 AccessDenied`. The full harness was not run. This is a private
access failure, not full provider acceptance; the responsible cache/auth configuration remains
unverified. **The three prepared speech samples have not been uploaded through Cantos API.**

The harness now keeps attempted keys separate from probes whose absence, successful creation
and actual-byte integrity were established. A failed or uncertain mutation halts later writes
and cleanup. Cleanup is disabled unless the operator separately verified conditional DELETE;
the existing write/delete acknowledgement alone is insufficient. Before enabled cleanup it
rechecks integrity and the original read pin; only eligible current objects can receive a
conditional DELETE. Versioned probes are retained rather than
introducing version-history deletion. Unknown or changed objects remain for operator review;
a generated key or idempotent adapter success alone is not ownership evidence.

Anonymous diagnostics check the exact unsigned object URL, a no-cache request and a bounded
query variant. They audit the built request and record only closed cache-header categories,
body length/hash comparisons and status. A query denial cannot overrule an exact-URL leak.
Raw provider header values, signed URLs and response bodies are never printed by diagnostics.

Next acceptance belongs to the storage operator/provider: correct the private-access boundary,
then verify anonymous denial of the exact URL after a legitimate signed read warms it, as well
as query variants and signed hash/Range reads. Only then resume full provider acceptance and
the three-audio API checksum/Range/replay/restart sequence. Header declarations and offline
mocks do not repair or accept the live target.
