# Long Vân S3 private target: profile, live-acceptance harness and evidence

Status record for using Long Vân as a **configuration target** of the existing provider-independent
private storage ([ADR 0037](../decisions/0037-provider-independent-private-storage.md),
[object storage](../architecture/object-storage.md)). It separates what the repository implements,
what was observed without credentials, and what is still `NOT_RUN`. The original profile
record below used no credential. The [operational follow-up](#offline-harness-hardening-and-live-blocker)
records a later private-access failure separately; **full Long Vân acceptance remains blocked**.

## Native Server local handoff (26 October 2026)

Review boundary: make the existing private Long Vân acceptance handoff current and executable
for `cantos-server`/Turso. No new adapter, domain rule, schema, runtime fallback, publication
S3, migration, CDN or VPS change is selected; no ADR or queue-order change is needed.

Local-run provenance: fetched `origin/develop` and clean main checkout were both pinned to
`cda154e38f6c0d2dcf3a44354b4853838b458019`, equal to the submitted brief snapshot. The initial
handoff left three documentation changes uncommitted in the isolated
`docs/longvan-native-acceptance-20261026` worktree. The owner subsequently requested one Draft PR
to `develop`; its refreshed base is `4ce35ea84ab58694d37a5c842f51644ca4a201be`. Incoming changes
only affect Studio sign-in, so the storage source/tests behind the local results below are
unchanged. This publication grant does not authorize provider investigation or activation.
Current issues
[#309](https://github.com/loveoverflowcom/cantos/issues/309) (closed, filesystem-first decision
retained; its comment leaves real-host acceptance separate) and
[#446](https://github.com/loveoverflowcom/cantos/issues/446) (open, custody/access/binding layers)
were read with comments. Historical merged PRs
[#112](https://github.com/loveoverflowcom/cantos/pull/112) and
[#134](https://github.com/loveoverflowcom/cantos/pull/134) establish profile/harness provenance,
not current provider acceptance or authority for external operations.

### Source composition trace

Paths below are relative to `services/cantos-server/src/`. This is documented source review;
no running standalone binary or real S3 endpoint is qualified by the trace.

| Stage | Implemented source and ordering |
| --- | --- |
| Binary → Store | `bin/cantos-server.rs` opens the single Turso owner with `Intent::Serve`; mutually exclusive `CANTOS_STORAGE_CONFIG` / `CANTOS_AUDIO_ROOT` select `Store::with_storage_config` / `with_audio_storage` |
| Store → registry | `application.rs` owns `Store.media_storage`; `contexts/media/application.rs` constructs `StorageRegistry`; `storage/config.rs` validates topology and named secret inputs at startup |
| Registry → S3 | `contexts/media/storage/registry.rs` selects the active target only for new writes; `S3Storage` / `sigv4.rs` perform conditional create and bounded actual GET verification, with no retry/fallback |
| Bytes → Turso receipt | `contexts/media/application.rs::upload_audio_intent` closes its read snapshot before inspection/storage, then starts a short writer and rechecks current owner, intent/revision and authentication before `turso::insert_receipt`; version 20 `platform/turso/private_media.sql` persists immutable bytes/digest and recorded target/key |
| Receipt → authorized read | `contexts/media/read_application.rs` resolves exact scope/recorded target, performs `open_verified`, then reauthorizes the same immutable location; HTTP `studio/audio_reads.rs` supplies protected GET/HEAD/Range. Neither read nor write holds the Turso writer over storage I/O |

### Claim → failure → oracle → evidence

These results use Rust/Cargo 1.98.1 on macOS arm64, Python 3.13.5 and installed ffmpeg/ffprobe.
Build-only resource settings were `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 TMPDIR=/private/tmp`; disk started near 6.2 GiB
free. They do not change runtime storage semantics. Run output is retained in ignored
`artifacts/agent-runs/longvan-20261026/` in the isolated worktree.

| Claim | Failure prevented | Existing oracle | Evidence held in this run |
| --- | --- | --- | --- |
| Conditional create and actual-byte integrity | Overwrite, false receipt after PUT/metadata-only success, corruption/truncation | `s3::tests::put_and_duplicate_verify_actual_bytes_without_unconditional_retry`, `conflicting_or_corrupt_prior_objects_never_become_verified`, `truncated_actual_verification_and_range_bodies_are_never_successful` | PASS, controlled loopback HTTP double; **mock-integration-tested**, no provider durability claim |
| Read pin and Range | Replacement bytes served, wrong `206`/headers/compression, missing strong pin | `s3::tests::providers_ignoring_the_read_pin_cannot_expose_a_replacement_range`, `incorrect_range_status_headers_compression_and_changed_pin_fail_closed`, version/ETag and range tests | PASS, HTTP double; **mock-integration-tested** |
| Probe ownership and privacy diagnostics | Uncertain writes deleted, changed/versioned probes purged, query `403` hides exact-URL exposure | Offline `live_acceptance` tests, including `exact_cached_private_bytes_remain_a_failure_when_query_is_denied`, seed/cleanup race regressions | PASS, 26 offline tests; 2 live tests ignored; **example-tested / mock-integration-tested** |
| Receipt after bytes and reauthorization; no writer during storage I/O | Revoked owner accepted; serialized writer held across slow object work | `registry::legacy_intake_tests::owner_deactivation_after_inspection_is_rechecked_before_receipt_commit` pauses the named `PrivateStorage` double and independently acquires the real Turso writer before resuming; cancellation/rollback/lost-ack tests | PASS, real-file Turso plus named storage double (**integration-tested** engine; **fault-injected / mock-integration-tested** storage boundary) |
| Immutable receipt/replay/restart | Duplicate or changed acceptance, digest mismatch, durable result lost after reopen | `audio_intake_turso`: same-byte replay/conflict, lost HTTP response, rollback/retry, mutation barriers, migration replay and independent digest barriers | PASS, 19 tests, actual Turso/filesystem and synthetic Ogg/Opus; **integration-tested**, no S3 or production Identity |
| Historical target retained, no migration/fallback | New write default reroutes old reads or mutates immutable receipt | `changing_the_write_target_preserves_exact_historical_reads_without_copying_or_fallback` plus registry missing-target tests | PASS, real-file two-filesystem topology and named registry doubles; recorded bytes/digests unchanged; absent target `503 unavailable` |
| Config and manual test discovery | Inline secrets/default fallback, stale package/test command, accidental live run | Profile/config/registry tests; `--list` discovery below; `#[ignore]` on both operator tests | PASS for local parser/discovery; not a startup/provider probe |
| Provider privacy/durability and real S3 restart | Activation despite live exposure or unproven retention/reopen | Fresh operator gate 1–4 evidence | **NOT_RUN**; historical privacy failure remains blocking; provider/secrets/VPS UNKNOWN |

No new regression was added: these existing exact oracles cover the stated invariants and no
source behavior gap was confirmed. Only stale instructions were repaired (including current
Range/cache wording matched to the private-read contract). Destructive fault injection remains
local; the live harness remains test-only, ignored/manual, outside CI and preflight.

### Exact local commands and statuses

Use the build-only environment above for the attributable run. Discovery selected 736 library
tests and 19 `audio_intake_turso` tests; zero selected tests are never counted as PASS.

| Command | Result / actual scope |
| --- | --- |
| `cargo test --locked -p cantos-server --lib -- --list` | PASS, 736 discovered (discovery only) |
| `cargo test --locked -p cantos-server --test audio_intake_turso -- --list` | PASS, 19 discovered (discovery only) |
| `cargo test --locked -p cantos-server --lib -- --ignored --list live_private_bucket_acceptance` | PASS, exactly 1 manual test discovered; not executed |
| `cargo test --locked -p cantos-server --lib live_acceptance` | PASS, 26 passed / 2 ignored |
| `cargo test --locked -p cantos-server --test audio_intake_turso` | PASS, 19 passed |
| `cargo test --locked -p cantos-server --lib -- contexts::media::storage::s3::tests contexts::media::storage::registry::tests contexts::media::storage::config::tests contexts::media::storage::registry::legacy_intake_tests contexts::media::read_application::tests` | PASS, 39 passed; HTTP doubles, real-file fault tests, config/registry/read-readiness |
| `cargo xtask preflight --changed-only --base origin/develop` | Final docs-only handoff gate; exact result retained in the run record/final report; never runs the live harness |
| `cargo test --locked -p cantos-server --lib -- --ignored --nocapture live_private_bucket_acceptance` | **NOT_RUN**; remote probe writes require separate approval |
| Real Server/S3 restart, privacy remediation, secret access/import/IAM/binding, provider policy, deployment | **NOT_RUN**, outside local grant |

No performance change or measurement; throughput, provider retention/durability and live
privacy remain unaccepted. No release/manifest is produced, no rights/QC/publication gate is
waived and no delivery host is qualified. Local results do **not** activate Long Vân.

## Original profile boundary (14 October 2026)

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
Those are either already present, deliberately excluded by the storage ADR (then numbered
0022, now ADR 0037), or owned by other work. The original inventory/plan and verification
ledger below retain their old package/database names as historical provenance; use the
current procedure and native handoff for today's commands.

Baseline: `origin/develop` `5dccf3a`, 14 October 2026.

## Original inventory (14 October 2026)

| Need | Owner in the code | Gap for Long Vân |
| --- | --- | --- |
| Provider-independent private port | `PrivateStorage` in `services/cantos-api/src/contexts/media/storage/port.rs` | none |
| Generic S3-compatible adapter, SigV4, path or virtual addressing, no redirects, no proxy | `storage/s3.rs`, `storage/sigv4.rs` | no live evidence for any provider |
| Checked non-secret topology, named secret env vars, capability flags | `storage/config.rs`, `config/storage.example.toml` | no Long Vân profile |
| Durable target selection and historical reads | `storage/registry.rs`, migration `0015_media_storage_targets.sql` | none |
| Content-addressed keys, integrity, normalized ranges | `domain/object_storage.rs` | none |
| Owner-authorized Range/ETag delivery and optional version-pinned 60 s grants | Media read application and `/read-access` routes | grants and Range unproven on Long Vân |

## Original supplied plan versus repository reality

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

This historical observation used `cantos-api`; the current package is `cantos-server`. Both
build `reqwest` with `rustls-tls`, which resolves to bundled Mozilla roots (`cargo tree -e features -i reqwest` shows
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

This is a **future operator handoff**, not authority to execute remote operations. The
original 26 October task permitted local read/edit/test only; the later Draft PR request
adds documentation commit/push/PR authority. Neither authorizes secret import/access/binding,
IAM or provider-policy changes, live object writes/deletes, restart or deployment. Current provider,
S3 secret/access and VPS state are **UNKNOWN**; historical records do not refresh them.
Keep [#309's filesystem-first decision](https://github.com/loveoverflowcom/cantos/issues/309).

### 1. Privacy remediation and fresh recheck — BLOCKED

The [observed exact-URL exposure](#offline-harness-hardening-and-live-blocker) remains unresolved.
The storage operator/provider must remediate access/cache behavior, then supply fresh,
separately authorized evidence: signed reads warm the **same exact URL**, followed by unsigned
exact URL, no-cache and query-variant checks, plus unsigned listing denial. Each must deny
access without returning private bytes. Query-variant `403` cannot excuse exact-URL exposure;
ACL/header declarations or offline PASS cannot close the blocker. Retain sanitized status,
body length/hash comparison, cache category, date, source SHA and operator attribution in a
private record. Do not rerun the full write harness as a substitute for this prerequisite.
No public ACL, disabled TLS verification or presigned read may bypass it.

### 2. Separately approved secret custody and reader access — NOT_RUN

After gate 1, obtain explicit approval for the exact bucket/region, reader, secret/version and
permitted writes/budget. Follow the [Secret Manager guide](../deployment/google-secret-manager.md)
and [#446](https://github.com/loveoverflowcom/cantos/issues/446), retaining the distinction
`IMPORTED` → `VERIFIED_ACCESS` → `RUNTIME_BOUND`. That guide records historical ops-key imports;
it does not establish today's S3 pair, IAM or Server binding. Never request/paste credentials
in chat/issues or put values in TOML, argv, logs, shell history or Git. Inject through the
approved protected local route; do not print the environment. Clear selected inputs afterwards.

| Consumer | Inputs | Boundary |
| --- | --- | --- |
| Native Server | Absolute `CANTOS_STORAGE_CONFIG`; TOML-named `CANTOS_S3_ACCESS_KEY_ID`, `CANTOS_S3_SECRET_ACCESS_KEY`, optional configured `CANTOS_S3_SESSION_TOKEN` | Loaded once at startup; every configured target needs its declared secrets; mutually exclusive with `CANTOS_AUDIO_ROOT` |
| Manual harness | `CANTOS_LIVE_S3_ENDPOINT`, `CANTOS_LIVE_S3_REGION`, `CANTOS_LIVE_S3_BUCKET`, `CANTOS_LIVE_S3_ACCESS_KEY_ID`, `CANTOS_LIVE_S3_SECRET_ACCESS_KEY` plus exact acknowledgement below | Independent injection; runtime variables do not satisfy the harness; no harness session-token input exists |

Retain the original filesystem target ID/root and bytes. For an isolated campaign, copy the
[Long Vân example](../../services/cantos-server/config/storage.longvan.example.toml) to a private
run directory, replace the placeholder region/bucket, and include every target required by
that run's historical receipts. The profile is a declaration, never a default activation.
Keep `presigned_reads = false`; topology changes copy/migrate nothing.

### 3. Full provider harness — NOT_RUN, manual only

Preconditions: gates 1–2 accepted; an exclusively owned disposable private test bucket without
real data; confirmed signing region (never inferred from hostname); trusted TLS; reviewed probe
retention/budget. Commands run from repository root with Rust **1.98.1**. Discover without effects:

```sh
cargo test --locked -p cantos-server --lib live_acceptance -- --list
cargo test --locked -p cantos-server --lib -- --ignored --list live_private_bucket_acceptance
```

The second command **lists** the ignored test; it does not run it. Never remove `--list` during
local discovery. After separate authorization, the credential-free TLS preflight makes one
unsigned endpoint GET and no writes:

```sh
CANTOS_LIVE_S3_ENDPOINT=https://s3-hn1-r1.longvan.net \
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp \
cargo test --locked -p cantos-server --lib -- --ignored --nocapture live_endpoint_tls_preflight
```

Full harness, **DO NOT RUN under the local task grant**. The required key pair is already
injected securely at this point; the following are nonsecret selectors/explicit opt-ins:

```sh
export CANTOS_LIVE_S3_ENDPOINT=https://s3-hn1-r1.longvan.net
export CANTOS_LIVE_S3_REGION='<region confirmed by Long Vân>'
export CANTOS_LIVE_S3_BUCKET='<disposable private test bucket>'
export CANTOS_LIVE_S3_ACKNOWLEDGE=write-and-delete-private-probe-objects
export CANTOS_LIVE_S3_FULL_ENVELOPE=1
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp \
cargo test --locked -p cantos-server --lib -- --ignored --nocapture live_private_bucket_acceptance
```

The exact acknowledgement is historical and does not enable deletion by itself. The test
writes random probe objects under `private/audio/` and **retains them by default**. Leave
`CANTOS_LIVE_S3_CONDITIONAL_DELETE_VERIFIED` unset. Enabling cleanup requires separately obtained
provider conditional-DELETE evidence and separate authority. Failed, changed, uncertain and
versioned probes remain retained even then. On uncertain writes, stop later mutations and
reconcile attempted/owned/retained keys privately; never blindly retry or delete.

Optional nonsecret settings: `CANTOS_LIVE_S3_PATH_STYLE=0` tests virtual-hosted addressing;
`CANTOS_LIVE_S3_REQUEST_TIMEOUT_MS` defaults to 60000. Keep
`CANTOS_LIVE_S3_PRESIGNED_READS` unset for this private API-stream scope. Full-envelope acceptance
requires `CANTOS_LIVE_S3_FULL_ENVELOPE=1`; the default 8 MiB ladder cannot accept the 32 MiB limit.
The harness prints sanitized capability rows and retained probe keys for private reconciliation,
never credentials, raw headers/bodies, endpoint/bucket or signed URLs.

A zero process exit is insufficient. Require every private-write/read capability row below to
PASS. Record cleanup as deliberately NOT_RUN with an owner/retention decision and presigned rows
as NOT_RUN with the feature disabled; neither is falsely promoted to PASS. CORS, playback,
performance, durability/SLA and restart are separate claims. Do not add these ignored tests to
ordinary CI, preflight or builds.

### 4. Isolated native Server/Turso intake — NOT_RUN

Only after gates 1–3 and **separate** authorization for real Server/S3 intake and an isolated
process restart. `audio_intake_turso` is the available local real-file oracle; it always selects
filesystem targets and does **not** consume `CANTOS_LIVE_S3_*` or become a live S3 test when those
variables are set. There is no automated real-S3 restart command in that target.

Use a new owner-only absolute run directory outside Git, a new `CANTOS_SERVER_DATABASE`, a
run-specific topology and approved audio; never a production file, existing host unit or
another task's fixture. Build via `cargo xtask build cantos-server`; set `SERVER_BIN_DIR` to the
actual output directory. With no Server owning the run's database:

```sh
# Future isolated campaign only; inputs are resolved/reviewed first.
umask 077
export CANTOS_SERVER_DATABASE="$ACCEPTANCE_ROOT/server.db"
"$SERVER_BIN_DIR/cantos-migrate"
"$SERVER_BIN_DIR/cantos-migrate" --check
"$SERVER_BIN_DIR/cantos-operator" create-creator > "$ACCEPTANCE_ROOT/actor.txt"
# Issue a short-lived development token to an owner-only file, never the terminal.
"$SERVER_BIN_DIR/cantos-operator" issue-token "$(cat "$ACCEPTANCE_ROOT/actor.txt")" 1 \
  > "$ACCEPTANCE_ROOT/token.txt"
```

Create the run directory with mode 0700 and `umask 077` before these commands. Register only
synthetic/approved evidence with the existing `register-evidence` operator command, then start
the built Server in the foreground using `CANTOS_ENV=development`, an unused explicit loopback
`CANTOS_BIND_ADDRESS`, matching `CANTOS_STUDIO_WEB_ORIGIN` and the reviewed
`CANTOS_STORAGE_CONFIG`. Do not configure Identity, public media or an installed service for
this narrow development-token campaign. This does not qualify production Identity/login.
The operator commands require the Server stopped; they cannot share its exclusive Turso owner.

Use the existing Studio JSON endpoints: `POST /api/v1/studio/session` with a protected
file-backed `{"token":…}` request and private cookie jar, then
`POST /api/v1/studio/scripts/{script}/revisions` with `expected_revision = 0`, a stable lowercase UUID `operation_id` and approved
`script_json`. The [Editorial contract](../../services/cantos-server/contracts/studio/editorial/v1/README.md)
owns the body; register all that Script IR's evidence refs before starting the Server.
Keep cookie/token bytes in protected files, never curl argv or tracked reports. Use the
returned exact revision/content digest and a new stable lowercase UUID per intent:

```sh
# intent.json contains operation_id, expected_revision, expected_content_digest and
# explicit profile_id (for example opus-mono-20-v1), per the intake contract.
curl --fail-with-body --silent --show-error --cookie "$COOKIE_JAR" \
  -H "Origin: $STUDIO_ORIGIN" -H 'Content-Type: application/json' \
  --data-binary @"$ACCEPTANCE_ROOT/intent.json" \
  "$SERVER_ORIGIN/api/v1/studio/scripts/$SCRIPT_ID/audio-intents" \
  > "$ACCEPTANCE_ROOT/intent-response.json"
# Resolve UPLOAD_PATH and STATUS_PATH from that response; require same-origin API paths.
curl --fail-with-body --silent --show-error --cookie "$COOKIE_JAR" \
  -H "Origin: $STUDIO_ORIGIN" -H 'Content-Type: application/octet-stream' \
  --request PUT --data-binary @"$APPROVED_AUDIO" "$SERVER_ORIGIN$UPLOAD_PATH" \
  > "$ACCEPTANCE_ROOT/receipt.json"
curl --fail-with-body --silent --show-error --cookie "$COOKIE_JAR" \
  "$SERVER_ORIGIN$UPLOAD_PATH" > "$ACCEPTANCE_ROOT/readback.ogg"
cmp "$APPROVED_AUDIO" "$ACCEPTANCE_ROOT/readback.ogg"
```

The [intake](../../services/cantos-server/contracts/studio/audio-intake/v1/README.md) and
[private-read](../../services/cantos-server/contracts/studio/private-content-reads/v1/README.md)
contracts own exact DTOs/statuses. Repeat for each of the three approved samples only when
that sample scope is separately authorized. Keep a private per-sample matrix:

| Step | Required independent oracle |
| --- | --- |
| Admit | Exact receipt observed SHA-256/length/profile equals locally measured bytes; Turso row records the chosen target/key and canonical receipt bytes/digest |
| Full read / Range | `200` and byte-for-byte equality; first/middle/final/suffix `206` with exact slice/Content-Range/length; past-end `416`; accepted SHA-256 ETag and private no-store headers |
| Replay / conflict | Identical PUT yields identical original receipt, including ID/time; different valid bytes yield `409 operation_reused`, original object/receipt unchanged |
| Authorization | No cookie `401`; another owner `404`; expired/revoked credential denied before read/replay; no public/static or presigned access |
| Restart | Stop **only** the run-owned foreground Server after exact process ownership is recorded; reopen the same Turso file/topology and read/replay the same immutable result; do not restart an installed service |
| Historical target | Preserve a filesystem receipt/root, change only active writes to accepted S3, verify old target/key/receipt bytes/digest unchanged and no copy; omitting old target returns `503 unavailable` without fallback |

Do not mutate accepted rows or corrupt/delete live provider objects to obtain these checks.
Use the existing local tests for destructive fault injection. Retain database/WAL, media and
private reports for the approved owner; no automatic remote cleanup, migration or activation.
A completed provider harness alone cannot accept this Server/Turso restart layer.

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

## Historical verification ledger (original profile change)

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
   Either widens the trust surface and changes the dependency set that the storage ADR (now ADR 0037) pinned, so it
   needs an owner decision and its own change. Until then, run acceptance from a network that
   does not re-sign TLS.
2. **Signing region** and whether Long Vân's release honors `If-None-Match: *`: only Long Vân
   or the live run can answer. Do not set `conditional_put = true` on an active write target
   whose `conditional_create_enforced` is not `PASS`.
3. **Bucket versioning**: needed before `presigned_reads = true` is meaningful.
4. **Production isolation**: a private bucket for intake only. Publication/CDN delivery needs its
   own reviewed storage ([ADR 0037 §8](../decisions/0037-provider-independent-private-storage.md)).

## Deferred acceptance

**Deferred security debt — owner decision, 26 October 2026.** Prioritize business work;
further Long Vân privacy investigation is deferred. The [historical exact unsigned URL/cache
exposure](#offline-harness-hardening-and-live-blocker) is **not remediated by this decision**;
current provider state remains UNKNOWN. Keep filesystem-first and existing storage guards;
this note grants no S3 activation, credential, provider-policy or deployment authority.
Future owner: storage operator/provider, individual unassigned. Resume only with an assigned
owner, the exact owned synthetic probe metadata and separately approved secure operator scope.
Audit bucket/object policy and gateway cache authorization, then require fresh unsigned
exact-URL denial after signed cache warm, plus no-cache/query/listing checks with no private
bytes returned. A query-variant `403` cannot override exact-URL exposure. Record sanitized
results in this evidence record before considering the remaining provider/intake gates.

No individual assignee was established by this task; role owners below must assign the
campaign before acceptance. A closed issue, stored credential or local PASS is not a resume gate.

| Gap / status | Owner | Impact | Prerequisite and resume condition |
| --- | --- | --- | --- |
| Privacy remediation/recheck, NOT_RUN (historical FAIL retained) | Storage operator/provider | Private bytes may remain anonymously retrievable; blocks all activation | Separate provider/cache remediation approval and fresh exact-URL denial after signed cache warm, no-cache/query/listing checks; gate 1 evidence accepted |
| S3 custody/access, import/IAM/runtime binding, NOT_RUN; present state UNKNOWN | Secret-custody operator and intended consumer reader (#446) | No usable scoped/pinned runtime credential established | Exact approved secret/version/reader and secure local route; separately record imported, verified access and binding; no credentials in chat |
| Full provider harness, NOT_RUN | Storage operator | Conditional create, body envelope, byte reads/pins/Range and privacy still unaccepted for this target | Gates 1–2; disposable bucket, confirmed region/TLS, authorized probe budget and retention; gate 3 command with all required rows PASS, deliberate cleanup hold recorded |
| Isolated Server/Turso/S3 intake and real S3 restart, NOT_RUN | Native Server acceptance operator | Provider-only PASS cannot establish durable application receipts or reopen/read authorization | Gates 1–3 and separate intake/restart grant; isolated files/process and approved samples; gate 4 per-sample byte/receipt/Range/replay/reopen/denial matrix passes |
| Publication S3, media migration, CDN, VPS deployment/activation, NOT_RUN | Respective publication/storage/deployment owners; outside this review | No public delivery, automatic copy or production readiness claim | Separate scoped work and owner authorization; preserve filesystem-first and historical targets; deployment STOP remains in force |
| CORS/direct grants, playback, throughput/cost and provider durability/SLA, NOT_RUN | Respective Studio/Theatre/storage owners | Unmeasured independent capabilities | Separate consumers, representative inputs and authorized campaigns; no inference from API-stream or probe results |

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
