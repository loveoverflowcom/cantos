package com.cantos.ui.preview

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.cantos.ui.Button
import com.cantos.ui.ButtonVariant
import com.cantos.ui.CantosTheme
import com.cantos.ui.Density
import com.cantos.ui.ErrorState
import com.cantos.ui.FieldHelp
import com.cantos.ui.FieldLabel
import com.cantos.ui.FieldState
import com.cantos.ui.FilterChip
import com.cantos.ui.Select
import com.cantos.ui.SelectOption
import com.cantos.ui.Surface
import com.cantos.ui.SurfaceTone
import com.cantos.ui.TextArea
import com.cantos.ui.TextField
import com.cantos.ui.ThemeMode
import com.cantos.ui.generated.PreviewCatalog
import com.cantos.ui.preview.workbench.ComponentSampleIntent
import com.cantos.ui.preview.workbench.ComponentSampleResult
import com.cantos.ui.preview.workbench.ComponentSampleState
import com.cantos.ui.preview.workbench.ConstraintFrame
import com.cantos.ui.preview.workbench.PreviewLocale
import com.cantos.ui.preview.workbench.PreviewMode
import com.cantos.ui.preview.workbench.PreviewRecipe
import com.cantos.ui.preview.workbench.PreviewRecipeResult
import com.cantos.ui.preview.workbench.PreviewSettings
import com.cantos.ui.preview.workbench.PreviewSnapshot
import com.cantos.ui.preview.workbench.PreviewSnapshotCodec
import com.cantos.ui.preview.workbench.SnapshotExportResult
import com.cantos.ui.preview.workbench.SnapshotRestoreResult
import com.cantos.ui.preview.workbench.reduceComponentSample
import com.cantos.ui.preview.workbench.replayComponentSample
import com.cantos.ui.preview.workbench.validateComponentSample
import com.cantos.ui.tokenDp

/** Preview-only orchestration; all specimens mount the gallery's actual component renderer. */
@Composable
fun ComponentWorkbench(initialRecipe: PreviewRecipe? = null, systemIsDark: Boolean = false, modifier: Modifier = Modifier) {
    val initial = remember {
        initialRecipe ?: (PreviewRecipe.of("button", "enabled", PreviewMode.Isolated) as? PreviewRecipeResult.Accepted)?.recipe
    }
    if (initial == null) {
        ErrorState("Recipe unavailable", "The canonical button specimen is missing.")
        return
    }
    var recipe by remember { mutableStateOf(initial) }
    var sample by remember { mutableStateOf(ComponentSampleState.initial(initial)) }
    var events by remember { mutableStateOf(listOf<ComponentSampleIntent>()) }
    var epoch by remember { mutableStateOf(0L) }
    var payload by remember { mutableStateOf("") }
    var status by remember { mutableStateOf("Recipe ready. Only synthetic sample data can be exported.") }
    var filter by remember { mutableStateOf("") }
    val scroll = rememberScrollState()
    val changeRecipe: (String, String, PreviewMode, Int, PreviewSettings) -> Unit = { id, state, mode, seed, settings ->
        when (val next = PreviewRecipe.of(id, state, mode, seed, settings)) {
            is PreviewRecipeResult.Accepted -> {
                recipe = next.recipe
                sample = ComponentSampleState.initial(next.recipe)
                events = emptyList()
                epoch++
                status = "Selected ${next.recipe.component.id}/$state · seed $seed"
            }
            is PreviewRecipeResult.Rejected -> status = "Recipe rejected: ${next.error}"
        }
    }
    val dispatch: (ComponentSampleIntent) -> Unit = { intent ->
        if (events.size >= MaxRecordedIntents) status = "Event budget reached. Reset before recording more intents."
        else when (val next = reduceComponentSample(recipe, sample, intent)) {
            is ComponentSampleResult.Accepted -> { sample = next.state; events = events + intent }
            is ComponentSampleResult.Rejected -> status = "Intent rejected: ${next.error}"
        }
    }
    Column(modifier.verticalScroll(scroll), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Text("Scenario workbench", style = MaterialTheme.typography.titleLarge)
        FieldHelp("Preview UI · action replay · no network, audio or persistence. Snapshot import never constructs live adapters.")
        TextField("Filter recipes", filter, { filter = it })
        val components = PreviewCatalog.components.filter {
            it.id.contains(filter.trim(), ignoreCase = true) || it.name.contains(filter.trim(), ignoreCase = true)
        }
        if (components.isNotEmpty()) {
            Select("Recipe component", components.map { SelectOption(it.id, it.name) }, recipe.component.id, { id ->
                val selected = components.first { it.id == id }
                changeRecipe(selected.id, selected.states.first(), recipe.mode, recipe.seed, recipe.settings)
            }, expandedDescription = "Recipe components open", collapsedDescription = "Recipe components closed",
                placeholder = "Mounted recipe hidden by filter", emptyDescription = "No matching recipe")
        } else FieldHelp("No matches. The mounted recipe and field selection are retained.")
        val component = PreviewCatalog.components.first { it.id == recipe.component.id }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            component.states.forEach { state ->
                FilterChip(state, recipe.stateId == state, { changeRecipe(component.id, state, recipe.mode, recipe.seed, recipe.settings) })
            }
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            PreviewMode.entries.forEach { mode ->
                FilterChip(mode.name, recipe.mode == mode, {
                    // Changing presentation preserves the current sample and event sequence.
                    val next = PreviewRecipe.of(component.id, recipe.stateId, mode, recipe.seed, recipe.settings)
                    if (next is PreviewRecipeResult.Accepted) recipe = next.recipe
                })
            }
            listOf(0, 1).forEach { seed -> FilterChip("Seed $seed", recipe.seed == seed, {
                changeRecipe(component.id, recipe.stateId, recipe.mode, seed, recipe.settings)
            }) }
        }
        RecipeEnvironmentControls(recipe.settings) { settings ->
            // Locale changes the synthetic fixture. Other appearance edits preserve live input.
            if (settings.locale != recipe.settings.locale) {
                changeRecipe(component.id, recipe.stateId, recipe.mode, recipe.seed, settings)
            } else {
                val next = PreviewRecipe.of(component.id, recipe.stateId, recipe.mode, recipe.seed, settings)
                if (next is PreviewRecipeResult.Accepted) recipe = next.recipe
            }
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Button("Reset to selected", { sample = ComponentSampleState.initial(recipe); events = emptyList(); epoch++; status = "Reset selected recipe" })
            Button("Recreate", { epoch++; status = "Recreated mount; detached UI projection retained" }, variant = ButtonVariant.Secondary)
            Button("Replay intents", {
                when (val next = replayComponentSample(recipe, events)) {
                    is ComponentSampleResult.Accepted -> { sample = next.state; epoch++; status = "Replayed ${events.size} intents" }
                    is ComponentSampleResult.Rejected -> status = "Replay rejected: ${next.error}"
                }
            }, variant = ButtonVariant.Secondary)
            Button("Export snapshot", {
                when (val result = PreviewSnapshotCodec.encode(PreviewSnapshot(recipe, sample, events))) {
                    is SnapshotExportResult.Exported -> { payload = result.payload; status = "Synthetic snapshot exported" }
                    is SnapshotExportResult.Rejected -> status = "Export rejected: ${result.error}. Reset removes free-form input."
                }
            }, variant = ButtonVariant.Text)
            Button("Restore snapshot", {
                when (val result = PreviewSnapshotCodec.decode(payload)) {
                    is SnapshotRestoreResult.Restored -> {
                        recipe = result.snapshot.recipe; sample = result.snapshot.sample; events = result.snapshot.events
                        epoch++; status = "Restored validated synthetic snapshot"
                    }
                    is SnapshotRestoreResult.Rejected -> status = "Restore rejected: ${result.error}"
                }
            }, variant = ButtonVariant.Text)
        }
        FieldHelp(status)
        TextArea("Snapshot payload", payload, { payload = it }, minLines = 2, maxLines = 5,
            help = "Versioned synthetic values and intents only. Copy/paste explicitly; no filesystem export.")
        // The fixed frame may be wider than this window. Scroll it instead of silently shrinking it.
        Box(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())) {
            key(epoch) {
                val mountedEpoch = epoch
                RecipeFrame(recipe, sample, { if (mountedEpoch == epoch) dispatch(it) }, systemIsDark, interactive = true)
            }
        }
        SelectionContainer {
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                FieldLabel("Read-only projection · epoch $epoch")
                TextArea("Projection", sample.toString(), {}, state = FieldState.ReadOnly, minLines = 2, maxLines = 5)
                FieldLabel("Intent log · ${events.size}")
                // Intent kind only; free-form input never enters the diagnostic log.
                FieldHelp(events.joinToString(" → ") { it::class.simpleName ?: "Intent" }.ifEmpty { "No intents" })
            }
        }
    }
}

/** Named preview budget: bounds in-memory replay/log cost; it is not a product token. */
private const val MaxRecordedIntents = 256

@Composable
private fun RecipeEnvironmentControls(settings: PreviewSettings, onChange: (PreviewSettings) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        ThemeMode.entries.forEach { mode -> FilterChip("Frame ${mode.name}", settings.themeMode == mode, { onChange(settings.copy(themeMode = mode)) }) }
        Density.entries.forEach { density -> FilterChip(density.name, settings.density == density, { onChange(settings.copy(density = density)) }) }
        PreviewSettings.FONT_SCALES.forEach { scale -> FilterChip("Frame text $scale×", settings.fontScale == scale, { onChange(settings.copy(fontScale = scale)) }) }
        PreviewLocale.entries.forEach { locale -> FilterChip("Fixture ${locale.name}", settings.locale == locale, { onChange(settings.copy(locale = locale)) }) }
        FilterChip("Frame reduced motion", settings.reducedMotion, { onChange(settings.copy(reducedMotion = it)) })
        listOf(ViewportPreset.Small, ViewportPreset.Typical, ViewportPreset.Medium, ViewportPreset.Expanded).forEach { preset ->
            FilterChip("Frame ${preset.label}", settings.constraints.widthDp == preset.widthDp.toInt(), {
                onChange(settings.copy(constraints = ConstraintFrame(preset.widthDp.toInt(), settings.constraints.heightDp)))
            })
        }
    }
}

/** Static roots can mount this immutable recipe without creating an owner, job or port. */
@Composable
fun RecipeFrame(
    recipe: PreviewRecipe,
    sample: ComponentSampleState = ComponentSampleState.initial(recipe),
    onIntent: (ComponentSampleIntent) -> Unit = {},
    systemIsDark: Boolean = false,
    /** Static Android roots keep false; only an interactive host requests actual keyboard focus. */
    interactive: Boolean = false,
) {
    val frame = recipe.settings.constraints
    CantosTheme(recipe.settings.preferences(systemIsDark)) {
        Surface(Modifier.requiredSize(frame.widthDp.dp, frame.heightDp.dp).testTag("recipe-frame"), tone = SurfaceTone.Surface) {
            if (validateComponentSample(recipe, sample) is ComponentSampleResult.Rejected) {
                ErrorState("Invalid sample", "The projection is incompatible with this component state. Select a validated recipe.")
                return@Surface
            }
            Column(Modifier.verticalScroll(rememberScrollState()).padding(tokenDp("space.md")),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                val density = LocalDensity.current
                FieldHelp("Constraint frame ${frame.widthDp}×${frame.heightDp} dp · ${density.density} px/dp · font ${density.fontScale} · ${recipe.settings.locale} · ${recipe.settings.themeMode}")
                if (recipe.mode == PreviewMode.Catalog) {
                    Text(PreviewCatalog.components.first { it.id == recipe.component.id }.name, style = MaterialTheme.typography.titleLarge)
                    FieldHelp("${recipe.component.id}/${recipe.stateId} · seed ${recipe.seed} · registration is not qualification")
                }
                if (recipe.mode == PreviewMode.StateMatrix) {
                    FieldHelp("Immutable state matrix. Operate modal and focus journeys in Isolated mode.")
                    PreviewCatalog.components.first { it.id == recipe.component.id }.states.forEach { state ->
                        FieldLabel(state)
                        if (recipe.component == com.cantos.ui.PreviewExample.Dialog && state == "open") {
                            FieldHelp("Open this modal in Isolated mode; an immutable modal would block the matrix controls.")
                            return@forEach
                        }
                        // State matrix is immutable; the interactive isolated specimen records intents.
                        val next = PreviewRecipe.of(recipe.component.id, state, PreviewMode.Isolated, recipe.seed, recipe.settings)
                        if (next is PreviewRecipeResult.Accepted) {
                            ComponentExample(recipe.component, state, {}, Modifier,
                                sampleState = ComponentSampleState.initial(next.recipe), onSampleIntent = {})
                        }
                    }
                } else {
                    val focus = remember { FocusRequester() }
                    ComponentExample(recipe.component, recipe.stateId, {}, Modifier.focusRequester(focus), sample, onIntent)
                    if (interactive && recipe.stateId == "focus") {
                        LaunchedEffect(recipe.component, recipe.stateId) { focus.requestFocus() }
                    }
                }
            }
        }
    }
}
