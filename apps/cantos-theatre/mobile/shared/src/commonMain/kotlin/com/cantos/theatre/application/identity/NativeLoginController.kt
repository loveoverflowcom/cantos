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
        val origin = configuration.issuer.substringBefore("/oauth2/openid/")
        if (authorizationEndpoint != "$origin/ui/oauth2") { cancel(); return close(LoginFailure.InvalidConfiguration) }
        prepare(configuration, now)?.let { return it }
        val attempt = preparedAttempt() ?: return close(LoginFailure.InvalidConfiguration)
        return openPrepared(authorizationUrl(attempt, authorizationEndpoint))
    }
    fun prepare(configuration: NativeConfiguration, now: Long): LoginFailure? {
        cancel()
        if (generation == Long.MAX_VALUE) return close(LoginFailure.Exhausted)
        generation += 1
        val stateSecret = crypto.secret() ?: return close(LoginFailure.InvalidEntropy)
        val nonce = crypto.secret() ?: return close(LoginFailure.InvalidEntropy)
        val verifier = crypto.secret() ?: return close(LoginFailure.InvalidEntropy)
        val challenge = crypto.challenge(verifier) ?: return close(LoginFailure.InvalidEntropy)
        val attempt = NativeAttempt.create(configuration, stateSecret, nonce, verifier, challenge, now) ?: return close(LoginFailure.InvalidEntropy)
        state = NativeLoginState.Waiting(generation, attempt)
        return null
    }
    internal fun preparedAttempt(): NativeAttempt? = (state as? NativeLoginState.Waiting)?.attempt
    fun openPrepared(url: String): LoginFailure? {
        val attempt = preparedAttempt() ?: return close(LoginFailure.InvalidConfiguration)
        val endpoint = attempt.configuration.issuer.substringBefore("/oauth2/openid/") + "/ui/oauth2"
        if (!validAuthorizationUrl(attempt, endpoint, url)) return close(LoginFailure.InvalidConfiguration)
        if (!browser.open(url)) return close(LoginFailure.Cancelled)
        return null
    }
    private fun authorizationUrl(attempt: NativeAttempt, authorizationEndpoint: String): String {
        // Pinned ASCII values; redirect uses the one encoded exact registered URI.
        return "$authorizationEndpoint?client_id=${attempt.configuration.client}&response_type=code&scope=openid" +
            "&redirect_uri=com.cantos.theatre%3A%2Foauth2%2Fcallback&state=${attempt.state.value}&nonce=${attempt.nonce.value}" +
            "&code_challenge_method=S256&code_challenge=${attempt.challenge}"
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

/** Strict decoded parameter equivalence, independent of provider query ordering. */
internal fun validAuthorizationUrl(attempt: NativeAttempt, endpoint: String, url: String): Boolean {
    if (url.length > 8192 || url.substringBefore('?') != endpoint || '#' in url || '?' !in url) return false
    val pairs = linkedMapOf<String, String>()
    fun decode(value: String): String? {
        val output = StringBuilder()
        var i = 0
        while (i < value.length) {
            val c = value[i]
            when {
                c == '%' -> {
                    if (i + 2 >= value.length) return null
                    val code = value.substring(i + 1, i + 3).toIntOrNull(16) ?: return null
                    if (code !in 32..126) return null
                    output.append(code.toChar()); i += 3
                }
                c == '+' -> { output.append(' '); i += 1 }
                c.code in 33..126 -> { output.append(c); i += 1 }
                else -> return null
            }
        }
        return output.toString()
    }
    for (pair in url.substringAfter('?').split('&')) {
        val name = decode(pair.substringBefore('=')) ?: return false
        if ('=' !in pair || name in pairs) return false
        pairs[name] = decode(pair.substringAfter('=')) ?: return false
    }
    return pairs == mapOf("client_id" to attempt.configuration.client, "response_type" to "code", "scope" to "openid",
        "redirect_uri" to attempt.configuration.redirect, "state" to attempt.state.value, "nonce" to attempt.nonce.value,
        "code_challenge_method" to "S256", "code_challenge" to attempt.challenge)
}
