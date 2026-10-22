package com.cantos.theatre.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class NetworkRecoveryTrackerTest {
    private val a = 1L
    private val b = 2L
    private val c = 3L

    private fun defaultTracker() = NetworkRecoveryTracker(networkReportingForSdk(34))

    @Test
    fun registrationKindFollowsTheFirstSdkThatHasADefaultRegistration() {
        assertEquals(NetworkReporting.AllNetworks, networkReportingForSdk(23))
        assertEquals(NetworkReporting.CurrentDefault, networkReportingForSdk(24))
        assertEquals(NetworkReporting.CurrentDefault, networkReportingForSdk(35))
        assertTrue(networkCapabilitiesNeedProbe(23))
        assertTrue(networkCapabilitiesNeedProbe(25))
        assertFalse(networkCapabilitiesNeedProbe(26))
    }

    @Test
    fun replacedDefaultNetworkIsNotKeptAsProofOfConnectivity() {
        val tracker = defaultTracker()
        assertTrue(tracker.capabilities(a, usable = true))
        // The platform stops reporting A once B is the default and may never report A lost.
        assertFalse(tracker.capabilities(b, usable = true))
        tracker.lost(b)
        // No default remains; the next usable network is exactly one opportunity.
        assertTrue(tracker.capabilities(c, usable = true))
        assertFalse(tracker.capabilities(c, usable = true))
    }

    @Test
    fun allNetworksRegistrationWouldHaveKeptTheReplacedNetwork() {
        // Contrast case: the old set semantics are only correct when every loss is reported.
        val tracker = NetworkRecoveryTracker(NetworkReporting.AllNetworks)
        assertTrue(tracker.capabilities(a, usable = true))
        assertFalse(tracker.capabilities(b, usable = true))
        tracker.lost(b)
        assertFalse(tracker.capabilities(c, usable = true), "A is still reported usable")
        val reported = NetworkRecoveryTracker(NetworkReporting.AllNetworks)
        reported.capabilities(a, usable = true)
        reported.capabilities(b, usable = true)
        reported.lost(b)
        reported.lost(a)
        assertTrue(reported.capabilities(c, usable = true))
    }

    @Test
    fun duplicateCapabilitiesDoNotRepeatTheOpportunity() {
        val tracker = defaultTracker()
        assertTrue(tracker.capabilities(a, usable = true))
        repeat(3) { assertFalse(tracker.capabilities(a, usable = true)) }
    }

    @Test
    fun staleLossOfAReplacedDefaultKeepsTheCurrentDefault() {
        val tracker = defaultTracker()
        tracker.capabilities(a, usable = true)
        tracker.capabilities(b, usable = true)
        tracker.lost(a)
        assertFalse(tracker.capabilities(b, usable = true), "B is still the usable default")
        tracker.lost(b)
        assertTrue(tracker.capabilities(a, usable = true))
    }

    @Test
    fun defaultThatIsNotYetValidatedMakesTheValidationTheOpportunity() {
        val tracker = defaultTracker()
        assertTrue(tracker.capabilities(a, usable = true))
        assertFalse(tracker.capabilities(b, usable = false))
        assertTrue(tracker.capabilities(b, usable = true))
    }

    @Test
    fun capabilityLossDoesNotNeedALossCallback() {
        val tracker = defaultTracker()
        assertTrue(tracker.capabilities(a, usable = true))
        assertFalse(tracker.capabilities(a, usable = false))
        assertTrue(tracker.capabilities(a, usable = true))
    }

    @Test
    fun anyUsableNetworkCountsForAnAllNetworksRegistration() {
        val tracker = NetworkRecoveryTracker(networkReportingForSdk(23))
        assertTrue(tracker.capabilities(a, usable = true))
        assertFalse(tracker.capabilities(b, usable = true))
        tracker.lost(a)
        assertFalse(tracker.capabilities(b, usable = true))
        tracker.lost(b)
        assertTrue(tracker.capabilities(a, usable = true))
    }

    @Test
    fun eventsAfterCloseHaveNoEffect() {
        val tracker = defaultTracker()
        tracker.close()
        assertFalse(tracker.capabilities(a, usable = true))
        tracker.lost(a)
        assertFalse(tracker.capabilities(a, usable = true))
    }
}
