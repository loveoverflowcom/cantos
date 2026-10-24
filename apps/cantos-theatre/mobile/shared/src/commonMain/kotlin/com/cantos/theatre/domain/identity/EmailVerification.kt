package com.cantos.theatre.domain.identity

/**
 * The signup email action handler this build claims (ADR 0048 stage 1): exact HTTPS origin and
 * path. The parser compares against this pinned value, never against whatever URL arrived, because
 * an explicit intent can deliver any URL to the exported Activity without App Link verification.
 */
const val ProductionVerifyEmailHandler: String = "https://cantos.cc/verify-email"

/**
 * One Firebase `VERIFY_EMAIL` action captured from the mailed link. It lives only in process RAM:
 * never in an Intent after capture, saved state, navigation, a log or a derived string. The
 * locators are public; Identity checks them against the consumed action, so the link alone grants
 * nothing. Same rules as the Web `ProofLink::from_firebase_query`.
 */
class VerificationLink private constructor(
    internal val operationId: String,
    internal val proofId: String,
    internal val oobCode: String,
) {
    override fun toString(): String = "VerificationLink([redacted])"

    companion object {
        /** Far above a real link (oobCode ≤ 4096 plus an encoded continuation); bounds the work. */
        const val MAX_LINK_CHARS: Int = 12_288
        private val QueryKeys = setOf("mode", "oobCode", "continueUrl", "apiKey", "lang")

        /** Whether [link] addresses [handler] at all. Anything else is not this app's link. */
        fun claims(link: String, handler: String): Boolean =
            link == handler || link.startsWith("$handler?") || link.startsWith("$handler#")

        /**
         * Exactly the Firebase keys, once each; `mode=verifyEmail`; a bounded oobCode; and a
         * `continueUrl` that is the same pinned [handler] with exactly `operation` and `proof` as
         * 32 lowercase hex each. A fragment anywhere, a malformed escape or an unknown key refuses.
         */
        fun parse(link: String, handler: String): VerificationLink? {
            if (link.length > MAX_LINK_CHARS || !handler.startsWith("https://")) return null
            val fields = formFields(link.removePrefixOrNull("$handler?") ?: return null, QueryKeys) ?: return null
            if (fields["mode"] != "verifyEmail") return null
            val code = fields["oobCode"] ?: return null
            if (code.isEmpty() || code.length > 4096 || !code.all { it.isAsciiLetterOrDigit() || it in "-_." }) return null
            val continuation = fields["continueUrl"]?.removePrefixOrNull("$handler?") ?: return null
            val locators = formFields(continuation, setOf("operation", "proof")) ?: return null
            val operation = locators["operation"] ?: return null
            val proof = locators["proof"] ?: return null
            return if (lowerHex(operation, 32) && lowerHex(proof, 32)) VerificationLink(operation, proof, code) else null
        }
    }
}

private fun String.removePrefixOrNull(prefix: String): String? = if (startsWith(prefix)) substring(prefix.length) else null

private fun Char.isAsciiLetterOrDigit(): Boolean = this in 'a'..'z' || this in 'A'..'Z' || this in '0'..'9'

private fun lowerHex(value: String, length: Int): Boolean =
    value.length == length && value.all { it in '0'..'9' || it in 'a'..'f' }

/**
 * `application/x-www-form-urlencoded` pairs, like the Web parser: empty segments are skipped, a key
 * without `=` has an empty value. Unlike it, a malformed escape or invalid UTF-8 refuses instead of
 * being replaced. Every key must be in [allowed] and appear once.
 */
private fun formFields(query: String, allowed: Set<String>): Map<String, String>? {
    if ('#' in query) return null
    val fields = mutableMapOf<String, String>()
    for (part in query.split('&')) {
        if (part.isEmpty()) continue
        val separator = part.indexOf('=')
        val key = formDecode(if (separator < 0) part else part.substring(0, separator)) ?: return null
        val value = if (separator < 0) "" else formDecode(part.substring(separator + 1)) ?: return null
        if (key !in allowed || fields.put(key, value) != null) return null
    }
    return fields
}

private fun formDecode(raw: String): String? {
    val bytes = ArrayList<Byte>(raw.length)
    var index = 0
    while (index < raw.length) {
        val char = raw[index]
        when {
            char == '+' -> bytes += ' '.code.toByte()
            char == '%' -> {
                if (index + 2 >= raw.length) return null
                val high = raw[index + 1].digitToIntOrNull(16) ?: return null
                val low = raw[index + 2].digitToIntOrNull(16) ?: return null
                bytes += (high * 16 + low).toByte()
                index += 2
            }
            char.code < 0x80 -> bytes += char.code.toByte()
            else -> bytes.addAll(char.toString().encodeToByteArray().toList())
        }
        index += 1
    }
    return try {
        bytes.toByteArray().decodeToString(throwOnInvalidSequence = true)
    } catch (_: CharacterCodingException) {
        null
    }
}

const val MIN_PASSWORD_SCALARS: Int = 8
const val MAX_PASSWORD_SCALARS: Int = 128
const val MAX_PASSWORD_BYTES: Int = 512

/** Client hint only; the server's password policy stays authoritative (400 on refusal). */
enum class PasswordIssue { TooShort, TooLong, Mismatch }

/** Unicode scalars, not UTF-16 units, as the server counts them. Nothing is trimmed. */
fun passwordIssue(password: String, repeat: String): PasswordIssue? {
    val scalars = password.codePointCount()
    return when {
        scalars < MIN_PASSWORD_SCALARS -> PasswordIssue.TooShort
        scalars > MAX_PASSWORD_SCALARS || password.encodeToByteArray().size > MAX_PASSWORD_BYTES -> PasswordIssue.TooLong
        password != repeat -> PasswordIssue.Mismatch
        else -> null
    }
}

private fun String.codePointCount(): Int {
    var count = 0
    var index = 0
    while (index < length) {
        index += if (this[index].isHighSurrogate() && index + 1 < length && this[index + 1].isLowSurrogate()) 2 else 1
        count += 1
    }
    return count
}

/** An account exists, created by this request or by an identical earlier one. Never a session. */
enum class CompletionStatus { Completed, Replayed }

/**
 * Definite refusals ([Invalid], [Refused], [Forbidden], [NotConfigured]) had no effect.
 * [RateLimited], [Unavailable] and [UnknownOutcome] may have committed: only the identical request
 * is ever repeated.
 */
sealed interface CompletionFailure {
    /** 400: the password or request shape was refused. Nothing happened; the link is intact. */
    data object Invalid : CompletionFailure
    /** 401: the link is expired, spent, or does not match a replayed request. */
    data object Refused : CompletionFailure
    /** 403: this client is not admitted. */
    data object Forbidden : CompletionFailure
    /** 404: Identity is not configured. */
    data object NotConfigured : CompletionFailure
    /** 429 with the server's bounded Retry-After, if it sent one. */
    data class RateLimited(val retryAfterSeconds: Long?) : CompletionFailure
    /** 503: the effect is unknown. */
    data object Unavailable : CompletionFailure
    /** Network loss, deadline or an unreadable answer: the effect is unknown. */
    data object UnknownOutcome : CompletionFailure
}

private val CompletionFailure.retainsRequest: Boolean
    get() = this is CompletionFailure.RateLimited || this == CompletionFailure.Unavailable || this == CompletionFailure.UnknownOutcome

sealed interface VerificationPhase {
    /** No link is held. */
    data object Idle : VerificationPhase
    /** A link addressed this app but could not be read. Nothing is held. */
    data object LinkRefused : VerificationPhase
    /** A link was captured. Opening it alone never reaches the API. */
    data object AwaitingConfirmation : VerificationPhase
    data object Completing : VerificationPhase
    /** The identical request is retained and may be sent again, never a new one. */
    data class CompleteRetry(val failure: CompletionFailure) : VerificationPhase
    /** The API refused outright; [CompletionFailure.Invalid] keeps the link for a new password. */
    data class CompleteRefused(val failure: CompletionFailure) : VerificationPhase
    data class Completed(val status: CompletionStatus) : VerificationPhase
}

/**
 * The exact completion request: built once from the owner's explicit confirmation, retained
 * unchanged for an identical retry, dropped on success or a definite refusal.
 */
class SignupCompletionRequest internal constructor(
    val operationId: String,
    val proofId: String,
    val firebaseOobCode: String,
    val password: String,
) {
    override fun toString(): String = "SignupCompletionRequest([redacted])"
}

/** A request this state issued; [issued] fences its answer against later states. */
class IssuedCompletion internal constructor(val issued: Long, val request: SignupCompletionRequest) {
    override fun toString(): String = "IssuedCompletion($issued, [redacted])"
}

/**
 * RAM-only completion of an emailed signup in the app. A pure value: every transition returns a
 * new state, and a refused transition returns this one. No state here authenticates a listener or
 * touches the active session; completion creates an account and never a session.
 */
class EmailVerificationState private constructor(
    val phase: VerificationPhase,
    /** Controlled-input mirrors. They exist only while the owner is typing a password. */
    val password: String,
    val repeat: String,
    val confirmed: Boolean,
    /** Advanced by every issue and every clear, so late answers of older requests are inert. */
    val epoch: Long,
    private val link: VerificationLink?,
    private val retained: SignupCompletionRequest?,
    /** Earliest instant (ms) the retained request may be sent again after a 429. */
    private val holdUntilMs: Long,
) {
    override fun toString(): String = "EmailVerificationState($phase, [redacted])"

    private fun with(
        phase: VerificationPhase = this.phase, password: String = this.password, repeat: String = this.repeat,
        confirmed: Boolean = this.confirmed, epoch: Long = this.epoch, link: VerificationLink? = this.link,
        retained: SignupCompletionRequest? = this.retained, holdUntilMs: Long = this.holdUntilMs,
    ) = EmailVerificationState(phase, password, repeat, confirmed, epoch, link, retained, holdUntilMs)

    private val canEditPassword: Boolean get() = phase == VerificationPhase.AwaitingConfirmation && link != null

    val passwordIssue: PasswordIssue? get() = passwordIssue(password, repeat)

    /** A captured link, a policy-shaped matching password and the explicit confirmation, together. */
    val canConfirm: Boolean get() = canEditPassword && confirmed && passwordIssue == null

    /** Whole seconds (rounded up) until the retained request may be sent again, if a 429 holds it. */
    fun retryWaitSeconds(nowMs: Long): Long? {
        val remaining = holdUntilMs - nowMs
        return if (phase is VerificationPhase.CompleteRetry && remaining > 0) (remaining + 999) / 1000 else null
    }

    fun editPassword(value: String) = if (canEditPassword) with(password = value, confirmed = false) else this
    fun editRepeat(value: String) = if (canEditPassword) with(repeat = value, confirmed = false) else this
    /** The owner's affirmation that this mailbox creates this account with this password. */
    fun confirm(value: Boolean) = if (canEditPassword) with(confirmed = value) else this

    /** Forgets every secret and fences every outstanding answer. It cannot undo a created account. */
    fun clear(): EmailVerificationState = Initial.with(epoch = epoch + 1)

    /** One pending completion at a time: a new link while one is in flight is refused. */
    fun capture(link: VerificationLink): EmailVerificationState =
        if (phase == VerificationPhase.Completing) this
        else clear().with(phase = VerificationPhase.AwaitingConfirmation, link = link)

    fun linkRefused(): EmailVerificationState =
        if (phase == VerificationPhase.Completing) this else clear().with(phase = VerificationPhase.LinkRefused)

    /** Only the explicit confirmation builds the request; the editable password copies are cleared before any I/O. */
    fun beginComplete(): Pair<EmailVerificationState, IssuedCompletion>? {
        val captured = link
        if (!canConfirm || captured == null) return null
        val request = SignupCompletionRequest(captured.operationId, captured.proofId, captured.oobCode, password)
        val next = with(phase = VerificationPhase.Completing, password = "", repeat = "", confirmed = false,
            epoch = epoch + 1, retained = request)
        return next to IssuedCompletion(next.epoch, request)
    }

    /** Repeats the identical retained request after an unknown outcome, never before a 429's Retry-After. */
    fun beginRetry(nowMs: Long): Pair<EmailVerificationState, IssuedCompletion>? {
        val request = retained
        if (phase !is VerificationPhase.CompleteRetry || request == null || nowMs < holdUntilMs) return null
        val next = with(phase = VerificationPhase.Completing, epoch = epoch + 1)
        return next to IssuedCompletion(next.epoch, request)
    }

    /** Settles only the newest request; an older or fenced answer returns this state unchanged. */
    fun finish(issued: Long, result: CompletionResult, nowMs: Long): EmailVerificationState {
        if (issued != epoch || phase != VerificationPhase.Completing) return this
        return when (result) {
            is CompletionResult.Done -> Initial.with(phase = VerificationPhase.Completed(result.status), epoch = epoch)
            is CompletionResult.Failed -> {
                val failure = result.failure
                when {
                    failure.retainsRequest -> with(
                        phase = VerificationPhase.CompleteRetry(failure),
                        holdUntilMs = ((failure as? CompletionFailure.RateLimited)?.retryAfterSeconds ?: 0)
                            .coerceIn(0, Long.MAX_VALUE / 1000).let { nowMs.saturatingAdd(it * 1000) },
                    )
                    // Nothing happened: the link is intact and the owner may choose another password.
                    failure == CompletionFailure.Invalid -> with(phase = VerificationPhase.CompleteRefused(failure), retained = null)
                    else -> Initial.with(phase = VerificationPhase.CompleteRefused(failure), epoch = epoch)
                }
            }
        }
    }

    /** After a 400 the same link accepts a fresh, explicit attempt with a new password. */
    fun editAfterRefusal(): EmailVerificationState =
        if (phase == VerificationPhase.CompleteRefused(CompletionFailure.Invalid) && link != null) {
            with(phase = VerificationPhase.AwaitingConfirmation)
        } else this

    internal val retainedRequest: SignupCompletionRequest? get() = retained

    companion object {
        val Initial = EmailVerificationState(VerificationPhase.Idle, "", "", false, 0, null, null, 0)
    }
}

private fun Long.saturatingAdd(other: Long): Long = if (this > Long.MAX_VALUE - other) Long.MAX_VALUE else this + other

/** What the completion port answered for one issued request. */
sealed interface CompletionResult {
    data class Done(val status: CompletionStatus) : CompletionResult
    data class Failed(val failure: CompletionFailure) : CompletionResult
}
