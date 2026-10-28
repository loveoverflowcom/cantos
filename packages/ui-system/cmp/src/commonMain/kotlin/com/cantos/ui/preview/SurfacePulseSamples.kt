package com.cantos.ui.preview

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.intl.LocaleList
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.isSpecified
import com.cantos.ui.CantosShapes
import com.cantos.ui.Icon
import com.cantos.ui.LocalUiPreferences
import com.cantos.ui.SurfacePulse
import com.cantos.ui.loadingRevealFor
import com.cantos.ui.toAnimationSpec
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp

/** Which side of the reveal a specimen shows. The host drives it; no timer lives here. */
internal enum class PulseStage(val id: String) {
    Running("running"), Idle("idle"), Ready("revealed");

    companion object {
        fun fromState(state: String): PulseStage = entries.firstOrNull { it.id == state } ?: Running
    }
}

internal const val PulseRegionLoadingName = "Đang tải danh sách tác phẩm"
internal const val PulseRegionReadyName = "Danh sách tác phẩm"

/**
 * The host names the whole region once. While loading, one node carries the name and state and the
 * placeholders and still-hidden content are cleared; after the reveal the content reads normally.
 */
internal fun Modifier.pulseRegionSemantics(stage: PulseStage): Modifier =
    if (stage == PulseStage.Ready) this
    else clearAndSetSemantics { contentDescription = PulseRegionLoadingName; stateDescription = PulseRegionLoadingName }

/** Reveal progress 0..1 on the canonical loading-reveal spec; reduced motion snaps. */
@Composable
private fun revealProgress(ready: Boolean): Float =
    animateFloatAsState(if (ready) 1f else 0f, loadingRevealFor(LocalUiPreferences.current.motion).toAnimationSpec(),
        label = "surface-pulse-reveal").value

/**
 * A placeholder and its loaded content share one cell, so the slot is as large as the taller of the
 * two and the reveal moves nothing. Under reduced motion the reveal is instantaneous.
 */
@Composable
internal fun PulseSlot(
    stage: PulseStage,
    modifier: Modifier = Modifier,
    placeholder: @Composable () -> Unit,
    loaded: @Composable () -> Unit,
) {
    val reveal = revealProgress(stage == PulseStage.Ready)
    Box(modifier) {
        Box(Modifier.alpha(1f - reveal)) { placeholder() }
        Box(Modifier.alpha(reveal)) { loaded() }
    }
}

/** A placeholder line is exactly one line box of the style it stands in for, so it scales with font scale. */
@Composable
private fun lineHeightOf(style: TextStyle): Dp = with(LocalDensity.current) {
    (style.lineHeight.takeIf { it.isSpecified } ?: (style.fontSize * 1.4f)).toDp()
}

@Composable
private fun PulseLines(count: Int, style: TextStyle, active: Boolean, lastLineFraction: Float) {
    val lineHeight = lineHeightOf(style)
    Column(Modifier.fillMaxWidth()) {
        repeat(count) { index ->
            SurfacePulse(Modifier.fillMaxWidth(if (index == count - 1) lastLineFraction else 1f).height(lineHeight),
                active, CantosShapes.reading)
        }
    }
}

/** A cover frame and two text lines; [placeholderTitleLines] reserves the title's lines. */
@Composable
internal fun PulseRow(
    stage: PulseStage,
    title: String,
    meta: String,
    placeholderTitleLines: Int,
    compact: Boolean = false,
    language: String = "vi-VN",
) {
    val running = stage == PulseStage.Running
    val titleStyle = MaterialTheme.typography.titleMedium
    val metaStyle = MaterialTheme.typography.labelSmall
    val coverShape = if (compact) CantosShapes.reading else CantosShapes.container
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        PulseSlot(stage, Modifier.width(if (compact) 48.dp else 80.dp).aspectRatio(if (compact) 1f else 3f / 4f),
            placeholder = { SurfacePulse(Modifier.fillMaxSize(), running, coverShape) },
            loaded = {
                Box(Modifier.fillMaxSize().clip(coverShape).background(tokenColor("color.primaryContainer")),
                    contentAlignment = Alignment.Center) { Icon("menu_book", tint = tokenColor("color.onPrimaryContainer")) }
            })
        PulseSlot(stage, Modifier.weight(1f),
            placeholder = {
                Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                    PulseLines(placeholderTitleLines, titleStyle, running, 0.8f)
                    PulseLines(1, metaStyle, running, 0.5f)
                }
            },
            loaded = {
                Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                    Text(taggedText(title, language), style = titleStyle, overflow = TextOverflow.Clip)
                    Text(taggedText(meta, language), style = metaStyle, color = tokenColor("color.onSurfaceVariant"))
                }
            })
    }
}

/** Specimen text carries its own language tag so a screen reader picks the right voice. */
private fun taggedText(value: String, language: String): AnnotatedString =
    AnnotatedString(value, listOf(AnnotatedString.Range(SpanStyle(localeList = LocaleList(language)), 0, value.length)))

/** The title bar's loaded state: a heading in the same pill-shaped bar the placeholder fills. */
@Composable
internal fun PulseBarLoaded(text: String) {
    Box(Modifier.fillMaxWidth().height(tokenDp("size.control")).padding(horizontal = tokenDp("space.md")),
        contentAlignment = Alignment.CenterStart) {
        Text(taggedText(text, "vi-VN"), style = MaterialTheme.typography.titleMedium)
    }
}
