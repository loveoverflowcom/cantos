package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.ports.PlaybackAccessFailure
import com.cantos.theatre.ports.PlaybackAccessResult
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.KSerializer
import kotlinx.serialization.descriptors.elementNames
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue

class PlaybackAccessHttpTest {
    private val publication = "80000000-0000-4000-8000-000000000008"
    private val manifest = CanonicalDiscoveryCorpus.fixtureJsonByFile.getValue("manifest.json")
    private val audio = AudioIdentity.of("10000000-0000-4000-8000-000000000001", "20000000-0000-4000-8000-000000000002",
        publication, "60000000-0000-4000-8000-000000000006", "b".repeat(64), 1000)!!

    @Test
    fun native_manifest_and_grant_names_and_required_fields_are_schema_bound() {
        val definitions = Json.parseToJsonElement(CanonicalDiscoveryCorpus.schemaJson).jsonObject.getValue("\$defs").jsonObject
        val serializers: Map<String, KSerializer<*>> = mapOf("PublicationManifest" to ManifestWire.serializer(), "PlaybackAuthorization" to PlaybackGrantWire.serializer())
        serializers.forEach { (name, serializer) ->
            val definition = definitions.getValue(name).jsonObject
            val descriptor = serializer.descriptor
            assertEquals(definition.getValue("properties").jsonObject.keys, descriptor.elementNames.toSet(), name)
            assertEquals(definition.getValue("required").jsonArray.map { it.jsonPrimitive.content }.toSet(),
                (0 until descriptor.elementsCount).filterNot(descriptor::isElementOptional).map(descriptor::getElementName).toSet())
        }
        assertEquals(Json.parseToJsonElement(manifest), catalogJson.encodeToJsonElement(ManifestWire.serializer(), catalogJson.decodeFromString<ManifestWire>(manifest)))
    }

    private fun grant(hash: String = audio.sha256, expires: Long = 125_000) = """{
        "publication_id":"$publication","rendition_id":"${audio.renditionId}","sha256":"$hash",
        "url":"https://media.example/api/v1/theatre/media/secret-grant","expires_at_ms":$expires,
        "content_type":"audio/ogg","byte_range_supported":true} """

    @Test
    fun grant_cannot_substitute_another_hash_or_expired_location() = runTest {
        for (body in listOf(grant("a".repeat(64)), grant(expires = 5000))) {
            val http = QueueHttp(listOf(manifest, body))
            val api = PlaybackAccessHttp(ApiOrigin.parse("https://api.example"), http) { 5000 }
            assertEquals(PlaybackAccessResult.Refused(PlaybackAccessFailure.Protocol), api.authorize(audio))
            assertEquals(listOf("GET", "POST"), http.requests.map { it.method })
            assertTrue(http.requests.all { it.bearer == null && !it.toString().contains(publication) })
        }
    }

    @Test
    fun exact_manifest_and_grant_preserve_identity_and_never_route_audio_through_json() = runTest {
        val http = QueueHttp(listOf(manifest, grant()))
        val api = PlaybackAccessHttp(ApiOrigin.parse("https://api.example"), http) { 5000 }
        val result = assertIs<PlaybackAccessResult.Accepted<*>>(api.authorize(audio))
        assertFalse(result.value.toString().contains("secret-grant"))
        assertEquals(2, http.requests.size)
        assertTrue(http.requests.all { "/media/" !in it.url })
    }

    @Test
    fun hostile_manifest_or_wrong_work_cannot_construct_a_playable_identity() = runTest {
        for (body in listOf(manifest.replace("\"duration_ms\": 1000", "\"duration_ms\": \"1000\""),
            manifest.replace("\"schema_version\": \"1\"", "\"schema_version\": \"1\", \"extra\": true"))) {
            val api = PlaybackAccessHttp(ApiOrigin.parse("https://api.example"), QueueHttp(listOf(body))) { 5000 }
            assertEquals(PlaybackAccessResult.Refused(PlaybackAccessFailure.Protocol), api.manifest(publication))
        }
        listOf("http://media.example/audio", "https://user:pass@media.example/audio", "https://media.example/audio#fragment", "https://media.example/\nsecret")
            .forEach { assertFalse(safeMediaUrl(it)) }
    }

    private class QueueHttp(private val replies: List<String>) : NativeHttpPort {
        val requests = mutableListOf<NativeHttpRequest>()
        override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
            requests += request
            completion(NativeHttpResponse(200, "application/json", replies[requests.lastIndex], null))
            return object : NativeRequestHandle { override fun cancel() = Unit }
        }
    }
}
