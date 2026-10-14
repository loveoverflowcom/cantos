package com.cantos.theatre.artworkfixtures

import kotlin.test.Test
import kotlin.test.assertEquals

/** Test-only cross-language compatibility. This creates no native artwork API or codec. */
class ArtworkFixtureCompatibilityTest {
    @Test
    fun independentArtworkGoldensExerciseActualEnumsOneofAndUnsignedBits() {
        assertEquals(38, CanonicalArtworkVectors.goldens.size)
        for (case in CanonicalArtworkVectors.goldens) {
            case.verifySerializer()
            assertEquals(
                if (case.applicationValid) null else "Shape",
                ArtworkFixtureEnvelope(hex(case.hex)).scan(case.message), case.name,
            )
        }
    }

    @Test
    fun independentHostileCorpusAgreesOnClosedEnumsDuplicateOneofAndBounds() {
        assertEquals(42, CanonicalArtworkVectors.hostile.size)
        for (case in CanonicalArtworkVectors.hostile) {
            assertEquals(case.error, ArtworkFixtureEnvelope(hex(case.hex)).scan(case.message), case.name)
        }
    }

    @Test
    fun deepestLegalSchemaAndGlobalFieldBudgetHaveExactBoundaries() {
        val deepest = CanonicalArtworkVectors.goldens.single { it.name == "intent-admitted-with-asset" }
        assertEquals(null, ArtworkFixtureEnvelope(hex(deepest.hex)).scan(deepest.message))
        assertEquals("TooDeep", ArtworkFixtureEnvelope(hex(deepest.hex), maxDepth = 3).scan(deepest.message))
        assertEquals("TooLarge", ArtworkFixtureEnvelope(hex(deepest.hex), maxFields = 1).scan(deepest.message))
        // Rights normally bind first; relax only this test's independent item budget
        // to exercise the aggregate field budget at exactly 256 and 257 fields.
        val exact = byteArrayOf(10, 0) + byteArrayOf(18, 0).repeatBytes(255)
        assertEquals(null, ArtworkFixtureEnvelope(exact, maxRights = 256).scan("ImageDeclarations"))
        assertEquals("TooLarge", ArtworkFixtureEnvelope(exact + byteArrayOf(18, 0), maxRights = 256).scan("ImageDeclarations"))
        // Finish wire admission before checking required nested DTO presence.
        assertEquals("Shape", ArtworkFixtureEnvelope(hex("0a0010011a00")).scan("SetWorkArtworkRequest"))
        assertEquals("Malformed", ArtworkFixtureEnvelope(hex("0a0010011a0000")).scan("SetWorkArtworkRequest"))
        assertEquals("Shape", ArtworkFixtureEnvelope(hex("0a0012001a00200128013200")).scan("CreateImageIntentRequest"))
        assertEquals("Malformed", ArtworkFixtureEnvelope(hex("0a0012001a0020012801320000")).scan("CreateImageIntentRequest"))
    }

    private fun hex(value: String) = value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    private fun ByteArray.repeatBytes(count: Int) = ByteArray(size * count) { this[it % size] }
}

/** Independently reviewed fixture admission, not a serializer or production/native boundary. */
private class ArtworkFixtureEnvelope(
    private val bytes: ByteArray,
    private val maxDepth: Int = 4,
    private val maxFields: Int = 256,
    private val maxRights: Int = 64,
) {
    private data class Field(val kind: String, val repeated: Boolean = false)
    private class Rejected(val reason: String) : Exception()
    private var cursor = 0
    private var fields = 0
    private var missingShape = false

    fun scan(message: String): String? {
        if (bytes.size > 32768) return "TooLarge"
        return try {
            readMessage(message, bytes.size, 1)
            if (missingShape) "Shape" else null
        } catch (failure: Rejected) {
            failure.reason
        }
    }

    private fun reject(reason: String): Nothing = throw Rejected(reason)

    private fun shape(message: String): List<Field> = when (message) {
        "ImageDeclarations" -> listOf(Field("string"), Field("string", repeated = true))
        "CreateImageIntentRequest" -> listOf(Field("string"), Field("string"), Field("string"), Field("u64"), Field("mime"), Field("ImageDeclarations"))
        "ImageIntentDocument" -> listOf(Field("string"), Field("string"), Field("string"), Field("string"), Field("string"), Field("CreateImageIntentRequest"), Field("u64"), Field("u64"), Field("string"), Field("string"))
        "ImageVariantMetadata" -> listOf(Field("string"), Field("u64"), Field("mime"), Field("u32"), Field("u32"), Field("string"))
        "ImageAssetDocument" -> listOf(Field("string"), Field("string"), Field("string"), Field("string"), Field("string"), Field("ImageVariantMetadata"), Field("ImageDeclarations"), Field("u64"), Field("string"))
        "ImageIntentResponse" -> listOf(Field("ImageIntentDocument"), Field("ImageAssetDocument"), Field("state"))
        "WorkArtworkSelection" -> listOf(Field("string"), Field("string"), Field("string"))
        "ClearArtwork" -> emptyList()
        "SetWorkArtworkRequest" -> listOf(Field("string"), Field("u64"), Field("WorkArtworkSelection"), Field("ClearArtwork"))
        "WorkArtworkResponse" -> listOf(Field("string"), Field("u64"), Field("WorkArtworkSelection"), Field("u64"))
        else -> error("Unreviewed fixture message $message")
    }

    private fun readMessage(message: String, end: Int, depth: Int) {
        if (depth > maxDepth) reject("TooDeep")
        val shape = shape(message)
        var seen = 0
        var rights = 0
        while (cursor < end) {
            if (++fields > maxFields) reject("TooLarge")
            val tag = varint(end)
            val number = tag shr 3
            if (number == 0uL || number > 0x1fffffffuL) reject("Malformed")
            if (number > shape.size.toULong()) reject("Shape")
            val index = number.toInt() - 1
            val field = shape[index]
            val scalar = field.kind in setOf("u64", "u32", "mime", "state")
            if ((tag and 7uL).toInt() != if (scalar) 0 else 2) reject("Shape")
            val bit = 1 shl index
            if (!field.repeated && seen and bit != 0) reject("Shape")
            if (message == "SetWorkArtworkRequest" && index in 2..3 && seen and 12 != 0) reject("Shape")
            seen = seen or bit
            if (field.repeated && ++rights > maxRights) reject("TooLarge")
            if (scalar) {
                val value = varint(end)
                if (field.kind == "u32" && value > 0xffffffffuL) reject("Shape")
                if (field.kind == "mime" && value !in 1uL..2uL) reject("Shape")
                if (field.kind == "state" && value !in 1uL..3uL) reject("Shape")
            } else {
                val length = varint(end)
                if (length > (end - cursor).toULong()) reject("Truncated")
                val fieldEnd = cursor + length.toInt()
                if (field.kind == "string") {
                    utf8(fieldEnd)
                    cursor = fieldEnd
                } else readMessage(field.kind, fieldEnd, depth + 1)
            }
        }
        val required = when (message) {
            "ImageDeclarations" -> 1
            "ImageIntentResponse" -> 5
            "SetWorkArtworkRequest", "WorkArtworkResponse" -> 3
            else -> (1 shl shape.size) - 1
        }
        if (seen and required != required) missingShape = true
        if (message == "SetWorkArtworkRequest" && seen and 12 == 0) missingShape = true
    }

    private fun varint(end: Int): ULong {
        var value = 0uL
        for (index in 0 until 10) {
            if (cursor == end) reject("Truncated")
            val byte = bytes[cursor++].toInt() and 255
            if (index == 9 && byte > 1) reject("Malformed")
            value = value or ((byte and 127).toULong() shl (index * 7))
            if (byte and 128 == 0) {
                if (index > 0 && byte == 0) reject("Malformed")
                return value
            }
        }
        reject("Malformed")
    }

    private fun utf8(end: Int) {
        var offset = cursor
        while (offset < end) {
            val first = bytes[offset++].toInt() and 255
            if (first <= 127) continue
            val count = when (first) {
                in 194..223 -> 1
                in 224..239 -> 2
                in 240..244 -> 3
                else -> reject("InvalidUtf8")
            }
            if (count > end - offset) reject("InvalidUtf8")
            val second = bytes[offset].toInt() and 255
            if ((first == 224 && second < 160) || (first == 237 && second > 159) ||
                (first == 240 && second < 144) || (first == 244 && second > 143)
            ) reject("InvalidUtf8")
            repeat(count) {
                if ((bytes[offset++].toInt() and 255) !in 128..191) reject("InvalidUtf8")
            }
        }
    }
}
