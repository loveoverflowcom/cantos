package com.cantos.theatre.app.workbench

import com.cantos.theatre.app.mockAppContainer
import com.cantos.theatre.adapters.mock.sandbox.database.MockStorageConfig
import com.cantos.theatre.adapters.mock.sandbox.database.MockStorageDatabase
import com.cantos.theatre.app.AppContainer
import com.cantos.theatre.app.DemoAppSession
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.Theme
import com.cantos.ui.ThemeMode
import com.cantos.ui.preview.workbench.PreviewSettings
import com.cantos.ui.preview.workbench.PreviewLocale
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** One mount owns its binding and replay scope. A borrowed app owner always stays borrowed. */
class TheatreMount internal constructor(
    val recipe: TheatreRecipe,
    val session: DemoAppSession,
    val database: MockStorageDatabase?,
    internal val scope: CoroutineScope,
    private val ownsContainer: Boolean,
) {
    private var closed = false
    internal var closeCount = 0
        private set

    internal fun close() {
        if (closed) return
        closed = true
        closeCount += 1
        session.close()
        scope.cancel()
        if (ownsContainer) session.container.close()
    }
}

/**
 * Desktop-only orchestration over existing mock ports. Selection/remount fences callbacks by
 * mount identity and cancels old replay/write work; reset intentionally makes a new database.
 */
class TheatreWorkbenchController(
    private val parentScope: CoroutineScope,
    initialRecipe: TheatreRecipe = TheatreRecipe.fixture(TheatreFixture.Fresh),
    systemIsDark: Boolean = false,
    private val injectedContainer: AppContainer? = null,
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

    private fun createMount(recipe: TheatreRecipe, restored: ShellState?): TheatreMount {
        val ownerJob = SupervisorJob(parentScope.coroutineContext[Job])
        val scope = CoroutineScope(parentScope.coroutineContext.minusKey(Job) + ownerJob)
        val seed = restored ?: recipe.initialState(hostIsDark)
        var database: MockStorageDatabase? = null
        val app = injectedContainer ?: mockAppContainer(scope, seed,
            progressConfig = MockStorageConfig(writeLatencyMs = recipe.writeLatencyMs)) { start ->
            MockStorageDatabase(start).also {
                database = it
                if (recipe.fixture == TheatreFixture.Offline) it.failAllWrites()
            }
        }
        val generation = epoch
        val session = DemoAppSession(app, seed, resetSelected = ::resetToSelected, onEvent = { event ->
            if (!closed && generation == epoch) append("intent ${eventLabel(event)}")
        })
        return TheatreMount(recipe, session, database, scope, ownsContainer = injectedContainer == null)
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
        mutableMount.value.session.dispatch(ShellEvent.SetTheme(
            if (settings.preferences(hostIsDark).theme == Theme.Dark) ShellTheme.Dark else ShellTheme.Light))
        mutableMount.value.session.dispatch(ShellEvent.SetLocale(
            if (settings.locale == PreviewLocale.En) ShellLocale.English else ShellLocale.Vietnamese))
        append("settings ${recipe.id}")
    }

    fun updateSystemTheme(isDark: Boolean) {
        if (closed || isDark == hostIsDark) return
        hostIsDark = isDark
        if (selectedRecipe.settings.themeMode == ThemeMode.System) updateSettings(selectedRecipe.settings)
    }

    /** Recreates app/controllers from a synthetic projection, never from a service or scope. */
    fun recreate() {
        if (closed) return
        if (injectedContainer != null) {
            append("recreate unavailable: owner is externally injected")
            return
        }
        val snapshot = snapshot()
        when (val exported = encodeTheatreSnapshot(snapshot)) {
            is TheatreSnapshotExport.Exported -> replace(selectedRecipe, snapshot.state, "recreate ${selectedRecipe.id}")
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
                current.session.dispatch(step.event())
            }
            if (!closed && mutableMount.value === current) append("replay complete ${current.recipe.steps.size} intents")
        }
    }

    /** Use this identity-fenced callback for host actions, never capture a mutable current owner. */
    fun callback(): (ShellEvent) -> Unit {
        val current = mutableMount.value
        return { event -> if (!closed && mutableMount.value === current) current.session.dispatch(event) }
    }

    fun snapshot(): TheatreSnapshot = TheatreSnapshot(selectedRecipe, mutableMount.value.session.snapshot())

    fun restore(snapshot: TheatreSnapshot) {
        if (closed) return
        if (injectedContainer != null) {
            append("restore unavailable: owner is externally injected")
            return
        }
        when (val exported = encodeTheatreSnapshot(snapshot)) {
            is TheatreSnapshotExport.Exported -> replace(snapshot.recipe, snapshot.state, "restore ${snapshot.recipe.id}")
            is TheatreSnapshotExport.UnsupportedState -> append("restore unavailable: ${exported.reason}")
        }
    }

    private fun replace(recipe: TheatreRecipe, state: ShellState?, label: String) {
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
        if (injectedContainer == null) return false
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
