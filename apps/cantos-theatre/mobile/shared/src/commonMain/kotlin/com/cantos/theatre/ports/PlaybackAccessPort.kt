package com.cantos.theatre.ports

import com.cantos.theatre.domain.playback.AudioIdentity

enum class PlaybackAccessFailure { Network, Unavailable, NotFound, IdentityChanged, Protocol }
sealed interface PlaybackAccessResult<out T> {
    data class Accepted<T>(val value: T) : PlaybackAccessResult<T>
    data class Refused(val reason: PlaybackAccessFailure) : PlaybackAccessResult<Nothing>
}
class PlaybackGrant(val location: PlaybackLocation, val expiresAtMs: Long) {
    override fun toString(): String = "PlaybackGrant([redacted])"
}
interface PlaybackAccessPort {
    suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity>
    suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant>
}
