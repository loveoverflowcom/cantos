package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.SharedCatalogHttp
import com.cantos.theatre.adapters.http.sharedKey
import com.cantos.theatre.application.catalog.SharedCatalogController
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.ports.NativeHttpPort
import kotlinx.coroutines.CoroutineScope

/** Borrow the existing Identity authorization coordinator; never establish a second session. */
fun nativeSharedCatalog(
    scope: CoroutineScope,
    origin: ApiOrigin,
    account: NativeAccountController?,
    authorizedHttp: NativeHttpPort,
): SharedCatalogController? = account?.let { owner ->
    val current = { owner.verifiedSession.value?.takeIf(owner::accepts) }
    SharedCatalogController(scope, SharedCatalogHttp(origin, authorizedHttp, owner::revalidate, current)) { current()?.sharedKey() }
}
