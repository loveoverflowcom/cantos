# Private artwork v1 independent wire corpus

Authority: [artwork.proto](../artwork.proto), package
`cantos.studio.artwork.v1`, and the [reviewed artwork plan](../../../../../../../docs/archive/superseded-plans/012-private-work-artwork.md#7-json-and-protobuf-contract-scopepresence).
These are synthetic wire/DTO fixtures, not admitted images, rights evidence, stored receipt
integrity or public artwork. Existing pilot generated types and all 13 pilot goldens remain unchanged.

## Corpus contract

- `goldens.json`: 40 independently authored values/byte expectations across all ten schema
  messages. `value` is raw JSON/application shape, including explicit `artwork: null` for clear.
  Optional `wire_value` records a proto-only shape when needed for the clear arm or enum zero.
  `application_valid` means complete raw DTO presence/types/closed enums, not domain validity:
  present empty strings, zero revisions, full uint64 and uint32 maxima deliberately survive the
  transport and still require domain validation. Missing response nullable fields normalize to
  explicit JSON null; an omitted command `artwork` never normalizes to clear.
- `hostile.json`: 42 exact-category strict-admission vectors. Null `error` means admitted wire/DTO
  shape, not authorized domain success. Closed enum zero/unknown/full-u64, both duplicate-oneof
  orders, duplicate clear/scalars/nested fields, reserved fields, wrong wire types, malformed
  canonical varints, overflow, truncation and invalid UTF-8 are represented. Rights entries have
  exact 64/65 cases; the envelope has a 32,769-byte rejection case.
- Both-direction goldens preserve Unicode, decomposed Vietnamese marks, quotation marks,
  backslashes and dollar signs. Numeric cases distinguish absent scalars from present zero;
  selected/clear/never-selected history responses preserve nullable timestamps.
- Receipt digests use synthetic correctly shaped `ii-r1:sha256:` / `ia-r1:sha256:` values. No
  fixture claims that these synthetic suffixes equal the corresponding domain document hash.

The Python wire-spec encoder/strict reader is independent of production Rust and Kotlin.
`cargo xtask verify repository` runs its schema/enum/oneof/presence/budget/corpus tests.
`cargo xtask verify contracts` / the existing `protobuf-reference` gate compare isolated pinned
prost/protoc output exactly against both checked-in generated modules and compare bytes/values
against both independent corpora. Both external schemas are registered for warm Cargo rebuilds.
Expectations must never be copied from the implementation under test.

## Schema limits and Kotlin scope

The deepest legal v1 path is response → intent → request → declarations, four levels counting
root. Legal messages cannot exceed 256 fields before the per-declaration 64-rights cap binds.
Schema-hostile deeper/more-field constructions necessarily encounter an earlier shape/item
failure. Independent unit probes therefore tighten depth/field budgets on legal input, and
exercise the exact 256/257 aggregate boundary with only the fixture item bound relaxed. These
are bounded example tests, not fuzzing or a formal proof.

The existing Theatre Gradle commonTest harness embeds this corpus and the isolated Kotlin
schema mirror in a separate test package. It exercises actual Kotlin enum serializers and
`@ProtoOneOf` sealed alternatives plus uint64 raw bits. Uint32 uses nonnegative Long values
in this fixture mirror to avoid signed-Int varint expansion; the independent fixture scan
checks the uint32 bound in the test-only admission reader. Closed-enum/duplicate-oneof/budget
rejection is checked without invoking a serializer on hostile input. Nullable wire mirrors still
round-trip explicit invalid presence/default cases to prove that wire presence is not application
authority.

No artwork code is added to commonMain, native HTTP routes, playback, UI or a shipping serializer.
Kotlin/JVM fixture evidence does not establish Android/iOS compilation or device behavior;
real storage, SQL, rights ownership, image decoding and HTTP acceptance are separate gates.

## Focused executed evidence, 2026-10-13 UTC

- Independent Python schema/byte/value/hostile/harness tests: **11 tests PASS**.
- Pinned isolated prost 0.13.5/protoc exact generated-text check: **13 pilot + 38 artwork
  vectors PASS**, with both schema authorities tracked for warm regeneration.
- Actual Kotlin 2.2.20, its matching serialization compiler plugin and kotlinx serialization
  core/protobuf JVM 1.9.0: **51 standalone schema vectors PASS**. This includes enum zero
  presence, actual sealed `@ProtoOneOf` set/clear alternatives, full uint64 and uint32 maxima.
- The generated commonTest fixture mirror and `ArtworkFixtureCompatibilityTest` compiled for
  JVM 17 and executed with JUnit 4.13.2: **3 methods PASS**, checking all 38 goldens, all 42
  hostile cases, exact depth/global-field bounds and prescan-before-presence error priority.
- Existing pilot generated output and 13-golden files: **byte-identical to baseline**.

These are focused independent wire/JVM fixture results. Full Gradle, Android/iOS compilation,
Native/device execution, PostgreSQL/storage/HTTP/image admission and aggregate preflight remain
separate evidence. No native artwork runtime was added or activated.
