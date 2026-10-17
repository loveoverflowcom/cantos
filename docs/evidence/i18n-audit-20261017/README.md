# i18n/l10n ownership audit — 17 October 2026

Source-bound implementation/audit record for `develop@8786bdc3e967884460f9a27f3e36c29fa27d1326`,
including merged PRs [#226](https://github.com/loveoverflowcom/cantos/pull/226) and
[#230](https://github.com/loveoverflowcom/cantos/pull/230). Audit addresses always identify
that baseline; moved keys identify the resulting resource owner. This is not an accepted
catalog architecture or a claim of production acceptance.

## Audit tables

Each table uses `file:line | string | classification | resource owner | proposed key | action`.
The 877 baseline-addressed rows distinguish production UI, misplaced resources, preview tool UI, diagnostics,
fixtures/authored content and technical identifiers/proper names; retained literals are not
automatically violations.

- [Theatre Web](theatre-web.md): all Rust/HTML, public Home/Work, auth diagnostic and copy owners.
- [Studio Web](studio-web.md): all Rust/HTML, live workspace/dashboard/artwork, authoring and Forge.
- [Theatre Mobile and shared CMP](cmp.md): production/common/platform/sandbox copy and shared previews.
- [Shared Leptos](leptos-preview.md): primitives, HTML, gallery, auth samples, workbench and supporting samples.

## Invariant and ownership

Visible and accessible copy in repaired production surfaces resolves through an existing
typed resource owner in both `vi-VN` and `en`. Switching UI locale preserves authored text,
its source language, draft/selection and playback ownership. External errors map to message
keys; protocol values stay protocol values. Named interpolation reads the original template
once and preserves titles such as `Lời hẹn {time}`.

| Boundary | Change and cheapest oracle |
| --- | --- |
| Theatre public UI | Home/Work/auth-diagnostic tables extend the existing `messages.rs` macro; semantic reuse and exhaustive state/error/kind mappings, paired resources and actual HTML render tests |
| Studio UI | Existing `live/copy.rs` and artwork owners receive inline translations; Forge retains `messages/forge.rs`; typed error/review projection, selected request values and content-language adapters tested independently |
| CMP | Existing Shell/Auth/Membership/launch owners stay authoritative; sandbox maps move into `DemoShellCopy.kt`; creator text keeps its language annotation and exact content |
| Shared previews | Package-owned English tool UI; recipe locale governs specimens. Vietnamese long text and input/a11y fixtures stay Vietnamese. Neither shared package imports an application catalog |
| Regression guard | Tokenized changed-source sinks, declared owners, recognized bilingual catalog forms and placeholder multiplicity; reviewed exact exceptions. Arbitrary indirect dataflow and all dynamic mappings still require typed/compiler/renderer tests |

## Open decisions and justified exceptions

[#167](https://github.com/loveoverflowcom/cantos/issues/167) remains OPEN/RFC. No localization
library, generator, new dependency, universal catalog or persistence/negotiation policy is
selected here. [#168](https://github.com/loveoverflowcom/cantos/issues/168) and
[#169](https://github.com/loveoverflowcom/cantos/issues/169) are closed after bounded work;
their original issue bodies are not a current implementation inventory. Their spike/acceptance
records and #168's closing comment retain broader migration, first-render policy, formatting
and native/IME acceptance questions. This PR does not reopen or close issues.

Existing auth/membership/Studio owner APIs, including the read-only `english` adapter, remain.
Preview tool UI is deliberately English, not a bilingual app interface. Vietnamese sample
labels, manuscripts, titles, character names, typography specimens and generated design
inventory records remain literal content. Protocol IDs, CSS/test IDs and developer-only
assertions/logs remain untranslated. Existing supported duration/plural/speed vectors are
tested; this change does not invent full date/number/money formatting or hour-boundary policy.

No route, retry, pagination, API/auth capability, release gate, provider call or backend
architecture changes. The public page limit remains 20; a shared constant now supplies its
request, decoder bound and named notice parameter. No bundle/startup/cache improvement is
claimed; dependency files are unchanged and byte/performance deltas are unmeasured.

## Verification

Commands below ran from this branch worktree on macOS, Rust 1.98.1, locked dependencies,
2 Cargo jobs, dev/test debug information disabled, incremental compilation disabled and
`TMPDIR=/private/tmp`. CMP used installed Gradle 8.14.3 and Microsoft JDK 21.0.7. Logs live
in ignored `artifacts/agent-runs/i18n-20261017/`, `i18n-theatre-20261017/` and
`i18n-cmp-20261017/`; no credentials, private runtime data or generated media is committed.

| Command / result | Evidence and limits |
| --- | --- |
| `cargo test --locked -p cantos-theatre` — PASS | 259 unit + 3 compile-fail doc tests; both-locale public Home/Work actual HTML, typed mappings, title braces/content tags, locale projection with unchanged player/library facts |
| `cargo test --locked -p cantos-studio --lib` — PASS | Final 263 tests; typed error/review maps, work labels/payload, artwork HTML/drafts, actual Document content-language adapter. This does not execute the WASM-only authoring editor |
| `cargo test --locked -p cantos-ui --test gallery` — PASS | Final 35 tests including specimen resources, interpolation, English tool UI and Vietnamese fixtures |
| `cargo xtask ui-preview cmp verify` — PASS | 214/214 JVM tests, no failures/skips; final common/resource and rendered semantics/capture tests |
| `cargo xtask theatre-demo verify` — PASS | Shared 204 executed + sandbox 219 executed, zero failures; one existing live-origin opt-in test skipped because its origin is unset |
| `cargo test --locked -p cantos-ui --test auth_account` — PASS | Final 18 tests; the independent harness reuses preview `messages` and `recipe` modules. Handoff initially caught and then repaired missing harness imports; scoped clippy also passed |
| `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` — PASS | 589 selected, 584 executed, zero failures; 5 optional pinned-Pillow PNG-adapter tests skipped. Discovery confirmed their exact reasons; guard/checker tests all ran |
| Guard focused primitives — PASS | 39 localization + 111 checker tests; final changed UI diff clean. [Supported syntax, 49 exact reviewed exceptions and limitations](guard.md) |
| `cargo xtask verify postgres --keep` — UNSUPPORTED | Missing ffmpeg/ffprobe, prerequisite precheck ran no suite and created no disposable cluster |
| `cargo xtask app cantos-studio --web` — blocked | Standard launcher correctly refused because no cantos-server listens on 127.0.0.1:8080; no substitute API was started |

The original Studio WorkKind regression failed with untranslated `novel/story` before its fix.
Development compile/test mistakes (a misplaced Theatre test and old translated preview-label
expectations) were repaired; final focused runs above supersede them, and initial logs remain.
They are not unresolved failures or passing baseline evidence.

### Actual browser and visual observations

Chrome 155.0.8059.80 was driven by native Mac input against the final worktree through canonical xtask
launchers on 8082 and 8083. The original sandbox falsely reported occupied ports because bind
was restricted; the authorized loopback launcher ran after escalation. Both owned supervisors
were stopped with Ctrl-C afterwards; unrelated processes/settings/auth were left alone.

- Public Home and Work unavailable routes: actual vi-VN/en headings, body/error, Retry, Back,
  navigation, locale control and accessible region names changed. Home document language was
  read from live DOM as `en`. Real API requests failed because the API is absent; Ready catalog,
  pagination and real playback were not browser-accepted here.
- Existing explicit `?mode=mock`: navigation to settings and a real vi→en switch preserved
  `Người giữ tiếng chuông`, `Sơn Hà Ký`, the offered paused position `12:48 / 28:42` and the
  simulated player identity; transport accessible copy changed. This is session/mock evidence,
  not native audio or public-delivery acceptance.
- Leptos valid `auth-tonal/error`, long-vietnamese, dark, comfortable, text200, frame320,
  reduced, isolated recipe: trusted input `Lời hẹn {time}` remained exact through vi→en→vi;
  label/helper/error/CTA names changed and tool controls stayed English. Live DOM for the two
  *visible* email fields returned page `en`, specimen `vi-VN`, exact entered value. Tab reached
  the Continue CTA with visible focus. Hidden retained fixtures were excluded from that probe.
- Opened Chrome screenshots: Public Work Vietnamese at desktop; Home in both locales at
  actual responsive viewport375×844 CSS px; gallery dark English specimen at frame 320/text 200.
  Home copy wrapped, and the gallery full label and CTA wrapped with visible focus. The frame
  constraint is distinct from the browser viewport. The exported, opened viewport PNG is
  `artifacts/agent-runs/i18n-20261017/home-en-375x844.png`. Native AX clicks under responsive scaling
  were unreliable; language was switched on desktop then inspected at the same narrow width.
- Opened JVM images and exact dp/theme/text provenance are in the CMP run record. Root also
  inspected the light 1140×840 tool gallery and dark 320×844/text2 auth-button specimen: English
  tool controls and Vietnamese specimens coexist, and the complete action label wraps.

These observations are `DOM-tested`, `interaction-tested`, scoped `accessibility-checked`
(keyboard/focus and exposed names) and `screenshot-inspected`; no screen reader, IME/caret,
Android/iOS device, smooth-motion or real-media claim follows from them.

### Source review and residual acceptance

Independent read-only Cantos code review covered the full final UI/tooling diff and callers.
Assessment: **no-actionable-findings** for frozen source after six guard regression groups
and preview tool/fixture classifications were repaired and independently re-probed. Reviewed
source SHA256: `573b5681f713e0fb72fdf4cd2c65e1bd2744b7f0e2a7c0787675f3e009b6b58b`
(canonical sorted path/byte-hash/mode JSON, excluding docs and Markdown). CMP's 22 provenance
hashes matched final sources. Later evidence-only index text does not change that source scope.
Review is not merge approval or runtime acceptance.

| Deferred layer | Reason, next command and criterion | Owner |
| --- | --- | --- |
| Studio browser drafts/IME/caret | Provide trusted installed ffmpeg/ffprobe, then fresh `cargo xtask verify postgres --keep`; start `cargo xtask run cantos-studio --web --test-cluster`; drive locales on authoring/artwork/Forge. Preserve drafts/caret and content language with no remount or loss | Unassigned — assignment required before acceptance |
| Real public Ready/detail/pagination | The API is absent; use the documented fresh disposable seeded catalog and canonical Theatre run, then repeat both locales. Native render tests cover the copy, not real HTTP here | Theatre acceptance owners under #172/#179/#180 |
| Android/iOS / screen reader | No native or assistive-technology run in this task; use existing native hosts and record locale/focus/content-language voice behavior | Unassigned — assignment required before acceptance |
| Catalog/formatting architecture | #167 is still open; broader #168/#169 migration and locale precedence are not accepted by this repair | #167 decision owner |

Handoff preflight is recorded separately below with each actual gate status. No GitHub CI
status was read, waited on or dispatched. No merge, deployment or issue closure is authorized
by this record.

### Final handoff gates

Original handoff command:

```sh
cargo xtask preflight --changed-only --skip audio-inspection --skip postgres --skip cmp-ui-preview --skip theatre-cmp-demo
```

The repository gate **FAIL** produced 22 findings, identical in order and text to clean
`develop@8786bdc3` (`baseline-repository.log`). No qualification/check was relaxed. Subsequent
steps in that run were NOT_RUN. The independent run below retained that failure and reused
unchanged CMP evidence; it is not a full green aggregate:

```sh
cargo xtask preflight --changed-only --skip repository --skip audio-inspection --skip postgres --skip cmp-ui-preview --skip theatre-cmp-demo
```

```text
NOT_RUN      repository                                skipped by --skip
  PASS         rust-format
  PASS         script-ir-reference
  PASS         protobuf-reference
  PASS         rust-lint (workspace)
  PASS         rust-tests (workspace)
  PASS         forge-functions
  PASS         dbsp
  PASS         wav-reference
  NOT_RUN      audio-inspection                          skipped by --skip
  PASS         web-lint (cantos-studio, cantos-theatre)
  PASS         web-build (cantos-studio)
  PASS         web-build (cantos-theatre)
  PASS         ui-preview
  NOT_RUN      cmp-ui-preview                            skipped by --skip
  NOT_RUN      theatre-cmp-demo                          skipped by --skip
  PASS         mutation
  NOT_RUN      postgres                                  skipped by --skip
preflight: PASS with 5 explicitly skipped (NOT_RUN) step(s)
```

The first independent attempt caught missing test-harness modules. After the reviewed
harness-only repair, 18 focused auth tests and scoped clippy passed; the final run above
passed all available steps on unchanged final runtime inputs. Workspace tests executed
**2,216 native tests + 38 doctests**, zero failures; 341 default opt-in tests were ignored
(including DB/audio suites), so they establish no real database/media acceptance.
Mutation ran the 16-test baseline and killed all 9 named mutants, with zero survivors,
timeouts or unviable cases. Forge boundary tests use offline fixtures/doubles.

Skip reasons: repository's original FAIL remains; PostgreSQL/audio lack ffmpeg/ffprobe;
CMP gallery/Theatre gates reuse their attributed final source-matching PASS runs (214 and
423 executed, plus one pre-existing live-origin skip). Raw final log:
`artifacts/agent-runs/i18n-20261017/preflight-independent-final.log`.

After evidence-only documentation packaging, the final repository primitive was compared
again with the same baseline; source hashes, localization diff, whitespace and clean main
checkout are checked before commit/push. Commit/PR packaging alone does not rerun these gates.
