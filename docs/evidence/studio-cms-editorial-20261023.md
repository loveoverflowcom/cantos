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
