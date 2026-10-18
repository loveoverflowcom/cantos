package com.cantos.theatre.adapters.identity

import android.content.Context
import com.cantos.theatre.adapters.http.NativeIdentityClient
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeIdentityAvailability
import kotlinx.coroutines.CoroutineScope

/** Caller retains one process owner. No credential is accepted before API confirmation. */
fun createAndroidNativeAccount(
    scope: CoroutineScope,
    context: Context,
    apiOrigin: String?,
    issuer: String?,
): NativeAccountController? = when (val resolved = NativeClientConfiguration.resolve(apiOrigin, issuer)) {
    is NativeIdentityAvailability.Available -> resolved.configuration.let { configuration ->
        NativeAccountController(scope, configuration, NativeIdentityClient(configuration, AndroidNativeHttp(scope, configuration.apiOrigin)),
            AndroidIdentityCrypto(), AndroidSystemBrowser(context.applicationContext), AndroidCredentialStore(context),
            AndroidLogoutBarrier(context), now = { System.currentTimeMillis() / 1000 })
    }
    is NativeIdentityAvailability.Missing, is NativeIdentityAvailability.Invalid -> null
}
