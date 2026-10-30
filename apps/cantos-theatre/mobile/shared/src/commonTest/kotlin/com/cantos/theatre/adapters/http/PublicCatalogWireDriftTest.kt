package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.catalog.CatalogCheck
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.elementNames
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNotNull

/** Consumer-owned DTOs drift against the actual Server schema, with no old-reader compatibility lane. */
class PublicCatalogWireDriftTest {
    private val fixtures = CanonicalDiscoveryCorpus.fixtureJsonByFile
    private val definitions = Json.parseToJsonElement(CanonicalDiscoveryCorpus.metadataSchemaJson).jsonObject.getValue("\$defs").jsonObject
    private val serializers = mapOf(
        "cover" to CoverImageWire.serializer(),
        "work" to MetadataWorkWire.serializer(),
        "works" to MetadataWorkPageWire.serializer(),
        "unit" to MetadataUnitWire.serializer(),
        "units" to MetadataUnitPageWire.serializer(),
        "playback" to UnitPlaybackWire.serializer(),
    )

    @Test
    fun generation_readers_match_the_exact_required_properties_and_closed_enums() {
        serializers.forEach { (name, serializer) ->
            val definition = definitions.getValue(name).jsonObject
            assertEquals(JsonPrimitive(false), definition["additionalProperties"], name)
            val descriptor = serializer.descriptor
            assertEquals(definition.getValue("properties").jsonObject.keys, descriptor.elementNames.toSet(), name)
            assertEquals(definition.getValue("required").jsonArray.map { it.jsonPrimitive.content }.toSet(),
                (0 until descriptor.elementsCount).filterNot(descriptor::isElementOptional).map(descriptor::getElementName).toSet(), name)
        }
        assertEquals(listOf("novel", "story", "podcast"), WorkKindWire.serializer().descriptor.elementNames.toList())
        assertEquals(listOf("chapter", "episode"), UnitKindWire.serializer().descriptor.elementNames.toList())
    }

    private fun <T> roundTrip(serializer: KSerializer<T>, file: String) {
        val raw = fixtures.getValue(file)
        val parsed = catalogJson.decodeFromString(serializer, raw)
        assertEquals(Json.parseToJsonElement(raw), Json.parseToJsonElement(catalogJson.encodeToString(serializer, parsed)), file)
    }

    @Test
    fun all_current_generation_fixtures_and_playback_contracts_round_trip() {
        roundTrip(MetadataWorkPageWire.serializer(), "metadata/covered_works.json")
        roundTrip(MetadataWorkWire.serializer(), "metadata/covered_work.json")
        for (name in listOf("metadata/presented_units.json", "metadata/units_republished.json")) roundTrip(MetadataUnitPageWire.serializer(), name)
        for (name in listOf("metadata/unreleased.json", "metadata/released.json")) roundTrip(MetadataUnitWire.serializer(), name)
        roundTrip(MetadataWorkPageWire.serializer(), "empty.json")
        roundTrip(ManifestWire.serializer(), "manifest.json")
        for (name in listOf("not_found", "invalid_cursor", "invalid_request", "unavailable", "rendition_unavailable")) roundTrip(ListenerErrorWire.serializer(), "$name.json")
    }

    @Test
    fun replaced_release_only_shapes_are_rejected_by_current_readers() {
        for (file in listOf("works.json", "presented_works.json", "cover/covered_works.json")) {
            assertFailsWith<SerializationException>(file) { catalogJson.decodeFromString(MetadataWorkPageWire.serializer(), fixtures.getValue(file)) }
        }
        for (file in listOf("units.json", "presented_units.json")) {
            assertFailsWith<SerializationException>(file) { catalogJson.decodeFromString(MetadataUnitPageWire.serializer(), fixtures.getValue(file)) }
        }
    }

    @Test
    fun null_audio_and_the_exact_release_transition_are_distinct_domain_values() {
        val absent = catalogJson.decodeFromString(MetadataUnitWire.serializer(), fixtures.getValue("metadata/unreleased.json"))
        assertEquals(null, absent.playback)
        assertIs<CatalogCheck.Valid<*>>(absent.toDomain())
        val audio = catalogJson.decodeFromString(MetadataUnitWire.serializer(), fixtures.getValue("metadata/released.json"))
        val accepted = assertIs<CatalogCheck.Valid<com.cantos.theatre.domain.catalog.PublicUnit>>(audio.toDomain()).value
        val playback = assertNotNull(audio.playback)
        assertEquals(playback.publicationId, accepted.publicationId?.value)
        assertEquals(playback.durationMs, accepted.durationMs)
    }

    @Test
    fun unknown_fields_missing_nullable_fields_and_wrong_number_types_are_protocol_failures() {
        val raw = Json.parseToJsonElement(fixtures.getValue("metadata/unreleased.json")).jsonObject
        for (field in listOf("playback", "ordinal", "unit_id")) {
            assertFailsWith<SerializationException>(field) { catalogJson.decodeFromString(MetadataUnitWire.serializer(), JsonObject(raw - field).toString()) }
        }
        for (changes in listOf(mapOf("private_owner" to JsonPrimitive("secret")), mapOf("ordinal" to JsonPrimitive("1")), mapOf("position" to JsonPrimitive(0.5)), mapOf("kind" to JsonPrimitive("unknown")))) {
            assertFailsWith<SerializationException> { catalogJson.decodeFromString(MetadataUnitWire.serializer(), JsonObject(raw + changes).toString()) }
        }
        val work = Json.parseToJsonElement(fixtures.getValue("metadata/covered_work.json")).jsonObject
        assertFailsWith<SerializationException> { catalogJson.decodeFromString(MetadataWorkWire.serializer(), JsonObject(work - "cover").toString()) }
        assertFailsWith<SerializationException> { catalogJson.decodeFromString(MetadataWorkPageWire.serializer(), """{"items":[]}""") }
        assertNotNull(catalogJson.decodeFromString(MetadataWorkWire.serializer(), work.toString()).cover)
    }
}
