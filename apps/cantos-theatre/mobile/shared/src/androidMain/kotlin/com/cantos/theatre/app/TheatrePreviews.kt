package com.cantos.theatre.app

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoProgress
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoWorkId

/** Thin wrappers around the app composition; no network, engine or alternate screen copy. */
@Preview(name = "Discovery · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreDiscoveryLightPreview() {
    CantosDemoApp()
}

@Preview(name = "Player · vi · dark", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatrePlayerDarkPreview() {
    CantosDemoApp(initialState = DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Player), theme = DemoTheme.Dark))
}

@Preview(name = "Work · vi · 2x narrow", widthDp = 320, heightDp = 800, locale = "vi", fontScale = 2f)
@Composable
fun TheatreWorkNarrowLargePreview() {
    CantosDemoApp(initialState = DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Work(DemoWorkId.SonHa))))
}

@Preview(name = "Discovery · vi · wide", widthDp = 840, heightDp = 840, locale = "vi")
@Composable
fun TheatreDiscoveryWidePreview() {
    CantosDemoApp()
}

@Preview(name = "Error · vi · dark", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreErrorDarkPreview() {
    CantosDemoApp(initialState = DemoState(catalogMode = DemoCatalogMode.Error, theme = DemoTheme.Dark))
}

@Preview(name = "My library · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreLibraryLightPreview() {
    CantosDemoApp(initialState = DemoState(
        stack = listOf(DemoRoute.Library),
        history = listOf(DemoProgress(DemoEpisodeId.SonHa03, 768)),
    ))
}

@Preview(name = "My library · vi · dark · 2x narrow", widthDp = 320, heightDp = 800, locale = "vi", fontScale = 2f)
@Composable
fun TheatreLibraryNarrowLargePreview() {
    CantosDemoApp(initialState = DemoState(stack = listOf(DemoRoute.Library), theme = DemoTheme.Dark))
}

@Preview(name = "History · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreHistoryLightPreview() {
    CantosDemoApp(initialState = DemoState(
        stack = listOf(DemoRoute.Library, DemoRoute.History),
        history = listOf(DemoProgress(DemoEpisodeId.SonHa03, 768)),
    ))
}

/** Static storage-simulator fixtures; a preview never simulates a process restart. */
@Preview(name = "Fresh session · fake storage · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreFreshSessionPreview() {
    FakeStorageSessionApp(FakeStorageSession.Fresh)
}

@Preview(name = "Restored session · fake storage · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreRestoredSessionPreview() {
    FakeStorageSessionApp(FakeStorageSession.Restored)
}
