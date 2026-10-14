package com.cantos.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosLogoAsset
import com.cantos.ui.generated.CantosLogoShape
import com.cantos.ui.generated.CantosLogos
import kotlin.math.min

enum class LogoVariant(internal val file: String) {
    Cantos("cantos-logo"),
    CantosStudio("cantos-studio-logo"),
    Symbol("cantos-symbol"),
}

enum class LogoTone(internal val suffix: String) { Light(""), Dark("-dark"), Mono("-mono") }

internal fun logoAsset(variant: LogoVariant, tone: LogoTone): CantosLogoAsset =
    CantosLogos.asset(variant.file + tone.suffix + ".svg")

/**
 * Original local outlined artwork, with no font, bitmap, network or SVG runtime dependency.
 * Decorative by default beside a heading/control; only standalone marks need the caller's
 * localized [accessibleName]. The caller selects the tone for its surface and symbol clear space.
 * ContentScale.Fit keeps the official ratio even in a caller-supplied square or narrow slot.
 * Both preview renderers must qualify before new app adoption.
 */
@Composable
fun Logo(
    variant: LogoVariant = LogoVariant.Cantos,
    modifier: Modifier = Modifier,
    tone: LogoTone = LogoTone.Light,
    accessibleName: AccessibleName? = null,
) {
    val asset = logoAsset(variant, tone)
    val painter = remember(asset) { OfficialLogoPainter(asset) }
    Image(
        painter = painter,
        contentDescription = null,
        modifier = modifier.width(asset.width.dp).clearAndSetSemantics {
            if (accessibleName != null) {
                contentDescription = accessibleName.text
                role = Role.Image
            }
        },
        contentScale = ContentScale.Fit,
    )
}

private class OfficialLogoPainter(private val asset: CantosLogoAsset) : Painter() {
    override val intrinsicSize: Size = Size(asset.width, asset.height)
    private val paths: List<Path?> = asset.shapes.map { shape ->
        shape.pathData?.let { PathParser().parsePathString(it).toPath() }
    }

    override fun DrawScope.onDraw() {
        // Never paint holes with a paper/dark color: an isolated layer makes them transparent
        // without erasing the caller's background. All other artwork remains filled vector paths.
        val canvas = drawContext.canvas
        canvas.saveLayer(Rect(Offset.Zero, size), Paint())
        val factor = min(size.width / asset.width, size.height / asset.height)
        withTransform({
            translate((size.width - asset.width * factor) / 2f, (size.height - asset.height * factor) / 2f)
            scale(factor, factor, pivot = Offset.Zero)
        }) {
            asset.shapes.zip(paths).forEach { (shape, path) ->
                translate(shape.translateX, shape.translateY) { drawShape(shape, path) }
            }
        }
        canvas.restore()
    }
}

private fun DrawScope.drawShape(shape: CantosLogoShape, path: Path?) {
    val blend = if (shape.knockout) BlendMode.Clear else BlendMode.SrcOver
    if (path == null) {
        shape.fill?.let { color ->
            drawRoundRect(Color(color), size = Size(shape.width, shape.height),
                cornerRadius = CornerRadius(shape.radius), blendMode = blend)
        }
    } else {
        shape.fill?.let { drawPath(path, Color(it), blendMode = blend) }
        shape.stroke?.let { color ->
            drawPath(path, Color(color), blendMode = blend,
                style = Stroke(shape.strokeWidth, cap = if (shape.roundCap) StrokeCap.Round else StrokeCap.Butt))
        }
    }
}
