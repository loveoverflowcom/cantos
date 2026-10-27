# Semantic presentation resources

[ADR0050](../../docs/decisions/0050-localization-semantic-catalog.md) records the owner's
26 October 2026 approval. `catalog/v1/catalog.json` is the versioned JSON source for Studio
and Theatre Web plus Theatre CMP text resources. Apps own generated projections and domain-to-message mappings; shared UI
receives labels from callers. No service DTO or content language is generated here.

Run from the repository root:

```sh
python3 scripts/localization_catalog.py --resources
python3 scripts/localization_catalog.py --resources --check
python3 -m unittest discover -s scripts -p 'test_localization*.py'
cargo test -p cantos-localization
```

`catalog/v1/schema.json` describes structural shape. `scripts/localization_resources.py` is
the strict executable validator for cross-field completeness, blank text, named arguments,
plural categories, symbol collisions and deterministic drift. It uses the original-template
parser from #505; inserted titles remain opaque. Plain keys accept no arguments. Parameterized
messages expose required named Rust fields (`text: &str`, `count: u64`) and return typed errors.
Kotlin text messages expose sealed constructors with required named `String` fields; generated
expressions insert those values as opaque text. CMP namespaces use explicit `cmp_*` platform
variants and are excluded from Web projections. `UiLocale`, its canonical saved tags, autonyms,
direction and default are generated into the existing preference owner; `ShellLocale` is a
read-only generated representation in the presentation leaf. Generated exhaustive adapters
connect it to the single preference authority without a handwritten two-language switch.
The v1 production AST supports plain text and cardinal plural nodes. Closed domain states map
to generated plain keys in app adapters; the older #505 closed-select fixture does not imply
that a general nested message/ICU syntax is implemented in this schema.

Breaking schema/argument changes require a new major source directory and explicit consumer
migration. Text additions/changes regenerate app files and the digest manifest. Every declared
locale must translate every key and every required plural form. No missing-key fallback exists.
Preview messages are excluded through each app's existing preview/debug build boundary.

Release UI locales are `vi-VN` and `en`; choices, aliases, autonyms and direction derive from
resources. Unsupported saved choices visibly use the approved Vietnamese default. Web stores
only explicit choices, separately per app, under `cantos.<app>.ui-locale.v1`. There is no device
or browser negotiation. Storage refusal produces an explicit localized notice; the current
session choice remains usable. This is distinct from the existing CMP atomic store controller.
Content tags are independent and unknown tags such as `zh-Hant` stay visible unchanged.

`catalog/tests/third-locale.json` has real Japanese translations for a small test namespace.
The generator exposes it in `rust/tests/generated/third_locale.rs` and a **text-only** CMP
`commonTest` projection. Release output excludes `testOnly` locales; full Japanese app coverage
and Japanese CMP plural/formatting support are neither fabricated nor claimed.

The renderer-neutral `rust/` adapter pins ICU4X 2.3 and compiled CLDR 48.2.1 data in Cargo.lock.
Qualified requests are exact decimal strings, cardinal plurals (including fractional operands),
integer minor units for USD/VND, numeric player clocks, speed hundredths and Gregorian instants
with explicit UTC/fixed offsets. Date presentation uses the browser's actual offset at that
instant, includes the offset, and uses UTC in native HTML tests. IANA zone rules, arbitrary
currencies, general duration styles and alternate calendars remain unqualified; unsupported
requests are typed refusals. Final FE adapters show explicit localized format-unavailable copy.

The native Rust and WASM/Node probe results belong to [the #506 evidence](../../docs/evidence/web-localization-506.md).
They establish neither Kotlin ICU4X bindings nor Android/iOS support. The [#507 record](../../docs/evidence/cmp-localization-507.md)
separates generated CMP text, registry and renderer evidence from missing native formatter
qualification. The original numeric shell namespace (nine keys, speed/spoken time/clock) and
auth retry-after wording retain working vi/en behavior in explicit compatibility owners.
`localization_kotlin.py` rejects numeric/plural requests before projection; it supplies no
English, ISO, or handcrafted replacement formatter. Exhaustive compatibility matches force a
decision before another release locale compiles. ICU4X bindings and each native target still
need actual qualification before those consumers move. The original #505 fixture remains
unchanged, source-bound evidence; no Web app is imported into CMP.
