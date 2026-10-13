@file:OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)
import kotlinx.serialization.Serializable
import kotlinx.serialization.KSerializer
import kotlinx.serialization.protobuf.ProtoBuf
import kotlinx.serialization.protobuf.ProtoNumber

// Mirror the reviewed proto2 wire representation, not application witnesses.
// Long carries uint64 raw bits; use toULong()/toLong() at the value boundary.
@Serializable data class ValidateScriptRequest(@ProtoNumber(1) val scriptJson: String? = null)
@Serializable data class SaveRevisionRequest(
    @ProtoNumber(1) val expectedRevision: Long? = null,
    @ProtoNumber(2) val operationId: String? = null,
    @ProtoNumber(3) val scriptJson: String? = null,
)
@Serializable data class RevisionResponse(
    @ProtoNumber(1) val scriptId: String? = null,
    @ProtoNumber(2) val revision: Long? = null,
    @ProtoNumber(3) val acceptedBy: String? = null,
    @ProtoNumber(4) val acceptedAt: String? = null,
    @ProtoNumber(5) val contentDigest: String? = null,
    @ProtoNumber(6) val exportDigest: String? = null,
    @ProtoNumber(7) val scriptJson: String? = null,
)
@Serializable data class FieldIssue(
    @ProtoNumber(1) val path: String? = null,
    @ProtoNumber(2) val rule: String? = null,
)
@Serializable data class ScriptValidationResponse(@ProtoNumber(1) val issues: List<FieldIssue> = emptyList())
fun <T> checkVector(name: String, serializer: KSerializer<T>, expected: T, hex: String) {
    val bytes = hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    check(ProtoBuf.encodeToByteArray(serializer, expected).contentEquals(bytes)) { "encode $name" }
    check(ProtoBuf.decodeFromByteArray(serializer, bytes) == expected) { "decode $name" }
}
