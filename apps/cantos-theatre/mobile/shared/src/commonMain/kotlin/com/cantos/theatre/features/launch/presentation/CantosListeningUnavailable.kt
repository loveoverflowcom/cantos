package com.cantos.theatre.features.launch.presentation

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.cantos.theatre.resources.Res
import com.cantos.theatre.presentation.model.HostConfigurationFault
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.resources.noto_sans_regular
import com.cantos.ui.CantosTheme
import com.cantos.ui.Logo
import com.cantos.ui.LogoTone
import com.cantos.ui.LogoVariant
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import org.jetbrains.compose.resources.Font
import org.jetbrains.compose.resources.stringResource

/**
 * The explicit state of a host composed without production listening adapters. It is neither
 * a loading state nor an empty shell: no catalog, library, player or sample content is drawn,
 * and nothing here can construct an adapter. The host resolves appearance and text scale.
 * A non-null [fault] names why the host refused its configuration (#298): the typed diagnostic
 * replaces the generic copy, and nothing falls back to another environment or a sample.
 */
@Composable
fun CantosListeningUnavailable(
    preferences: UiPreferences,
    modifier: Modifier = Modifier,
    locale: ShellLocale = ShellLocale.Vietnamese,
    fault: HostConfigurationFault? = null,
) {
    val resources = launchResources(locale)
    val (title, body) = fault?.let { faultResources(it, locale) } ?: (resources.unavailableTitle to resources.unavailableBody)
    CantosTheme(preferences, uiFontFamily = FontFamily(Font(Res.font.noto_sans_regular))) {
        Column(
            modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)
                .semantics { testTag = "listening-unavailable" }
                .safeDrawingPadding().verticalScroll(rememberScrollState()).padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
        ) {
            // Decorative beside the heading, which already names Cantos.
            Logo(
                LogoVariant.Symbol,
                Modifier.size(88.dp),
                tone = if (preferences.theme == Theme.Dark) LogoTone.Dark else LogoTone.Light,
            )
            Text(
                stringResource(title),
                Modifier.widthIn(max = 440.dp).semantics { heading(); testTag = "listening-unavailable-title" },
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.onSurface,
                textAlign = TextAlign.Center,
            )
            Text(
                stringResource(body),
                Modifier.widthIn(max = 440.dp).semantics { testTag = "listening-unavailable-body" },
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
        }
    }
}
