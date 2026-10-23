# PR73 local browser acceptance — 2026-10-13

Status: **partial; acceptance FAIL**. Actual Studio/Safari flows ran against Axum and fresh
PostgreSQL. Two primary-flow findings and one contrast finding remain open; this report applies
no application fix. Native CMP interaction, browser header observation and the remaining timing/
identity cases are blocked or not run. This is agent QA, not user production/listening acceptance.

## Source, environment and ownership

- PR73: `feat/live-client-boundaries`, HEAD `2605c9ee2861ab3f0ba7b35f668ab2e7b11a09e2`, tree
  `dbc21ad858c3ed16b64a9efee2994c8b9da367a1`; clean throughout UI execution. Fresh fetched remote
  branch SHA/tree matched. Develop remained `7396ede88466ef56dc9e7759956a4072d6672a5b`.
- Worktree `/private/tmp/cantos-live-clients`; original checkout and other worktrees preserved.
  This follow-up changes evidence/planning documents only. PR69 remains untouched.
- Safari **18.6 (20621.3.11.11.3)**, macOS **15.6**, ARM64. CUA native trusted click/paste/keyboard
  and AX observations were used. No browser DOM/CDP surface was connected.
- Local origins: Studio `http://127.0.0.1:8081`, API `127.0.0.1:8080`, Theatre `8082`.
  Trunk's existing same-origin `/api/` proxy and supported xtask launch were used.
- Synthetic development creator alias `alice`, original text and sine fixture only. No production
  identity, manuscript, voice, provider, release, listening approval or persistent secret grant.
- Run started at 01:33 UTC; browser work ended about 02:08 UTC. UI action count is unknown;
 16 captures were retained in addition to transient observations. Local scenario
  `studio-live-boundary-v1` is an unreviewed draft, not a promoted regression baseline.
- Owned `caffeinate -i -d -t 7200` PID 33874 had both idle-system/display-sleep assertions.
  No sleep, lock, password or TLS settings changed.
- Studio WASM SHA256 `0e25ac3c7cb435b273b9cc061cff2183487246dca90667352c3aba8187c34db2`;
  Theatre WASM `b52d759a64e71bcab21d43a6b1c3610bf7cd2b4755efc2f63e2eec62471c65cd`.
  Cached dev builds completed through Trunk at the pinned clean source.

Captures and raw local records remain outside Git under
`artifacts/ui-evidence/2026-10-13-pr73-live/` (root ignore verified). Sixteen unchanged native
JPEG captures were emitted and opened by Codex at capture time; `.jpg` reflects the actual
format. `images.json` records bytes, SHA256, dimensions, UTC capture time and inspector.
No capture is a screen-reader, motion, exact viewport measurement or production audio result.

## Executed matrix

| Cell | Observation / boundary |
| --- | --- |
| Wide light, vi-VN, 100%, pointer + keyboard | Native frame 1621×862; real forms, diacritics, selection and visible focus |
| Wide dark, vi-VN, 100% | Same frame; tonal surfaces and focus inspected; metadata contrast finding |
| Compact dark, vi-VN, 200%, keyboard | Frame 645×795; Safari Page Menu explicitly showed 200%; stacked navigation/help, focus advanced to session summary |
| Compact light, en, 100% | Frame 645×795; localized navigation/status wrapped within the visible first screen |
| Theatre wide light, vi-VN, pointer | Frame 1622×862; real unavailable boundary and separate mock player |

Frame sizes include native browser chrome. Exact CSS `innerWidth`, full deep-form 320px reflow,
text-only zoom and all English/dark form cells were **NOT_RUN**. The narrow first-screen images
show wrapping without visible horizontal page scrolling, not a full #66 reflow acceptance.
Vietnamese used trusted paste after CUA character typing dropped diacritics; no native Telex/IME
or VoiceOver walkthrough is claimed.

## Flow ledger

PASS below means the stated observation passed, not that the whole capability is accepted.

| ID / check | Result | Attributable observation |
| --- | --- | --- |
| C01 fresh source/remote | PASS | HEAD/tree matched fetched PR branch; develop unchanged |
| C02 awake assertion | PASS | `pmset -g assertions` attributed both bounded assertions to PID 33874 |
| C03 backend fixture | PASS | Complete fresh `verify postgres --keep`, actual bytes and socket/API/PG restart assertions |
| C04 clean-session sign-in | PASS | Private Safari window showed alice; Catalog read returned real fixture work |
| C05 existing-profile sign-in | FAIL | Twice: login success/alice, then Catalog 401; F02 |
| C06 explicit Catalog kind | PASS | Explicit podcast selection created private work; episode creation advanced work r1→r2 |
| C07 default Catalog kind | FAIL | Visible novel created story; F01 |
| C08 absent content | PASS | New unit head 404 was shown as absent; append became available from base 0 |
| C09 invalid save | PASS | Empty text 422 with `details.body.text · invalid_text`; no saved success/revision |
| C10 text save + focus | PASS | Option-Tab reached visible Save focus; Space stored exact Vietnamese text as r1 |
| C11 content history/exact | PASS | r1 text remained readable after recording r2; both immutable IDs listed |
| C12 stale save | PASS | Reading r1 then saving against newer r2 returned 409 `StaleRevision`, current revision 2; draft stayed |
| C13 narration Brief | PASS | Profile required explicit selection; version1 created/read, pinned text revision/profile |
| C14 stale Brief | PASS | After r2, Brief 1 showed `content_changed`; prepare disabled |
| C15 recording Brief | PASS | Version2 bound r2; independent owner GET confirmed editorial mode recording |
| C16 file/prepare | PASS | Native chooser selected only owned 6,377-byte Ogg; browser hash readiness and submission ID appeared |
| C17 binary admission | PASS | PUT completed; owner status showed admitted rendition and verified mono/Opus/24 kbps facts |
| C18 exact bytes | PASS | Displayed checksum matched local file; read-only owner oracle asserted SHA256/length/codec/profile |
| C19 review consent | PASS | Confirmation enabled review; changed evidence unchecked it and disabled review |
| C20 named review | PASS | Reconfirmed and stored QC rejected; no rights/editorial/listening approval |
| C21 private candidate | PASS | Create/read same ID; QC/rights/editorial and public-delivery blockers remained |
| C22 reload persistence | PASS | Re-sign-in/select restored r2; exact submission and candidate IDs reopened; file/bearer remained absent |
| C23 outage/recovery | PASS | Stopping owned API/Trunk made Catalog fail; input/facts retained. After restart, Stay on Page + Refresh recovered acknowledgement with input retained |
| C24 visual/focus cells | PASS | Limited cells above were actually inspected; F03 remains; no complete accessibility claim |
| C25 Theatre reserved group | PASS | Probe displayed listener unavailable, no live catalog/player; explicit mock link separately opened labelled simulation |
| C26 Theatre outage | PASS | API stopped: probe said boundary unverified/no mock substitution; no fabricated success |
| C27 CMP desktop interaction | BLOCKED | xtask launched host; CUA inventory saw MainKt, but exact ID and name both returned Invalid app |
| C28 Compose host capture | BLOCKED | xtask printed UNSUPPORTED: official Android CLI absent from PATH; no IDE/device provisioning |
| C29 browser auth headers | BLOCKED | Safari Web Inspector shortcut unavailable; actual Cookie/Origin/Authorization headers not observed |
| C30 timing/identity remainder | NOT_RUN | Delayed pending write cancel, lost-write exact retry/reconcile, concurrent tab writes, same/different actor, expiry and navigation/late-result races not driven in browser |
| C31 Kani execution | NOT_RUN | `cargo kani --version`: no such command; neither harness executed |
| C32 listener/native production | BLOCKED | No listener/public delivery contracts; no live CMP/iOS transport/playback, device or listening acceptance |
| C33 chip metadata contrast | FAIL | Type/revision difficult to distinguish in light/dark; F03 |

Ledger: **24 PASS, 3 FAIL, 4 BLOCKED, 2 NOT_RUN**. Repeated Theatre probe/cancel reset was
observed, but fast localhost responses do not prove a pending-request race. Existing pure/client
and PostgreSQL regressions cover the named timing/idempotency invariants separately; they are
not substituted for C30. UI upload exercised 24 kbps only; the fresh PostgreSQL gate executed
`audio_all_profiles_recording_and_text_admit_real_bytes_replay_and_keep_declarations_separate`.

Authentication inference is narrower than C29: successful owner reads reached handlers using
`cookie_token`, and successful browser PUT reached Audio Lifecycle's `bearer` parser, which
rejects any Cookie header and requires exactly one Bearer Authorization header. Thus cookie-free
producer ingress and cookie-authorized owner ingress were exercised through the real API.
This is inferred from actual success plus the inspected handler contract, not captured browser
headers. The Fetch shell explicitly uses SameOrigin for owners and Omit for producers; direct
browser header/Origin inspection remains blocked and is not reported as passing.

## Opened image records

All records: native CUA JPEG; Codex inspected the emitted image in the same call. Times below
are local file-save UTC, immediately after capture; exact capture/inspection wall times are not separately logged.

| Image / frame pixels / saved UTC | SHA256 of opened bytes | Concrete observation |
| --- | --- | --- |
| `studio-audio-admitted.jpg` · 1621×862 · 01:43:07 | `cb2b6a2188b8efebcb1ddf4585bc72ac8499ade60ea967f19258db6b516acbf2` | Owner JSON shows admitted state and exact rendition binding. |
| `studio-audio-verified-media.jpg` · 1621×862 · 01:43:17 | `496a9f767c2e48cd97ebaed528fdd9b21da1b7070aef26b85cf1c68e2900d1f8` | Verified SHA256/6377 bytes, ogg/opus, mono/48kHz/2000ms/24162bps/profile. |
| `studio-brief-and-audio.jpg` · 1621×862 · 01:40:50 | `77d4652ab8d2a4a540de452c26e1fb3243b1c884e9d84e8a60948c4b11f845a9` | Brief 1 private snapshot above native file chooser; no playback URL/control. |
| `studio-brief-stale.jpg` · 1621×862 · 01:47:16 | `2e55e5cd1e64aff77cf9674dc38f20e849481f10550c20ec2f3a8c7bbea6ca44` | Frozen Brief 1 delivery declarations; freshness content_changed at bottom. |
| `studio-candidate-blocked.jpg` · 1621×862 · 01:44:50 | `5bb98ca4a6b832c963c1e2d3a2eb524fc2521db08e2fdd73b59c38215fc08331` | Private candidate with rejected QC review ID and all four initial blockers. |
| `studio-catalog-401.jpg` · 1621×862 · 01:36:23 | `581e4b3c14541740dd3928b2bd6547d28b6adee9486073693a790471929e7444` | Alice summary and Catalog 401 error; generic disabled action reasons. |
| `studio-compact-dark-200.jpg` · 645×795 · 01:53:50 | `e6074b0fde49bd9c9e6a098853111ead53963ecdec40593f9895b2dcfcc5ddca` | Stacked navigation and wrapped Vietnamese private-boundary help at 200%. |
| `studio-compact-light-en.jpg` · 645×795 · 01:55:20 | `1c7dc35b241be442dc670899b949513d4b5f558c74dfe3eab77523347dafbd5e` | English navigation/status wrap on compact light first screen. |
| `studio-content-keyboard-focus.jpg` · 1621×862 · 01:39:47 | `5c44373d67ae185b13b36b289f4fd57e3343d3d14eaeae584eed3eb0467a451f` | Vietnamese draft/provenance; visible double focus outline on Save. |
| `studio-dark-wide.jpg` · 1621×862 · 01:52:01 | `c78b8ca77e5a4adca1dfa5f6d3bcf60f1193f12478d25d0902bed4786f5d145e` | Dark surfaces/focus; pale metadata poorly distinguished on primary chips. |
| `studio-default-kind-after.jpg` · 1621×862 · 01:50:39 | `fc0544fc5654fff862b7d163f18cad91073720d18ca04d3fe62b2b0fe3242030` | New story chip while untouched Work type selector still says novel. |
| `studio-default-kind-before.jpg` · 1621×862 · 01:50:24 | `f979d78bdb0bf09db65757d76229b413cf1a6a7d442c56a59b571b376221d181` | Original probe fields and untouched native novel selection. |
| `studio-review-consent-invalidated.jpg` · 1621×862 · 01:43:55 | `ef0abfcb6a79445b2d49ff978e3699d1bb05e1cbc4f11ef146375f7ac4ad8f53` | Changed evidence, unchecked consent, review disabled; candidate available. |
| `studio-transport-failure.jpg` · 1622×862 · 02:01:49 | `a827caa8878777420ef38394e1ce16eed5480e0d7315dd18178f8d9b42feaa94` | Interrupted connection error with current actor and real prior Catalog facts. |
| `theatre-live-unsupported.jpg` · 1622×862 · 01:57:22 | `7406e3aa07300c143f4a3db1ded6945414a5b4a95741d659c892d44ea51cf760` | Cantos unavailable message; API probe/cancel and explicit mock link only. |
| `theatre-mock-explicit.jpg` · 1622×862 · 01:57:50 | `4d14b39af810a4a784305d4706361bb82ef0e6943c8d8f1527d75c44c1388f3b` | Mock-only banner, simulated player/seek and no real audio clock. |

## Findings — proposals only

### F01 — studio/work-kind/default-display-disagrees-with-write

- **Bug · P1 · high confidence**: fresh form twice displayed novel; an untouched selector's
  successful creation returned story. The default probe write reproduced this once; source
  separately shows the story signal and first novel option.
- Component/rule/authority: `SelectField` and CreateWork; product contract
  [Catalog creation](../../services/cantos-server/contracts/studio/catalog/v1/README.md#creation-and-partial-update),
  [UI states](../design/ui-system.md#components-and-interaction-states).
- Actual/expected/impact: form says novel, saved immutable kind is story. Selected kind must
  match submitted/stored kind. A creator can silently create the wrong work classification;
  metadata PATCH cannot change immutable kind.
- Repro: fresh page → sign in → Content & audio → Create work → fill title/synopsis/rights,
  leave kind untouched → Create. Explicitly changing the selector worked around it.
- Region/images: wide form kind near x811/y636 and Catalog chip near x725/y580;
  `studio-default-kind-before.jpg` + `studio-default-kind-after.jpg`, both opened.
- Cause boundary: source initializes kind to story while novel is the first option; generic
  `prop:value` does not establish the observed initial selected option. Exact Leptos mounting
  mechanism still needs the fix owner's regression; no framework-wide claim.
- Suggested fix: bind each option's selected state to the typed initial value after options
  exist (or use the existing qualified select adapter). Retain explicit selection behavior.
- Recheck: fresh mount and all 3 work kinds; visible selected option, sent DTO and persisted kind
  agree. A pure DTO test alone cannot prove initial browser selection.
- Related execution: #66/PR73; a separate bug issue was not filed.

### F02 — studio/session/login-success-followed-by-catalog401

- **Bug · P1 · high confidence in the symptom**, cause unknown. It reproduced in the existing
  normal Safari profile before and after API restart; clean private Safari worked throughout.
  Introduction by PR73 is **not established**.
- Component/rule/authority: development sign-in/current Studio transport; product contract
  [live capability boundary](../work-plan/live-client-api-matrix.md) and backend scoped
  cookie/session authorization. No auth or cookie protection was weakened.
- Actual/expected/impact: alice/login-success is shown, then real Catalog read 401 asks for
  sign-in again. A successful sign-in should establish usable current Studio access, or explain
  a failed exchange. The existing profile cannot enter the primary flow.
- Repro: normal existing Safari profile → local sign-in → Content & audio → Refresh works.
  Same synthetic account in a new private window succeeds. Two observations, same profile.
- Region/image: status at x51/y423–697; `studio-catalog-401.jpg` is adequate context for the401;
  AX also records login success. Second AX observation reproduced the state; its extra screenshot
  was NOT_CAPTURED (`noWindowsAvailable` after closing the private window).
- Root cause: unknown. Source rejects multiple `cantos_session` cookie values and the retained
  legacy/current mounts use overlapping paths; that is an investigation hypothesis, not observed
  browser headers. Existing cookies were neither extracted nor deleted.
- Suggested fix: first reproduce with controlled legacy/current cookie coexistence and capture
  only redacted path/name/presence evidence; then choose a safe mount migration/session recovery
  fix. Preserve HttpOnly, SameSite, Origin checks and producer-cookie separation.
- Recheck: clean and migrated profiles, same/different actors, expiry/re-login, denied Origin,
  producer bearer with credentials omit. No clearing the user's cookies to claim a fix.
- Related execution: #66/PR73; a separate bug issue was not filed.

### F03 — studio/catalog-chip/metadata-contrast

- **Contract deviation · P2 · high confidence in visible/source mismatch**. Type/revision text
  is difficult to distinguish on primary chips in both light and dark images.
- Component/rule/authority: `.live-list small`; internal
  [accessibility targets](../../.agents/skills/cantos-ui-design/references/accessibility.md#1-measuring-the-targets)
  and product [accessibility acceptance](../design/ui-system.md#accessibility-and-motion-acceptance).
- Actual/expected/impact: titles use the primary foreground, while nested small text overrides
  it with `on-surface-variant`. Type/revision should remain legible on the same chip background.
  The redundant heading/revision is a partial workaround; kind is harder to inspect.
- Repro: load works in light, switch dark; observed in both wide cells. Regions x66–1040/y580
  in default-after and y800 in dark-wide. Context/detail are visible within these two opened frames.
- Source cause: `studio.css` `.live-list small` assigns surface foreground inside primary buttons.
  Generated token-pair calculation is 1.18:1 light / 1.13:1 dark, **not a browser-resolved color
  measurement**; no numerical WCAG pass/fail claim is based on unavailable computed styles.
- Suggested fix: inherit the button foreground or use a tonal list row with matching foreground.
  Do not retune global tokens to mask an incorrect role pairing.
- Recheck: resolved foreground/background≥4.5:1for metadata, both themes, long Vietnamese,
  200% and focus; inspect the actual resulting pixels and names.
- Related execution: #66/PR73; a separate bug issue was not filed.

## Fixture lineage and permitted mutations

| Record | Exact identity / retained state |
| --- | --- |
| Work | `706115bd-90de-46b7-a38e-d9ce6f3e6840`, private podcast, r4 |
| Unit | `ad353e14-5fb9-4136-9854-b9050486dff1`, episode, metadata r1 |
| Content1 | `e49a77bf-bbc2-4d2a-b4b0-93e590ce53b8`, original vi-VN text |
| Content2 | `110dc105-ad93-4f03-b4d4-b5ad71b9ed32`, recording, current head |
| Brief 1 / Brief 2 | `38f011a3-3717-4071-ba97-1f31613d7388` / `62035829-9c7f-4c01-a381-d33f6f3a5a1f` |
| Submission / rendition | `1b7b1222-b319-470c-8707-120b9d3e2adb` / `99a51d81-d7ca-46ae-999a-15d813938410` |
| Rejected QC / candidate | `8f095ee5-7346-4304-9273-9015dd56e3fc` / `8a4ab0a4-5387-431f-8800-380547c161ca` |
| Default-kind probe | Original synthetic work titled `PR73 kiểm tra lựa chọn mặc định`, stored story r1; UUID not captured |

Rights reference `catalog-rights` is the disposable fixture declaration, not production clearance.
No existing fixture was edited/deleted. The new records were created through actual UI APIs,
not database edits. A local read-only owner oracle fetched the same 7 records and asserted bytes
and mode/freshness; its successful output is separate from browser header evidence.

Tone recipe (test-only, not a backend encoder): installed FFmpeg, lavfi sine 440 Hz, duration 2s,
48kHz, mono, libopus 24k, CBR (`-vbr off`), Ogg. File SHA256
`d050c3569c86c19b10e2a0c754441e0c46cd6d105af0b2cb1fec3a8686135a36`, 6,377 bytes.
Server inspection reported Opus mono, 48 kHz, 2,000ms, 24,162bps, `opus-mono-24-v1`.
No playback/listening was run. After content/brief changes, the same immutable candidate returned
`stale_brief`, `metadata_changed`, `qc_not_approved`, `rights_not_approved`,
`editorial_not_approved`, `public_delivery_not_implemented`.

## Commands and cleanup

Use the environment prefix recorded in [implementation evidence](live-client-integration.md#local-checks).
All commands ran in `/private/tmp/cantos-live-clients` at the pinned source.

| Command | Result / local log |
| --- | --- |
| `cargo xtask verify postgres --keep` | PASS `/tmp/cantos-live-acceptance-postgres.log` |
| `CANTOS_TEST_CONTEXT=audio cargo xtask run cantos-studio --web --test-cluster` | Started/ready, stopped; `/tmp/cantos-live-acceptance-runtime.log`, recovery log |
| `cargo xtask app cantos-theatre --web` | Started/ready, stopped; `/tmp/cantos-live-acceptance-theatre.log` |
| `cargo xtask theatre-demo run` | Desktop host launched/stopped; UI BLOCKED; `/tmp/cantos-live-acceptance-cmp-runtime.log` |
| `cargo xtask compose-preview cantos-theatre --file apps/cantos-theatre/mobile/shared/src/androidMain/kotlin/com/cantos/theatre/app/TheatrePreviews.kt --composable TheatreDiscoveryLightPreview` | UNSUPPORTED `/tmp/cantos-live-acceptance-compose-preview.log` |
| `RUSTUP_TOOLCHAIN=1.87.0 cargo kani --version` | NOT_RUN proof: command absent; no toolchain installed |

Two existing Kani harnesses remain proposed bounded checks:
`completion_requires_current_pending_generation` and
`cancellation_never_wraps_or_accepts_previous_completion`. They cover all u64 generations,
four representative phase classes for completion, unwind 2, no assumptions/stubs. Kani did not
verify Rust, Kotlin, network or apps in this run. Prior implementation tests/preflight remain
at their original source/log pins; they were not silently rerun or inferred from UI screenshots.

Owned API/Trunk/Gradle process groups 35730, 35737, 36381, 36522, 36928, 36935 were stopped and a
read-only process check found none remaining. The fresh cluster
`target/revision-evidence/cluster-1791855221176320000/pgdata` was stopped with pg_ctl; its
disposable data and ignored evidence were retained for a recheck. Only QA tabs/windows created
by this run were closed. Existing browser tabs and cookies were preserved.

Repository/document-delta verification and final remote pin are recorded in the completion
addendum after the report is frozen. No remote CI was checked, no PR merged and no backend
endpoint, app code, token, fixture baseline or expectation was changed to make the audit pass.

## Documentation completion checks

`cargo xtask verify repository` returned PASS with 225 Python tests
(`/tmp/cantos-live-acceptance-repository.log`).
`cargo xtask preflight --changed-only --base 2605c9ee2861ab3f0ba7b35f668ab2e7b11a09e2`
returned PASS repository and rust-format (`/tmp/cantos-live-acceptance-doc-preflight.log`).
That base is the previously verified PR73 head: this audit's delta is documentation only.
The full implementation's 16-gate preflight, WASM, Kotlin/JVM/Android and independent review
remain at the source/log pins in [implementation evidence](live-client-integration.md), not new
runtime or Kani passes. Final record-only preflight is rerun before commit/push; its result and
the resulting remote SHA/tree are also returned in the task handoff.

## Next recheck

Fix F01 with a failing browser-default regression, investigate F02 using controlled cookie
fixtures, then correct F03. Rerun the same local scenario and retain before/after image records.
Complete browser header, pending-write loss/cancel/reconcile, actor/expiry and concurrent-edit
cases; extend exact 320 CSS px/deep-form reflow and named screen-reader coverage.
Listener identity, immutable publication/public delivery, iOS concrete transport and actual native
playback remain backend/platform prerequisites. Never use Studio private owner APIs as listeners.
