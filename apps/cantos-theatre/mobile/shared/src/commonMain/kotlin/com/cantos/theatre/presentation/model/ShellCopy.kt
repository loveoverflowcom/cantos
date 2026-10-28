package com.cantos.theatre.presentation.model

import com.cantos.theatre.presentation.model.generated.LiveKey
import com.cantos.theatre.presentation.model.generated.ShellKey
import com.cantos.theatre.presentation.model.generated.ShellMessage

typealias CopyKey = ShellKey

/** Generated text resources; numeric formatting stays with its explicit compatibility owner. */
class ShellCopy(private val locale: ShellLocale, private val live: Boolean = false) {
    private val legacy = LegacyShellCopy(locale, live)

    fun text(key: CopyKey): String = when {
        live && key == CopyKey.LibrarySession -> LiveKey.LibrarySession.text(locale)
        live && key == CopyKey.HistoryHelp -> LiveKey.HistoryHelp.text(locale)
        else -> key.text(locale)
    }

    fun text(message: ShellMessage): String = message.text(locale)

    fun text(key: LegacyCopyKey, vararg parameters: Pair<String, String>): String =
        legacy.text(key, *parameters)

    fun themeName(theme: ShellThemeMode): String = text(when (theme) {
        ShellThemeMode.Light -> CopyKey.Light
        ShellThemeMode.Dark -> CopyKey.Dark
        ShellThemeMode.System -> CopyKey.System
    })

    fun themeAction(theme: ShellThemeMode): String = text(ShellMessage.ThemeCycle(
        mode = themeName(theme),
        next = text(when (theme) {
            ShellThemeMode.Dark -> CopyKey.SwitchToLight
            ShellThemeMode.Light -> CopyKey.SwitchToSystem
            ShellThemeMode.System -> CopyKey.SwitchToDark
        }),
    ))

    fun speed(percent: Int): String = legacy.speed(percent)

    fun spokenTime(seconds: Int): String {
        val minutes = seconds / 60
        val remainder = seconds % 60
        return text(ShellMessage.SecondsSpoken(
            minutes = text(if (minutes == 1) LegacyCopyKey.MinuteOne else LegacyCopyKey.MinutesOther, "count" to minutes.toString()),
            seconds = text(if (remainder == 1) LegacyCopyKey.SecondOne else LegacyCopyKey.SecondsOther, "count" to remainder.toString()),
        ))
    }
}

/** Existing clock contract; not an ICU4X/native formatter qualification. */
fun clockTime(seconds: Int): String = "${seconds / 60}:${(seconds % 60).toString().padStart(2, '0')}"
