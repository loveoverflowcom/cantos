# Generated Theatre account identity: source and renderer evidence

Scope: #670 and #662, one draft PR initially pinned to `develop@e09bd769c1f1ef196150caf0cff98d3e00f87c9a`, then integrated with `develop@0901b20b9a97d3b10780bef91ec93e1c3b43f13c` after upstream #671/#667 merged. Later integration evidence supersedes earlier snapshots below.
This record accepts no deployment, real user/browser/device flow or shared UI qualification. The later Linux router/Identity CI evidence below is separate from those boundaries.
The [contract](../../../services/cantos-server/contracts/theatre/account-presentation/v1/README.md)
owns the recipe and boundary. The work queue is unchanged: these issues remain the execution
owners. No migration, auth/session format, mailbox proof, profile CRUD or photo source changes.

## Source and architecture

One Server recipe computes a frozen V1 pseudonym and opaque display seed from the durable
actor. The existing Listening application service projects it; HTTP only admits and renders.
Web/CMP consume the same response and shared Pair initials. Web binds the payload to the
existing account generation; CMP checks current intent, exact admitted grant and expiry before
publishing an asynchronous display result. Display failure grants/revokes no authority.
Guest geometry remains unchanged. Personalized profile/navigation use 64/48-unit circles and
the existing Lavender token pair. Gold and stable personal tones are diagnostics only.

Behavior impact: `cantos.identity.bearer-lifecycle` is an existing draft executable capability;
no lifecycle guard changes. The optional display callback reuses its existing late-result
fence, with a new Kotlin regression. The pseudonym is a pure projection, not a new state
machine. No model expectation/lifecycle was edited. The selected model gate was initially owed; its later model-only PASS is recorded below.
Broad Kani selectors reach unchanged kernels through contract/module paths; the ignored
assurance record gives NOT_APPLICABLE reasons per kernel and defers the model verifier.

## Checks actually run

All commands used the isolated task checkout. Logs are retained in ignored
`artifacts/agent-runs/avatar-670-662/`. The [source manifest](source-manifest.json) binds changed
source bytes to this record; capture metadata explicitly retains its producer's
`source_revision: unrecorded` rather than inventing a revision.

| Check | Result and actual boundary |
| --- | --- |
| Production alias module, narrow Cargo harness importing the actual source with `#[path]` | PASS: independent hashlib golden vectors for four validated actors and replay/reconstruction. `cargo test --offline --manifest-path /tmp/cantos-avatar-core-probe/Cargo.toml`. This is pure-core evidence, not a Server build or signup journey. |
| Theatre Web native suite | PASS: 579 tests. `CARGO_TARGET_DIR=/tmp/cantos-avatar-web-target CARGO_INCREMENTAL=0 cargo test --locked -p cantos-theatre --features preview --lib --config 'profile.test.package.cantos-theatre.opt-level=1' -- --test-threads=2`. Includes canonical Unicode/tone vectors and presentation clearing/out-of-order account switch. |
| Shared Leptos Avatar suite | PASS: 12 tests. `cargo test --locked -p cantos-ui --test avatar` with the same target directory. Includes pinned tone, picture priority, URL refusal and initials. |
| Theatre Web WASM | PASS: `cargo check --locked -p cantos-theatre --target wasm32-unknown-unknown --features preview`. This compiles the real cookie presentation fetch, not just native test code. |
| Theatre CMP | PASS: `cargo xtask theatre-preview verify`: shared 622 executed/pass, 3 configured skips; sandbox 175 executed/pass, 4 configured skips. Native HTTP decoding, two actors, non-cancellable late display response, logout/restore and unchanged logout confirmation are exercised with synthetic ports. No real Identity/device acceptance. |
| Shared CMP UI | PASS: `cargo xtask ui-preview cmp verify`, 320 tests; then only `GeneratedAliasCaptureTest` rerun after selecting a distinct Sky seed for C. Production Avatar over the JVM Compose test root; no native device. |
| Leptos preview | PASS: `NO_COLOR=true CARGO_TARGET_DIR=/tmp/cantos-avatar-wasm-target CARGO_INCREMENTAL=0 trunk build preview.html --locked --dist <task-artifacts>/web-preview`, from `packages/ui-system/leptos`. Loopback IAB inspection of Light/Dark and 320px/200% Dark confirmed ML fits, exact 64/48 diameters and canonical computed colors. No real authenticated account was used. |
| Format/whitespace | PASS: `cargo fmt --all -- --check`; `git diff --check`. |
| Clean-base repository primitive | PASS at the pinned base in a separate local shared clone: `python3 scripts/check_repository.py --base e09bd769c1f1ef196150caf0cff98d3e00f87c9a`. |
| Changed repository primitive | FAIL: stale shared UI qualifications and downstream admission refusals. Newly introduced architecture/resource-owner findings were fixed. At the original e09 baseline this qualification failure was introduced by the avatar diff. The integration section below records the newer base separately; it grants no admission exemption. |

Initial failures are retained: full Server compilation exhausted disk while archiving
`turso_sync_sdk_kit`; an unoptimized Web test binary was corrupted during stripping for the
same reason; a new response test initially used an unavailable `copy` method; one Web regression
caught the accidental guest-size change. The test defect and guest change were fixed, and
their affected suites subsequently passed. Only this task's recreatable Cargo outputs were
removed after retaining logs; other worktrees/data/caches were preserved.

## Opened visual evidence

Synthetic fixture `Mây Lam`, same opaque seed in each direction; C resolves Sky. Each pair
shows profile 64 and navigation 48. Both themes use bundled Noto; narrow captures set preview
text scale to 2.0. Adjacent text names the decorative avatar. No border, shadow or gradient
was introduced. Token contrast pairs remain unchanged. This is qualitative inspection against
the three Drive Me v9.1 images named in #670; it is not a pixel comparison or approved-reference
qualification. Images are documentation-only screenshots and are never packaged by an app.

| Light | Dark |
| --- | --- |
| ![CMP Light, 320dp, text 200%](generated-alias-light-320.png) | ![CMP Dark, 320dp, text 200%](generated-alias-dark-320.png) |
| ![CMP Light, 840dp](generated-alias-light-840.png) | ![CMP Dark, 840dp](generated-alias-dark-840.png) |

All four PNGs were opened and inspected after the final capture. Their adjacent JSON records
retain bitmap size, viewport, density, font source, token digest and PNG digest. No screen-reader
walk, OS reduced-motion test, Android/iOS runtime or real sign-in was performed.

## Remaining acceptance and review

- The initial missing-tool gap was resolved with isolated official Node 24.19.0 (published
  SHA-256 checked), npm 11.9.0 and locked Quint 0.33.0 with lifecycle scripts disabled. The
  selected `cantos.identity.bearer-lifecycle` gate PASSed, model-only. Implementation conformance,
  exhaustive proof and device acceptance remain NOT_RUN. Full preflight now FAILs at the
  repository qualification gate; subsequent planned steps are NOT_RUN, not passed.
- On the initial e09 baseline, shared Avatar's API/source change made 20 existing Leptos qualification records stale and
  triggers downstream two-renderer admission refusals, including the new CMP Avatar imports.
  Keep all existing records unchanged. Re-run/review the required scenarios against approved
  matching references and record actual evidence before adoption/merge. Gallery tests and these
  images do not waive the admission gate.
- Full Server runtime/test linking remains unrun on the Mac due disk, but `cargo check --locked
  -p cantos-server --lib` PASSed on this Mac. The production real-Identity cookie/Bearer router
  journey PASSed in Linux CI on the unchanged implementation (details below). It includes
  presentation parity, no-store, raw-ID absence, mixed credentials and foreign-session refusals.
  A router driven in-process plus a real Identity executable is not an authenticated browser,
  the Server binary over TCP, an external mail provider or a device.
- #662 AC-07 still needs the same real account on a signed-in browser and Android APK/device,
  narrow/large text, Light/Dark and switch/logout/relogin observations. No test account was
  requested or credentials invented. No merge, deployment or APK distribution was performed.
- #670/#662 acceptance remains partial: source integration and scoped synthetic evidence exist;
  shared admission and real renderer/account acceptance are not complete. Do not auto-close.

Read-only review covered all changed source, call sites, fixture generation, dependency direction,
session fences, image optionality, localization and visual evidence. Required evidence gaps above
block ready/merge status. No unrelated product change or new dependency was introduced.


## CI regression repair and later checks

[CI run 41051474595](https://github.com/loveoverflowcom/cantos/actions/runs/41051474595)
checked implementation head `79b5378abe74193f0dcebe61c4f97f208162ead0`:
Web and Forge passed. Rust format, Protobuf reference and workspace lint passed. The actual
Server alias vectors, `real_identity_actor_session_journey` and the production
`catalog_to_playback_to_library_over_the_production_composition_with_a_real_identity` passed;
`theatre_business_journey` reported 2 passed / 2 ignored. The workspace Rust job then failed
only at `design_discovery_preserves_baseline_and_never_promotes_a_proposal_to_an_api`:
left 199 versus the frozen baseline 198. Later job stages were NOT_RUN.

This is an introduced test-oracle defect, not a baseline failure: comparing the pinned base
and this catalog shows the same component IDs and exactly one added state,
`avatar/generated-alias`. The repair adds that named state to the existing explicit Theatre
extension list, checks its presence, and retains the archived v3 baseline assertion at 198.
It changes no product, contract, dictionary, colors, capture or model expectations. The complete
focused gallery suite then PASSed (39/39) with
`cargo test --locked -p cantos-ui --test gallery`. Source-bound implementation results above
remain attributable because only that oracle and this evidence record changed afterwards.

Later local WASM lint also PASSed:
`cargo clippy --locked -p cantos-ui -p cantos-theatre --target wasm32-unknown-unknown
--features cantos-ui/preview,cantos-theatre/preview -- -D warnings`.
The final PR body records the CI outcome on its last head. Qualification records remain unchanged;
the newer-base comparison below distinguishes pre-existing failures from avatar adoption.


## Integration with newer develop

Upstream advanced to `0901b20b9a97d3b10780bef91ec93e1c3b43f13c` by merging #671 and #667
while this PR was in CI. A non-mutating merge-tree identified one actual content conflict in
`NativeAccountController.kt`. Integration preserves the new session-loss/credential-attempt
fields on `Failed` and the optional generated presentation on `SignedIn`. No upstream save,
session-expiry, localization or token change was discarded. This is distinct from qualification.

The additional regression `late_presentation_cannot_replace_confirmed_session_loss_after_revalidation`
drives login, a non-cancellable display request, confirmed Unauthorized session loss, then the late
display result. Pending/Expired and the session-loss fact remain visible; no identity is republished.
The post-integration `cargo xtask theatre-preview verify` PASSed: shared **640 total, 637 passed,
3 configured skips**; sandbox **187 total, 183 passed, 4 configured skips**, zero failures/errors.
`cargo xtask ui-preview cmp verify` also PASSed **320/320**. The four comparison captures were
rerendered with canonical tokens **0.13.0** and opened again. Their PNG hashes are unchanged;
sidecars retain the actual new token digest and `source_revision: unrecorded`. The source manifest
now binds 54 current inputs and records both the initial and integrated bases.

Baseline classification changed with upstream: a clean detached checkout of **0901b20b** already
FAILs the repository primitive with the same **20** stale Leptos qualifications. The integrated tree
has those same 20, plus the Avatar adoption requirements at `AccountScreens.kt` and
`TopBarActions.kt`. This does not waive fresh Avatar/two-renderer qualification. The initial **e09**
base passed both local checks and [base CI 41043154126](https://github.com/loveoverflowcom/cantos/actions/runs/41043154126);
the initial avatar diff's 20 stale records were introduced against that earlier base.
All qualification files and their reference/policy hashes remain unchanged.

Earlier Web native/WASM/loopback inspection, gallery and Server/model results above remain
attributable to their stated source snapshots. The new head is checked by its own CI run, recorded
in the PR body; no pre-integration CI result is relabeled as a final-head result. Local full Server
linking and a new local Web build remain unrun after integration due disk. Browser/device acceptance
and approved matching-reference qualification remain outstanding.
