package com.cantos.ui.preview.workbench

import com.cantos.ui.PreviewExample
import com.cantos.ui.generated.PreviewCatalog
import kotlin.math.abs
import kotlin.math.ulp

enum class PreviewMode { Catalog, Isolated, StateMatrix }

sealed interface PreviewRecipeError {
    data class UnknownComponent(val id: String) : PreviewRecipeError
    data class UnknownState(val componentId: String, val stateId: String) : PreviewRecipeError
    data object InvalidSeed : PreviewRecipeError
}

sealed interface PreviewRecipeResult {
    data class Accepted(val recipe: PreviewRecipe) : PreviewRecipeResult
    data class Rejected(val error: PreviewRecipeError) : PreviewRecipeResult
}

/** A catalog-bound recipe. Construction and restoration always check the canonical inventory. */
@ConsistentCopyVisibility
data class PreviewRecipe private constructor(
    val component: PreviewExample,
    val stateId: String,
    val mode: PreviewMode,
    val seed: Int,
    val settings: PreviewSettings,
) {
    val schemaVersion: Int get() = 1

    companion object {
        fun of(
            componentId: String,
            stateId: String,
            mode: PreviewMode = PreviewMode.Catalog,
            seed: Int = 0,
            settings: PreviewSettings = PreviewSettings(),
        ): PreviewRecipeResult {
            val component = PreviewExample.fromId(componentId)
                ?: return PreviewRecipeResult.Rejected(PreviewRecipeError.UnknownComponent(componentId))
            val entry = PreviewCatalog.components.firstOrNull { it.id == componentId }
                ?: return PreviewRecipeResult.Rejected(PreviewRecipeError.UnknownComponent(componentId))
            if (stateId !in entry.states) {
                return PreviewRecipeResult.Rejected(PreviewRecipeError.UnknownState(componentId, stateId))
            }
            if (seed < 0) return PreviewRecipeResult.Rejected(PreviewRecipeError.InvalidSeed)
            return PreviewRecipeResult.Accepted(PreviewRecipe(component, stateId, mode, seed, settings))
        }
    }
}

/** Fixed original sample strings. Export encodes IDs from this table, never free-form typed content. */
object SyntheticPreviewText {
    const val narratorVi = "Người dẫn chuyện"
    const val characterNameVi = "Nguyễn Thị Minh Phương"
    const val titleVi = "Ánh đèn cuối sân khấu"
    const val dialogueVi = "Người dẫn chuyện: Ngày mai, mình có diễn tiếp không?\nỖ, Ữ, Nguyễn Thị Minh Phương"
    const val richVi = "Ánh đèn cuối sân khấu · Ỗ, Ữ"
    const val richDialogueVi = "Người dẫn chuyện: Ngày mai, mình có diễn tiếp không?\nỖ, Ữ"
    const val alternateTitleVi = "Ngày mai, mình có diễn tiếp không?"
    const val titleEn = "The last light on stage"
    const val dialogueEn = "Narrator: Will we perform again tomorrow?\nMinh Phương keeps the light on."
    const val richEn = "The last light on stage · Minh Phương"
    const val alternateTitleEn = "Will we perform again tomorrow?"
    const val number = "1,25"
    const val dialogNoteVi = "Nguyễn Thị Minh Phương"
    const val dialogNoteEn = "Minh Phương keeps the stage light on."

    val values: List<String> get() = listOf(
        "", titleVi, dialogueVi, richVi, richDialogueVi, alternateTitleVi,
        titleEn, dialogueEn, richEn, alternateTitleEn, number, dialogNoteVi, dialogNoteEn,
    )
}

/** Synthetic widget values, detached from Compose TextFieldValue and every runtime owner. */
data class ComponentSampleState(
    val title: String = SyntheticPreviewText.titleVi,
    val dialogue: String = SyntheticPreviewText.dialogueVi,
    val number: String = SyntheticPreviewText.number,
    val richText: String = SyntheticPreviewText.richVi,
    val selectionStart: Int = 0,
    val selectionEnd: Int = 7,
    val compositionStart: Int? = null,
    val compositionEnd: Int? = null,
    val checked: Boolean = false,
    val tab: Int = 0,
    val stableChoice: String? = "narrator",
    val range: Float = 45f,
    val presses: Int = 0,
    val reverseOptions: Boolean = false,
    val interrupted: Boolean = false,
    val dialogOpen: Boolean = false,
    val dialogNote: String = SyntheticPreviewText.dialogNoteVi,
) {
    init {
        require(selectionStart in 0..richText.length && selectionEnd in 0..richText.length)
        require((compositionStart == null) == (compositionEnd == null))
        require(compositionStart == null || (compositionStart in 0..richText.length && compositionEnd in 0..richText.length))
        require(tab in 0..3 && range.isFinite() && range in 0f..100f && presses >= 0)
    }

    companion object {
        fun initial(recipe: PreviewRecipe): ComponentSampleState = initial(
            recipe.component, recipe.stateId, recipe.settings.locale, recipe.seed,
        )

        fun initial(
            example: PreviewExample,
            stateId: String,
            locale: PreviewLocale = PreviewLocale.ViVn,
            seed: Int = 0,
        ): ComponentSampleState {
            val english = locale == PreviewLocale.En
            val title = if (seed % 2 == 0) {
                if (english) SyntheticPreviewText.titleEn else SyntheticPreviewText.titleVi
            } else {
                if (english) SyntheticPreviewText.alternateTitleEn else SyntheticPreviewText.alternateTitleVi
            }
            val dialogue = if (english) SyntheticPreviewText.dialogueEn else SyntheticPreviewText.dialogueVi
            val richText = when (example) {
                PreviewExample.NumberField -> SyntheticPreviewText.number
                PreviewExample.TextArea -> if (english) dialogue else SyntheticPreviewText.richDialogueVi
                else -> if (english) SyntheticPreviewText.richEn else SyntheticPreviewText.richVi
            }
            return ComponentSampleState(
                title = title,
                dialogue = dialogue,
                richText = richText,
                selectionEnd = minOf(7, richText.length),
                checked = stateId in listOf("selected", "current"),
                tab = if (stateId == "current") 1 else 0,
                stableChoice = if (stateId == "missing-value") "removed" else "narrator",
                range = when (stateId) {
                    "zero", "minimum" -> 0f
                    "maximum" -> 100f
                    "stepped" -> 50f
                    else -> 45f
                },
                dialogOpen = stateId == "open",
                dialogNote = if (english) SyntheticPreviewText.dialogNoteEn else SyntheticPreviewText.dialogNoteVi,
            )
        }
    }
}

sealed interface ComponentSampleIntent {
    data class SetTitle(val value: String) : ComponentSampleIntent
    data class SetDialogue(val value: String) : ComponentSampleIntent
    data class SetNumber(val value: String) : ComponentSampleIntent
    data class SetRichText(
        val text: String,
        val selectionStart: Int,
        val selectionEnd: Int,
        val compositionStart: Int? = null,
        val compositionEnd: Int? = null,
    ) : ComponentSampleIntent
    data class SetChecked(val value: Boolean) : ComponentSampleIntent
    data class SetTab(val value: Int) : ComponentSampleIntent
    data class SetStableChoice(val value: String?) : ComponentSampleIntent
    data class SetRange(val value: Float) : ComponentSampleIntent
    data object Press : ComponentSampleIntent
    data object ReverseOptions : ComponentSampleIntent
    data class SetInterrupted(val value: Boolean) : ComponentSampleIntent
    data class SetDialogOpen(val value: Boolean) : ComponentSampleIntent
    data class SetDialogNote(val value: String) : ComponentSampleIntent
}

enum class ComponentSampleError { InvalidSelection, InvalidTab, InvalidRange, PressCountExhausted }

sealed interface ComponentSampleResult {
    data class Accepted(val state: ComponentSampleState) : ComponentSampleResult
    data class Rejected(val error: ComponentSampleError) : ComponentSampleResult
}

/** Pure value replacement: replay never fires a callback, accesses a service or starts a job. */
fun reduceComponentSample(state: ComponentSampleState, intent: ComponentSampleIntent): ComponentSampleResult {
    val next = when (intent) {
        is ComponentSampleIntent.SetTitle -> state.copy(title = intent.value)
        is ComponentSampleIntent.SetDialogue -> state.copy(dialogue = intent.value)
        is ComponentSampleIntent.SetNumber -> state.copy(number = intent.value)
        is ComponentSampleIntent.SetRichText -> {
            if (intent.selectionStart !in 0..intent.text.length || intent.selectionEnd !in 0..intent.text.length ||
                (intent.compositionStart == null) != (intent.compositionEnd == null) ||
                (intent.compositionStart != null &&
                    (intent.compositionStart !in 0..intent.text.length || intent.compositionEnd !in 0..intent.text.length))
            ) return ComponentSampleResult.Rejected(ComponentSampleError.InvalidSelection)
            state.copy(
                richText = intent.text, selectionStart = intent.selectionStart, selectionEnd = intent.selectionEnd,
                compositionStart = intent.compositionStart, compositionEnd = intent.compositionEnd,
            )
        }
        is ComponentSampleIntent.SetChecked -> state.copy(checked = intent.value)
        is ComponentSampleIntent.SetTab -> {
            if (intent.value !in 0..3) return ComponentSampleResult.Rejected(ComponentSampleError.InvalidTab)
            state.copy(tab = intent.value)
        }
        is ComponentSampleIntent.SetStableChoice -> state.copy(stableChoice = intent.value)
        is ComponentSampleIntent.SetRange -> {
            if (!intent.value.isFinite() || intent.value !in 0f..100f) {
                return ComponentSampleResult.Rejected(ComponentSampleError.InvalidRange)
            }
            state.copy(range = intent.value)
        }
        ComponentSampleIntent.Press -> {
            if (state.presses == Int.MAX_VALUE) return ComponentSampleResult.Rejected(ComponentSampleError.PressCountExhausted)
            state.copy(presses = state.presses + 1)
        }
        ComponentSampleIntent.ReverseOptions -> state.copy(reverseOptions = !state.reverseOptions)
        is ComponentSampleIntent.SetInterrupted -> state.copy(interrupted = intent.value)
        is ComponentSampleIntent.SetDialogOpen -> state.copy(dialogOpen = intent.value)
        is ComponentSampleIntent.SetDialogNote -> state.copy(dialogNote = intent.value)
    }
    return ComponentSampleResult.Accepted(next)
}

/** Relations that the mounted component, rather than a generic widget record, requires. */
fun validateComponentSample(recipe: PreviewRecipe, state: ComponentSampleState): ComponentSampleResult {
    if (recipe.component == PreviewExample.NavTabs) {
        val lastTab = if (recipe.stateId == "overflow") 3 else 2
        if (state.tab !in 0..lastTab) return ComponentSampleResult.Rejected(ComponentSampleError.InvalidTab)
    }
    if (recipe.component == PreviewExample.RangeField && recipe.stateId == "stepped") {
        // The real sample uses three interior ticks over 0..100. Preserve its one-ULP contract.
        val ticks = listOf(0f, 25f, 50f, 75f, 100f)
        if (ticks.none { abs(state.range - it) <= maxOf(state.range.ulp, it.ulp) }) {
            return ComponentSampleResult.Rejected(ComponentSampleError.InvalidRange)
        }
    }
    return ComponentSampleResult.Accepted(state)
}

/** Validate the complete candidate before publishing it to a real component. */
fun reduceComponentSample(
    recipe: PreviewRecipe,
    state: ComponentSampleState,
    intent: ComponentSampleIntent,
): ComponentSampleResult = when (val next = reduceComponentSample(state, intent)) {
    is ComponentSampleResult.Accepted -> validateComponentSample(recipe, next.state)
    is ComponentSampleResult.Rejected -> next
}

fun replayComponentSample(recipe: PreviewRecipe, events: List<ComponentSampleIntent>): ComponentSampleResult {
    var state = ComponentSampleState.initial(recipe)
    for (event in events) {
        when (val next = reduceComponentSample(recipe, state, event)) {
            is ComponentSampleResult.Accepted -> state = next.state
            is ComponentSampleResult.Rejected -> return next
        }
    }
    return ComponentSampleResult.Accepted(state)
}
