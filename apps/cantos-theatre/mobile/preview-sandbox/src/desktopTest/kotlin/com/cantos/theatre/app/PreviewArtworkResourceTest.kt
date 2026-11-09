package com.cantos.theatre.app

import com.cantos.theatre.preview.resources.Res
import java.security.MessageDigest
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class PreviewArtworkResourceTest {
    @Test
    fun exact_bundled_paintings_decode_and_missing_or_invalid_art_falls_back() = runTest {
        assertNotNull(loadArtwork("files/artwork/son_ha_moon.webp"))
        assertNotNull(loadArtwork("files/artwork/den_khuya.webp"))
        assertNull(loadArtwork("files/artwork/absent.webp"))
        assertNull(loadArtwork("files/provenance.json"))
    }

    /** The manifest describes the packaged bytes themselves, never a prior derivative's size or hash. */
    @Test
    fun painting_provenance_matches_the_packaged_vp8_bytes_and_keeps_its_lineage() = runTest {
        val files = Json.parseToJsonElement(Res.readBytes("files/provenance.json").decodeToString())
            .jsonObject.getValue("files").jsonObject
        val paintings = files.keys.filter { it.startsWith("files/artwork/") }
        assertEquals(setOf("files/artwork/den_khuya.webp", "files/artwork/son_ha_moon.webp"), paintings.toSet())
        paintings.forEach { path ->
            val record = files.getValue(path).jsonObject
            assertEquals(setOf("sha256", "bytes", "width", "height", "format", "encoding", "source",
                "derivedFromSha256", "priorDerivatives"), record.keys, path)
            val bytes = Res.readBytes(path)
            val hash = sha256(bytes)
            assertEquals(record.getValue("sha256").jsonPrimitive.content, hash, path)
            assertEquals(record.getValue("bytes").jsonPrimitive.int, bytes.size, path)
            assertEquals("RIFF", bytes.decodeToString(0, 4), path)
            assertEquals(bytes.size - 8, le32(bytes, 4), path)
            assertEquals("WEBPVP8 ", bytes.decodeToString(8, 16), path)
            val bitmap = assertNotNull(loadArtwork(path), path)
            assertEquals(record.getValue("width").jsonPrimitive.int, bitmap.width, path)
            assertEquals(record.getValue("height").jsonPrimitive.int, bitmap.height, path)

            val prior = record.getValue("priorDerivatives").jsonArray.map { it.jsonObject }
            assertTrue(prior.isNotEmpty(), path)
            prior.forEach { derivative ->
                assertEquals(setOf("path", "sha256", "bytes", "width", "height", "encoding"), derivative.keys, path)
                assertNotEquals(hash, derivative.getValue("sha256").jsonPrimitive.content, path)
            }
            assertEquals(path.removeSuffix(".webp") + ".avif", prior.first().getValue("path").jsonPrimitive.content, path)
        }
    }

    private fun le32(bytes: ByteArray, offset: Int): Int =
        (0..3).sumOf { (bytes[offset + it].toInt() and 0xff) shl (8 * it) }

    private fun sha256(bytes: ByteArray): String = MessageDigest.getInstance("SHA-256")
        .digest(bytes).joinToString("") { "%02x".format(it.toInt() and 0xff) }
}
