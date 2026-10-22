package com.cantos.theatre.domain.identity

/** Noncredential session facts: never a bearer, refresh secret or other credential. */
class NativeSession(
    val actor: String,
    val session: String,
    val expiresAt: Long,
    val absoluteExpiresAt: Long = expiresAt,
    val authenticatedAt: Long? = null,
) {
    override fun toString(): String = "NativeSession([redacted])"

    companion object {
        /** Identity v1 never treats a missing authentication time as recent authentication. */
        fun identityV1(
            actor: String, session: String, expiresAt: Long, absoluteExpiresAt: Long,
            authenticatedAt: Long, now: Long,
        ): IdentityV1Validation<NativeSession> {
            val structural = restoreIdentityV1(actor, session, expiresAt, absoluteExpiresAt, authenticatedAt)
            if (structural is IdentityV1Validation.Refused) return structural
            return when {
                now < 0 || expiresAt <= now || !nativeDeadlineBounds(expiresAt, absoluteExpiresAt, now) ->
                    IdentityV1Validation.Refused(IdentityV1Invalid.Deadline)
                authenticatedAt > now -> IdentityV1Validation.Refused(IdentityV1Invalid.AuthenticatedAt)
                else -> structural
            }
        }

        /** Structural recovery retains expired facts for cleanup, without admitting a request. */
        internal fun restoreIdentityV1(
            actor: String, session: String, expiresAt: Long, absoluteExpiresAt: Long, authenticatedAt: Long,
        ): IdentityV1Validation<NativeSession> = when {
            !Regex("[a-z0-9_-]{1,64}").matches(actor) -> IdentityV1Validation.Refused(IdentityV1Invalid.Actor)
            !NativeGrant.isSessionId(session) || session == "00000000-0000-0000-0000-000000000000" ->
                IdentityV1Validation.Refused(IdentityV1Invalid.Session)
            expiresAt <= 0 || absoluteExpiresAt < expiresAt ->
                IdentityV1Validation.Refused(IdentityV1Invalid.Deadline)
            authenticatedAt <= 0 || authenticatedAt >= expiresAt ->
                IdentityV1Validation.Refused(IdentityV1Invalid.AuthenticatedAt)
            else -> IdentityV1Validation.Accepted(NativeSession(
                actor, session, expiresAt, absoluteExpiresAt = absoluteExpiresAt, authenticatedAt = authenticatedAt,
            ))
        }
    }
}

enum class IdentityV1Invalid { Credential, Actor, Session, Deadline, AuthenticatedAt, Rotation }
sealed interface IdentityV1Validation<out T> {
    class Accepted<T>(val value: T) : IdentityV1Validation<T>
    data class Refused(val reason: IdentityV1Invalid) : IdentityV1Validation<Nothing>
}

// Existing native wire bounds; these are not issuance policy.
private fun nativeDeadlineBounds(expiresAt: Long, absoluteExpiresAt: Long, now: Long): Boolean =
    expiresAt - now <= 120 && absoluteExpiresAt - now <= 900

/** A native-purpose Cantos pair; validity in time is checked separately from secure restoration. */
class NativeGrant private constructor(
    val accessToken: String,
    val refreshToken: String,
    val view: NativeSession,
    val refreshExpiresAt: Long,
) {
    val expiresAt: Long get() = view.expiresAt
    val absoluteExpiresAt: Long get() = view.absoluteExpiresAt
    val authenticatedAt: Long? get() = view.authenticatedAt

    override fun toString(): String = "NativeGrant([redacted])"

    /** Expired access may be restored only to refresh; it is never evidence for dispatch. */
    fun admit(now: Long, previous: NativeGrant? = null): NativeGrant? {
        if (now < 0 || refreshExpiresAt <= now || !nativeDeadlineBounds(expiresAt, absoluteExpiresAt, now) ||
            refreshExpiresAt - now > 600 ||
            authenticatedAt?.let { it > now } == true) return null
        if (previous != null && (view.actor != previous.view.actor || view.session != previous.view.session ||
                absoluteExpiresAt != previous.absoluteExpiresAt || authenticatedAt != previous.authenticatedAt ||
                accessToken == previous.accessToken || refreshToken == previous.refreshToken ||
                refreshExpiresAt < previous.refreshExpiresAt || expiresAt < previous.expiresAt)) return null
        return this
    }

    companion object {
        private val identityAccess = Regex("ci1_n_[0-9a-f]{64}")
        private val identityRefresh = Regex("ci1_r_[0-9a-f]{64}")
        private val sessionId = Regex("[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}")

        fun isSessionId(value: String): Boolean = sessionId.matches(value)
        fun isIdentityV1AccessToken(value: String): Boolean = identityAccess.matches(value)
        fun isIdentityV1RefreshToken(value: String): Boolean = identityRefresh.matches(value)

        /** The only admission entry: a fresh API response is checked against the current time and, on rotation, the previous pair. */
        fun identityV1(
            accessToken: String, refreshToken: String, actor: String, session: String,
            expiresAt: Long, refreshExpiresAt: Long, absoluteExpiresAt: Long,
            authenticatedAt: Long, now: Long, previous: NativeGrant? = null,
        ): IdentityV1Validation<NativeGrant> {
            val grant = when (val checked = restoreIdentityV1(
                accessToken, refreshToken, actor, session, expiresAt, refreshExpiresAt, absoluteExpiresAt, authenticatedAt,
            )) {
                is IdentityV1Validation.Accepted -> checked.value
                is IdentityV1Validation.Refused -> return checked
            }
            when (val checked = NativeSession.identityV1(actor, session, expiresAt, absoluteExpiresAt, authenticatedAt, now)) {
                is IdentityV1Validation.Accepted -> Unit
                is IdentityV1Validation.Refused -> return checked
            }
            if (previous != null && (grant.admit(now, previous) == null)) {
                return IdentityV1Validation.Refused(IdentityV1Invalid.Rotation)
            }
            return if (grant.admit(now) == null) IdentityV1Validation.Refused(IdentityV1Invalid.Deadline)
            else IdentityV1Validation.Accepted(grant)
        }

        /** Secure-store decoding; an expired pair only enables cleanup/refresh and is never evidence for dispatch. */
        fun restoreIdentityV1(
            accessToken: String, refreshToken: String, actor: String, session: String,
            expiresAt: Long, refreshExpiresAt: Long, absoluteExpiresAt: Long, authenticatedAt: Long,
        ): IdentityV1Validation<NativeGrant> {
            if (!isIdentityV1AccessToken(accessToken) || !isIdentityV1RefreshToken(refreshToken)) {
                return IdentityV1Validation.Refused(IdentityV1Invalid.Credential)
            }
            val view = when (val checked = NativeSession.restoreIdentityV1(actor, session, expiresAt, absoluteExpiresAt, authenticatedAt)) {
                is IdentityV1Validation.Accepted -> checked.value
                is IdentityV1Validation.Refused -> return checked
            }
            if (refreshExpiresAt < expiresAt || absoluteExpiresAt < refreshExpiresAt) {
                return IdentityV1Validation.Refused(IdentityV1Invalid.Deadline)
            }
            return IdentityV1Validation.Accepted(NativeGrant(accessToken, refreshToken, view, refreshExpiresAt))
        }
    }
}
