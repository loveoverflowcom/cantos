package com.cantos.theatre.adapters.identity

import android.content.Context
import com.cantos.theatre.adapters.http.NativeIdentityClient
import com.cantos.theatre.application.identity.NativeAccountController
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import com.cantos.theatre.application.identity.RegistrationController
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.NativeRegistrationHttp
import com.cantos.theatre.ports.RegistrationBrowser
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeIdentityAvailability
import kotlinx.coroutines.CoroutineScope

/** The same shared enrollment owner as Desktop Preview, with an external setup browser. */
fun createAndroidRegistration(scope: CoroutineScope, context: Context, apiOrigin: String?, issuer: String?): RegistrationController? =
    when (val resolved = NativeClientConfiguration.resolveForApplication(apiOrigin, issuer, context.packageName)) {
        is NativeIdentityAvailability.Available -> resolved.configuration.let { config ->
            RegistrationController(scope, NativeRegistrationHttp(ApiOrigin.parse(config.apiOrigin),
                config.identity.issuer.substringBefore("/oauth2/openid/"), AndroidNativeHttp(scope, config.apiOrigin)),
                object : RegistrationBrowser {
                    override fun openSetup(url: String, completion: (Boolean) -> Unit) {
                        val opened = try {
                            context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                            true
                        } catch (_: ActivityNotFoundException) { false }
                          catch (_: SecurityException) { false }
                        completion(opened)
                    }
                })
        }
        is NativeIdentityAvailability.Missing, is NativeIdentityAvailability.Invalid -> null
    }

/** Caller retains one process owner. No credential is accepted before API confirmation. */
fun createAndroidNativeAccount(
    scope: CoroutineScope,
    context: Context,
    apiOrigin: String?,
    issuer: String?,
): NativeAccountController? = when (val resolved = NativeClientConfiguration.resolveForApplication(apiOrigin, issuer, context.packageName)) {
    is NativeIdentityAvailability.Available -> resolved.configuration.let { configuration ->
        val now = { System.currentTimeMillis() / 1000 }
        NativeAccountController(scope, configuration, NativeIdentityClient(configuration, AndroidNativeHttp(scope, configuration.apiOrigin), now),
            AndroidIdentityCrypto(), AndroidSystemBrowser(context.applicationContext),
            SecureNativeCredentialJournal(AndroidCredentialStore(context), configuration.apiOrigin, configuration.identity.issuer),
            AndroidLogoutBarrier(context), now = now)
    }
    is NativeIdentityAvailability.Missing, is NativeIdentityAvailability.Invalid -> null
}
