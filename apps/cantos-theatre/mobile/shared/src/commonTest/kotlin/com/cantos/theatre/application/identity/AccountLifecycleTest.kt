package com.cantos.theatre.application.identity

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class AccountLifecycleTest {
    private fun session(actor: String, n: Int) = AccountSession.fromServer(actor, "00000000-0000-4000-8000-${n.toString().padStart(12, '0')}")!!
    @Test fun accountSwitchNeverRelabelsIntentAndLateCallbacksCannotMutateB() {
        val owner = AccountLifecycle()
        val operation = "00000000-0000-4000-8000-000000000003"
        owner.enter(session("A", 1))
        val old = owner.fence()!!
        owner.project(old, "private A")
        assertNull(owner.enqueue(operation, "same immutable body"))
        assertFalse((owner.dispatch(operation) as IntentDispatch.Ready).intent.readReceiptFirst)
        owner.leave()
        assertNull(owner.projection)
        owner.enter(session("B", 2))
        assertFalse(owner.project(old, "late A"))
        assertFalse(owner.acknowledge(old, operation))
        assertEquals(IntentDispatch.Refused(AccountFailure.WrongAccount), owner.dispatch(operation))
        owner.enter(session("A", 4))
        val recovered = owner.dispatch(operation) as IntentDispatch.Ready
        assertTrue(recovered.intent.readReceiptFirst)
        assertEquals("same immutable body", recovered.intent.body)
        assertTrue(owner.acknowledge(recovered.fence, operation))
    }
    @Test fun boundedAccountSchedulesRejectEveryPreviousGeneration() {
        for (switches in 1..64) {
            val owner = AccountLifecycle()
            owner.enter(session("A", 1))
            val old = owner.fence()!!
            repeat(switches) { owner.enter(session(if (it % 2 == 0) "B" else "A", it + 2)) }
            assertFalse(owner.accepts(old))
        }
    }
}
