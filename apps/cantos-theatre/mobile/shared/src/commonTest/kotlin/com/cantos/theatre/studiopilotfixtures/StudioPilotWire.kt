@file:OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)

package com.cantos.theatre.studiopilotfixtures

import kotlinx.serialization.Serializable
import kotlinx.serialization.protobuf.ProtoNumber

// Mirrors packages/contracts/protobuf/studio_v1.proto. Optional scalars expose wire presence.
// Long is uint64 raw bits, converted only with toULong/toLong, never signed arithmetic.
@Serializable
internal data class ValidateWire(@ProtoNumber(1) val scriptJson: String? = null)

@Serializable
internal data class SaveWire(
    @ProtoNumber(1) val expectedRevision: Long? = null,
    @ProtoNumber(2) val operationId: String? = null,
    @ProtoNumber(3) val scriptJson: String? = null,
)

@Serializable
internal data class RevisionWire(
    @ProtoNumber(1) val scriptId: String? = null,
    @ProtoNumber(2) val revision: Long? = null,
    @ProtoNumber(3) val acceptedBy: String? = null,
    @ProtoNumber(4) val acceptedAt: String? = null,
    @ProtoNumber(5) val contentDigest: String? = null,
    @ProtoNumber(6) val exportDigest: String? = null,
    @ProtoNumber(7) val scriptJson: String? = null,
)

@Serializable
internal data class IssueWire(
    @ProtoNumber(1) val path: String? = null,
    @ProtoNumber(2) val rule: String? = null,
)

@Serializable
internal data class ValidationWire(@ProtoNumber(1) val issues: List<IssueWire> = emptyList())
