package com.cantos.theatre.adapters.protobuf

/** Transport values only. Presence is checked, but identity, rights and content are untrusted. */
sealed interface StudioPilotValue

data class ValidateScriptRequest(val scriptJson: String) : StudioPilotValue

data class SaveRevisionRequest(
    val expectedRevision: ULong,
    val operationId: String,
    val scriptJson: String,
) : StudioPilotValue

data class RevisionResponse(
    val scriptId: String,
    val revision: ULong,
    val acceptedBy: String,
    val acceptedAt: String,
    val contentDigest: String,
    val exportDigest: String,
    val scriptJson: String,
) : StudioPilotValue

data class FieldIssue(val path: String, val rule: String)

data class ScriptValidationResponse(val issues: List<FieldIssue>) : StudioPilotValue

enum class StudioPilotKind {
    ValidateScriptRequest,
    SaveRevisionRequest,
    RevisionResponse,
    ScriptValidationResponse,
}

enum class ProtobufFailure {
    Truncated,
    TooDeep,
    TrailingBytes,
    InvalidUtf8,
    Shape,
    Malformed,
    TooLarge,
}

sealed interface ProtobufResult<out T> {
    data class Value<T>(val value: T) : ProtobufResult<T>
    data class Failure(val reason: ProtobufFailure) : ProtobufResult<Nothing>
}
