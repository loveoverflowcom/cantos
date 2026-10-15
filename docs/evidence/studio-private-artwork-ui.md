# Studio private Work artwork UI

Scope: [issue #156](https://github.com/loveoverflowcom/cantos/issues/156), based on develop
`a079e72`. Owner: Studio Web `live/artwork`; contract:
[artwork-v1](../../packages/contracts/artwork-v1.md), policy:
[012](../work-plan/012-private-work-artwork.md). This adds the private owner UI without a new
API, migration, public projection, provider, image transformation or publication decision.

The [Studio package guide](../../apps/cantos-studio/web/README.md#private-work-artwork) describes
the controls, preview recipes and recovery. Dependencies are the implemented private backend
contract/storage boundary and existing UI primitives. Broader #66/#131 and live artwork IO
acceptance remain separate. No ADR is needed: no dependency, framework or policy is selected.

## Claim and oracle ledger

| Claim / failure | Owner / oracle | Evidence scope |
| --- | --- | --- |
| Input bounds use bytes, not character counts; server remains authoritative | `Draft`, file/dimension helpers; exact boundary tests | Example-tested; browser decode is a hint, not a server witness |
| Prepare carries provenance/rights without alt; admission leaves the cover unchanged | `Command` DTO encoding and `Projection`; exact assertions | Example-tested |
| Wrong work/revision/alt/owner/hash cannot acknowledge a different operation | Command correlation and existing RequestState | Exact recovery/CAS tests |
| Unknown writes block fresh mutation/discard; exact identity survives | existing RequestState, exact intent reads and immutable replay | Lost response and late callback tests; browser HTTP doubles |
| Conflict preserves replacement and compares an explicitly reloaded head | Studio-owned projection and form | Browser scenario; no last-write-wins |
| Current private image is separate from local replacement | fenced metadata/byte read, exact hash/length/type, Blob URL lifecycle | Browser HTTP doubles; real object storage remains unrun |
| Explicit clear only unlinks; local discard cannot cancel committed operations | confirmed null PATCH and separate discard intent | Browser keyboard/dialog and request assertions |

## UI contract and provenance

Low–Medium Studio intensity: current artwork and new replacement lead, quiet token-based
containers, one primary next step. Reused catalog APIs: `button` (Filled/Tonal/Text),
`text-field` (TextField, controlled values/read-only/error), `badge` inside a native polite
region, and the existing Studio confirmation adapter (native containment, Escape and focus return). Native file selection is an app-owned
input with a persistent label and associated bound/error text. The existing `studio-artwork`
design declaration remains Web-owned; no new shared API or token is introduced. Canonical
`tokens.json` v0.6.0 is unchanged; styles consume its roles. vi-VN/en copy lives in `artwork/copy.rs`.

The debug-only v1 composition preview uses the existing Studio bundle/runtime and the exact
production form with injected facts/intents. Theme/locale keep draft text; scenario selection
explicitly resets projection. It has no network/auth/storage capability. No CMP admission is
claimed for this Web-only composition; native Studio remains out of scope.

Reference: Drive `Cantos-Studio-Web-104.html`, file
`15Bb_vw4pwbBQpQEmA1zpXVohOGZz_4V8`, S02. The original is retained only in ignored
`artifacts/ui-evidence/issue156/reference.html`. Reference fixture pictures and proposed global
geometry do not become production assets or tokens. Region comparison is qualitative; there is
no aggregate fidelity percentage or owner-approved pixel baseline.

## Execution and independent gates

Run artifacts: ignored `artifacts/ui-evidence/issue156/`. Exact commands and final outcomes are
recorded below after the final checks. No provider cost or real manuscript/media is involved.

Initial diagnostics: the first form compile exposed an invalid `ViewFn::new` call, corrected to
the established closure API. The first Rust gate stopped on unformatted new source, corrected
with rustfmt. A sandboxed workspace run then failed 41 existing loopback fixture binds with
EPERM; local-socket execution passed the artwork tests but hit the existing concurrent xtask
port-release test (`probe::tests::port_ownership_reflects_an_existing_listener`). That unchanged
test releases an ephemeral port then immediately expects it to remain free. The single-threaded
`RUST_TEST_THREADS=1 cargo xtask verify rust` run passed every workspace Rust gate, including
the unchanged port-release test and all 10 artwork regressions; no test was altered or skipped.

`cargo xtask app cantos-studio --web` refused because no local API listens on 8080; no server
was started. Browser evidence uses the xtask-built CSR bundle with Playwright static-delivery
and HTTP doubles at loopback origin, without a listening mock server. This tests real browser
inputs and the production client shell; it is not a running API or storage integration. The
first fixture delivery of a 242 MiB name-section-heavy WASM artifact closed the browser; the scoped
browser build uses `CARGO_PROFILE_DEV_STRIP=symbols` (still a debug build, with its preview entry).

## Final UI observation record

Runtime source SHA-256 (100 pinned source/build inputs):
`2220f58aa0c115404c03bdaea0fb08dfd51ba7c9d61d094729ae7e0bfc02e539`. The full source, build-asset and image digests are retained in
`artifacts/ui-evidence/issue156/{runtime-source,capture-manifest}.json`; the portable
[capture manifest](images/studio-private-artwork/manifest.json) and five original synthetic
screenshots are committed. No downloaded reference art is committed.

Renderer: Chrome **153.0.8010.36**, headless on Linux, DPR 1; vi-VN, Noto Sans available,
`document.fonts.ready` settled. The reference embeds Noto Sans; exact font-file/version parity
was not established. Actual browser viewports are 1280×900 and 320×900 CSS px. Root text is
16 px (100%) or 32 px (200% text stress); this is not browser zoom. Motion preference is
`reduce`; no new animation or frame-pacing claim. The existing shared token focus indicator is
visible and labels/statuses have text beyond color. Styles were manually checked for token
roles, scope, focus and reflow; no separate CSS/view formatter is configured.

The debug composition exercised all 13 named states without API traffic and retained text
through a theme change. The real production client against HTTP doubles exercised:
invalid MIME/dimensions and UTF-8 alt, failed PUT with retained fields, lost prepare/PUT reads,
lost PATCH with byte-identical replay, conflict and explicit revision comparison, expiry with
an explicit new intent, owner denial, 401 with deliberately delayed cookie revalidation and
same-owner draft restoration, exact private Blob preview, and explicit null clear. An actual
replacement retains the old cover until the newly admitted immutable asset is selected.
Selecting an invalid replacement retains previous draft/bytes but blocks prepare/upload until
a valid file is chosen again. No page errors were observed. These are DOM/interaction and
mock-integration evidence, not server authorization or object-store acceptance.

Native dialog Tab visits its close/confirm controls and browser chrome (BODY while Chrome
has focus), without focusing background controls; Escape closes and restores the opener.
The initial stricter Tab assertion was a harness error, not a changed shared Dialog rule.
Likewise, cookie revalidation is automatic after 401; the final scenario delays that actual
observation rather than assuming a visible retry control or a persistent signed-out screen.
OS IME, a screen reader and real OIDC reload were not exercised. Draft/operation retention
is limited to the existing mounted session; confirming a browser reload can discard it.

Images opened with `view_image`: all seven final preview/product captures in the run's browser
report, S02 reference, and an unscaled 200% detail tile. Findings rechecked at the same scenarios:
qualified single-line fields avoid the inherited editor minimum, the empty error strip is hidden,
disabled reasons name the missing artwork step, and the native file button wraps at 200% text.
The captured narrow pages have no horizontal document overflow, and actions remain reachable
by scrolling. The prior reason captures and hashes remain in `before-reasons/`; early initial
captures were exploratory and are not an approved immutable pixel baseline. Reference comparison
is qualitative: current artwork, replacement, bounds, provenance/rights and alt anatomy are
retained; the existing Studio workspace and token geometry are preserved rather than adopting
the prototype's proposed global geometry. No aggregate fidelity score is claimed.

- [Replacement / light](images/studio-private-artwork/replacement-1280-light.png)
- [Conflict / light](images/studio-private-artwork/conflict-1280-light.png)
- [Selected replacement / light](images/studio-private-artwork/current-1280-light.png)
- [Current / narrow dark](images/studio-private-artwork/current-320-dark.png)
- [Current / narrow dark, 200% text](images/studio-private-artwork/current-320-dark-text200.png)

The first final preflight rejected direct new imports of unqualified Dialog/Status/TextArea/UiRoot.
No qualification record or baseline exemption was changed. The subsequent token test required
the existing Web shape alias in artwork.css; that alias preserves the same reading radius
without consuming the shared CMP shape role directly. The form now uses qualified TextField
(single-line alt and provenance, matching S02's source field) and Badge APIs. Artwork clear
extends the existing Studio `DiscardDraftDialog` target projection; its shared Dialog import,
focus and locale ownership remain at the established app adapter. The preview is hosted by the
existing Studio view root, without another UiRoot import site. New adopted APIs therefore have
fresh qualification; compatibility owners remain explicitly legacy rather than being promoted.

Independent results already executed: workspace Rust format/lint/tests **PASS** (serial local
socket run), all five contracts gates **PASS**, and Web/shared-preview checkpoint **PASS**.
The final Studio lint/tests/build and repository hygiene are covered by the final-diff preflight
reported in the PR. The first ten artwork regressions ran in the workspace suite; final Studio
preflight also includes the added invalid-replacement guard regression. No Quint model is
registered for artwork (the catalog inventory explicitly excludes it from the audio pilot);
native exact CAS/recovery tests retain their own authority. No performance/cache claim is made.

## Deferred acceptance and residual risk

- Real PostgreSQL, owner/right resolution, storage and process-restart IO are NOT_RUN for this
  UI slice. Reuse [the backend acceptance ledger](private-work-artwork.md); provision a fresh
  disposable PostgreSQL toolchain and private storage, run `cargo xtask verify postgres`, then
  the authorized local owner journey. Acceptance needs actual metadata/bytes readback, owner
  denial, expiry, exact retry after response loss, and unchanged private history. Next owner:
  unassigned — assignment required before production acceptance.
- No live issuer/provider, public delivery, publication, native runtime, OS IME or screen-reader
  walkthrough is claimed. A browser fixture cannot establish any of them.
- 200% text stress and real browser zoom are separate observations; only executed conditions
  below count. Reduced-motion input does not establish animation frame pacing.
- Remote CI is not checked or awaited. Opening the requested PR and closing #156 do not accept
  the broader backend or phase-exit gates.
