package com.cantos.theatre.features.shell.presentation

import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellWorkId
import kotlin.test.Test
import kotlin.test.assertEquals

class TopBarRoutesTest {
    @Test
    fun only_top_level_destinations_carry_the_official_brand_mark() {
        val expected = mapOf(
            ShellRoute.Discovery to true,
            ShellRoute.Search to true,
            ShellRoute.Library to true,
            ShellRoute.History to true,
            ShellRoute.Work(ShellWorkId("w")) to false,
            ShellRoute.Player to false,
            ShellRoute.Queue to false,
            ShellRoute.Account to false,
            ShellRoute.Settings to false,
            ShellRoute.Unavailable to false,
        )
        expected.forEach { (route, brand) -> assertEquals(brand, topBarShowsBrandMark(route), route.toString()) }
    }
}
