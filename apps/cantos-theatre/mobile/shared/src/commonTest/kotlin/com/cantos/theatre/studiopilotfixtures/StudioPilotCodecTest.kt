@file:OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)

package com.cantos.theatre.studiopilotfixtures

import kotlinx.serialization.protobuf.ProtoBuf
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotEquals

class StudioPilotCodecTest {
    @Test
    fun independentGoldensAgreeForEveryPilotMessageAndPresenceCase() {
        assertEquals(13, CanonicalProtobufVectors.goldens.size)
        assertEquals(StudioPilotKind.entries.toSet(), CanonicalProtobufVectors.goldens.map { it.kind }.toSet())
        for (case in CanonicalProtobufVectors.goldens) {
            val bytes = hex(case.hex)
            if (case.expected == null) {
                assertEquals(ProtobufResult.Failure(ProtobufFailure.Shape), StudioPilotCodec.decode(case.kind, bytes), case.name)
            } else {
                assertEquals(ProtobufResult.Value(case.expected), StudioPilotCodec.decode(case.kind, bytes), case.name)
                assertContentEquals(bytes, success(StudioPilotCodec.encode(case.expected)), case.name)
            }
        }
    }

    @Test
    fun sharedRustHostileCorpusAgreesOnExactFailureCategories() {
        assertEquals(26, CanonicalProtobufVectors.hostile.size)
        for (case in CanonicalProtobufVectors.hostile) {
            val decoded = StudioPilotCodec.decode(case.kind, hex(case.hex))
            if (case.error == null) {
                val expected: StudioPilotValue = when (case.name) {
                    "empty-script" -> ValidateScriptRequest("")
                    "empty-validation" -> ScriptValidationResponse(emptyList())
                    "unsigned-max-present" -> SaveRevisionRequest(ULong.MAX_VALUE, "", "")
                    "reordered-zero-present" -> SaveRevisionRequest(0uL, "", "")
                    "repeated-merged" -> ScriptValidationResponse(listOf(FieldIssue("", ""), FieldIssue("", "")))
                    else -> error("Unreviewed successful hostile case: ${case.name}")
                }
                assertEquals(ProtobufResult.Value(expected), decoded, case.name)
            } else assertEquals(ProtobufResult.Failure(case.error), decoded, case.name)
        }
    }

    @Test
    fun nullableWireDefaultsExposeAbsenceRatherThanInventingApplicationFacts() {
        val nullWire = ProtoBuf.encodeToByteArray(ValidateWire.serializer(), ValidateWire(null))
        assertContentEquals(byteArrayOf(), nullWire)
        assertEquals(ProtobufResult.Failure(ProtobufFailure.Shape), StudioPilotCodec.decode(
            StudioPilotKind.ValidateScriptRequest, nullWire,
        ))
        assertContentEquals(hex("0a00"), ProtoBuf.encodeToByteArray(ValidateWire.serializer(), ValidateWire("")))
        assertEquals(ProtobufResult.Value(ValidateScriptRequest("")), StudioPilotCodec.decode(
            StudioPilotKind.ValidateScriptRequest, hex("0a00"),
        ))
    }

    @Test
    fun fullUnsignedRevisionRangeIsPreservedWithoutSignedArithmetic() {
        for (revision in listOf(0uL, 1uL, Long.MAX_VALUE.toULong(), 1uL shl 63, ULong.MAX_VALUE)) {
            val request = SaveRevisionRequest(revision, "", "")
            assertEquals(ProtobufResult.Value(request), StudioPilotCodec.decode(
                StudioPilotKind.SaveRevisionRequest, success(StudioPilotCodec.encode(request)),
            ))
            val response = RevisionResponse("", revision, "", "", "", "", "")
            assertEquals(ProtobufResult.Value(response), StudioPilotCodec.decode(
                StudioPilotKind.RevisionResponse, success(StudioPilotCodec.encode(response)),
            ))
        }
    }

    @Test
    fun everyRequiredScalarPresenceIsCheckedIncludingNestedIssues() {
        val requests = listOf(
            StudioPilotKind.ValidateScriptRequest to listOf("0a00"),
            StudioPilotKind.SaveRevisionRequest to listOf("0800", "1200", "1a00"),
            StudioPilotKind.RevisionResponse to listOf("0a00", "1000", "1a00", "2200", "2a00", "3200", "3a00"),
        )
        for ((kind, fields) in requests) {
            for (omitted in fields.indices) assertEquals(
                ProtobufResult.Failure(ProtobufFailure.Shape),
                StudioPilotCodec.decode(kind, hex(fields.filterIndexed { index, _ -> index != omitted }.joinToString(""))),
            )
        }
        for (nested in listOf("0a00", "0a020a00", "0a021200")) assertEquals(
            ProtobufResult.Failure(ProtobufFailure.Shape),
            StudioPilotCodec.decode(StudioPilotKind.ScriptValidationResponse, hex(nested)),
        )
    }

    @Test
    fun unicodeAndExactScriptTextAreNeverNormalized() {
        val texts = listOf("", "\u0000", "Ánh đèn “Việt” 😀 e\u0301\\\n{}", "é", "e\u0301", " {\n  \"a\": 1\n}\r\n")
        for (text in texts) {
            val request = ValidateScriptRequest(text)
            assertEquals(ProtobufResult.Value(request), StudioPilotCodec.decode(
                StudioPilotKind.ValidateScriptRequest, success(StudioPilotCodec.encode(request)),
            ))
        }
        assertNotEquals(
            success(StudioPilotCodec.encode(ValidateScriptRequest("é"))).toList(),
            success(StudioPilotCodec.encode(ValidateScriptRequest("e\u0301"))).toList(),
        )
    }

    @Test
    fun invalidUtf16IsRefusedBeforeKotlinCanReplaceIt() {
        for (text in listOf("\ud800", "\udfff", "a\ud800b", "\ud800\ud800")) assertEquals(
            ProtobufResult.Failure(ProtobufFailure.InvalidUtf8), StudioPilotCodec.encode(ValidateScriptRequest(text)),
        )
        assertEquals(ProtobufResult.Failure(ProtobufFailure.InvalidUtf8), StudioPilotCodec.encode(
            ScriptValidationResponse(listOf(FieldIssue("valid", "\ud800"))),
        ))
    }

    @Test
    fun invalidUtf8ClassesAreRefusedBeforeStringAllocation() {
        for (raw in listOf("80", "c080", "c1bf", "e08080", "eda080", "f0808080", "f4908080", "f5808080", "ff", "e282", "c261")) {
            val bytes = hex(raw)
            assertEquals(ProtobufResult.Failure(ProtobufFailure.InvalidUtf8), StudioPilotCodec.decode(
                StudioPilotKind.ValidateScriptRequest, byteArrayOf(10, bytes.size.toByte()) + bytes,
            ), raw)
        }
    }

    @Test
    fun everyScalarTruncationFailsAndTrailingOrDuplicateTagsAreNeverIgnored() {
        val bytes = success(StudioPilotCodec.encode(SaveRevisionRequest(ULong.MAX_VALUE, "operation", "Việt 😀")))
        for (end in 0 until bytes.size) assertIs<ProtobufResult.Failure>(StudioPilotCodec.decode(
            StudioPilotKind.SaveRevisionRequest, bytes.copyOf(end),
        ), "prefix $end")
        assertEquals(ProtobufResult.Failure(ProtobufFailure.Shape), StudioPilotCodec.decode(
            StudioPilotKind.SaveRevisionRequest, bytes + bytes,
        ))
        assertEquals(ProtobufResult.Failure(ProtobufFailure.Malformed), StudioPilotCodec.decode(
            StudioPilotKind.SaveRevisionRequest, bytes + byteArrayOf(0),
        ))
    }

    @Test
    fun repeatedOnlyConcatenationIsAnOrderedListAsTheContractSpecifies() {
        val first = ScriptValidationResponse(listOf(FieldIssue("first", "a")))
        val second = ScriptValidationResponse(listOf(FieldIssue("second", "b")))
        assertEquals(ProtobufResult.Value(ScriptValidationResponse(first.issues + second.issues)), StudioPilotCodec.decode(
            StudioPilotKind.ScriptValidationResponse,
            success(StudioPilotCodec.encode(first)) + success(StudioPilotCodec.encode(second)),
        ))
        assertEquals(ProtobufResult.Value(ScriptValidationResponse(emptyList())), StudioPilotCodec.decode(
            StudioPilotKind.ScriptValidationResponse, byteArrayOf(),
        ))
    }

    @Test
    fun maximumEnvelopeIsAcceptedAndTheNextByteIsRefused() {
        val exact = ValidateScriptRequest("a".repeat(StudioPilotCodec.MAX_BYTES - 5))
        val bytes = success(StudioPilotCodec.encode(exact))
        assertEquals(StudioPilotCodec.MAX_BYTES, bytes.size)
        assertEquals(ProtobufResult.Value(exact), StudioPilotCodec.decode(StudioPilotKind.ValidateScriptRequest, bytes))
        assertEquals(ProtobufResult.Failure(ProtobufFailure.TooLarge), StudioPilotCodec.encode(
            ValidateScriptRequest(exact.scriptJson + "a"),
        ))
        assertEquals(ProtobufResult.Failure(ProtobufFailure.TooLarge), StudioPilotCodec.decode(
            StudioPilotKind.ValidateScriptRequest, bytes + byteArrayOf(0),
        ))
    }

    @Test
    fun repeatedIssueBoundIsInclusiveAndCheckedOnEncodeAndDecode() {
        val value = ScriptValidationResponse(List(StudioPilotCodec.MAX_ISSUES) { FieldIssue("", "") })
        val bytes = success(StudioPilotCodec.encode(value))
        assertEquals(ProtobufResult.Value(value), StudioPilotCodec.decode(StudioPilotKind.ScriptValidationResponse, bytes))
        assertEquals(ProtobufResult.Failure(ProtobufFailure.TooLarge), StudioPilotCodec.encode(
            ScriptValidationResponse(value.issues + FieldIssue("", "")),
        ))
        assertEquals(ProtobufResult.Failure(ProtobufFailure.TooLarge), StudioPilotCodec.decode(
            StudioPilotKind.ScriptValidationResponse, bytes + hex("0a040a001200"),
        ))
    }

    @Test
    fun allTwoByteEnvelopesHaveClosedOutcomesForEveryRoot() {
        for (value in 0..65535) {
            val bytes = byteArrayOf((value shr 8).toByte(), value.toByte())
            for (kind in StudioPilotKind.entries) {
                when (val result = StudioPilotCodec.decode(kind, bytes)) {
                    is ProtobufResult.Failure -> Unit
                    is ProtobufResult.Value -> assertEquals(
                        ProtobufResult.Value(result.value),
                        StudioPilotCodec.decode(kind, success(StudioPilotCodec.encode(result.value))),
                    )
                }
            }
        }
    }

    @Test
    fun boundedDeterministicLongerInputsNeverEscapeTheTypedBoundary() {
        var state = 0x12345678u
        repeat(10_000) { iteration ->
            val bytes = ByteArray(iteration % 257) {
                state = state * 1664525u + 1013904223u
                (state shr 24).toByte()
            }
            for (kind in StudioPilotKind.entries) {
                val result = StudioPilotCodec.decode(kind, bytes)
                if (result is ProtobufResult.Value) assertEquals(
                    ProtobufResult.Value(result.value),
                    StudioPilotCodec.decode(kind, success(StudioPilotCodec.encode(result.value))),
                )
            }
        }
    }

    private fun success(result: ProtobufResult<ByteArray>): ByteArray = assertIs<ProtobufResult.Value<ByteArray>>(result).value
    private fun hex(value: String): ByteArray = value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
}
