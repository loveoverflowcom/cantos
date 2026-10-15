package com.cantos.theatre.app.workbench

import com.cantos.theatre.adapters.mock.sandbox.sessions.FakeStorageSession
import com.cantos.theatre.adapters.mock.sandbox.sessions.seed
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoLibraryTab
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoTimer
import com.cantos.theatre.demo.DemoWorkId
import com.cantos.ui.Theme
import com.cantos.ui.preview.workbench.PreviewLocale
import com.cantos.ui.preview.workbench.PreviewSettings

/** Fixed synthetic fixture revision; it never denotes an account or native storage session. */
enum class TheatreFixture(val id: String) {
    Fresh("theatre-fresh-v1"), Restored("theatre-restored-v1"), Empty("theatre-empty-v1"),
    Loading("theatre-loading-v1"), Error("theatre-error-v1"), Offline("theatre-offline-v1"),
}

/** Every step reuses an app intent. Delays belong to the explicit desktop simulation clock. */
enum class TheatreStep {
    OpenSearch, SearchNight, OpenLibrary, SelectSaved, SaveSonHa, OpenSonHa, SeekFifteen,
    Bookmark, ToggleSimulation, TimerFifteen;

    fun event(): DemoEvent = when (this) {
        OpenSearch -> DemoEvent.Navigate(DemoRoute.Search)
        SearchNight -> DemoEvent.Search("Đèn")
        OpenLibrary -> DemoEvent.Navigate(DemoRoute.Library)
        SelectSaved -> DemoEvent.SelectLibrary(DemoLibraryTab.Saved)
        SaveSonHa -> DemoEvent.SetSaved(DemoWorkId.SonHa, true)
        OpenSonHa -> DemoEvent.OpenEpisode(DemoEpisodeId.SonHa01)
        SeekFifteen -> DemoEvent.Seek(15)
        Bookmark -> DemoEvent.Bookmark
        ToggleSimulation -> DemoEvent.ToggleSimulation
        TimerFifteen -> DemoEvent.SetTimer(DemoTimer.Fifteen)
    }
}

sealed interface TheatreRecipeResult {
    data class Accepted(val recipe: TheatreRecipe) : TheatreRecipeResult
    data class UnknownRecipe(val id: String) : TheatreRecipeResult
    data class Invalid(val reason: String) : TheatreRecipeResult
}

/** Preview-only immutable recipe. No backend, file path, auth, scope or engine can be supplied. */
class TheatreRecipe private constructor(
    val fixture: TheatreFixture,
    val settings: PreviewSettings,
    val seed: Int,
    val writeLatencyMs: Long,
    steps: List<TheatreStep>,
) {
    private val script = steps.toList()
    val steps: List<TheatreStep> get() = script.toList()
    val id: String get() = fixture.id

    fun initialState(systemIsDark: Boolean): DemoState {
        val base = when (fixture) {
            TheatreFixture.Restored -> FakeStorageSession.Restored.seed()
            else -> FakeStorageSession.Fresh.seed()
        }
        return base.copy(
            catalogMode = when (fixture) {
                TheatreFixture.Empty -> DemoCatalogMode.Empty
                TheatreFixture.Loading -> DemoCatalogMode.Loading
                TheatreFixture.Error, TheatreFixture.Offline -> DemoCatalogMode.Error
                TheatreFixture.Fresh, TheatreFixture.Restored -> DemoCatalogMode.Ready
            },
            theme = if (settings.preferences(systemIsDark).theme == Theme.Dark) DemoTheme.Dark else DemoTheme.Light,
            locale = if (settings.locale == PreviewLocale.En) DemoLocale.English else DemoLocale.Vietnamese,
        )
    }

    companion object {
        const val Version = 1
        const val SimulationStepMs = 50L
        fun select(
            id: String,
            settings: PreviewSettings = PreviewSettings(),
            seed: Int = 1,
            writeLatencyMs: Long = 0L,
            steps: List<TheatreStep> = listOf(TheatreStep.OpenSearch, TheatreStep.SearchNight),
        ): TheatreRecipeResult {
            val fixture = TheatreFixture.entries.firstOrNull { it.id == id } ?: return TheatreRecipeResult.UnknownRecipe(id)
            if (seed != 1) return TheatreRecipeResult.Invalid("Only synthetic fixture seed 1 is supported")
            if (writeLatencyMs !in 0L..5000L) return TheatreRecipeResult.Invalid("Write latency must be 0..5000 simulation ms")
            if (steps.size > 40) return TheatreRecipeResult.Invalid("A recipe allows at most 40 intents")
            return TheatreRecipeResult.Accepted(TheatreRecipe(fixture, settings, seed, writeLatencyMs, steps))
        }

        fun fixture(fixture: TheatreFixture, settings: PreviewSettings = PreviewSettings()): TheatreRecipe =
            (select(fixture.id, settings) as TheatreRecipeResult.Accepted).recipe
    }
}
