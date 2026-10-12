# API surface split and Theatre rename evidence

Scope: [decision 0015](../decisions/0015-api-surface-boundaries.md). Proposed design and implemented
behavior are separated below; results are filled from the local run recorded at the end.

## Implemented

- `apps/cantos` → `apps/cantos-theatre` (`git mv`); package `cantos-theatre` unchanged. Cargo
  workspace, xtask (`cantos-theatre` target, `cantos` accepted as alias), scripts, skills and docs
  updated. Historical evidence and accepted decisions keep their original paths on purpose.
- `services/cantos-api/src/http.rs` keeps only shared state, middleware and error mapping. Studio
  handlers moved verbatim to `http/studio.rs` (+ `http/studio/{adaptations,imports,production}.rs`).
  `http/theatre.rs` and `http/admin.rs` are empty routers answering `404 not_found`.
- Routes: `/api/v1/studio/*` is canonical; the unprefixed `/api/v1/*` paths are mounted from the same
  `studio::routes` with `Deprecation: true`. Studio session cookies are scoped to the mount that
  issued them (`Path=/api/v1/studio`; the alias keeps `Path=/api/v1`).
- `cantos-contracts` gained `routes` (base paths) and empty documented `theatre`/`admin` modules.
  No DTO, error code, migration, persisted identifier or digest changed.
- Studio Web calls `/api/v1/studio/*`; Python harnesses and Rust integration tests use it too.
- Theatre Web `<title>` and heading say **Cantos**; tests guard against "Cantos Theatre".

## Tests added

| Claim | Test |
| --- | --- |
| Theatre/Admin serve nothing for any method, with or without a session | `surface_routes::reserved_theatre_and_admin_surfaces_serve_nothing_even_when_a_session_is_presented` |
| No route leaks across groups or double prefixes | `surface_routes::surfaces_do_not_leak_routes_into_each_other` |
| Studio requires authentication at both mounts with identical errors | `surface_routes::studio_routes_require_authentication_at_the_current_and_legacy_paths` |
| Legacy alias is flagged; current paths are not | `surface_routes::legacy_studio_paths_are_flagged_deprecated_and_current_ones_are_not` |
| Session endpoint exists for Studio only | `surface_routes::session_endpoint_exists_for_studio_only_and_shares_validation_with_the_legacy_alias` |
| Origin guard covers every group | `surface_routes::origin_guard_covers_every_surface_for_mutations` |
| Legacy write ↔ current read are one revision; cookie scopes; Studio session grants nothing on Theatre/Admin | `revisions_postgres::legacy_studio_paths_remain_compatible_and_sessions_stay_scoped_to_their_mount` (disposable PostgreSQL) |
| Listener title/heading never say "Cantos Theatre" | `cantos-theatre` `view::tests` |

The existing PostgreSQL, wire, import, adaptation and production suites now run through
`/api/v1/studio`, the canonical mount.

## Not covered

- Real authentication/authorization for Theatre and Admin: none exists, by design.
- No removal date for the legacy alias; no browser run of Studio against the new prefix beyond the
  Trunk build and WASM lint.
- Mobile surfaces remain `UNSUPPORTED` (no Gradle project); `cmp-ui-preview` needs Gradle.
- Tagline "Vọng Đài · Theatre of Voices" on the Theatre skeleton was left unchanged: it is a
  tagline, not the product name. Review whether it should also drop "Theatre".

## Local verification

macOS, stable Rust 1.97.1 (the pinned 1.87.0 is not installed), Python 3.14 (the default 3.9 lacks
`tomllib` and fails the same Python tests on the base branch). Remote CI was not awaited.

| Gate | Result |
| --- | --- |
| repository, rust-format, script-ir-reference, wav-reference, mutation | PASS |
| rust-tests (workspace), including the new `surface_routes` suite | PASS |
| web-lint, web-build (cantos-studio, cantos-theatre), ui-preview | PASS |
| postgres (`scripts/test_postgres.py`, run with `LC_ALL=en_US.UTF-8`; includes the legacy-compat test) | PASS |
| rust-lint (workspace) | NOT_RUN: clippy 1.97 reports `cloned_ref_to_slice_refs` in untouched `tests/wav_inspection.rs`; the touched crates showed no other finding |
| cmp-ui-preview | NOT_RUN (`--skip`; Gradle unavailable); mobile surfaces UNSUPPORTED |
