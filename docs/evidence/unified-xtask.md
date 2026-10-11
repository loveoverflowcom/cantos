# Evidence: cargo xtask command surface

Change: `xtask/` workspace member, `.cargo/config.toml` alias, the CI workflow rewritten to call
`cargo xtask ci --job <job>`, and `scripts/mutate_script_ir.py` copying the new member into its
disposable workspace. Decision: [ADR 0012](../decisions/0012-xtask-command-surface.md). Contract:
[`xtask/README.md`](../../xtask/README.md).

Environment: Linux x86_64, base `develop` at `dc7c2de`, uncommitted working tree of this change.
Rust 1.87.0 selected with `RUSTUP_TOOLCHAIN=1.87.0` unless stated (default toolchain 1.96.1),
Trunk 0.21.14, Python 3.14.4, Git 2.53.0. PostgreSQL server binaries are not installed on this
machine.

## Verification commands

| Command | Result |
| --- | --- |
| `cargo test -p xtask` (1.96.1, repeated after each edit) | PASS: 60 tests: CLI parsing, build/run/app plans, gate commands, CI jobs, preflight selection, Git change collection, Cargo metadata graph, executor, HTTP/TCP probes and real process-group lifecycle |
| `cargo xtask ci --job repository` | PASS: repository (272 text files, 15 skills, 103 Python tests, diff checks) and script-ir-reference |
| `cargo xtask ci --job rust` | PASS: rust-format, rust-lint (workspace, includes xtask), rust-tests (workspace: 432 passed, 0 failed, 54 ignored PostgreSQL/live tests), wav-reference |
| `cargo xtask ci --job web` | PASS: web-lint for both WASM crates; web-build produced `dist/` for Studio and Theatre |
| `cargo xtask verify contracts` | PASS: script-ir-reference, contract-tests (130 passed), wav-reference |
| `cargo xtask verify mutation` | PASS: baseline 15 tests in the disposable copy, which now includes `xtask`; 9 of 9 mutants killed, 0 survived, timed out or unviable |
| `cargo xtask build all` | PASS: `cantos-api` binaries and both Web clients (dev); notes that mobile and Kanidm are not built |
| `cargo xtask build cantos --web --release` | PASS: `trunk build --locked --release` |
| `cargo xtask verify postgres` | UNSUPPORTED here: missing `initdb`, `pg_ctl`, `psql`; exit 1 and nothing ran |
| `cargo xtask doctor` from `apps/cantos/web` | PASS: root discovered from a subdirectory; Rust 1.87 note shown under 1.96.1; PostgreSQL tools, the postgres gate and both mobile surfaces reported UNSUPPORTED |
| `cargo +1.97.1 xtask doctor` | reported `rustc 1.97.1`: a `+toolchain` override reaches the commands xtask starts |
| `cargo xtask preflight --changed-only --dry-run` | 45 changed paths; broadened to every gate because `.cargo/config.toml` (verification tooling) changed |
| `preflight --changed-only --base HEAD --dry-run` after a throwaway edit to `docs/product/brief.md` | selected only repository and rust-format (edit reverted) |
| the same after a throwaway edit to `packages/contracts/api/src/lib.rs` | selected rust-lint/rust-tests for cantos-api, cantos-contracts and cantos-studio, wav-reference, Studio web-lint/web-build and postgres; not Theatre (edit reverted) |
| `cargo xtask preflight --changed-only --base no-such-ref` | error before any gate: base ref does not exist |

`verify repository` was also observed failing (`FAIL`, exit 1) while this record did not exist yet,
because two documents linked to it.

## Refusals

| Command | Result |
| --- | --- |
| `build cantos-studio --mobile` | UNSUPPORTED (no Gradle project), exit 1 |
| `build cantos` (no surface) | usage error naming `--web`/`--mobile`, exit 2 |
| `run cantos-api` without `DATABASE_URL` | refused before building, exit 1 |
| `run cantos-studio --web --test-cluster` without a retained cluster | refused: `target/revision-evidence/run.json` missing, exit 1 |
| `app cantos-studio --web` with no API on 8080 | refused with the command to start it, exit 1 |
| `run cantos-api --web-origin https://evil.example` | usage error, exit 2 |
| `run cantos --web` while a separate `python3 -m http.server` held 127.0.0.1:8082 | refused, exit 1; the foreign process was still alive afterwards |

## Process lifecycle

- `run cantos --web`: `trunk serve` became ready after 51 s; `GET /` returned
  `<title>Cantos Theatre</title>`. SIGINT to xtask stopped its process group, exit 0; port 8082 had
  no listener and no `trunk` process remained.
- `run cantos-studio --web` with a synthetic `DATABASE_URL` pointing at closed port 9 (the host
  creates its pool lazily, so it starts without a database): `cantos-server` became ready on 8080
  with `CANTOS_WEB_ORIGIN=http://127.0.0.1:8081`, then `trunk serve` on 8081 after compiling the
  WASM bundle. `GET /` returned `<title>Cantos Studio</title>`; `/api/v1/scripts/x/head` through
  the Trunk proxy returned the API's 401. Killing `cantos-server` with SIGKILL made xtask stop
  Trunk and exit 1 naming the exited process; ports 8080 and 8081 were free and no child remained.

These sessions verify orchestration only. No database, sign-in, Studio interaction or browser was
exercised, so they are not Studio runtime evidence.

## Not run

- `verify postgres` and `ci --job postgres`: UNSUPPORTED locally. The GitHub `postgres` job now
  installs PostgreSQL 17 and runs `scripts/test_postgres.py` (all four database suites plus the
  HTTP kill/restart checks) for the first time in CI; its result is only known from that run.
- The GitHub Actions workflow itself, including `cargo install trunk` and the PGDG install step,
  had not run when this record was written.
- `run cantos-api --test-cluster`, `run` against a real database, `app cantos-studio --web` against
  a live API, `--open`, `run --release` and `build cantos-api --release` (only the plans are
  unit-tested).
- macOS, and any shell other than bash for the `sh -c` process tests.
- Branch protection and required-check configuration are outside the repository and unchanged.
