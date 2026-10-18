package com.cantos.theatre.ports

import com.cantos.theatre.domain.identity.NativeGrant

/** One atomic secure-store value, including recovery intent before a one-use effect. */
sealed interface NativeCredentialRecord {
    class Active(val grant: NativeGrant) : NativeCredentialRecord {
        override fun toString(): String = "NativeCredentialRecord.Active([redacted])"
    }
    class Refreshing(val grant: NativeGrant) : NativeCredentialRecord {
        override fun toString(): String = "NativeCredentialRecord.Refreshing([redacted])"
    }
    class Flow(val flow: String, val state: String, val verifier: String) : NativeCredentialRecord {
        override fun toString(): String = "NativeCredentialRecord.Flow([redacted])"
    }
    /** Unbound old token: discard locally; never send it to a possibly changed API origin. */
    class Legacy(val accessToken: String) : NativeCredentialRecord {
        override fun toString(): String = "NativeCredentialRecord.Legacy([redacted])"
    }
    /** Revocation completed; a crash before clearing the logout barrier can finish locally. */
    data object Cleared : NativeCredentialRecord
}

sealed interface NativeJournalRead {
    data object Missing : NativeJournalRead
    class Found(val record: NativeCredentialRecord) : NativeJournalRead {
        override fun toString(): String = "NativeJournalRead.Found([redacted])"
    }
    data class Failed(val reason: CredentialFailure) : NativeJournalRead
}

/** Implementations replace and read back the entire record through the platform secure store. */
interface NativeCredentialJournal {
    fun read(): NativeJournalRead
    fun write(record: NativeCredentialRecord): CredentialFailure?
    fun clear(): CredentialFailure?
}
