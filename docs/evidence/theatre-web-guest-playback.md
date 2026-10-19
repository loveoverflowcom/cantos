# Theatre Web guest playback — issue 269

Related issue: [#269](https://github.com/loveoverflowcom/cantos/issues/269) (slice 1 of 4 of
[#13](https://github.com/loveoverflowcom/cantos/issues/13)). Baseline:
`develop@78fbb8e15089617634dbbf1526d388d2911fd2ff`. The tested tree is that baseline plus this
change; the pull request records the head SHA, which a commit cannot contain. This record names
what ran, with which doubles, and what did not. It accepts no deployed, CDN, native or
human-listening result.

## Review boundary

Delivers: a guest chooses a published unit in live Theatre Web and **hears its audio** through one
page-owned player; play, pause, seek, speed and route changes share one session bound to the exact
release; failure, expiry and withdrawal have named, bounded behavior; the local media origin the API
advertises is one a browser can open.

Does not deliver: save/progress/history/resume (#272), search and route work (#270), auth/Library
(#271), queue, transcript, offline, CMP/native playback, Safari/iOS, deployed or CDN delivery and
CORS, rendition fallback, or any change to the backend contract.

## Source facts that drove the design

| Fact (read from `develop@78fbb8e1`) | Consequence |
| --- | --- |
| The player core keyed audio by a synthetic mock `EpisodeId` and reported `Status::Simulating` | A closed `TrackId` (`Fixture` or the pinned `Public` release) and `Status::Playing` plus a closed `Failure`; the mock keeps its simulated labels |
| The media port returned callbacks synchronously, a shape only a fake can have | The session accepts late callbacks through one ordered inbox; every callback carries the identity it belongs to |
| The seed host hard-coded the media origin `https://localhost:8080` while the API speaks HTTP on 8080 and admits only an HTTPS origin | Explicit origin `https://localhost:8443` plus a loopback TLS front end; no client or admission change |
| Seek acknowledgement required an *exactly equal* position | The driver acknowledges a completed seek, then reports where the element landed; a real element never lands on the millisecond |
| The range control read a `Memo` from its `blur` handler | Removing a focused slider by navigating fired `blur` after disposal and aborted the page; found by the browser run, fixed and unit-tested |

## Design in one paragraph

Pure core, thin shell. `core/public_track.rs` admits a manifest only for the requested
work/unit/publication and pins identity, digests and duration; `AdmittedGrant` admits an
authorization only for that pin (publication, rendition, SHA-256, `audio/ogg`, byte ranges,
HTTPS URL, bounded expiry) and its URL is a redacted capability. `application/public_listen.rs`
fences each listener intent with a ticket. `application/media_driver.rs` is a pure state machine
from engine/grant/timer/element inputs to commands and player events (grant refresh 30 s before
expiry for the same publication, backoff 1/2/4 s over four attempts, three recoveries without
progress, `404` stops). `platform/audio.rs` executes those commands on the one
`HTMLAudioElement`. The reducer, scrub logic and timeline control are reused.

## Acceptance criteria (issue #269)

Evidence labels follow the [engineering vocabulary](../../.agents/skills/cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified).
"Browser" below is **Google Chrome** driven over CDP on this Mac, not the in-app browser.

| Criterion | Status | Evidence |
| --- | --- | --- |
| Guest picks a seeded unit from API/PG and hears real bytes; manifest → authorization → media with masked capability | met (local) | `integration-tested` + `interaction-tested`: `GET /publications/{id}` 200, `POST …/playback-authorizations` 200, `GET /media/<redacted>` **206** `Range: bytes=0-`, `Content-Range: bytes 0-384847/384848`; `audio.duration` 125.0 s, `readyState` 4 |
| Play, pause, seek, speed; UI follows the engine; seek with Range/206 in the browser | met (local) | Pause holds position; speed reaches `playbackRate`; pointer seek confirmed (no lingering pending state). With audio bytes throttled, a seek beyond what was buffered issued `Range: bytes=294912-…` and `bytes=32768-…`, each **206**; `currentTime` reached the chosen part |
| Home/Work/full/mini keep one owner and one source; rapid switching drops old callbacks | met (local) | One `<audio id="theatre-audio-engine">` and the same element object across `#discover`, `#library`, `#account`, two Works and `#player`, zero new authorizations. With a held-back older manifest or authorization, the older intent never reached the element or the media route; the older row is not stuck "preparing" |
| Browser uses exactly the URL the backend returned; no TLS/scheme/CORS error from the local composition | **partial** | The advertised origin is now `https://localhost:8443` (an exact, admissible HTTPS origin) and `curl --cacert` with the exact URL returns `206`. The certificate is self-signed: a browser follows it only after the **operator** trusts it. Chrome ran with `--ignore-certificate-errors-spki-list` pinned to that one public key. The in-app browser refused the origin and was not used for audio |
| Grant expiry/retry; withdrawn/replaced release | met (local) | Refresh `POST` for the **same** publication 90.0 s after the grant (expiry − 30 s), no media request after it, playback ran to the end. A **real Studio withdrawal** during playback: manifest `404` at once, buffered audio continued, the next refresh got `404`, playback stopped with the named reason and attached nothing. Fault-injected: `404` → one request, no loop; `503` → four attempts at 0.17/1.17/3.18/7.18 s then a named failure, and an explicit Play after recovery plays; changed SHA → "release changed", nothing played; persistently failing media → one authorization plus three recoveries, then a named failure; one transient media failure → recovered with a second authorization for the same publication |
| API/media down and browser refusal have deliberate retry; reload does not autoplay | met (local) | The same fault set; an untrusted (non-gesture) activation made Chrome refuse `play()`: named, nothing claimed playing, an explicit press played. Reload on `#player` shows the empty state (not Home) with no audio element and no request; Back/Forward and the sign-in route never restart a paused source |
| Evidence tied to a SHA; no CDN/production/native inference | met | Baseline above; local origins only; nothing here is a deployed, CDN, CMP or listening result |

The issue is therefore referenced with `Part of`, not `Closes`: the TLS criterion depends on an
operator trusting a local certificate, and the in-app browser could not be used for audio.

## What ran

| Layer / claim | Status | Level and command | Verified scope / gaps |
| --- | --- | --- | --- |
| Pure identity, grant admission, listen intent, driver, reducer failure semantics | PASS | `example-tested`, bounded matrices; `cargo test --locked -p cantos-theatre` (336 lib tests, 3 doctests) | Exact variants for every rejection; backoff and recovery budgets; stale tickets/attachments |
| Session + reducer + driver with a simulated element | PASS | `mock-integration-tested`: `application/media_session_tests.rs` | Indicator follows engine facts; system pause; withdrawn → retry only by press; seek confirmation; source switch |
| Assertion strength | PASS | `mutation-tested` by hand: 12 mutants of the fences, budgets, identity check, blur guard and inbox guard — 12 killed (two survivors strengthened the tests, then killed) | Not the automated `mutation` gate |
| Format, lint | PASS | `cargo fmt --all -- --check`; `cargo clippy --locked -p cantos-theatre --all-targets -- -D warnings` and `--target wasm32-unknown-unknown` | — |
| Local-origin tooling | PASS | `python3 -m unittest test_serve_theatre_media` under Python 3.9 and 3.14: advertised origin rules, no inherited origin, grant never logged, `206`/Range and TLS name verification through the front end | A real handshake against a generated certificate |
| Wire to a real API, PostgreSQL, filesystem storage, real Ogg/Opus | PASS (local) | `integration-tested`: seeded retained cluster (`cargo xtask seed theatre`), `scripts/serve_studio.py`, `curl --cacert` and CDP journeys above | Loopback only; synthetic tones (125 s, 2–5 s), no voice |
| Real browser behavior | PASS (local) | `interaction-tested` in Chrome over CDP; `cross-viewport-inspected` 1280×900, 390×844 and 320 px reflow; `cross-theme-inspected` light/dark; keyboard Enter on the transport; focus ring seen | Not a screen reader (`accessibility-checked` for keyboard/focus/semantics only) |

Handoff gates on the final tree (`PATH` with a `python3 → 3.14` shim, `LC_ALL=C`,
`TMPDIR=/private/tmp`, `RUSTUP_TOOLCHAIN=1.98.1`):

```text
cargo xtask preflight --changed-only --skip repository
  NOT_RUN  repository (skipped: red on clean develop, see below)
  PASS     rust-format     PASS  rust-lint (cantos-theatre)   PASS  rust-tests (cantos-theatre)
  PASS     web-lint (cantos-theatre)                           PASS  web-build (cantos-theatre)
```

The `repository` gate's parts, run separately: `xtask services validate` PASS; `git diff --check`
clean; `python3 -m unittest discover -s scripts -p 'test_*.py'` ran 780 tests (774 on clean
`develop`) with the **identical 60 failing/erroring tests** on both trees (VPS/OIDC, Android hosts,
mutation workspace): the six added tests pass. `scripts/check_repository.py` fails on `develop` and
here with the same six stale preview qualifications; it reported four broken links, two icon
literals and an unqualified `Select` for this change before they were fixed.

Not covered: Safari/iOS and Firefox, a screen-reader walkthrough, a human listening to the audio
(`audio-measured`/`audio-listened` were not run; the bytes are synthetic signals), deployed
Firebase Hosting/CORS/CDN addressing, `Cache-Control` behavior of a CDN, long sessions beyond one
refresh, device and native playback.

## Environment, doubles and pre-existing conditions

- Cluster: `cargo xtask verify postgres --suite schema --keep` finished with its `bearer_postgres`
  test failing (`NotYetValid` in `signed_auth_time_is_preserved…`), on a tree identical to
  `develop`; the retained-cluster receipt records `FAIL`/`NOT_RUN` and the seed lane consumes it
  by design. That failure is unrelated to this change and was not investigated further.
- Fault injection used CDP `Fetch` interception (`404`, `503`, a rewritten SHA, failed media, delayed
  answers). Those results are `fault-injected`, with the real browser, element and wire client.
- The headless Chrome profile was throwaway; autoplay refusal was observed there without a gesture.
- `python3 scripts/check_repository.py` fails on clean `develop@78fbb8e1` too (six stale
  preview qualifications and the resulting "new use" lines). After this change only that
  baseline set remains: the repository gate is red independently of this slice.
- At a doubled default text size on a 390 px screen the live shell's topbar already overflows by
  3 px before any player exists; the player's own overflow (cover, mini-player title) was fixed.

## Reproduce

```bash
cargo xtask verify postgres --suite schema --keep
cargo xtask seed theatre
cargo xtask run cantos-theatre --web --test-cluster      # with CANTOS_TEST_CONTEXT=theatre
python3 scripts/serve_studio.py tls-front --generate     # third terminal
```

Accept `https://localhost:8443` in the browser, open `http://127.0.0.1:8082/`, choose
_Mùa gió chướng_ and press **Nghe**.
