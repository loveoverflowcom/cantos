package com.cantos.theatre.app.workbench

import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.adapters.preview.PreviewCatalogScenario
import com.cantos.theatre.app.PreviewOwners
import com.cantos.theatre.app.PreviewSeed
import com.cantos.theatre.adapters.preview.previewLibraryStore
import com.cantos.ui.Theme
import com.cantos.ui.ThemeMode
import com.cantos.ui.preview.workbench.PreviewLocale
import com.cantos.ui.preview.workbench.PreviewSettings
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch

/** One mount owns its owners and replay scope. A borrowed owner set always stays borrowed. */
class TheatreMount internal constructor(
    val recipe: TheatreRecipe,
    val owners: PreviewOwners,
    internal val scope: CoroutineScope,
    private val ownsOwners: Boolean,
) {
    private var closed = false
    internal var closeCount = 0
        private set

    /** The read-only projection of the live owners; it is derived, never stored. */
    fun snapshot(): ShellState = owners.projection()

    /** Re-evaluated whenever any owner the projection reads changes. */
    fun observe(): Flow<ShellState> = with(owners) {
        combine(
            listOf(
                shell.navigation, catalog.state, shell.known, runtime.listening.playback.state, runtime.listening.selection,
                runtime.listening.failure, shell.requested, shell.library, shell.ui, preferences.state,
            ),
        ) { projection() }
    }

    internal fun close() {
        if (closed) return
        closed = true
        closeCount += 1
        scope.cancel()
        if (ownsOwners) owners.close()
    }
}

/**
 * Desktop-only orchestration over the Preview owners. Selection and remount fence callbacks by
 * mount identity and cancel old replay/write work; reset intentionally builds new owners and a
 * new in-memory database. Scenario controls (fixture, reset, catalog state, failure injection)
 * act here, outside the product navigation, and reach the owners only through their ports.
 */
class TheatreWorkbenchController(
    private val parentScope: CoroutineScope,
    initialRecipe: TheatreRecipe = TheatreRecipe.fixture(TheatreFixture.Fresh),
    systemIsDark: Boolean = false,
    private val injectedOwners: PreviewOwners? = null,
) {
    private var closed = false
    private var epoch = 0L
    private var hostIsDark = systemIsDark
    private var replayJob: Job? = null
    private val mutableLog = MutableStateFlow<List<String>>(emptyList())
    val eventLog: StateFlow<List<String>> = mutableLog.asStateFlow()
    private val mutableRecipe = MutableStateFlow(initialRecipe)
    val recipe: StateFlow<TheatreRecipe> = mutableRecipe.asStateFlow()
    private val mutableMount = MutableStateFlow(createMount(initialRecipe, null))
    val mount: StateFlow<TheatreMount> = mutableMount.asStateFlow()
    val selectedRecipe: TheatreRecipe get() = mutableRecipe.value

    private fun createMount(recipe: TheatreRecipe, restored: TheatreSnapshot?): TheatreMount {
        val ownerJob = SupervisorJob(parentScope.coroutineContext[Job])
        val scope = CoroutineScope(parentScope.coroutineContext.minusKey(Job) + ownerJob)
        val initial = recipe.initialSeed(hostIsDark)
        val seed = restored?.let {
            initial.copy(state = it.state, signedIn = it.signedIn)
        } ?: initial
        val owners = injectedOwners ?: PreviewOwners(scope, seed, previewLibraryStore())
        return TheatreMount(recipe, owners, scope, ownsOwners = injectedOwners == null)
    }

    fun select(recipe: TheatreRecipe) {
        if (closed || borrowedUnavailable("select")) return
        replace(recipe, null, "select ${recipe.id}")
    }

    fun resetToSelected() {
        if (closed || borrowedUnavailable("reset")) return
        replace(selectedRecipe, null, "reset-to-selected ${selectedRecipe.id}")
    }

    /** Appearance is an input change, preserving accepted listening/navigation and text values. */
    fun updateSettings(settings: PreviewSettings) {
        if (closed || borrowedUnavailable("settings")) return
        val recipe = (TheatreRecipe.select(selectedRecipe.id, settings, selectedRecipe.seed,
            selectedRecipe.writeLatencyMs, selectedRecipe.steps) as TheatreRecipeResult.Accepted).recipe
        mutableRecipe.value = recipe
        val shell = mutableMount.value.owners.shell
        shell.dispatch(ShellEvent.SetTheme(
            if (settings.preferences(hostIsDark).theme == Theme.Dark) ShellTheme.Dark else ShellTheme.Light))
        shell.dispatch(ShellEvent.SetLocale(
            if (settings.locale == PreviewLocale.En) ShellLocale.English else ShellLocale.Vietnamese))
        append("settings ${recipe.id}")
    }

    fun updateSystemTheme(isDark: Boolean) {
        if (closed || isDark == hostIsDark) return
        hostIsDark = isDark
        if (selectedRecipe.settings.themeMode == ThemeMode.System) updateSettings(selectedRecipe.settings)
    }

    /** Recreates every owner from a synthetic projection, never from a service or scope. */
    fun recreate() {
        if (closed) return
        if (injectedOwners != null) {
            append("recreate unavailable: owner is externally injected")
            return
        }
        val snapshot = snapshot()
        when (val exported = encodeTheatreSnapshot(snapshot)) {
            is TheatreSnapshotExport.Exported -> replace(selectedRecipe, snapshot, "recreate ${selectedRecipe.id}")
            is TheatreSnapshotExport.UnsupportedState -> append("recreate unavailable: ${exported.reason}")
        }
    }

    fun replay() {
        if (closed || borrowedUnavailable("replay")) return
        resetToSelected()
        val current = mutableMount.value
        replayJob = current.scope.launch {
            for (step in current.recipe.steps) {
                // Explicit simulation time, unrelated to native media/timer/background clocks.
                delay(TheatreRecipe.SimulationStepMs)
                if (closed || mutableMount.value !== current) return@launch
                val event = step.event()
                append("intent ${eventLabel(event)}")
                current.owners.shell.dispatch(event)
            }
            if (!closed && mutableMount.value === current) append("replay complete ${current.recipe.steps.size} intents")
        }
    }

    /**
     * Use this identity-fenced callback for host actions, never capture a mutable current owner.
     * Reset and catalog-mode are workbench scenario controls, handled here and never forwarded
     * to the product shell, which has no such events.
     */
    fun callback(): (ShellEvent) -> Unit {
        val current = mutableMount.value
        return { event ->
            if (!closed && mutableMount.value === current) {
                append("intent ${eventLabel(event)}")
                when (event) {
                    ShellEvent.Reset -> resetToSelected()
                    is ShellEvent.CatalogMode -> setCatalogScenario(current, event.mode)
                    else -> current.owners.shell.dispatch(event)
                }
            }
        }
    }

    private fun setCatalogScenario(mount: TheatreMount, mode: ShellCatalogMode) {
        mount.owners.catalogPort.setScenario(when (mode) {
            ShellCatalogMode.Ready -> PreviewCatalogScenario.Ready
            ShellCatalogMode.Loading -> PreviewCatalogScenario.Loading
            ShellCatalogMode.Empty -> PreviewCatalogScenario.Empty
            ShellCatalogMode.Error -> PreviewCatalogScenario.Error
        })
        mount.owners.catalog.dispatch(PublicCatalogIntent.RefreshWorks)
    }

    fun snapshot(): TheatreSnapshot {
        val owners = mutableMount.value.owners
        val library = owners.shell.library.value
        val settled = library.pending == 0 && library.failure == null && library.failures.isEmpty() && !library.refreshing
        return TheatreSnapshot(selectedRecipe, owners.projection(), owners.account.state.value is NativeAccountState.SignedIn, settled)
    }

    fun restore(snapshot: TheatreSnapshot) {
        if (closed) return
        if (injectedOwners != null) {
            append("restore unavailable: owner is externally injected")
            return
        }
        when (val exported = encodeTheatreSnapshot(snapshot)) {
            is TheatreSnapshotExport.Exported -> replace(snapshot.recipe, snapshot, "restore ${snapshot.recipe.id}")
            is TheatreSnapshotExport.UnsupportedState -> append("restore unavailable: ${exported.reason}")
        }
    }

    private fun replace(recipe: TheatreRecipe, state: TheatreSnapshot?, label: String) {
        check(epoch < Long.MAX_VALUE) { "Workbench mount epoch exhausted" }
        epoch += 1L
        replayJob?.cancel()
        replayJob = null
        mutableMount.value.close()
        mutableRecipe.value = recipe
        mutableMount.value = createMount(recipe, state)
        mutableLog.value = listOf(label)
    }

    private fun append(value: String) { mutableLog.value = (mutableLog.value + value).takeLast(40) }

    private fun borrowedUnavailable(action: String): Boolean {
        if (injectedOwners == null) return false
        append("$action unavailable: owner is externally injected")
        return true
    }

    fun close() {
        if (closed) return
        closed = true
        replayJob?.cancel()
        mutableMount.value.close()
    }
}

private fun eventLabel(event: ShellEvent): String = when (event) {
    is ShellEvent.Search -> "Search (${event.query.length} characters)"
    else -> event::class.simpleName ?: "ShellEvent"
}
