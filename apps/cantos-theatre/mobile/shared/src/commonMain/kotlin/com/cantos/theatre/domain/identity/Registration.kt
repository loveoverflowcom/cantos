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

/** RAM-only action capability. Import never consumes it; explicit confirmation does. */
class RegistrationAction private constructor(internal val operation: String, internal val code: String) {
    override fun toString(): String = "RegistrationAction([redacted])"
    companion object {
        fun parse(link: String, actionOrigin: String): RegistrationResult<RegistrationAction> =
            candidate(link, actionOrigin)?.let { RegistrationResult.Accepted(it) } ?: RegistrationResult.Refused(RegistrationFailure.InvalidAction)

        private fun candidate(link: String, actionOrigin: String): RegistrationAction? {
            if (!NativeClientConfiguration.validApiOrigin(actionOrigin)) return null
            if (link.length > 8192 || link.any { it.code !in 33..126 } || '#' in link) return null
            val prefix = "$actionOrigin/?"
            if (!link.startsWith(prefix)) return null
            val fields = mutableMapOf<String, String>()
            for (part in link.removePrefix(prefix).split('&')) {
                val name = part.substringBefore('=')
                if ('=' !in part || name !in setOf("mode", "oobCode", "continueUrl", "apiKey", "lang") || name in fields) return null
                fields[name] = decodeRegistrationParameter(part.substringAfter('=')) ?: return null
            }
            if (fields["mode"] != "verifyEmail") return null
            val continuation = fields["continueUrl"] ?: return null
            val continuationPrefix = "$actionOrigin/?firebaseEnrollment="
            if (!continuation.startsWith(continuationPrefix)) return null
            val operation = continuation.removePrefix(continuationPrefix)
            if (!Regex("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").matches(operation) ||
                operation == "00000000-0000-0000-0000-000000000000") return null
            val code = fields["oobCode"] ?: return null
            if (code.isEmpty() || code.length > 4096 || code.any { it.code !in 33..126 }) return null
            return RegistrationAction(operation, code)
        }
    }
}

private fun decodeRegistrationParameter(raw: String): String? = buildString {
    var index = 0
    while (index < raw.length) {
        val char = raw[index]
        if (char == '%') {
            if (index + 2 >= raw.length) return null
            val byte = raw.substring(index + 1, index + 3).toIntOrNull(16) ?: return null
            if (byte !in 33..126) return null
            append(byte.toChar())
            index += 3
        } else {
            if (char == '+' || char.code !in 33..126) return null
            append(char)
            index += 1
        }
    }
}

class RegistrationSetup private constructor(val login: String, internal val url: String) {
    override fun toString(): String = "RegistrationSetup([redacted])"
    companion object {
        fun parse(login: String, url: String, issuerOrigin: String): RegistrationResult<RegistrationSetup> =
            candidate(login, url, issuerOrigin)?.let { RegistrationResult.Accepted(it) } ?: RegistrationResult.Refused(RegistrationFailure.Protocol)

        private fun candidate(login: String, url: String, issuerOrigin: String): RegistrationSetup? {
            if (!NativeClientConfiguration.validApiOrigin(issuerOrigin)) return null
            if (!Regex("[a-zA-Z0-9_-]{1,128}").matches(login) || url.length > 8192 ||
                url.any { it.code !in 33..126 } || '#' in url) return null
            val prefix = "$issuerOrigin/ui/reset?token="
            if (!url.startsWith(prefix)) return null
            val token = url.removePrefix(prefix)
            if (token.isEmpty() || token.any { !it.isLetterOrDigit() && it !in "-._~*%" }) return null
            val decoded = decodeRegistrationParameter(token) ?: return null
            if (decoded.length !in 1..4096) return null
            return RegistrationSetup(login, url)
        }
    }
}

enum class RegistrationFailure { InvalidEmail, InvalidAction, Disabled, Unavailable, Denied, Protocol, ProofUnavailable, StartUnknownOutcome, UnknownOutcome, Browser }

sealed interface RegistrationPhase {
    data object Idle : RegistrationPhase
    data object Checking : RegistrationPhase
    data object Editing : RegistrationPhase
    data object Submitting : RegistrationPhase
    data object Waiting : RegistrationPhase
    class Confirmable(val action: RegistrationAction) : RegistrationPhase
    class Verifying(val action: RegistrationAction) : RegistrationPhase
    class SetupReady(val setup: RegistrationSetup) : RegistrationPhase
    data object CredentialSetup : RegistrationPhase
    data object Cancelling : RegistrationPhase
    data object Cancelled : RegistrationPhase
    data class Failed(val reason: RegistrationFailure) : RegistrationPhase
}

class RegistrationState(val email: String = "", val phase: RegistrationPhase = RegistrationPhase.Idle, val firstParty: Boolean = false) {
    override fun toString(): String = "RegistrationState([redacted])"
}

/** Back keeps the email/intent, forgets capabilities and never pretends to undo a remote write. */
fun suspendRegistration(state: RegistrationState): RegistrationState = RegistrationState(state.email, when (state.phase) {
    RegistrationPhase.Submitting -> RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome)
    is RegistrationPhase.Verifying, RegistrationPhase.Cancelling,
    is RegistrationPhase.SetupReady -> RegistrationPhase.Failed(RegistrationFailure.UnknownOutcome)
    is RegistrationPhase.Confirmable -> RegistrationPhase.Waiting
    RegistrationPhase.Checking -> RegistrationPhase.Idle
    RegistrationPhase.Idle, RegistrationPhase.Editing, RegistrationPhase.Waiting, RegistrationPhase.CredentialSetup,
    RegistrationPhase.Cancelled, is RegistrationPhase.Failed -> state.phase
}, state.firstParty)

enum class RegistrationCommand { Submit, Confirm, Cancel, OpenSetup }

/** Eligibility is pure; duplicate clicks cannot dispatch a second consuming command. */
fun RegistrationState.allows(command: RegistrationCommand): Boolean = when (command) {
    RegistrationCommand.Submit -> phase == RegistrationPhase.Editing
    RegistrationCommand.Confirm -> phase is RegistrationPhase.Confirmable
    RegistrationCommand.Cancel -> phase is RegistrationPhase.Confirmable
    RegistrationCommand.OpenSetup -> phase is RegistrationPhase.SetupReady
}
