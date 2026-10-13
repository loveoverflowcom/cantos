# PR73 browser finding corrections

Scope: the three findings at `7484d75e75ce2554927fe59fde58438e1f410473` on draft
[PR73](https://github.com/loveoverflowcom/cantos/pull/73), branch `feat/live-client-boundaries`,
base `develop`. No publication, TTS, Theatre endpoint or native transport is added.
The original clean checkout and other worktrees are preserved. No ADR is needed: existing
session isolation and semantic foreground rules determine these corrections.

## Invariants and implementation boundary

| Finding / invariant | Confirmed cause and responsible owner | Regression oracle |
| --- | --- | --- |
| F01: displayed kind equals submitted and stored kind | App-local select's early `prop:value` left first novel option selected although form value was story. WorkKindOptions binds option selected to the same value; actor reset uses the single story default | SSR selected option and actual Command JSON for default, all choices and reset; real Safari creation |
| F02: sign-in implies currently accepted owner cookie | Existing normal Safari request sent two `cantos_session` values; parser rejected before a DB lookup. Private control sent one and read Catalog. Successful login retires only the other known Studio cookie path, retaining fail-closed ambiguity handling | Real HTTP/PostgreSQL legacy→current→legacy/current exchange, missing/duplicate/revoked/expired cookie; real old-profile recheck |
| F02: owner denial removes authenticated UI state without losing intent | POST acknowledgement alone established UI actor. Additive current-only GET session confirms the same nonempty actor; current owner 401 clears the visible authentication while retaining draft ownership. Producer capability 401 is separate | Actor equality table; owner/producer/reconciliation error classification and retained pending snapshot |
| F03: metadata uses its containing chip's foreground | App-local `small` overrode on-primary with on-surface-variant. It now inherits the button's semantic foreground, including native disabled state | Paired-token contrast calculation and inspected light/dark/focus/disabled output |

No settled Content, Brief, audio receipt, review or candidate is modified by these fixes.
Domain decisions, DTOs and media/private-delivery rules remain backend-owned. New GET reuses
SessionResponse and the existing PostgreSQL authenticate boundary; there is no new schema,
cache, credential persistence or dependency. Studio remains Web-only.

## Reproduction, local evidence and limitations

Local raw records are ignored under `artifacts/ui-evidence/2026-10-13-pr73-fixes/` and
`/tmp/cantos-pr73-fixes/`. They use synthetic fixture identities and original Vietnamese QA
text. The previous report and all 16 retained images were opened on this Mac before the fix.

Before repair, real Safari/CUA clicks reproduced successful normal-profile login followed by
Catalog 401, and private-profile untouched novel display followed by stored story. Temporary
backend instrumentation logged only Cookie field/session-value counts: normal `1/2`, private
`1/1`. No values, tokens, Authorization contents or private browser data were extracted. The
instrumentation was removed from source before verification. This proves duplicate-cookie
parser rejection for the observed failure; expiry/revocation/transport are distinct tested
classes, not guessed causes for that reproduction. Attribution to a particular historical PR
is unknown. No browser-wide cookie clearing or security setting was used.

The Work option test and overlapping-cookie migration test both failed before repair. The
first fixed PG diagnostic also exposed a Set-Cookie ordering compatibility expectation, which
was preserved; a test-fixture timestamp type mistake was corrected before the final rerun.
Failed diagnostics remain separate logs, never counted as passing evidence.

## Source and environment

Source checkpoint: `66268f11c5f0d8957f03ced4bbb3e865cac8394d`, tree
`fe8016bc892533533ba3929b89abab0365290977`. Evidence below applies to these exact source
bytes, before the separate documentation commit. It does not supersede the original 7484d75
browser ledger or claim whole-product acceptance.

Ignored `source.json` records 12 Rust/CSS file hashes, combined build snapshot SHA-256
`45eb45e8182ee8ec225d7f1ffe291d3f5995cb325a70bd7d008935fde30d087a` and served WASM SHA-256
`4314bf054f664cedb6b4d3324af4fac900cac09df4b745751b052c4ced774cff`. Independent review
also hashed all 13 changed source/contract/test files:
`8706d54dd428b6e7e6a91a03502ab5caa48e9948ed0c17916cfc576b9150599e`.

Runtime: macOS 15.6 (24G84), Safari 18.6 (20621.3.11.11.3), native CUA clicks, typing,
keyboard and menus. `CANTOS_TEST_CONTEXT=audio cargo xtask run cantos-studio --web --test-cluster` served
loopback Studio 8081 / Axum 8080 using the new disposable PostgreSQL fixture at port 50527.
Rust 1.87.0 and existing PostgreSQL 17.4, Gradle 8.14.3 and local audio tools were reused. No inherited
database, external provider, production system or private manuscript was used.

The owned synthetic Ogg Opus tone is 440 Hz, 2 seconds, mono 48 kHz, 24 kbps CBR, 6,377 bytes;
SHA-256 `4b81c35c485d860bcc3d67835ca22efe5c29a547645d9fbc9c2e9be5a96bc52e`.
It contains no generated speech. Listening and production rights acceptance are NOT_RUN.

## Actual Safari rechecks

| Check | Result | Observed outcome / supporting record |
| --- | --- | --- |
| F01 untouched default | PASS | Select shows story before input; creation returns story; actual owner read agrees |
| F01 changed selections | PASS | Native selections and creation for novel/podcast match chips and owner API reads |
| F01 actor reset | PASS | Fresh private window: select podcast with empty fields, switch alice→bob, observe story, create stored bob/story Work |
| F02 existing profile | PASS | Same failing normal profile signs in and reads Catalog without cookie clearing; duplicate-click login/read settle successfully |
| F02 clean control | PASS | New private window signs in and reads actual Catalog with a one-cookie jar |
| F02 revoked session | PASS | Revoke only synthetic alice through actual local API; owner Candidate read 401 makes UI actor unknown, disables actions and retains draft text/facts |
| F02 recovery | PASS | Restore only that disposable session; same-actor re-login preserves stored Vietnamese text, podcast selection and private receipt/candidate |
| Cancel confirmation | PASS | Native re-login Cancel preserves all inputs |
| Reload/recovery | PASS | Stay on Page retains saved bob metadata; actual reload clears authenticated UI; re-login and Catalog return same stored story/r1 |
| Content happy/error | PASS | Head 404 means absent; empty text save 422 preserves input/auth; Vietnamese text then stores immutable r1 |
| Brief/upload | PASS | Narration Brief v1 with 24 kbps profile; native chooser selects tone; producer PUT admitted; actual owner read confirms bytes/binding |
| Review consent/rejection | PASS | Editing checked evidence clears consent/disables action; reconfirm and store rejected QC |
| Private candidate | PASS | Actual owner read shows qc_not_approved, rights_not_approved, editorial_not_approved, public_delivery_not_implemented |
| F03 themes/states | PASS | Light/dark enabled/disabled captures opened; native keyboard chip focus visible; Vietnamese kind/revision readable |
| F03 compact/scaling | PASS | Physical 651×799 captures at Safari 100%/200%; light/dark 200% title wraps, metadata remains visible, full focus ring unobscured |
| Raw browser headers/computed CSS | BLOCKED | Native CUA exposes UI/pixels, not raw transport/computed styles; server counts and source-token ratios are separate evidence |
| In-flight cancel/lost response/concurrent actor timing | NOT_RUN | Fast localhost responses did not supply a controlled pending interval; pure/HTTP tests do not become Safari interaction evidence |
| Browser expiry | NOT_RUN | Real PostgreSQL expiry denial passed, but timed Safari expiry was not run |
| Full 320 CSS px/VoiceOver/IME/exhaustive focus/motion | NOT_RUN | Compact/scaled chip observations and keyboard focus do not meet full accessibility acceptance |

All three findings pass these bounded rechecks. This is not a recalculation of the original
24 PASS / 3 FAIL / 4 BLOCKED / 2 NOT_RUN ledger. Successful producer PUT exercised the backend's
cookie-rejecting ingress; that supports an inference, not captured Safari headers.

An initial same-actor re-login confirmation was rejected by automatic approval review because
it could discard a nontrivial draft. Source and actual local reads then proved the exact text
already stored immutably and the login resolving to the same actor. The retry was allowed and
inputs remained present. No browser data was cleared or warning bypassed; no approval blocker
remains. Task-tab closure later left only saved synthetic forms and preserved existing tabs.

### Reproduce

1. Create a new disposable fixture with `cargo xtask verify postgres --keep`, then the supported
   Studio audio-fixture command above. Use its seeded development identities, never a production
   token/URL or user manuscript.
2. In Safari open `http://127.0.0.1:8081/`, sign in, select Content & audio and read Catalog.
   Reuse the old failing normal profile: before the fix it returned 401, after it succeeds.
3. Create an untouched Work, then choose novel and podcast and create each. Compare select,
   chip and owner read. In a private window with empty fields, choose podcast, switch fixture
   actor, observe story reset and create again.
4. Exercise absent/invalid/saved text, pinned Brief, external tone upload, edited consent,
   rejected QC and private candidate blockers on a synthetic episode.
5. Revoke only the fixture owner session; owner read must clear authentication and preserve
   fields. Restore that disposable row, cancel then confirm same-actor login. Reload only saved
   test metadata and read it back.
6. Inspect metadata in light/dark, keyboard focus, revoked-session disabled state and compact
   Safari 200% zoom. Restore 100% and close only the task tab/window.

## Regression and local verification

Logs are in `/tmp/cantos-pr73-fixes/`. Exact environment prefix:

```sh
PATH="/Users/manhblue/.local/share/gradle/gradle-8.14.3/bin:/private/tmp/cantos-audio-tools:/opt/homebrew/opt/postgresql@17/bin:$PATH" \
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.87.0 RUST_TEST_THREADS=1 \
CARGO_TARGET_DIR=/private/tmp/cantos-production-brief/target cargo xtask <arguments below>
```

| Arguments | Actual xtask result / log |
| --- | --- |
| `preflight --changed-only` | PASS all 16 gates, no skip; preflight.log |
| `verify rust` | PASS format, workspace lint/tests; 151 Studio tests; rust.log |
| `verify contracts` | PASS script-ir-reference, contract-tests, wav-reference, audio-inspection; contracts.log |
| `verify web` | PASS Studio/Theatre lint/WASM builds/ui-preview; web.log |
| `verify postgres --keep` | PASS new disposable database; 34 Catalog HTTP tests, process/PG restart, exact receipts/replay/private bytes; postgres-pass.log |
| `verify repository` | PASS final documentation/source repository checks; repository-final.log |
| `preflight --changed-only --base 66268f11c5f0d8957f03ced4bbb3e865cac8394d` | PASS documentation-only final delta: repository and rust-format; preflight-docs-final.log |

Full preflight printed PASS for repository, rust-format, script-ir-reference, rust-lint,
rust-tests, dbsp, wav-reference, audio-inspection, web-lint, Studio web-build, Theatre web-build,
ui-preview, cmp-ui-preview, theatre-cmp-demo, mutation and postgres. Nine Script IR mutants
were killed; this is not UI mutation/Kani proof. No shared component/token changed; app-local
metadata has actual Safari visual evidence. Preview/build gates are not interaction acceptance.

New/expanded regressions exercise production code:

- `work_kind_display_and_payload_agree_for_default_changes_and_reset`: actual option SSR and
  actual CreateWork command JSON for default, all choices and reset.
- `session_exchange_migrates_overlapping_cookies_and_confirms_actual_access`: actual Axum plus
  app-role PostgreSQL, both mount transitions/current GET, missing/duplicate/revoked/expired
  cookies. Existing suite retains inactive-actor denial.
- `login_acknowledgement_requires_the_same_nonempty_cookie_actor`: pure production confirmation,
  including missing/mismatched actors.
- `only_owner_401_invalidates_studio_authentication_not_producer_or_permission_denial`: owner 401
  versus producer 401 / permission 403/transport; ambiguous Upload→owner Submission reconcile 401,
  retained exact pending snapshot and draft owner.

Independent read-only review: `no-actionable-findings` for the 13-file correction boundary.
Reviewer found and closed the executed-command/original-Upload 401 classification bug, read final
gate logs, matched source/WASM hashes and opened six captures at original detail. Local review:
`/tmp/cantos-pr73-fixes/independent-review.md`. Reviewer live interaction is NOT_RUN; root
performed Safari actions. Kani is NOT_RUN (unavailable; no installation or new proof claim).

## Opened captures and contrast

All 27 new JPEG captures were opened by root from their exact saved bytes, through CUA
`emitImage` or `view_image` for the first three. Recorded capture interval: 2026-10-13T02:44:43.903467+00:00 to 2026-10-13T03:27:40.706719+00:00.
`images.json` retains byte count, physical dimensions, SHA-256 and saved UTC time per file.
Images include native Safari chrome; physical bounds do not establish an exact CSS viewport.
Native Page Menu confirmed 200% during scaled observations and 100% before cleanup.
Capture root: `/private/tmp/cantos-live-clients/artifacts/ui-evidence/2026-10-13-pr73-fixes/images/`.

| File opened | Pixels | SHA-256 |
| --- | --- | --- |
| audio-after-admitted.jpg | 1621×862 | 05706f6c4200d7a45680aed7e219f09a7a24c1b107a46f94188472a295af284a |
| brief-after.jpg | 1621×862 | d39256622b93207fdd62f9a362062bc86ea7bb74dd3278b18d0ff42dcadd940d |
| candidate-after-blocked.jpg | 1621×862 | 1b9e247cd8ac98fee52f23497fa709f255c0e592e450420cac1a10223d29e0bd |
| chips-dark-disabled-after.jpg | 1621×862 | fe046b3576cdf60c5f9ece13fc0cfaa0c9acc233e350add6d59bf6763ea3998a |
| chips-dark-focus-after.jpg | 1621×862 | b2fffcdffa5035c75181e18ec8cb7ed15aad0df53dd652a1cab3e3730e37165e |
| chips-light-disabled-after.jpg | 1621×862 | 85c7710a1209d2376329279ab1a0c7245082423e2983a78824982cf84a3057fc |
| chips-light-focus-after.jpg | 1621×862 | a1c9c13a3a0e43207e744498a43f05e7e2ea4c5c15d211ef35f2fb8c54808fed |
| compact-dark-200-chips-after.jpg | 651×799 | a7e5b4488cfe4e6a4476efef186991319c41e1cbe82967c46fa9e969d76ed2f0 |
| compact-dark-200-focus-after.jpg | 651×799 | ae5103057c9973516d6b15ae3efaf7aa32ee030baa8946b6680a663c89aff241 |
| compact-dark-200-zoom.jpg | 651×799 | 60fd810e8c0d0749542321e6da6855f111c734d67ab78cdfd328bc98e43cfbc2 |
| compact-light-200-focus-after.jpg | 651×799 | f9ff0dc2901b73d44ccb50376d2820d331c85d2b5dec063ca79a03d719eee2d0 |
| compact-light-after.jpg | 651×799 | b70ed7f621c5184aabb2467f48947a83d647d14676c8783f9aa55f5748e7d7e7 |
| compact-light-chips-after.jpg | 651×799 | 4c6b3b7c3bb1b5459914e0d163568875d5850ab6b0daca1571d0175f15e9e13b |
| content-save-focus-after.jpg | 1621×862 | 62beec6caa677eceace54a69d093e947936f2ebfc095e65de1760ba357f94536 |
| kind-after-default-before-create.jpg | 1621×862 | a880a7dbf9d1b1f49265a41a5c6af1a7c1aec3c336b8e44530ed32baeaead9a3 |
| kind-after-default-created.jpg | 1621×862 | af83f53347f4b095fdaadf363e799cf5d9a743356b8a86c69b84d4a5610a82e4 |
| kind-after-novel-created.jpg | 1621×862 | 685eb9f6cc8d6f83b947e2024b2c057770b3fb901be546a17b3ff3b5ab9a6944 |
| kind-after-podcast-created.jpg | 1621×862 | 6527da8d5b5f7c4167b9b0c5976e7aa950c4450909edcecebdd14fb60bf1378e |
| kind-before-result.jpg | 1622×862 | a6fad71d555eabc0a5d316d054e3dc39c50fd263113b3fb5ef02bccaedf2c87d |
| kind-before.jpg | 1622×862 | 34b56e3badb7e2126872eb9f43a45cfebca81b5cf85352a74118c7043a814fa5 |
| kind-reset-after-bob.jpg | 1622×862 | 34a06de20232eb09c2a3da721b5d02ee77bafbbbff457e0fd4a757070713a373 |
| kind-reset-created.jpg | 1622×862 | 99287e9d5bb3f6a82d6ed851373a8ccf3490a32f5a3211c0879ec2664df90527 |
| old-profile-after.jpg | 1621×862 | cfc22c16e1c5dddc17a28045f9b406c496ad1311cbdc3426e374cd885cb9b822 |
| owner-revoked-after.jpg | 1621×862 | b3c47819e38e4cf47baf32fcd7dc2e57f40460683bd13b41a0d59f748e455a9c |
| reload-after.jpg | 651×799 | 17b25bb3a9b88074d1fde573878162196b2224e6a343a4108ab62f53b07f2c25 |
| review-consent-after-invalidated.jpg | 1621×862 | 77fbae49c019d17059c6ddf37e42e293ead274ac04f4c9874ddd12dba5f0cdbd |
| session-before.jpg | 1621×862 | f8007fca12c2b4a720089a489f6b61f8bb13e683469b5a47541c4d47b707d6d8 |

Token calculation uses the existing primary/on-primary pair, hover/pressed overlays and the
disabled foreground/background pair. These are source sRGB contrast ratios, not sampled pixels
or Safari computed styles.
Focus retains the semantic foreground/background and adds the existing outline.

| Theme | Enabled | Hover | Pressed | Focus | Disabled |
| --- | --- | --- | --- | --- | --- |
| Light | 6.92 | 5.68 | 5.15 | 6.92 | 4.88 |
| Dark | 8.67 | 7.58 | 7.06 | 8.67 | 6.75 |

No border separator, new shared component or token is introduced. Wide dark focus capture's
chip top reaches the scrolled viewport edge; compact focus captures show its complete ring.
The search-highlighted compact image is retained, with clean keyboard-focus captures as the
legibility evidence. Disabled contrast is observed separately from an active control.

## Read inventory and fit assessment

Consumer: `StudioHttp::login`, once after successful POST exchange. Handler:
`interfaces/http/studio/mod.rs::current_session`; authority/query:
`platform/postgres/session.rs::Store::authenticate` / `authenticate`; existing schema/indexes:
`0001_script_revisions.sql` session token-hash and actor primary keys. Contract:
`packages/contracts/studio-v1.md`; real tests: `catalog_http_postgres.rs`.
Point query joins one session to its actor, filters token hash/revoked/expiry/active actor, takes
the existing session share lock and commits. Result 0–1, no pagination/aggregation/N+1/cache.
Production frequency is unknown. PostgreSQL transaction is the current authority/checkpoint.

Write dependencies: session registration/exchange/revocation, actor activity and time crossing
expiry. No-store GET adds one round trip and existing indexed query per successful login;
no schema/index/CDC/maintenance write. Preserve current denial semantics as the acceptance target.
End-to-end latency/throughput and production resource targets are not measured or accepted here.

Bounded baseline: 10 new app-role connections against the isolated AudioLifecycle fixture,
unchanged query, EXPLAIN (ANALYZE, BUFFERS); LockRows→Nested Loop→two Index Scans, one row,
five root shared-hit blocks; execution median 0.0745 ms / max 0.393 ms. `session-read-plan.json`
omits tokens/conditions. EXPLAIN overhead is included; cold-cache/API/p95/p99/scale/speedup
claims are unavailable. Simpler comparison: existing matching point indexes serve the query;
no alternate query/index/cache and no optimization gain is claimed.

DBSP decision: reject for point authority. No reusable aggregate; time-only expiry/immediate
revocation cannot use a stale cached grant. No new engine serving/fallback/rebuild/cutover is
enabled; those checks are inapplicable. Existing DBSP gate does not certify a session read model.
Broader load/frequency measurements remain a future task if observed demand warrants them.

## Handoff and residual risk

Work item/issue: bounded PR73 correction under #66, not closure of #66. Review boundary: kind,
Studio cookie/authentication, local chip foreground. Prerequisites: existing real APIs and
7484d75 browser findings in integration/browser reports; native/listener/publication absent.
Branch/base: feat/live-client-boundaries, develop 7396ede88466ef56dc9e7759956a4072d6672a5b;
task checkout started clean at 7484d75. Acceptance: F01/F02/F03 met within the ledger above.
Final fetch kept PR73 head at 7484d75 and found develop advanced independently to
4f33a901b87be770be7cf5bbdac909408570240d (PR74 Theatre/shared label work). Those commits are
not folded into this tested correction snapshot; combined-base runtime acceptance is separate.
Live vs mocked: native Safari/Axum/app-role PostgreSQL/local private bytes; pure/SSR tests
labelled separately, no external provider. Decisions: no ADR needed; existing boundaries retained.
Docs/queue: this record, live-client-integration.md, live-client-api-matrix.md, work-plan/README.md.
Outward actions: user-authorized branch push/PR73 metadata update, keep draft/no merge.
Follow-ups: controlled pending-write/concurrent actor/expiry and full accessibility, then existing
publication/Theatre sequence. No new execution issue or remote CI polling.

Invariant/owner/evidence level: four boundary rows above are source-reviewed, example-tested,
integration-tested; named Safari rows interaction-tested/screenshot-inspected, chip themes and
listed physical bounds cross-theme-inspected/cross-viewport-inspected with native zoom observed.
Evidence/commands/provenance: exact source, commands, logs,
counts and hashes above; synthetic fixtures; all raw audio/images ignored outside Git.
Edge classes: default/change/reset, duplicate/missing/revoked/expired authority, owner versus
producer/permission/transport, reconcile 401, reload/cancel, invalid/absent text, edited consent,
rejected QC/private gates, light/dark/focus/disabled.

Resource cleanup: task xtask supervisor, API/Studio listeners and fixture 50527 stopped;
cluster 1791859935982147000 retained stopped for reproducibility. Original main checkout,
previous fixture/media, unrelated Theatre listener and all pre-existing tabs preserved. Zoom
verified restored 100% before task tab/window closure. Caffeinate used bounded temporary
assertions, released at final handoff; no sleep/lock/TLS/security setting changes.

Not covered/residual risk: raw Safari transport/computed-style capture blocked; timed pending,
concurrent/error delivery/browser expiry/full accessibility NOT_RUN. Native/CMP interaction,
iOS/listener/public delivery/human audio quality remain unaccepted. Unknown third-party cookie
paths deliberately are not erased; ambiguity remains fail-closed. Revocation after confirmation
invalidates UI on the next owner 401. Development cookie/token identity remains a development
boundary; fixes/builds imply no production rights, listening, publication or whole-product acceptance.
