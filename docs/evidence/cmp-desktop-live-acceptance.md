# CMP Desktop Preview live acceptance and Library diagnostics

Status: implemented preview composition and tests; **native device acceptance, GUI window interaction
and real-audio acceptance are not part of this record.**
Integration base: `12fb751ec71e20233fe4caaee35e4544fe4a0d4b` (`refactor/cantos-native-turso`,
after PR #388). All results below were produced on that revision plus the uncommitted working tree
of this change; GitHub owns issue execution status and this record does not tick #335/#336.

## Claim and boundary

The opt-in live Desktop Preview now mounts the durable Library through the same production owners the
Android and iOS hosts build, so the live window can log in through Identity v1 and read and write the
Library over real HTTP. Before this change `LiveEnrollmentOwners` passed `library = null` to
`NativeSessionRuntime`, so the live window had Identity but no Library, and it never dispatched
`PublicCatalogIntent.Start`, so its Discovery never requested works (defect 3).

Owner/direction: `:preview-sandbox` desktop composition → `:shared` owners and ports. Nothing in
`:shared`, Android or iOS imports the sandbox, and no `:shared` code changed. Mocks exist only in
sandbox composition and tests. The default window stays offline (mock ports); live mode is selected
only by an explicit `CANTOS_THEATRE_PREVIEW_API_ORIGIN`, never falls back to a mock, and an invalid
origin is a typed refusal that mounts no owner.

## Evidence ledger

Runner: JVM desktop (Compose `runDesktopComposeUiTest`, Gradle 8.14.3, JDK 17.0.19, Rust 1.98.1),
macOS arm64. Source: base SHA above, dirty tree. Levels use the foundation vocabulary.

| Claim | Evidence (command, class) | Result | Level | Live / mocked |
| --- | --- | --- | --- | --- |
| Offline preview compiles and the whole JVM suite passes | `cargo xtask theatre-preview build`, `cargo xtask theatre-preview verify` | PASS (shared 490 tests, 3 explicit-lane skips; sandbox 124 tests, 2 explicit-lane skips: the real-server and the window lane). Baseline before the change: 490 / 103, 0 skipped in the sandbox | example-tested | mocked ports |
| The live window composition wires Identity → authorized HTTP → Library → runtime → shell | `LiveLibraryPreviewTest` (10 tests) over a loopback protocol fixture and the real `PreviewNativeHttp` | PASS: admitted session, four Library sections read with Bearer and the session stamp, no ambient cookie, stamped save, lost committed response settled by receipt without replay, unstamped contracted 503 retryable with authority kept, 401 revalidates, wrong stamp fences, missing success stamp refused, logout discards a held late response, outage leaves the save pending, oversized body fails closed | example-tested / fault-injected | **mocked server** (the fixture stamps its own responses) |
| The live window screens work with the real product controls | `LiveDesktopUiTest`: Register → Login → real v1 login over HTTP → Library tab | PASS | semantics-tested | mocked server |
| Real Server + Identity accept the live composition | `cargo test --locked -p cantos-server --test theatre_business_journey cmp_live_desktop_preview_against_real_identity_and_http -- --ignored --exact --nocapture` (`LiveDesktopPreviewRealServerTest`) | PASS, `LIVE-DESKTOP-PREVIEW-OK`: the real public catalog listed the seeded work; a guest save sent nothing; real login admitted; all four sections loaded; shell save and a bookmark reached the Server; logout fenced the account and Library; Rust then read 1 saved row, 1 bookmark row, 0 anomalies | integration-tested | **live** Server, Identity, SQLite file; in-memory preview store; audio unavailable |
| The existing production CMP path still holds | `... cmp_authorized_library_against_real_identity_and_http -- --ignored --exact --nocapture` | PASS, `LIVE-AUTHORIZED-LIBRARY-OK requests=23` (also run on the unmodified base) | integration-tested | live Server and Identity |
| Product journeys by semantics only | `DesktopAcceptanceJourneyTest` (4): one player owner across Queue, Library and the mini player with pause, ±15 s seek and elapsed text; the sleep-timer dialog restores focus; the Back-key predicate and its hookup to the shell's Back; logout clears the Library owner | PASS | semantics-tested | mocked ports (decoder stand-in, no audio) |
| Escape delivery in a real window | `CANTOS_PREVIEW_WINDOW_TESTS=1 ... :preview-sandbox:desktopTest --tests '*DesktopWindowEscapeTest'` (explicit; opens a short-lived AWT `ComposeWindow`) | PASS: with nothing focused `Window.onKeyEvent` received Escape and ran Back; a focused control that handled Escape kept it and the window did not also go Back | integration-tested (AWT event, static content) | not the application, not an OS keystroke |
| Origin gate | `LivePreviewOriginTest` (2): HTTPS and explicit loopback `http` admitted; empty, remote cleartext, private-network cleartext, userinfo, path/query, other schemes refused | PASS | example-tested | n/a |
| A desktop window opens in live and offline mode | `cargo xtask theatre-preview run` (offline, `http://127.0.0.1:9`, `http://example.com`); window observed with CoreGraphics (420×900, on screen) | window present in all three; **its content was not observed** | observed existence only | n/a |

## Gates and baseline deltas

| Gate | Result on this change | Clean base `12fb751e` | Delta |
| --- | --- | --- | --- |
| `repository` (`scripts/check_repository.py`, run under a Python 3.14 shim because the default 3.9 cannot import it) | **FAIL**, 31 findings | the same 31 findings (compared as sorted sets) | 0; none in a touched file; not fixed here |
| `rust-format`, `rust-lint (cantos-server)` (`cargo fmt --check`, `clippy --all-targets -D warnings`) | PASS | n/a | n/a |
| `rust-tests (cantos-server)` (`cargo test -p cantos-server --locked --no-fail-fast`, identity binary set) | 871 passed, **6 FAIL** in `surface_policy_baseline` | `surface_policy_baseline`: the same 6 FAIL, 4 passed | 0; unrelated surface-policy 404s |
| `theatre-cmp-preview` (`cargo xtask theatre-preview verify`) | PASS | PASS (490 / 103) | +21 sandbox tests |
| `audio-inspection` | NOT_RUN (`--skip`; no ffmpeg on this machine) | n/a | n/a |
| `wav-reference` | NOT_RUN (blocked behind the first `rust-tests` failure in preflight) | n/a | n/a |

`cargo xtask preflight --changed-only` stops at the first failing gate, so the gates after `repository` and
`rust-tests` were run individually or skipped as listed; preflight itself reports FAIL.

## Defects found, root cause, regression

1. **Escape did nothing in the offline window after pointer navigation.** Symptom: select a work with
   the mouse, press Escape, stay on the work. Cause: the handler was `Modifier.onPreviewKeyEvent` on a
   composable. With nothing focused the scene root holds focus and key dispatch starts at the root, never
   visiting a descendant. Reproduced in the Compose scene (red: route unchanged after
   click → Escape) and, with a throwaway `ComposeWindow` experiment, in an AWT window: a window-level
   preview handler fired once, a content-level one zero times. Fix: `PreviewBackKeys` installed through
   `Window(onKeyEvent = …)` (post-content, so a focused control or dialog that handles Escape keeps it; a
   window-level *preview* handler would take the key first), extracted from `main` as `PreviewDesktopHost`.
   Regression: `DesktopWindowEscapeTest` (explicit real-window lane, delivery and precedence) plus
   `DesktopAcceptanceJourneyTest.the_back_key_is_the_shell_back_only_below_the_root_and_only_on_escape_release`
   (predicate and hookup in the Compose scene). Neither drives the application through a real window: that
   window does not recompose after its first frame in this JVM test. Escape with the timer dialog or the modal
   error dialog open is **not** exercised.
2. **A cancelled request could throw an unchecked JDK exception into the shared coroutine scope.**
   Symptom (captured on stderr in a full verify run): `RuntimeException(NullPointerException: this.http
   is null)` from `getResponseCode` inside `PreviewNativeHttp`, followed by
   `UncaughtExceptionsBeforeTest` in an unrelated later test. Cause: `cancel()` called `disconnect()`
   while the I/O coroutine is in its read; the JDK can then throw an unchecked exception that none of the
   `catch` clauses handled. In the Compose host that scope is shared by the account, Library and catalog
   owners. Fix: `cancel()` cancels the job before disconnecting, and the transport maps any other
   `RuntimeException` (and the previously uncaught `URISyntaxException`) to a `transport` or `protocol` failure
   after `ensureActive()`, so cancelled work delivers nothing. A programming error such as an NPE in this adapter
   is also reported as a transport failure; that is accepted for a diagnostic adapter. The race itself did not reproduce over about 290
   real-socket attempts; the regression uses a stub connection that blocks until `disconnect()` and then
   throws the captured exception
   (`cancelling_a_blocked_read_whose_jdk_connection_then_throws_unchecked_...`; red without the
   `catch`, green with it).
3. **The live window never requested the public catalog.** `LiveEnrollmentOwners` built a
   `PublicCatalogController` and never dispatched `PublicCatalogIntent.Start` (found by review of the diff), so
   Discovery in live mode stayed unloaded. Fixed by starting it as the native hosts do.
   Regression: `LiveLibraryPreviewTest.the_live_window_requests_the_public_catalog_without_signing_in`; the real
   Server lane also requires the seeded work to appear in the catalog.
4. **Fixture defect, not a product defect:** the loopback fixture recomputed `authenticated_at` and the
   expiries per request, so a request pair that crossed a second boundary made the client see changed
   session facts and, correctly, refuse them as a protocol violation. The fixture now fixes the session
   facts at first login. Two intermittent failures of the 401 test and one failed sign-in are attributed to
   this; the attribution is by mechanism and the reproduction of the sign-in failure, not by a forced repeat
   of the 401 failure.

Checked and found sound: the JDK transport reports a genuine `401` on a streamed `PUT` as `401`
(revalidation follows), not as a status-0 transport failure; the public catalog transport sends an
intentional empty `Cookie` header and no other transport sends any.

## Not covered / residual acceptance

| Gap | Current behavior | Prerequisite | Next check | Owner |
| --- | --- | --- | --- | --- |
| GUI interaction with the real window | Window exists (CoreGraphics); `screencapture` and assistive access are denied in this session, so no pixel, click or key was observed in the window | Screen Recording and Accessibility permission for the host process | open `theatre-preview run` and walk it by hand | operator |
| Escape fix in a real window | Verified in a Compose scene and with a throwaway AWT `ComposeWindow`; not exercised through `main()` | as above | press Escape in the offline window after pointer navigation | operator |
| Escape with a dialog open | Not exercised; the window handler runs only for keys the content left unconsumed, which is the design, not an observation | a window that recomposes in a test, or a manual run | press Escape with the timer or error dialog open | operator |
| Desktop connectivity signals | The live window does not call `networkRestored()`/`foregrounded()` | a desktop adapter (non-goal) | n/a | CMP native |
| Injected clock | The preview's clock is whole seconds (`now() * 1000`), unlike Android's `currentTimeMillis` | n/a | n/a | CMP preview |
| Library durability | Preview store is SQLDelight in memory | file-backed run | existing `SqlLibraryPersistenceTest`; process death needs a device | native owners |
| Audio | No desktop decoder: playback requests fail as unavailable by design | decoder adapter (non-goal) | native gates | CMP native |
| Android/iOS | Not applicable here; no APK, emulator or Xcode | devices | native gates stay open | CMP native |
| Intermittent 401 timeout | Two failures of `a_401_on_a_library_write_...` before the fixture fix (attributed to defect 4). After it: 3 repetitions of the 18 live/journey tests and the full verify passed. A recurrence would be a new finding | n/a | re-run `LiveLibraryPreviewTest` if it recurs | CMP preview |
| Semantic driver for agents | A JSON command driver over the Compose test API was proposed by the requester; not built | decision | separate slice | CMP preview |
