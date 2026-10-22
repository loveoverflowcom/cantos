package com.cantos.theatre.domain.identity

sealed interface RegistrationResult<out T> {
    class Accepted<T>(val value: T) : RegistrationResult<T>
    data class Refused(val reason: RegistrationFailure) : RegistrationResult<Nothing>
}

/** Client validation helps editing; the API still owns mailbox proof and account admission. */
class RegistrationEmail private constructor(val value: String) {
    override fun toString(): String = "RegistrationEmail([redacted])"
    companion object {
        fun parse(raw: String): RegistrationResult<RegistrationEmail> =
            candidate(raw)?.let { RegistrationResult.Accepted(it) } ?: RegistrationResult.Refused(RegistrationFailure.InvalidEmail)

        private fun candidate(raw: String): RegistrationEmail? {
            val value = raw.trim()
            if (value.length > 254 || value.any { it.code > 127 }) return null
            val parts = value.split('@')
            if (parts.size != 2) return null
            val (local, domain) = parts
            if (local.isEmpty() || local.length > 64 || local.startsWith('.') || local.endsWith('.') ||
                ".." in local || local.any { !it.isLetterOrDigit() && it !in ".!#$%&'*+-/=?^_`{|}~" } ||
                '.' !in domain || domain.length > 253 || domain.split('.').any { label ->
                    label.isEmpty() || label.length > 63 || label.startsWith('-') || label.endsWith('-') ||
                        label.any { !it.isLetterOrDigit() && it != '-' }
                }) return null
            return RegistrationEmail("$local@${domain.lowercase()}")
        }
    }
}

/**
 * Why a signup initiation did not reach the "check your mail" state. Initiation only mails a
 * proof link that Web completes; no outcome here ever grants a session. A 429 is refused before
 * any effect, while a 503, timeout or transport loss leaves the effect unknown.
 */
enum class RegistrationFailure { InvalidEmail, Disabled, Denied, Protocol, RateLimited, StartUnknownOutcome }

sealed interface RegistrationPhase {
    data object Idle : RegistrationPhase
    data object Editing : RegistrationPhase
    data object Submitting : RegistrationPhase
    /** The API acknowledged the initiation; the proof and password are completed on Web. */
    data object Waiting : RegistrationPhase
    data object Cancelled : RegistrationPhase
    data class Failed(val reason: RegistrationFailure) : RegistrationPhase
}

class RegistrationState(val email: String = "", val phase: RegistrationPhase = RegistrationPhase.Idle) {
    override fun toString(): String = "RegistrationState([redacted])"
}

/** Back keeps the email/intent and never pretends to undo a remote write. */
fun suspendRegistration(state: RegistrationState): RegistrationState = RegistrationState(state.email, when (state.phase) {
    RegistrationPhase.Submitting -> RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome)
    RegistrationPhase.Idle, RegistrationPhase.Editing, RegistrationPhase.Waiting,
    RegistrationPhase.Cancelled, is RegistrationPhase.Failed -> state.phase
})

enum class RegistrationCommand { Submit }

/** Eligibility is pure; duplicate clicks cannot dispatch a second signup. */
fun RegistrationState.allows(command: RegistrationCommand): Boolean = when (command) {
    RegistrationCommand.Submit -> phase == RegistrationPhase.Editing
}
