package com.cantos.theatre.domain.playback

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class PlaybackVectorTest {
    @Test
    fun play_pause_matches_the_canonical_vector() = replay("play-pause.json")

    @Test
    fun buffering_matches_the_canonical_vector() = replay("buffering.json")

    @Test
    fun interruption_matches_the_canonical_vector() = replay("interruption.json")

    @Test
    fun stale_response_matches_the_canonical_vector() = replay("stale-response.json")

    @Test
    fun restore_session_matches_the_canonical_vector() = replay("restore-session.json")

    @Test
    fun preparing_races_matches_the_canonical_vector() = replay("preparing-races.json")

    @Test
    fun invalid_controls_matches_the_canonical_vector() = replay("invalid-controls.json")

    @Test
    fun generated_inventory_matches_all_seven_reviewed_hashes_and_fifty_one_steps() {
        // The Gradle source generator separately checks SHA-256 against the exact source bytes.
        val inventory = decodePlaybackIndex(CanonicalPlaybackVectors.indexJson)
        assertEquals(reviewedPlaybackHashes, inventory)
        assertEquals(reviewedPlaybackHashes.keys, CanonicalPlaybackVectors.jsonByFile.keys)
        assertEquals(51, inventory.keys.sumOf { decodePlaybackVector(fixture(it)).steps.size })
        inventory.keys.forEach { file -> assertEquals(file, "${decodePlaybackVector(fixture(file)).id}.json") }
    }

    @Test
    fun unknown_versions_ids_and_empty_steps_cannot_be_quarantined_as_success() {
        val original = fixtureValue("play-pause.json")
        listOf(
            Triple("vector_format", JsonPrimitive(2), PlaybackVectorError.Format),
            Triple("contract", JsonPrimitive("theatre-mock-playback@2"), PlaybackVectorError.Contract),
            Triple("id", JsonPrimitive(""), PlaybackVectorError.VectorId),
            Triple("id", JsonPrimitive("unlisted"), PlaybackVectorError.VectorId),
            Triple("steps", JsonArray(emptyList()), PlaybackVectorError.EmptySteps),
        ).forEach { (path, replacement, error) ->
            assertDecodeError(error, path) { decodePlaybackVector(replaceAt(original, path, replacement)) }
        }
    }

    @Test
    fun unknown_fields_are_rejected_at_every_nested_shape_and_every_event_or_effect_variant() {
        val original = fixtureValue("play-pause.json")
        listOf(
            "", "initial_state", "initial_state/session", "initial_state/session/source",
            "initial_state/session/intent", "steps/0", "steps/0/expected", "steps/0/expected/state",
            "steps/0/expected/effects/0/identity", "steps/0/expected/effects/0/source",
        ).forEach { path ->
            val objectValue = at(original, path) as JsonObject
            val changed = replaceAt(original, path, JsonObject(objectValue + ("future" to JsonNull)))
            assertDecodeError(PlaybackVectorError.ObjectFields, path) { decodePlaybackVector(changed) }
        }
        CanonicalPlaybackVectors.jsonByFile.values.forEach { json ->
            val vector = Json.parseToJsonElement(json) as JsonObject
            val steps = vector.getValue("steps") as JsonArray
            steps.forEach { raw ->
                val step = raw as JsonObject
                val event = step.getValue("event") as JsonObject
                assertDecodeError(PlaybackVectorError.ObjectFields, "event ${event.getValue("type")}") {
                    decodePlaybackEvent(JsonObject(event + ("future" to JsonNull)))
                }
                val expected = step.getValue("expected") as JsonObject
                val effects = expected.getValue("effects") as JsonArray
                effects.forEach { effect ->
                    val objectValue = effect as JsonObject
                    assertDecodeError(PlaybackVectorError.ObjectFields, "effect ${objectValue.getValue("type")}") {
                        decodePlaybackEffect(JsonObject(objectValue + ("future" to JsonNull)))
                    }
                }
            }
        }
        val restore = fixtureValue("restore-session.json")
        val path = "steps/0/event/snapshot"
        val value = at(restore, path) as JsonObject
        assertDecodeError(PlaybackVectorError.ObjectFields, path) {
            decodePlaybackVector(replaceAt(restore, path, JsonObject(value + ("future" to JsonNull))))
        }
        assertDecodeError(PlaybackVectorError.ObjectFields, "set_speed") {
            decodePlaybackEffect(Json.parseToJsonElement("""{"type":"set_speed","future":null}"""))
        }
    }

    @Test
    fun missing_fields_and_wrong_json_container_types_fail_the_harness() {
        val original = fixtureValue("play-pause.json")
        val state = at(original, "initial_state") as JsonObject
        assertDecodeError(PlaybackVectorError.ObjectFields, "missing session") {
            decodePlaybackVector(replaceAt(original, "initial_state", JsonObject(state - "session")))
        }
        listOf(
            Triple("initial_state", JsonArray(emptyList()), PlaybackVectorError.ObjectFields),
            Triple("steps", JsonObject(emptyMap()), PlaybackVectorError.ArrayType),
            Triple("contract", JsonPrimitive(1), PlaybackVectorError.StringType),
            Triple("steps/0/expected/effects", JsonNull, PlaybackVectorError.ArrayType),
        ).forEach { (path, replacement, error) ->
            assertDecodeError(error, path) { decodePlaybackVector(replaceAt(original, path, replacement)) }
        }
    }

    @Test
    fun unknown_tags_fail_at_every_closed_semantic_level() {
        val original = fixtureValue("play-pause.json")
        listOf(
            "steps/0/event/type" to PlaybackVectorError.EventType,
            "steps/0/expected/effects/0/type" to PlaybackVectorError.EffectType,
            "initial_state/status" to PlaybackVectorError.Status,
            "initial_state/pause_cause" to PlaybackVectorError.PauseCause,
            "initial_state/session/intent/type" to PlaybackVectorError.Intent,
            "initial_state/session/intent/cause" to PlaybackVectorError.PauseCause,
            "initial_state/session/engine" to PlaybackVectorError.Engine,
            "initial_state/session/interruption" to PlaybackVectorError.Interruption,
            "initial_state/session/source/work_id" to PlaybackVectorError.WorkKey,
        ).forEach { (path, error) ->
            assertDecodeError(error, path) { decodePlaybackVector(replaceAt(original, path, JsonPrimitive("future"))) }
        }
        assertDecodeError(PlaybackVectorError.Rejection, "rejection") {
            decodePlaybackEffect(Json.parseToJsonElement("""{"type":"rejected","reason":"future"}"""))
        }
    }

    @Test
    fun integer_lexemes_bounds_sources_positions_and_speeds_are_strict() {
        val original = fixtureValue("play-pause.json")
        listOf(
            Triple("initial_state/generation", JsonPrimitive(4_294_967_296L), PlaybackVectorError.IntegerBounds),
            Triple("initial_state/generation", JsonPrimitive(-1), PlaybackVectorError.IntegerBounds),
            Triple("initial_state/generation", JsonPrimitive("1"), PlaybackVectorError.IntegerType),
            Triple("initial_state/generation", JsonPrimitive(1.5), PlaybackVectorError.IntegerType),
            Triple("initial_state/generation", JsonPrimitive(1.0), PlaybackVectorError.IntegerType),
            Triple("initial_state/generation", JsonPrimitive(true), PlaybackVectorError.IntegerType),
            Triple("initial_state/session/source/episode_index", JsonPrimitive(65_536), PlaybackVectorError.IntegerBounds),
            Triple("initial_state/session/source/duration_ms", JsonPrimitive(0), PlaybackVectorError.Source),
            Triple("initial_state/session/position_ms", JsonPrimitive(MAX_PLAYBACK_GENERATION), PlaybackVectorError.Position),
            Triple("initial_state/session/speed_permille", JsonPrimitive(999), PlaybackVectorError.Speed),
        ).forEach { (path, replacement, error) ->
            assertDecodeError(error, path) { decodePlaybackVector(replaceAt(original, path, replacement)) }
        }
        listOf(Int.MIN_VALUE.toLong() - 1, Int.MAX_VALUE.toLong() + 1).forEach { delta ->
            assertDecodeError(PlaybackVectorError.IntegerBounds, "skip $delta") {
                decodePlaybackEvent(Json.parseToJsonElement("""{"type":"skip_requested","delta_ms":$delta}"""))
            }
        }
        assertDecodeError(PlaybackVectorError.BooleanType, "resume hint") {
            decodePlaybackEvent(Json.parseToJsonElement("""{"type":"interruption_ended","resume_allowed":1}"""))
        }
    }

    @Test
    fun raw_negative_zero_is_rejected_like_the_web_harness_in_signed_and_unsigned_fields() {
        assertDecodeError(PlaybackVectorError.IntegerType, "signed negative zero") {
            decodePlaybackEvent(Json.parseToJsonElement("""{"type":"skip_requested","delta_ms":-0}"""))
        }
        val json = fixture("play-pause.json").replaceFirst("\"generation\": 1", "\"generation\": -0")
        assertTrue(json.contains("\"generation\": -0"), "raw fixture mutation selected the first generation")
        assertDecodeError(PlaybackVectorError.IntegerType, "unsigned negative zero") {
            decodePlaybackVector(json)
        }
    }

    @Test
    fun a_snapshot_pause_cause_must_agree_with_the_status_shape() {
        val original = fixtureValue("play-pause.json")
        assertDecodeError(PlaybackVectorError.StatusCause, "paused without cause") {
            decodePlaybackVector(replaceAt(original, "initial_state/pause_cause", JsonNull))
        }
        assertDecodeError(PlaybackVectorError.StatusCause, "loading with cause") {
            decodePlaybackVector(replaceAt(original, "initial_state/status", JsonPrimitive("loading")))
        }
        val inconsistent = decodePlaybackVector(replaceAt(original, "initial_state/pause_cause", JsonPrimitive("cold_restore")))
        assertTrue(PlaybackVectorSnapshot.of(inconsistent.initial.state) != inconsistent.initial)
    }

    @Test
    fun inventory_rejects_unlisted_duplicate_missing_malformed_and_unknown_fields() {
        val original = Json.parseToJsonElement(CanonicalPlaybackVectors.indexJson)
        listOf(
            Triple("vector_format", JsonPrimitive(2), PlaybackVectorError.Format),
            Triple("contract", JsonPrimitive("theatre-mock-playback@2"), PlaybackVectorError.Contract),
            Triple("vectors/0/file", JsonPrimitive("unlisted.json"), PlaybackVectorError.VectorFile),
            Triple("vectors/0/sha256", JsonPrimitive("not-a-digest"), PlaybackVectorError.Digest),
        ).forEach { (path, replacement, error) ->
            assertDecodeError(error, path) { decodePlaybackIndex(replaceAt(original, path, replacement)) }
        }
        assertDecodeError(PlaybackVectorError.DuplicateFile, "duplicate file") {
            decodePlaybackIndex(replaceAt(original, "vectors/1", at(original, "vectors/0")))
        }
        val entries = at(original, "vectors") as JsonArray
        assertDecodeError(PlaybackVectorError.Inventory, "missing file") {
            decodePlaybackIndex(replaceAt(original, "vectors", JsonArray(entries.dropLast(1))))
        }
        listOf("", "vectors/0").forEach { path ->
            val value = at(original, path) as JsonObject
            assertDecodeError(PlaybackVectorError.ObjectFields, path) {
                decodePlaybackIndex(replaceAt(original, path, JsonObject(value + ("future" to JsonNull))))
            }
        }
    }

    private fun replay(file: String) {
        val vector = decodePlaybackVector(fixture(file))
        assertEquals(file, "${vector.id}.json")
        assertEquals(vector.initial, PlaybackVectorSnapshot.of(vector.initial.state), "$file initial projection")
        var state = vector.initial.state
        vector.steps.forEachIndexed { index, step ->
            val transition = reducePlayback(state, step.event)
            assertEquals(step.expected, PlaybackVectorSnapshot.of(transition.nextState), "$file#$index complete state")
            assertEquals(step.effects, transition.effects, "$file#$index complete ordered effects")
            state = transition.nextState
        }
    }

    private fun fixture(file: String): String = CanonicalPlaybackVectors.jsonByFile.getValue(file)

    private fun fixtureValue(file: String): JsonElement = Json.parseToJsonElement(fixture(file))

    private fun assertDecodeError(error: PlaybackVectorError, context: String, action: () -> Unit) {
        assertEquals(error, assertFailsWith<PlaybackVectorDecodeException>(context, action).reason, context)
    }

    private fun at(value: JsonElement, path: String): JsonElement =
        if (path.isEmpty()) value else path.split('/').fold(value) { current, key ->
            when (current) {
                is JsonObject -> current.getValue(key)
                is JsonArray -> current[key.toInt()]
                else -> error("Test mutation path does not select a container")
            }
        }

    private fun replaceAt(value: JsonElement, path: String, replacement: JsonElement): JsonElement =
        replaceAt(value, if (path.isEmpty()) emptyList() else path.split('/'), replacement)

    private fun replaceAt(value: JsonElement, keys: List<String>, replacement: JsonElement): JsonElement {
        if (keys.isEmpty()) return replacement
        val key = keys.first()
        val remaining = keys.drop(1)
        return when (value) {
            is JsonObject -> JsonObject(value + (key to replaceAt(value.getValue(key), remaining, replacement)))
            is JsonArray -> JsonArray(value.mapIndexed { index, entry ->
                if (index == key.toInt()) replaceAt(entry, remaining, replacement) else entry
            })
            else -> error("Test mutation path does not select a container")
        }
    }
}
