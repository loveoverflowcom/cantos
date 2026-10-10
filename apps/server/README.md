# Cantos script revision backend

Implemented in one Rust/Axum package: the pure Script IR reader and `validate-script` CLI,
consumed by [the contract corpus](../../contracts/README.md), plus authenticated Script IR
creation, append-only saves, optimistic concurrency, immutable acceptance, source/rights
references and actor history in PostgreSQL. This is part of
[issue #1](https://github.com/loveoverflowcom/cantos/issues/1). The Leptos consumer, importers,
AI adaptation, production and publication are future slices. One root Cargo workspace keeps
modules in this package; no domain crate or separate service has been introduced.

## Script IR CLI

```sh
cargo run --locked --bin validate-script -- contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json
```

`src/script_ir/wire.rs` owns version dispatch and raw shape decoding. `validation.rs` admits
immutable domain values defined in `model.rs`; `canonical.rs` encodes content and speech without
I/O. The CLI performs only file reads/output after validation. It does not save, authorize or
mark a script production-ready. [Evidence](../../docs/evidence/script-ir-contract.md) separates
these local checks from the remaining application gates.

## Local development

Requirements: the pinned Rust toolchain in [rust-toolchain.toml](../../rust-toolchain.toml),
PostgreSQL 17, and optionally Podman or Docker for the disposable database below. Substitute
`docker` for `podman` if needed. This password is exclusively a local test credential.

```sh
podman run -d --name cantos-dev-postgres \
  -e POSTGRES_PASSWORD=cantos_local_only -e POSTGRES_DB=cantos \
  -p 127.0.0.1:55432:5432 docker.io/library/postgres:17-alpine
podman exec cantos-dev-postgres pg_isready -U postgres -d cantos
export DATABASE_URL=postgresql://postgres:cantos_local_only@localhost:55432/cantos
export CANTOS_BIND_ADDRESS=127.0.0.1:8080
cargo run -p cantos-server -- migrate
cargo run -p cantos-server -- create-creator
```

Copy the returned UUID, then issue a token valid for one hour. The token command prints the
credential **once**; keep it in a local secret store and out of Git, screenshots and logs.

```sh
cargo run -p cantos-server -- issue-token CREATOR_UUID 1
cargo run -p cantos-server -- serve
```

`serve` also applies checksummed migrations before binding. `GET /healthz` checks PostgreSQL
readiness. Environment is read directly; the binary does not auto-load `.env`. Credentials are
never accepted as query parameters, cookies or creator IDs in a request. Use TLS before exposing
this beyond loopback. Provisioning commands are local operator tools, not public HTTP routes.
To rotate, issue another token; `revoke-tokens CREATOR_UUID` revokes every existing token for that
creator. There is no account registration or browser login in this slice.

## Use the API

The [HTTP contract](../../contracts/http/script-api.md) describes requests, responses, errors,
retry behavior and pagination. Its JSON Schema is checked against committed fixtures **and real
HTTP responses** in integration tests. Start with the original Vietnamese
[Script IR fixture](../../contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json). The complete
[create-request fixture](../../contracts/http/fixtures/create-request.json) can be sent with
`Content-Type: application/json` and `Authorization: Bearer <token>` to `POST /v1/scripts`.
Replace its operation UUID for a new creation; keep the same UUID when retrying the same write.

- Read the returned script at `GET /v1/scripts/{id}`.
- Save at `POST /v1/scripts/{id}/versions`, with `base_version` equal to the last observed head.
- Accept an exact saved version at `POST /v1/scripts/{id}/revisions`.
- Read immutable versions/revisions or bounded history through the documented routes.

A `409 conflict` means reload and resolve against the reported current head. Never automatically
retry with a changed base while silently discarding another edit. A lost response can safely
retry its original operation. A replay returns its original version, which may be older than
the current head; reread the script for current editor state.

Source text is preserved exactly as UTF-8 text; Script IR text is normalized under the versioned
contract. Rights evidence remains `pending_review` and does not authorize generation or release.
Source/rights corrections currently require a new script; a dedicated revision workflow is
future work. All saved versions are valid Script IR; incomplete editor drafts remain a future
client concern. Accepted revision IDs pin saved content; no production worker is claimed.

## Verification

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
export TEST_DATABASE_URL=postgresql://postgres:cantos_local_only@localhost:55432/cantos
cargo test -p cantos-server --test persistence --locked -- --ignored --test-threads=1
python3 scripts/check_repository.py
python3 -m unittest discover -s scripts -p 'test_*.py'
git diff --check
```

The PostgreSQL suite is explicitly ignored in the default test run. The explicit command and CI
job run it; missing database configuration fails rather than passes. The test role needs CREATEDB
and access only to a disposable PostgreSQL instance. Each test creates an isolated database and
drops it on success; a failed assertion retains that database for diagnosis. The restart test
kills/reaps and restarts the actual server executable, then compares stored history and digests.

See [the evidence ledger](../../docs/evidence/script-revisions.md) for named coverage and limits.
Migration SQL is embedded from [migrations](migrations/0001_script_revisions.sql); never edit an
applied migration. The first migration has no prior deployed schema. Back up data before later
migrations; there is no destructive down migration. Triggers protect settled records, but a
privileged database owner can deliberately bypass them. Split migration/runtime database roles
before shared deployment; the local slice uses one operator-owned role.

## Boundaries and decisions

Pure content reading/validation/digests live in `script_ir`, pure save/accept decisions in
`editorial`,
credential lookup in `auth`, transactions in `store`, and extraction/status mapping in `http`.
There is no fake database or provider. Decision records:
[Script IR](../../docs/decisions/0002-script-ir-contract.md),
[authentication](../../docs/decisions/0003-creator-authentication.md),
[persistence](../../docs/decisions/0004-editorial-persistence.md).
