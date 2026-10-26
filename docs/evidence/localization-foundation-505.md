# Localization foundation and API-to-FE audit — #505

**Boundary: pending-decision experiment and proposal**, not production catalog adoption.
Base: fetched `origin/develop` = `645a060ddec5bffa8975cc8d94ca9266a498a390`, 26 October 2026.
Current #505/#167 bodies and comments, #168/#169, #180 comments and merged #503 were read.
#167 still says OPEN/RFC and has no acceptance comment. The new
[ADR 0050](../decisions/0050-localization-semantic-catalog.md) remains **proposed**.
The existing [Web](leptos-localization-spike.md) and [CMP](cmp-localization-spike.md) candidate
experiments are reused; their measurements were not rerun or promoted to current acceptance.

## Source inventory

| Surface / classification | Actual owner and observed boundary |
| --- | --- |
| Studio Web, shipping presentation | `apps/cantos-studio/web/src/messages.rs` (`Locale`, `is_english`), `messages/*`, `live/copy.rs` and feature copy owners still use paired catalogs/boolean adapters. #503's shorter entry copy and #510's later repairs are already merged; no duplication here. |
| Theatre Web, live and explicit preview | `apps/cantos-theatre/web/src/messages.rs` (`Locale`, `resources!`, `format`, `error_key`); `features/auth_copy.rs` and `membership_preview/copy.rs` retain distinct owners. Preview-gated fixture strings are not proof of live catalog behavior. |
| Theatre CMP, shipping shared presentation | `presentation/model/ShellCopy.kt`, feature `AuthCopy.kt` and `MembershipCopy.kt`; explicit launch XML resources; separate saved `PreferenceLocale`/shell projections. The #169 repairs remain intact. |
| Shared UI-system/tooling | Caller-injected app labels; gallery tool copy under `leptos/preview/messages.rs` and `cmp/.../preview/PreviewToolCopy.kt`. Fixed-English developer tools and localized component specimens have distinct policies. No shared package imports this fixture or app resources. |
| Content metadata and examples | Work/Unit/adaptation language is authored/released metadata. Titles, Script IR and production language are not UI strings. Existing content fixtures and original Vietnamese remain untouched. |
| New experiment | `docs/evidence/fixtures/localization-foundation` only; neither a Cargo member nor a Gradle source set. Registry/generator/typed projection evidence does not mean any app consumes it. |

The existing [localization owner map](../../.agents/skills/cantos-ui-design/references/localization.md)
continues to own the transitional app policy. This PR adds a link to the proposal, without
turning the still-paired production owners into a new runtime or broadening their allow-list.

## API-to-presentation audit

Source observations below are **confirmed** at the pinned base. They are not live API tests.

| Trace | Finding / next owner |
| --- | --- |
| Server `adapters/inbound/http/errors.rs::{error_response,failure,storage_error_response}` → `contract/studio::{ApiError,ErrorCode,FieldIssue}` | Stable error enum, optional current revision and field `{path, rule}` facts; no user-facing prose field. Storage failures map to codes. Arbitrary `params` are not part of this envelope. No schema/HTTP/auth/proto defect proven. |
| Studio `src/contract/mod.rs::ApiError` → `api.rs::decode` and `live/command.rs::ClientError` → `live/copy.rs::error_key` → `live/workspace.rs` | Consumer-owned typed representation and local message mapping exist. `error_key` exhaustively handles closed client errors, with deliberate HTTP-status fallback. `copy::diagnostic` displays technical `path · rule` under `<code>` after the localized summary. Localized field-rule guidance is still a **confirmed presentation coverage gap**, to inventory/map in #506; backend prose or a new service parameter envelope is not required to fix it. |
| Theatre discovery `contracts/theatre/discovery/v1/discovery.schema.json` → Server `src/contract/theatre/discovery.rs::{WorkSummary,CoveredWorkSummary,ListenerError}` → Web `src/contract/discovery.rs`, CMP `adapters/http/PublicCatalogHttp.kt::{CoveredWorkSummaryWire,toDomain}` | `language` is content metadata; the schema is a bounded string, not an enum of UI choices. Listener failure has `{code,retryable}`, a different envelope from Studio `ApiError`; preserve both. The fixture guard checks that content-language schemas remain open to `zh-Hant` independently of supported UI catalogs. |
| Theatre Web `application/errors.rs::ErrorKind` → `messages.rs::error_key` → public-home/work/account/error-dialog consumers | Closed local diagnostic-to-key mapping; views consume existing resources. This experiment does not replace transport classification or error handling. |
| CMP `adapters/http/ListenerClient.kt`, `PublicCatalogHttp.kt` → `PublicCatalogRead` → shared error presentation | HTTP/status plus typed listener codes are classified before presentation; strict not-found envelope checks are distinct from generic listener errors. No UI translation request is added to HTTP. |
| Theatre `behavior/discovery_retired_language.md` and `locale_preference.md` | Existing draft capabilities model content-filter retirement and saved preference respectively, not this stateless catalog. Neither model nor binding is changed; Quint/native conformance is inapplicable to this slice. |

**Suspected:** other Studio field-rule presentation surfaces may need semantic guidance beyond
the inspected workspace/delegation paths. #506 must inventory them before changing mappings.
**Not checked:** every endpoint/error mapper, live Server/Identity, deployed API configuration,
browser/Compose semantics, Android/iOS devices or current installed artifacts. No claim of
universal API parity, all field-rule coverage or native acceptance follows from this audit.

## Invariant and evidence ledger

| Claim / failure mode | Owner / oracle | Evidence level and result |
| --- | --- | --- |
| Resource-derived locale registry; new locale must not require consumer boolean branches | Fixture metadata plus third-locale vectors and fourth-locale generation test | `example-tested`, `statically-checked`; Python and generated Rust/Kotlin fixture only |
| Same semantic keys/typed args and exact outputs across projections | Deterministic projection manifest, independent 22-vector oracle, native compiler positive/negative count probes | `type-enforced`, `example-tested`; Rust native and Kotlin/JVM, no renderer |
| Missing/blank keys, named arguments, plural/select drift or generated edits must fail | Whole-fixture validator and exact diagnostic negative controls, wired into existing localization/repository check | `example-tested`, `statically-checked`; legacy raw-literal/error checks retained |
| Content braces/markup/tokens are opaque, argument order independent | Original-template parser and hand-authored swapped-order/literal-content vectors | `example-tested`; actual escaped HTML/Compose sink remains #506/#507 evidence |
| UI locale cannot narrow content language or rewrite content | Independent discovery schema check; render leaves `zh-Hant` metadata/title unchanged | `statically-checked`, `example-tested`; not an HTTP/content-persistence acceptance claim |
| Existing application/state behavior preserved | Diff contains no app, service, schema or behavior-model source change | review; full production catalog adoption and locale switching not claimed |

## Executed checks and limits

The final PR verification section records the exact final-content commands, preflight summary,
baseline comparison and source SHA. Full logs remain ignored under
`artifacts/agent-runs/issue505/` in the isolated checkout.

The fixture tests execute both dependency-free generated projections against the same
hand-authored corpus with wrong-count compile-negative probes. Kotlin uses the already cached
2.2.20 compiler and stdlib; its invocation/classpath is documented in the
[fixture README](fixtures/localization-foundation/README.md). This is JVM compilation/execution,
not Compose rendering or Native linking. No toolchain installation or application dependency
change occurred. Rust uses the pinned 1.98.1 compiler.

An initial Rust harness compile failed because the newly written harness passed `&str` to
generated `String` fields. This was an introduced **test defect**, corrected in the harness;
it was not an application or generator regression and the compiler assertion was retained.

## Deferred acceptance and handoff

- #167/ADR 0050: owner review and acceptance of source/schema placement, runtime/CLDR formatter,
  fallback/negotiation and release exposure. This is the exact gate before wholesale adoption.
- #506: migrate Studio/Theatre Web only after the decision, preserve state/document language,
  map technical field rules into semantic guidance, test actual escaped text/attributes and
  locale-aware formatter output, browser/caret/focus/IME/long-text behavior.
- #507: migrate Theatre CMP only after the same decision; preserve saved preferences and app
  locale authority, prove launch/resource alignment, Android/iOS build/runtime/semantics and
  large-text/accessibility behavior. Do not infer these from the new JVM fixture.
- Full CLDR fractional/ordinal/range plural rules, number/date/duration/currency formatting,
  translator workflow and comparable app bundle/startup measurements are NOT_RUN here.
- No backend API gap was established that warrants service contract changes. No deployment,
  provider, secret, storage, SSH, production or user-data action is part of this work.

The queue adds only the newly explicit dependency sequence; GitHub retains execution status.
Remaining risk: this prototype does not resolve the adoption decision, the full formatter
boundary or production app coverage. Merge alone must not close those acceptance gaps.
