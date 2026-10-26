package com.cantos.theatre.ports

import com.cantos.theatre.domain.playback.AudioIdentity

enum class PlaybackAccessFailure { Network, Unavailable, NotFound, IdentityChanged, Protocol }
sealed interface PlaybackAccessResult<out T> {
    data class Accepted<T>(val value: T) : PlaybackAccessResult<T>
    data class Refused(val reason: PlaybackAccessFailure) : PlaybackAccessResult<Nothing>
}
/** [expiresAtMs] is on the server's clock: issued as the server's now + [TTL_MS]. */
class PlaybackGrant(val location: PlaybackLocation, val expiresAtMs: Long) {
    override fun toString(): String = "PlaybackGrant([redacted])"

    companion object {
        const val TTL_MS = 120_000L
        /** A listener's clock may run behind the server's; the upper bound is a sanity check only. */
        const val CLOCK_SKEW_ALLOWANCE_MS = 300_000L
    }
}
interface PlaybackAccessPort {
    suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity>
    suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant>
}
