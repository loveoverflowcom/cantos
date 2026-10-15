package com.cantos.theatre.app.workbench

import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.demo.DemoEpisodeIds
import com.cantos.theatre.adapters.mock.sandbox.sessions.MockStorageSession
import com.cantos.theatre.adapters.mock.sandbox.sessions.seed
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.ShellWorkId
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

    fun event(): ShellEvent = when (this) {
        OpenSearch -> ShellEvent.Navigate(ShellRoute.Search)
        SearchNight -> ShellEvent.Search("Đèn")
        OpenLibrary -> ShellEvent.Navigate(ShellRoute.Library)
        SelectSaved -> ShellEvent.SelectLibrary(ShellLibraryTab.Saved)
        SaveSonHa -> ShellEvent.SetSaved(DemoWorkIds.SonHa, true)
        OpenSonHa -> ShellEvent.OpenEpisode(DemoEpisodeIds.SonHa01)
        SeekFifteen -> ShellEvent.Seek(15)
        Bookmark -> ShellEvent.Bookmark
        ToggleSimulation -> ShellEvent.ToggleSimulation
        TimerFifteen -> ShellEvent.SetTimer(ShellTimer.Fifteen)
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

    fun initialState(systemIsDark: Boolean): ShellState {
        val base = when (fixture) {
            TheatreFixture.Restored -> MockStorageSession.Restored.seed()
            else -> MockStorageSession.Fresh.seed()
        }
        return base.copy(
            catalogMode = when (fixture) {
                TheatreFixture.Empty -> ShellCatalogMode.Empty
                TheatreFixture.Loading -> ShellCatalogMode.Loading
                TheatreFixture.Error, TheatreFixture.Offline -> ShellCatalogMode.Error
                TheatreFixture.Fresh, TheatreFixture.Restored -> ShellCatalogMode.Ready
            },
            theme = if (settings.preferences(systemIsDark).theme == Theme.Dark) ShellTheme.Dark else ShellTheme.Light,
            locale = if (settings.locale == PreviewLocale.En) ShellLocale.English else ShellLocale.Vietnamese,
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
