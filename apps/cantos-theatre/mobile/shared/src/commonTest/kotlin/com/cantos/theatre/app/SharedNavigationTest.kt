package com.cantos.theatre.app

import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import kotlin.test.Test
import kotlin.test.assertEquals

class SharedNavigationTest {
    @Test fun shared_login_return_keeps_only_the_neutral_work_locator() {
        val route = TheatreRoute.SharedWork(PublicCatalogSamples.workId(1))
        val selected = reduceNavigation(NavigationState(listOf(TheatreRoute.Library)), NavigationEvent.Navigate(route))
        val login = reduceNavigation(selected, NavigationEvent.OpenLogin)
        assertEquals(listOf(TheatreRoute.Library, route, TheatreRoute.Login), login.stack)
        assertEquals(selected, reduceNavigation(login, NavigationEvent.AuthCompleted))
        assertEquals(selected, reduceNavigation(login, NavigationEvent.Back))
        assertEquals(listOf(TheatreRoute.Library), reduceNavigation(selected, NavigationEvent.Back).stack)
    }
}
