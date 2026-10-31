# Private Web source teardown repair (#676)

Implementation and bounded local evidence, 31 October 2026. Base:
`develop@7875453d774c67acec053da2521e190c33a71867`. This repairs the teardown panic recorded by
[#654](https://github.com/loveoverflowcom/cantos/issues/654) / PR #678 and indexed by the
[#676 review ledger](https://github.com/loveoverflowcom/cantos/issues/676). It does not enable
restricted Web playback: the Shared listen controls and their imperative entrypoint stay closed,
and Server restricted serving stays default OFF.

## Failure and cause

The #654 campaign recorded a panic, without a location, when an admitted private source lost its
session. This run reproduced it with the explicit, ignored #654 browser host
(`web_sharing_fixture::web_sharing_browser_host`): real Identity process and private RPC,
separate Turso files, production cookie/CORS/private routes and actual supplied Opus bytes. For
the campaign only, the WASM build reopened the Shared listen controls, the host added one actor
grant for the reader, and a temporary panic hook printed messages. None of these is committed.

After an actual credentialed `206` private media response and confirmed playback, three
session-loss cases ran: logout from Account, cookies cleared then a seek (the media request is
refused, so the player's denial path clears access), and cookies cleared then the Shared route
reopened.

| Case | Before the repair | After the repair (two runs each) |
| --- | --- | --- |
| Logout from Account | no panic; source released | no panic; source released |
| Cookies cleared, seek | `public_work.rs` row read a disposed row `Memo`; shared `Button` read a disposed caller `Signal` | no panic; source released (`src` cleared) |
| Cookies cleared, Shared route reopened | `shared_access.rs` read a disposed `admitted` signal | no panic; **audio keeps playing** (below) |

The cause is a lifetime gap in the pinned `reactive_graph` 0.1.8. A render effect has no `Drop` and
keeps running when its root sources change, including after its component owner has been cleaned.
Session loss changes root state (player, listen, Work detail, account and Shared access) in the
same tick that unmounts the Work rows, Shared Library or Shared detail. Their queued renders then
read `Memo`, `Signal::derive` or `StoredValue` values allocated in the cleaned owner. This is the
same class as the [EdgeProgress repair](theatre-navigation-runtime.md).

## Repair

Following that precedent, each affected view owns what its queued renders read:

- The shared Leptos `Button` converts its reactive props to `ArcSignal` when it is built, as
  `EdgeProgress` already does. Its public API is unchanged.
- The Work detail shape, Ready Work membership and hero, each unit row's projections and the
  Save control use `ArcMemo`/`ArcSignal` or plain owned values instead of owner-arena values.
  Row actions move into a small `UnitActions` component that receives row-owned inputs.
- The Shared Library and Shared detail admission checks read root signals directly.

No panic is swallowed and no `try_get` fallback is added. Wire contracts, authorization, the
player reducer, the media driver, CSS, tokens and CMP are unchanged.

## Legacy VPS fixture lock dependency

The #658 Python run kept one failure on macOS: `test_vps_oidc_reconcile` called the host
`flock(1)` with no guard. Removing only `flock` from `PATH` on Linux reproduced exactly one
FAIL (`test_nested_operations_reuse_the_same_lock_descriptor`) and showed that
`test_pending_created_between_entry_guard_and_lock_refuses_stale_mutation` passed vacuously,
because its double still writes the pending marker after the missing command. Both real-lock
cases now use the same guard as `test_release_vps.HostPromotionTests`. A host without
`flock` reports them as skipped, never passed. No fake lock or installer was added.

## Independent checks

Linux 7.0 x86_64, Rust 1.98.1, Trunk 0.21.14, Google Chrome 156 headless over a local CDP
driver, Node 24.22.2. The host needed libdav1d pkg-config metadata. An already extracted
development package was selected through `PKG_CONFIG_PATH`; no system package changed. Full logs
and screenshots stay in the ignored `artifacts/agent-runs/676/`.

| Layer / claim | Result / evidence |
| --- | --- |
| Native regressions | Four Theatre and one shared UI test build the component in an owner, clean it, change a root input and render the held view. Before the fix each failed at the production class of read; mutation runs that restored one arena projection at a time failed at exactly that projection (Work shape, unit row, Save control, Shared Library, Shared detail) |
| Theatre library | PASS: 589 tests with `--features preview` (585 existing plus 4 new) |
| Shared UI library | PASS: 175 tests in `cantos-ui`, including the new Button case |
| Python fixture | PASS with host `flock`: 10 of 10 run. Without `flock`: 8 PASS, 2 skipped, 0 FAIL |
| Full Python suite | FAIL as on base: 1,337 tests, 1 failure, 24 errors, 18 skipped. All 25 (`test_release_native_forward` 9 and `test_release_native_v27_live` 15 data-inventory errors, one `test_localization` no-diff-base failure) reproduce identically on a clean base checkout; none touch this change |
| Actual browser renderer | PASS for the panic claim: 0 panics in 6 post-fix runs (3 cases × 2) against the real host. Synthetic 60 s mono Ogg/Opus, sha256 `4b919974de2c2a067c5c3acc91ef988e576e6876b4d9e327f0afa8b172c71510`. Local TLS on `https://localhost:654{2,3}` used a self-signed certificate that Chrome was told to ignore |
| Handoff preflight | `cargo xtask preflight --changed-only --base origin/develop`: **FAIL** at repository with 66 UI qualification findings, identical line-for-line to a clean base checkout; later gates NOT_RUN. No assurance obligation derived |
| Remaining selected gates | Same command with `--skip repository`: PASS rust-format, rust-lint and rust-tests (`cantos-server`, `cantos-studio`, `cantos-theatre`, `cantos-ui`), wav-reference, audio-inspection, web-lint, both web-builds and ui-preview; repository NOT_RUN. An unrelated untracked `services/cantos-server/schema/table-ownership.json`, read by no code, broadened the selection to the Server lanes |

This is real local process/cookie/HTTP/byte and headless-browser evidence. It is not trusted
HTTPS, deployed, Android/iOS, screen-reader or production evidence.

## Residual risk and follow-ups

- **Active source after a refused Shared read.** With cookies cleared and the Shared Work
  reread, the Server answers `404` and the page shows the denial, but the active private source
  keeps playing the bytes already fetched (about 6 s observed). The client did not yet know the
  session had ended, and no rule clears the player when the Shared detail for the current source's
  Work is refused. That is a behavior change for the viewer-access/playback owners and the
  behavior SSOT impact procedure, not part of this repair. It must be settled before Web listen is
  enabled.
- **Focus handoff on removed rows.** `focus_before_unit_actions_removed` reads two row `NodeRef`
  values, which are also owner-arena values. While shared rows never offer actions, it is not
  reachable from the teardown path. Re-check it when the lock is lifted.
- Other views were not audited beyond the observed path. `PublicCover` and `loading_presence`
  keep owner-local signals that only their own DOM events write.
- HTTPS cookie/GET/Range/seek/refresh/revoke/withdraw/wrong-session qualification, the
  inherited UI qualification findings and native private playback remain open in the #676 ledger.
