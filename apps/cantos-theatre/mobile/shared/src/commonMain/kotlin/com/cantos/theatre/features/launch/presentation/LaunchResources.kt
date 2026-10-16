package com.cantos.theatre.features.launch.presentation

import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.listening_unavailable_body
import com.cantos.theatre.resources.listening_unavailable_body_en
import com.cantos.theatre.resources.listening_unavailable_title
import com.cantos.theatre.resources.listening_unavailable_title_en
import com.cantos.theatre.resources.splash_category
import com.cantos.theatre.resources.splash_category_en
import com.cantos.theatre.resources.splash_status
import com.cantos.theatre.resources.splash_status_en
import com.cantos.theatre.resources.splash_tagline
import com.cantos.theatre.resources.splash_tagline_en
import org.jetbrains.compose.resources.StringResource

internal data class LaunchResources(
    val status: StringResource,
    val tagline: StringResource,
    val category: StringResource,
    val unavailableTitle: StringResource,
    val unavailableBody: StringResource,
)

// CMP 1.9 has no public subtree locale override. Select these existing XML resources
// explicitly instead of changing the process locale or remounting the application.
internal fun launchResources(locale: ShellLocale): LaunchResources = when (locale) {
    ShellLocale.Vietnamese -> LaunchResources(
        Res.string.splash_status, Res.string.splash_tagline, Res.string.splash_category,
        Res.string.listening_unavailable_title, Res.string.listening_unavailable_body,
    )
    ShellLocale.English -> LaunchResources(
        Res.string.splash_status_en, Res.string.splash_tagline_en, Res.string.splash_category_en,
        Res.string.listening_unavailable_title_en, Res.string.listening_unavailable_body_en,
    )
}
