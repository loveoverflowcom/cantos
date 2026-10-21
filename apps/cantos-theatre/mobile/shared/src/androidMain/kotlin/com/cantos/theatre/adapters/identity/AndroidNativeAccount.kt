package com.cantos.theatre.adapters.identity

import android.content.Context
import com.cantos.theatre.adapters.http.NativeIdentityV1Client
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.RegistrationController
import com.cantos.theatre.domain.identity.NativeIdentityV1Configuration
import com.cantos.theatre.domain.identity.NativeIdentityV1ConfigurationResult
import kotlinx.coroutines.CoroutineScope

/** Caller retains one process owner. V1 has no provider, browser or callback authority. */
fun createAndroidNativeAccount(scope: CoroutineScope, context: Context, apiOrigin: String?): NativeAccountController? =
    (NativeIdentityV1Configuration.resolve(apiOrigin) as? NativeIdentityV1ConfigurationResult.Available)?.configuration?.let { configuration ->
        val now = { System.currentTimeMillis() / 1000 }
        NativeAccountController(scope, configuration,
            NativeIdentityV1Client(configuration, AndroidNativeHttp(scope, configuration.apiOrigin), now),
            SecureNativeCredentialJournal(AndroidCredentialStore(context), configuration),
            AndroidLogoutBarrier(context), now)
    }

/** Signup initiates mail only; proof and password completion belong to Web. */
fun createAndroidRegistration(scope: CoroutineScope, apiOrigin: String?): RegistrationController? =
    (NativeIdentityV1Configuration.resolve(apiOrigin) as? NativeIdentityV1ConfigurationResult.Available)?.configuration?.let { configuration ->
        RegistrationController(scope, NativeIdentityV1Client(configuration,
            AndroidNativeHttp(scope, configuration.apiOrigin)) { System.currentTimeMillis() / 1000 })
    }
