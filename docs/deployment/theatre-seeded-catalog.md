# Backend-owned synthetic Theatre catalog

Status: **implemented tooling**, with independent local PostgreSQL/storage acceptance recorded
in the current PR. Remote environments require explicit operator authorization; implementing
this lane does not seed or deploy a VPS. Native playback remains a separate acceptance step.

## Seed ownership and publication

`cargo xtask seed theatre` uses the retained disposable cluster. An explicit `--environment`
lane instead reconciles one dataset in an **existing** epoch-2 database through an existing
Studio actor/session and explicit API/Web origins. Persistent mutations, upload, session
validation and acceptance reads go to that running API over bounded HTTP, so the configured
server's OIDC/session policy remains authoritative. Both run `theatre_development_catalog_seed`: real Studio HTTP routes,
app-role use cases, immutable content/brief revisions, actual Ogg/Opus intake, QC/rights/editorial
records, distribution clearance, exact approval, publication and withdrawal. Publication uses
the same filesystem promotion/observation and origin-delivery service as the real server.
`VerifiedDeliveryFake` is absent from this seed path. No seed SQL inserts catalog/publication
rows around the domain gates. Persistent mode performs no migration, grants, account/session
creation, reset, truncate or database drop.

The dataset `theatre-catalog-v2` has the following fixed Work/unit identities. They are owned
only when the selected actor and private receipt/journal establish ownership; an existing ID
without that journal is a collision and is refused.

| Work ID | Kind | Title | Public result |
| --- | --- | --- | --- |
| `5eed0001-0000-4000-8000-000000000000` | novel | Mùa gió chướng | 4 chapters |
| `5eed0002-0000-4000-8000-000000000000` | story | Chiếc đèn lồng giấy | 2 of 3 chapters; the second is withdrawn, so positions are 0 and 2 |
| `5eed0003-0000-4000-8000-000000000000` | podcast | Chuyện đêm khuya | 23 episodes: a default page of 20 plus a next page of 3 |
| `5eed0004-0000-4000-8000-000000000000` | story | The Lantern Keeper's Notes | 2 chapters, `en-US` |
| `5eed0005-0000-4000-8000-000000000000` | podcast | Tín hiệu đã tắt | published then withdrawn: `not_found` |
| `5eed0006-0000-4000-8000-000000000000` | novel | Bản thảo riêng tư | private Work with a release: `not_found` |
| `5eed0007-0000-4000-8000-000000000000` | story | Truyện chưa phát hành | approved, never published: `not_found` |

Unit IDs are `5eed<work>-<unit>-4000-8000-000000000000` with one-based four-digit numbers, for
example `5eed0003-0021-4000-8000-000000000000` for the 21st episode. Work and unit IDs are the
same after every reseed. `publication_id` values are minted by the publication use case, so a
disposable reseed creates new releases with new IDs; persistent replay preserves IDs. The first
novel chapter has a 125-second synthetic sine tone for byte-range/seek/resume work;
other fixtures are 2–5 seconds. These locally authored signals contain no voice, third-party
music or downloaded media. They are generated outside the API by this explicit developer
seed, inspected on intake and copied into separate publication storage. No generated audio
belongs in Git. The rights window ends 30 days after the initial seed; expiry in persistent
mode requires an explicit reviewed dataset revision, never a reset or silent renewal.

## Run it

Prerequisites: Rust 1.98.1, Python 3, PostgreSQL server tools (`initdb`, `pg_ctl`, `psql`) on
`PATH`, `ffmpeg`/`ffprobe`, and Trunk for Web. Nothing here installs tools, starts Podman or uses
cloud credentials, Firebase, Kanidm, Gemini, Forge or S3.

```bash
cargo xtask verify postgres --keep
```

```bash
cargo xtask seed theatre
```

```bash
cargo xtask run cantos-theatre --web --test-cluster
```

The API listens on `127.0.0.1:8080` and Trunk on `127.0.0.1:8082`. Both origins must return
the same page:

```bash
curl -s 'http://127.0.0.1:8080/api/v1/theatre/works?limit=20'
```

```bash
curl -s 'http://127.0.0.1:8082/api/v1/theatre/works?limit=20'
```

Open `http://127.0.0.1:8082/`, choose a Work and use **Tải thêm** on _Chuyện đêm khuya_.
`?mode=mock` and `?mode=membership-preview` remain separate offline previews; live mode never
falls back to them.

To **hear** a unit the browser must be able to open the HTTPS grant URL the API returns. Start the
[local TLS front end](#local-tls-front-end-for-browser-playback) in a third terminal and accept its
certificate in the browser you use; without it the catalog still works and the player reports a
connection failure rather than a simulated success:

```bash
python3 scripts/serve_studio.py tls-front --generate
```

### Independently retained ownership after a narrow check

When the full PostgreSQL campaign is unavailable or has failed, the explicit narrow command
can retain its own **new** disposable cluster:

```bash
cargo xtask verify postgres --suite schema --keep
```

This runs the same schema/SQL tests as the non-retained narrow suite. It does not run the full
media/restart campaign. The runner writes `target/revision-evidence/retained-cluster.json`
only after it initializes, starts and bootstraps that exact cluster. This receipt contains
ownership, scope and separate `gate_status` / `full_gate_status`; it contains no fixture URLs.
The final scoped status can be `PASS` or `FAIL`; for a narrow run the full status is always
`NOT_RUN`. An earlier full-gate `FAIL` remains a failed result in its own evidence. Retaining
resources does not accept or bypass any required gate.

After the runner exits, `seed theatre` may consume this ownership proof and create a **new**
seed database, independent of the check's fixture databases. It refuses an active runner PID,
a scope/path mismatch, a stopped cluster or a port served by another data directory. The seed
receipt and output preserve the originating statuses. The full success-only `run.json` keeps
its original meaning and is never synthesized from ownership.

With the seed complete, only the Theatre context can serve it without the full fixture record:

```bash
CANTOS_TEST_CONTEXT=theatre cargo xtask run cantos-api --test-cluster \
  --web-origin http://127.0.0.1:8082
```

The same matching seed supports `run cantos-theatre --web --test-cluster`. Unseeded Theatre
and editorial/catalog/audio contexts still require the original complete fixture record.
Ownership-only startup confirms the real PostgreSQL `data_directory` again before serving.
No receipt is retroactively created for an older failed cluster.

### Production CMP hosts

Follow the [mobile host guide](../../apps/cantos-theatre/mobile/README.md) for the production
Android/iOS composition and explicit origin. Android debug can use loopback with `adb reverse`:

```bash
cargo xtask theatre-host android-debug --api-origin http://127.0.0.1:8080
adb reverse tcp:8080 tcp:8080
```

The production catalog uses the returned UUIDs directly and does not import preview-sandbox.
Catalog builds are separate from native playback/device acceptance.

### Observe a change

The seed extends the synthetic Studio session of `alice` (token: 64 lowercase `a`) for the
rights window, so a withdrawal can be made through the real Studio route of the running API.
Read the release ID and pointer version of a unit from `theatre-seed.json`, then:

```bash
curl -sS -X POST "http://127.0.0.1:8080/api/v1/studio/content-units/5eed0003-0023-4000-8000-000000000000/publication/withdraw" -H 'Origin: http://127.0.0.1:8082' -H 'Content-Type: application/json' -H 'Accept: application/json' -b "cantos_session=$(printf 'a%.0s' $(seq 64))" -d '{"operation_id":"'"$(python3 -c 'import uuid;print(uuid.uuid4())')"'","release_id":"RELEASE_ID","expected_pointer_version":1,"reason":"Local seed change"}'
```

Refresh the Web detail or the Android list: episode 23 disappears and the page ends after 22.
The seed receipt records `private_root`, `publication_root`, `playback_key_file` and the origin the
seed itself minted grants for. The local test-cluster host admits the storage paths only beneath its
exact retained cluster and composes the real publication/origin adapter. The API itself speaks plain
HTTP on `127.0.0.1:8080` yet its admission accepts only an `https://host[:port]` media origin, so a
grant URL is never one a browser can open unless something terminates TLS for it. A configured
persistent host uses its operator-managed HTTPS origin and key. Persistent acceptance fetches the
exact advertised HTTPS grant URL; it never substitutes the configured API origin or downgrades TLS.

### Local TLS front end for browser playback

`serve_studio.py` advertises `https://localhost:8443` as the media origin (override with
`CANTOS_LOCAL_MEDIA_ORIGIN=https://localhost:<port>`; it must be an exact loopback HTTPS origin and
never the API's own port 8080). `tls-front` is the loopback TLS terminator for exactly that origin:
it forwards `/api/` to the API byte for byte, including `Range` requests and `206` answers, serves
nothing else and never logs a media grant.

```bash
python3 scripts/serve_studio.py tls-front --generate
```

`--generate` mints a 30-day self-signed certificate for `localhost` and `127.0.0.1` under the
ignored `target/local-tls/` (or set `CANTOS_LOCAL_TLS_CERT` and `CANTOS_LOCAL_TLS_KEY`). The script
edits no trust store: a browser follows the grant only after **you** accept or trust that
certificate for `https://localhost:8443`; otherwise the player reports a connection failure instead
of playing. Verify the whole chain with the exact advertised URL and the certificate checked:

```bash
curl --cacert target/local-tls/localhost.pem -H 'Range: bytes=0-3' "$GRANT_URL"
```

This proves manifest, authorization, TLS and `206` mechanics on loopback. It is not a deployed
proxy, CDN, CORS or native playback result.

A disposable reseed restores the dataset. Persistent replay deliberately refuses changed
metadata or pointers rather than overwriting an operator edit.

### Reset

```bash
cargo xtask seed theatre --reset
```

It drops only the database named by the seed record for the same retained cluster and removes
the record. A complete matching full-run record can serve its empty fixture again; an
ownership-only environment requires a new seed before Theatre startup.

## Safety rules the primitive enforces

- It never reads `DATABASE_URL` or inherited `PG*`/`CANTOS_*` variables, and builds its admin URL
  only from the retained record's loopback port.
- The record must say `retained: true` and name a `cluster-<number>` with `pgdata/PG_VERSION`
  under the exact full or named narrow evidence directory. An ownership receipt additionally
  requires consistent scope/status, the current OS owner and an inactive runner. A legacy
  full-run record must include its Theatre fixture. Unknown targets are refused.
- Before any write it asks the server for `SHOW data_directory` and refuses unless it is that
  retained `pgdata`. A different PostgreSQL on the same port is never touched.
- It drops only `cantos_test_` plus 32 hex databases named by a seed record for the same cluster
  and port, never a verification fixture named by the run record. The drop uses
  `WITH (FORCE)`, so stop `run … --test-cluster` before reseeding or resetting; a host still
  serving the old seed database loses its connections and must be restarted anyway.

## Troubleshooting

| Symptom | Cause and action |
| --- | --- |
| `seed theatre needs the disposable cluster retained by …` | No valid retained ownership/full-run record; run `cargo xtask verify postgres --keep` or the explicit narrow ownership lane above |
| `REFUSED: retained cluster … is not reachable` | The cluster was stopped or the machine restarted; run `verify postgres --keep` again, then seed |
| `REFUSED: … served by another PostgreSQL data directory` | The port belongs to another server; nothing was changed |
| `UNSUPPORTED … missing psql` | Install the PostgreSQL client and server tools yourself; xtask never installs them |
| `run` says `127.0.0.1:8080 … is already in use` | Another process owns the port; stop it yourself or use `app cantos-theatre --web` against an API you own |
| Seed/ownership mismatch at startup | Records name different clusters; seed the selected retained cluster. Startup refuses to fall back to an old empty fixture |
| Empty Home after seeding | The API was started before seeding; restart `run … --test-cluster` and check its first line |
| Catalog became empty after a month | The seeded rights window expired; seed again |
| `rendition_unavailable` | The served host has no matching publication root/key/origin, bytes are missing/corrupt, or public eligibility changed; check the receipt and configured real media adapter |

## Named persistent environment

Run this only for a specifically authorized environment. The operator supplies an existing
loopback/socket app-role URL (or a separately established local tunnel), a valid Studio session
and a registered synthetic rights evidence ID owned by that actor. The named lane defaults to
`__Secure-cantos_studio`, matching secure [Studio OIDC session naming](../../services/cantos-api/src/platform/studio_oidc/mod.rs).
Use `--studio-cookie-name cantos_session` only when the selected server explicitly uses the
insecure development spelling. These are the only admitted names; the tool sends exactly one
configured cookie and never retries with another spelling after an authentication failure.
The explicit API origin must serve that same database and its real media composition. Before any mutation the tool
compares the API-authenticated actor and canonical `x-cantos-studio-session` fence with the
selected database's exact token-hash session row; missing/legacy/mismatched fences refuse the
run. All later authenticated Studio requests carry that fence. Anonymous Theatre/media
requests omit Studio cookies, fences and Origin headers. Producer uploads send only their
upload bearer plus the exact Studio Origin required by the current audio PUT guard; they
never carry a Studio cookie or session fence. The Web origin must match the
Studio origin fence. HTTPS is required except for explicitly named loopback development
origins. The command neither creates credentials nor bypasses rights registration. Roots must already exist, be separate
private directories, and match the real server's configuration. Keep the receipt/recovery
journal outside Git or under ignored `artifacts/`; its journal includes producer upload
capabilities and must remain private.

```bash
cargo xtask seed theatre --environment catalog-staging \
  --api-origin https://api.example.org --web-origin https://studio.example.org \
  --database-url-env CATALOG_SEED_DATABASE_URL \
  --studio-token-env CATALOG_SEED_STUDIO_TOKEN --studio-cookie-name __Secure-cantos_studio \
  --rights-ref synthetic-signal-rights \
  --private-root /srv/cantos/private-audio \
  --publication-root /srv/cantos/publication-audio \
  --record /srv/cantos/seed/theatre.json
```

Credential **variable names** enter argv; their values never enter a receipt or command
output. No inherited `DATABASE_URL`, `PG*` or `CANTOS_*` setting chooses the destination.
The receipt lists Work, Unit, Publication, Rendition, SHA-256 and byte length for handoff.

A dataset advisory lock serializes seed writers in the database. The private fsynced journal
binds dataset/source fingerprint, environment, connection endpoint/user, database name/OID,
actor, API/Web origins, rights reference and both roots. Each run first proves its current
Studio session against that same DB/API; validated session fences are private audit history.
A new valid session for the same actor preserves the dataset and original operation IDs, while
a different actor is refused. The journal records each exact operation and payload **before**
the HTTP effect, then records successful responses. A lost response replays the same
idempotency operation; a completed response is reused, followed by fresh Studio/public API
reconciliation of metadata, revisions, publication pointers, manifests and real bytes. Repeated
runs produce no duplicate Works/releases. Other datasets are never updated or deleted.

Keep the receipt **and** `.journal.json` together. On failure, rerun the exact command with
the same inputs; do not delete the journal. A collision, source/target mismatch, expired rights,
missing fixture, stale publication or operator metadata edit is a refusal requiring explicit
reconciliation. There is no persistent `--reset`. A failed run can have completed earlier
immutable operations; the journal is the recovery authority. The receipt is written atomically
only after the whole acceptance walk succeeds.

## Not established by this tooling

Native playback, progress/sync, offline downloads, identity login, a deployed TLS/CDN path,
real manuscript rights, human listening or provider behavior. Synthetic QC/approval records
are explicitly scoped to these owned signals and are not human-listened quality evidence.
