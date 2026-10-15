package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.material3.Text
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.ui.UiPreferences
import com.cantos.ui.generated.CantosTokens
import java.io.File
import kotlin.math.abs
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/**
 * The launch entrance on a manual clock: a full-motion frame sequence and its reduced final frame.
 * JVM Desktop Compose only. It adds no timer, hold or readiness wait and observes no OS launch, no
 * recording and no real frame pacing; readiness, hold and lifecycle stay with the existing tests.
 */
@OptIn(ExperimentalTestApi::class)
class LaunchMotionReplayTest {
    private val frameMillis = 16L

    /** The frames of one entrance and where its copy block (wordmark and tagline) settles, in root pixels. */
    private class Entrance(val frames: List<Pair<Long, ImageBitmap>>, val copy: Rect, val copyTops: List<Float>)

    /** One bitmap per frame from t = 0 to [untilMillis] on the manual clock, starting at the first frame. */
    private fun entranceFrames(reducedMotion: Boolean, untilMillis: Long): Entrance {
        val frames = mutableListOf<Pair<Long, ImageBitmap>>()
        val tops = mutableListOf<Float>()
        var copy = Rect.Zero
        runDesktopComposeUiTest(width = 390, height = 844) {
            mainClock.autoAdvance = false
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp).testTag("capture-root")) {
                    CantosLocalLaunch(false, UiPreferences(reducedMotion = reducedMotion)) {}
                }
            }
            val origin = mainClock.currentTime
            while (true) {
                val now = mainClock.currentTime - origin
                frames += now to onNodeWithTag("capture-root").captureToImage()
                tops += onNodeWithTag("splash-copy", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot.top
                if (now >= untilMillis) {
                    // Grown by the 8 dp entrance travel (at most 24 px at 3x) so faint offset frames are inside it.
                    copy = onNodeWithTag("splash-copy", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot.inflate(24f)
                    break
                }
                mainClock.advanceTimeByFrame()
            }
        }
        return Entrance(frames, copy, tops)
    }

    /** True when every pixel in [bounds] is the background, that is, nothing is drawn there. */
    private fun isBlank(bitmap: ImageBitmap, bounds: Rect): Boolean {
        val pixels = bitmap.toPixelMap()
        val background = pixels[2, 2]
        for (y in bounds.top.toInt().coerceAtLeast(0) until bounds.bottom.toInt().coerceAtMost(bitmap.height)) {
            for (x in bounds.left.toInt().coerceAtLeast(0) until bounds.right.toInt().coerceAtMost(bitmap.width)) {
                val pixel = pixels[x, y]
                if (abs(pixel.red - background.red) + abs(pixel.green - background.green) + abs(pixel.blue - background.blue) > 0.01f) return false
            }
        }
        return true
    }

    /** A labelled frame for opening: the PNG and a sidecar that says which clock, state and tokens produced it. */
    private fun writeFrame(name: String, timeMillis: Long, bitmap: ImageBitmap, reducedMotion: Boolean) {
        val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
        check(output.isDirectory || output.mkdirs())
        val png = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
        File(output, "$name.png").writeBytes(png)
        File(output, "$name.json").writeText(
            """
            {
              "name": "$name",
              "recipe": "launch-entrance",
              "state": "local-launch, not ready",
              "time_ms": $timeMillis,
              "timebase": "compose-test-manual-clock-16ms-frames",
              "reduced_motion": $reducedMotion,
              "viewport_dp": {"width": 390, "height": 844},
              "tokens_version": "${CantosTokens.version}",
              "emphasis_duration_ms": ${CantosTokens.number("motion.emphasis.duration").toInt()},
              "renderer": "jvm-desktop-compose-test-root",
              "not_evidence_of": ["android-device", "ios-device", "os-window", "runtime-recording", "os-reduce-motion", "design-pixel-match"]
            }
            """.trimIndent() + "\n",
        )
    }

    private fun distance(a: ImageBitmap, b: ImageBitmap): Double {
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

    @Test
    fun full_motion_converges_on_the_reduced_final_frame_by_the_canonical_duration_and_stays_there() {
        val duration = CantosTokens.number("motion.emphasis.duration").toLong()
        val entrance = entranceFrames(reducedMotion = false, untilMillis = 480)
        val full = entrance.frames
        val final = entranceFrames(reducedMotion = true, untilMillis = 0).frames.single().second
        val distances = full.map { (_, bitmap) -> distance(bitmap, final) }
        val start = distances.first()
        val percent = distances.map { (it / start * 100).toInt() }
        assertTrue(start > 0.0, "the first frame must differ from the final frame: there is an entrance to replay")
        // A fade that also travels is not monotone in a plain image distance while the content is faint and
        // offset, so monotone convergence is asserted from the midpoint, where it is.
        val midpoint = full.indexOfFirst { it.first >= duration / 2 }
        assertTrue(distances[midpoint] > 0.001 * start && distances[midpoint] < start, "no mid-flight frame at ${duration / 2} ms: $percent")
        distances.drop(midpoint).zipWithNext().forEachIndexed { index, (a, b) ->
            assertTrue(b <= a + 0.002 * start, "frame ${midpoint + index + 1} moved away from the final frame: overshoot or a restart: $percent")
        }
        val settled = full.indexOfFirst { it.first >= duration + 6 * frameMillis }
        // Opacity is its own property: the copy starts fully transparent, fades in and is drawn once settled.
        assertTrue(isBlank(full.first().second, entrance.copy), "the copy must start fully transparent")
        assertFalse(isBlank(full[midpoint].second, entrance.copy), "the copy must be fading in at the midpoint")
        assertFalse(isBlank(full[settled].second, entrance.copy), "the copy must be drawn once settled")
        writeFrame("splash-motion-full-frame0", full.first().first, full.first().second, reducedMotion = false)
        writeFrame("splash-motion-full-mid", full[midpoint].first, full[midpoint].second, reducedMotion = false)
        writeFrame("splash-motion-full-settled", full[settled].first, full[settled].second, reducedMotion = false)
        writeFrame("splash-motion-reduced-frame0", 0, final, reducedMotion = true)
        distances.drop(settled).forEach { assertTrue(it <= 0.001 * start, "not settled by ${full[settled].first} ms or left it again: $percent") }
    }

    /** Independent of Compose's solver: bisect x(s) = progress, then evaluate y(s). */
    private fun eased(x1: Double, y1: Double, x2: Double, y2: Double, progress: Double): Double {
        if (progress <= 0.0) return 0.0
        if (progress >= 1.0) return 1.0
        fun bezier(a: Double, b: Double, s: Double) = 3 * (1 - s) * (1 - s) * s * a + 3 * (1 - s) * s * s * b + s * s * s
        var low = 0.0
        var high = 1.0
        repeat(60) {
            val middle = (low + high) / 2
            if (bezier(x1, x2, middle) < progress) low = middle else high = middle
        }
        return bezier(y1, y2, (low + high) / 2)
    }

    @Test
    fun the_copy_travels_with_the_declared_spatial_duration_and_easing_frame_by_frame() {
        val duration = CantosTokens.number("motion.emphasis.duration")
        val entrance = entranceFrames(reducedMotion = false, untilMillis = 480)
        val times = entrance.frames.map { it.first }
        val tops = entrance.copyTops
        val settled = tops.last()
        val travel = tops.first() - settled
        // 8 dp at the test host's 1 px/dp; the copy block also fades, but only its position is read here.
        assertEquals(8.0, travel.toDouble(), 0.01, "the copy must start 8 dp below its resting place")
        // The animation starts on the last frame that still shows the starting offset.
        val start = times[tops.indexOfLast { abs(it - tops.first()) < 1e-3 }]
        times.indices.filter { times[it] >= start }.forEach { index ->
            val expected = settled + travel * (1.0 - eased(0.4, 0.0, 0.2, 1.0, (times[index] - start) / duration))
            assertEquals(expected, tops[index].toDouble(), 0.25, "copy top at ${times[index]} ms")
        }
        assertTrue(tops.zipWithNext().all { (a, b) -> b <= a + 1e-3 }, "the copy moved backwards: overshoot or a restart")
    }

    @Test
    fun reduced_motion_shows_the_final_frame_on_the_first_frame_and_never_changes() {
        val entrance = entranceFrames(reducedMotion = true, untilMillis = 480)
        val frames = entrance.frames
        val first = frames.first().second
        assertFalse(isBlank(first, entrance.copy), "reduced motion must draw the copy on the first frame")
        frames.forEach { (time, bitmap) -> assertEquals(0.0, distance(first, bitmap), "reduced motion changed at $time ms") }
    }

    @Test(timeout = 120_000)
    fun readiness_mid_entrance_leaves_no_animation_or_coroutine_pending() {
        val ready = mutableStateOf(false)
        runDesktopComposeUiTest(width = 390, height = 844) {
            mainClock.autoAdvance = false
            setContent {
                CantosLocalLaunch(ready.value, UiPreferences()) {
                    Text("restored destination", Modifier.testTag("destination"))
                }
            }
            onNodeWithTag("local-launch").assertExists()
            mainClock.advanceTimeBy(48)
            runOnIdle { ready.value = true }
            mainClock.advanceTimeByFrame()
            onNodeWithTag("local-launch").assertDoesNotExist()
            onNodeWithTag("destination").assertTextEquals("restored destination")
            // With the clock running again, idle is reached only if no entrance animation survived.
            mainClock.autoAdvance = true
            waitForIdle()
            onNodeWithTag("destination").assertTextEquals("restored destination")
        }
    }
}
