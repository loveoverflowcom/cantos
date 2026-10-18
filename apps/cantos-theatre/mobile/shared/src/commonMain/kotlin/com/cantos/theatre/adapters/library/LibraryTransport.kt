package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.ContinuePage
import com.cantos.theatre.adapters.library.wire.HistoryPage
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.SavedPage
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.ports.LocalLibraryIntent

/** Private HTTP protocol consumed by the native sync adapter, never a domain/application port. */
class LibraryAuthorization(val account: String, val session: String, val bearer: String) {
    override fun toString() = "LibraryAuthorization([redacted])"
}
sealed interface LibraryResult<out T> {
    data class Value<T>(val value: T) : LibraryResult<T>
    data class Failed(val code: String, val retryable: Boolean, val unknownOutcome: Boolean = false) : LibraryResult<Nothing>
}
interface LibraryApi {
    suspend fun continuePage(auth: LibraryAuthorization, cursor: String? = null): LibraryResult<ContinuePage>
    suspend fun savedPage(auth: LibraryAuthorization, cursor: String? = null): LibraryResult<SavedPage>
    suspend fun historyPage(auth: LibraryAuthorization, cursor: String? = null): LibraryResult<HistoryPage>
    suspend fun saved(auth: LibraryAuthorization, work: String): LibraryResult<SavedRecord>
    suspend fun progress(auth: LibraryAuthorization, publication: String): LibraryResult<MutationState.Progress>
    suspend fun mutate(auth: LibraryAuthorization, intent: LocalLibraryIntent): LibraryResult<Receipt>
    suspend fun receipt(auth: LibraryAuthorization, operation: String): LibraryResult<Receipt>
}
