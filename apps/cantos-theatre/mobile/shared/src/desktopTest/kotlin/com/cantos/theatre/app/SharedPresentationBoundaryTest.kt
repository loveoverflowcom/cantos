package com.cantos.theatre.app

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellWorkId
import org.junit.Test

/** This module's renderer must work without the diagnostic project or any host providers. */
@OptIn(ExperimentalTestApi::class)
class SharedPresentationBoundaryTest {
    @Test
    fun ordinary_shared_mount_has_preferences_without_sample_notices_or_diagnostic_controls() {
        val state = ShellState(
            selectedEpisode = ShellEpisodeId("published-episode-8", ShellWorkId("published-work-42")),
            stack = listOf(ShellRoute.Discovery, ShellRoute.Settings),
        )
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { CantosPresentation(state, ShellCatalogView(), {}) }
            onNodeWithTag("setting-Theme").assertExists()
            onNodeWithTag("setting-Language").assertExists()
            onNodeWithTag("setting-Diagnostics").assertDoesNotExist()
            onNodeWithTag("demo-notice").assertDoesNotExist()
            onNodeWithTag("reset").assertDoesNotExist()
        }
    }
}
