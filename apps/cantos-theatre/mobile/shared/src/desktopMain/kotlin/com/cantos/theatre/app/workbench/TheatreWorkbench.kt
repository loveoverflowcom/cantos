package com.cantos.theatre.app.workbench

import androidx.compose.foundation.background
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.cantos.theatre.app.CantosDemoApp
import com.cantos.theatre.app.DemoAppSession
import com.cantos.theatre.demo.DemoState
import com.cantos.ui.Button
import com.cantos.ui.ButtonVariant
import com.cantos.ui.Density
import com.cantos.ui.MotionPreference
import com.cantos.ui.TextField
import com.cantos.ui.Surface
import com.cantos.ui.Space
import com.cantos.ui.ThemeMode
import com.cantos.ui.UiPreferences
import com.cantos.ui.CantosTheme
import com.cantos.ui.tokenColor
import com.cantos.ui.preview.workbench.ConstraintFrame
import com.cantos.ui.preview.workbench.PreviewLocale
import com.cantos.ui.tokenDp

// Diagnostic controls need a stable reading column, separate from the declared content frame.
private val WorkbenchControlWidth = 360.dp

/** Actual Theatre composition inside a declared frame; desktop controls remain outside it. */
@Composable
fun TheatreWorkbench(controller: TheatreWorkbenchController? = null) {
    val scope = rememberCoroutineScope()
    val systemIsDark = isSystemInDarkTheme()
    val owned = remember(controller) { controller ?: TheatreWorkbenchController(scope, systemIsDark = systemIsDark) }
    DisposableEffect(owned, controller) { onDispose { if (controller == null) owned.close() } }
    SideEffect { owned.updateSystemTheme(systemIsDark) }
    val mount by owned.mount.collectAsState()
    val log by owned.eventLog.collectAsState()
    val snapshot = observeSnapshot(mount.session)
    var transfer by remember { mutableStateOf("") }
    var outcome by remember { mutableStateOf("Snapshot contains synthetic Theatre UI facts only.") }
    val recipe by owned.recipe.collectAsState()
    val settings = recipe.settings
    CantosTheme(UiPreferences()) {
        Surface(Modifier.fillMaxSize().background(tokenColor("color.surface")), inset = Space.None) {
            Row(Modifier.fillMaxSize().padding(tokenDp("space.md")), horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                Column(Modifier.width(WorkbenchControlWidth).verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                    Text("Cantos · scenario workbench", style = MaterialTheme.typography.headlineSmall)
                    Text("Desktop simulation · seed ${recipe.seed} · ${recipe.id}", style = MaterialTheme.typography.bodySmall)
                    Text("Fixture", style = MaterialTheme.typography.titleMedium)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                        TheatreFixture.entries.forEach { fixture ->
                            Button(fixture.name, { owned.select(TheatreRecipe.fixture(fixture, settings)) },
                                variant = if (recipe.fixture == fixture) ButtonVariant.Tonal else ButtonVariant.Text)
                        }
                    }
                    Text("Theme / constraints / preferences", style = MaterialTheme.typography.titleMedium)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                        ThemeMode.entries.forEach { mode -> Button(mode.name, {
                            owned.updateSettings(settings.copy(themeMode = mode))
                        }, variant = if (settings.themeMode == mode) ButtonVariant.Tonal else ButtonVariant.Text) }
                        listOf(1f, 1.3f, 2f).forEach { scale -> Button("${scale}×", {
                            owned.updateSettings(settings.copy(fontScale = scale))
                        }, variant = if (settings.fontScale == scale) ButtonVariant.Tonal else ButtonVariant.Text) }
                        listOf(320, 390, 840).forEach { width -> Button("$width dp", {
                            owned.updateSettings(settings.copy(constraints = ConstraintFrame(width, settings.constraints.heightDp)))
                        }, variant = if (settings.constraints.widthDp == width) ButtonVariant.Tonal else ButtonVariant.Text) }
                        Density.entries.forEach { density -> Button(density.name, {
                            owned.updateSettings(settings.copy(density = density))
                        }, variant = if (settings.density == density) ButtonVariant.Tonal else ButtonVariant.Text) }
                        PreviewLocale.entries.forEach { locale -> Button(locale.name, {
                            owned.updateSettings(settings.copy(locale = locale))
                        }, variant = if (settings.locale == locale) ButtonVariant.Tonal else ButtonVariant.Text) }
                        Button(if (settings.reducedMotion) "Motion: reduced" else "Motion: full", {
                            owned.updateSettings(settings.copy(reducedMotion = !settings.reducedMotion))
                        }, variant = ButtonVariant.Text)
                        Button("Write delay: ${recipe.writeLatencyMs} ms", {
                            owned.select(recipe.withLatency(if (recipe.writeLatencyMs == 0L) 500L else 0L))
                        }, variant = ButtonVariant.Text)
                    }
                    Text("Appearance preserves accepted UI values. Fixture selection and Reset create a fresh mount.", style = MaterialTheme.typography.bodySmall)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                        Button("Reset to selected", owned::resetToSelected, variant = ButtonVariant.Secondary,
                            controlModifier = Modifier.testTag("workbench-reset"))
                        Button("Recreate owners", owned::recreate, variant = ButtonVariant.Secondary)
                        Button("Replay intents", owned::replay)
                    }
                    Text("Script: ${recipe.steps.joinToString { it.name }} · 50 ms simulation steps", style = MaterialTheme.typography.bodySmall)
                    Text("Readonly projection", style = MaterialTheme.typography.titleMedium)
                    Text("route=${snapshot.route} · episode=${snapshot.selectedEpisode}\nposition=${snapshot.positionMs} ms · saved=${snapshot.savedWorks.size}\nquery=${snapshot.query} · tab=${snapshot.libraryTab} · timer=${snapshot.timer}",
                        Modifier.testTag("workbench-snapshot"), style = MaterialTheme.typography.bodySmall)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                        Button("Export snapshot", {
                            when (val exported = encodeTheatreSnapshot(owned.snapshot())) {
                                is TheatreSnapshotExport.Exported -> { transfer = exported.text; outcome = "Exported version 1 synthetic UI snapshot." }
                                is TheatreSnapshotExport.UnsupportedState -> outcome = exported.reason
                            }
                        }, variant = ButtonVariant.Text, controlModifier = Modifier.testTag("workbench-export"))
                        Button("Restore snapshot", {
                            when (val decoded = decodeTheatreSnapshot(transfer)) {
                                is TheatreSnapshotResult.Accepted -> { owned.restore(decoded.snapshot); outcome = "Restored ${decoded.snapshot.recipe.id}" }
                                is TheatreSnapshotResult.UnsupportedVersion -> outcome = "Unsupported snapshot version ${decoded.version}"
                                is TheatreSnapshotResult.UnknownRecipe -> outcome = "Unknown recipe"
                                is TheatreSnapshotResult.Invalid -> outcome = decoded.reason
                            }
                        }, variant = ButtonVariant.Secondary, controlModifier = Modifier.testTag("workbench-restore"))
                    }
                    TextField("Snapshot transfer", transfer, { transfer = it }, Modifier.fillMaxWidth(),
                        controlModifier = Modifier.testTag("workbench-transfer-input"))
                    Text(outcome, Modifier.testTag("workbench-import-result"), style = MaterialTheme.typography.bodySmall)
                    Text("Event log", style = MaterialTheme.typography.titleMedium)
                    SelectionContainer {
                        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                            log.forEach { Text(it, style = MaterialTheme.typography.bodySmall) }
                        }
                    }
                    Text("Memory only. Offline simulates unavailable catalog/writes. Timer is a selection. Native audio, process death and screen readers remain untested.", style = MaterialTheme.typography.bodySmall)
                }
                Box(Modifier.weight(1f).fillMaxSize().horizontalScroll(rememberScrollState()).verticalScroll(rememberScrollState()), contentAlignment = Alignment.TopStart) {
                    key(mount) {
                        CantosDemoApp(
                            modifier = Modifier.requiredSize(settings.constraints.widthDp.dp, settings.constraints.heightDp.dp).testTag("workbench-theatre-frame"),
                            appSession = mount.session, textScale = settings.fontScale, density = settings.density,
                            motionPreference = if (settings.reducedMotion) MotionPreference.Reduced else MotionPreference.Full,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun observeSnapshot(session: DemoAppSession): DemoState {
    val ui by session.ui.collectAsState()
    val navigation by session.container.navigation.collectAsState()
    val catalog by session.container.catalog.state.collectAsState()
    val listening by session.container.listening.state.collectAsState()
    val playback by session.container.playback.state.collectAsState()
    val preferences by session.container.preferences.state.collectAsState()
    return com.cantos.theatre.app.projectDemoState(navigation, catalog, listening, playback, preferences,
        ui.query, ui.libraryTab, ui.timer, session.seed)
}

private fun TheatreRecipe.withLatency(latency: Long): TheatreRecipe =
    (TheatreRecipe.select(id, settings, seed, latency, steps) as TheatreRecipeResult.Accepted).recipe
