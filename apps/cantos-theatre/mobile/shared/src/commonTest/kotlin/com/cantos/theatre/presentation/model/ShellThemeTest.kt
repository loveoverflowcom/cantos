package com.cantos.theatre.presentation.model

import kotlin.test.Test
import kotlin.test.assertEquals

class ShellThemeTest {
    @Test
    fun the_cycle_visits_every_mode_once_and_returns_to_the_selected_mode() {
        val transitions = listOf(
            ShellThemeMode.Dark to ShellThemeMode.Light,
            ShellThemeMode.Light to ShellThemeMode.System,
            ShellThemeMode.System to ShellThemeMode.Dark,
        )
        transitions.forEach { (current, next) ->
            assertEquals(next, current.cycle())
            assertEquals(current, current.cycle().cycle().cycle())
        }
    }

    @Test
    fun only_system_follows_the_host_without_losing_the_selected_mode() {
        for (hostDark in listOf(false, true)) {
            assertEquals(ShellTheme.Light, ShellThemeMode.Light.resolve(hostDark))
            assertEquals(ShellTheme.Dark, ShellThemeMode.Dark.resolve(hostDark))
            assertEquals(if (hostDark) ShellTheme.Dark else ShellTheme.Light, ShellThemeMode.System.resolve(hostDark))
        }
        assertEquals(ShellThemeMode.Dark, ShellThemeMode.System.cycle())
    }

    @Test
    fun the_accessible_theme_name_keeps_current_mode_and_next_action_in_both_locales() {
        for ((locale, expected) in listOf(
            ShellLocale.Vietnamese to "Hệ thống · Chuyển sang giao diện tối",
            ShellLocale.English to "System · Switch to dark appearance",
        )) {
            assertEquals(expected, ShellCopy(locale).themeAction(ShellThemeMode.System))
        }
    }
}
