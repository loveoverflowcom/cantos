package com.cantos.ui.preview

import com.cantos.ui.Density
import com.cantos.ui.PreviewExample
import com.cantos.ui.Theme
import com.cantos.ui.ThemeMode
import com.cantos.ui.generated.PreviewCatalog
import com.cantos.ui.preview.workbench.ComponentSampleError
import com.cantos.ui.preview.workbench.ComponentSampleIntent
import com.cantos.ui.preview.workbench.ComponentSampleResult
import com.cantos.ui.preview.workbench.ComponentSampleState
import com.cantos.ui.preview.workbench.ConstraintFrame
import com.cantos.ui.preview.workbench.PreviewLocale
import com.cantos.ui.preview.workbench.PreviewMode
import com.cantos.ui.preview.workbench.PreviewRecipe
import com.cantos.ui.preview.workbench.PreviewRecipeError
import com.cantos.ui.preview.workbench.PreviewRecipeResult
import com.cantos.ui.preview.workbench.PreviewSettings
import com.cantos.ui.preview.workbench.PreviewSnapshot
import com.cantos.ui.preview.workbench.PreviewSnapshotCodec
import com.cantos.ui.preview.workbench.PreviewSnapshotError
import com.cantos.ui.preview.workbench.SnapshotExportResult
import com.cantos.ui.preview.workbench.SnapshotRestoreResult
import com.cantos.ui.preview.workbench.SyntheticPreviewText
import com.cantos.ui.preview.workbench.reduceComponentSample
import com.cantos.ui.preview.workbench.replayComponentSample
import com.cantos.ui.preview.workbench.validateComponentSample
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs

class PreviewWorkbenchTest {
    private fun recipe(
        componentId: String = "text-field",
        stateId: String = "selection",
        settings: PreviewSettings = PreviewSettings(),
        seed: Int = 0,
    ): PreviewRecipe = assertIs<PreviewRecipeResult.Accepted>(
        PreviewRecipe.of(componentId, stateId, PreviewMode.Isolated, seed, settings),
    ).recipe

    @Test
    fun every_recipe_binds_a_real_component_and_catalog_state_and_unknown_values_are_typed() {
        PreviewCatalog.components.forEach { component ->
            component.states.forEach { state ->
                val actual = assertIs<PreviewRecipeResult.Accepted>(PreviewRecipe.of(component.id, state)).recipe
                assertEquals(PreviewExample.fromId(component.id), actual.component)
                assertEquals(state, actual.stateId)
                assertEquals(1, actual.schemaVersion)
            }
        }
        assertEquals(
            PreviewRecipeResult.Rejected(PreviewRecipeError.UnknownComponent("missing")),
            PreviewRecipe.of("missing", "enabled"),
        )
        assertEquals(
            PreviewRecipeResult.Rejected(PreviewRecipeError.UnknownState("button", "readonly")),
            PreviewRecipe.of("button", "readonly"),
        )
        assertEquals(PreviewRecipeResult.Rejected(PreviewRecipeError.InvalidSeed), PreviewRecipe.of("button", "enabled", seed = -1))
    }

    @Test
    fun settings_keep_system_resolution_injected_and_reject_unsupported_frames_and_scales() {
        for (scale in PreviewSettings.FONT_SCALES) {
            val settings = PreviewSettings(ThemeMode.System, Density.Comfortable, PreviewLocale.En, scale,
                ConstraintFrame(320, 720), reducedMotion = true)
            assertEquals(Theme.Light, settings.preferences(false).theme)
            assertEquals(Theme.Dark, settings.preferences(true).theme)
            assertEquals(scale, settings.preferences(false).textScale)
            assertEquals(Density.Comfortable, settings.preferences(false).density)
            assertEquals(true, settings.preferences(false).reducedMotion)
        }
        assertFailsWith<IllegalArgumentException> { ConstraintFrame(0, 720) }
        assertFailsWith<IllegalArgumentException> { ConstraintFrame(390, Int.MAX_VALUE) }
        assertFailsWith<IllegalArgumentException> { PreviewSettings(fontScale = Float.NaN) }
        assertFailsWith<IllegalArgumentException> { PreviewSettings(fontScale = 1.1f) }
    }

    @Test
    fun locale_and_seed_choose_fixed_original_fixtures_without_randomness() {
        assertEquals(SyntheticPreviewText.titleVi, ComponentSampleState.initial(recipe()).title)
        val english = recipe(settings = PreviewSettings(locale = PreviewLocale.En), seed = 5)
        val initial = ComponentSampleState.initial(english)
        assertEquals(SyntheticPreviewText.alternateTitleEn, initial.title)
        assertEquals(SyntheticPreviewText.dialogueEn, initial.dialogue)
        assertEquals(SyntheticPreviewText.richEn, initial.richText)
        assertEquals(initial, ComponentSampleState.initial(english))
        assertEquals(ComponentSampleState.initial(recipe()), ComponentSampleState.initial(PreviewExample.TextField, "selection"))
    }

    @Test
    fun identical_recipe_and_events_produce_exact_widget_projection_including_ime_state() {
        val recipe = recipe()
        val events = listOf(
            ComponentSampleIntent.SetTitle(SyntheticPreviewText.alternateTitleVi),
            ComponentSampleIntent.SetRichText(SyntheticPreviewText.richVi, 9, 2, 2, 9),
            ComponentSampleIntent.SetChecked(true), ComponentSampleIntent.SetTab(2),
            ComponentSampleIntent.SetStableChoice("minh-phuong"), ComponentSampleIntent.SetRange(50f),
            ComponentSampleIntent.Press, ComponentSampleIntent.Press, ComponentSampleIntent.ReverseOptions,
            ComponentSampleIntent.SetInterrupted(true), ComponentSampleIntent.SetDialogOpen(true),
            ComponentSampleIntent.SetDialogNote(SyntheticPreviewText.titleVi),
        )
        val expected = ComponentSampleState.initial(recipe).copy(
            title = SyntheticPreviewText.alternateTitleVi, selectionStart = 9, selectionEnd = 2,
            compositionStart = 2, compositionEnd = 9, checked = true, tab = 2, stableChoice = "minh-phuong",
            range = 50f, presses = 2, reverseOptions = true, interrupted = true, dialogOpen = true,
            dialogNote = SyntheticPreviewText.titleVi,
        )
        repeat(3) { assertEquals(ComponentSampleResult.Accepted(expected), replayComponentSample(recipe, events)) }
        assertEquals(ComponentSampleResult.Accepted(ComponentSampleState.initial(recipe)), replayComponentSample(recipe, emptyList()))
    }

    @Test
    fun invalid_commands_are_rejected_before_value_replacement() {
        val before = ComponentSampleState.initial(recipe())
        val cases = listOf(
            ComponentSampleIntent.SetRichText("", 1, 0) to ComponentSampleError.InvalidSelection,
            ComponentSampleIntent.SetRichText("", 0, 0, 0, null) to ComponentSampleError.InvalidSelection,
            ComponentSampleIntent.SetTab(4) to ComponentSampleError.InvalidTab,
            ComponentSampleIntent.SetRange(Float.NaN) to ComponentSampleError.InvalidRange,
            ComponentSampleIntent.SetRange(-1f) to ComponentSampleError.InvalidRange,
        )
        cases.forEach { (event, rejection) ->
            assertEquals(ComponentSampleResult.Rejected(rejection), reduceComponentSample(before, event))
        }
        assertEquals(ComponentSampleState.initial(recipe()), before)
        assertEquals(ComponentSampleResult.Rejected(ComponentSampleError.PressCountExhausted),
            reduceComponentSample(before.copy(presses = Int.MAX_VALUE), ComponentSampleIntent.Press))
    }

    @Test
    fun snapshot_round_trip_is_versioned_complete_and_detached_from_mutable_input_lists() {
        val recipe = recipe(settings = PreviewSettings(ThemeMode.System, Density.Comfortable, PreviewLocale.En,
            2f, ConstraintFrame(840, 900), reducedMotion = true), seed = 17)
        val events = mutableListOf<ComponentSampleIntent>(
            ComponentSampleIntent.SetRichText(SyntheticPreviewText.richEn, 8, 1, 1, 8),
            ComponentSampleIntent.SetTitle(SyntheticPreviewText.titleEn),
            ComponentSampleIntent.SetDialogue(SyntheticPreviewText.dialogueEn),
            ComponentSampleIntent.SetNumber(SyntheticPreviewText.number),
            ComponentSampleIntent.SetChecked(true), ComponentSampleIntent.SetTab(3),
            ComponentSampleIntent.SetStableChoice(null), ComponentSampleIntent.SetRange(100f),
            ComponentSampleIntent.Press, ComponentSampleIntent.ReverseOptions, ComponentSampleIntent.SetInterrupted(true),
            ComponentSampleIntent.SetDialogOpen(true), ComponentSampleIntent.SetDialogNote(SyntheticPreviewText.dialogNoteEn),
        )
        val sample = assertIs<ComponentSampleResult.Accepted>(replayComponentSample(recipe, events)).state
        val snapshot = PreviewSnapshot(recipe, sample, events)
        val expectedEvents = events.toList()
        events.clear()
        val encoded = assertIs<SnapshotExportResult.Exported>(PreviewSnapshotCodec.encode(snapshot)).payload
        val restored = assertIs<SnapshotRestoreResult.Restored>(PreviewSnapshotCodec.decode(encoded)).snapshot
        assertEquals(recipe, restored.recipe)
        assertEquals(sample, restored.sample)
        assertEquals(expectedEvents, restored.events)
        assertEquals(encoded, assertIs<SnapshotExportResult.Exported>(PreviewSnapshotCodec.encode(restored)).payload)
        assertFalse(encoded.contains(SyntheticPreviewText.titleEn))
    }

    @Test
    fun malformed_unknown_version_and_tampered_projection_are_typed_without_live_fallback() {
        val recipe = recipe()
        val encoded = assertIs<SnapshotExportResult.Exported>(PreviewSnapshotCodec.encode(
            PreviewSnapshot(recipe, ComponentSampleState.initial(recipe), emptyList()),
        )).payload
        assertEquals(SnapshotRestoreResult.Rejected(PreviewSnapshotError.UnsupportedVersion(99)),
            PreviewSnapshotCodec.decode(encoded.replace("PREVIEW|1", "PREVIEW|99")))
        assertEquals(SnapshotRestoreResult.Rejected(PreviewSnapshotError.Malformed), PreviewSnapshotCodec.decode("{}"))
        assertEquals(SnapshotRestoreResult.Rejected(PreviewSnapshotError.ProjectionMismatch),
            PreviewSnapshotCodec.decode(encoded.replace("state|", "state|999|")))
        assertEquals(SnapshotRestoreResult.Rejected(PreviewSnapshotError.InvalidRecipe(PreviewRecipeError.UnknownComponent("unknown"))),
            PreviewSnapshotCodec.decode(encoded.replace("recipe|text-field|", "recipe|unknown|")))
        assertEquals(SnapshotRestoreResult.Rejected(PreviewSnapshotError.Malformed),
            PreviewSnapshotCodec.decode(encoded.replace("|1.0|390|", "|NaN|390|")))
        assertEquals(SnapshotRestoreResult.Rejected(PreviewSnapshotError.Malformed), PreviewSnapshotCodec.decode("$encoded\nevent|backend|start"))
        assertEquals(SnapshotRestoreResult.Rejected(PreviewSnapshotError.LimitExceeded),
            PreviewSnapshotCodec.decode("x".repeat(PreviewSnapshotCodec.MAX_PAYLOAD_CHARS + 1)))
    }

    @Test
    fun export_refuses_freeform_credentials_paths_and_inconsistent_projections() {
        val recipe = recipe()
        for (sentinel in listOf("password=private-sentinel", "/Users/private/manuscript.txt", "https://audio.invalid/?token=private")) {
            val events = listOf(ComponentSampleIntent.SetTitle(sentinel))
            val sample = assertIs<ComponentSampleResult.Accepted>(replayComponentSample(recipe, events)).state
            assertEquals(SnapshotExportResult.Rejected(PreviewSnapshotError.NonSyntheticContent),
                PreviewSnapshotCodec.encode(PreviewSnapshot(recipe, sample, events)))
            val choiceEvents = listOf(ComponentSampleIntent.SetStableChoice(sentinel))
            val choiceSample = assertIs<ComponentSampleResult.Accepted>(replayComponentSample(recipe, choiceEvents)).state
            assertEquals(SnapshotExportResult.Rejected(PreviewSnapshotError.NonSyntheticContent),
                PreviewSnapshotCodec.encode(PreviewSnapshot(recipe, choiceSample, choiceEvents)))
        }
        assertEquals(SnapshotExportResult.Rejected(PreviewSnapshotError.ProjectionMismatch),
            PreviewSnapshotCodec.encode(PreviewSnapshot(recipe, ComponentSampleState.initial(recipe).copy(checked = true), emptyList())))
        assertEquals(SnapshotExportResult.Rejected(PreviewSnapshotError.LimitExceeded),
            PreviewSnapshotCodec.encode(PreviewSnapshot(recipe, ComponentSampleState.initial(recipe),
                List(PreviewSnapshotCodec.MAX_EVENTS + 1) { ComponentSampleIntent.Press })))
    }

    @Test
    fun restored_tabs_cannot_reference_a_tab_outside_the_actual_recipe_labels() {
        val ordinary = recipe("nav-tabs", "idle")
        val before = ComponentSampleState.initial(ordinary)
        assertEquals(ComponentSampleResult.Rejected(ComponentSampleError.InvalidTab),
            reduceComponentSample(ordinary, before, ComponentSampleIntent.SetTab(3)))
        assertEquals(ComponentSampleResult.Rejected(ComponentSampleError.InvalidTab),
            validateComponentSample(ordinary, before.copy(tab = 3)))
        val payload = assertIs<SnapshotExportResult.Exported>(PreviewSnapshotCodec.encode(
            PreviewSnapshot(ordinary, before, emptyList()),
        )).payload
        assertEquals(SnapshotRestoreResult.Rejected(PreviewSnapshotError.InvalidIntent(ComponentSampleError.InvalidTab)),
            PreviewSnapshotCodec.decode("$payload\nevent|tab|3"))
        val overflow = recipe("nav-tabs", "overflow")
        assertEquals(ComponentSampleResult.Accepted(ComponentSampleState.initial(overflow).copy(tab = 3)),
            replayComponentSample(overflow, listOf(ComponentSampleIntent.SetTab(3))))
    }

    @Test
    fun restored_stepped_range_values_must_match_the_actual_component_tick_contract() {
        val stepped = recipe("range-field", "stepped")
        val before = ComponentSampleState.initial(stepped)
        for (tick in listOf(0f, 25f, 50f, 75f, 100f)) {
            assertEquals(ComponentSampleResult.Accepted(before.copy(range = tick)),
                reduceComponentSample(stepped, before, ComponentSampleIntent.SetRange(tick)))
        }
        assertEquals(ComponentSampleResult.Rejected(ComponentSampleError.InvalidRange),
            replayComponentSample(stepped, listOf(ComponentSampleIntent.SetRange(26f))))
        val payload = assertIs<SnapshotExportResult.Exported>(PreviewSnapshotCodec.encode(
            PreviewSnapshot(stepped, before, emptyList()),
        )).payload
        assertEquals(SnapshotRestoreResult.Rejected(PreviewSnapshotError.InvalidIntent(ComponentSampleError.InvalidRange)),
            PreviewSnapshotCodec.decode("$payload\nevent|range|26.0"))
        val continuous = recipe("range-field", "enabled")
        assertEquals(ComponentSampleResult.Accepted(ComponentSampleState.initial(continuous).copy(range = 26f)),
            replayComponentSample(continuous, listOf(ComponentSampleIntent.SetRange(26f))))
    }
}
