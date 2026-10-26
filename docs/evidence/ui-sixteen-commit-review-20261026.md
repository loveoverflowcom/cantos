# Sixteen-commit Theatre UI review — 26 October 2026

Source-bound review and repair record, not a deployment or feature-acceptance certificate.
The corresponding Draft PR owns the final head, final-check receipts and CI disposition.

## Scope and selection

Fetched `origin/develop` and created an isolated checkout at
`7277976cb4c7ee16a7256d25bbbebd75b5730813`. The reference checkout stayed clean and unchanged.
The fix branch is `fix/ui-sixteen-commit-review`, targeting `develop`.

Selection: walk reachable non-merge history in `git log --no-merges` order and keep the latest
sixteen commits with substantive changes to Theatre Leptos/CMP presentation, shared UI or
those consumers' contracts/tests. Count constituent commits once, without their merge commits.
This is a source/consumer review, not the last sixteen repository commits or sixteen PRs.
Search v4.3 supersedes prior arrow/voice Search designs.

The selection excludes `d13ae3287240d23f26bf7744b5e1137eb26700d6` (format only),
`70b931cdf74d09c62d3b0903d1997782646bad42` (source-document/release packaging only), OCI and
credential work. `9e1bf7bccbda21f2fcf2cb9425c2b6168b2d150c` changes server-internal storage lanes
and their tests, with no UI or consumer contract change; it is outside this consumer-based rule.
The included clock-skew commit changes the native player consumer contract, not a security expiry rule.
A broader interpretation including server-only delivery changes would produce a different manifest.

| # | Full SHA | Exact title | Touched UI surface |
| --- | --- | --- | --- |
| 1 | `e41a0667b3c97bc1884539340f1c24287bdbb85d` | fix(theatre-web): drop literal accessible-name fallback on search magnifier (#453) | Web Search magnifier accessible name |
| 2 | `f3a6be17cbdf7b3d6b8e030b0d2df76c8b3be3c0` | feat(theatre-cmp): Search v4.3 single magnifier with immediate submit (#453) | CMP Search presentation, submit intent and shell tests |
| 3 | `c67b2088dbed01347fbdacf504fe4ec361ef6510` | feat(theatre-web): adopt Search v4.3 docked shell with one icon-only magnifier (#453) | Web Search form, Clear action and dock CSS |
| 4 | `dabc72ace61bb14b3359eb3c425640d2793e5c90` | feat(ui-system): Search v4.3 docked shell for Theatre Web/CMP (#452) | Shared Web/CMP Search, semantic tokens and generator tests |
| 5 | `50c71563f5c97006eb36790b2ce11ce777fd6288` | fix(theatre-cmp): keep painted-control focus perimeter inside the control (#478) | Shared CMP painted-control focus perimeter and pixel tests |
| 6 | `2fd44ddc552bca5b48979e4da21c5a18fd31083d` | fix(theatre-web): drop the signed-in status line and lead Back with an arrow | Web signed-in menu status and Back action |
| 7 | `7be43154deb330d55102e21f7f6b0a01ae267cbc` | fix(theatre-web): draw one flush focus perimeter on painted controls | Web painted-control focus CSS and tests |
| 8 | `26e42c6eea73a5529ca8225a66e9c91dee60f8e5` | feat(theatre-cmp): add signed-in Account layout and sign-out confirmation sheet | CMP Account, logout sheet, Back overlay and preview tests |
| 9 | `807be13e53714c849ad98ec8bf6024d0b48e2f19` | feat(theatre-web): add signed-in avatar account dropdown and logout confirmation (#473) | Web account dropdown, logout dialog and shell bindings |
| 10 | `b63a99bf21ac4b1c2f33bc59aa04a277efc894f9` | fix(theatre-web): remove Login privacy footnote and adopt v4 floating fields with an inline password eye | Web Login floating fields and password eye |
| 11 | `5fdfa7671670475c5b062dd0ed53504d1bba5bce` | fix(theatre-web): center Discovery empty group and make the locale select a tonal pill | Web Discovery empty state and locale Select |
| 12 | `578e7728b0d731ecda388d4da589649f114b26ba` | fix(theatre-web): give every Discovery card thumbnail one fixed 2:3 frame | Web Discovery public-cover thumbnail frame |
| 13 | `5391649c0ff82df74cfcc56320305561483b56f8` | feat(theatre): show the public cover on Web and CMP Discovery and Work detail | Web/CMP public-cover DTOs, byte pipeline, Work artwork and tests |
| 14 | `31a0c48e4afefb57f93856e7f26c21896c02a32d` | fix(theatre-mobile): tolerate device clock skew in playback grants | CMP playback grant HTTP, expiry/refresh consumer contract and tests |
| 15 | `cae006e109237bac109df106d5d8ef2b1716f855` | fix(ui): preserve Theatre Select error perimeter | Shared CMP Select invalid perimeter and pixel oracle |
| 16 | `759ab18b59596f21e28c70fae45c395a097ca153` | feat(theatre): adopt qualified brand and search compositions | Web/CMP brand, Search and Library empty-state shared adoption |

The selected commits touch 127 unique paths. Reviewers read the affected Web, CMP and shared
component functions, their callers, contracts, source tests and generators. The small generator
version allowlist change and test were also checked. Review is complete for the declared source
slices; runtime, pixels, native targets and integration acceptance remain partial as stated below.
Studio has no selected UI change and is outside this review.

## Findings and disposition

### P1 — Cover cache bypassed current disclosure

Class: confirmed defect; category: security/rule; confidence: confirmed-from-source and
reproduced. Introduced by `5391649c0ff82df74cfcc56320305561483b56f8`.

The original [CoverImages](../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/app/CoverImages.kt)
returned a decoded image for 120 seconds before consulting the byte source and retained a 404
for the life of the process. A load after revocation could therefore reuse a prior authorization;
re-disclosing the same pinned image could never recover from the remembered 404.
[Discovery v1.2](../../services/cantos-server/contracts/theatre/discovery/v1/README.md#public-cover-v12-composition-gated)
requires revalidation before every use and `no-store` errors; [ADR 0033](../decisions/0033-release-pinned-public-cover.md)
keeps the current exact-image disclosure authoritative. Existing cache tests asserted the wrong
policy and were corrected against that independent contract, not against invented new behavior.

Fixed: every load calls the production byte-source port with the exact disclosed request.
Only a fresh `Verified` result permits bounded decoded-pixel reuse. Denials discard pixels and
are never retained. The existing memory budget, read concurrency and constructor are preserved.
The adjacent HTTP outcome comment now describes a current denial rather than a permanent URL fact.
Tests cover immediate revocation/refusal/unavailability, 404 then success at the same URL,
request metadata, exact Work URL, decoded LRU, concurrency and cancellation.

### P2 — Replaced cover/source retained another image

Class: confirmed defect; category: logic; confidence: confirmed-from-source and reproduced.
Introduced by the same public-cover commit.

[TheatreArtwork](../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/features/shell/presentation/TheatreArtwork.kt)
keyed its producer only by SHA. A different Work URL with the same digest or a replacement host
source did not reload; a different digest retained the previous Ready bitmap while loading and
could attach the replacement alt text to the old image.

Fixed: the full immutable cover descriptor and source key recreate the remembered state,
resetting to fallback immediately and canceling the old producer. Production-composable tests
cover same SHA/different Work, a suspended replacement read and replacement source.

### P2 — Enter on password eye submitted credentials

Class: confirmed defect; category: logic/accessibility; confidence: confirmed-from-source and
reproduced. Reachability introduced by `b63a99bf21ac4b1c2f33bc59aa04a277efc894f9`.

The [Login form](../../apps/cantos-theatre/web/src/features/auth.rs) intercepted bubbled Enter
from every target. With nonempty credentials, Enter on the new eye cleared the password and
submitted login instead of activating the visibility button.

Fixed: explicit form submission accepts Enter only from an actual input outside IME composition.
Buttons retain native activation. Both localized eye names are validated before mounting, with
no English fallback or production panic; reveal changes attributes on the existing control.
The password input, secret lifetime and auth effect owner stay unchanged. The deterministic
production-decision regression covers input/button targets, composing input, Space and Escape;
exact Vietnamese/English names and existing form-rendering cases remain tested.

### P2 — Account menu used the CMP corner scale on Web

Class: confirmed defect; category: rule; confidence: reproduced. Introduced by
`807be13e53714c849ad98ec8bf6024d0b48e2f19` in `theatre.css` account-menu and menu-row radii.
The existing `WebShapeIndependence` regression rejected `--cantos-shape-overlay` and
`--cantos-shape-control`: regular Web corners belong to the independently owned
`--cantos-web-shape-*` mapping in `foundation.css.in`, not the canonical CMP density scale.
The app CSS now reads the existing Web overlay/control roles. No token, generated output,
admission record or test assertion was weakened. The focused regression went from 5 tests /
1 failure to 5 passed. Runtime pixels are recorded separately in the PR.

### P1 — Current component admission remains incomplete

Class: required evidence gap; category: rule/evidence; confidence: confirmed-from-source and
reproduced on the untouched base. The selected Search v4.3 shared commit `dabc72ac` changed
tokens/source after the existing qualification; its own message explicitly leaves re-admission open.

The baseline repository gate rejects stale records for `button`, `text-field`, `surface`,
`badge`, `back-button`, `logo`, `select`, `icon`, `status`, `dialog`, `checkbox` and `error-state`.
New Web Login/Search `IconButton` sites also have no two-renderer qualification. This PR retains
that real failure. No digest-only rebind, baseline exemption, weakened validator or fake PASS is used.

A full current requalification needs opened, attributed browser/JVM reference and runtime captures,
keyboard/focus/reduced-motion observations, semantics and per-component qualification records.
Available unit/build/preview results do not satisfy that qualitative acceptance. Shared Avatar
also remains unqualified; current account views use app-owned generic person glyphs. No account
name/photo projection is authorized or introduced by these fixes.

## Assurance ledger

| Claim / owner | Evidence and reviewed boundary | Disposition |
| --- | --- | --- |
| Search v4.3, query/history/API ownership | shared fields/tokens, Web Search and CMP shell; controlled callbacks, separate Clear, one stable right magnifier | No further grounded source defect; fresh component admission and real IME/input remain open |
| Focus perimeter and invalid Select | shared CMP shape/error propagation and pixel-oracle source; Web CSS cascade | Existing fixes preserved; no style churn; runtime qualification separate |
| Web/CMP account/logout | typed session projection, confirmation-only intent, cancel/Back/focus owner, cleanup/retry fences and mocked journeys | No further grounded source defect; real identity/browser/device acceptance open |
| Login typing, secret lifetime and localization | real form decision/SSR tests; no new auth factor, storage or production mock | Enter fixed; actual browser/IME/screen-reader scope is independent |
| Discovery empty/no-match, locale and covers | completed-empty vs failure mapping, public DTO constructors, exact cover request, thumbnail geometry | Cover pipeline/lifetime fixed; no fallback from live failure to mock catalog |
| Native grant clock skew | exact identity/hash binding, strict expiry and refresh cap, existing boundaries/tests | No introduced defect found; no expiry/security relaxation |
| Production vs preview | build feature guards, production composition roots, dependency boundaries; real owners with explicit external doubles | Mock results prove only their named scope; no real API/device promotion |
| Quint SSOT | existing draft `cantos.media.public-cover-projection`; current-disclosure Fetch, revoked/expired/withdrawn runs | Added named same-image restore-disclosure scenario; no changed transition/property or duplicate UI reducer |

## Attributable checkpoint evidence

Commands ran from the isolated checkout with Rust 1.98.1. CMP used installed Gradle 8.14.3,
JDK 23, Kotlin 2.2.20 and Compose 1.9.0. Tool preparation was confined to this task: official
Node 24.19.0 archive checked against its official SHA-256, npm 11.9.0 and locked Quint 0.33.0.
The default PATH lacked Gradle and the default Node/npm did not satisfy the Quint pins;
prepared per-command paths resolved those prerequisites without global tool changes.

| Check | Actual result / scope |
| --- | --- |
| Untouched-base `cargo xtask verify repository` | FAIL: stale qualification and new unqualified imports, before any fixes |
| Auth regression with old production decision | FAIL: 1 selected test, expected non-input Enter assertion |
| Cover regressions with old production source | FAIL: 18 tests, 8 intended assertion failures |
| Corrected `features::auth::tests` | PASS: 16 tests, no failures; native SSR/decision scope |
| Corrected `CoverImagesTest` + `PublicCoverUiTest` | PASS: 18 tests, no failures; mocked byte source/decoder and JVM production composables |
| `cargo xtask verify behavior --capability cantos.media.public-cover-projection --setup` | PASS: typecheck, 17 named runs, sampled simulation, witnesses, negative controls and freshness; model-only |
| Independent final source review | No new actionable defect in the seven source/model/test fix paths; adjacent comment correction applied separately |

The supplemental canonical-temp Python run completed 1,169 cases with 6 failures and 15 skips.
Two findings belonged to this review: the Web corner-role regression above and the explicit
catalog inventory fixture still expecting 16 cover runs after this PR added the seventeenth.
The fixture now expects exactly 17 runs while retaining all six properties, twelve witnesses
and one sensitivity control per property. The remaining failures were two native-isolation
fixture failures (including source-snapshot instability) and two legacy shell fixture failures
(BSD `sed` incompatibility and missing `flock` on macOS). They are reported separately from
source regressions; no real VPS operation was run and no legacy tool/runtime change is made.
The PR supplies the final targeted reruns rather than promoting this failed full-suite receipt.
In the safe continuation, 75 focused UI-token/behavior tests and the two native-isolation
regressions passed. Both native fixtures recovered without a source change; the two legacy
shell/tool failures remain unclosed from the full-suite receipt. No full-suite PASS is claimed.

After unrelated Studio file reads in the Documents checkout reported short reads, the pushed
head was recovered into `/tmp/cantos-ui16-final-495` for the bounded follow-up. Existing source,
evidence and unrelated checkouts were preserved. Both locations refer to the same single PR.

Raw logs/reports remain ignored under `artifacts/agent-runs/ui-sixteen-review/`, Gradle's
`build/test-results/desktopTest/` and `target/verification/behavior/`. The Draft PR reports the
final-input preflight, later scoped checks, content identity and terminal CI results. No stale
receipt is promoted after a source change. Commit/push packaging of unchanged source does not
invalidate its scoped results.

## Deferred acceptance and residual risk

- Current two-renderer qualification remains an actual baseline failure. Next: run
  `cargo xtask ui-preview leptos verify` and `cargo xtask ui-preview cmp verify`, drive current
  gallery recipes in both hosts, open named captures/references, then record genuine
  per-component evidence and rerun repository admission. Next owner: unassigned; assignment
  required before acceptance. Builds alone cannot close it.
- Browser/native keyboard, platform IME, screen readers, real Server/Identity logout and real
  HTTP cover revocation/re-disclosure require their actual targets and fixtures. The PR ledger
  names any later observations separately; no full device/API acceptance follows from mocks.
- Kotlin has no configured formatter/linter gate; conventions review plus compilation/tests
  are reported as such, not as a Kotlin lint PASS. Android/iOS, native playback and secure-store
  process-death acceptance remain outside the executed JVM scope.
- Quint model lifecycle remains `draft`; client-cache implementation conformance, real DB/API,
  object storage and exhaustive proof remain NOT_RUN. Exact Kotlin regressions exercise the
  repaired implementation independently of the model.

No deployment, VPS/SSH/GHCR operation, production data access, cleanup, credential read or
modification to unrelated checkouts occurred. No old #454 scope or broader backend delivery
repair is duplicated. Residual risk remains the named qualification and runtime acceptance;
this review does not authorize rollout or merge.
