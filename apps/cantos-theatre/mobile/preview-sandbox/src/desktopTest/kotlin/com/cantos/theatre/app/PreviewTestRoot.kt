package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.previewLibraryStore
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import com.cantos.theatre.ports.LocalePreferenceStore
import com.cantos.ui.Density
import com.cantos.ui.MotionPreference

/**
 * Mounts the one Cantos Theatre application over freshly built preview owners, exactly as the
 * Desktop Preview does, and hands the owners to the test for assertions about the real state.
 */
@Composable
internal fun PreviewTestRoot(
    seed: PreviewSeed = PreviewSession.Guest.seed(),
    modifier: Modifier = Modifier,
    textScale: Float = 1f,
    density: Density = Density.Comfortable,
    motionPreference: MotionPreference = MotionPreference.Reduced,
    localeStore: LocalePreferenceStore? = null,
    backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
    onOwners: (PreviewOwners) -> Unit = {},
) {
    val scope = rememberCoroutineScope()
    val owners = remember(seed) { PreviewOwners(scope, seed, previewLibraryStore(), localeStore).also(onOwners) }
    DisposableEffect(owners) { onDispose { owners.close() } }
    PreviewTheatreApp(owners, modifier, textScale, backHandler = backHandler, density = density, motionPreference = motionPreference)
}
