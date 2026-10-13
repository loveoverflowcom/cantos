package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.CallbackOutcome
import com.cantos.theatre.domain.identity.LoginFailure
import com.cantos.theatre.domain.identity.NativeAttempt
import com.cantos.theatre.domain.identity.NativeCallback
import com.cantos.theatre.domain.identity.NativeConfiguration
import com.cantos.theatre.domain.identity.NativeLoginState
import com.cantos.theatre.domain.identity.consumeNativeCallback
import com.cantos.theatre.ports.IdentityCrypto
import com.cantos.theatre.ports.SystemBrowser

/** One owner on the app dispatcher. Process restart drops pending codes; login starts fresh. */
class NativeLoginController(private val crypto: IdentityCrypto, private val browser: SystemBrowser) {
    var state: NativeLoginState = NativeLoginState.Idle
        private set
    private var generation: Long = 0

    fun begin(configuration: NativeConfiguration, authorizationEndpoint: String, now: Long): LoginFailure? {
        cancel()
        if (generation == Long.MAX_VALUE) return close(LoginFailure.Exhausted)
        generation += 1
        // Endpoint must be same exact issuer origin. Discovery validation belongs to transport.
        val origin = configuration.issuer.substringBefore("/oauth2/openid/")
        if (authorizationEndpoint != "$origin/ui/oauth2") return close(LoginFailure.InvalidConfiguration)
        val stateSecret = crypto.secret() ?: return close(LoginFailure.InvalidEntropy)
        val nonce = crypto.secret() ?: return close(LoginFailure.InvalidEntropy)
        val verifier = crypto.secret() ?: return close(LoginFailure.InvalidEntropy)
        val challenge = crypto.challenge(verifier) ?: return close(LoginFailure.InvalidEntropy)
        val attempt = NativeAttempt.create(configuration, stateSecret, nonce, verifier, challenge, now) ?: return close(LoginFailure.InvalidEntropy)
        state = NativeLoginState.Waiting(generation, attempt)
        // Pinned ASCII values; redirect uses the one encoded exact registered URI.
        val url = "$authorizationEndpoint?client_id=${configuration.client}&response_type=code&scope=openid" +
            "&redirect_uri=com.cantos.theatre%3A%2Foauth2%2Fcallback&state=${stateSecret.value}&nonce=${nonce.value}" +
            "&code_challenge_method=S256&code_challenge=$challenge"
        if (!browser.open(url)) return close(LoginFailure.Cancelled)
        return null
    }
    fun callback(callback: NativeCallback, now: Long): CallbackOutcome {
        val result = consumeNativeCallback(state, callback, now)
        state = when (result) {
            is CallbackOutcome.Exchange -> result.state
            is CallbackOutcome.Refused -> if (state is NativeLoginState.Exchanging ||
                (state is NativeLoginState.Waiting && result.reason == LoginFailure.InvalidCallback)) state else NativeLoginState.Closed(result.reason)
        }
        return result
    }
    fun acceptsExchange(generation: Long): Boolean = (state as? NativeLoginState.Exchanging)?.generation == generation
    fun cancel() { browser.cancel(); state = NativeLoginState.Closed(LoginFailure.Cancelled) }
    private fun close(reason: LoginFailure): LoginFailure { state = NativeLoginState.Closed(reason); return reason }
}
