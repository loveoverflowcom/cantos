package com.cantos.ui

import com.cantos.ui.generated.CantosIcons
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import org.junit.Test

/** The topbar theme action paints one registry glyph per mode, matching the Web `ThemeCycleButton`. */
class TopbarControlsTest {
    @Test
    fun each_mode_has_its_own_registry_glyph_and_system_is_one_split_sun_not_two_icons() {
        val names = ThemeMode.entries.map(::themeModeIconName)
        assertEquals(3, names.toSet().size)
        names.forEach { name -> assertEquals(1, CantosIcons.icons.count { it.name == name }) }
        assertEquals("theme_system", themeModeIconName(ThemeMode.System))
        assertEquals(1, CantosIcons.paths("theme_system").size)
        assertNotEquals(CantosIcons.paths("dark_mode"), CantosIcons.paths("theme_system"))
        assertNotEquals(CantosIcons.paths("light_mode"), CantosIcons.paths("theme_system"))
    }
}
