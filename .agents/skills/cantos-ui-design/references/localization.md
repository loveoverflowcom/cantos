# Localization

> **Scope.** Make every visible and accessibility string in Studio and Theatre a localized
> resource on both renderers: the locale-set and framework decisions, the difference between UI
> language and content language, semantic keys, parameters instead of concatenation, plural and
> number/date/duration/currency formatting, typed error presentation, layout under text expansion,
> key and placeholder parity, and the checks that make a missing string visible before a user
> sees it. Use whenever a change adds or edits copy, a label, an error, a status name, an
> accessibility name, a date or a number shown to a person.

The current applications already expose Vietnamese and English UI choices (`vi-VN` and
`en` on Web). **Resources and typed keys exist** on both
renderers; what remains undecided is a unified catalog format, library, generator and
persistent locale policy. Treat the code below as implemented where explicitly identified,
and all replacement approaches as candidates pending
[architecture #167](https://github.com/loveoverflowcom/cantos/issues/167),
[Leptos #168](https://github.com/loveoverflowcom/cantos/issues/168) and
[CMP #169](https://github.com/loveoverflowcom/cantos/issues/169).

## 0. Current owners, interim rules and open choices

| Surface | Existing source of UI copy | Known gap |
|---|---|---|
| Theatre Web | [`messages.rs`](../../../../apps/cantos-theatre/web/src/messages.rs) (`Locale`, `resources!`, public catalog and auth-diagnostic keys, single-pass formatting tests), [`features/auth_copy.rs`](../../../../apps/cantos-theatre/web/src/features/auth_copy.rs), [`membership_preview/copy.rs`](../../../../apps/cantos-theatre/web/src/membership_preview/copy.rs) | Multiple existing macros; locale is session memory; catalog/runtime unification remains open |
| Studio Web | [`messages.rs`](../../../../apps/cantos-studio/web/src/messages.rs), [`messages/`](../../../../apps/cantos-studio/web/src/messages), [`live/copy.rs`](../../../../apps/cantos-studio/web/src/live/copy.rs), [`live/dashboard_copy.rs`](../../../../apps/cantos-studio/web/src/live/dashboard_copy.rs), [`live/delegation/copy.rs`](../../../../apps/cantos-studio/web/src/live/delegation/copy.rs), [`live/artwork/copy.rs`](../../../../apps/cantos-studio/web/src/live/artwork/copy.rs) | Structs, keys and `english: bool` coexist; root locale is session memory |
| Theatre CMP shared presentation | [`ShellCopy.kt`](../../../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/presentation/model/ShellCopy.kt) (`CopyKey`, `ShellText`, `ShellCopy`), [`AuthCopy.kt`](../../../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/features/auth/presentation/AuthCopy.kt), [`MembershipCopy.kt`](../../../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/features/membership/presentation/MembershipCopy.kt) | [`ShellLocale`](../../../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/presentation/model/ShellState.kt) and [`PreferenceLocale`](../../../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/application/preferences/PreferencesController.kt) are separate representations; application preference has explicit native key/value adapters; [#169 evidence](../../../../docs/evidence/issue169-cmp-locale-acceptance.md) distinguishes native/JVM acceptance from pending architecture adoption |
| Theatre CMP sandbox copy | [`preview-sandbox/.../app/DemoShellCopy.kt`](../../../../apps/cantos-theatre/mobile/preview-sandbox/src/commonMain/kotlin/com/cantos/theatre/app/DemoShellCopy.kt) injects `DemoShellCopyOverrides` into shared presentation | Demo diagnostics belong in the sandbox, not the shared catalog; production release exclusion is not yet accepted ([isolation record](../../../../docs/evidence/issue122-cmp-sandbox-isolation.md)) |
| CMP optional launch presentation | [`values/strings.xml`](../../../../apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/values/strings.xml) (explicit Vietnamese/English IDs), consumed by [`CantosLocalLaunch.kt`](../../../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/features/launch/presentation/CantosLocalLaunch.kt) via `stringResource` | Five existing launch/unavailable strings select public resource IDs from the application locale; [#169 evidence](../../../../docs/evidence/issue169-cmp-locale-acceptance.md) records this bounded Compose 1.9 adapter. Native launch/skip behavior retains separate [launch evidence](../../../../docs/evidence/theatre-cmp-launch.md) |

This is a source inventory, not a new ownership architecture or a runtime test result.
Follow the caller to its actual owner; do not restore the removed `demo/DemoCopy.kt` or
move sandbox-only wording into `:shared` to satisfy this map.

The shared Leptos and CMP development galleries own their tool copy under
`preview/messages.rs` and `preview/PreviewToolCopy.kt`, respectively. Their tool interface
uses English; recipe locale selects component specimens, not an application preference.
Vietnamese typography/content fixtures remain Vietnamese. This bounded preview policy
does not choose an app catalog architecture or authorize shared packages to import app copy.

**While the decision is open:** search the current app/feature copy owner and existing
semantic key **before** adding any UI, preview, accessibility or error string. Reuse the
existing key **within that owner's API** where its meaning matches; otherwise extend that
owner in **both** Vietnamese and English, with a focused resource/placeholder test. Equal
English wording alone does not authorize merging keys, moving copy between applications,
or introducing a shared catalog. Do not invent a new
feature-local translation enum/macro, copy text into `view!`/`@Composable`, or introduce a
new dependency just because the framework offers one. This transitional policy does not
block ordinary UI fixes pending #167 or require a catalog/locale migration during them.

**No library selection yet.** The issue's candidate version tracks are not compatibility
evidence. A spike must resolve exact versions against the actual Cargo/Gradle pins:
- Web: `leptos_i18n`, `leptos_fluent`, current typed
  Rust catalogs plus Fluent/ICU4X adapters. See #168.
- CMP: Compose Multiplatform Resources (already used for splash), moko-resources,
  Lyricist; compare a generated typed-catalog baseline as well. See #169.
- Cross-renderer: canonical catalog + generated adapters, renderer-native catalogs +
  parity manifest, or a Fluent/ICU-style common message model. See #167.

Version compatibility, maintenance, CLDR plurals, CSR/WASM size, Android/iOS support,
locale override, translator workflow and migration cost require a scoped spike and ADR.
Do not turn an option into a recommendation or a second runtime until that review
is recorded. `packages/ui-system` components still receive their localized labels from
callers rather than importing app-owned resource sets.

**Interpolation and locale hazards:** Resolve named parameters from the **original**
message template exactly once. Injected work titles such as `Lời hẹn {time}` must retain
their braces; never recursively expand title content. UI locale must remain independent
from a Work's content language; switching it must not rewrite Script IR, reset playback,
discard Studio drafts or change server authorization. When changing locale behavior, check
document `<html lang>` as well as inner containers, mixed-language content spans, launch
resources and accessible names. Existing code does not establish complete alignment; report
remaining gaps rather than implying this docs-only change fixes them.

## 1. Open decisions before adopting or migrating resources

These are review questions for #167, not permission to implement a preferred answer or a
prerequisite to reuse an existing resource. Record a selected strategy only after acceptance.

| Decision | Options to weigh | Constraint |
|---|---|---|
| locale set | Vietnamese and English already implemented; confirm BCP-47 variants, future locales and source/fallback policy in #167 | preserve both existing translations; do not silently select a canonical source locale |
| default and fallback | **candidate policy**: stored choice, then approved browser/OS preference, then `vi-VN`; current roots use memory defaults | a missing key is a visible build or test failure, never a silent English fallback in a view |
| framework, Web | `leptos_i18n`, `leptos_fluent`, existing typed catalog + Fluent/ICU4X (all candidates, #168) | must support named parameters, CLDR plural rules and locale-aware formatting; verify the version |
| framework, CMP | Compose Multiplatform Resources, moko-resources, Lyricist (all candidates, #169) | must run on Android and iOS; typed access preferred over string lookup |
| one catalog or two | one source catalog with generated Web and Kotlin consumers, or two resource sets with a parity check | the same semantic key must mean the same thing on both renderers |
| storage and switching | **proposed** key-value preference and app-owned locale; storage port, precedence and first-render behavior require review; existing roots are memory-only | switching never reloads unsaved Studio edits; this PR adds no storage authority |

Record the outcome with [`templates/adr.md`](../../../../templates/adr.md) and
[#167](https://github.com/loveoverflowcom/cantos/issues/167). A decision made
implicitly in a component is a defect, as for tokens ([`tokens.md`](tokens.md)).
Compare canonical generation and renderer-native catalogs fairly; **one SSOT is a
candidate, not a preapproved generator or library**. If distinct resources are
chosen, add a cross-renderer parity check (§ 8).

## 2. UI language and content language

Two languages are on screen at once, and they are not the same thing.

| | UI language | Content language |
|---|---|---|
| what it is | buttons, labels, errors, statuses, accessibility names | work and episode titles, synopsis, character names, dialogue text, script notes |
| owned by | the localization resources | the creator's script data ([`cantos-script-ir`](../../cantos-script-ir/SKILL.md)) |
| changes with the UI locale | yes | **never** |
| tagged for assistive technology | the page or app locale | its own language tag where it differs, e.g. `vi` inside an `en` label |

- **Never translate or rewrite content to fit the UI locale.** An English UI shows
  `Một lời hẹn` as written. Never store a UI string in content either: a new adaptation's
  narrator display name comes from the **adaptation's language**, not from whichever locale the
  creator's UI happens to be in.
- **Do not normalize content for display** beyond what the Script IR already guarantees (NFC text);
  the display layer renders what it is given
  ([`vietnamese-text.md`](../../cantos-script-ir/references/vietnamese-text.md)).
- **Tag mixed-language runs.** On the Web set `lang` on the content element; on CMP use the
  renderer's locale annotation for the span when available. Without it, a screen reader reads
  Vietnamese with the wrong voice and mispronounces tone marks
  ([`typography.md`](typography.md), [`accessibility.md`](accessibility.md#2-accessible-names-action--item--state)).
- **Sorting and search follow the content language**, with diacritics handled deliberately; this
  is a product decision to record, not a default of the sort function.

## 3. Semantic keys

Keys name a meaning, not the English copy. Group by surface, then by role.
The dotted names below are illustrative, not an accepted namespace/API or permission to
rename or consolidate the current catalogs; that ownership decision remains in #167.

| Good | Counterexample | Why |
|---|---|---|
| `studio.script.save.action` | `Save` | the text can change without breaking references |
| `theatre.player.bookmark.add.name` | `bookmark_button_1` | role and place are readable |
| `publish.blocker.qc_blocking_finding` | `error_17` | one key per domain variant, so the mapping is checkable |
| reuse an existing owner's cancel key for the same action | duplicate keys inside that owner for one meaning | reuse only when the **meaning** is identical; equal English wording does not establish shared ownership |

- A key is stable. Rename by adding the new key, migrating references and retiring the old one in
  one change; never repurpose a key for a different meaning.
- One key per domain variant for any state shown to a person: each `BlockReason`, `FailureReason`,
  `PauseCause`, QC finding and download failure has its own message
  ([`component-states.md`](component-states.md#2-availability-is-a-type)). A shared catch-all
  hides new variants.
- Accessibility names are strings: `...name` and `...description` keys sit beside the visible label
  and follow § 4 like any other copy.
- Developer-only text (logs, assertions, telemetry names, serialization fields, CSS classes, test
  IDs) is not localized and must not reach the UI.

## 4. Parameters, plurals and formatting

**No concatenation.** Word order differs between languages, so a sentence is one resource with
named parameters.

```text
studio.publish.action.name = "Xuất bản tập {title}, bản sửa đổi {revision}"
studio.publish.action.name = "Publish episode {title}, revision {revision}"
```

Never `t("delete") + " " + name`, and never a fragment that only makes sense after another one.
Do not `String::replace` iteratively on evolving output: title parameters may contain
literal `{time}`. Replace placeholders from the original template once, then test
parameter ordering, repeated placeholders and literal story titles.

**Plurals belong in the resource**, using the locale's CLDR rules. Vietnamese has a single plural
category; English has `one` and `other`. A resource for a count therefore has the categories its
locale needs and the code passes only the number.

```text
theatre.library.episode_count   vi: other = "{count} tập"
                                en: one = "{count} episode", other = "{count} episodes"
```

Never `if count == 1` in a view.

**Formatting uses locale-aware formatters, never hand-built strings.**

| Value | Rule |
|---|---|
| position and duration | compare renderer adapters against a reviewed output contract (`12:05`, `1:02:09` as target examples); current Web/CMP helpers are independent and hour-boundary support needs evidence; the accessible form is a spoken duration (`12 phút 5 giây`), not the clock text |
| dates and times | locale formatter; show time zone only when it matters; never an ISO string in the UI |
| numbers and percentages | locale grouping and decimal marks; a download percentage is an integer unless the design says otherwise |
| money | integer minor units plus currency from the domain ([cost and budget](../../cantos-production-pipeline/references/cost-and-budget.md#2-money-is-integers-with-a-currency)); format with the currency's own minor-unit count (VND has none) — never a float, never a hard-coded symbol |
| unavailable values | an explicit "unavailable" message, never `0`, `--` as the only signal, or an empty string |
| file sizes | one shared formatter with a stated unit base |

**Markup in messages** (a link or emphasis inside a sentence) uses a named slot the framework
supports, not HTML fragments concatenated around translated text.

## 5. Errors are typed, then mapped

A person never sees `error.to_string()`, a provider response body, a SQL message, a stack trace or
a raw HTTP status. The flow is:

```text
typed error or reason (domain / API contract)
        ↓  pure, exhaustive presentation function
message key + parameters + recovery action + (optional) support reference
        ↓
localized resource
```

- The mapping is a `match` with **no wildcard arm**, so a new error variant fails to compile or to
  pass the table test until it has copy.
- Every message says what happened in the user's terms, whether their work is safe, and what they
  can do next: retry, edit, wait, contact support. A recoverable failure keeps the user's input
  ([UI system](../../../../docs/design/ui-system.md#studio-text-first-production)).
- An unknown error maps to one explicit "unexpected" key that carries a support reference
  (a correlation or operation ID), never to an English string in the view.
- A server-supplied reason (for example a publication blocker) is a **code plus parameters**; the
  client localizes it. Do not render server prose directly: it will not follow the UI locale.
- Announce important errors through the renderer's live-region mechanism without repeatedly
  interrupting screen-reader users ([`accessibility.md`](accessibility.md)).

## 6. Layout under text expansion

Vietnamese and English differ in length per phrase, and either can be longer. Content names can be
arbitrarily long.

- Do not size controls by one locale's label. Allow wrapping for labels and keep actionable text
  visible at 200% zoom and large font scales ([`typography.md`](typography.md)).
- Test with a **pseudo-locale** (proposed): expand by roughly a third, add stacked diacritics and
  bracket the string so truncation and concatenation are visible at a glance. A pseudo-locale is a
  development aid and never ships as a user choice.
- Use realistic Vietnamese in captures: `Người dẫn chuyện`, `Ngày mai, mình có diễn tiếp không?`,
  `Ánh đèn cuối sân khấu`; plus one long title and one long character name per list.
- Never truncate an actionable label to fit. Truncating content with an accessible full name is a
  deliberate design choice recorded per component.

## 7. What must be localized

All text that can reach a person: page and section titles, navigation, buttons, field labels,
placeholders, helper and validation text, dialog copy, toasts, empty and loading messages, tooltips,
menus, filter and sort names, table headers, status and enum labels, notifications, and every
accessibility name, description and alternative text. When in doubt it is visible — localize it.

## 8. Checks

| Check | How | Level |
|---|---|---|
| new displayed literals | bounded changed-line tokenizer checks known Web/Compose/HTML sinks with reviewed owners/exceptions in `scripts/localization_allowlist.json`, through the repository gate | `statically-checked`; indirect arbitrary dataflow still needs review |
| key parity | nonblank vi-VN/en pairs for recognized changed catalog forms; renderer catalog tests cover their typed APIs | `statically-checked`, `example-tested`; not a universal orphan-key check |
| placeholder parity | matching named parameter multiplicities in recognized paired resources | `statically-checked`, `example-tested` |
| plural coverage | each locale supplies the categories it needs | `example-tested` |
| error and reason mapping | table test: every variant maps to a key that exists | `example-tested` |
| `lang` and locale tags | a DOM or semantics test that mixed-language spans are tagged | `DOM-tested`, `semantics-tested` |
| layout | pseudo-locale and long-name captures, **opened** | `screenshot-inspected` |

Partial checks exist already; their source is not proof they ran in a particular report:
- Theatre Web [`messages.rs`](../../../../apps/cantos-theatre/web/src/messages.rs),
  `every_resource_has_both_locales_and_matching_parameters`, checks nonblank pairs and
  placeholder multiplicities plus selected count/duration/speed examples. Separate tests
  preserve literal title braces and original-template interpolation. These tests alone do
  not cover every Web copy owner.
- CMP [`preview-sandbox/.../DemoCopyTest.kt`](../../../../apps/cantos-theatre/mobile/preview-sandbox/src/commonTest/kotlin/com/cantos/theatre/demo/DemoCopyTest.kt)
  exercises shared `ShellCopy`: nonblank pairs, placeholder names/multiplicity, literal
  title braces, selected numeric labels and spoken time. It does not establish all
  auth/membership/launch or sandbox-override parity, or native locale switching.

There is **no unified cross-renderer parity gate or `cargo xtask i18n check` command** yet.
The bounded localization primitive is integrated into `cargo xtask verify repository`;
its fixtures test regressions and allowed technical/diagnostic/content examples before
integration. It does not prove runtime locale switching or complete dynamic error mapping.
Use the existing renderer gates in [xtask](../../../../xtask/README.md) for those layers.
Report only checks actually run.
Prefer a focused resource test during DEV, affected renderer evidence at CHECKPOINT,
and handoff preflight on the final diff; no full verification after each small edit.

## 9. Rule cards

| Rule | Failure mode | Good | Counterexample | Oracle | Status · exception |
|---|---|---|---|---|---|
| Every visible and accessibility string is a resource | untranslatable UI; English leaking into Vietnamese screens | `studio.publish.action` in every locale | `"Publish"` in a view | literal scan | proposed · logs and developer diagnostics |
| Content is never translated or stored as UI | an English UI rewrites a Vietnamese title | the title rendered as stored, tagged `vi` | translating a work title | review; tag test | proposed · none |
| Keys name meaning and are stable | broken references; one key, two meanings | rename by add, migrate, retire | repurposing `common.ok` | key-parity check | proposed · none |
| Parameters, not concatenation | untranslatable word order | `"Xuất bản tập {title}"` | `t("publish") + title` | review; placeholder parity | proposed · none |
| Plurals in resources | wrong forms in a language with different categories | CLDR categories per locale | `if count == 1` in a view | plural coverage test | proposed · none |
| Errors are typed then mapped exhaustively | raw provider or SQL text shown; a new variant unhandled | `match` with no wildcard arm | `error.to_string()` | mapping table test | proposed · none |
| Unavailable is not zero | a missing estimate reads as free | an explicit "unavailable" message | `0 ₫` for an unknown cost | presentation table test | proposed · none |
| Realistic Vietnamese in every capture | diacritic and wrapping defects hidden by ASCII | captures with stacked diacritics and long names | "Lorem ipsum" | opened captures | manual · none |

## 10. Evidence

| Claim | Oracle | Honest label |
|---|---|---|
| no hard-coded visible string | literal scan with a reviewed allow-list | `statically-checked` |
| locales are complete and consistent | key and placeholder parity | `statically-checked` |
| every typed error or reason has copy | exhaustive mapping table | `example-tested` |
| mixed-language content is tagged | DOM or semantics assertion | `DOM-tested` / `semantics-tested` |
| layout survives expansion and diacritics | pseudo-locale and long-name captures, opened | `screenshot-inspected`, `cross-viewport-inspected` |
| the copy reads naturally in Vietnamese | a named native-speaker review | recorded reviewer; not an automated claim |
