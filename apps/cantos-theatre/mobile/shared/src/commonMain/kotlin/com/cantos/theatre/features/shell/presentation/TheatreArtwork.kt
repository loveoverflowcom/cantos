package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.MenuBook
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.noto_sans_bold
import com.cantos.theatre.resources.noto_sans_regular
import com.cantos.theatre.resources.noto_serif_bold
import com.cantos.theatre.resources.noto_serif_regular
import org.jetbrains.compose.resources.Font

@Composable
internal fun theatreSans(): FontFamily = FontFamily(
    Font(Res.font.noto_sans_regular, FontWeight.Normal),
    Font(Res.font.noto_sans_bold, FontWeight.Bold),
)

@Composable
internal fun theatreSerif(): FontFamily = FontFamily(
    Font(Res.font.noto_serif_regular, FontWeight.Normal),
    Font(Res.font.noto_serif_bold, FontWeight.Bold),
)

/** A host supplies approved artwork. Missing artwork keeps the accessible tonal fallback. */
typealias WorkArtworkContent = @Composable (ShellWorkId, Modifier, Shape) -> Unit
val LocalWorkArtwork = androidx.compose.runtime.staticCompositionLocalOf<WorkArtworkContent?> { null }

@Composable
internal fun WorkArtwork(id: ShellWorkId, modifier: Modifier = Modifier, shape: Shape = MaterialTheme.shapes.large) {
    val supplied = LocalWorkArtwork.current
    if (supplied != null) {
        supplied(id, modifier, shape)
    } else {
        Surface(modifier, color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = shape) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Icon(Icons.AutoMirrored.Outlined.MenuBook, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}
