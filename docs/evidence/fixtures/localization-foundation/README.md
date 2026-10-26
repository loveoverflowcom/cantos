# Localization foundation experiment

**Proposal fixture only**, for [ADR 0050](../../../decisions/0050-localization-semantic-catalog.md)
and #505's pending-#167 branch. No Cargo workspace member, Gradle source set, shipping import,
application locale authority or locale-aware formatter is added.

`catalog.json` is a strict version-1 executable draft schema; `scripts/localization_catalog.py`
validates its exact fields, semantic IDs, named types, per-locale completeness, placeholder
multiplicity, selector and integer plural coverage. `generated/manifest.json`, `catalog.rs`
and `Catalog.kt` are deterministic projections. `vectors.json` is a hand-authored oracle and
must never be regenerated from either renderer. The catalog digest ignores JSON object order;
locale array order deliberately defines stable fixture enum ordinals, not a persisted wire ID.

`vi-VN`/`en` are candidate UI catalogs. `ja` is test-only and excluded by default from the
Python resolution experiment. The fixture compiler API deliberately includes all three to
exercise generation; it is not an app language picker. A new locale needs registry metadata
and complete translations, with no edits to a handwritten view/boolean switch. A fourth
test fixture (`ko`) checks that generation has no fixed third-locale special case.

From the repository root, primitives of the existing `repository` gate:

```sh
python3 scripts/localization_catalog.py          # regenerate only the three fixture outputs
python3 scripts/localization_catalog.py --check  # whole catalog, vectors, inventory and drift
python3 -m unittest discover -s scripts -p 'test_localization*.py'
cargo xtask verify repository
cargo xtask preflight --changed-only --base origin/develop
```

The ordinary test suite executes the Rust projection with installed `rustc`, including a
compile-negative wrong-count probe. Set `RUSTUP_TOOLCHAIN=1.98.1` to match the repository.
The optional Kotlin test needs **already installed** Kotlin 2.2.20 compiler jars:

```sh
CANTOS_KOTLIN_COMPILER_CP='<compiler + compiler dependencies, separated by platform path separator>' \
CANTOS_KOTLIN_STDLIB='<kotlin-stdlib-2.2.20.jar>' \
python3 -m unittest discover -s scripts -p test_localization_catalog.py -v
```

The compiler classpath contains kotlin-compiler-embeddable 2.2.20, kotlin-stdlib 2.2.20,
kotlin-script-runtime 2.2.20, kotlin-daemon-embeddable 2.2.20, kotlin-reflect 1.6.10,
kotlinx-coroutines-core-jvm 1.8.0 and JetBrains annotations 13.0 for the executed Mac test.
Use the compiler's pinned dependency graph; do not search random versions or download a new
toolchain as part of a repository check. The test invokes `K2JVMCompiler -no-stdlib -no-reflect
-Werror`, then runs its jar with the supplied stdlib. Without explicit classpaths it reports
an honest skip. This compiler test has no Compose/Android/iOS dependency or runtime claim.

Native tests compile generated types, execute the same 22 expected vectors and require an
incorrect text count to fail compilation. Reference-resolution tests separately check `en-GB`,
case, extensions, `vi` alias, unsupported `zh-Hant`, visible missing-key/argument diagnostics,
test-only exclusion, 0/1/2 plurals and closed select. Literal content braces/markup/format tokens
remain opaque text. Renderers still need real escaped-text/attribute and semantics tests.

The CLDR subset is whole-number `one`/`other` only. `integerOne` enumerates exact whole counts;
`other` means the remainder. It is sufficient for these three fixture languages, not a general
CLDR implementation. Decimal/date/duration/currency formatting, native locale negotiation,
saved preference, mixed-language spans, RTL/layout and error-enum-to-message adapters are
outside this executable fixture. The ADR states the approval and evidence needed before adoption.
