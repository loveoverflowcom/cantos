package com.cantos.ui.preview

import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.cantos.ui.AccessibleName
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.DisabledReason
import com.cantos.ui.Stack

/** Synthetic host facts only. A specimen never acknowledges a real media engine. */
@Composable
internal fun ListenHeroSample(state: String, onPress: () -> Unit, modifier: Modifier = Modifier) {
    val phase = state.removePrefix("listen-")
    val label = PreviewToolCopy.ListenFixture.label(phase)
    val status = PreviewToolCopy.ListenFixture.status(phase)
    val availability = when (phase) {
        "resolving" -> ButtonState.Loading(DisabledReason.of(status))
        "unavailable" -> ButtonState.Disabled(DisabledReason.of(status))
        else -> ButtonState.Enabled
    }
    Stack {
        Button(name = AccessibleName.of(PreviewToolCopy.ListenFixture.name(label)), onPress = onPress,
            state = availability, variant = ButtonVariant.ListenHero, controlModifier = modifier,
            icon = when (phase) { "buffering", "playing" -> "pause"; "failed" -> "refresh"; else -> "play_arrow" }) {
            Text(label)
        }
        if (phase != "resolving" && phase != "unavailable") Text(status)
    }
}
