package com.cantos.theatre.domain.identity

/** Noncredential session facts. The optional credential is retained for compatibility callers. */
class NativeSession(
    val actor: String,
    val session: String,
    val expiresAt: Long,
    val credential: String? = null,
    val absoluteExpiresAt: Long = expiresAt,
    val authenticatedAt: Long? = null,
) {
    override fun toString(): String = "NativeSession([redacted])"
}

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
        if (now < 0 || refreshExpiresAt <= now || expiresAt - now > 120 ||
            refreshExpiresAt - now > 600 || absoluteExpiresAt - now > 900 ||
            authenticatedAt?.let { it > now } == true) return null
        if (previous != null && (view.actor != previous.view.actor || view.session != previous.view.session ||
                absoluteExpiresAt != previous.absoluteExpiresAt || authenticatedAt != previous.authenticatedAt ||
                accessToken == previous.accessToken || refreshToken == previous.refreshToken)) return null
        return this
    }

    companion object {
        private val access = Regex("ca1_n_[0-9a-f]{64}")
        private val refresh = Regex("cr1_[0-9a-f]{64}")
        private val sessionId = Regex("[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}")

        fun isAccessToken(value: String): Boolean = access.matches(value)
        fun isRefreshToken(value: String): Boolean = refresh.matches(value)
        fun isSessionId(value: String): Boolean = sessionId.matches(value)

        /** Structural restoration also accepts expired pairs so durable cleanup remains possible. */
        fun restore(
            accessToken: String,
            refreshToken: String,
            actor: String,
            session: String,
            expiresAt: Long,
            refreshExpiresAt: Long,
            absoluteExpiresAt: Long,
            authenticatedAt: Long?,
        ): NativeGrant? {
            if (!isAccessToken(accessToken) || !isRefreshToken(refreshToken) ||
                actor.isEmpty() || actor.length > 128 || actor.any { it.isISOControl() } ||
                !isSessionId(session) || expiresAt < 0 || refreshExpiresAt < expiresAt ||
                absoluteExpiresAt < refreshExpiresAt ||
                authenticatedAt?.let { it < 0 || it > absoluteExpiresAt } == true) return null
            return NativeGrant(
                accessToken,
                refreshToken,
                NativeSession(actor, session, expiresAt, absoluteExpiresAt = absoluteExpiresAt, authenticatedAt = authenticatedAt),
                refreshExpiresAt,
            )
        }

        fun create(
            accessToken: String,
            refreshToken: String,
            actor: String,
            session: String,
            expiresAt: Long,
            refreshExpiresAt: Long,
            absoluteExpiresAt: Long,
            authenticatedAt: Long?,
            now: Long,
            previous: NativeGrant? = null,
        ): NativeGrant? = restore(
            accessToken, refreshToken, actor, session, expiresAt, refreshExpiresAt,
            absoluteExpiresAt, authenticatedAt,
        )?.admit(now, previous)
    }
}
