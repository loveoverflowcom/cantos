package com.cantos.ui.preview

import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.platform.Font as bytesFont

/**
 * The shared Noto Sans / Noto Serif faces for the desktop preview host. The bytes are the committed
 * bundle in `packages/ui-system/shared/assets/noto-fonts` (the same bytes the Theatre mock and the
 * Leptos preview ship, recorded in its `provenance.json`); Gradle copies them under [RESOURCE_ROOT]
 * and nothing is downloaded or read from the machine's installed fonts.
 *
 * Only a composition root calls this; components receive the resulting families through
 * `CantosTheme`. Static masters exist for 400 and 700 only, so other requested weights resolve to the
 * nearest of those two rather than to a synthesized medium.
 */
object PreviewFonts {
    const val RESOURCE_ROOT = "cantos-preview-fonts"

    /** Face file → (family, weight); the order is the load order. */
    val faces: List<Face> = listOf(
        Face("noto_sans_regular.ttf", Family.Sans, FontWeight.Normal),
        Face("noto_sans_bold.ttf", Family.Sans, FontWeight.Bold),
        Face("noto_serif_regular.ttf", Family.Serif, FontWeight.Normal),
        Face("noto_serif_bold.ttf", Family.Serif, FontWeight.Bold),
    )

    enum class Family { Sans, Serif }

    data class Face(val file: String, val family: Family, val weight: FontWeight)

    /** Raw bytes of one bundled face; fails loudly when the bundle is missing from the classpath. */
    fun bytes(file: String): ByteArray {
        val path = "$RESOURCE_ROOT/$file"
        val stream = PreviewFonts::class.java.classLoader.getResourceAsStream(path)
            ?: error("Preview font $path is not on the classpath; the shared Noto bundle was not copied")
        return stream.use { it.readBytes() }
    }

    private fun family(which: Family): FontFamily = FontFamily(
        faces.filter { it.family == which }.map { face -> bytesFont("${face.family}-${face.file}", bytes(face.file), face.weight) },
    )

    /** UI text: Noto Sans. */
    val ui: FontFamily by lazy { family(Family.Sans) }

    /** Editorial text: Noto Serif. */
    val editorial: FontFamily by lazy { family(Family.Serif) }
}
