package com.cantos.ui.preview.workbench

import com.cantos.ui.Density
import com.cantos.ui.ThemeMode

/** Detached synthetic values only. No coroutine, controller, port or native object is serializable here. */
class PreviewSnapshot(
    val recipe: PreviewRecipe,
    val sample: ComponentSampleState,
    events: List<ComponentSampleIntent>,
) {
    private val storedEvents = events.toList()
    val events: List<ComponentSampleIntent> get() = storedEvents.toList()
    val schemaVersion: Int get() = 1
}

sealed interface PreviewSnapshotError {
    data object Malformed : PreviewSnapshotError
    data class UnsupportedVersion(val version: Int) : PreviewSnapshotError
    data class InvalidRecipe(val error: PreviewRecipeError) : PreviewSnapshotError
    data object LimitExceeded : PreviewSnapshotError
    data object NonSyntheticContent : PreviewSnapshotError
    data object ProjectionMismatch : PreviewSnapshotError
    data class InvalidIntent(val error: ComponentSampleError) : PreviewSnapshotError
}

sealed interface SnapshotExportResult {
    data class Exported(val payload: String) : SnapshotExportResult
    data class Rejected(val error: PreviewSnapshotError) : SnapshotExportResult
}

sealed interface SnapshotRestoreResult {
    data class Restored(val snapshot: PreviewSnapshot) : SnapshotRestoreResult
    data class Rejected(val error: PreviewSnapshotError) : SnapshotRestoreResult
}

/**
 * Strict, bounded v1 interchange. Text is represented by a fixed original fixture ID, so export
 * cannot accidentally retain passwords, private paths, signed URLs or manuscripts typed into a sample.
 * Restore validates first; this pure codec has no fallback adapter, service or effect interpreter.
 */
object PreviewSnapshotCodec {
    const val MAX_EVENTS: Int = 256
    const val MAX_PAYLOAD_CHARS: Int = 16_384
    private const val HEADER = "CANTOS-CMP-PREVIEW"

    fun encode(snapshot: PreviewSnapshot): SnapshotExportResult {
        val events = snapshot.events
        if (events.size > MAX_EVENTS) return SnapshotExportResult.Rejected(PreviewSnapshotError.LimitExceeded)
        val eventLines = mutableListOf<String>()
        for (event in events) {
            eventLines += encodeEvent(event)
                ?: return SnapshotExportResult.Rejected(PreviewSnapshotError.NonSyntheticContent)
        }
        val sampleLine = encodeSample(snapshot.sample)
            ?: return SnapshotExportResult.Rejected(PreviewSnapshotError.NonSyntheticContent)
        when (val replay = replayComponentSample(snapshot.recipe, events)) {
            is ComponentSampleResult.Rejected -> return SnapshotExportResult.Rejected(PreviewSnapshotError.InvalidIntent(replay.error))
            is ComponentSampleResult.Accepted -> if (replay.state != snapshot.sample) {
                return SnapshotExportResult.Rejected(PreviewSnapshotError.ProjectionMismatch)
            }
        }
        val payload = (listOf("$HEADER|1", encodeRecipe(snapshot.recipe), sampleLine) + eventLines).joinToString("\n")
        if (payload.length > MAX_PAYLOAD_CHARS) return SnapshotExportResult.Rejected(PreviewSnapshotError.LimitExceeded)
        return SnapshotExportResult.Exported(payload)
    }

    fun decode(payload: String): SnapshotRestoreResult {
        if (payload.length > MAX_PAYLOAD_CHARS) return SnapshotRestoreResult.Rejected(PreviewSnapshotError.LimitExceeded)
        val lines = payload.split('\n')
        if (lines.size < 3) return SnapshotRestoreResult.Rejected(PreviewSnapshotError.Malformed)
        if (lines.size > MAX_EVENTS + 3) return SnapshotRestoreResult.Rejected(PreviewSnapshotError.LimitExceeded)
        val header = lines[0].split('|')
        if (header.size != 2 || header[0] != HEADER) return SnapshotRestoreResult.Rejected(PreviewSnapshotError.Malformed)
        val version = header[1].toIntOrNull() ?: return SnapshotRestoreResult.Rejected(PreviewSnapshotError.Malformed)
        if (version != 1) return SnapshotRestoreResult.Rejected(PreviewSnapshotError.UnsupportedVersion(version))
        val recipe = when (val result = decodeRecipe(lines[1])) {
            is RecipeDecode.Accepted -> result.recipe
            is RecipeDecode.Rejected -> return SnapshotRestoreResult.Rejected(result.error)
        }
        val events = mutableListOf<ComponentSampleIntent>()
        for (line in lines.drop(3)) {
            events += decodeEvent(line) ?: return SnapshotRestoreResult.Rejected(PreviewSnapshotError.Malformed)
        }
        val sample = when (val replay = replayComponentSample(recipe, events)) {
            is ComponentSampleResult.Accepted -> replay.state
            is ComponentSampleResult.Rejected -> return SnapshotRestoreResult.Rejected(PreviewSnapshotError.InvalidIntent(replay.error))
        }
        if (encodeSample(sample) != lines[2]) return SnapshotRestoreResult.Rejected(PreviewSnapshotError.ProjectionMismatch)
        return SnapshotRestoreResult.Restored(PreviewSnapshot(recipe, sample, events))
    }

    private fun encodeRecipe(recipe: PreviewRecipe): String = with(recipe.settings) {
        listOf(
            "recipe", recipe.component.id, recipe.stateId, recipe.mode.name, recipe.seed.toString(),
            themeMode.name, density.name, locale.name, fontScale.toString(),
            constraints.widthDp.toString(), constraints.heightDp.toString(), flag(reducedMotion),
        ).joinToString("|")
    }

    private sealed interface RecipeDecode {
        data class Accepted(val recipe: PreviewRecipe) : RecipeDecode
        data class Rejected(val error: PreviewSnapshotError) : RecipeDecode
    }

    private fun decodeRecipe(line: String): RecipeDecode {
        val fields = line.split('|')
        if (fields.size != 12 || fields[0] != "recipe") return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        val mode = PreviewMode.entries.firstOrNull { it.name == fields[3] }
            ?: return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        val seed = fields[4].toIntOrNull() ?: return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        val theme = ThemeMode.entries.firstOrNull { it.name == fields[5] }
            ?: return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        val density = Density.entries.firstOrNull { it.name == fields[6] }
            ?: return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        val locale = PreviewLocale.entries.firstOrNull { it.name == fields[7] }
            ?: return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        val scale = fields[8].toFloatOrNull() ?: return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        val width = fields[9].toIntOrNull() ?: return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        val height = fields[10].toIntOrNull() ?: return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        val reduced = boolean(fields[11]) ?: return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        if (scale !in PreviewSettings.FONT_SCALES || width !in ConstraintFrame.WIDTH_RANGE || height !in ConstraintFrame.HEIGHT_RANGE) {
            return RecipeDecode.Rejected(PreviewSnapshotError.Malformed)
        }
        return when (val recipe = PreviewRecipe.of(
            fields[1], fields[2], mode, seed,
            PreviewSettings(theme, density, locale, scale, ConstraintFrame(width, height), reduced),
        )) {
            is PreviewRecipeResult.Accepted -> RecipeDecode.Accepted(recipe.recipe)
            is PreviewRecipeResult.Rejected -> RecipeDecode.Rejected(PreviewSnapshotError.InvalidRecipe(recipe.error))
        }
    }

    private fun encodeSample(sample: ComponentSampleState): String? {
        val title = textId(sample.title) ?: return null
        val dialogue = textId(sample.dialogue) ?: return null
        val number = textId(sample.number) ?: return null
        val rich = textId(sample.richText) ?: return null
        val choice = choiceId(sample.stableChoice) ?: return null
        val dialogNote = textId(sample.dialogNote) ?: return null
        return listOf(
            "state", title, dialogue, number, rich, sample.selectionStart, sample.selectionEnd,
            sample.compositionStart ?: "-", sample.compositionEnd ?: "-", flag(sample.checked), sample.tab,
            choice, sample.range, sample.presses, flag(sample.reverseOptions), flag(sample.interrupted),
            flag(sample.dialogOpen), dialogNote,
        ).joinToString("|")
    }

    private fun encodeEvent(event: ComponentSampleIntent): String? = when (event) {
        is ComponentSampleIntent.SetTitle -> textId(event.value)?.let { "event|title|$it" }
        is ComponentSampleIntent.SetDialogue -> textId(event.value)?.let { "event|dialogue|$it" }
        is ComponentSampleIntent.SetNumber -> textId(event.value)?.let { "event|number|$it" }
        is ComponentSampleIntent.SetRichText -> textId(event.text)?.let {
            "event|rich|$it|${event.selectionStart}|${event.selectionEnd}|${event.compositionStart ?: "-"}|${event.compositionEnd ?: "-"}"
        }
        is ComponentSampleIntent.SetChecked -> "event|checked|${flag(event.value)}"
        is ComponentSampleIntent.SetTab -> "event|tab|${event.value}"
        is ComponentSampleIntent.SetStableChoice -> choiceId(event.value)?.let { "event|choice|$it" }
        is ComponentSampleIntent.SetRange -> "event|range|${event.value}"
        ComponentSampleIntent.Press -> "event|press"
        ComponentSampleIntent.ReverseOptions -> "event|reverse"
        is ComponentSampleIntent.SetInterrupted -> "event|interrupted|${flag(event.value)}"
        is ComponentSampleIntent.SetDialogOpen -> "event|dialog-open|${flag(event.value)}"
        is ComponentSampleIntent.SetDialogNote -> textId(event.value)?.let { "event|dialog-note|$it" }
    }

    private fun decodeEvent(line: String): ComponentSampleIntent? {
        val fields = line.split('|')
        if (fields.size < 2 || fields[0] != "event") return null
        if (fields[1] == "press" && fields.size == 2) return ComponentSampleIntent.Press
        if (fields[1] == "reverse" && fields.size == 2) return ComponentSampleIntent.ReverseOptions
        if (fields[1] == "rich" && fields.size == 7) {
            val text = text(fields[2]) ?: return null
            val start = fields[3].toIntOrNull() ?: return null
            val end = fields[4].toIntOrNull() ?: return null
            val compositionStart = if (fields[5] == "-") null else fields[5].toIntOrNull() ?: return null
            val compositionEnd = if (fields[6] == "-") null else fields[6].toIntOrNull() ?: return null
            return ComponentSampleIntent.SetRichText(text, start, end, compositionStart, compositionEnd)
        }
        if (fields.size != 3) return null
        return when (fields[1]) {
            "title" -> text(fields[2])?.let(ComponentSampleIntent::SetTitle)
            "dialogue" -> text(fields[2])?.let(ComponentSampleIntent::SetDialogue)
            "number" -> text(fields[2])?.let(ComponentSampleIntent::SetNumber)
            "checked" -> boolean(fields[2])?.let(ComponentSampleIntent::SetChecked)
            "tab" -> fields[2].toIntOrNull()?.let(ComponentSampleIntent::SetTab)
            "choice" -> if (fields[2] == "-") ComponentSampleIntent.SetStableChoice(null)
                else choices.firstOrNull { it == fields[2] }?.let(ComponentSampleIntent::SetStableChoice)
            "range" -> fields[2].toFloatOrNull()?.let(ComponentSampleIntent::SetRange)
            "interrupted" -> boolean(fields[2])?.let(ComponentSampleIntent::SetInterrupted)
            "dialog-open" -> boolean(fields[2])?.let(ComponentSampleIntent::SetDialogOpen)
            "dialog-note" -> text(fields[2])?.let(ComponentSampleIntent::SetDialogNote)
            else -> null
        }
    }

    private val choices: List<String> get() = listOf("narrator", "minh-phuong", "unavailable", "removed")
    private fun choiceId(value: String?): String? = if (value == null) "-" else choices.firstOrNull { it == value }
    private fun textId(value: String): Int? = SyntheticPreviewText.values.indexOf(value).takeIf { it >= 0 }
    private fun text(id: String): String? = id.toIntOrNull()?.let { SyntheticPreviewText.values.getOrNull(it) }
    private fun flag(value: Boolean): String = if (value) "1" else "0"
    private fun boolean(value: String): Boolean? = when (value) { "1" -> true; "0" -> false; else -> null }
}
