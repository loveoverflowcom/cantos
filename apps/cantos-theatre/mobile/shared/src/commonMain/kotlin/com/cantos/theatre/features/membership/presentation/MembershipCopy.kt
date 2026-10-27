package com.cantos.theatre.features.membership.presentation

import com.cantos.theatre.domain.membership.MembershipFieldError
import com.cantos.theatre.domain.membership.PlansStatus
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.generated.MembershipKey

internal class MembershipCopy(private val locale: ShellLocale) {
    fun text(key: MembershipText): String = key.text(locale)
    fun plans(status: PlansStatus): String = text(when (status) {
        PlansStatus.Unpublished -> MembershipText.Unpublished
        PlansStatus.Loading -> MembershipText.PlansLoading
        PlansStatus.Empty -> MembershipText.PlansEmpty
        PlansStatus.Unavailable -> MembershipText.PlansUnavailable
        PlansStatus.Failed -> MembershipText.PlansFailed
    })
    fun error(error: MembershipFieldError): String = text(when (error) {
        MembershipFieldError.NameRequired -> MembershipText.NameRequired
        MembershipFieldError.NameTooLong -> MembershipText.NameTooLong
        MembershipFieldError.NameInvalid -> MembershipText.NameInvalid
        MembershipFieldError.EmailRequired -> MembershipText.EmailRequired
        MembershipFieldError.EmailTooLong -> MembershipText.EmailTooLong
        MembershipFieldError.EmailInvalid -> MembershipText.EmailInvalid
    })
}

internal typealias MembershipText = MembershipKey
