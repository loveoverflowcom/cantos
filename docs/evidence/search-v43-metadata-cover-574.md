# Search v4.3 and metadata-only cover proposal

Source checkpoint: 2026-10-28; base `6234edd27db1421161053540a860bc5b610bb292`,
branch `feat/search-v43-metadata-covers`. This is source-bound development evidence, not
production acceptance or an accepted amendment to ADR 0033.

Integration update: while verification ran, develop merged #576. The branch was rebased onto
`231b559ca9b39eefd41a6acff2976bcb2ecbeb8d` to resolve localization output conflicts. All ten
upstream Studio keys and all four Search keys were retained with identical values, then the
canonical generator was rerun. Hash comparison of every previously changed file found only
the catalog and its seven generated projections/manifest changed; the Search/cover production
and test inputs stayed identical. Passed component/cover evidence is reused for those unchanged
inputs. Generator/format checks were rerun; post-rebase full preflight remains UNSUPPORTED
at the missing locked Quint prerequisite. Upstream Studio implementation changes are not
claimed locally requalified by the earlier Studio/workspace checks; final-head CI is separate.

## Scope and invariants

The shared FloatingSearch shell owns the stable header, fill, perimeter and expandable body
on Leptos and CMP. The app supplies its existing results and states. Web retains explicit
submit, composition guards, query admission and URL/history; CMP retains its Search route,
query-on-change controller and real submit intent. Collapse retains the raw draft and restores
focus; clear retains focus. The body scrolls inside the measured remaining viewport. There is
one trailing magnifier and no autocomplete endpoint, microphone or demo suggestion service.

The server proposal requires a distinct, immutable image consent bound to the exact Work
identity, metadata revision, asset digest, expiry and private evidence reference. Existing
release-only consent is never widened. A valid representative release takes precedence,
including when its pinned cover is absent. Private replacement, clear, expiry, revocation and
metadata withdrawal cannot expose an unapproved image or keep an old URL authorized. The
ordinary release-pinned projection and delivery admission remain in use.

Migration 28 creates only an append-only binding table; it backfills no grants or sample data.
The listener capability defaults off, and `record_metadata` has no production HTTP route.
See the [proposal contract](../../services/cantos-server/contracts/studio/cover-disclosure/v1/metadata-proposal.md)
and [ADR 0033](../decisions/0033-release-pinned-public-cover.md).

## Reference and asset inspection

Fresh issue #453/#574 and PR #487/#488/#555 were read, along with ADR 0033/0051. The Search
v4.3 handoff text and `02-desktop-docked-dark.png` were downloaded from the issue's Drive
references and opened. Other reference images and interactive design motion were not all
inspected; this is not a claim of complete visual or motion qualification.

All six original generated PNGs were found in the local task-13 fictional sample, matched by
manifest key to immutable Work identity, and checked for hash, byte length and 1024×1536 size.
No image bytes were copied into Git or uploaded. The manifest expressly says public-cover
policy and production disclosure have not been approved. Metadata approval supplies no image
permission. Actual per-image public display approval/expiry and a reviewed non-login operator
intake remain outstanding; no human owner was impersonated.

## Verification checkpoint

Local command output is retained outside Git in the task-4 workspace. The final PR report
records the final commit and final gate outcomes; early failures below were not erased.

| Layer | Observation |
| --- | --- |
| Pure cover core | 15 public-cover kernel tests passed, including exact metadata revision, wrong image, tombstone, expiry and revoke cases. |
| Real SQL/HTTP/private storage | 9 focused cover tests passed with `TMPDIR=/private/tmp`: authenticated owner/replay binding, append-only constraints, real decoded fixture bytes, presentation isolation, late revoke, half-open expiry and metadata withdrawal. One explicitly ignored loopback browser fixture is excluded. |
| Shared CMP | `cargo xtask ui-preview cmp verify` passed, including raw query/focus/clear/collapse and stable header bounds on light/dark JVM renders. |
| Theatre CMP | `cargo xtask theatre-preview verify` passed with the added Search captures and collapse/raw-query/focus assertions (575 shared tests, 3 skipped; 155 sandbox tests, 2 skipped). Preview ports are declared mocks around the production composition, not live/native acceptance. |
| Web | Final Web/shared module suite passed: 543 application tests, all shared component suites and 11 combined doctests. Actual production Web composition was driven against disposable Turso HTTP with the original six fictional metadata Works and no audio or image consent. |
| WASM/build | Theatre Web dev build passed. WASM clippy passed with `-D warnings` for shared UI, Theatre and Studio, including preview features/all targets. The transitive future-incompatibility notice is retained in the command output. |
| Preflight | The unskipped command was UNSUPPORTED before execution because the locked Quint tool was absent. A behavior-skipped attempt failed repository qualification. An independent attempt passed format, Script IR reference, Protobuf reference and workspace Rust lint; workspace test linking failed with disk-full `errno=28` before the full suite ran. Remaining gates were NOT_RUN, never a full PASS. |
| Baseline | A separate read-only-purpose local clone at the exact base reproduces stale Leptos qualification records and app admission errors. No qualification hashes were rewritten to manufacture acceptance. The added dock-specific clipping and expanded-state stroke assertions passed in the shared suite. |

The first Gradle attempt was sandbox-blocked at its local socket; an approved local run then
found JBR 25 incompatible. Existing JDK 23 ran the two CMP suites successfully. Existing nullable
release fixture assumptions and two obsolete cached catalog wire fixtures were repaired only
inside preview/tests so the production composition could be exercised. The first SQL run used
macOS's symlinked `/var` temporary root and was refused by private-storage admission; the same
tests passed with canonical `/private/tmp`. These are environment/fixture diagnoses, not live
service failures or approval denials.

Reproduction commands (repository toolchain pins apply):

```sh
TMPDIR=/private/tmp cargo test -p cantos-server --lib contexts::media::turso::cover::tests --locked
cargo test -p cantos-server --lib contexts::media::domain::public_cover --locked
cargo test -p cantos-ui -p cantos-theatre --features cantos-theatre/preview --locked
cargo xtask ui-preview cmp verify
cargo xtask theatre-preview verify
cargo xtask build cantos-theatre --web
cargo clippy -p cantos-ui -p cantos-theatre -p cantos-studio --all-targets --target wasm32-unknown-unknown --features cantos-ui/preview,cantos-theatre/preview --locked -- -D warnings
cargo xtask preflight --changed-only --base 6234edd27db1421161053540a860bc5b610bb292
```

The CMP runs use the already installed JDK 23 and Gradle 8.14.3 with a task-local Gradle home.
The five skipped tests are the opt-in live bookmark, live public catalog, live authorized
Library, live Desktop Preview/server and desktop-window Escape tests. None is counted as PASS.

Inspected JVM captures and their original sidecars are retained below. They use existing
preview catalog/image ports (including the existing `Đèn Khuya` fixture), not any of the six
metadata-only Works' actual images. Their sidecars retain the observed dirty-source label;
packaging does not rewrite them into a later capture claim.

- [390 dark / Vietnamese](search-v43-metadata-cover-574/issue453-search-dock-390-dark-vi.png)
- [1100 light / English](search-v43-metadata-cover-574/issue453-search-dock-1100-light-en.png)
- [320 dark / English / 200% text](search-v43-metadata-cover-574/issue453-search-dock-no-results-320-dark-en-text2.png)

## Browser observations

IAB browser screenshots were opened and DOM/accessibility measurements recorded during the
session. Actual viewports included 1280×720, 1440×900, 390×844 and 320×568, with Vietnamese and
English, light and dark observations. This is a bounded sample, not the Cartesian matrix.

- Typing a padded Vietnamese draft did not change the URL or invent suggestions; the dock
  showed the currently applied six real API rows. Existing query admission trims on submit.
- Enter applied a query and the actual no-results state; Back/Forward restored applied URL
  and input state. Blur/outside dismissal retained the draft without submitting.
- Collapse kept the query and focused the same input; clear kept focus and input geometry.
- ArrowDown focused the first real result navigation button. Closed content is inert on Web
  and absent on CMP. Native browser search decoration was disabled to avoid a second clear.
- At 390×844 the dock bottom measured 844; at 320×568 it measured 568, with a larger scroll
  extent than body height. This verifies viewport clamping, not a real OS keyboard.
- The final built Web source was rechecked at 320×568: Collapse wraps as a complete control,
  the settled dock bottom is 568, the same input remains 72 high, and collapse retains the
  exact padded query with focus. CMP's trailing action inset is derived as (72−52)/2 = 10.
- Loading and actual HTTP error/retry were observed while the disposable fixture restarted.
  Empty/no-results are kept distinct in the existing real state projections.

## Acceptance still open

- Actual Android/iOS host, native IME and screen-reader acceptance: NOT_RUN. JVM tests do not
  establish those results. Composition/229 guards have unit coverage, not a real IME run.
- Motion recording/frame/reversal qualification and full renderer admission refresh: NOT_RUN.
  Reduced motion is exercised in JVM tests; CSS uses the existing reduced-motion preference.
- Actual six-cover browser/CMP public delivery and operator rights workflow: NOT_RUN, pending
  the explicit image policy/permissions. Synthetic fixture image tests do not accept real art.
- Enabled-policy coexistence of a metadata disclosure and a valid release in one integration fixture: NOT_RUN. Existing default-policy release regression tests passed; release precedence is also checked in source and revalidated by the metadata fetch admission.
- Turso latency/throughput campaign: NOT_RUN. The metadata path reuses three bounded batch
  cover statements; no cache or measured speedup is claimed.
- Kani and new implementation conformance to Quint: NOT_RUN. The accepted release kernel
  remains separate from this proposed metadata scope.

No merge, deployment, public upload, production consent, audio release or issue closure was
performed. Work queue order is unchanged: these are the two explicitly selected existing
issues. The known prior loss of task10/cantos-531 `target/verification` evidence was not
reconstructed or reused as evidence here.
