package com.cantos.theatre.adapters.protobuf

/** Bounded scan of buffered bytes before kotlinx constructs message values, strings or lists. */
internal class StrictProtobufEnvelope(private val bytes: ByteArray) {
    private var cursor = 0
    private var fields = 0
    private var issues = 0
    private var failure: ProtobufFailure? = null

    fun scan(kind: StudioPilotKind): ProtobufFailure? {
        if (bytes.size > StudioPilotCodec.MAX_BYTES) return ProtobufFailure.TooLarge
        message(kind, bytes.size, 0, false)
        return failure
    }

    private fun reject(reason: ProtobufFailure): Boolean {
        failure = reason
        return false
    }

    private fun message(kind: StudioPilotKind, end: Int, depth: Int, issue: Boolean): Boolean {
        if (depth > StudioPilotCodec.MAX_DEPTH) return reject(ProtobufFailure.TooDeep)
        var seen = 0
        val required = if (issue) 3 else when (kind) {
            StudioPilotKind.ValidateScriptRequest -> 1
            StudioPilotKind.SaveRevisionRequest -> 7
            StudioPilotKind.RevisionResponse -> 127
            StudioPilotKind.ScriptValidationResponse -> 0
        }
        while (cursor < end) {
            if (++fields > StudioPilotCodec.MAX_FIELDS) return reject(ProtobufFailure.TooLarge)
            val key = varint(end) ?: return false
            if (key == 0uL || key > 0xffffffffuL || key shr 3 == 0uL) {
                return reject(ProtobufFailure.Malformed)
            }
            val number = (key shr 3).toInt()
            val wire = (key and 7uL).toInt()
            val maxField = if (issue) 2 else when (kind) {
                StudioPilotKind.ValidateScriptRequest, StudioPilotKind.ScriptValidationResponse -> 1
                StudioPilotKind.SaveRevisionRequest -> 3
                StudioPilotKind.RevisionResponse -> 7
            }
            if (number > maxField) return reject(ProtobufFailure.Shape)
            val nested = !issue && kind == StudioPilotKind.ScriptValidationResponse
            val unsigned = !issue && (
                (kind == StudioPilotKind.SaveRevisionRequest && number == 1) ||
                    (kind == StudioPilotKind.RevisionResponse && number == 2)
                )
            if (wire != if (unsigned) 0 else 2) return reject(ProtobufFailure.Shape)
            if (!nested) {
                val bit = 1 shl (number - 1)
                if (seen and bit != 0) return reject(ProtobufFailure.Shape)
                seen = seen or bit
            }
            if (unsigned) {
                varint(end) ?: return false
            } else {
                val length = varint(end) ?: return false
                if (length > (end - cursor).toULong()) return reject(ProtobufFailure.Truncated)
                val fieldEnd = cursor + length.toInt()
                if (nested) {
                    if (++issues > StudioPilotCodec.MAX_ISSUES) return reject(ProtobufFailure.TooLarge)
                    if (!message(kind, fieldEnd, depth + 1, true)) return false
                } else {
                    if (!utf8(fieldEnd)) return false
                    cursor = fieldEnd
                }
            }
        }
        return seen == required || reject(ProtobufFailure.Shape)
    }

    private fun varint(end: Int): ULong? {
        var value = 0uL
        for (index in 0 until 10) {
            if (cursor == end) {
                reject(ProtobufFailure.Truncated)
                return null
            }
            val byte = bytes[cursor++].toInt() and 255
            if (index == 9 && byte > 1) {
                reject(ProtobufFailure.Malformed)
                return null
            }
            value = value or ((byte and 127).toULong() shl (index * 7))
            if (byte and 128 == 0) {
                if (index > 0 && byte == 0) {
                    reject(ProtobufFailure.Malformed)
                    return null
                }
                return value
            }
        }
        reject(ProtobufFailure.Malformed)
        return null
    }

    private fun utf8(end: Int): Boolean {
        var offset = cursor
        while (offset < end) {
            val first = bytes[offset++].toInt() and 255
            if (first <= 127) continue
            val count = when (first) {
                in 194..223 -> 1
                in 224..239 -> 2
                in 240..244 -> 3
                else -> return reject(ProtobufFailure.InvalidUtf8)
            }
            if (count > end - offset) return reject(ProtobufFailure.InvalidUtf8)
            val second = bytes[offset].toInt() and 255
            if ((first == 224 && second < 160) || (first == 237 && second > 159) ||
                (first == 240 && second < 144) || (first == 244 && second > 143)
            ) return reject(ProtobufFailure.InvalidUtf8)
            repeat(count) {
                if ((bytes[offset++].toInt() and 255) !in 128..191) {
                    return reject(ProtobufFailure.InvalidUtf8)
                }
            }
        }
        return true
    }
}
