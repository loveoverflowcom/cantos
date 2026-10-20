package com.cantos.theatre.ports

import com.cantos.theatre.domain.identity.RegistrationAction
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.domain.identity.RegistrationSetup

sealed interface RegistrationCapability {
    class Available(val actionOrigin: String) : RegistrationCapability
    data object Disabled : RegistrationCapability
}

interface RegistrationPort {
    suspend fun configuration(): RegistrationResult<RegistrationCapability>
    suspend fun start(email: RegistrationEmail): RegistrationResult<Unit>
    suspend fun confirm(action: RegistrationAction): RegistrationResult<RegistrationSetup>
    suspend fun cancel(action: RegistrationAction): RegistrationResult<Unit>
}

/** Opens only the capability already validated by the shared HTTP adapter, never a WebView. */
interface RegistrationBrowser {
    fun openSetup(url: String, completion: (Boolean) -> Unit)
}
