package com.cantos.theatre.app

import com.cantos.theatre.demo.demoState
import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.demo.DemoEpisodeIds
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import com.cantos.theatre.adapters.mock.sandbox.sessions.MockStorageSession
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellWorkId

/** Thin wrappers around the app composition; no network, engine or alternate screen copy. */
@Preview(name = "Discovery · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreDiscoveryLightPreview() {
    CantosDemoApp()
}

@Preview(name = "Player · vi · dark", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatrePlayerDarkPreview() {
    CantosDemoApp(initialState = demoState(stack = listOf(ShellRoute.Discovery, ShellRoute.Player), theme = ShellTheme.Dark))
}

@Preview(name = "Work · vi · 2x narrow", widthDp = 320, heightDp = 800, locale = "vi", fontScale = 2f)
@Composable
fun TheatreWorkNarrowLargePreview() {
    CantosDemoApp(initialState = demoState(stack = listOf(ShellRoute.Discovery, ShellRoute.Work(DemoWorkIds.SonHa))))
}

@Preview(name = "Discovery · vi · wide", widthDp = 840, heightDp = 840, locale = "vi")
@Composable
fun TheatreDiscoveryWidePreview() {
    CantosDemoApp()
}

@Preview(name = "Error · vi · dark", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreErrorDarkPreview() {
    CantosDemoApp(initialState = demoState(catalogMode = ShellCatalogMode.Error, theme = ShellTheme.Dark))
}

@Preview(name = "My library · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreLibraryLightPreview() {
    CantosDemoApp(initialState = demoState(
        stack = listOf(ShellRoute.Library),
        history = listOf(ShellProgress(DemoEpisodeIds.SonHa03, 768)),
    ))
}

@Preview(name = "My library · vi · dark · 2x narrow", widthDp = 320, heightDp = 800, locale = "vi", fontScale = 2f)
@Composable
fun TheatreLibraryNarrowLargePreview() {
    CantosDemoApp(initialState = demoState(stack = listOf(ShellRoute.Library), theme = ShellTheme.Dark))
}

@Preview(name = "History · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreHistoryLightPreview() {
    CantosDemoApp(initialState = demoState(
        stack = listOf(ShellRoute.Library, ShellRoute.History),
        history = listOf(ShellProgress(DemoEpisodeIds.SonHa03, 768)),
    ))
}

/** Static storage-simulator fixtures; a preview never simulates a process restart. */
@Preview(name = "Fresh session · mock storage · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreFreshSessionPreview() {
    MockStorageSessionApp(MockStorageSession.Fresh)
}

@Preview(name = "Restored session · mock storage · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreRestoredSessionPreview() {
    MockStorageSessionApp(MockStorageSession.Restored)
}

/** Pending local initialization is a static preview input, never a native-host timer. */
@Preview(name = "Local launch · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreLocalLaunchLightPreview() {
    CantosLocalLaunch(false, UiPreferences(reducedMotion = true)) {}
}

@Preview(name = "Local launch · vi · dark · 2x", widthDp = 320, heightDp = 568, locale = "vi", fontScale = 2f)
@Composable
fun TheatreLocalLaunchLargePreview() {
    CantosLocalLaunch(false, UiPreferences(theme = Theme.Dark, reducedMotion = true)) {}
}
