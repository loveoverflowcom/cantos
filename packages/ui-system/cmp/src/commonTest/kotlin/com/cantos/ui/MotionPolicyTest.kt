package com.cantos.ui

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.SnapSpec
import androidx.compose.animation.core.TweenSpec
import com.cantos.ui.generated.CantosTokens
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue

/** The pure core of motion: bounded exhaustive over intent × property × preference. */
class MotionPolicyTest {
    private val combinations = MotionIntent.entries.flatMap { intent -> MotionProperty.entries.map { intent to it } }

    // Written as a table so a mis-wired role fails here instead of agreeing with the production `when`.
    private val expectedRoles = mapOf(
        (MotionIntent.Feedback to MotionProperty.Effects) to "motion.feedback.duration",
        (MotionIntent.Navigation to MotionProperty.Effects) to "motion.navigation.duration",
        (MotionIntent.Emphasis to MotionProperty.Effects) to "motion.emphasis.duration",
        (MotionIntent.Feedback to MotionProperty.Spatial) to "motion.shape.duration",
        (MotionIntent.Navigation to MotionProperty.Spatial) to "motion.navigation.duration",
        (MotionIntent.Emphasis to MotionProperty.Spatial) to "motion.emphasis.duration",
    )

    @Test
    fun reduced_preference_is_the_final_state_at_once_for_every_intent_and_property() {
        assertEquals(6, combinations.size)
        combinations.forEach { (intent, property) ->
            assertEquals(
                MotionSpec.Instant,
                motionFor(intent, property, MotionPreference.Reduced),
                "Reduced motion must not travel or fade over time for $intent/$property",
            )
        }
    }

    @Test
    fun full_effects_use_the_canonical_duration_and_easing_and_cannot_overshoot() {
        val canonicalEasing = MotionEasing(0.2f, 0f, 0f, 1f)
        assertEquals("[0.2,0,0,1]", CantosTokens.text("motion.effects.easing"))
        MotionIntent.entries.forEach { intent ->
            val spec = assertIs<MotionSpec.Timed>(motionFor(intent, MotionProperty.Effects, MotionPreference.Full))
            assertEquals(CantosTokens.number(expectedRoles.getValue(intent to MotionProperty.Effects)).toInt(), spec.durationMillis)
            assertEquals(canonicalEasing, spec.easing)
            assertFalse(spec.easing.mayOvershoot, "An effects curve overshot for $intent")
        }
    }

    @Test
    fun full_spatial_uses_a_canonical_duration_and_the_named_spatial_easing() {
        MotionIntent.entries.forEach { intent ->
            val spec = assertIs<MotionSpec.Timed>(motionFor(intent, MotionProperty.Spatial, MotionPreference.Full))
            assertEquals(CantosTokens.number(expectedRoles.getValue(intent to MotionProperty.Spatial)).toInt(), spec.durationMillis)
            assertEquals(SpatialStandardEasing, spec.easing)
        }
    }

    @Test
    fun the_roles_the_policy_reads_differ_exactly_where_the_token_values_differ() {
        // Feedback is the one intent whose spatial and effects durations are separate roles; if the two
        // were wired to the same role the exhaustive checks above would still agree with each other.
        val effects = assertIs<MotionSpec.Timed>(motionFor(MotionIntent.Feedback, MotionProperty.Effects, MotionPreference.Full))
        val spatial = assertIs<MotionSpec.Timed>(motionFor(MotionIntent.Feedback, MotionProperty.Spatial, MotionPreference.Full))
        assertEquals(140, effects.durationMillis)
        assertEquals(220, spatial.durationMillis)
        combinations.forEach { (intent, property) ->
            assertEquals(expectedRoles.getValue(intent to property), motionDurationRole(intent, property))
        }
    }

    @Test
    fun every_full_duration_is_positive_whole_milliseconds_inside_the_bound() {
        combinations.forEach { (intent, property) ->
            val spec = assertIs<MotionSpec.Timed>(motionFor(intent, property, MotionPreference.Full))
            assertTrue(spec.durationMillis in 1..MaxMotionDurationMillis, "$intent/$property = ${spec.durationMillis} ms")
            val role = CantosTokens.number(motionDurationRole(intent, property))
            assertEquals(role, spec.durationMillis.toDouble(), "A token must be a whole number of milliseconds")
        }
    }

    @Test
    fun the_token_source_zeroes_every_duration_role_the_policy_reads_under_reduced_motion() {
        combinations.forEach { (intent, property) ->
            assertEquals(0.0, CantosTokens.number(motionDurationRole(intent, property), reducedMotion = true))
        }
    }

    @Test
    fun no_canonical_motion_duration_role_is_left_unread_by_the_policy() {
        val read = combinations.map { (intent, property) -> motionDurationRole(intent, property) }.toSet()
        val canonical = CantosTokens.names.filter { it.startsWith("motion.") && it.endsWith(".duration") }.toSet()
        assertEquals(canonical, read, "A new or removed motion duration token needs a policy decision")
    }

    @Test
    fun easing_values_must_be_finite_and_a_function_of_time() {
        listOf(Float.NaN, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY).forEach { bad ->
            assertFailsWith<IllegalArgumentException> { MotionEasing(bad, 0f, 0.2f, 1f) }
            assertFailsWith<IllegalArgumentException> { MotionEasing(0.2f, bad, 0.2f, 1f) }
        }
        assertFailsWith<IllegalArgumentException> { MotionEasing(-0.01f, 0f, 0.2f, 1f) }
        assertFailsWith<IllegalArgumentException> { MotionEasing(0.2f, 0f, 1.01f, 1f) }
        assertFalse(MotionEasing(0.2f, 0f, 0f, 1f).mayOvershoot)
        assertTrue(MotionEasing(0.2f, -0.5f, 0f, 1f).mayOvershoot)
        assertTrue(MotionEasing(0.2f, 0f, 0f, 1.5f).mayOvershoot)
    }

    @Test
    fun a_duration_outside_the_bound_is_rejected_at_construction_and_by_copy() {
        val easing = MotionEasing(0.2f, 0f, 0f, 1f)
        listOf(0, -1, MaxMotionDurationMillis + 1, Int.MAX_VALUE).forEach { bad ->
            assertFailsWith<IllegalArgumentException> { MotionSpec.Timed(bad, easing) }
            assertFailsWith<IllegalArgumentException> { MotionSpec.Timed(140, easing).copy(durationMillis = bad) }
        }
        assertEquals(MaxMotionDurationMillis, MotionSpec.Timed(MaxMotionDurationMillis, easing).durationMillis)
        assertEquals(1, MotionSpec.Timed(1, easing).durationMillis)
    }

    @Test
    fun the_easing_token_parser_accepts_the_canonical_value_and_rejects_malformed_ones() {
        assertEquals(MotionEasing(0.2f, 0f, 0f, 1f), parseEasingToken("[0.2,0,0,1]"))
        assertEquals(MotionEasing(0.4f, 0f, 0.2f, 1f), parseEasingToken(" [ 0.4, 0, 0.2, 1 ] "))
        listOf("", "[]", "[0.2,0,0]", "[0.2,0,0,1,1]", "[a,b,c,d]", "0.2,0,0").forEach { bad ->
            assertFailsWith<IllegalArgumentException>("accepted $bad") { parseEasingToken(bad) }
        }
    }

    @Test
    fun preferences_project_the_resolved_motion_fact_and_copy_cannot_desynchronize_it() {
        assertEquals(MotionPreference.Full, UiPreferences().motion)
        assertEquals(MotionPreference.Reduced, UiPreferences(reducedMotion = true).motion)
        assertEquals(MotionPreference.Reduced, UiPreferences().copy(reducedMotion = true).motion)
        assertEquals(MotionPreference.Full, UiPreferences(reducedMotion = true).copy(reducedMotion = false).motion)
    }

    @Test
    fun an_entrance_starts_at_its_final_value_only_when_the_spec_is_instant() {
        assertEquals(1f, MotionSpec.Instant.entranceStart())
        MotionIntent.entries.forEach { intent ->
            MotionProperty.entries.forEach { property ->
                assertEquals(1f, motionFor(intent, property, MotionPreference.Reduced).entranceStart())
                assertEquals(0f, motionFor(intent, property, MotionPreference.Full).entranceStart())
            }
        }
    }

    @Test
    fun the_compose_mapping_snaps_for_instant_and_tweens_the_exact_spec_otherwise() {
        assertIs<SnapSpec<Float>>(MotionSpec.Instant.toAnimationSpec())
        val easing = MotionEasing(0.2f, 0f, 0f, 1f)
        val tween = assertIs<TweenSpec<Float>>(MotionSpec.Timed(140, easing).toAnimationSpec())
        assertEquals(140, tween.durationMillis)
        assertEquals(0, tween.delay)
        assertEquals(CubicBezierEasing(0.2f, 0f, 0f, 1f), tween.easing)
    }
}
