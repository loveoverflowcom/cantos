package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.MaxPageItems
import com.cantos.theatre.domain.catalog.MaxSafeMillis
import com.cantos.theatre.domain.catalog.PublicUnitId
import com.cantos.theatre.domain.catalog.PublicUnitKind
import com.cantos.theatre.domain.catalog.PublicUnitPage
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.domain.catalog.PublicWorkPage
import com.cantos.theatre.domain.catalog.PublicationId
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.SerialKind
import kotlinx.serialization.descriptors.StructureKind
import kotlinx.serialization.descriptors.elementNames
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue

/**
 * **Drift check, not code generation.** The Kotlin wire DTOs are hand-written; this test pins
 * them to the shared strict JSON Schema (property names, required set, JSON types, nullability,
 * enum values, `additionalProperties:false`) and decodes/re-encodes every default-v1 fixture.
 * Presentation-only opt-in fixtures must be refused by these default DTOs. A schema or fixture
 * change in `services/cantos-server/contracts/theatre/discovery/v1` fails here until the DTOs and
 * fixture inventory are reviewed.
 */
class PublicCatalogWireDriftTest {
    private val defs: JsonObject =
        Json.parseToJsonElement(CanonicalDiscoveryCorpus.schemaJson).jsonObject.getValue("\$defs").jsonObject

    private val dtoByDefinition: Map<String, KSerializer<*>> = mapOf(
        "WorkSummary" to WorkSummaryWire.serializer(),
        "WorkPage" to WorkPageWire.serializer(),
        "UnitSummary" to UnitSummaryWire.serializer(),
        "UnitPage" to UnitPageWire.serializer(),
        "ListenerError" to ListenerErrorWire.serializer(),
        "PublicationManifest" to ManifestWire.serializer(),
        "PlaybackAuthorization" to PlaybackGrantWire.serializer(),
    )

    private enum class Shape { WorkPage, UnitPage, ListenerError, Manifest, PresentedWorkPage, PresentedUnitPage }

    /** Every shared fixture must be classified; a new file fails until reviewed. */
    private val fixtureShapes = mapOf(
        "works.json" to Shape.WorkPage,
        "empty.json" to Shape.WorkPage,
        "units.json" to Shape.UnitPage,
        "units_empty_continuation.json" to Shape.UnitPage,
        "units_end_empty.json" to Shape.UnitPage,
        "units_republished.json" to Shape.UnitPage,
        "not_found.json" to Shape.ListenerError,
        "invalid_cursor.json" to Shape.ListenerError,
        "invalid_request.json" to Shape.ListenerError,
        "unavailable.json" to Shape.ListenerError,
        "rendition_unavailable.json" to Shape.ListenerError,
        "manifest.json" to Shape.Manifest,
        // #251 requires presentation=1; the native client continues requesting default v1.
        "presented_works.json" to Shape.PresentedWorkPage,
        "presented_units.json" to Shape.PresentedUnitPage,
    )

    private fun definition(name: String): JsonObject = defs.getValue(name).jsonObject

    private fun SerialDescriptor.requiredNames(): Set<String> =
        (0 until elementsCount).filterNot(::isElementOptional).map(::getElementName).toSet()

    @Test
    fun every_dto_matches_its_schema_definition_names_required_set_and_strictness() {
        dtoByDefinition.forEach { (name, serializer) ->
            val schema = definition(name)
            val descriptor = serializer.descriptor
            assertEquals(JsonPrimitiveFalse, schema["additionalProperties"], "$name must be strict in the schema")
            assertEquals(schema.getValue("properties").jsonObject.keys, descriptor.elementNames.toSet(), "$name property names")
            assertEquals(
                schema.getValue("required").jsonArray.map { it.jsonPrimitive.content }.toSet(),
                descriptor.requiredNames(),
                "$name required set (every v1 field is required, including nullable next_cursor)",
            )
        }
    }

    @Test
    fun every_dto_property_matches_its_schema_json_type_nullability_and_enum_values() {
        val definitionBySerialName = dtoByDefinition.entries.associate { (name, serializer) -> serializer.descriptor.serialName to name }
        dtoByDefinition.forEach { (name, serializer) ->
            val properties = definition(name).getValue("properties").jsonObject
            val descriptor = serializer.descriptor
            for (index in 0 until descriptor.elementsCount) {
                val field = descriptor.getElementName(index)
                val element = descriptor.getElementDescriptor(index)
                val property = properties.getValue(field).jsonObject
                val label = "$name.$field"
                val alternatives = property["anyOf"]?.jsonArray?.map { it.jsonObject }
                val nullable = alternatives?.any { it["type"]?.jsonPrimitive?.content == "null" } == true
                assertEquals(nullable, element.isNullable, "$label nullability")
                val shape = alternatives?.single { it["type"]?.jsonPrimitive?.content != "null" } ?: property
                val enumValues = shape["enum"]?.jsonArray?.map { it.jsonPrimitive.content }
                when {
                    shape["const"] != null -> {
                        val constant = shape.getValue("const").jsonPrimitive
                        val kind = when { constant.isString -> PrimitiveKind.STRING
                            constant.booleanOrNull != null -> PrimitiveKind.BOOLEAN
                            else -> PrimitiveKind.LONG }
                        assertEquals(kind, element.kind, "$label constant wire type")
                    }
                    enumValues != null -> {
                        if (name == "PublicationManifest" && field == "profile_id") {
                            assertEquals(PrimitiveKind.STRING, element.kind, "$label validated by playback adapter")
                            assertEquals(listOf("opus-mono-20-v1", "opus-mono-24-v1", "opus-mono-32-v1", "opus-mono-40-v1"), enumValues)
                        } else {
                            assertEquals(SerialKind.ENUM, element.kind, "$label is an enum")
                            assertEquals(enumValues, element.elementNames.toList(), "$label enum values and order")
                        }
                    }
                    else -> when (val type = shape.getValue("type").jsonPrimitive.content) {
                        "string" -> assertEquals(PrimitiveKind.STRING, element.kind, label)
                        "integer" -> assertEquals(PrimitiveKind.LONG, element.kind, label)
                        "boolean" -> assertEquals(PrimitiveKind.BOOLEAN, element.kind, label)
                        "array" -> {
                            assertEquals(StructureKind.LIST, element.kind, label)
                            val reference = shape.getValue("items").jsonObject.getValue("\$ref").jsonPrimitive.content
                            assertEquals(reference.removePrefix("#/\$defs/"),
                                definitionBySerialName[element.getElementDescriptor(0).serialName], "$label item definition")
                            assertEquals(MaxPageItems.toLong(), shape.getValue("maxItems").jsonPrimitive.long, "$label maxItems")
                        }
                        else -> error("$label has an unmapped schema type $type")
                    }
                }
            }
        }
    }

    @Test
    fun domain_bounds_agree_with_schema_patterns_and_ranges() {
        val unit = definition("UnitSummary").getValue("properties").jsonObject
        val uuidPattern = Regex(unit.getValue("unit_id").jsonObject.getValue("pattern").jsonPrimitive.content)
        val cursorPattern = Regex(
            definition("WorkPage").getValue("properties").jsonObject.getValue("next_cursor").jsonObject
                .getValue("anyOf").jsonArray.map { it.jsonObject }.single { it["pattern"] != null }
                .getValue("pattern").jsonPrimitive.content,
        )
        val uuid = "abcdef01-0000-4000-8000-00000000000a"
        listOf(uuid, uuid.uppercase(), uuid.replace('1', 'g'), uuid.drop(1), "$uuid\n", "").forEach { sample ->
            val schemaAccepts = uuidPattern.matches(sample)
            assertEquals(schemaAccepts, PublicWorkId.parse(sample) is CatalogCheck.Valid, "work id '$sample'")
            assertEquals(schemaAccepts, PublicUnitId.parse(sample) is CatalogCheck.Valid, "unit id '$sample'")
            assertEquals(schemaAccepts, PublicationId.parse(sample) is CatalogCheck.Valid, "publication id '$sample'")
        }
        val cursor = "lc1_" + "0123456789abcdef".repeat(4)
        listOf(cursor, cursor.uppercase(), cursor.drop(1), cursor + "0", "lc1_").forEach { sample ->
            assertEquals(cursorPattern.matches(sample), CatalogCursor.parse(sample) is CatalogCheck.Valid, "cursor '$sample'")
        }
        assertEquals(Int.MAX_VALUE.toLong(), unit.getValue("position").jsonObject.getValue("maximum").jsonPrimitive.long)
        listOf("published_at_ms", "duration_ms").forEach {
            assertEquals(MaxSafeMillis, unit.getValue(it).jsonObject.getValue("maximum").jsonPrimitive.long, it)
        }
        // Deliberate, documented difference: the schema still says minimum 0, the server emits
        // 1..=3_600_000 and this client refuses 0 rather than reinterpreting it (ADR 0035 §3).
        assertEquals(0L, unit.getValue("duration_ms").jsonObject.getValue("minimum").jsonPrimitive.long)
    }

    @Test
    fun every_shared_fixture_is_classified_as_default_round_trip_or_unsupported_presentation() {
        assertEquals(fixtureShapes.keys, CanonicalDiscoveryCorpus.fixtureJsonByFile.keys, "unclassified or missing fixture")
        CanonicalDiscoveryCorpus.fixtureJsonByFile.forEach { (file, json) ->
            when (fixtureShapes.getValue(file)) {
                Shape.WorkPage -> assertRoundTrip(WorkPageWire.serializer(), json, file)
                Shape.UnitPage -> assertRoundTrip(UnitPageWire.serializer(), json, file)
                Shape.ListenerError -> assertRoundTrip(ListenerErrorWire.serializer(), json, file)
                Shape.Manifest -> assertRoundTrip(ManifestWire.serializer(), json, file)
                Shape.PresentedWorkPage -> assertPresentationRefused(WorkPageWire.serializer(), json, file, "unit_count")
                Shape.PresentedUnitPage -> assertPresentationRefused(UnitPageWire.serializer(), json, file, "ordinal")
            }
        }
    }

    private fun <T> assertPresentationRefused(serializer: KSerializer<T>, json: String, file: String, field: String) {
        val items = Json.parseToJsonElement(json).jsonObject.getValue("items").jsonArray
        assertTrue(items.isNotEmpty(), "$file must exercise its opt-in field")
        assertTrue(items.all { field in it.jsonObject }, "$file must carry $field")
        val refusal = assertFailsWith<SerializationException>("$file requires explicit presentation support") {
            catalogJson.decodeFromString(serializer, json)
        }
        assertTrue(refusal.message?.contains(field) == true, "$file must be refused for its unsupported $field")
    }

    private fun <T> assertRoundTrip(serializer: KSerializer<T>, json: String, file: String) {
        val decoded = catalogJson.decodeFromString(serializer, json)
        val encoded: JsonElement = Json.parseToJsonElement(catalogJson.encodeToString(serializer, decoded))
        assertEquals(Json.parseToJsonElement(json), encoded, "$file must round-trip without loss")
    }

    @Test
    fun shared_fixtures_map_to_validated_domain_values_by_server_identity() {
        val fixtures = CanonicalDiscoveryCorpus.fixtureJsonByFile
        val works = catalogJson.decodeFromString(WorkPageWire.serializer(), fixtures.getValue("works.json")).toDomain()
        assertIs<CatalogCheck.Valid<PublicWorkPage>>(works)
        val work: PublicWork = works.value.items.single()
        assertEquals("10000000-0000-4000-8000-000000000001", work.id.value)
        assertEquals(PublicWorkKind.Story, work.kind)
        assertEquals("Ánh đèn cuối sân khấu", work.title)
        assertEquals(null, works.value.next)

        fun units(file: String) = catalogJson.decodeFromString(UnitPageWire.serializer(), fixtures.getValue(file)).toDomain(work)
        val first = assertIs<CatalogCheck.Valid<PublicUnitPage>>(units("units.json")).value.items.single()
        val republished = assertIs<CatalogCheck.Valid<PublicUnitPage>>(units("units_republished.json")).value.items.single()
        assertEquals(PublicUnitKind.Chapter, first.kind)
        // Same unit identity and position, new immutable release: a version change, not a new row.
        assertEquals(first.id, republished.id)
        assertEquals(first.position, republished.position)
        assertEquals("80000000-0000-4000-8000-000000000008", first.publicationId.value)
        assertEquals("80000000-0000-4000-8000-000000000009", republished.publicationId.value)

        val continuation = assertIs<CatalogCheck.Valid<PublicUnitPage>>(units("units_empty_continuation.json")).value
        assertTrue(continuation.items.isEmpty())
        assertEquals("lc1_" + "0".repeat(63) + "1", continuation.next?.value, "an empty page with a cursor is not the end")
        val end = assertIs<CatalogCheck.Valid<PublicUnitPage>>(units("units_end_empty.json")).value
        assertTrue(end.items.isEmpty() && end.next == null)

        // The same unit page read for a podcast is refused: chapters never belong to it.
        val podcast = PublicWork.of(work.id, PublicWorkKind.Podcast, work.title, "", "vi-VN", 0)
        assertIs<CatalogCheck.Valid<PublicWork>>(podcast)
        assertIs<CatalogCheck.Invalid>(catalogJson.decodeFromString(UnitPageWire.serializer(), fixtures.getValue("units.json")).toDomain(podcast.value))
    }

    @Test
    fun strict_decoding_rejects_unknown_missing_and_mistyped_fields() {
        val work = """{"work_id":"10000000-0000-4000-8000-000000000001","kind":"story","title":"T","description":"","language":"vi-VN","first_published_at_ms":5000}"""
        catalogJson.decodeFromString(WorkSummaryWire.serializer(), work)
        val hostile = listOf(
            work.replace("}", ""","genre":"drama"}"""),
            work.replace(""","language":"vi-VN"""", ""),
            work.replace("\"story\"", "\"audio_drama\""),
            work.replace("\"story\"", "\"Story\""),
            work.replace("5000", "\"5000\""),
            work.replace("5000", "5000.5"),
            work.replace("5000", "5e3"),
            work.replace("5000", "null"),
            work.replace("\"T\"", "null"),
            work.replace("\"T\"", "7"),
        )
        hostile.forEach { json ->
            assertFailsWith<SerializationException>(json) { catalogJson.decodeFromString(WorkSummaryWire.serializer(), json) }
        }
        // next_cursor is required even though it is nullable.
        assertFailsWith<SerializationException>("missing next_cursor") { catalogJson.decodeFromString(WorkPageWire.serializer(), """{"items":[]}""") }
        assertEquals(null, catalogJson.decodeFromString(WorkPageWire.serializer(), """{"items":[],"next_cursor":null}""").nextCursor)
        assertFailsWith<SerializationException>("extra error field") {
            catalogJson.decodeFromString(ListenerErrorWire.serializer(), """{"code":"not_found","retryable":false,"current_revision":null}""")
        }
        assertFailsWith<SerializationException>("quoted boolean") {
            catalogJson.decodeFromString(ListenerErrorWire.serializer(), """{"code":"not_found","retryable":"false"}""")
        }
    }

    private companion object {
        val JsonPrimitiveFalse: JsonElement = JsonPrimitive(false)
    }
}
