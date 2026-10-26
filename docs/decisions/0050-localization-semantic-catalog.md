# ADR 0050: Resource-driven semantic localization contract

- Status: accepted architectural direction; production adoption requires target-specific qualification
- Date: 2026-10-26
- Decision owner: repository owner via [#167](https://github.com/loveoverflowcom/cantos/issues/167)
- Related issues: [#505](https://github.com/loveoverflowcom/cantos/issues/505),
  [Web #506](https://github.com/loveoverflowcom/cantos/issues/506),
  [CMP #507](https://github.com/loveoverflowcom/cantos/issues/507)
- Supersedes: N/A

## Owner acceptance — 26 October 2026

At 16:26 UTC the repository owner approved the concrete direction presented at 16:12 UTC:
"Duyệt hướng trên, làm tiếp đi" (conversation message
`Sentinel_e5fd476bc7fc8191b0647efa4e9b9a6f`, forwarded in the #506 task).
The accepted choice is one versioned JSON source with app-owned generated Rust/Kotlin APIs,
a common pinned ICU4X core and CLDR data strategy, release UI locales `vi-VN` and `en`,
test-only `ja`, saved supported preference before the `vi-VN` default, and no automatic
browser/device negotiation. Missing translations must fail validation.

The implementation placement is `packages/localization/catalog/v1`; schema-breaking changes
use a new major directory/version, while content changes update the deterministic digest.
This is a presentation resource owner, not a service contract or an application import.
Each app keeps its own generated representation. UI-system components still receive copy
from their callers.

Acceptance selects the architecture, not working bindings or formatter coverage. Qualify each
affected formatter and target before migrating its production consumers. Web/WASM or native
Rust evidence cannot establish Kotlin/Compose, Android or iOS readiness. Native adapters and
#507 adoption remain separate. Unsupported formatting returns typed errors; it must not
substitute English, ISO strings or hand-built localized output. Retain the existing owner for
a namespace until its replacement passes. The original #505 fixture remains bounded historical
evidence, including its deliberately incomplete integer plural schema.

## Original #505 context (historical)

The owner requires adding UI languages through resources and generation/validation without
new `english: bool` branches in consumers. That direction does not select a runtime or format.
At base `645a060ddec5bffa8975cc8d94ca9266a498a390`, #167 is OPEN/RFC, with no decision
comment. #168/#169 delivered bounded changes and candidate evidence, not broad adoption.
The original proposal followed #505's explicit pending-ADR branch. Merging that fixture did not accept
this ADR or complete #167/#505's production acceptance checklist.

Reuse the [Leptos spike](../evidence/leptos-localization-spike.md) and
[CMP spike](../evidence/cmp-localization-spike.md). Their historical measurements are tied to
their named source/tool/environment; this PR does not remeasure those candidates.
The new [fixture and audit](../evidence/localization-foundation-505.md) isolate the contract
questions without installing another application runtime or upgrading Leptos/Kotlin/Compose.

## Selected direction and original fixture contract

Use a renderer-neutral, versioned semantic catalog and locale registry as generation inputs,
with each application owning its generated projection and presentation adapter. Prefer a
small JSON schema with explicit named argument and plural/select nodes over boolean pairs.
The original executable draft schema was [the fixture validator](../../scripts/localization_catalog.py);
the [fixture catalog](../evidence/fixtures/localization-foundation/catalog.json) is experiment
input, not the production canonical source. The accepted production source is now [catalog/v1](../../packages/localization/catalog/v1/catalog.json),
with its [structural schema](../../packages/localization/catalog/v1/schema.json), strict validator
and [runtime contract](../../packages/localization/README.md). The original fixture stays in place.

The original fixture contract below describes that bounded experiment. The owner acceptance
and production runtime contract above supersede its unresolved policy statements:


- Semantic namespaces are `common`, `studio`, `theatre`. Keys name stable meanings. Reuse
  needs identical semantics; platform-specific variants stay explicit. Shared UI-system
  primitives continue receiving caller-provided accessible labels and import no app catalog.
- Each key declares named argument types. Generated Rust enum fields and Kotlin sealed
  message constructors carry text, nonnegative whole counts and closed select enums.
  Consumers use named fields; source argument order has no meaning. Plain text parameters
  do not claim to distinguish two different string meanings at compile time.
- The registry declares canonical BCP-47 UI tags, autonyms, explicit aliases, test-only
  exposure, fallback and plural data. Supported UI tags come from metadata. Valid content
  tags such as `zh-Hant` need not have a UI catalog and are never filtered through this registry.
- Resolution returns the requested and resolved tag, including whether it was exact.
  Case-insensitive regional lookup removes subtags per the bounded
  [RFC 4647 lookup](https://www.rfc-editor.org/rfc/rfc4647.html#section-3.4) experiment;
  `en-GB` resolves to `en`, explicit `vi` alias to `vi-VN`. Unsupported requests return a
  diagnostic naming the proposed fallback `vi-VN`; applying that fallback is an explicit
  outer policy, never a silent lookup result. This experiment is not a full IANA tag validator.
- Accepted Web precedence is saved supported choice, then `vi-VN`, with visible unsupported/storage
  notices and no automatic browser/device negotiation. Preserve the existing CMP controller
  until its separately qualified #507 migration.
- Every supported locale must contain every key, argument and required plural/select form.
  Missing keys, blank strings, unknown/missing arguments, selector drift and generated-file
  drift fail validation. A missing translation never becomes backend prose or a key on screen.
- Parse the original template once. Insert content as opaque text, including `{time}`, `%1$s`,
  Unicode and markup-looking titles. Bind the result through escaped Leptos text/attributes or
  Compose text, never `inner_html`, HTML parsing or another template pass. Plain string
  generation does not itself establish renderer escaping; #506/#507 must test the actual sink.
- Cardinal plural/select rules belong to resources and formatting adapters, never views.
  The fixture covers the **integer** CLDR 48 subset for `vi`/`en` and test-only `ja`
  ([pinned source](https://www.unicode.org/cldr/charts/48/supplemental/language_plural_rules.html)).
  `integerOne` is a finite set of whole counts; all remaining counts use `other`. That schema
  cannot express general CLDR rules, fractions, ordinal or range plurals. Do not extend it
  into a handwritten all-language runtime; select/review the full CLDR adapter before adoption.
- Number/date/duration/currency formatting requires explicit typed requests and a reviewed
  locale-aware adapter: decimal value; instant plus time zone; duration plus clock/spoken
  style; integer minor units plus currency. The original fixture implemented no such adapter; #506 now owns the bounded
  [ICU4X Web qualification](../evidence/web-localization-506.md).
  Unavailable formatting must be a typed refusal, never an ISO string, English fallback or
  manually concatenated number presented as localized. Fixture count `to_string` and supplied
  `time: text` are interpolation examples, not locale-aware formatters.
- Generation is deterministic, time/path-independent and accompanied by a manifest of the
  catalog digest, locales, keys and argument types. Whole-catalog validation is independent
  of changed-line literal checks. The existing repository gate runs both. Native compiler
  checks provide distinct evidence from byte drift/parity.

## Alternatives considered and original evidence

| Architecture | Benefits / observed evidence | Costs / limitations | Proposal disposition |
| --- | --- | --- | --- |
| Canonical semantic catalog + generated typed projections | One key/argument/translation source; reversible fixture now demonstrates Rust/Kotlin parity and a resource-only third locale; baseline typed Rust spike adds no runtime | Own schema/versioning/generation; full CLDR and formatting adapters still need target evidence; translator workflow unmeasured | Selected contract ownership; ICU4X target qualification remains required |
| Renderer-native resources + parity manifest | Compose Resources already serves launch/assets; retained Compose and moko JVM fixtures pass; preserves native tooling | Duplicate catalogs/translations; resource varargs do not guarantee named typed arguments; Compose 1.9 app-locale override/native evidence remain bounded | Viable alternative if native integration/translator costs outweigh canonical generation |
| Fluent/ICU common catalog + thin renderer adapters | Rich message model; Leptos spike exercises nonrecursive Fluent insertion | Historical Web fixture adds about 75–108 KB gzip for Fluent variants; `leptos-fluent` needed graph correction; CMP equivalent/native pipeline unproven | Not selected for this migration |

`leptos_i18n` JSON is another realization of canonical generation: the retained 0.5.11 spike
resolves against Leptos 0.7.8, adds 61,273 optimized gzip bytes in that tiny fixture and exposes
a Vietnamese-default plural limitation. Those are historical spike observations, not current
app footprint or a rejection of newer versions. Compose/moko/Lyricist candidate results and
transitive-version caveats remain in the CMP spike; no candidate was reinstalled here.

## API and content boundary

Server errors retain typed code/revision/field-rule facts. FE maps these exhaustively to local
semantic keys, arguments and recovery actions. Existing envelopes do **not** have arbitrary
`params` or prose fields; do not generate a new service schema from this UI experiment.
Unknown open wire values map to a deliberate unexpected/protocol presentation, never raw
SQL/provider text. Content language and content-language filters keep their service-owned
meaning; UI locale changes do not rewrite Work/Unit/Script IR, discovery queries or playback.
The [source audit](../evidence/localization-foundation-505.md#api-to-presentation-audit) records
confirmed presentation gaps separately from backend defects. No backend defect was proven here.

## Consequences and migration

- User impact now: none; fixture sources are outside Cargo/Gradle application graphs.
- Compatibility: no HTTP/auth/proto/schema/migration, content, audio, rights or provider changes.
- Cost: local generation, builds and verification; no deployment or service operation.
- Owner acceptance is recorded above. #506/#507 migrate one coherent namespace at a time, preserving
  interpolation, document language, saved preference and mounted form/player/account state.
- Before each adoption, establish registry-to-renderer adapter behavior, HTML/semantics safety,
  full formatter vectors, missing-key diagnostics and native target evidence. Never mount both
  the old and new locale authorities. Retain the old namespace until the replacement passes.
- Web rollback reverts the generated-resource/adapters commit as a unit; it never rewrites
  content, playback or credentials. The original fixture remains independently reproducible.

## Qualification boundary

The [fixture README](../evidence/fixtures/localization-foundation/README.md) owns reproducible
generation, validation, native compile tests and exact limits. That original fixture proves only its bounded
contract with independent hand-authored vectors. Leptos CSR/WASM, Compose resource environments,
Android/iOS builds/runtime, text expansion, screen readers and full CLDR formatters remain
unverified by this fixture. Existing spikes retain their own separate evidence.

The owner acceptance above unblocks staged implementation. Actual adoption still requires the
named target/formatter evidence; revisit an adapter if its WASM/native footprint, locale override
or translator workflow is unsuitable. A merge or a passing fixture never establishes missing
runtime, platform or production acceptance.

#506 Web implementation and actual target results are recorded in [the Web evidence](../evidence/web-localization-506.md).
The [#507 CMP record](../evidence/cmp-localization-507.md) separates generated text/registry
adoption from the retained numeric compatibility namespaces and missing native ICU4X bindings.
Architecture acceptance does not close #167/#505 or establish Android/iOS runtime acceptance.
