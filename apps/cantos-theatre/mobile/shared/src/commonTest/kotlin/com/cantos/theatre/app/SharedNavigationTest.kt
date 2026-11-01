package com.cantos.theatre.app

import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import kotlin.test.Test
import kotlin.test.assertEquals

class SharedNavigationTest {
    @Test fun shared_login_return_keeps_only_the_neutral_work_locator() {
        val route = TheatreRoute.SharedWork(PublicCatalogSamples.workId(1))
        val selected = reduceNavigation(NavigationState(), NavigationEvent.Navigate(route))
        val login = reduceNavigation(selected, NavigationEvent.OpenLogin)
        assertEquals(listOf(TheatreRoute.Landing, route, TheatreRoute.Login), login.stack)
        assertEquals(listOf(TheatreRoute.Discovery, route), reduceNavigation(login, NavigationEvent.AuthCompleted, NavigationAdmission.SignedIn).stack)
        assertEquals(selected, reduceNavigation(login, NavigationEvent.Back))
        assertEquals(listOf(TheatreRoute.Landing), reduceNavigation(selected, NavigationEvent.Back).stack)
    }
}
