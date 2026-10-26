package com.cantos.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class ThemeModeTest {
    /**
     * `UiPreferences` carries a resolved [Theme] and has no mode or host-fact field, so a nested theme
     * cannot re-resolve System: that guarantee is type-enforced, not something this table can observe.
     */
    @Test
    fun explicit_modes_ignore_the_host_and_system_follows_it() {
        val table = listOf(
            Triple(ThemeMode.Light, false, Theme.Light),
            Triple(ThemeMode.Light, true, Theme.Light),
            Triple(ThemeMode.Dark, false, Theme.Dark),
            Triple(ThemeMode.Dark, true, Theme.Dark),
            Triple(ThemeMode.System, false, Theme.Light),
            Triple(ThemeMode.System, true, Theme.Dark),
        )
        table.forEach { (mode, systemIsDark, expected) ->
            assertEquals(expected, mode.resolve(systemIsDark), "$mode with a ${if (systemIsDark) "dark" else "light"} host")
        }
        assertEquals(ThemeMode.entries.size * 2, table.size, "every mode is checked against both host facts")
    }

    @Test
    fun the_modes_are_exactly_light_dark_and_system_in_chip_order() {
        assertEquals(listOf(ThemeMode.Light, ThemeMode.Dark, ThemeMode.System), ThemeMode.entries)
    }

    @Test
    fun cycle_steps_dark_light_system_dark() {
        assertEquals(ThemeMode.Light, ThemeMode.Dark.cycle())
        assertEquals(ThemeMode.System, ThemeMode.Light.cycle())
        assertEquals(ThemeMode.Dark, ThemeMode.System.cycle())
    }
}
