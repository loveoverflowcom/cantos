package com.cantos.theatre.domain.playback

/** Durable streaming identity. URLs, display order and the active-release pointer are absent. */
@ConsistentCopyVisibility
data class AudioIdentity private constructor(
    val workId: String,
    val unitId: String,
    val publicationId: String,
    val renditionId: String,
    val sha256: String,
    val durationMs: Long,
) {
    companion object {
        private val uuid = Regex("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
        fun of(workId: String, unitId: String, publicationId: String, renditionId: String,
               sha256: String, durationMs: Long): AudioIdentity? =
            if (listOf(workId, unitId, publicationId, renditionId).all(uuid::matches) &&
                sha256.matches(Regex("[0-9a-f]{64}")) && durationMs in 1..3_600_000) {
                AudioIdentity(workId, unitId, publicationId, renditionId, sha256, durationMs)
            } else null
    }
}
