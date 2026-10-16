package com.cantos.theatre.features.launch.presentation

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTag
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.cantos_brand_symbol_dark
import com.cantos.theatre.resources.cantos_brand_symbol_light
import com.cantos.theatre.resources.cantos_brand_wordmark_dark
import com.cantos.theatre.resources.cantos_brand_wordmark_light
import com.cantos.theatre.resources.noto_sans_regular
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.CantosTheme
import com.cantos.ui.LocalUiPreferences
import com.cantos.ui.MotionIntent
import com.cantos.ui.MotionProperty
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import com.cantos.ui.animateWith
import com.cantos.ui.entranceStart
import com.cantos.ui.rememberMotionSpec
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.Font
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource
import androidx.compose.ui.text.font.FontFamily

/** Presentation only: readiness belongs to the existing local owner, never catalog or identity IO.
 * Ready content wins in the same composition, with no entrance/exit wait or navigation event.
 * The current native owners initialize synchronously and therefore skip this optional handoff.
 */
@Composable
fun CantosLocalLaunch(
    localReady: Boolean,
    preferences: UiPreferences,
    modifier: Modifier = Modifier,
    locale: ShellLocale = ShellLocale.Vietnamese,
    content: @Composable () -> Unit,
) {
    if (localReady) {
        content()
    } else {
        CantosTheme(preferences, uiFontFamily = FontFamily(Font(Res.font.noto_sans_regular))) {
            CantosSplashPresentation(modifier, locale)
        }
    }
}

@Composable
private fun CantosSplashPresentation(modifier: Modifier, locale: ShellLocale) {
    val preferences = LocalUiPreferences.current
    val dark = preferences.theme == Theme.Dark
    // Opacity is an effect and never overshoots; travel, scale and rotation are spatial. Each has its own spec.
    val effectsSpec = rememberMotionSpec(MotionIntent.Emphasis, MotionProperty.Effects)
    val spatialSpec = rememberMotionSpec(MotionIntent.Emphasis, MotionProperty.Spatial)
    // An Instant spec starts at its final value, so Reduced motion draws the final frame at once,
    // before any coroutine can run.
    val effects = remember { Animatable(effectsSpec.entranceStart()) }
    val spatial = remember { Animatable(spatialSpec.entranceStart()) }
    LaunchedEffect(effectsSpec, spatialSpec) {
        launch { effects.animateWith(1f, effectsSpec) }
        launch { spatial.animateWith(1f, spatialSpec) }
    }
    val effectsProgress = effects.value
    val spatialProgress = spatial.value
    val resources = launchResources(locale)
    val status = stringResource(resources.status)
    val tagline = stringResource(resources.tagline)
    val category = stringResource(resources.category)
    val symbol = painterResource(if (dark) Res.drawable.cantos_brand_symbol_dark else Res.drawable.cantos_brand_symbol_light)
    val wordmark = painterResource(if (dark) Res.drawable.cantos_brand_wordmark_dark else Res.drawable.cantos_brand_wordmark_light)
    BoxWithConstraints(
        modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)
            .semantics { contentDescription = status; testTag = "local-launch" }
            .safeDrawingPadding().padding(24.dp),
    ) {
        val landscape = maxWidth > maxHeight
        val largeText = LocalDensity.current.fontScale >= 1.8f
        // Constraints exclude the 24 dp safe content padding on each side.
        val compact = maxWidth < 312.dp
        val haloSize = if (compact || maxHeight < 400.dp) 200.dp else 232.dp
        val copyWidth = if (landscape) (maxWidth - haloSize - 32.dp).coerceAtMost(300.dp) else maxWidth.coerceAtMost(440.dp)
        val wordmarkWidth = if (compact) 176.dp else 194.dp
        // Splash-specific reference geometry, retaining the canonical body family/weight.
        val taglineSize = if (compact) 16.sp else 17.sp
        val density = LocalDensity.current
        val copyMotion = Modifier.graphicsLayer {
            alpha = effectsProgress
            translationY = with(density) { 8.dp.toPx() } * (1f - spatialProgress)
        }
        val brand: @Composable () -> Unit = {
            Box(Modifier.size(haloSize), contentAlignment = Alignment.Center) {
                Box(Modifier.fillMaxSize().graphicsLayer {
                    scaleX = .92f + .08f * spatialProgress
                    scaleY = scaleX
                    rotationZ = -18f + 6f * spatialProgress
                }.background(MaterialTheme.colorScheme.primaryContainer, RoundedCornerShape(42, 54, 40, 52)))
                Image(symbol, null, Modifier.size(128.dp))
            }
        }
        val copy: @Composable () -> Unit = {
            Column(
                Modifier.width(copyWidth).then(copyMotion).clearAndSetSemantics { testTag = "splash-copy" },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Image(wordmark, null, Modifier.width(wordmarkWidth))
                Text(tagline, style = MaterialTheme.typography.bodyLarge.copy(fontSize = taglineSize, lineHeight = taglineSize * 1.55f), textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (landscape) {
            Row(Modifier.align(Alignment.Center), horizontalArrangement = Arrangement.spacedBy(32.dp), verticalAlignment = Alignment.CenterVertically) {
                brand()
                copy()
            }
        } else {
            Column(
                Modifier.align(Alignment.Center).offset(y = if (largeText || maxHeight < 480.dp) 0.dp else 64.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(24.dp),
            ) {
                brand()
                copy()
            }
        }
        if (!largeText) Text(
            category,
            modifier = Modifier.align(Alignment.BottomCenter).offset(x = if (landscape) (haloSize + 32.dp) / 2 else 0.dp).clearAndSetSemantics {},
            style = MaterialTheme.typography.bodySmall.copy(fontSize = 11.sp, lineHeight = 16.5.sp),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
    }
}
