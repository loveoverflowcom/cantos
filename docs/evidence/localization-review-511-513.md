# Localization integration review — PRs #511, #512 and #513

Review scope: the actual final source and integration of the three localization changes,
including the unmerged CMP branch. This record preserves partial issue acceptance.

## Provenance and review boundary

| PR / issue | Reviewed source | Integration / current state |
| --- | --- | --- |
| #511 / #505 | `c6b1accb96835ad57cf89fc140a9ecc057bde460` | Merged as `bf04e14c0415a82e42c930623af8818aa1c0b602` |
| #512 / #506 | `48788b618c71ca4e87531f5a960c35ee6a909bac` | Merged as `da2bb95bc55b5e01680d1c84644c6a9d46cf0521` |
| #513 / #507 | `c5832b4bdd84966b504680e6c1f0ebff09b64717` | OPEN / DRAFT; base is the #512 merge |

Remote heads were read with `git ls-remote` and fetched before branching. The new
`fix/localization-review-511-513` branch starts at the verified #513 head and targets
`feat/cmp-localization-507`. Its review diff contains only these repairs and this report.
After #513 merges, the fix PR needs ancestry review and retargeting to `develop`; neither
PR is merged or enabled for automatic merge by this task.

Read current PR/issue bodies, conversation comments and review threads for all three PRs,
issues #505/#506/#507/#167, repository instructions and the review, engineering, work-item,
localization, renderer, behavior and diagnostics owners. Issue comments and review threads
were empty; existing conversation comments only reported failed Cloudflare previews. No
provider investigation was needed. The accepted [ADR0050](../decisions/0050-localization-semantic-catalog.md)
records architecture approval, not native compatibility.

Coverage is complete-for-declared-scope: historical fixture, production catalog/schema,
Rust/Kotlin generators and generated APIs, Web roots/persistence/formatters/field guidance,
CMP text/preference/host/auth/Membership/player consumers, build boundaries and relevant
tests/models/evidence. Generated bodies were traced to their generators; CMP migrated
translations were compared with the preceding source. No new browser/device/live service
observation was made. Unrelated backend, provider and deployment implementations were not
re-reviewed.

## Confirmed defects and repairs

Assessment of the reviewed heads: `changes-requested` for the following generator defects.
Each is class `confirmed defect`, category `logic`, severity P2, confidence high: a valid
catalog reproduces the wrong output or invalid source. Current production translations do
not use the triggering cases; these are concrete failures of the accepted extension API.

| Finding | Trigger and observed failure | Owner / repair |
| --- | --- | --- |
| Plain-message brace decoding differs from parameterized messages | A valid zero-argument template `Literal {{braces}}` emits doubled braces in Rust and Kotlin; the original-template contract requires `Literal {braces}` | `scripts/localization_resources.py` and `scripts/localization_kotlin.py`: parse plain templates through the existing `segments` parser |
| Kotlin control-character quoting emits invalid source | Valid text containing U+000C is quoted with JSON's `\f`, which Kotlin rejects | `scripts/localization_kotlin.py`: reuse the existing Kotlin-aware quote primitive from the #505 generator |
| A named text argument can be shadowed by the resolver parameter | A valid `locale: text` argument produces a `locale: String` property, but `text(locale: ShellLocale)` resolves bare `locale` to the enum; output contains `English` instead of caller text | `scripts/localization_kotlin.py`: use explicit `this.<argument>` property access for every substitution |

The original #505 fixture already decodes plain templates, quotes Kotlin controls correctly
and qualifies argument access. It needs no repair. #512 introduced the production Rust
plain-key bypass; #513 introduced the analogous Kotlin bypass and its separate quoting and
bare-argument paths. Regression tests fail before each repair and assert independent expected
text afterward. Compiler probes exercise generated code, including literal inserted braces
and dollar signs; no second interpolation pass is added.

The canonical catalog and manifest stay unchanged. Existing generated Web/Rust output stays
byte-identical. Regenerated production and test-only Kotlin messages only qualify property
reads; their existing translated output and typed constructors stay unchanged.

## Three-PR finding and acceptance matrix

| Slice | Source review / evidence disposition | Acceptance still outstanding |
| --- | --- | --- |
| #511 foundation | No new defect in its historical generator; its typed fixture, strict validation, drift, original-template parser and closed select boundaries were read | #505/#167 remain partial; historical integer fixture success never establishes full runtime/native compatibility |
| #512 Web | Plain-key brace defect repaired here; no additional grounded defect in saved preference/default/refusal, generated choices, ICU4X adapters, localized field guidance or caller-injected labels | Real IME, screen readers, RTL/long text, live auth/network/audio remain NOT_RUN; prior static browser fixtures are bounded |
| #513 CMP | Three Kotlin generator defects repaired here; actual consumer migration preserves text, opaque parameters, preference refusal/retry and existing state owners | ICU4X Kotlin/native binding and numeric consumers; Android notification locale authority; iOS 2.2.20 compile/runtime; real reopen/persistence, IME, TalkBack/VoiceOver, auth/audio/device acceptance |

## Assurance ledger

| Claim | Owner / target | Required by | Evidence held / disposition |
| --- | --- | --- | --- |
| Accepted source, app-owned projections, no service/app dependency inversion | ADR0050, resource/generator owners | `adr:0050` | Source reviewed; held for bounded text generation |
| Complete translations, typed arguments and drift; literal content stays opaque | Catalog validators, Rust/Kotlin generators | `adr:0050`, `issue:#505`, `issue:#506`, `issue:#507` | Focused regression/compiler controls; fixes above; held for named generator cases |
| Saved supported locale before Vietnamese default; no implicit OS/browser negotiation | Web preference adapter; CMP PreferencesController | `adr:0050` | Actual roots/controller read; existing attributed tests not rerun for unchanged paths; held for source claim |
| Locale changes preserve draft/player/auth/input ownership | Leptos signals and CMP composition/controller owners | `issue:#506`, `issue:#507` | No remount-by-locale introduced; retained browser/JVM evidence is explicitly bounded; device/IME gap-required |
| Locale registry/projections do not rewrite content tags | Generated locale types, discovery-language adapters | `adr:0050` | Source reviewed; unknown content tags remain independent; held |
| All CMP surfaces share selected-locale authority | Android notification adapter and app hosts | `issue:#507` | `AndroidMediaEngine.kt` still uses OS resources; gap-required, retained partial acceptance |
| Native formatter adoption follows actual qualification | Compatibility owners and target build | `adr:0050` | No Kotlin ICU4X binding exists; numeric owners retained; gap-required before migration/release |
| Model and native implementation evidence remain distinct | Registered locale-preference capability | `policy:cantos-behavior` | Draft model unchanged; no Kotlin observer; held for honest boundary, no conformance claim |

## Verification and baseline findings

Full command output is retained locally under
`artifacts/agent-runs/localization-review/`; this committed record carries the conclusions.
Final command results and content provenance are recorded in the fix PR. Checks run on the
final implementation content before commit/PR packaging; packaging does not change those inputs.

| Review check | Result / scope |
| --- | --- |
| Four focused regressions replayed against the pinned old generator modules | Expected FAIL for all four; `catalog-before.log` |
| Baseline Kotlin 2.2.20 compiler control | Expected FAIL on unsupported `\f`; `catalog-compiler-before.log` |
| `python3 -m unittest discover -s scripts -p 'test_localization*.py' -v` with cached pinned Kotlin compiler classpath and standard library supplied | PASS, 89 tests, zero skips; includes generated Kotlin compilation/execution across vi/en/ja and historical typed compiler controls |
| Actual production `ShellLocale` / `TheatreResources` plus test-only `ThirdLocale` / resources compiled with cached Kotlin/JVM 2.2.20 | PASS; compilation only, no Compose/device/native-binding claim |
| `python3 scripts/localization_catalog.py --resources --check` | PASS, complete deterministic drift |
| Python syntax and `git diff --check` | PASS |
| Scoped final preflight | FAIL at repository on the same 44 qualification findings as all three original PR CI logs; zero new/removed findings. Its rust-format is NOT_RUN after that failure |
| `cargo fmt --all -- --check` | PASS separately after preflight stopped |
| Independent review of the fix-only source diff | No actionable findings; no additional runtime claim |

The optional compiler tests were enabled with `CANTOS_KOTLIN_COMPILER_CP` and
`CANTOS_KOTLIN_STDLIB`, using already installed Kotlin 2.2.20 jars; no tooling was downloaded.
Actual compiler execution is JVM evidence and cannot qualify Kotlin/Native or Android/iOS.

Handoff uses `cargo xtask preflight --changed-only --base feat/cmp-localization-507` with
explicit `--skip` for `script-ir-reference`, `protobuf-reference`, `rust-lint`, `rust-tests`,
`forge-functions`, `turso`, `wav-reference`, `audio-inspection`, `web-lint`, `web-build`,
`ui-preview`, `cmp-ui-preview`, `theatre-cmp-preview` and `mutation`. These stay NOT_RUN.
The conservative impact map selects every gate for this generator path. Backend, media,
Script IR, Protobuf, shared component and full renderer campaigns concern unchanged inputs;
the changed generator boundary is checked through the full localization suite, actual generated
Kotlin compiler/runtime controls and actual production projection compilation. Web generated
bodies do not change; existing CMP bodies only qualify property access. This scoped delivery
does not claim full preflight, native target or release acceptance.

The review reran `cargo test --offline --locked -j 1 -p cantos-localization` using Rust
1.98.1: PASS, three formatter tests and three catalog tests. This closes the earlier local
ENOSPC execution gap for this Rust fixture, while retaining both earlier failed attempts as
historical evidence. It is not Kotlin/native or device qualification.

The existing 44 shared UI qualification/adoption findings remain failures. The older xtask
assertions remain disclosed: `preflight::graph::tests::the_real_workspace_matches_the_test_graph`,
`preflight::graph::tests::workspace_dependencies_point_inward` and
`services::tests::identity_environment_names_exist_in_the_identity_sources`. Their inputs are
unchanged by these repairs; no assertion, gate, qualifier hash or workflow is weakened.

Prior final-head CI was inspected with its source provenance: #511 Web PASS / repository
FAIL / three historical xtask failures; #512 Web PASS / repository FAIL / Rust CANCELLED;
#513 Web PASS / repository FAIL / Rust CANCELLED during ffmpeg setup before source checks.
The fix PR's stacked base is outside `repository-checks.yml`'s automatic `develop` PR filter;
absence of a run is NOT_RUN, never CI PASS. Retargeting later requires fresh integration review.

## Resource handling and residual acceptance

The machine began with 229 MiB free. Active-process and nested-data inspection preceded removal
of three verified regenerable compiler-intermediate directories from the completed #492 target:
native incremental state, WASM incremental state and WASM dependency intermediates. Free space
increased to about 2.8 GiB. Sources, evidence, DBs, secrets, binaries, #506/#507 checkouts and
Android artifacts were preserved. No process was stopped. No unrelated pending browser/file
approval was acted upon. This is bounded cache recovery, not final workspace cleanup.

Code readiness applies only to the repaired, checked generator scope. Release readiness is
not established. Next acceptance owners remain #505/#506/#507 and the existing shared UI
qualification owners; no issue is closed. Native bindings, notification language, iOS,
device/IME/accessibility/live audio/auth and baseline qualification failures remain explicit.
No deployment, distribution, production write, provider activation or Long Vân privacy change
is part of this review.
