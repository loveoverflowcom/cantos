# Private Work artwork v1

This contract defines one owner-only, externally prepared PNG/JPEG Work cover. Shared raw
DTOs, JSON Schema and the opt-in Protobuf edge representation are implemented independently
of runtime activation. The [archived reviewed plan](../../../../../../docs/archive/superseded-plans/012-private-work-artwork.md) records
product policy, storage prerequisites and layered acceptance. A private admitted image is
neither a publication approval nor public delivery. No client UI/native route is added.

## Metadata and byte boundaries

Only new `/api/v1/studio/works/{work}` artwork paths support JSON default and opt-in
`application/protobuf`. JSON errors remain the existing ApiError, including for Protobuf
requests/success responses. Existing Catalog, Theatre, Publication, authentication, audio,
MessagePack and the four Studio revision/validation pilot routes are unchanged.

- POST `image-intents`: CreateImageIntentRequest → ImageIntentDocument
- GET `image-intents/{id}`: ImageIntentResponse with exact intent, nullable asset and derived state
- PUT `image-intents/{id}/bytes`: actual PNG/JPEG bytes → ImageAssetDocument
- GET `image-assets/{id}`: exact immutable ImageAssetDocument
- GET/HEAD `image-assets/{id}/bytes`: exact private accepted bytes with inspected MIME/length
- GET `artwork?work_revision=R`: exact WorkArtworkResponse, current head when R is omitted
- PATCH `artwork`: SetWorkArtworkRequest → original immutable WorkArtworkResponse

The sibling [cover disclosure v1](../../cover-disclosure/v1/README.md) records the owner's
separate public-display decision for one admitted image; it is mounted with these routes and is
JSON only. Neither admission nor selection here implies that decision.

Methods above describe the reviewed surface, not a runtime activation claim. All reads/writes
require the active Work owner. Current ownership, rights, expiry, lifecycle, exact asset binding
and optimistic revision are rechecked by application composition. Private storage locations
never appear in metadata. Binary image transfer is separate from metadata negotiation:
no JSON/base64/multipart/Protobuf byte wrapper, URL ingestion, thumbnail generation or public grant.

## Required presence

The [JSON Schema](artwork.schema.json), [raw DTOs](../../../../src/contract/studio/artwork.rs) and
[Protobuf authority](artwork.proto) name the ten metadata messages.
Every non-nullable scalar/message is required. Protobuf scalar `optional` preserves presence;
zero/empty present values remain raw values for domain validation, never manufactured defaults.
Missing required nested messages fail. Unknown/reserved fields and duplicate singulars fail.

ImageMime is exactly `image/png` or `image/jpeg`; wire enum 1 or 2. ImageIntentState is
`pending`, `admitted`, or `expired`; wire enum 1, 2 or 3. Enum zero/unknown values fail.
Raw u64 timestamps/revisions may represent the full unsigned width; domain validation bounds
persisted timestamps to nonnegative i64 and revisions to 1..i64::MAX. Dimensions are raw u32;
the prescan rejects overflow before narrowing. IDs/digests/profiles remain raw strings and are
validated by domain constructors.

SetWorkArtworkRequest requires `artwork: null` to clear or a complete WorkArtworkSelection
to set/replace. Omission fails and cannot clear a cover. Protobuf requires exactly one arm:
field 3 artwork or field 4 empty ClearArtwork. Absent, duplicate, both-arm and nonempty-clear
messages fail before prost's last-oneof-wins behavior. Response artwork/asset/changed_at_ms
absence maps to explicit JSON null. Repeated rights absence maps to an empty list, subsequently
rejected by admission's 1..64 distinct owner-bound evidence policy.

## Bounds and receipts

Artwork metadata prescan is allocation-free before prost decoding: <=32768 bytes, depth <=4
(root depth 1), <=256 total fields and <=64 rights entries per ImageDeclarations. It checks
canonical varints, lengths/cursors, exact wire types, UTF-8, enum values, u32 width, duplicates,
oneof and unknown/reserved fields. All reviewed field numbers fit a checked u16 seen mask.
These bounds are independent of the unchanged older pilot's 8 MiB/65536 issue budget.
Encoding applies the same new bounds. JSON HTTP admission applies the separate 32 KiB body bound.

The deepest legal schema path is response → intent → request → declarations (depth 4).
Legal v1 fields cannot exhaust 256 before per-declaration rights limits; focused unit tests
exercise the shared depth/field-budget primitive independently rather than invent a legal
recursive v1 message. Finite hostile byte tests are example/property evidence, not fuzzing.

New declarations canonicalize distinct rights refs in ascending UTF-8 token order before
new-write receipts and fingerprints. Provenance/alt Unicode is preserved without normalization;
old Catalog arrays and receipts retain exact bytes. JSON/Proto encode the same semantic intent.
Domain digests use fixed ordered preimage structs excluding their digest and storage columns:
ii-r1 intent, ia-r1 asset, wa-r1 change, image-operation1 command. Replay binds the command
kind, path Work and complete validated request, including explicit set versus clear. Cover changes
advance Catalog metadata only, preserve historical receipts/bytes and never rewrite releases.

## Evidence

The [independent corpus](fixtures/README.md) is checked by Python, production Rust,
pinned isolated prost/protoc generation drift and the fixture-only Kotlin harness. The old
Studio schema/generated types and 13 goldens stay exact. Corpus agreement proves representation
compatibility; HTTP, PostgreSQL, storage, issuer, browser and native runtime acceptance are separate.

### Executed metadata evidence, 2026-10-13 UTC

- `cargo test --locked --offline -p cantos-contracts`: PASS, 87 tests including 9 new artwork
  tests and 3 new prescan tests. Existing JSON, MessagePack, audio, Catalog, Theatre and pilot
  fixture suites passed unchanged. No ignored test was promoted to execution evidence.
- `cargo clippy --locked --offline -p cantos-contracts --all-targets -- -D warnings`: PASS.
- `python3 scripts/reference_protobuf.py`: PASS, 13 pilot + 38 artwork independent byte vectors.
- `python3 -m unittest discover -s scripts -p test_reference_protobuf.py -v`: PASS, 11 tests.
- Pinned isolated prost 0.13.5/protoc generation and both corpora: PASS, exact generated-text
  comparison and 13 pilot + 38 artwork vectors. The focused production Rust run also consumes
  42 independent hostile artwork vectors with exact typed rejection categories.
- Source formatter checks and `git diff --check`: PASS for the wire worker's final source scope.

- Final `cargo test --locked --offline -p cantos-contracts --test artwork_wire`: PASS, 9 tests
  after adding exact malformed-trailing-data versus missing-child-presence priority assertions.
- Kotlin 2.2.20 / kotlinx.serialization 1.9.0: PASS, 51 actual serializer vectors (13 pilot +
  38 artwork), including enum serializers, `@ProtoOneOf` and full unsigned raw bits.
- Fixture-only commonTest on JVM/JUnit 4.13.2: PASS, 3 methods over 38 goldens, 42 hostile
  inputs and depth/field/mixed-error probes. No Gradle, Android/iOS or native runtime run is claimed.
- Warm schema drift on an isolated temporary copy: baseline PASS; changing CreateImageIntentRequest
  content_type at tag 5 from ImageMime to int32 rebuilt the cached oracle and failed the exact
  generated-text comparison (expected exit 101); restoring the copied schema returned PASS.
  The shared authoritative schema/generated files were never mutated by the experiment.
- `cargo clippy --locked --offline --manifest-path scripts/protobuf-oracle/rust/Cargo.toml
  --all-targets -- -D warnings` with the isolated shared oracle target: PASS.

- Final `cargo test --locked --offline -p cantos-contracts`: PASS, 87 tests after restoring
  the old pilot's wrong-wire/length-before-issue-cap ordering and adding exact at-cap regressions.
  Final contracts all-targets clippy: PASS.
- Exact `preflight::select::tests::protobuf_inputs_select_drift_goldens_and_both_language_consumers`
  xtask regression: PASS, 1 test (148 filtered). Artwork Protobuf schemas/fixtures/adapters/test changes
  retain the existing conservative ProtobufReference/TheatreCmpDemo and contract-dependent gates.
- Isolated ordering mutation: the production scanner source snapshot with early item counting
  returned TooLarge for legacy/new wrong-wire-at-cap inputs; desired Shape assertions failed
  (expected exit 101). Corrected production tests pass. This is one named mutation probe,
  not broad parser mutation coverage. No historical cached-library failure is claimed because
  the captured cache had already been rebuilt with the correction.

These establish example/differential/static metadata evidence only. Aggregate xtask gates,
PostgreSQL/HTTP/storage/issuer, browser, Android/iOS and production acceptance are separate. Hostile corpus and finite short-byte loops are not a
fuzzing claim. Full logs are in ignored `artifacts/agent-runs/issue104-wire/` and
`artifacts/agent-runs/issue104-artwork-oracles/` for the implementation session.

The following legacy authorities were compared byte-for-byte against the feature's starting
HEAD and remain unchanged. Before/after SHA-256 is identical for each:

- `protobuf/studio_v1.proto`: `56f63aa7464f7edbc5b26ccced5dbe79fb033fcd5b3a034f1875590758262234`
- `api/src/protobuf/generated.rs`: `e180e713002cb4a590295271c4f2133565638d351a043f3688bc2bc0d93c8197`
- `fixtures/protobuf/v1/goldens.json`: `f9370eed5574a6a4066dfccbe50a37be6033bf05d76a4973371fa4d48f02e829`

## Implemented HTTP checkpoint and safe activation

The adapter lives only under the current Studio mount, with no deprecated alias. The shipped
host mounts artwork when the existing private storage topology is configured through
`Store::with_audio_storage` or `Store::with_storage_config`; without private storage it remains
unmounted. `router_with_private_artwork` is a trusted composition override. `ArtworkService::new`
uses the reviewed shared registry, filesystem and S3 adapters with a sealed inspected image
payload and actual MIME. Every private-artwork composition disables same-process static files.
No provider, database, production or public-delivery acceptance is implied by activation.
See the [runtime ledger](../../../../../../docs/evidence/private-work-artwork.md).

Current Studio cookie selection and session fencing apply before handler/body admission.
Artwork reads accept the middleware-owned `studio_session_id` query fence alongside the
optional exact `work_revision`; unknown/duplicate business fields remain invalid. A forged
or duplicate fence is refused by the existing Studio middleware.

Metadata request/success formats are negotiated separately. Omitted/equal/wildcard Accept uses
JSON; explicit Protobuf uses the same strict version/parameter policy as the older pilot.
MessagePack, unsupported Protobuf version/encoding and content encoding are refused. Accept
refusal happens before body extraction/use-case effects. All errors stay JSON ApiError; new
metadata responses carry `Vary: Accept`, `Cache-Control: no-store` and `nosniff`.

- Prepare success, including exact replay: 201.
- PUT admission success, including exact replay: 200.
- Metadata/artwork/preview reads and artwork PATCH success: 200.
- Malformed metadata/body/query: 400 `invalid_request`.
- Session failure: 401 `unauthenticated`; absent/foreign Work or asset: 404 `not_found`.
- Unsupported request representation/encoding: 415 `invalid_request`; unsupported Accept: 406.
- Metadata >32 KiB or image body >8 MiB: 413 `invalid_request`.
- Invalid image declarations, actual decode/type/hash/count or asset/alt binding: 422
  `invalid_request` with safe field/rule issues; absent rights evidence: 422 `evidence_unavailable`.
- Expired intent or duplicate intent: 409 `invalid_request`; reused operation: 409 `operation_reused`.
- Stale Work revision: 409 `stale_revision` with current revision; archived new writes follow
  existing Catalog lifecycle conflict mapping. Equal historical operation replay remains exact.
- Busy decoder/storage unavailable: 503 `unavailable`; corrupt stored facts/object evidence:
  500 `corrupt_revision`. No provider/path/credential text is returned.

Raw PUT accepts only exact `image/png` or `image/jpeg` with no Content-Encoding. Its success
metadata still negotiates JSON/Protobuf; the image body itself does not. Active owner/intent
checks precede admission, which is acquired before buffering. GET/HEAD preview uses inspected
MIME, byte length and quoted SHA-256 ETag. HEAD verifies/open-reauthorizes then drops the stream;
Range/conditional headers do not confer a grant or change the full-byte 200 policy. Actual
loopback HTTP, database, target/version resolution and storage readback remain separate pending
acceptance. Executed Router probes establish extractor/header/limit and composition behavior only.
