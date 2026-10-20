package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.BookmarkPage
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.PresentedContinuePage
import com.cantos.theatre.adapters.library.wire.PresentedHistoryPage
import com.cantos.theatre.adapters.library.wire.PresentedSavedPage
import com.cantos.theatre.adapters.library.wire.Receipt
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
    /** Page reads always opt in to `presentation=1`: every item carries its required, nullable metadata. */
    suspend fun continuePage(auth: LibraryAuthorization, cursor: String? = null): LibraryResult<PresentedContinuePage>
    suspend fun savedPage(auth: LibraryAuthorization, cursor: String? = null): LibraryResult<PresentedSavedPage>
    suspend fun historyPage(auth: LibraryAuthorization, cursor: String? = null): LibraryResult<PresentedHistoryPage>
    /** Live bookmarks only; every item carries its required, nullable exact-release metadata. */
    suspend fun bookmarkPage(auth: LibraryAuthorization, cursor: String? = null): LibraryResult<BookmarkPage>
    suspend fun saved(auth: LibraryAuthorization, work: String): LibraryResult<SavedRecord>
    suspend fun progress(auth: LibraryAuthorization, publication: String): LibraryResult<MutationState.Progress>
    suspend fun mutate(auth: LibraryAuthorization, intent: LocalLibraryIntent): LibraryResult<Receipt>
    suspend fun receipt(auth: LibraryAuthorization, operation: String): LibraryResult<Receipt>
}
