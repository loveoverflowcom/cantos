package com.cantos.theatre.application.catalog

import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicUnitPage
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkPage
import com.cantos.theatre.ports.SharedCatalogFailure
import com.cantos.theatre.ports.SharedCatalogPort
import com.cantos.theatre.ports.SharedCatalogRead
import com.cantos.theatre.ports.SharedSessionKey
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class SharedCatalogControllerTest {
    private val a = SharedSessionKey("actor-a", "session-a", 1)
    private val b = SharedSessionKey("actor-b", "session-b", 2)
    private val work = PublicCatalogSamples.work(1)
    private val page = PublicCatalogSamples.worksPage(listOf(work))

    private class Port : SharedCatalogPort {
        var reads = 0
        var works: suspend (SharedSessionKey, CatalogCursor?) -> SharedCatalogRead<PublicWorkPage> = { _, _ ->
            SharedCatalogRead.Ready(PublicCatalogSamples.worksPage(emptyList()))
        }
        var detail: suspend () -> SharedCatalogRead<PublicWork> = { SharedCatalogRead.Ready(PublicCatalogSamples.work(1)) }
        var units: suspend (CatalogCursor?) -> SharedCatalogRead<PublicUnitPage> = {
            val work = PublicCatalogSamples.work(1)
            SharedCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, listOf(PublicCatalogSamples.unit(1))))
        }
        override suspend fun works(session: SharedSessionKey, cursor: CatalogCursor?): SharedCatalogRead<PublicWorkPage> {
            reads += 1
            return works.invoke(session, cursor)
        }
        override suspend fun work(session: SharedSessionKey, id: PublicWorkId): SharedCatalogRead<PublicWork> {
            reads += 1
            return detail()
        }
        override suspend fun units(session: SharedSessionKey, work: PublicWork, cursor: CatalogCursor?): SharedCatalogRead<PublicUnitPage> {
            reads += 1
            return units.invoke(cursor)
        }
    }

    @Test fun guest_locator_survives_login_without_requesting_or_exposing_metadata() = runTest {
        var session: SharedSessionKey? = null
        val port = Port()
        val owner = SharedCatalogController(backgroundScope, port) { session }
        owner.show(work.id)
        runCurrent()
        assertEquals(SharedCatalogPhase.SignInRequired, owner.state.value.phase)
        assertEquals(work.id, owner.state.value.locator)
        assertNull(owner.state.value.work)
        assertEquals(0, port.reads)
        session = a
        owner.sessionChanged()
        runCurrent()
        assertEquals(SharedCatalogPhase.Ready, owner.state.value.phase)
        assertEquals(work.id, owner.state.value.work?.id)
        assertEquals(2, port.reads)
        owner.close()
    }

    @Test fun account_a_to_b_to_a_never_accepts_a_late_first_incarnation() = runTest {
        var session: SharedSessionKey? = a
        val delayed = CompletableDeferred<SharedCatalogRead<PublicWorkPage>>()
        val port = Port().also { it.works = { _, _ -> withContext(NonCancellable) { delayed.await() } } }
        val owner = SharedCatalogController(backgroundScope, port) { session }
        owner.show()
        runCurrent()
        session = b
        owner.sessionChanged()
        assertTrue(owner.state.value.works.isEmpty())
        session = a.copy(epoch = 3)
        owner.sessionChanged()
        owner.leave()
        delayed.complete(SharedCatalogRead.Ready(page))
        runCurrent()
        assertEquals(SharedCatalogState(), owner.state.value)
        owner.close()
    }

    @Test fun logout_purges_rows_and_cursor_before_its_async_transport_finishes() = runTest {
        var session: SharedSessionKey? = a
        val port = Port().also { it.works = { _, _ -> SharedCatalogRead.Ready(
            PublicCatalogSamples.worksPage(listOf(work), PublicCatalogSamples.cursor(1))) } }
        val owner = SharedCatalogController(backgroundScope, port) { session }
        owner.show()
        runCurrent()
        assertEquals(listOf(work), owner.state.value.works)
        session = null
        assertEquals(SharedCatalogPhase.SignInRequired, owner.visibleState().phase)
        assertTrue(owner.visibleState().works.isEmpty())
        owner.sessionChanged()
        assertEquals(SharedCatalogState(phase = SharedCatalogPhase.SignInRequired), owner.state.value)
        owner.close()
    }

    @Test fun every_current_refusal_purges_previous_pages_and_private_titles() = runTest {
        for (failure in SharedCatalogFailure.entries) {
            val port = Port().also { it.works = { _, cursor -> if (cursor == null)
                SharedCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(work), PublicCatalogSamples.cursor(1)))
                else SharedCatalogRead.Refused(failure) } }
            val owner = SharedCatalogController(backgroundScope, port) { a }
            owner.show()
            runCurrent()
            owner.more()
            runCurrent()
            assertTrue(owner.state.value.works.isEmpty(), failure.name)
            assertNull(owner.state.value.cursor)
            assertNull(owner.state.value.work)
            assertTrue(owner.state.value.units.isEmpty())
            owner.close()
        }
    }

    @Test fun repeated_cursor_and_duplicate_cross_page_rows_refuse_the_walk() = runTest {
        val cursor = PublicCatalogSamples.cursor(1)
        val port = Port().also { it.works = { _, _ -> SharedCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(work), cursor)) } }
        val owner = SharedCatalogController(backgroundScope, port) { a }
        owner.show()
        runCurrent()
        owner.more()
        runCurrent()
        assertEquals(SharedCatalogPhase.Protocol, owner.state.value.phase)
        assertTrue(owner.state.value.works.isEmpty())
        owner.close()
    }

    @Test fun denial_of_units_never_leaves_work_metadata_or_uses_public_fallback() = runTest {
        val port = Port().also { it.units = { SharedCatalogRead.Refused(SharedCatalogFailure.Denied) } }
        val owner = SharedCatalogController(backgroundScope, port) { a }
        owner.show(work.id)
        runCurrent()
        assertEquals(SharedCatalogPhase.Denied, owner.state.value.phase)
        assertNull(owner.state.value.work)
        assertEquals(2, port.reads)
        owner.close()
    }

    @Test fun changing_route_discards_delayed_detail_success_and_failure() = runTest {
        for (reply in listOf(SharedCatalogRead.Ready(work), SharedCatalogRead.Refused(SharedCatalogFailure.Denied))) {
            val delayed = CompletableDeferred<SharedCatalogRead<PublicWork>>()
            val port = Port().also { it.detail = { withContext(NonCancellable) { delayed.await() } } }
            val owner = SharedCatalogController(backgroundScope, port) { a }
            owner.show(work.id)
            runCurrent()
            owner.leave()
            delayed.complete(reply)
            runCurrent()
            assertEquals(SharedCatalogState(), owner.state.value)
            assertEquals(1, port.reads)
            owner.close()
        }
    }

    @Test fun private_state_and_session_diagnostics_never_print_metadata_or_bearer() {
        val state = SharedCatalogState(a, work.id, SharedCatalogPhase.Ready, listOf(work), work)
        assertEquals("SharedCatalogState([redacted])", state.toString())
        assertEquals("SharedSessionKey([redacted])", a.toString())
    }
}
