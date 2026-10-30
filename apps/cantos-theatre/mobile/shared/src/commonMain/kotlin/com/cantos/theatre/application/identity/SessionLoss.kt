package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.ports.NativeApiFailure

/** Evidence from the existing account owner, never inferred from an arbitrary HTTP 401. */
enum class SessionLossReason { Deadline, SessionRejected }

/** Nonsecret process epoch. Only the account owner binds this to a confirmed terminal loss. */
@ConsistentCopyVisibility
data class ConfirmedSessionLoss internal constructor(val epoch: Long, val reason: SessionLossReason)

/** Explicit account changes retain their existing player fence; passive checks cannot stop public audio. */
internal enum class AccountChange { Explicit, Passive }

/** A refused session/refresh is not a failed password attempt. */
internal fun NativeApiFailure.accountFailure(): NativeAccountFailure = when (this) {
    NativeApiFailure.Unauthorized -> NativeAccountFailure.Expired
    NativeApiFailure.Protocol -> NativeAccountFailure.Protocol
    NativeApiFailure.Denied, NativeApiFailure.Unavailable, NativeApiFailure.UnknownOutcome,
    NativeApiFailure.SessionChanged -> NativeAccountFailure.Api
}

/** A clock/shape anomaly is not expiry. Only a spent immutable deadline establishes local loss. */
internal fun NativeGrant.deadlineFailure(now: Long): NativeAccountFailure =
    if (now >= absoluteExpiresAt || now >= refreshExpiresAt) NativeAccountFailure.Expired
    else NativeAccountFailure.Protocol
