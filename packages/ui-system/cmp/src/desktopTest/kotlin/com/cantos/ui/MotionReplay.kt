package com.cantos.ui

import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import kotlin.math.abs
import androidx.compose.ui.geometry.Size as GeometrySize
import androidx.compose.ui.unit.Density as UnitDensity

/*
 * Test-owned virtual-clock replay. A recipe is a declarative list of inputs and observation marks
 * at virtual times. The clock is Compose's manual test clock and is advanced only here, never by
 * production UI. The same recipe can be driven through different drivers and replayed to compare
 * final state. Everything here is JVM Desktop Compose: it proves ordering, retargeting and final
 * state, not real frame pacing, an OS reduce-motion signal, or any Android/iOS behavior.
 */

internal const val ManualClockTimebase = "compose-test-manual-clock-16ms-frames"
internal const val FrameMillis = 16L

internal enum class MotionInput { Down, Up, Cancel }

internal sealed interface MotionStep {
    val atMillis: Long

    data class Input(override val atMillis: Long, val input: MotionInput) : MotionStep

    /** Observes the sample at [atMillis]; [capture] also writes a PNG with its sidecar. */
    data class Mark(override val atMillis: Long, val name: String, val capture: Boolean = false) : MotionStep
}

/** Steps are ordered by time; steps at the same time run in the listed order (a mark before an input sees the state before it). */
internal data class MotionRecipe(val id: String, val steps: List<MotionStep>) {
    init {
        require(steps.zipWithNext().all { (a, b) -> a.atMillis <= b.atMillis }) { "Recipe $id is not in time order" }
    }
}

/** One mark per frame in [fromMillis]..[toMillis]; named by its time. */
internal fun everyFrame(fromMillis: Long, toMillis: Long): List<MotionStep> =
    (fromMillis..toMillis step FrameMillis).map { MotionStep.Mark(it, "t$it") }

/** One observed value at one virtual time, for either driver. */
internal data class Sample(val name: String, val atMillis: Long, val value: Double)

/** A replay's complete, comparable outcome: the event log and every sample. Equal recipes must give equal results. */
internal data class MotionReplayResult(val recipe: String, val reducedMotion: Boolean, val log: List<String>, val samples: List<Sample>) {
    fun at(name: String): Double = samples.last { it.name == name }.value

    fun frames(): List<Sample> = samples.filter { it.name.startsWith("t") && it.name.drop(1).all(Char::isDigit) }
}

@OptIn(ExperimentalTestApi::class)
private fun androidx.compose.ui.test.ComposeUiTest.advanceTo(originMillis: Long, atMillis: Long) {
    while (mainClock.currentTime - originMillis < atMillis) mainClock.advanceTimeByFrame()
}

/** Pressed fraction of the Auth action shape, read from the real shape outline: 0 = capsule, 1 = pressed radius. */
internal fun pressedFraction(shape: Shape, widthPx: Float = 300f, heightPx: Float = 60f): Double {
    val outline = shape.createOutline(GeometrySize(widthPx, heightPx), LayoutDirection.Ltr, UnitDensity(1f)) as Outline.Rounded
    val capsule = minOf(widthPx, heightPx) / 2f
    val pressed = CantosTokens.number("shape.authPressed").toFloat()
    return ((capsule - outline.roundRect.topLeftCornerRadius.x) / (capsule - pressed)).toDouble()
}

/**
 * Drives the real `authActionShape` through interaction-source events; the observable is the shape
 * outline itself, so every sample is an exact float rather than a quantized pixel.
 */
@OptIn(ExperimentalTestApi::class)
internal fun replayAuthShape(recipe: MotionRecipe, reducedMotion: Boolean, leaveMountedAfter: Boolean = true): MotionReplayResult {
    val log = mutableListOf<String>()
    val samples = mutableListOf<Sample>()
    runDesktopComposeUiTest {
        var shape: Shape? = null
        lateinit var source: MutableInteractionSource
        var mounted by mutableStateOf(true)
        setContent {
            UiRoot(preferences = UiPreferences(reducedMotion = reducedMotion)) {
                if (mounted) {
                    source = remember { MutableInteractionSource() }
                    shape = authActionShape(source)
                }
            }
        }
        waitForIdle()
        mainClock.autoAdvance = false
        val origin = mainClock.currentTime
        var press: PressInteraction.Press? = null
        recipe.steps.forEach { step ->
            advanceTo(origin, step.atMillis)
            val now = mainClock.currentTime - origin
            when (step) {
                is MotionStep.Input -> {
                    when (step.input) {
                        MotionInput.Down -> press = PressInteraction.Press(Offset.Zero).also { source.tryEmit(it) }
                        MotionInput.Up -> press?.let { source.tryEmit(PressInteraction.Release(it)) }
                        MotionInput.Cancel -> press?.let { source.tryEmit(PressInteraction.Cancel(it)) }
                    }
                    log += "t=$now ${step.input}"
                }
                is MotionStep.Mark -> samples += Sample(step.name, now, pressedFraction(requireNotNull(shape)))
            }
        }
        if (!leaveMountedAfter) {
            runOnIdle { mounted = false }
            mainClock.autoAdvance = true
            waitForIdle()
            log += "unmounted and idle"
        }
    }
    return MotionReplayResult(recipe.id, reducedMotion, log, samples)
}

/**
 * Total per-channel difference between two equally sized frames, in channel units (0..1 per channel
 * per pixel). It is deliberately a plain image distance: the ripple, the state layer and the focus ring
 * all paint in these frames, so a single painted-corner measurement would be confounded by them. Shape
 * claims are made on the exact shape outline by [replayAuthShape], not on these pixels.
 */
internal fun imageDistance(a: ImageBitmap, b: ImageBitmap): Double {
    require(a.width == b.width && a.height == b.height) { "Frames must be the same size" }
    val first = a.toPixelMap()
    val second = b.toPixelMap()
    var total = 0.0
    for (y in 0 until a.height) for (x in 0 until a.width) {
        val p = first[x, y]
        val q = second[x, y]
        total += abs(p.red - q.red) + abs(p.green - q.green) + abs(p.blue - q.blue) + abs(p.alpha - q.alpha)
    }
    return total
}

/**
 * Drives the production [Button] with real touch dispatch. Samples are the image distance to the first frame; the result also
 * carries the intents dispatched at each sample and fails fast if the hit box ever moves.
 */
@OptIn(ExperimentalTestApi::class)
internal fun replayAuthButton(recipe: MotionRecipe, reducedMotion: Boolean, captureName: String? = null): Pair<MotionReplayResult, List<Int>> {
    val log = mutableListOf<String>()
    val samples = mutableListOf<Sample>()
    val intentsAt = mutableListOf<Int>()
    runDesktopComposeUiTest {
        var intents = 0
        setContent {
            UiRoot(preferences = UiPreferences(reducedMotion = reducedMotion)) {
                Button(
                    "Đăng nhập với Cantos ID", { intents += 1 },
                    variant = ButtonVariant.AuthPrimary,
                    controlModifier = Modifier.requiredSize(300.dp, 60.dp).testTag("motion-action"),
                )
            }
        }
        waitForIdle()
        mainClock.autoAdvance = false
        val action = onNodeWithTag("motion-action")
        val bounds = action.fetchSemanticsNode().boundsInRoot
        val origin = mainClock.currentTime
        var first: ImageBitmap? = null
        recipe.steps.forEach { step ->
            advanceTo(origin, step.atMillis)
            val now = mainClock.currentTime - origin
            when (step) {
                is MotionStep.Input -> {
                    when (step.input) {
                        MotionInput.Down -> action.performTouchInput { down(center) }
                        MotionInput.Up -> action.performTouchInput { up() }
                        MotionInput.Cancel -> action.performTouchInput { moveTo(Offset(-20f, -20f)); up() }
                    }
                    log += "t=$now ${step.input} intents=$intents"
                }
                is MotionStep.Mark -> {
                    check(bounds == action.fetchSemanticsNode().boundsInRoot) { "The hit box moved at ${step.name}" }
                    val bitmap = action.captureToImage()
                    val reference = first ?: bitmap.also { first = it }
                    samples += Sample(step.name, now, imageDistance(reference, bitmap))
                    intentsAt += intents
                    if (step.capture && captureName != null) {
                        val mode = if (reducedMotion) "reduced" else "full"
                        writeCapture(
                            CaptureScenario(
                                name = "cmp-motion-$captureName-$mode-${step.name}",
                                section = "motion",
                                component = "button",
                                state = "auth-primary ${recipe.id} ${step.name}",
                                themeMode = ThemeMode.Light,
                                systemIsDark = false,
                                preferences = UiPreferences(reducedMotion = reducedMotion),
                                viewportWidthDp = 300,
                                viewportHeightDp = 60,
                                hostDensity = 1f,
                                hostFontScale = 1f,
                                frame = CaptureFrame(recipe.id, step.name, now, ManualClockTimebase),
                            ),
                            bitmap,
                        )
                    }
                }
            }
        }
    }
    return MotionReplayResult(recipe.id, reducedMotion, log, samples) to intentsAt
}

/**
 * Independent of Compose's solver: bisect x(s) = progress for the parametric s, then evaluate y(s).
 * [progress] is elapsed / duration and is clamped to 0..1.
 */
internal fun easedProgress(easing: MotionEasing, progress: Double): Double {
    if (progress <= 0.0) return 0.0
    if (progress >= 1.0) return 1.0
    fun bezier(first: Double, second: Double, s: Double) = 3 * (1 - s) * (1 - s) * s * first + 3 * (1 - s) * s * s * second + s * s * s
    var low = 0.0
    var high = 1.0
    repeat(60) {
        val middle = (low + high) / 2
        if (bezier(easing.x1.toDouble(), easing.x2.toDouble(), middle) < progress) low = middle else high = middle
    }
    return bezier(easing.y1.toDouble(), easing.y2.toDouble(), (low + high) / 2)
}
