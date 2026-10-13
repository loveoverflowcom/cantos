package com.cantos.theatre.domain.identity

/** Native protocol facts only. A completed code callback is never an authenticated account. */
enum class LoginFailure { InvalidConfiguration, InvalidEntropy, InvalidCallback, Expired, Replay, Cancelled, Exhausted }

class LoginSecret private constructor(internal val value: String) {
    override fun toString(): String = "LoginSecret([redacted])"
    companion object {
        fun parse(value: String): LoginSecret? = value.takeIf {
            it.length in 43..128 && it.all { c -> c.isLetterOrDigit() && c.code < 128 || c in "-._~" }
        }?.let(::LoginSecret)
    }
}

class NativeConfiguration private constructor(val issuer: String, val redirect: String) {
    val client: String = "cantos-theatre-native"
    companion object {
        /** Operator-owned exact URI. No runtime client/scopes or Studio secret. */
        fun create(issuer: String, redirect: String): NativeConfiguration? {
            val validIssuer = Regex("https://[a-z0-9.-]+(?::[0-9]+)?/oauth2/openid/cantos-theatre-native").matches(issuer)
            return if (validIssuer && redirect == "com.cantos.theatre:/oauth2/callback") NativeConfiguration(issuer, redirect) else null
        }
    }
}

class NativeAttempt private constructor(
    val configuration: NativeConfiguration,
    internal val state: LoginSecret,
    internal val nonce: LoginSecret,
    internal val verifier: LoginSecret,
    val challenge: String,
    internal val startedAt: Long,
) {
    override fun toString(): String = "NativeAttempt([redacted])"
    companion object {
        /** S256 must be computed by IdentityCrypto from this verifier before construction. */
        internal fun create(configuration: NativeConfiguration, state: LoginSecret, nonce: LoginSecret, verifier: LoginSecret, challenge: String, now: Long): NativeAttempt? =
            if (now >= 0 && Regex("[A-Za-z0-9_-]{43}").matches(challenge)) NativeAttempt(configuration, state, nonce, verifier, challenge, now) else null
    }
}

sealed interface NativeLoginState {
    data object Idle : NativeLoginState
    class Waiting internal constructor(val generation: Long, internal val attempt: NativeAttempt) : NativeLoginState
    class Exchanging internal constructor(val generation: Long) : NativeLoginState
    data class Closed(val reason: LoginFailure) : NativeLoginState
}

/** Callback parser must preserve the exact base URI and reject duplicate query parameters. */
class NativeCallback(val redirect: String, internal val state: String, internal val code: String, val hasError: Boolean) {
    override fun toString(): String = "NativeCallback([redacted])"
}
class CodeExchange internal constructor(val generation: Long, internal val attempt: NativeAttempt, internal val code: String) {
    override fun toString(): String = "CodeExchange([redacted])"
    val client: String get() = attempt.configuration.client
    val redirect: String get() = attempt.configuration.redirect
    fun verifierForExchange(): String = attempt.verifier.value
    fun nonceForVerification(): String = attempt.nonce.value
    fun codeForExchange(): String = code
}
sealed interface CallbackOutcome {
    class Exchange(val state: NativeLoginState.Exchanging, val request: CodeExchange) : CallbackOutcome
    data class Refused(val reason: LoginFailure) : CallbackOutcome
}

fun consumeNativeCallback(state: NativeLoginState, callback: NativeCallback, now: Long): CallbackOutcome = when (state) {
    NativeLoginState.Idle -> CallbackOutcome.Refused(LoginFailure.Replay)
    is NativeLoginState.Exchanging -> CallbackOutcome.Refused(LoginFailure.Replay)
    is NativeLoginState.Closed -> CallbackOutcome.Refused(state.reason)
    is NativeLoginState.Waiting -> {
        val attempt = state.attempt
        when {
            now < attempt.startedAt || now - attempt.startedAt >= 300 -> CallbackOutcome.Refused(LoginFailure.Expired)
            callback.redirect != attempt.configuration.redirect || callback.state != attempt.state.value -> CallbackOutcome.Refused(LoginFailure.InvalidCallback)
            callback.hasError -> CallbackOutcome.Refused(LoginFailure.Cancelled)
            callback.code.isEmpty() || callback.code.length > 4096 || callback.code.any { it.code < 33 || it.code > 126 } -> CallbackOutcome.Refused(LoginFailure.InvalidCallback)
            else -> CallbackOutcome.Exchange(NativeLoginState.Exchanging(state.generation), CodeExchange(state.generation, attempt, callback.code))
        }
    }
}
