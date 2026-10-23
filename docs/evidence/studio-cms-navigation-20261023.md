# Studio CMS bounded read navigation

Source-bound follow-on to [PR410's CMS evidence](studio-cms-editorial-20261023.md), not a new
production-readiness decision. Base: `origin/develop@4d31459b3355c8c49def1dbecf0415871b129344`,
the verified merge of reviewed head `c79df5c081cb213d1e3d7704e3fb92c043b73fba`.
Delivery uses a new branch and Draft PR into `develop`; PR410 is already merged.

## Capability matrix at the merged base

Actual source and owner contracts were inspected before choosing this bounded slice. The
earlier results below retain their original provenance; they were not rerun wholesale here.

| Capability | Implemented caller / authority | Earlier evidence and remaining boundary | Follow-on |
| --- | --- | --- | --- |
| Identity/login | Existing Studio cookie/session owner; Identity authentication, Server roster | Real Identity17 and browser login/logout; production admission and HTTPS separate | Reuse; ADR0047 stays Proposed |
| Dashboard/Work | `live/dashboard.rs`, workspace and Catalog commands | Browser create/read/update; bounded forward pages and first-page refresh | Previous/Next over acknowledged pages |
| Unit/Content revisions | Existing Catalog/Content v1 commands and unit-bound drafts | Catalog61 and browser CAS/history; history UI always requested `after_revision=0`, hiding revision21 onward | Unit Previous/Next and accessible first-page refresh; complete history paging |
| Manuscript/Script | Existing import/editor and exact accepted Script binding | Script native/browser r1/r2/pinned-r1; wider import/IME corpus separate | Preserve current-head/history/draft separation |
| Artwork/delegation | Independent Work-owned components and dirty owners | Earlier fixtures/tests; final dirty-owner repair lacked browser acceptance | Scoped smoke only; no authority change |
| Production Brief | Saved Content binding, immutable read/copy/freshness | Native chain and browser Text; Script/Source browser and producer read-grant UI remain | Reuse existing form; no new grant capability |
| Private audio | Existing prepare/producer PUT/status/revoke and exact receipt | Actual native/browser bytes and filesystem; network-loss/expiry matrix remains | Reuse unchanged owners |
| Audio Review | Exact rendition/checksum consent and named decisions | Native/browser named reviews; no human-listened or legal verification | Preserve |
| Release Candidate | Existing create/read and immutable bindings | Native/browser exact candidate; known IDs required after reload | Preserve |
| Clearance/approval | Publication v1 Studio consumer | Browser clearance; approval interruption recovered by exact HTTP operation | Full browser approval recovery remains separate |
| Publication/withdrawal | Explicit actions, immutable receipts, separate current-pointer GET | Browser publish/withdraw and actual Server restart | Preserve; default unavailable delivery stays fail-closed |
| Theatre visibility | Existing public discovery/manifest/media owner | Anonymous loopback full/range bytes and withdrawal; TLS/CDN/device separate | No listener or delivery change |

## Changed invariant and boundary

A failed, cancelled or stale page read cannot advance accepted navigation, replace a draft or
change its CAS base. Work/unit/history reads remain bounded to20 and replace pages rather than
accumulated. Previous reuses visited opaque server cursors; refresh explicitly returns to the
first page. Only a successful correlated read advances its trail. Actor reset clears all trails;
Work and unit changes clear their dependent navigation. History inspection never becomes current
Content or a production binding. A paged unit set remains ineligible for reordering.

`live/pages.rs` owns only app-local read navigation. Existing `Command`, session fence and
request-generation owners still control transport, cancellation, reply admission and actor
isolation. No Server contract, schema, migration, domain gate, dependency or shared UI API changes.
Existing `ActionButton`, toolbar and paired VI/EN copy owners are reused. The Studio surface
keeps its low-intensity, text-first hierarchy; no new motion is introduced.
Pager unavailability has its own app-local copy: an absent direction or pending request does
not incorrectly claim that an unsaved form is blocking it.

Only explicit paging controls request focus recovery. If their pending read leaves focus on
the document body, a successful read focuses its localized results and a failed read focuses
the existing status region. The same request generation, confirmed session, visible pane and
unchanged focus are rechecked on the deferred frame; typing and automatic reads keep focus.

The original checkout was clean, with no stash or unpushed commit, and was safely returned to
updated `develop`. The isolated worktree reuses its reproducible target cache. Only the two
inactive Rust incremental directories were removed: free space increased from about5GiB to14GiB.
`target/native-dev`, DB/WAL, media, private metadata, binaries and earlier artifacts were retained.

## Verification provenance

Scoped logs, fixture helpers and opened captures live under ignored
`artifacts/agent-runs/studio-cms-navigation-20261023/`. The disposable native fixture uses the
existing prebuilt Server/Identity binaries from the unchanged merged backend, new private Turso
files, a loopback SMTP sink and fixture-only Studio roster. Data is seeded through real Studio
HTTP handlers:22 synthetic Works,22 chapters and25 immutable Content revisions. No production
data, provider, deployment or paid service is involved. Fixture admission is not a supported
production onboarding procedure.

The PR records exact checked source identity, commands and independent gate results. Historical
PR410 results remain separate, including its macOS baseline failures and cancelled Rust CI.

| Layer / observation | Executed result and limit |
| --- | --- |
| L1 ownership and independent review | PASS, source-reviewed: final navigation, scoped resets, partial reorder guard, request/focus tickets and paired copy; no actionable source findings |
| L2/L3 deterministic state | PASS, example-tested: `RUSTUP_TOOLCHAIN=1.98.1 CARGO_INCREMENTAL=0 cargo test --locked -p cantos-studio live::pages::`; six tests after the focus repair; final copy is covered by the handoff package checks |
| CSR build | PASS, compiled: `RUSTUP_TOOLCHAIN=1.98.1 CARGO_INCREMENTAL=0 cargo xtask build cantos-studio --web`; later changes rebuilt successfully through the same Trunk pipeline, with final copy build ending at16:13:27UTC |
| Real native HTTP/browser | PASS for the named paging journey: real Identity login/cookie,22 Works/chapters and25 revisions; no fake responses or development bearer |
| History/head/CAS | PASS, interaction-tested/integration-tested: first20 → revisions21–25 → inspect21 with unsaved text retained → append26, preserving the current25 save base; no historical rebase |
| Failure/restart | PASS, integration-tested: stop only the verified loopback fixture Server, request Next, observe error plus unchanged first20, restart same files, explicitly request Next → final six revisions; repeated on final focus source |
| Keyboard/focus | PASS for scoped Chrome navigation: Dashboard Next focuses results heading; Tab/Enter Previous returns first page; history Shift-Tab/Enter Next focuses the localized results list; failed read retained keyboard focus on its retry control. Lost-focus fallback and stale-ticket rejection have deterministic tests |
| Ancillary repair smoke | PASS for named IAB paths: lookup edit/erase and Brief edit/revert release the Catalog guard; simultaneous Artwork/Delegation drafts block it, clearing one retains the other, clearing both releases it |
| Renderer | Opened desktop history/focus,320×720 VI light/dark and200% English dark history captures; long IDs and controls wrap within the available width. No screen-reader, OS IME, reduced-motion or complete app-wide accessibility acceptance |

Browsers: in-app Chromium (version not exposed) for initial interaction/data checks; native
Google Chrome on macOS for final keyboard/focus, failure recovery and layout. No browser
version claim is inferred. The native Chrome screenshot includes DevTools' explicit320×720
viewport and305px body width. No CSS change was needed in this follow-on. Captures and private
fixture metadata are local-only; no manuscript, secret, database or media is committed.

The in-app browser's native discard confirmation could not be surfaced: the click timed out,
`getJsDialog` returned no dialog, and closing that task tab had an uncertain outcome. Accessing
the native ChatGPT app to inspect the dialog was denied by the computer-use tool. No forbidden
operation was retried. The unit-selection/discard browser check is therefore NOT_RUN; the
native Chrome lane above used only settled data and made no claim about that confirmation.

The history captures precede only the final pager-reason wording correction; they establish
wrapping, theme and focus for the otherwise identical composition. Final source checks and a
final-copy observation are reported in the PR. The earlier PR410 audio-player320px recheck is
still separate: this new fixture has no admitted rendition, and history layout does not prove
native media-control layout.

## Remaining acceptance and ownership

- Studio roster provisioning: owner decision under Proposed ADR0047, then a separately scoped
  implementation. The normal local runner cannot provision Studio through this fixture.
- Producer Brief read-grant management/export, Script/Source browser journeys and broader import
  corpus remain independent creator follow-ups. Copying an immutable Brief already exists.
- Account-B/multitab races, actual lost-network mutation acknowledgement, HTTPS/Secure-cookie/CORS,
  production SMTP, TLS/CDN, native devices and named listening remain separate acceptance.
- Browser reload still loses RAM-only drafts/intents after the existing unload guard. This slice
  adds no persistent private browser storage or discovery contract for unknown immutable IDs.
- No publication or rights policy is changed. No release is produced in this follow-on; delivery,
  prior-release recovery and publication fault points are not exercised again.

Queue: no change; the current Studio acceptance boundary and its owner dependencies remain the
same. No issue is closed, and issues416/417 are outside this task.
