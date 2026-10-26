# Semantic presentation resources

[ADR0050](../../docs/decisions/0050-localization-semantic-catalog.md) records the owner's
26 October 2026 approval. `catalog/v1/catalog.json` is the versioned JSON source for Studio
and Theatre Web. Apps own generated projections and domain-to-message mappings; shared UI
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
The same generator exposes it only in `rust/tests/generated/third_locale.rs`. Release output
excludes `testOnly` locales; full Japanese app coverage is neither fabricated nor claimed.

The renderer-neutral `rust/` adapter pins ICU4X 2.3 and compiled CLDR 48.2.1 data in Cargo.lock.
Qualified requests are exact decimal strings, cardinal plurals (including fractional operands),
integer minor units for USD/VND, numeric player clocks, speed hundredths and Gregorian instants
with explicit UTC/fixed offsets. Date presentation uses the browser's actual offset at that
instant, includes the offset, and uses UTC in native HTML tests. IANA zone rules, arbitrary
currencies, general duration styles and alternate calendars remain unqualified; unsupported
requests are typed refusals. Final FE adapters show explicit localized format-unavailable copy.

The native Rust and WASM/Node probe results belong to [the #506 evidence](../../docs/evidence/web-localization-506.md).
They establish neither Kotlin bindings nor Android/iOS support. #507 must qualify native
bindings, Kotlin generation, locale override and formatter parity before CMP adoption; it must
not import either Web app or copy their generated translations. The original #505 fixture and
its Kotlin compiler evidence remain unchanged, source-bound experiments.
