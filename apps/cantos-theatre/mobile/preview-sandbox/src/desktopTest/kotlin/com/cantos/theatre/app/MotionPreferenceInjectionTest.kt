package com.cantos.theatre.app

import com.cantos.theatre.features.shell.presentation.WithDemoArtwork
import com.cantos.theatre.app.WithDemoShellPresentation
import com.cantos.theatre.demo.demoState
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.ui.LocalUiPreferences
import com.cantos.ui.MotionPreference
import org.junit.Test

/** The mock keeps its Reduced default; a diagnostics host injects Full as a value. No OS signal is read. */
@OptIn(ExperimentalTestApi::class)
class MotionPreferenceInjectionTest {
    private val state = demoState(stack = listOf(ShellRoute.Discovery, ShellRoute.MembershipPreview))

    @Test
    fun the_mock_defaults_to_reduced_motion() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(state, ShellCatalogView(), {}, membershipContent = {
                    Text(LocalUiPreferences.current.motion.name, Modifier.testTag("motion-probe"))
                }) } }
            }
            onNodeWithTag("motion-probe").assertTextEquals("Reduced")
        }
    }

    @Test
    fun a_host_can_inject_full_motion_without_changing_the_default() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(state, ShellCatalogView(), {}, motionPreference = MotionPreference.Full, membershipContent = {
                    Text(LocalUiPreferences.current.motion.name, Modifier.testTag("motion-probe"))
                }) } }
            }
            onNodeWithTag("motion-probe").assertTextEquals("Full")
        }
    }
}
