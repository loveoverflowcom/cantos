package com.cantos.theatre.app

import com.cantos.theatre.adapters.identity.SecureNativeCredentialJournal
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.RegistrationController
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.CredentialRead
import com.cantos.theatre.ports.CredentialStore
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeHttpPort
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import platform.Foundation.NSDate

/** Swift-only foreign boundary; all outcomes are translated before entering common owners. */
class IosStoreRead(val value: String?, val failure: String?) {
    override fun toString(): String = "IosStoreRead([redacted])"
}
interface IosIdentityPlatform : NativeHttpPort {
    fun secureSecret(): String?
    fun s256(verifier: String): String?
    fun openBrowser(url: String): Boolean
    fun openSetup(url: String, completion: (Boolean) -> Unit)
    fun cancelBrowser()
    fun readCredential(): IosStoreRead
    fun writeCredential(value: String): String?
    fun clearCredential(): String?
    fun readLogoutBarrier(): String
    fun markLogoutBarrier(): String?
    fun clearLogoutBarrier(): String?
}

class IosNativeIdentityOwner(apiOrigin: String?, platform: IosIdentityPlatform) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private fun failure(value: String?): CredentialFailure? = when (value) {
        null -> null
        "denied" -> CredentialFailure.Denied
        "corrupt" -> CredentialFailure.Corrupt
        else -> CredentialFailure.Unavailable
    }
    val availability = com.cantos.theatre.domain.identity.NativeIdentityV1Configuration.resolve(apiOrigin)
    private val configuration = (availability as? com.cantos.theatre.domain.identity.NativeIdentityV1ConfigurationResult.Available)?.configuration
    private val now = { NSDate().timeIntervalSince1970.toLong() }
    private val api = configuration?.let { com.cantos.theatre.adapters.http.NativeIdentityV1Client(it, platform, now) }
    val registration = api?.let { RegistrationController(scope, it) }
    val controller = configuration?.let { config ->
        NativeAccountController(scope, config, requireNotNull(api),
            SecureNativeCredentialJournal(object : CredentialStore {
                override fun read(): CredentialRead {
                    val result = platform.readCredential()
                    val error = failure(result.failure)
                    return if (error != null) CredentialRead.Failed(error) else result.value?.let { value -> CredentialRead.Found(value) } ?: CredentialRead.Missing
                }
                override fun write(value: String): CredentialFailure? = failure(platform.writeCredential(value))
                override fun clear(): CredentialFailure? = failure(platform.clearCredential())
            }, config),
            object : LogoutBarrier {
                override fun read(): LogoutBarrierRead = when (platform.readLogoutBarrier()) {
                    "clear" -> LogoutBarrierRead.Clear
                    "pending" -> LogoutBarrierRead.Pending
                    else -> LogoutBarrierRead.Unavailable
                }
                override fun mark(): CredentialFailure? = failure(platform.markLogoutBarrier())
                override fun clear(): CredentialFailure? = failure(platform.clearLogoutBarrier())
            }, now)
    }
    // Old ASWebAuthenticationSession callbacks have no v1 authority, including cancellation.
    fun callback(state: String, code: String, hasError: Boolean) = Unit
    fun browserCancelled() = Unit
    fun close() { registration?.close(); controller?.close(); scope.cancel() }
}
