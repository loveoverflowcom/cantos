package com.cantos.theatre.app

import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeIdentityAvailability
import com.cantos.theatre.adapters.http.NativeIdentityClient
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeCallback
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.CredentialRead
import com.cantos.theatre.ports.CredentialStore
import com.cantos.theatre.ports.IdentityCrypto
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.SystemBrowser
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
    fun cancelBrowser()
    fun readCredential(): IosStoreRead
    fun writeCredential(value: String): String?
    fun clearCredential(): String?
    fun readLogoutBarrier(): String
    fun markLogoutBarrier(): String?
    fun clearLogoutBarrier(): String?
}

class IosNativeIdentityOwner(apiOrigin: String?, issuer: String?, platform: IosIdentityPlatform) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private fun failure(value: String?): CredentialFailure? = when (value) {
        null -> null
        "denied" -> CredentialFailure.Denied
        "corrupt" -> CredentialFailure.Corrupt
        else -> CredentialFailure.Unavailable
    }
    val availability = NativeClientConfiguration.resolve(apiOrigin, issuer)
    val controller = when (val resolved = availability) {
        is NativeIdentityAvailability.Available -> resolved.configuration.let {
            NativeAccountController(scope, it, NativeIdentityClient(it, platform),
                object : IdentityCrypto {
                    override fun secret(): LoginSecret? = platform.secureSecret()?.let(LoginSecret::parse)
                    override fun challenge(verifier: LoginSecret): String? = platform.s256(verifier.value)
                },
                object : SystemBrowser {
                    override fun open(url: String): Boolean = platform.openBrowser(url)
                    override fun cancel() = platform.cancelBrowser()
                },
                object : CredentialStore {
                    override fun read(): CredentialRead {
                        val result = platform.readCredential()
                        val error = failure(result.failure)
                        return if (error != null) CredentialRead.Failed(error) else result.value?.let { value -> CredentialRead.Found(value) } ?: CredentialRead.Missing
                    }
                    override fun write(value: String): CredentialFailure? = failure(platform.writeCredential(value))
                    override fun clear(): CredentialFailure? = failure(platform.clearCredential())
                },
                object : LogoutBarrier {
                    override fun read(): LogoutBarrierRead = when (platform.readLogoutBarrier()) {
                        "clear" -> LogoutBarrierRead.Clear
                        "pending" -> LogoutBarrierRead.Pending
                        else -> LogoutBarrierRead.Unavailable
                    }
                    override fun mark(): CredentialFailure? = failure(platform.markLogoutBarrier())
                    override fun clear(): CredentialFailure? = failure(platform.clearLogoutBarrier())
                }, now = { NSDate().timeIntervalSince1970.toLong() })
        }
        is NativeIdentityAvailability.Missing, is NativeIdentityAvailability.Invalid -> null
    }
    fun callback(state: String, code: String, hasError: Boolean) {
        controller?.callback(NativeCallback("com.cantos.theatre:/oauth2/callback", state, code, hasError))
    }
    fun browserCancelled() { controller?.cancelLogin() }
    fun close() { controller?.close(); scope.cancel() }
}
