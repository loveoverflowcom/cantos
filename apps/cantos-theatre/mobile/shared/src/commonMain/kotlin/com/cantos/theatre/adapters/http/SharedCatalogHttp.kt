package com.cantos.theatre.adapters.http

import com.cantos.theatre.application.identity.NativeVerifiedSession
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicUnitPage
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkPage
import com.cantos.theatre.ports.NativeHttpFailure
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.ports.SharedCatalogFailure
import com.cantos.theatre.ports.SharedCatalogPort
import com.cantos.theatre.ports.SharedCatalogRead
import com.cantos.theatre.ports.SharedSessionKey
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.KSerializer
import kotlin.coroutines.resume

internal fun NativeVerifiedSession.sharedKey(): SharedSessionKey = SharedSessionKey(session.actor, session.session, intent)

/** S07's strict Discovery shapes, on the shared route only. No cache or anonymous retry. */
class SharedCatalogHttp(
    origin: ApiOrigin,
    private val http: NativeHttpPort,
    private val invalidateSession: (String) -> Unit = {},
    private val current: () -> NativeVerifiedSession?,
) : SharedCatalogPort {
    private val base = "${origin.probeUrl}/shared/works"

    override suspend fun works(session: SharedSessionKey, cursor: CatalogCursor?): SharedCatalogRead<PublicWorkPage> =
        read(session, page(base, cursor), cursor, MetadataWorkPageWire.serializer()) { wire ->
            if (wire.items.any { it.cover != null } || wire.items.size > CatalogPageLimit) null
            else wire.toDomain().acceptedOrNull()
        }

    override suspend fun work(session: SharedSessionKey, id: PublicWorkId): SharedCatalogRead<PublicWork> =
        read(session, "$base/${id.value}", null, MetadataWorkWire.serializer()) { wire ->
            if (wire.cover != null) null else wire.toDomain().acceptedOrNull()?.takeIf { it.id == id }
        }

    override suspend fun units(session: SharedSessionKey, work: PublicWork, cursor: CatalogCursor?): SharedCatalogRead<PublicUnitPage> =
        read(session, page("$base/${work.id.value}/units", cursor), cursor, MetadataUnitPageWire.serializer()) { wire ->
            if (wire.items.size > CatalogPageLimit) null else wire.toDomain(work).acceptedOrNull()
        }

    private fun page(path: String, cursor: CatalogCursor?): String =
        "$path?limit=$CatalogPageLimit" + (cursor?.let { "&cursor=${it.value}" } ?: "")

    private suspend fun <W, T> read(
        session: SharedSessionKey,
        url: String,
        cursor: CatalogCursor?,
        serializer: KSerializer<W>,
        map: (W) -> T?,
    ): SharedCatalogRead<T> {
        val lease = current()?.takeIf { it.sharedKey() == session }
            ?: return SharedCatalogRead.Refused(SharedCatalogFailure.SignInRequired)
        val response = withTimeoutOrNull(15_000) {
            suspendCancellableCoroutine { continuation ->
                val handle = http.send(NativeHttpRequest("GET", url, null, lease.bearer, lease.session.session, CatalogBodyLimitBytes)) {
                    if (continuation.isActive) continuation.resume(it)
                }
                continuation.invokeOnCancellation { handle.cancel() }
            }
        } ?: NativeHttpResponse(0, null, null, NativeHttpFailure.Timeout)
        if (current()?.sharedKey() != session) return SharedCatalogRead.Refused(SharedCatalogFailure.SignInRequired)
        val failure = when {
            response.failure in setOf(NativeHttpFailure.Unauthenticated, NativeHttpFailure.SessionChanged) || response.status == 401 || response.status == 409 -> SharedCatalogFailure.SignInRequired
            response.failure == NativeHttpFailure.AuthorityChanged -> SharedCatalogFailure.Restart
            response.failure == NativeHttpFailure.Denied || response.status == 403 || response.status == 404 -> SharedCatalogFailure.Denied
            response.failure != null -> SharedCatalogFailure.Unavailable
            // The session header is required on success, including empty pages.
            response.status == 200 && response.actualSession != session.session -> SharedCatalogFailure.Protocol
            else -> null
        }
        if (failure != null) {
            if (failure == SharedCatalogFailure.SignInRequired) invalidateSession(session.session)
            return SharedCatalogRead.Refused(failure)
        }
        val classified = classifyCatalogResponse(CatalogHttpResponse(response.status, response.contentType,
            response.body?.encodeToByteArray() ?: byteArrayOf()), cursor != null, serializer, map)
        return when (classified) {
            is PublicCatalogRead.Ready -> SharedCatalogRead.Ready(classified.value)
            PublicCatalogRead.InvalidCursor -> SharedCatalogRead.Refused(SharedCatalogFailure.Restart)
            PublicCatalogRead.NotFound -> SharedCatalogRead.Refused(SharedCatalogFailure.Denied)
            is PublicCatalogRead.AccessFailure -> SharedCatalogRead.Refused(SharedCatalogFailure.SignInRequired)
            is PublicCatalogRead.Protocol -> SharedCatalogRead.Refused(SharedCatalogFailure.Protocol)
            is PublicCatalogRead.Unavailable, PublicCatalogRead.Transport ->
                SharedCatalogRead.Refused(SharedCatalogFailure.Unavailable)
        }
    }
}

/** Closed GET plan; shared requests cannot reach auth, public media or another origin. */
internal fun sharedCatalogReadPath(route: String): Boolean {
    val id = "[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}"
    val page = "\\?limit=20(&cursor=lc1_[0-9a-f]{64})?"
    return route.matches(Regex("/shared/works$page")) ||
        route.matches(Regex("/shared/works/$id")) ||
        route.matches(Regex("/shared/works/$id/units$page"))
}
