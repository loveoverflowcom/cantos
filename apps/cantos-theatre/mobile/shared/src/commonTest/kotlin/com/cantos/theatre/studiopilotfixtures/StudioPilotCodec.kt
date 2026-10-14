@file:OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)

package com.cantos.theatre.studiopilotfixtures

import kotlinx.serialization.SerializationException
import kotlinx.serialization.protobuf.ProtoBuf

/**
 * Test-only Studio pilot fixture codec for Protobuf compatibility and JVM measurement. It lives
 * in `commonTest` so Theatre Mobile never ships it; it has no HTTP, auth, storage or playback.
 */
object StudioPilotCodec {
    const val MAX_BYTES = 8 * 1024 * 1024
    const val MAX_DEPTH = 4
    const val MAX_ISSUES = 65_536
    const val MAX_FIELDS = 196_616
    private val protobuf = ProtoBuf { encodeDefaults = false }

    fun encode(value: StudioPilotValue): ProtobufResult<ByteArray> {
        val size = encodedSize(value)
        if (size < 0) return ProtobufResult.Failure(ProtobufFailure.InvalidUtf8)
        if (size > MAX_BYTES) return ProtobufResult.Failure(ProtobufFailure.TooLarge)
        return try {
            val bytes = when (value) {
                is ValidateScriptRequest -> protobuf.encodeToByteArray(
                    ValidateWire.serializer(), ValidateWire(value.scriptJson),
                )
                is SaveRevisionRequest -> protobuf.encodeToByteArray(
                    SaveWire.serializer(), SaveWire(value.expectedRevision.toLong(), value.operationId, value.scriptJson),
                )
                is RevisionResponse -> protobuf.encodeToByteArray(
                    RevisionWire.serializer(), RevisionWire(
                        value.scriptId, value.revision.toLong(), value.acceptedBy, value.acceptedAt,
                        value.contentDigest, value.exportDigest, value.scriptJson,
                    ),
                )
                is ScriptValidationResponse -> protobuf.encodeToByteArray(
                    ValidationWire.serializer(), ValidationWire(value.issues.map { IssueWire(it.path, it.rule) }),
                )
            }
            val failure = StrictProtobufEnvelope(bytes).scan(kindOf(value))
            if (failure == null) ProtobufResult.Value(bytes) else ProtobufResult.Failure(failure)
        } catch (_: SerializationException) {
            ProtobufResult.Failure(ProtobufFailure.Malformed)
        }
    }

    fun decode(kind: StudioPilotKind, bytes: ByteArray): ProtobufResult<StudioPilotValue> {
        val failure = StrictProtobufEnvelope(bytes).scan(kind)
        if (failure != null) return ProtobufResult.Failure(failure)
        return try {
            val value = when (kind) {
                StudioPilotKind.ValidateScriptRequest -> {
                    val wire = protobuf.decodeFromByteArray(ValidateWire.serializer(), bytes)
                    ValidateScriptRequest(wire.scriptJson ?: return missing())
                }
                StudioPilotKind.SaveRevisionRequest -> {
                    val wire = protobuf.decodeFromByteArray(SaveWire.serializer(), bytes)
                    SaveRevisionRequest(
                        (wire.expectedRevision ?: return missing()).toULong(),
                        wire.operationId ?: return missing(), wire.scriptJson ?: return missing(),
                    )
                }
                StudioPilotKind.RevisionResponse -> {
                    val wire = protobuf.decodeFromByteArray(RevisionWire.serializer(), bytes)
                    RevisionResponse(
                        wire.scriptId ?: return missing(), (wire.revision ?: return missing()).toULong(),
                        wire.acceptedBy ?: return missing(), wire.acceptedAt ?: return missing(),
                        wire.contentDigest ?: return missing(), wire.exportDigest ?: return missing(),
                        wire.scriptJson ?: return missing(),
                    )
                }
                StudioPilotKind.ScriptValidationResponse -> {
                    val wire = protobuf.decodeFromByteArray(ValidationWire.serializer(), bytes)
                    ScriptValidationResponse(wire.issues.map {
                        FieldIssue(it.path ?: return missing(), it.rule ?: return missing())
                    })
                }
            }
            ProtobufResult.Value(value)
        } catch (_: SerializationException) {
            ProtobufResult.Failure(ProtobufFailure.Malformed)
        }
    }

    private fun missing() = ProtobufResult.Failure(ProtobufFailure.Shape)

    internal fun kindOf(value: StudioPilotValue): StudioPilotKind = when (value) {
        is ValidateScriptRequest -> StudioPilotKind.ValidateScriptRequest
        is SaveRevisionRequest -> StudioPilotKind.SaveRevisionRequest
        is RevisionResponse -> StudioPilotKind.RevisionResponse
        is ScriptValidationResponse -> StudioPilotKind.ScriptValidationResponse
    }

    // Calculate an exact bounded size without encoding strings, copying lists or allocating a
    // serializer output. Invalid UTF-16 cannot be silently replaced with a different manuscript.
    private fun encodedSize(value: StudioPilotValue): Long {
        val strings = when (value) {
            is ValidateScriptRequest -> listOf(value.scriptJson)
            is SaveRevisionRequest -> listOf(value.operationId, value.scriptJson)
            is RevisionResponse -> listOf(
                value.scriptId, value.acceptedBy, value.acceptedAt,
                value.contentDigest, value.exportDigest, value.scriptJson,
            )
            is ScriptValidationResponse -> {
                if (value.issues.size > MAX_ISSUES) return MAX_BYTES.toLong() + 1
                var total = 0L
                for (issue in value.issues) {
                    val path = stringSize(issue.path)
                    val rule = stringSize(issue.rule)
                    if (path < 0 || rule < 0) return -1
                    val nested = path + rule
                    total += 1 + varintSize(nested.toULong()) + nested
                    if (total > MAX_BYTES) return total
                }
                return total
            }
        }
        var total = when (value) {
            is SaveRevisionRequest -> 1L + varintSize(value.expectedRevision)
            is RevisionResponse -> 1L + varintSize(value.revision)
            else -> 0L
        }
        for (string in strings) {
            val size = stringSize(string)
            if (size < 0) return -1
            total += size
            if (total > MAX_BYTES) return total
        }
        return total
    }

    private fun stringSize(value: String): Long {
        if (value.length > MAX_BYTES) return MAX_BYTES.toLong() + 1
        var size = 0L
        var offset = 0
        while (offset < value.length) {
            val char = value[offset++].code
            size += when {
                char <= 127 -> 1
                char <= 2047 -> 2
                char in 0xd800..0xdbff -> {
                    if (offset == value.length || value[offset++].code !in 0xdc00..0xdfff) return -1
                    4
                }
                char in 0xdc00..0xdfff -> return -1
                else -> 3
            }
            if (size > MAX_BYTES) return size
        }
        return 1 + varintSize(size.toULong()) + size
    }

    private fun varintSize(value: ULong): Int {
        var remaining = value
        var size = 1
        while (remaining >= 128uL) {
            remaining = remaining shr 7
            size++
        }
        return size
    }
}
