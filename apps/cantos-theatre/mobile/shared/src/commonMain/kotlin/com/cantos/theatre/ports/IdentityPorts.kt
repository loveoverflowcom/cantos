package com.cantos.theatre.ports

/** OS adapters only; never plaintext preferences, UI saved state or demo memory persistence. */
enum class CredentialFailure { Unavailable, Corrupt, Denied }
sealed interface CredentialRead {
    data object Missing : CredentialRead
    class Found(val value: String) : CredentialRead { override fun toString(): String = "CredentialRead.Found([redacted])" }
    data class Failed(val reason: CredentialFailure) : CredentialRead
}
interface CredentialStore {
    fun read(): CredentialRead
    fun write(value: String): CredentialFailure?
    fun clear(): CredentialFailure?
}
