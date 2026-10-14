@file:OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)
import kotlinx.serialization.Serializable
import kotlinx.serialization.KSerializer
import kotlinx.serialization.protobuf.ProtoBuf
import kotlinx.serialization.protobuf.ProtoNumber
import kotlinx.serialization.protobuf.ProtoOneOf

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

// Artwork is a fixture-only schema mirror. It grants no native API/serializer authority.
@Serializable enum class ImageMime { Unspecified, Png, Jpeg }
@Serializable enum class ImageIntentState { Unspecified, Pending, Admitted, Expired }
@Serializable data class ImageDeclarations(
    @ProtoNumber(1) val provenance: String? = null,
    @ProtoNumber(2) val rightsRefs: List<String> = emptyList(),
)
@Serializable data class CreateImageIntentRequest(
    @ProtoNumber(1) val operationId: String? = null,
    @ProtoNumber(2) val id: String? = null,
    @ProtoNumber(3) val expectedSha256: String? = null,
    @ProtoNumber(4) val expectedByteLength: Long? = null,
    @ProtoNumber(5) val contentType: ImageMime? = null,
    @ProtoNumber(6) val declarations: ImageDeclarations? = null,
)
@Serializable data class ImageIntentDocument(
    @ProtoNumber(1) val schemaVersion: String? = null,
    @ProtoNumber(2) val id: String? = null,
    @ProtoNumber(3) val workId: String? = null,
    @ProtoNumber(4) val ownerId: String? = null,
    @ProtoNumber(5) val operationId: String? = null,
    @ProtoNumber(6) val request: CreateImageIntentRequest? = null,
    @ProtoNumber(7) val createdAtMs: Long? = null,
    @ProtoNumber(8) val expiresAtMs: Long? = null,
    @ProtoNumber(9) val digest: String? = null,
    @ProtoNumber(10) val profileId: String? = null,
)
@Serializable data class ImageVariantMetadata(
    @ProtoNumber(1) val sha256: String? = null,
    @ProtoNumber(2) val byteLength: Long? = null,
    @ProtoNumber(3) val contentType: ImageMime? = null,
    // Positive Long preserves every uint32 value without signed-Int varint expansion.
    @ProtoNumber(4) val widthPx: Long? = null,
    @ProtoNumber(5) val heightPx: Long? = null,
    @ProtoNumber(6) val profileId: String? = null,
)
@Serializable data class ImageAssetDocument(
    @ProtoNumber(1) val schemaVersion: String? = null,
    @ProtoNumber(2) val id: String? = null,
    @ProtoNumber(3) val intentId: String? = null,
    @ProtoNumber(4) val workId: String? = null,
    @ProtoNumber(5) val ownerId: String? = null,
    @ProtoNumber(6) val verifiedMedia: ImageVariantMetadata? = null,
    @ProtoNumber(7) val declarations: ImageDeclarations? = null,
    @ProtoNumber(8) val admittedAtMs: Long? = null,
    @ProtoNumber(9) val digest: String? = null,
)
@Serializable data class ImageIntentResponse(
    @ProtoNumber(1) val intent: ImageIntentDocument? = null,
    @ProtoNumber(2) val asset: ImageAssetDocument? = null,
    @ProtoNumber(3) val state: ImageIntentState? = null,
)
@Serializable data class WorkArtworkSelection(
    @ProtoNumber(1) val imageAssetId: String? = null,
    @ProtoNumber(2) val imageAssetDigest: String? = null,
    @ProtoNumber(3) val altText: String? = null,
)
@Serializable data object ClearArtwork
@Serializable sealed interface ArtworkChange {
    @Serializable data class Set(@ProtoNumber(3) val artwork: WorkArtworkSelection) : ArtworkChange
    @Serializable data class Clear(@ProtoNumber(4) val clearArtwork: ClearArtwork) : ArtworkChange
}
@Serializable data class SetWorkArtworkRequest(
    @ProtoNumber(1) val operationId: String? = null,
    @ProtoNumber(2) val expectedWorkRevision: Long? = null,
    @ProtoOneOf val change: ArtworkChange? = null,
)
@Serializable data class WorkArtworkResponse(
    @ProtoNumber(1) val workId: String? = null,
    @ProtoNumber(2) val workRevision: Long? = null,
    @ProtoNumber(3) val artwork: WorkArtworkSelection? = null,
    @ProtoNumber(4) val changedAtMs: Long? = null,
)
