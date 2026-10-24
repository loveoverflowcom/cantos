package com.cantos.theatre.adapters.identity

import android.content.Context
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.NativeIdentityV1Client
import com.cantos.theatre.application.identity.EmailVerificationController
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.RegistrationController
import com.cantos.theatre.domain.identity.ProductionVerifyEmailHandler
import kotlinx.coroutines.CoroutineScope

private val now = { System.currentTimeMillis() / 1000 }

/**
 * Caller retains one process owner. The [origin] is the one the host's origin owner admitted
 * (HTTPS; loopback HTTP only in a debug host). V1 has no provider, browser or callback authority.
 */
fun createAndroidNativeAccount(scope: CoroutineScope, context: Context, origin: ApiOrigin): NativeAccountController =
    NativeAccountController(scope, NativeIdentityV1Client(origin, AndroidNativeHttp(scope, origin), now),
        SecureNativeCredentialJournal(AndroidCredentialStore(context), origin), AndroidLogoutBarrier(context), now)

/** Signup initiates mail only; the mailed link completes in the app ([createAndroidVerification]) or on Web. */
fun createAndroidRegistration(scope: CoroutineScope, origin: ApiOrigin): RegistrationController =
    RegistrationController(scope, NativeIdentityV1Client(origin, AndroidNativeHttp(scope, origin), now))

/** Completes a signup from the verified App Link on the pinned handler; anonymous, never a session. */
fun createAndroidVerification(scope: CoroutineScope, origin: ApiOrigin): EmailVerificationController =
    EmailVerificationController(scope, NativeIdentityV1Client(origin, AndroidNativeHttp(scope, origin), now),
        ProductionVerifyEmailHandler, System::currentTimeMillis)
