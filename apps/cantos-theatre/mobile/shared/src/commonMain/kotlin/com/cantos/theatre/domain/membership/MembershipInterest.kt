package com.cantos.theatre.domain.membership

enum class MembershipMode { Unavailable, Fixture }
enum class PlansOrigin { Intro, Account, AccountRegistered, Received }
enum class PlansStatus { Unpublished, Loading, Empty, Unavailable, Failed }
enum class SubmissionStatus { Idle, Pending, Failed, Unknown, Received }

sealed interface MembershipScreen {
    data object Intro : MembershipScreen
    data object Form : MembershipScreen
    data object Received : MembershipScreen
    data object AccountRegistered : MembershipScreen
    data object Account : MembershipScreen
    data class Plans(val origin: PlansOrigin) : MembershipScreen
}

enum class MembershipFieldError { NameRequired, NameTooLong, NameInvalid, EmailRequired, EmailTooLong, EmailInvalid }
enum class MembershipEditRefusal { NotOnForm, Pending, OutcomeUnknown, AlreadyReceived }
enum class MembershipFinishOutcome { Received, Failed, Unknown }

sealed interface MembershipSubmitRefusal {
    data object NotOnForm : MembershipSubmitRefusal
    data class InvalidFields(val errors: List<MembershipFieldError>) : MembershipSubmitRefusal
    data object Unavailable : MembershipSubmitRefusal
    data object AlreadyPending : MembershipSubmitRefusal
    data object AlreadyReceived : MembershipSubmitRefusal
    data object SequenceExhausted : MembershipSubmitRefusal
}

/** Raw candidate text stays in this mounted preview session, never navigation or saved state. */
data class MembershipDraft(val name: String = "", val email: String = "") {
    override fun toString(): String = "MembershipDraft(redacted)"
}

@ConsistentCopyVisibility
data class RegistrationPayload private constructor(val name: String, val email: String) {
    companion object {
        internal fun validate(draft: MembershipDraft): PayloadValidation {
            val errors = validateMembershipDraft(draft)
            return if (errors.isEmpty()) PayloadValidation.Accepted(RegistrationPayload(draft.name.trimMembership(), draft.email.trimMembership()))
            else PayloadValidation.Refused(errors)
        }
    }

    override fun toString(): String = "RegistrationPayload(redacted)"
}

internal sealed interface PayloadValidation {
    data class Accepted(val payload: RegistrationPayload) : PayloadValidation
    data class Refused(val errors: List<MembershipFieldError>) : PayloadValidation
}

/** Opaque local intent and per-dispatch fence; these are not a backend operation contract. */
@ConsistentCopyVisibility
data class SubmissionAttempt private constructor(
    val operationId: ULong,
    val requestId: ULong,
    val payload: RegistrationPayload,
) {
    companion object {
        internal fun create(operationId: ULong, requestId: ULong, payload: RegistrationPayload): SubmissionAttempt =
            SubmissionAttempt(operationId, requestId, payload)
    }

    override fun toString(): String = "SubmissionAttempt(operationId=$operationId, requestId=$requestId, payload=redacted)"
}

/** Immutable preview state; no public construction/copy path can forge a receipt. */
@ConsistentCopyVisibility
data class MembershipState private constructor(
    val mode: MembershipMode,
    val screen: MembershipScreen = MembershipScreen.Intro,
    val draft: MembershipDraft = MembershipDraft(),
    val submission: SubmissionStatus = SubmissionStatus.Idle,
    val fieldErrors: List<MembershipFieldError> = emptyList(),
    val plansStatus: PlansStatus = PlansStatus.Unpublished,
    val receipt: SubmissionAttempt? = null,
    private val sequence: ULong = 0u,
    private val intent: SubmissionAttempt? = null,
    private val activeRequest: ULong? = null,
) {
    val pendingAttempt: SubmissionAttempt? get() = intent?.takeIf { activeRequest == it.requestId }

    companion object {
        fun unavailable(): MembershipState = MembershipState(MembershipMode.Unavailable)
        fun fixture(): MembershipState = MembershipState(MembershipMode.Fixture)
    }

    fun reduce(event: MembershipEvent): MembershipTransition = when (event) {
        MembershipEvent.OpenIntro -> changed(fenceRequest().copy(screen = MembershipScreen.Intro))
        MembershipEvent.ShowReceived -> if (receipt != null) changed(fenceRequest().copy(screen = MembershipScreen.Received)) else ignored()
        MembershipEvent.OpenForm -> when {
            screen != MembershipScreen.Intro -> ignored()
            receipt != null -> MembershipTransition(copy(screen = MembershipScreen.Received), MembershipResult.Ignored)
            else -> changed(copy(screen = MembershipScreen.Form))
        }
        is MembershipEvent.SetName -> edit(event.value, name = true)
        is MembershipEvent.SetEmail -> edit(event.value, name = false)
        MembershipEvent.Submit -> beginSubmission()
        is MembershipEvent.Finish -> finish(event.attempt, event.outcome)
        MembershipEvent.OpenAccount -> changed(fenceRequest().copy(screen = MembershipScreen.Account))
        MembershipEvent.ShowAccountRegisteredFixture -> if (mode == MembershipMode.Fixture) {
            changed(fenceRequest().copy(screen = MembershipScreen.AccountRegistered))
        } else ignored()
        MembershipEvent.OpenPlans -> openPlans()
        MembershipEvent.OpenPlansDeepLink -> changed(fenceRequest().copy(screen = MembershipScreen.Plans(PlansOrigin.Account)))
        is MembershipEvent.SetPlansStatus -> changed(copy(plansStatus = event.status))
        MembershipEvent.Back -> changed(fenceRequest().copy(screen = when (val current = screen) {
            MembershipScreen.Form -> MembershipScreen.Intro
            is MembershipScreen.Plans -> when (current.origin) {
                PlansOrigin.Intro -> MembershipScreen.Intro
                PlansOrigin.Account -> MembershipScreen.Account
                PlansOrigin.AccountRegistered -> MembershipScreen.AccountRegistered
                PlansOrigin.Received -> MembershipScreen.Received
            }
            MembershipScreen.Intro, MembershipScreen.Received, MembershipScreen.AccountRegistered,
            MembershipScreen.Account -> MembershipScreen.Account
        }))
        MembershipEvent.ClearSession -> changed(MembershipState(mode = mode, sequence = sequence))
    }

    private fun edit(value: String, name: Boolean): MembershipTransition {
        val refusal = when {
            screen != MembershipScreen.Form -> MembershipEditRefusal.NotOnForm
            submission == SubmissionStatus.Pending -> MembershipEditRefusal.Pending
            submission == SubmissionStatus.Unknown -> MembershipEditRefusal.OutcomeUnknown
            submission == SubmissionStatus.Received -> MembershipEditRefusal.AlreadyReceived
            else -> null
        }
        if (refusal != null) return MembershipTransition(this, MembershipResult.EditRefused(refusal))
        val nextDraft = if (name) draft.copy(name = value) else draft.copy(email = value)
        return changed(copy(draft = nextDraft, fieldErrors = emptyList(), submission = SubmissionStatus.Idle, intent = null))
    }

    private fun beginSubmission(): MembershipTransition {
        if (screen != MembershipScreen.Form) return refused(MembershipSubmitRefusal.NotOnForm)
        when (submission) {
            SubmissionStatus.Pending -> return refused(MembershipSubmitRefusal.AlreadyPending)
            SubmissionStatus.Received -> return refused(MembershipSubmitRefusal.AlreadyReceived)
            SubmissionStatus.Idle, SubmissionStatus.Failed, SubmissionStatus.Unknown -> Unit
        }
        val payload = when (val validation = RegistrationPayload.validate(draft)) {
            is PayloadValidation.Accepted -> validation.payload
            is PayloadValidation.Refused -> return copy(fieldErrors = validation.errors).refused(
                MembershipSubmitRefusal.InvalidFields(validation.errors),
            )
        }
        if (mode == MembershipMode.Unavailable) return copy(submission = SubmissionStatus.Failed, fieldErrors = emptyList())
            .refused(MembershipSubmitRefusal.Unavailable)
        if (sequence == ULong.MAX_VALUE) return refused(MembershipSubmitRefusal.SequenceExhausted)
        val request = sequence + 1u
        val attempt = SubmissionAttempt.create(intent?.operationId ?: request, request, intent?.payload ?: payload)
        return MembershipTransition(
            copy(sequence = request, intent = attempt, activeRequest = request, submission = SubmissionStatus.Pending, fieldErrors = emptyList()),
            MembershipResult.Started(attempt),
        )
    }

    private fun finish(attempt: SubmissionAttempt, outcome: MembershipFinishOutcome): MembershipTransition {
        if (mode != MembershipMode.Fixture || screen != MembershipScreen.Form || activeRequest != attempt.requestId || intent != attempt) {
            return ignored()
        }
        return changed(when (outcome) {
            MembershipFinishOutcome.Failed -> copy(activeRequest = null, submission = SubmissionStatus.Failed)
            MembershipFinishOutcome.Unknown -> copy(activeRequest = null, submission = SubmissionStatus.Unknown)
            MembershipFinishOutcome.Received -> copy(activeRequest = null, receipt = attempt, screen = MembershipScreen.Received, submission = SubmissionStatus.Received)
        })
    }

    private fun openPlans(): MembershipTransition {
        val origin = when (screen) {
            MembershipScreen.Intro -> PlansOrigin.Intro
            MembershipScreen.Account -> PlansOrigin.Account
            MembershipScreen.AccountRegistered -> PlansOrigin.AccountRegistered
            MembershipScreen.Received -> PlansOrigin.Received
            MembershipScreen.Form, is MembershipScreen.Plans -> return ignored()
        }
        return changed(copy(screen = MembershipScreen.Plans(origin)))
    }

    private fun fenceRequest(): MembershipState = copy(
        activeRequest = null,
        submission = if (submission == SubmissionStatus.Pending) SubmissionStatus.Unknown else submission,
    )

    private fun changed(next: MembershipState): MembershipTransition = MembershipTransition(next, MembershipResult.Changed)
    private fun ignored(): MembershipTransition = MembershipTransition(this, MembershipResult.Ignored)
    private fun refused(reason: MembershipSubmitRefusal): MembershipTransition = MembershipTransition(this, MembershipResult.SubmitRefused(reason))

    override fun toString(): String = "MembershipState(mode=$mode, screen=$screen, submission=$submission, privateFields=redacted)"
}

sealed interface MembershipEvent {
    data object OpenIntro : MembershipEvent
    data object ShowReceived : MembershipEvent
    data object OpenForm : MembershipEvent
    data class SetName(val value: String) : MembershipEvent { override fun toString(): String = "SetName(redacted)" }
    data class SetEmail(val value: String) : MembershipEvent { override fun toString(): String = "SetEmail(redacted)" }
    data object Submit : MembershipEvent
    data class Finish(val attempt: SubmissionAttempt, val outcome: MembershipFinishOutcome) : MembershipEvent
    data object OpenAccount : MembershipEvent
    data object ShowAccountRegisteredFixture : MembershipEvent
    data object OpenPlans : MembershipEvent
    data object OpenPlansDeepLink : MembershipEvent
    data class SetPlansStatus(val status: PlansStatus) : MembershipEvent
    data object Back : MembershipEvent
    data object ClearSession : MembershipEvent
}

sealed interface MembershipResult {
    data object Changed : MembershipResult
    data object Ignored : MembershipResult
    data class EditRefused(val reason: MembershipEditRefusal) : MembershipResult
    data class SubmitRefused(val reason: MembershipSubmitRefusal) : MembershipResult
    data class Started(val attempt: SubmissionAttempt) : MembershipResult
}

data class MembershipTransition(val state: MembershipState, val result: MembershipResult)

/** Modest shared preview contact hints, not an RFC mailbox or identity-provider policy. */
fun validateMembershipDraft(draft: MembershipDraft): List<MembershipFieldError> = buildList {
    val name = draft.name.trimMembership()
    val email = draft.email.trimMembership()
    if (name.isEmpty()) add(MembershipFieldError.NameRequired)
    else if (name.scalarLength() > 120) add(MembershipFieldError.NameTooLong)
    else if (name.any(Char::isISOControl)) add(MembershipFieldError.NameInvalid)
    if (email.isEmpty()) add(MembershipFieldError.EmailRequired)
    else if (email.scalarLength() > 254) add(MembershipFieldError.EmailTooLong)
    else {
        val at = email.indexOf('@')
        if (email.any { it.isISOControl() || it.isMembershipWhitespace() } || at <= 0 || at == email.lastIndex || email.indexOf('@', at + 1) >= 0) {
            add(MembershipFieldError.EmailInvalid)
        }
    }
}

/** Unicode White_Space matches the Rust preview validator, including NEL and NBSP. */
private fun Char.isMembershipWhitespace(): Boolean = when (code) {
    in 0x0009..0x000D, 0x0020, 0x0085, 0x00A0, 0x1680, in 0x2000..0x200A,
    0x2028, 0x2029, 0x202F, 0x205F, 0x3000 -> true
    else -> false
}

private fun String.trimMembership(): String = trim(Char::isMembershipWhitespace)

/** UTF-16 surrogate pairs count once, matching Rust char count for valid Unicode text. */
private fun String.scalarLength(): Int {
    var length = 0
    var index = 0
    while (index < this.length) {
        val char = this[index]
        index += if (char.isHighSurrogate() && index + 1 < this.length && this[index + 1].isLowSurrogate()) 2 else 1
        length += 1
    }
    return length
}
