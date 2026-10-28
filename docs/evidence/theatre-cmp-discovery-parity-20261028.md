# Theatre CMP Discovery/Search repair — 28 October 2026

Part of [#453](https://github.com/loveoverflowcom/cantos/issues/453), with artwork owned by
[#417](https://github.com/loveoverflowcom/cantos/issues/417), the header by
[#532](https://github.com/loveoverflowcom/cantos/issues/532), and shared controls by
[#452](https://github.com/loveoverflowcom/cantos/issues/452).

## Boundary and provenance

Prompt intake: `CANTOS_PROMPT_INPUT_V1`, Library input
`Pasted text(20261028-110643).txt` (361 lines, 13,307 bytes), read in full.
Fetched base and integration target: `origin/develop`,
`a86ad73c41cd76e2c6c9db2bca98f9692196edc5` → `develop`.
The repair uses a new isolated worktree and branch. It imports no uncommitted work from the
stopped cover/intake worktree. The base already contains the merged #577 source.

This PR repairs the app-owned Discovery/Search presentation. It changes no service contract,
catalog data, publication/cover policy, player, auth, shared component, token, qualification
receipt or native resource. The canonical localization generator produces the changed CMP copy;
the other generated app files change their catalog digest header only. No ADR is needed:
the existing route, query owner and approved design already determine the behavior. Work-queue
order and ownership are unchanged.

The owner screenshot is described in #453 but is **not hosted there or available to this
executor**. Its pixels were not inspected in this run. `adb devices -l` with platform-tools
35.0.2 returned an empty device inventory. Installed APK `versionName`, `versionCode`, variant,
channel, source SHA and package identity therefore remain **UNKNOWN**. Neither a stale APK nor
a native resource failure has been established. The earlier release SHA is not assigned to
the unknown screenshot.

## Confirmed causes and repair

| Observation | Classification and source evidence | Repair / owner |
| --- | --- | --- |
| Hero asserts an unfinished story without progress | `CONFIRMED_SOURCE_DEFECT`: `DiscoveryScreen` rendered `ContinuingHeading` unconditionally; the new regression failed on the base | Render that label only when the existing `continuingProgress` projection supplies a point; otherwise use the neutral v4 invitation |
| Discovery offers a 48dp search icon | `CONFIRMED_SOURCE_DEFECT`: base regression measured 48dp instead of a full search pill | Reuse admitted `FloatingSearch` in a read-only navigation composition; one full-area button enters the existing Search route, which receives input focus |
| Complete catalog empty offers Retry and reload instructions | `STATE_MAPPING_DEFECT`: `CatalogRecovery`'s final branch always emitted Retry | Complete empty has artwork and centered localized title/short copy, with no recovery action; filtered no-results keeps Clear Search; only failure offers Retry |
| Search can announce zero completed results while loading/failed | `STATE_MAPPING_DEFECT`: count depended only on `worksPhase`, whose empty projection can be `Complete` | Gate the completed count on `Ready`/`Empty` mode as well |
| Artwork/theme/avatar absent in the reported APK | `UNKNOWN`: current source already calls Works artwork and the shared shell header already renders the theme action/text avatar | Preserve those owners; native packaging/device reproduction remains pending |

Discovery has Medium expression: a compact tonal hero and search are the primary entry;
actual story content or truthful availability follows. Canonical `color.theatreHero`,
`color.theatreOnHero`, theme typography, shapes and spacing supply the treatment. No palette,
font, animation constant or fixed viewport height was added. Natural route scrolling, safe
drawing/IME padding, bottom navigation and mini-player composition remain at the shell owner.

`DiscoverySearchPill` has no editable query or local search state. Its admitted read-only field
is a visual composition with descendant semantics/focus excluded; the covering navigation
button is the single accessible action and receives pointer/keyboard input across the pill.
The Search route retains the existing mutable `expanded`/focus owner, canonical shared
field/dock, query-on-change, real `SubmitSearch`, conditional Clear, collapse and overlay Back.
Entry requests focus; Back first collapses the dock and then navigates, retaining raw query.
No suggestions, source-search API or production mock fallback were added.

## Design and rendered evidence

Actual authenticated Drive files were read; these PNGs were downloaded and opened:

| Reference | SHA-256 | Use |
| --- | --- | --- |
| [v4 board 1](https://drive.google.com/file/d/1qMZqRCcg1kYIMcF_0ySvCU2uFxFYdOmg/view) | `3e40db9eef2cf639dbcbe9a91314d95ec448ce18d013f627badb16edab0ac433` | Neutral guest invitation and search containment |
| [v4 board 3](https://drive.google.com/file/d/1GBHHKVsTWAvV3QXFVmNC3Ysxi9qMF222/view) | `bd8f3f013367ed63e24b17e5fae6d7983556bfc9f4e55fdf2daeafec6dc6bd99` | Mobile hierarchy/header/navigation direction |
| [Search v4.3 mobile](https://drive.google.com/file/d/1BmpRcb8Esyw8-OAhywd6iTg6cHK6eOuB/view) | `cd5bbcf02531ec076695d6a65cbeab900683ff261e2bf91c0024ee7260e1f510` | Trailing magnifier, joined field/dock and viewport constraint |

The [Search handoff](https://drive.google.com/file/d/1X4ahFW7jCPIXGutqW8icFve2ctJEDD_h/view)
and [artwork handoff](https://drive.google.com/file/d/1fvSBMbQH_6xhqOYqXpn1Gzvh9RZGmw30/view)
were read. Reference boards are design targets, never Android runtime evidence.

The catalog lists `text-field` and `icon-button` two-renderer qualification records
(`ui-requal-20261027-*`). The repository gate reports stale Leptos qualification inputs:
the clean pinned base and this patch produce the same 50 diagnostics, including app-use
diagnostics. This repair reuses the already-consumed APIs in the same app owner and changes no
shared source, token or receipt. It does not renew or accept the stale records. Their historical
bounded Web/JVM evidence establishes no native IME, AT or device parity.

`DiscoveryParityTest` captures the actual `CantosPresentation` with controlled synthetic facts,
confined to the JVM sandbox: completed empty, filtered no-results, loaded, partial, loading,
failed and retained rows after failure. Each has 320/390dp × light/dark × VI/EN × 1×/2× text:
112 baseline and 112 after PNGs, with semantics and source/hash sidecars. The frames are
844dp high, density 1, injected Reduced motion. These are **JVM Desktop Compose test-root**
captures, not phone screenshots. Initial top positions at 200% may require scrolling to reach
the artwork/cards. Captured cells that were not opened remain `screenshot-captured`.

Ignored local evidence: `artifacts/ui-evidence/discovery-parity/{before,after}/` and
`artifacts/agent-runs/discovery-parity/`. Baseline uses base rendering source plus the new
regressions; after records the dirty source explicitly. `source-inputs.json` binds final source
hashes, and each PNG has its own SHA-256 sidecar. Evidence is local-only, not uploaded by PR creation.

Opened baseline findings: 390/light/VI/1× shows the unconditional continuation label, small
search icon, left-aligned empty text and Retry; 320/dark/EN/2× shows oversized hero copy.
The same-state after views show the neutral compact hero, full pill, visible paired Works
artwork and centered empty text without Retry. An intermediate EN 2× capture exposed a midword
break; the neutral English invitation was shortened before the final recheck.

Search route mount now intentionally opens/focuses its dock, so the no-results mount comparison
also includes that behavior change. This is not a claim of identical local focus state or a
pixel-match oracle. No image was resized to simulate Android, and no golden/threshold was changed.

## Verification ledger

Environment: macOS arm64, Gradle 8.14.3, JetBrains JDK 21.0.11, Rust 1.98.1; pinned Kotlin
2.2.20/CMP 1.9.0/Material3 1.8.2. No toolchain was installed. Commands run from the new worktree;
full outputs remain in the ignored run directory.

| Check / scope | Result and meaning |
| --- | --- |
| Base + new `DiscoveryParityTest`, focused Gradle `:preview-sandbox:desktopTest --tests com.cantos.theatre.app.DiscoveryParityTest` | 5 selected, 3 FAIL (continuation, pill geometry, empty Retry), 2 PASS; 112 baseline captures generated |
| Changed focused `DiscoveryParityTest` + `DiscoveryPresentationTest` | 11 tests PASS at the first repair checkpoint; 112 after captures generated |
| First `cargo xtask theatre-preview verify` | FAIL compiling two new test imports; a test defect in this patch, corrected without relaxing assertions; original log retained |
| Final `cargo xtask theatre-preview verify` | PASS: shared 575 tests (3 skipped), sandbox 162 tests (2 skipped), zero failures/errors; full log `theatre-verify-final.log` |
| `python3 scripts/localization_catalog.py --resources` | Regenerated canonical resources; no manual generated edits |
| Changed-only preflight | FAIL at repository qualification; the 50 diagnostics are byte-identical on a clean detached checkout of the pinned base. Later gates NOT_RUN by the stop; final PR lists independent results and remaining gates |
| Resource validation/localization tests and Rust format | `--resources --check` PASS; `python3 -m unittest discover -s scripts -p 'test_localization*.py'`: 89 tests PASS, 2 skipped; `cargo fmt --all -- --check` PASS |

An intermediate full run failed one old owner-flow expectation that waited for a Loading Retry.
That test now requires actual loading feedback, no Retry and no empty artwork while retaining
the query; this follows the supplied state contract. Assertions were strengthened rather than
suppressed. Its failed output remains in `theatre-verify-repaired.log`.

Focused regressions additionally check the whole pill by real pointer input, its localized
single accessible action and lack of editable descendants; unchanged raw query/navigation on
Back collapse; input focus; loading/error absence of a completed zero-count announcement;
partial/loading/failed footers retaining rows; and existing query-on-change, real submit,
Clear Search and card identity. Existing owner-flow tests exercise production controllers over
the sandbox catalog/media/Library ports. No test queries a live API.

| Layer / claim | Evidence and limit |
| --- | --- |
| L1 ownership/dependencies | Source review: app composition, existing query/progress/navigation owners; no dependency edge or shared fork |
| L2 state projection | Existing closed catalog modes/phases retained; presentation distinguishes empty/loading/failure/retained/partial; durable domain policy is not changed |
| L3 regression/static | Failing-before/passing-after production-composable assertions and generated-resource drift checks; Kotlin lint/formatter has no configured gate, so touched Kotlin is manually reviewed |
| L4 app flow | JVM semantics/interaction and existing owner tests over named sandbox external ports; no production fallback |
| L5 visual/native | Opened JVM pixels support only named layout observations; Android/iOS, OS IME/insets/Back, TalkBack/VoiceOver and real catalog journeys remain NOT_RUN |

## Review and remaining acceptance

Self-review repaired the long English invitation and the new test API imports. The subsequent
read-only `cantos-code-review` pass inspected the whole diff, generated outputs and new tests,
plus callers, the catalog projection, query/submit owner, Back overlay, header and shared field.
Coverage is complete for this declared app/source increment. No newly introduced actionable
source defect was found. Assessment: `no-actionable-findings` for the bounded source increment;
`insufficient-evidence` for full Android design acceptance. This is not an approval.

| Review claim | Required by / owner | Held evidence / disposition |
| --- | --- | --- |
| Continuation derives from actual progress; query/navigation retain one owner | #453 D02/D03; CMP KT-STATE/ARC | Source trace and production-composable regressions; `held` for source/JVM |
| Empty, loading, failure and retained/partial content stay distinct | #453 D04; UI-STATE | Exact state/action and owner-flow assertions, 737 tests selected; `held` for source/JVM |
| Canonical localized copy and existing controls are reused | ADR0050, UI-COMPONENT/UI-STRING | Generator check and exact generated diff `held`; inherited stale shared qualification remains `gap-required`, with no receipt modification |
| Visual treatment and large text are real rendered output | UI-PIXELS, #453 D02/D07 | Opened attributed JVM before/after images; `held` only for those named captures |
| Affected installed APK, Android artwork/IME/AT and native design parity | #453 D01/D05/D06/D07 | Missing owner screenshot/device; P2 `required evidence gap`, `confirmed-from-source` from the empty inventory and absent attachment; `gap-required` for native acceptance |

No changed durable record, authorization, API, playback rule, dependency or cache/artifact
fingerprint earns a persistence/publication/formal pass. No such claim is made. The remote CI
result and final preflight are recorded in the PR, independently of this source review.
No merge, deployment or APK distribution follows from these checks.

All AC-CMP-D01..D07 remain unchecked at this source/preview handoff. D02/D03/D04 have bounded
source/JVM progress; D01/D05/D06/D07 still require the specified actual artifact/device and
same-condition Android evidence. Even D04 is not promoted to Android acceptance by a desktop test.

To resume native acceptance, provide the affected screenshot and an attached Android device with
the installed `com.cantos.theatre.dev` or `com.cantos.theatre` build identity. Record package
version/variant/channel/SHA before comparing real catalog states. Then use the existing
`cargo xtask theatre-host android-debug --api-origin <authorized-origin>` build path when a new
test artifact is needed, and separately install/run only the authorized artifact. Walk
320/390dp, light/dark, VI/EN, 200% text, guest/signed-in, keyboard/IME/Back, bottom safe area,
mini-player and TalkBack. iOS requires its named simulator/device separately. Theatre has no
Android static preview wrapper, so shared-component Studio previews cannot substitute.
Native acceptance owner: unassigned — assignment required before acceptance.

Residual risk: unknown installed APK, unavailable owner screenshot, missing Android/iOS and AT
walkthroughs, unobserved real OS motion/IME and live-catalog behavior. Shared/header/artwork
runtime deltas, if reproduced, return to #452/#532/#417 instead of being copied into this repair.
