# Monorepo restructure evidence

Scope: moving the existing code into [the new layout](../architecture/repository-layout.md) under
[decision 0009](../decisions/0009-monorepo-layout.md), plus a Theatre Web skeleton and README-only
guides. No feature, route, schema, migration or wire change is intended. Checks ran on Linux with
Rust 1.87.0 (the repository pin), Trunk 0.21.14 and Python 3, on the branch before the PR.

## What changed

- 162 files moved with `git mv`; `git diff -M` shows the 56 migration, schema, fixture and example
  files, and the token source, as 100% renames (byte-identical). Rust sources changed only in
  relative `include_*`/fixture paths and in the two package names below.
- Cargo packages: `cantos-server` → `cantos-api`, and the wire DTO crate `cantos-api` →
  `cantos-contracts`. Binary names are unchanged. `Cargo.lock` differs only by those renames and
  the new `cantos-theatre` entry; no dependency version moved.
- `apps/cantos/web` (`cantos-theatre`) is new. Kanidm, mobile and package READMEs are new.
- Paths and commands were updated in the README, `AGENTS.md`, `CONTRIBUTING.md`, CI, scripts,
  `.env.example`, `.gitignore`, current docs and the agent skills. CI gained a `web` job that lints
  both WASM packages with the commands run below.
- Evidence and decision records written before this change keep their original paths and package
  names on purpose; only their links were repaired. Raw result files under `docs/evidence/*.json`
  are untouched. `schema/adaptation/v1.schema.json` still names the old path in a `$comment`, left
  as is to keep contract bytes identical.

## Results

| Check | Result |
| --- | --- |
| `cargo +1.87.0 fmt --all -- --check` | PASS |
| `cargo +1.87.0 clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo +1.87.0 clippy -p cantos-studio -p cantos-theatre --target wasm32-unknown-unknown --locked -- -D warnings` | PASS |
| `cargo +1.87.0 test --workspace --locked --no-fail-fast` | PASS: 311 passed, 0 failed, 52 ignored (need PostgreSQL). Same totals as the same command before the move |
| PostgreSQL suites `revisions_postgres`, `imports_postgres`, `adaptations_postgres`, `production_postgres` with `--ignored --test-threads=1` | PASS: 12, 9, 21 and 9 tests, against a fresh `postgres:17-alpine` container with a restricted `cantos_app` role, the same setup the CI `postgres` job uses (`CANTOS_TEST_CLUSTER_URL`) |
| `trunk build --locked` in `apps/cantos-studio/web` and `apps/cantos/web` | PASS: both produce a `dist/` |
| Built `cantos-server` (default `CANTOS_WEB_DIST`) against a migrated disposable database | PASS: `/` serves the Studio bundle (200 `text/html`), session login 200 with an HttpOnly cookie, unauthenticated API 401, authenticated validation of the Script IR fixture 200 with no issues |
| Studio in the built-in browser against that host | PASS for load and sign-in only: Vietnamese UI rendered, no console errors, sign-in reported success. Save, import, adaptation and casting flows were **not** exercised in a browser |
| Theatre skeleton in the built-in browser | PASS: heading and the two "nothing exists yet" lines rendered, no console errors |
| `python3 scripts/check_repository.py`, `python3 -m unittest discover -s scripts -p 'test_*.py'` (80 tests), `git diff --check` | PASS |
| `python3 scripts/reference_script_ir.py`, `validate-script`, `diff-script`, `python3 scripts/reference_wav_inspection.py` | PASS; the fixture digest is unchanged |
| `python3 scripts/mutate_script_ir.py` | PASS: 9 killed, 0 survived (the runner now copies every workspace member) |
| `python3 scripts/studio_tokens.py --check` | PASS |
| `kanidmd configtest` on `services/kanidm/server.toml.example`, Kanidm 1.10.5 container, no network | PASS ("This config rocks!"); parse check only |

## Not run, and known limits

- **`scripts/test_postgres.py`, the benchmark harness and `production_oracle.py` are NOT_RUN:**
  `initdb`, `pg_ctl` and `psql` are not on `PATH` here. The same Rust suites ran against a container
  cluster instead. That does not exercise the Python runner's restart journey or its path edits
  (`scripts/serve_studio.py`, `ROOT / "target/debug/cantos-server"`), which are unchanged by name.
- GitHub Actions was not run. The new `web` job and the renamed `-p cantos-api` commands in the
  `postgres` job were executed locally as shown. The `postgres` job still omits `production_postgres`,
  as before.
- No Android or iOS build exists. Both mobile directories are guides; nothing about Compose,
  playback, downloads or lifecycle is built or claimed.
- The Theatre Web crate is a skeleton, and Kanidm is configuration only. No listener API, token
  set, OIDC integration or Kanidm deployment exists. A fresh Kanidm data directory logged a SQLite
  `no such table: id2entry` line before success; it was not investigated.
- No accessibility, viewport, text-scaling or visual-regression evidence was collected; no UI
  changed other than the new placeholder page.
- `tts_local` test `process::version_and_available_voice_are_required_before_synthesis` failed once
  in the first pre-change run (`ProcessStartFailed { phase: Version }` instead of
  `WrongRuntimeVersion`) and passed on every other run, before and after the move. Its cause was not
  investigated, so treat it as an unexplained pre-existing flake.
- `trunk serve` with its development proxy was not used.
