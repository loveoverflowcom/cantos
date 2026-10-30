package com.cantos.theatre.ports

import com.cantos.theatre.domain.identity.AccountPresentation
import com.cantos.theatre.domain.identity.NativeGrant

/** A separately authenticated display read; a display failure never changes login authority. */
interface AccountPresentationApi {
    suspend fun presentation(previous: NativeGrant): IdentityV1Result<AccountPresentation>
}
