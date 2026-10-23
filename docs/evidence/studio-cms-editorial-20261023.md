# Studio CMS Catalog and Editorial checkpoint

Source-bound Part 1 evidence on macOS arm64, Rust 1.98.1 and Trunk 0.21.14. This records
local implementation and acceptance, not production provisioning or release approval.

## Source and delivery

- Branch: `feat/studio-cms-editorial-20261023`.
- Fetched base: `origin/develop@4a52935a598b0f53ec659bf8fb45d1730cc87e8a`.
- PR #409 was read as open/unmerged, head `d069c389`; none of that release diff was used.
- The user selected a Draft PR for Part 1. Part 2 continues the same branch and PR in a separate
  work chat and owns the final combined preflight. No merge, deployment or production action.
- Cleanup found no concurrent Cargo/Trunk/Cantos process. Only verified reproducible Cargo
  outputs were removed (about 1.2 GiB); disk changed from 23 to 24 GiB free. All six existing
  `target/native-dev` database/WAL/owner hashes remained unchanged. Other tasks' artifacts,
  stashes, commits, global tools/caches and source assets were preserved.

## Invariants and implementation

App-local pure draft owners retain creation fields and per-unit metadata/content. A save
acknowledges its exact operation-bound full form snapshot; later typing survives. Work/unit
metadata and Content keep independent revision bases. Refreshing a list or inspecting history
cannot silently rebase a dirty draft. Explicit reload is a separate confirmed intent.

Catalog navigation uses the existing Editor/session/receipt owners. A settled accepted Script
export can be explicitly pinned into Content; dirty/unresolved drafts cannot. Opening a binding
reads its exact revision and checks the export digest. Source/Script content never implicitly
converts into empty text. Work title/synopsis edit, soft archive and complete-page unit reorder
reuse the existing commands. No service, schema, wire contract, authentication policy or
publication rule changed.

The scoped component reuse is Studio `ActionButton`, existing native fields/selects/details,
existing confirmations and shared shell/navigation. No shared token/component changed. Surface
intensity remains low; vi-VN/en labels belong to the existing `live/copy.rs` owner.

## Executed evidence

Full logs and disposable fixture helpers are ignored under
`artifacts/agent-runs/studio-cms-part1-20261023/` in the Mac checkout.

| Layer / claim | Result and evidence |
| --- | --- |
| L1 ownership and source review | PASS, source-reviewed. Existing client/domain/adapter boundaries; independent final source review found no actionable issue after the Work CAS, creation-discard, unit-base and exact-ack repairs. It ran no builds or browser tests. |
| L2/L3 Studio transitions | PASS, example-tested: `RUSTUP_TOOLCHAIN=1.98.1 cargo test --locked -p cantos-studio`: 312 tests and 2 compile-fail doctests. Includes per-unit draft retention, independent metadata CAS, initial reads, typing during save, exact form acknowledgement, history/head isolation, Work refresh/reload, bounded reorder and exact Script revision/digest. |
| L3 WASM | PASS, statically-checked: `RUSTUP_TOOLCHAIN=1.98.1 cargo clippy --locked -p cantos-studio --all-targets --target wasm32-unknown-unknown -- -D warnings`. Existing dependency future-incompatibility notice is separate. |
| Web build | PASS, compiled: `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask build cantos-studio --web`. The later fieldset CSS/class repair was compiled by the same Trunk pipeline for the browser recheck. |
| L5 native Identity + Turso | PASS, integration-tested: `RUSTUP_TOOLCHAIN=1.98.1 CANTOS_TEST_IDENTITY_BINARY="$PWD/target/debug/cantos-identity" cargo test --locked -p cantos-server --test identity_password_turso`: 17 passed. Actual Identity executable, Argon2id, private UDS, local SMTP and file-backed Turso, with development sessions disabled in this test composition. |
| New native chain | PASS: `identity_studio_catalog_content_and_script_receipts_survive_restart_without_cross_actor_access` earns two real Studio cookies, creates Work/unit/text/Script/managed binding, compares independent canonical bytes/digests, denies a rostered stranger, reopens the authoritative file, replays all five original mutation receipts with no duplicate rows, and isolates logout. This is router reconstruction/file reopen, not an OS-process restart. Replays model reconciliation but do not inject a lost network response. |
| Browser with real services | PASS for the journeys below, interaction-tested/integration-tested. Native Server and Identity binaries, two disposable Turso files, local SMTP and real HttpOnly cookie; no fake HTTP response or development token. |
| Additional existing native adapters/recovery | PASS, integration-tested: `RUSTUP_TOOLCHAIN=1.98.1 cargo test --locked -p cantos-server --test catalog_http_turso --test ingestion_turso --test editorial_recovery_turso`: 60 + 6 + 2 passed. These existing suites use explicit synthetic development authority, real Turso/HTTP and an actual Server-process recovery test; they do not replace the real Identity row. |
| Final formatting/lint/build checkpoint | PASS: touched-file `rustfmt +1.98.1 --check --edition 2021`, `cargo clippy --locked -p cantos-server --test identity_password_turso -- -D warnings`, final Studio WASM clippy and `cargo xtask build cantos-studio --web`, all with Rust 1.98.1. |
| Final combined preflight | NOT_RUN by the user's split-delivery instruction; Part 2 owns it on the final combined diff. |

Two new test comparison type errors and a missing locale key initially blocked compilation;
they were corrected before the passing runs. Trunk startup first refused inherited `NO_COLOR=1`
and an incomplete proxy CLI argument. The actual launcher used `NO_COLOR=true` and the existing
proxy configuration. Neither failure was treated as a runtime pass.

## Browser journey and renderer observations

Browser: in-app Chromium on the Mac (exact version not exposed). Local Studio
`http://127.0.0.1:8083`, proxy to the disposable Server on `18080`; unrelated `8081` was untouched.
The ignored Trunk config changes only this fixture's port/proxy. Its real command, from Studio Web:

```sh
env -u CANTOS_API_ORIGIN NO_COLOR=true RUSTUP_TOOLCHAIN=1.98.1 trunk serve \
  --config /Users/manhblue/Documents/Codex/2026-10-23/task-6/cantos-firebase/artifacts/agent-runs/studio-cms-part1-20261023/Trunk.browser-fixture.toml \
  --locked
```

1. Real Identity login opened the Writing desk; Create Work and two ContentUnits used the real API.
2. Work synopsis and selected-unit title edits saved. Text/provenance saved independently in both
   units. The second unit began empty; it never inherited the first unit's manuscript.
3. An unsaved unit-one draft survived switching to unit two, returning, opening the dashboard and
   Continue. After saving it, inspecting Content r1 left current form bytes/base intact; the next
   append produced r3 rather than a stale-base conflict.
4. The repository-authored Script fixture was validated and saved as r1 in the existing Editor.
   Its exact export was pinned into Content r4. A trusted structured dialogue edit then saved
   Script r2. No production/right approval was inferred from validation or storage.
5. The fixture controller terminated and restarted its own Server process on the same file.
   Full browser reload and explicit Script selection read r2 with the edited Vietnamese dialogue.
   Selecting the Work/unit and Open Editor then read pinned r1 with the original dialogue, while
   the authoritative Script head remained r2. Reload reconstructs selection through existing
   controls; no private draft or identifier is silently persisted in browser storage.
6. Import uploaded an original 128-byte UTF-8 fixture. The real response showed preserved source,
   two extracted paragraphs and SHA-256 `2ebc0cbcefdb1ce84b17420033ab3582e7b54b3e52a29637786563d6f8f96758`.
7. With a dirty unit-two draft, logout hid the private workspace. Same-owner sign-in restored the
   exact draft; switching vi-VN/en retained it. A final save and logout left no signed-in tab.

Opened browser captures: default 1264×712 light Script editor, and 320×720 dark Script/Content
forms. Keyboard Tab moved from Script ID to its read button with a visible focus ring. At 320 px,
the new fieldset originally overflowed around a long digest (document width 775 px). The same
fixture/theme/viewport recheck after scoped CSS showed document width 305 px and Content fieldset
241 px inside a 320 px viewport; text, identifiers and actions wrapped without horizontal overflow.
Screenshots were opened in the browser tool session; they are not committed. No new animation or
performance/cache claim is made. Debug Leptos tracking warnings were observed in existing shell
callbacks; no runtime failure was established from them.

## Capability matrix and decisions

| Capability | Implemented / wired | Current evidence / gap |
| --- | --- | --- |
| Login and Writing desk | Yes, existing owner reused | Real Identity/browser PASS; production domains/TLS separate |
| Work create/read/update/soft archive | Yes | Browser create/read/update; native Catalog archive/replay tests remain their own evidence |
| Unit create/read/update/soft archive/order | Yes | Browser create/read/update; reorder refuses incomplete pages; renderer archive/order acceptance remains separate |
| Manuscript import | Existing owner reused | Real browser TXT import PASS; DOCX/Markdown corpus checks separate |
| Immutable Content save/head/history | Yes, draft/CAS repairs | Real native and browser PASS for the named paths |
| Script validate/save/history/reload | Existing owner plus exact Catalog handoff | Real native/browser validate/save/reload/pinned-r1 PASS; revision conflict/request races covered in pure tests |
| Logout/account isolation | Existing session owner reused | Real native two-actor denial/logout and browser hidden/restored-owner draft PASS; browser account-B/multi-tab race NOT_RUN |
| Studio roster provisioning | Proposed ADR 0047 only | Production onboarding remains blocked on owner decision; no policy implementation added |
| Audio intake/brief/review/publication | Existing owners, excluded from Part 1 | Part 2 review/acceptance; no generation/provider/deployment work |

Safe provisioning directions remain distinct: (1) fixture-only admission for disposable acceptance,
used here and unusable as production onboarding; (2) ADR 0047's proposed Identity-owned offline
lookup plus explicit Server roster, requiring owner acceptance before implementation;
(3) a future audited Admin roster, deferred until native Admin verification exists. No email,
subject, password, cookie or proof secret is part of this record or the committed evidence.

## Remaining acceptance and continuation

- Part 2: continue this branch/PR, preserve shared shell/client/session ownership, and run
  `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask preflight --changed-only --base 4a52935a598b0f53ec659bf8fb45d1730cc87e8a`
  once on the final combined diff. No source from the unmerged release PR is a prerequisite.
- Owner decision: accepted Studio provisioning and supported local authorized Studio runner.
  The browser fixture uses documented out-of-band lookup in disposable stopped Server data only.
  Its HTTP development mode leaves development-token compatibility enabled but unused; none were
  created or used. The native integration test separately disables that compatibility.
- NOT_RUN: production HTTPS/Secure-cookie/CORS, real SMTP relay, browser multi-tab/account-B races,
  actual network response-loss injection, OS Vietnamese IME, 200% zoom, screen reader and motion
  recording. Pure/fixture evidence does not establish these capabilities.
- Browser reload loses RAM-only drafts/operation identities after the existing unload guard;
  durable client recovery and automatic ID-selection restore were not added.
- No measured performance claim, provider call, audio quality claim, migration, auth-policy
  relaxation, PR merge, deployed host, VPS/SSH or cleanup of user data.

## Local fixture retention

The signed-in browser tab was closed after final save/logout; fixture-owned Trunk, Server and
Identity were stopped. No unrelated process was stopped. The run-owned files are retained in
`/tmp/ccms-0w5bnr4x` and the ignored evidence directory above. The pointer
`browser-fixture-pointer.json` locates mode-0600 private metadata; inspect it locally when
reconstructing this disposable environment and never paste it into a report. The `setup` helper
creates a fresh fixture rather than reopening the old run; the retained old DB/WAL remains
evidence and can be reused only through its actual owner/configuration. No build output or
fixture secret is committed. Final checkpoint disk had about 15 GiB free after rebuilding.


## Part 2: Production Brief, private audio and publication

This continuation uses the same branch and Draft PR #410, preserving Part 1 commit `2045e159`.
A fresh fetch confirmed `origin/develop` and merge-base remain
`4a52935a598b0f53ec659bf8fb45d1730cc87e8a`; no PR #409 release changes were imported.
The Mac had about 15 GiB free before the build. No data/cache cleanup was required.

Contract intake: `CANTOS_PROMPT_INPUT_V1`, TASK work-item; read/review/edit/tests/commit/push and
updating the single Draft PR are authorized. No merge, deployment, production roster, real mail,
provider call or issue closure. ADR 0047 remains Proposed. No new ADR was needed: this consumes
existing stable service-owned contracts and adapters. The client owns its own raw representations.

### Changed claims and owners

| Claim / invariant | Failure mode | Owner / cheapest oracle | Evidence |
| --- | --- | --- | --- |
| Brief uses saved current Content, including Script/Source | Historical inspection or dirty text silently changes producer input | `brief_controls` pure tests; Server revalidates exact bindings | Source-reviewed; checkpoint Studio tests PASS; final gate record in PR #410 |
| Prepare preserves exact Brief rights and upload subject | Editable fields/wrong selected submission move authority | `brief_controls` and `Command` correlation tests | Source-reviewed; checkpoint Studio tests PASS; final gate record in PR #410 |
| Explicit rights/approval/publication actions use current candidate and pointer | A different candidate or stale pointer is acknowledged | Client DTO/command/confirmation/form tests; native HTTP | Source-reviewed; checkpoint Studio tests PASS; final gate record in PR #410 |
| Original publish receipt is separate from current active pointer | A replay after withdrawal falsely announces active publication | `Facts::apply_command`, exact replay tests | Source-reviewed; checkpoint Studio tests PASS; final gate record in PR #410 |
| Actual bytes precede public pointer | Staged or corrupt media becomes listener visible | Production Turso/HTTP/private+publication filesystem chain | PASS scoped regression below |
| Private preview uses owner authority and exact bytes | Producer token or stale owner exposes private audio | Client exact subject/integrity checks; existing native preview checks | Source-reviewed; checkpoint Studio tests PASS; final gate record in PR #410 |

The Web-owned compositions reuse `ActionButton`, native labeled fields/selects/details and token
styles, with low-to-medium intensity for review/publication. No shared component, design token,
service, registered SQL, server DTO, auth policy or publication rule changed. vi-VN/en copy stays
in `live/copy.rs`. The queue now points to acceptance of the integrated path; GitHub retains
execution/issue status.

### Native chain already executed

`RUSTUP_TOOLCHAIN=1.98.1 cargo test --locked -p cantos-server --test catalog_http_turso creator_intake_to_explicit_publication`
passed one new regression over real run-owned Turso, actual ffprobe/FFmpeg-inspected synthetic
Opus bytes, private filesystem readback and the existing real filesystem origin adapter. It
checks exact Content/Brief/intake identity, an unread handler acknowledgement then reopen/replay,
other-owner denial, private draft absence from Theatre, named QC/rights/editorial reviews,
clearance/approval, default delivery refusal, readiness staging without discovery, explicit
publish, Theatre discovery/manifest/grant and full/range bytes, reopen, exact original publish
receipt and withdrawal. Private bytes remain available to the authorized owner after withdrawal.

Its authority is explicitly synthetic development authority. Reopen reconstructs the router/file;
it is not an OS-process restart. Abandoning a handler response is not injected network loss.
The HTTPS origin is a routing identity with in-process request dispatch, not actual TLS or CDN
acceptance. The signal is test-only, with no speech, provider, human listening or quality claim.
An initial new-test assertion expected an old state on prepare replay; the actual contract
preserves immutable submission identity and projects current admitted state. The assertion was
corrected before PASS; no product source changed for that expectation.

Full local logs and fixture helpers are ignored under
`artifacts/agent-runs/studio-cms-part2-20261023/`. Browser fixtures use fresh disposable files,
separate owner-only private/publication directories, local SMTP, the real Identity executable,
and a private random playback key. No secret, fixture DB or generated media is committed.


### Browser and actual process boundary

The Part 2 browser fixture used fresh real Server/Identity binaries from the unchanged backend
source, disposable Turso files, real Identity UDS/Argon2id and a loopback SMTP sink. The synthetic
Studio roster is fixture-only. Studio was served on `http://127.0.0.1:8083` through the existing
Trunk proxy to `18080`; the private and publication roots and playback key were explicit. The
configured HTTPS media origin `https://api.example` was only a routing identity: public media
requests below were dispatched to the real loopback Server, with no TLS/CDN claim.

Executed with native Chrome controls after an in-app-browser confirmation limitation:

1. Identity login, Work/unit creation and Content r1 save. Chrome independently restored the
   saved records. Separate catalog visibility confirmation: Cancel preserved input/status;
   confirming the exact Work r3 changed only visibility to public (r4).
2. Brief v1 froze exact Content r1 with explicit `opus-mono-24-v1`, guidance, attribution and QA
   declarations. UI displayed current freshness plus exact `cc-r1` and `pb-r1` pins.
3. A generated one-second synthetic tone (3,300 bytes) was selected through the native file
   picker, prepared and uploaded through the existing producer-bearer path. UI showed admitted
   private bytes, rendition/profile/length/duration and SHA-256
   `77074ff964c8002bcb161ae48d927bedcb2fc88b37f47ad473ab319bffa5870f`.
4. Explicit private preview fetched and hashed actual bytes under the owner cookie/fence. Native
   media controls moved play → pause → ended at 0:01/0:01. This is renderer playback behavior,
   not human-listened quality, narration fidelity or codec acceptance on other devices.
5. Separate QC, rights and editorial reviews used the exact rendition and named synthetic
   evidence. Changing a decision invalidated its prior checkbox. Candidate creation stayed
   private; readiness separately listed missing distribution rights, approval and delivery.
6. Distribution clearance was confirmed and acknowledged, invalidating cached readiness.
   The owner session became unconfirmed during the later approval confirmation. Same-owner
   sign-in restored form text; current readiness still reported `approval_missing`. Recovery
   replayed the exact original operation `c20700e9-cfc9-4baa-ac18-8d9618543603` and request
   through the real local HTTP owner; it returned201 and readiness became blocker-free.
   This approval recovery is HTTP evidence, not a completed browser-only approval mutation.
7. Browser readiness refresh displayed no blockers. An explicit confirmation named the exact
   candidate/Content/Brief/rendition and expected pointer0/none. Publish returned an immutable
   receipt; the UI still required a separate active GET, which observed pointer1 and release
   `a25f89dc-306d-4c10-8711-7c8ae8d6b552`, manifest
   `rm-r1:sha256:16dc77dc89016019f4cfaaf9abb9e14d702f95608d4941ac10c13d5709381c4e`.
8. Independent actual anonymous Theatre HTTP observed one Work, that release/manifest, a grant,
   the exact3,300 bytes/hash, and `206 Content-Range: bytes 0-31/3300`. Only the owned Server
   process was restarted on the same file/key/roots; browser active GET still observed the
   same release/pointer. This is actual process restart, separate from router reconstruction.
9. Explicit browser withdrawal named the exact release/manifest/pointer1. Its receipt remained
   separate from a new active GET, which confirmed no active release. Anonymous Theatre then
   returned no Works and a new playback grant returned404. History/private files were retained.
10. With an unsaved publication note, creating a second unit asked for discard confirmation.
    Cancel preserved unit one and the exact note. Confirmation selected unit two with empty
    submission/candidate IDs, no prior Brief/audio/status and no publication observation.

Opened desktop Chrome captures covered exact status, preview controls and active-pointer read.
A DevTools responsive320×720 light capture exposed native audio and long identifier overflow.
A scoped Studio style correction was compiled by Trunk; the final same-scenario visual recheck
is **NOT_RUN** (the bounded browser lane ended during session/password prompt recovery). No
post-fix visual acceptance, dark-theme matrix, 200% text/zoom, OS Vietnamese IME, screen reader,
keyboard traversal of every new action or motion recording is claimed. The existing token/focus
styles are reused; those broader acceptance items remain open.

The in-app browser could read pages but its native confirmation became inaccessible; the
interrupted visibility attempt was independently reconciled as uncommitted before Chrome
continuation. No uncertain mutation was blindly replayed with a new operation. A transient host
disconnection also removed browser/execution tools; reconnect confirmed the same dirty tree,
15GiB free and retained fixture files. No cache/data cleanup or remote/production action occurred.
The task-owned Chrome tab and Trunk/Server/Identity/SMTP fixtures were stopped; disposable
files and sanitized readback records remain in the ignored Part2 evidence directory. Temporary
in-app tabs were not retained as deliverables.

### Combined capability and verification boundary

| Capability | Implemented / wired | Actual evidence / remaining boundary |
| --- | --- | --- |
| Part1 Catalog/Editorial drafts, CAS and exact Script handoff | Preserved | Part1 evidence above; independent combined source review |
| Saved current Content → exact immutable Brief | Yes, all existing body modes | Pure binding tests; native HTTP chain; browser Text r1/Brief v1; Script/Source renderer journey separate |
| Private prepare/upload and exact rendition receipt | Yes, existing API reused | Real native HTTP/filesystem and browser actual bytes; producer cookie isolation retained |
| Exact owner private preview | Yes | Bounded client bytes/hash/subject tests; actual browser fetch and native playback of synthetic tone |
| Named QC/rights/editorial review and candidate | Yes | Real native HTTP and browser named decisions; claims never imply legal/listening verification |
| Distribution clearance and separate publication approval | Yes | Client/native tests, browser clearance; approval HTTP exact-operation recovery as disclosed |
| Readiness → explicit publish → authoritative current pointer | Yes | Native filesystem origin chain; browser publish/receipt/current GET; actual Server restart |
| Anonymous Theatre discovery/full/range media | Existing native owner reused | Actual loopback HTTP and filesystem bytes for browser-created fixture; no deployed TLS/CDN or native-device claim |
| Explicit withdrawal / no automatic fallback | Yes | Native tests and browser receipt/current empty read; anonymous absence/grant404 |
| Production Studio provisioning | Proposed ADR0047 only | Owner decision still required; fixture authority cannot qualify production |
| Final combined preflight | Handoff command below | Exact results recorded in PR410; no clean-aggregate claim from focused tests |

Checkpoint commands actually executed with Rust1.98.1:

- `cargo test --locked -p cantos-studio`: PASS340 tests +2 compile-fail doctests at that checkpoint.
  Later created-unit/no-op cleanup regressions belong to the final handoff campaign.
- `cargo clippy --locked -p cantos-studio --all-targets --target wasm32-unknown-unknown -- -D warnings`:
  PASS after fixing a new component visibility lint and clone-on-Copy test warnings.
- `trunk serve --config artifacts/agent-runs/studio-cms-part2-20261023/Trunk.browser-fixture.toml --locked`
  from Studio Web with `NO_COLOR=true`: PASS compilation, including the final CSS correction;
  browser results are separately scoped above.
- The real native creator/origin regression command above: PASS1 test.
- Independent combined `cantos-code-review`: source-reviewed, findings fixed (new-unit discard,
  cross-unit projection clearing, local confirmation cancellation and preview no-op publication).
  The final narrow style/copy delta receives a separate incremental source pass.

Final HANDOFF command, using the same pinned base:

```sh
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask preflight --changed-only --base 4a52935a598b0f53ec659bf8fb45d1730cc87e8a
```

The aggregate result and any available-gate continuation are reported verbatim in PR410. The
Part1 baseline macOS repository failures remain separate from new regressions. Required checks,
merge protection and production acceptance retain their gates. No merge, deployment, real mail,
provider/TTS call or issue closure is part of this execution. The parent coordinates any later
user-authorized merge only after reviewing the actual handoff evidence.
