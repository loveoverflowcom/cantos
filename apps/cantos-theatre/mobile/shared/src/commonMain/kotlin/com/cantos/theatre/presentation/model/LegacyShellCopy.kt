package com.cantos.theatre.presentation.model

/** Compatibility only: ICU4X Kotlin/native formatting is not qualified. Keep existing output. */
enum class LegacyCopyKey(val vi: String, val en: String, val enOne: String? = null) {
    EpisodesCount("{count} tập · Tiếng Việt", "{count} episodes · Vietnamese", "{count} episode · Vietnamese"),
    ChaptersCount("{count} chương", "{count} chapters", "{count} chapter"),
    EpisodeLabel("Tập {index} · {title}", "Episode {index} · {title}"),
    Duration("{minutes} phút", "{minutes} minutes", "{minutes} minute"),
    Results("{count} tác phẩm", "{count} works", "{count} work"),
    MinuteOne("{count} phút", "{count} minute"),
    MinutesOther("{count} phút", "{count} minutes"),
    SecondOne("{count} giây", "{count} second"),
    SecondsOther("{count} giây", "{count} seconds"),
}

internal class LegacyShellCopy(private val locale: ShellLocale, private val live: Boolean) {
    fun text(key: LegacyCopyKey, vararg parameters: Pair<String, String>): String {
        val quantityName = when (key) {
            LegacyCopyKey.EpisodesCount, LegacyCopyKey.ChaptersCount, LegacyCopyKey.Results -> "count"
            LegacyCopyKey.Duration -> "minutes"
            else -> null
        }
        val one = quantityName != null && parameters.firstOrNull { it.first == quantityName }?.second == "1"
        val template = when (locale) {
            ShellLocale.Vietnamese -> if (live && key == LegacyCopyKey.EpisodesCount) "{count} tập" else key.vi
            ShellLocale.English -> if (live && key == LegacyCopyKey.EpisodesCount) {
                if (one) "{count} episode" else "{count} episodes"
            } else if (one) key.enOne ?: key.en else key.en
        }
        return Regex("\\{([^{}]+)\\}").replace(template) { match ->
            parameters.firstOrNull { it.first == match.groupValues[1] }?.second ?: match.value
        }
    }

    fun speed(percent: Int): String {
        val whole = percent / 100
        val fraction = (percent % 100).toString().padStart(2, '0').trimEnd('0')
        val separator = when (locale) {
            ShellLocale.Vietnamese -> ","
            ShellLocale.English -> "."
        }
        return if (fraction.isEmpty()) "$whole×" else "$whole$separator$fraction×"
    }
}
